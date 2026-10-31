use gtk4 as gtk;
use gtk::prelude::*;

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
    subtitle.add_css_class("tdm-subtitle");

    let panel = gtk::Box::new(gtk::Orientation::Vertical, 14);
    panel.add_css_class("tdm-panel");
    panel.set_width_request(360);

    let heading = gtk::Label::new(Some("Welcome back"));
    heading.add_css_class("tdm-accent");
    heading.set_halign(gtk::Align::Start);

    let username_label = gtk::Label::new(Some("USERNAME"));
    username_label.set_halign(gtk::Align::Start);
    username_label.add_css_class("tdm-subtitle");
    let username = gtk::Entry::new();
    username.set_placeholder_text(Some("Your username"));
    username.set_hexpand(true);
    username.set_activates_default(true);

    let password_label = gtk::Label::new(Some("PASSWORD"));
    password_label.set_halign(gtk::Align::Start);
    password_label.add_css_class("tdm-subtitle");
    let password = gtk::PasswordEntry::new();
    password.set_placeholder_text(Some("Password"));
    password.set_show_peek_icon(true);
    password.set_hexpand(true);

    let session_label = gtk::Label::new(Some("DESKTOP SESSION"));
    session_label.set_halign(gtk::Align::Start);
    session_label.add_css_class("tdm-subtitle");
    let session = gtk::DropDown::from_strings(&["Automatic selection", "Wayland", "X11"]);
    session.set_hexpand(true);

    let status = gtk::Label::new(Some(
        "Preview mode — authentication is not connected yet.",
    ));
    status.add_css_class("tdm-status");
    status.set_wrap(true);
    status.set_halign(gtk::Align::Start);

    let login = gtk::Button::with_label("Continue");
    login.add_css_class("suggested-action");
    login.set_hexpand(true);
    login.set_receives_default(true);

    let password_for_click = password.clone();
    let username_for_click = username.clone();
    let status_for_click = status.clone();
    login.connect_clicked(move |_| {
        // This preview intentionally does not submit or retain credentials.
        // Clear the field immediately so the prototype cannot be mistaken for
        // a functioning authentication screen.
        password_for_click.set_text("");
        if username_for_click.text().trim().is_empty() {
            status_for_click.set_text("Enter a username to preview the login flow.");
        } else {
            status_for_click.set_text(
                "Preview only: no credentials were sent or saved. greetd/PAM integration is still under development.",
            );
        }
    });

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let shutdown = gtk::Button::with_label("Shut down");
    let restart = gtk::Button::with_label("Restart");
    shutdown.set_sensitive(false);
    restart.set_sensitive(false);
    shutdown.set_tooltip_text(Some("Power controls are not connected in this preview."));
    restart.set_tooltip_text(Some("Power controls are not connected in this preview."));
    actions.append(&shutdown);
    actions.append(&restart);

    panel.append(&heading);
    panel.append(&username_label);
    panel.append(&username);
    panel.append(&password_label);
    panel.append(&password);
    panel.append(&session_label);
    panel.append(&session);
    panel.append(&login);
    panel.append(&status);

    let footer = gtk::Label::new(Some("TDM 0.2.0  •  GRAPHICAL GREETER PREVIEW"));
    footer.add_css_class("tdm-subtitle");

    outer.append(&brand);
    outer.append(&subtitle);
    outer.append(&panel);
    outer.append(&actions);
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
