use gtk::prelude::*;
use gtk4 as gtk;
use greetd_ipc::{
    codec::SyncCodec, AuthMessageType, ErrorType, Request, Response,
};
use std::cell::{Cell, RefCell};
use std::env;
use std::os::unix::net::UnixStream;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

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
    Finished(String),
    Failed(String),
}

enum UserReply {
    Answer(Option<String>),
    Cancel,
}

fn send_request(stream: &mut UnixStream, request: Request) -> Result<(), String> {
    request
        .write_to(stream)
        .map_err(|error| format!("Could not send greetd request: {error}"))
}

fn read_response(stream: &mut UnixStream) -> Result<Response, String> {
    Response::read_from(stream)
        .map_err(|error| format!("Could not read greetd response: {error}"))
}

fn cancel_greetd_session(stream: &mut UnixStream) {
    let _ = send_request(stream, Request::CancelSession);
    let _ = read_response(stream);
}

fn run_authentication(
    username: String,
    events: Sender<UiEvent>,
    replies: Receiver<UserReply>,
) {
    let result = (|| -> Result<String, String> {
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
                    // Authentication is complete. TDM does not launch a desktop
                    // session yet, so release the pending greetd session cleanly.
                    send_request(&mut stream, Request::CancelSession)?;
                    match read_response(&mut stream)? {
                        Response::Success => {
                            return Ok(
                                "PAM authentication succeeded. Desktop session launching is not implemented yet."
                                    .to_owned(),
                            );
                        }
                        Response::Error { description, .. } => {
                            return Err(format!(
                                "Authentication succeeded, but greetd could not close the pending session: {description}"
                            ));
                        }
                        Response::AuthMessage { .. } => {
                            return Err("Unexpected authentication prompt while closing the session.".to_owned());
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
                            return Ok("Sign-in cancelled.".to_owned());
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
        Ok(message) => {
            let _ = events.send(UiEvent::Finished(message));
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

    let prompt_label = gtk::Label::new(Some("PASSWORD"));
    prompt_label.set_halign(gtk::Align::Start);
    prompt_label.add_css_class("tdm-subtitle");
    let prompt_entry = gtk::Entry::new();
    prompt_entry.set_placeholder_text(Some("Authentication response"));
    prompt_entry.set_visibility(false);
    prompt_entry.set_hexpand(true);

    let status = gtk::Label::new(Some(
        "Authentication is handled by greetd and your system PAM policy.",
    ));
    status.add_css_class("tdm-status");
    status.set_wrap(true);
    status.set_halign(gtk::Align::Start);

    let login = gtk::Button::with_label("Sign in");
    login.add_css_class("suggested-action");
    login.set_hexpand(true);
    login.set_sensitive(true);

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
        thread::spawn(move || run_authentication(name, events, reply_rx));
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
                    status_for_events.set_text("Your system authentication service requested a response.");
                    prompt_for_events.grab_focus();
                }
                UiEvent::Status(message) => {
                    status_for_events.set_text(&message);
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
    panel.append(&prompt_label);
    panel.append(&prompt_entry);
    panel.append(&actions);
    panel.append(&cancel);
    panel.append(&status);

    let footer = gtk::Label::new(Some("TDM 0.0.5  •  GREETD / PAM AUTHENTICATION PREVIEW"));
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
