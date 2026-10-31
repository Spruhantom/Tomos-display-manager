use gtk::prelude::*;
use gtk4 as gtk;
use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::env;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::process::Command;
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
*:focus {
  outline: 3px solid #37d9ff;
  outline-offset: 2px;
}
"#;

const LIGHT_CSS: &str = r#"
window { background: #f5f7fa; color: #17212b; }
.tdm-title { color: #10202d; font-size: 30px; font-weight: 800; letter-spacing: 2px; }
.tdm-subtitle { color: #40566a; font-size: 13px; }
.tdm-panel { background: #ffffff; border: 1px solid #8ca2b5; border-radius: 18px; padding: 28px; }
.tdm-accent { color: #006b88; font-weight: 700; }
entry, dropdown { background: #ffffff; color: #17212b; border: 1px solid #536b7e; border-radius: 9px; padding: 10px; min-height: 22px; }
button { background: #dceef5; color: #10202d; border: 1px solid #28627a; border-radius: 9px; padding: 10px 18px; font-weight: 700; }
button:hover { background: #c3e3ef; }
button:disabled { background: #e3e7eb; color: #59636d; border-color: #89939d; }
.tdm-status { color: #714500; font-size: 12px; }
*:focus { outline: 3px solid #005f86; outline-offset: 2px; }
"#;

const HIGH_CONTRAST_CSS: &str = r#"
window { background: #000000; color: #ffffff; }
.tdm-title { color: #ffffff; font-size: 32px; font-weight: 900; }
.tdm-subtitle { color: #ffffff; font-size: 15px; }
.tdm-panel { background: #000000; border: 2px solid #ffffff; border-radius: 8px; padding: 28px; }
.tdm-accent { color: #ffff00; font-weight: 900; }
entry, dropdown { background: #000000; color: #ffffff; border: 2px solid #ffffff; border-radius: 4px; padding: 10px; min-height: 24px; }
button { background: #000000; color: #ffff00; border: 2px solid #ffff00; border-radius: 4px; padding: 10px 18px; font-weight: 900; }
button:hover { background: #222222; }
button:disabled { background: #000000; color: #aaaaaa; border-color: #aaaaaa; }
.tdm-status { color: #ffff00; font-size: 14px; }
*:focus { outline: 4px solid #00ffff; outline-offset: 3px; }
"#;

fn css_for_theme(theme: &str) -> &'static str {
    match theme {
        "light" => LIGHT_CSS,
        "high-contrast" => HIGH_CONTRAST_CSS,
        _ => CSS,
    }
}

fn configured_css() -> &'static str {
    let configured = env::var("TDM_THEME").ok().or_else(|| {
        std::fs::read_to_string("/etc/tdm/greeter.conf")
            .ok()
            .and_then(|contents| {
                contents.lines().find_map(|line| {
                    let (key, value) = line.split_once('=')?;
                    (key.trim() == "theme").then(|| value.trim().to_owned())
                })
            })
    });
    css_for_theme(configured.as_deref().unwrap_or("dark"))
}

fn connect_power_control(
    button: &gtk::Button,
    window: &gtk::ApplicationWindow,
    status: &gtk::Label,
    action: &'static str,
    description: &'static str,
) {
    let parent = window.clone();
    let status_label = status.clone();
    button.connect_clicked(move |_| {
        let dialog = gtk::Dialog::builder()
            .transient_for(&parent)
            .modal(true)
            .title("Confirm power action")
            .build();
        dialog.add_button("Cancel", gtk::ResponseType::Cancel);
        dialog.add_button("Continue", gtk::ResponseType::Ok);
        dialog.content_area().append(&gtk::Label::new(Some(description)));
        let status_label = status_label.clone();
        dialog.connect_response(move |dialog, response| {
            if response == gtk::ResponseType::Ok {
                match Command::new("loginctl").arg(action).status() {
                    Ok(result) if result.success() => {
                        status_label.set_text("Power request accepted by systemd-logind.");
                    }
                    Ok(result) => {
                        status_label.set_text(&format!("Power request was denied (exit status {}). Check system authorization.", result));
                    }
                    Err(error) => {
                        status_label.set_text(&format!("Could not request power action: {error}"));
                    }
                }
            }
            dialog.close();
        });
        dialog.present();
    });
}

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
    let socket = match env::var("GREETD_SOCK") {
        Ok(socket) => socket,
        Err(_) => {
            let _ = events.send(UiEvent::Failed(
                "GREETD_SOCK is not set. Start TDM through greetd.".to_owned(),
            ));
            return;
        }
    };
    run_authentication_at(&socket, username, selected_session, events, replies);
}

fn run_authentication_at(
    socket: &str,
    username: String,
    selected_session: session::Session,
    events: Sender<UiEvent>,
    replies: Receiver<UserReply>,
) {
    let result = (|| -> Result<AuthOutcome, String> {
        let mut stream = UnixStream::connect(socket)
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
    provider.load_from_data(configured_css());
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

    let username = gtk::Entry::new();
    username.set_placeholder_text(Some("Your username"));
    username.set_hexpand(true);
    let username_label = gtk::Label::new(Some("_USERNAME"));
    username_label.set_use_underline(true);
    username_label.set_mnemonic_widget(Some(&username));
    username_label.set_halign(gtk::Align::Start);
    username_label.add_css_class("tdm-subtitle");

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
    let session_label = gtk::Label::new(Some("_DESKTOP SESSION"));
    session_label.set_use_underline(true);
    session_label.set_mnemonic_widget(Some(&session_dropdown));
    session_label.set_halign(gtk::Align::Start);
    session_label.add_css_class("tdm-subtitle");
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
    login.set_receives_default(true);
    window.set_default_widget(Some(&login));
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
                    prompt_for_events.set_input_purpose(if secret {
                        gtk::InputPurpose::Password
                    } else {
                        gtk::InputPurpose::FreeForm
                    });
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

    let power_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    power_actions.set_halign(gtk::Align::Center);
    let poweroff = gtk::Button::with_label("Power off…");
    let reboot = gtk::Button::with_label("Restart…");
    let suspend = gtk::Button::with_label("Suspend…");
    poweroff.set_tooltip_text(Some("Request system power off"));
    reboot.set_tooltip_text(Some("Request system restart"));
    suspend.set_tooltip_text(Some("Request system suspend"));
    connect_power_control(
        &poweroff,
        &window,
        &status,
        "poweroff",
        "Power off this computer?",
    );
    connect_power_control(
        &reboot,
        &window,
        &status,
        "reboot",
        "Restart this computer?",
    );
    connect_power_control(
        &suspend,
        &window,
        &status,
        "suspend",
        "Suspend this computer?",
    );
    power_actions.append(&poweroff);
    power_actions.append(&reboot);
    power_actions.append(&suspend);

    outer.append(&brand);
    outer.append(&subtitle);
    outer.append(&panel);
    outer.append(&power_actions);
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
    fn selects_supported_themes_and_defaults_unknown_values_to_dark() {
        assert_eq!(css_for_theme("dark"), CSS);
        assert_eq!(css_for_theme("light"), LIGHT_CSS);
        assert_eq!(css_for_theme("high-contrast"), HIGH_CONTRAST_CSS);
        assert_eq!(css_for_theme("unknown"), CSS);
    }

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

    fn read_mock_request(stream: &mut UnixStream) -> serde_json::Value {
        let mut length = [0_u8; 4];
        stream.read_exact(&mut length).unwrap();
        let length = u32::from_ne_bytes(length) as usize;
        let mut payload = vec![0_u8; length];
        stream.read_exact(&mut payload).unwrap();
        serde_json::from_slice(&payload).unwrap()
    }

    fn write_mock_response(stream: &mut UnixStream, response: serde_json::Value) {
        let payload = serde_json::to_vec(&response).unwrap();
        stream
            .write_all(&(payload.len() as u32).to_ne_bytes())
            .unwrap();
        stream.write_all(&payload).unwrap();
        stream.flush().unwrap();
    }

    #[test]
    fn completes_full_greetd_ipc_conversation_with_mock_server() {
        use std::os::unix::net::UnixListener;
        use std::path::PathBuf;
        use std::time::{SystemTime, UNIX_EPOCH};

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let socket_path =
            std::env::temp_dir().join(format!("tdm-greetd-{}-{nonce}.sock", std::process::id()));
        let listener = UnixListener::bind(&socket_path).unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();

            let create = read_mock_request(&mut stream);
            assert_eq!(create["type"], "create_session");
            assert_eq!(create["username"], "tdm-test-user");
            write_mock_response(
                &mut stream,
                serde_json::json!({
                    "type": "auth_message",
                    "auth_message_type": "secret",
                    "auth_message": "Password:"
                }),
            );

            let answer = read_mock_request(&mut stream);
            assert_eq!(answer["type"], "post_auth_message_response");
            assert_eq!(answer["response"], "test-only-secret");
            write_mock_response(&mut stream, serde_json::json!({"type": "success"}));

            let start = read_mock_request(&mut stream);
            assert_eq!(start["type"], "start_session");
            assert_eq!(start["cmd"], serde_json::json!(["/usr/bin/true"]));
            assert_eq!(
                start["env"],
                serde_json::json!(["XDG_SESSION_TYPE=wayland"])
            );
            write_mock_response(&mut stream, serde_json::json!({"type": "success"}));
        });

        let selected_session = session::Session {
            name: "Test Wayland".to_owned(),
            exec: "/usr/bin/true".to_owned(),
            command: vec!["/usr/bin/true".to_owned()],
            kind: session::SessionKind::Wayland,
            file: PathBuf::from("/tmp/tdm-test.desktop"),
        };
        let (event_tx, event_rx) = mpsc::channel();
        let (reply_tx, reply_rx) = mpsc::channel();
        let socket_string = socket_path.to_string_lossy().into_owned();
        let client = thread::spawn(move || {
            run_authentication_at(
                &socket_string,
                "tdm-test-user".to_owned(),
                selected_session,
                event_tx,
                reply_rx,
            );
        });

        match event_rx.recv_timeout(Duration::from_secs(5)).unwrap() {
            UiEvent::Prompt { message, secret } => {
                assert_eq!(message, "Password:");
                assert!(secret);
            }
            _ => panic!("expected greetd password prompt"),
        }
        reply_tx
            .send(UserReply::Answer(Some("test-only-secret".to_owned())))
            .unwrap();
        match event_rx.recv_timeout(Duration::from_secs(5)).unwrap() {
            UiEvent::SessionStarted => {}
            _ => panic!("expected session-started event"),
        }

        client.join().unwrap();
        server.join().unwrap();
        let _ = std::fs::remove_file(socket_path);
    }
}
