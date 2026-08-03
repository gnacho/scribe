use gtk4::prelude::*;
use libadwaita as adw;

mod window;
mod editor;
mod settings;
mod file_manager;

use window::ScribeWindow;
use settings::AppSettings;

const APP_ID: &str = "app.scribe.Scribe";

fn main() {
    let app = adw::Application::builder()
        .application_id(APP_ID)
        .build();

    app.connect_startup(|app| {
        setup_shortcuts(app);
    });

    app.connect_activate(build_ui);
    app.run();
}

fn setup_shortcuts(app: &adw::Application) {
    app.set_accels_for_action("win.open", &["<Ctrl>o"]);
    app.set_accels_for_action("win.save", &["<Ctrl>s"]);
    app.set_accels_for_action("win.save-as", &["<Ctrl><Shift>s"]);
    app.set_accels_for_action("win.new-window", &["<Ctrl>n"]);
    app.set_accels_for_action("win.toggle-sidebar", &["<Ctrl>b"]);
    app.set_accels_for_action("win.toggle-search", &["<Ctrl>f"]);
    app.set_accels_for_action("win.toggle-code", &["<Ctrl><Shift>c"]);
    app.set_accels_for_action("win.focus-mode", &["F11"]);
    app.set_accels_for_action("win.preferences", &["<Ctrl>comma"]);
    app.set_accels_for_action("win.show-help-overlay", &["<Ctrl>question"]);
    app.set_accels_for_action("app.quit", &["<Ctrl>q"]);
}

fn build_ui(app: &adw::Application) {
    let settings = AppSettings::new();
    let win = ScribeWindow::new(app, &settings);
    win.present();
}
