use gtk::prelude::*;
use gtk4 as gtk;
use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::env;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

#[path = "../session.rs"]
mod session;

const CSS: &str = r#"
window {
  background: #101318;
  color: #e8f5ff;
}
.tdm-title {
  color: #f4f8ff;
  font-size: 30px;
  font-weight: 800;
  letter-spacing: 2px;
}
.tdm-subtitle {
  color: #8da3b8;
  font-size: 13px;
}
.tdm-panel {
  background: #191f28;
  border: 1px solid #263b4c;
  border-radius: 18px;
  padding: 28px;
}
.tdm-accent {
  color: #37d9ff;
  font-weight: 700;
}
entry, dropdown {
  background: #10161e;
  color: #f4f8ff;
  border: 1px solid #304657;
  border-radius: 9px;
  padding: 10px;
  min-height: 22px;
}
button {
  background: #123c50;
  color: #eafaff;
  border: 1px solid #2785a4;
  border-radius: 9px;
  padding: 10px 18px;
  font-weight: 700;
}
button:hover {
  background: #18546d;
}
button:disabled {
  background: #252b33;
  color: #74808d;
  border-color: #333b45;
}
.tdm-status {
  color: #ffc66d;
  font-size: 12px;
}
"#;

enum UiEvent {
    Prompt { message: String, secret: bool },
    Status(String),
    SessionStarted,
    Finished(String),
    Failed(String),
}

enum UserReply {
    Answer(Option<String>),
    Cancel,
}

enum AuthOutcome {
    Started,
    Cancelled,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Request {
    CreateSession { username: String },
    PostAuthMessageResponse { response: Option<String> },
    StartSession { cmd: Vec<String>, env: Vec<String> },
    CancelSession,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Response {
    Success,
    Error {
        error_type: ErrorType,
        description: String,
    },
    AuthMessage {
        auth_message_type: AuthMessageType,
        auth_message: String,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum ErrorType {
    Error,
    AuthError,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum AuthMessageType {
    Visible,
    Secret,
    Info,
    Error,
}

fn send_request<W: Write>(stream: &mut W, request: Request) -> Result<(), String> {
    let payload = serde_json::to_vec(&request)
        .map_err(|error| format!("Could not encode greetd request: {error}"))?;
    let length =
        u32::try_from(payload.len()).map_err(|_| "greetd request is too large".to_owned())?;
    stream
        .write_all(&length.to_ne_bytes())
        .and_then(|()| stream.write_all(&payload))
        .and_then(|()| stream.flush())
        .map_err(|error| format!("Could not send greetd request: {error}"))
}

fn read_response<R: Read>(stream: &mut R) -> Result<Response, String> {
    const MAX_RESPONSE_BYTES: u32 = 1024 * 1024;
    let mut length_bytes = [0_u8; 4];
    stream
        .read_exact(&mut length_bytes)
        .map_err(|error| format!("Could not read greetd response length: {error}"))?;
    let length = u32::from_ne_bytes(length_bytes);
    if length == 0 || length > MAX_RESPONSE_BYTES {
        return Err(format!("greetd response length is invalid: {length} bytes"));
    }

    let mut payload = vec![0_u8; length as usize];
    stream
        .read_exact(&mut payload)
        .map_err(|error| format!("Could not read greetd response payload: {error}"))?;
    serde_json::from_slice(&payload)
        .map_err(|error| format!("Could not decode greetd response: {error}"))
}

fn cancel_greetd_session(stream: &mut UnixStream) {
    let _ = send_request(stream, Request::CancelSession);
    let _ = read_response(stream);
}

fn run_authentication(
    username: String,
    selected_session: session::Session,
    events: Sender<UiEvent>,
    replies: Receiver<UserReply>,
) {
    let result = (|| -> Result<AuthOutcome, String> {
        let socket = env::var("GREETD_SOCK")
            .map_err(|_| "GREETD_SOCK is not set. Start TDM through greetd.".to_owned())?;
        let mut stream = UnixStream::connect(&socket)
            .map_err(|error| format!("Cannot connect to greetd: {error}"))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(120)))
            .map_err(|error| format!("Cannot set greetd read timeout: {error}"))?;
        stream
            .set_write_timeout(Some(Duration::from_secs(30)))
            .map_err(|error| format!("Cannot set greetd write timeout: {error}"))?;

        send_request(&mut stream, Request::CreateSession { username })?;

        loop {
            match read_response(&mut stream)? {
                Response::Success => {
                    let session_env = vec![format!(
                        "XDG_SESSION_TYPE={}",
                        selected_session.kind.env_value()
                    )];
                    send_request(
                        &mut stream,
                        Request::StartSession {
                            cmd: selected_session.command.clone(),
                            env: session_env,
                        },
                    )?;

                    match read_response(&mut stream)? {
                        Response::Success => return Ok(AuthOutcome::Started),
                        Response::Error { description, .. } => {
                            return Err(format!("Could not start desktop session: {description}"));
                        }
                        Response::AuthMessage { .. } => {
                            return Err(
                                "Unexpected authentication prompt while starting the session."
                                    .to_owned(),
                            );
                        }
                    }
                }
                Response::Error {
                    error_type: ErrorType::AuthError,
                    description,
                } => return Err(format!("Authentication failed: {description}")),
                Response::Error {
                    error_type: ErrorType::Error,
                    description,
                } => return Err(format!("greetd error: {description}")),
                Response::AuthMessage {
                    auth_message_type: AuthMessageType::Info,
                    auth_message,
                } => {
                    let _ = events.send(UiEvent::Status(auth_message));
                    send_request(
                        &mut stream,
                        Request::PostAuthMessageResponse { response: None },
                    )?;
                }
                Response::AuthMessage {
                    auth_message_type: AuthMessageType::Error,
                    auth_message,
                } => {
                    let _ = events.send(UiEvent::Status(auth_message));
                    send_request(
                        &mut stream,
                        Request::PostAuthMessageResponse { response: None },
                    )?;
                }
                Response::AuthMessage {
                    auth_message_type,
                    auth_message,
                } => {
                    let secret = matches!(auth_message_type, AuthMessageType::Secret);
                    events
                        .send(UiEvent::Prompt {
                            message: auth_message,
                            secret,
                        })
                        .map_err(|_| "Greeter window closed.".to_owned())?;

                    let answer = match replies.recv_timeout(Duration::from_secs(120)) {
                        Ok(UserReply::Answer(answer)) => answer,
                        Ok(UserReply::Cancel) => {
                            cancel_greetd_session(&mut stream);
                            return Ok(AuthOutcome::Cancelled);
                        }
                        Err(RecvTimeoutError::Timeout) => {
                            cancel_greetd_session(&mut stream);
                            return Err("Authentication timed out. Please try again.".to_owned());
                        }
                        Err(RecvTimeoutError::Disconnected) => {
                            cancel_greetd_session(&mut stream);
                            return Err("Greeter closed during authentication.".to_owned());
                        }
                    };

                    send_request(
                        &mut stream,
                        Request::PostAuthMessageResponse { response: answer },
                    )?;
                }
            }
        }
    })();

    match result {
        Ok(AuthOutcome::Started) => {
            let _ = events.send(UiEvent::SessionStarted);
        }
        Ok(AuthOutcome::Cancelled) => {
            let _ = events.send(UiEvent::Finished("Sign-in cancelled.".to_owned()));
        }
        Err(message) => {
            let _ = events.send(UiEvent::Failed(message));
        }
    }
}

fn build_window(app: &gtk::Application) {
    let provider = gtk::CssProvider::new();
    provider.load_from_data(CSS);
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("TomOS Display Manager")
        .default_width(900)
        .default_height(620)
        .resizable(true)
        .build();

    let outer = gtk::Box::new(gtk::Orientation::Vertical, 18);
    outer.set_halign(gtk::Align::Center);
    outer.set_valign(gtk::Align::Center);
    outer.set_margin_top(32);
    outer.set_margin_bottom(32);
    outer.set_margin_start(24);
    outer.set_margin_end(24);

    let brand = gtk::Label::new(Some("TOMOS"));
    brand.add_css_class("tdm-title");
    let subtitle = gtk::Label::new(Some("A calmer way into your Linux desktop"));

    let panel = gtk::Box::new(gtk::Orientation::Vertical, 14);
    panel.add_css_class("tdm-panel");
    panel.set_width_request(380);

    let heading = gtk::Label::new(Some("Welcome back"));
    heading.add_css_class("tdm-accent");
    heading.set_halign(gtk::Align::Start);

    let username_label = gtk::Label::new(Some("USERNAME"));
    username_label.set_halign(gtk::Align::Start);
    username_label.add_css_class("tdm-subtitle");
    let username = gtk::Entry::new();
    username.set_placeholder_text(Some("Your username"));
    username.set_hexpand(true);

    let session_label = gtk::Label::new(Some("DESKTOP SESSION"));
    session_label.set_halign(gtk::Align::Start);
    session_label.add_css_class("tdm-subtitle");
    let (sessions, session_warnings) = session::discover_sessions();
    let session_labels: Vec<String> = sessions
        .iter()
        .map(|item| format!("{} ({})", item.name, item.kind.label()))
        .collect();
    let session_label_refs: Vec<&str> = session_labels.iter().map(String::as_str).collect();
    let session_dropdown = if session_label_refs.is_empty() {
        gtk::DropDown::from_strings(&["No desktop sessions found"])
    } else {
        gtk::DropDown::from_strings(&session_label_refs)
    };
    session_dropdown.set_hexpand(true);
    session_dropdown.set_sensitive(!sessions.is_empty());
    let sessions = Rc::new(sessions);

    let prompt_label = gtk::Label::new(Some("AUTHENTICATION"));
    prompt_label.set_halign(gtk::Align::Start);
    prompt_label.add_css_class("tdm-subtitle");
    let prompt_entry = gtk::Entry::new();
    prompt_entry.set_placeholder_text(Some("Authentication response"));
    prompt_entry.set_visibility(false);
    prompt_entry.set_hexpand(true);

    let initial_status = if sessions.is_empty() {
        "No usable Wayland or X11 desktop sessions were found."
    } else if !session_warnings.is_empty() {
        "Choose a desktop session. Some session files could not be read; see the system log."
    } else {
        "Choose a desktop session and sign in using your system PAM account."
    };
    let status = gtk::Label::new(Some(initial_status));
    status.add_css_class("tdm-status");
    status.set_wrap(true);
    status.set_halign(gtk::Align::Start);

    let login = gtk::Button::with_label("Sign in");
    login.add_css_class("suggested-action");
    login.set_hexpand(true);
    login.set_sensitive(!sessions.is_empty());

    let cancel = gtk::Button::with_label("Cancel");
    cancel.set_sensitive(false);

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    actions.append(&login);
    actions.append(&cancel);

    let username_for_login = username.clone();
    let prompt_for_login = prompt_entry.clone();
    let prompt_label_for_login = prompt_label.clone();
    let status_for_login = status.clone();
    let login_for_login = login.clone();
    let cancel_for_login = cancel.clone();
    let sessions_for_login = sessions.clone();
    let dropdown_for_login = session_dropdown.clone();
    let busy = Rc::new(Cell::new(false));
    let reply_sender: Rc<RefCell<Option<Sender<UserReply>>>> = Rc::new(RefCell::new(None));
    let (event_tx, event_rx): (Sender<UiEvent>, Receiver<UiEvent>) = mpsc::channel();

    let busy_for_click = busy.clone();
    let sender_for_click = reply_sender.clone();
    login.connect_clicked(move |_| {
        if busy_for_click.get() {
            let answer = prompt_for_login.text().to_string();
            prompt_for_login.set_text("");
            if let Some(sender) = sender_for_click.borrow().as_ref() {
                let _ = sender.send(UserReply::Answer(Some(answer)));
            }
            login_for_login.set_sensitive(false);
            status_for_login.set_text("Checking response…");
            return;
        }

        let name = username_for_login.text().trim().to_owned();
        if name.is_empty() {
            status_for_login.set_text("Enter your Linux username first.");
            return;
        }

        let selected_index = dropdown_for_login.selected() as usize;
        let Some(selected_session) = sessions_for_login.get(selected_index).cloned() else {
            status_for_login.set_text("Select an available desktop session.");
            return;
        };

        let (reply_tx, reply_rx) = mpsc::channel();
        *sender_for_click.borrow_mut() = Some(reply_tx);
        busy_for_click.set(true);
        prompt_label_for_login.set_text("AUTHENTICATION");
        prompt_for_login.set_text("");
        prompt_for_login.set_visibility(false);
        login_for_login.set_label("Waiting for prompt…");
        login_for_login.set_sensitive(false);
        cancel_for_login.set_sensitive(true);
        status_for_login.set_text("Connecting to greetd…");

        let events = event_tx.clone();
        thread::spawn(move || run_authentication(name, selected_session, events, reply_rx));
    });

    let sender_for_cancel = reply_sender.clone();
    let status_for_cancel = status.clone();
    let cancel_for_cancel = cancel.clone();
    cancel.connect_clicked(move |_| {
        if let Some(sender) = sender_for_cancel.borrow().as_ref() {
            let _ = sender.send(UserReply::Cancel);
        }
        status_for_cancel.set_text("Cancelling sign-in…");
        cancel_for_cancel.set_sensitive(false);
    });

    let busy_for_events = busy.clone();
    let sender_for_events = reply_sender.clone();
    let login_for_events = login.clone();
    let cancel_for_events = cancel.clone();
    let prompt_for_events = prompt_entry.clone();
    let prompt_label_for_events = prompt_label.clone();
    let status_for_events = status.clone();
    let app_for_events = app.clone();
    gtk::glib::timeout_add_local(Duration::from_millis(100), move || {
        while let Ok(event) = event_rx.try_recv() {
            match event {
                UiEvent::Prompt { message, secret } => {
                    prompt_label_for_events.set_text(&message);
                    prompt_for_events.set_visibility(!secret);
                    prompt_for_events.set_text("");
                    prompt_for_events.set_placeholder_text(Some(if secret {
                        "Secret response"
                    } else {
                        "Your response"
                    }));
                    login_for_events.set_label("Send response");
                    login_for_events.set_sensitive(true);
                    status_for_events
                        .set_text("Your system authentication service requested a response.");
                    prompt_for_events.grab_focus();
                }
                UiEvent::Status(message) => {
                    status_for_events.set_text(&message);
                }
                UiEvent::SessionStarted => {
                    prompt_for_events.set_text("");
                    status_for_events.set_text("Desktop session accepted. Starting your desktop…");
                    app_for_events.quit();
                    return gtk::glib::ControlFlow::Break;
                }
                UiEvent::Finished(message) => {
                    busy_for_events.set(false);
                    *sender_for_events.borrow_mut() = None;
                    prompt_for_events.set_text("");
                    prompt_label_for_events.set_text("AUTHENTICATION");
                    login_for_events.set_label("Sign in");
                    login_for_events.set_sensitive(true);
                    cancel_for_events.set_sensitive(false);
                    status_for_events.set_text(&message);
                }
                UiEvent::Failed(message) => {
                    busy_for_events.set(false);
                    *sender_for_events.borrow_mut() = None;
                    prompt_for_events.set_text("");
                    prompt_label_for_events.set_text("AUTHENTICATION");
                    login_for_events.set_label("Try again");
                    login_for_events.set_sensitive(true);
                    cancel_for_events.set_sensitive(false);
                    status_for_events.set_text(&message);
                }
            }
        }
        gtk::glib::ControlFlow::Continue
    });

    let sender_for_close = reply_sender.clone();
    window.connect_close_request(move |_| {
        if let Some(sender) = sender_for_close.borrow().as_ref() {
            let _ = sender.send(UserReply::Cancel);
        }
        gtk::glib::Propagation::Proceed
    });

    panel.append(&heading);
    panel.append(&username_label);
    panel.append(&username);
    panel.append(&session_label);
    panel.append(&session_dropdown);
    panel.append(&prompt_label);
    panel.append(&prompt_entry);
    panel.append(&actions);
    panel.append(&status);

    let footer = gtk::Label::new(Some("TDM 0.0.6  •  WAYLAND / X11 SESSION LAUNCHING"));
    footer.add_css_class("tdm-subtitle");

    outer.append(&brand);
    outer.append(&subtitle);
    outer.append(&panel);
    outer.append(&footer);
    window.set_child(Some(&outer));
    window.present();
}

fn main() -> gtk::glib::ExitCode {
    let app = gtk::Application::builder()
        .application_id("org.tomos.display-manager.greeter")
        .build();

    app.connect_activate(build_window);
    app.run()
}

#[cfg(test)]
mod greetd_protocol_tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn decodes_secret_authentication_prompt_with_protocol_field_names() {
        let json =
            br#"{"type":"auth_message","auth_message_type":"secret","auth_message":"Password:"}"#;
        let mut frame = Vec::new();
        frame.extend_from_slice(&(json.len() as u32).to_ne_bytes());
        frame.extend_from_slice(json);

        match read_response(&mut Cursor::new(frame)).unwrap() {
            Response::AuthMessage {
                auth_message_type: AuthMessageType::Secret,
                auth_message,
            } => assert_eq!(auth_message, "Password:"),
            _ => panic!("expected secret authentication prompt"),
        }
    }

    #[test]
    fn encodes_start_session_request_as_argv_and_environment() {
        let mut frame = Cursor::new(Vec::new());
        send_request(
            &mut frame,
            Request::StartSession {
                cmd: vec!["sway".to_owned(), "--unsupported-test-flag".to_owned()],
                env: vec!["XDG_SESSION_TYPE=wayland".to_owned()],
            },
        )
        .unwrap();

        let bytes = frame.into_inner();
        let length = u32::from_ne_bytes(bytes[..4].try_into().unwrap()) as usize;
        let request: serde_json::Value = serde_json::from_slice(&bytes[4..4 + length]).unwrap();
        assert_eq!(request["type"], "start_session");
        assert_eq!(request["cmd"][0], "sway");
        assert_eq!(request["env"][0], "XDG_SESSION_TYPE=wayland");
    }

    #[test]
    fn rejects_oversized_greetd_response() {
        let mut frame = Cursor::new((MAX_RESPONSE_TEST + 1).to_ne_bytes().to_vec());
        assert!(read_response(&mut frame).is_err());
    }

    const MAX_RESPONSE_TEST: u32 = 1024 * 1024;
}
