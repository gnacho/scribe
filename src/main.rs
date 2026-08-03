use gtk4::prelude::*;
use libadwaita as adw;

mod window;
mod sidebar;
mod editor;

use window::ScribeWindow;

const APP_ID: &str = "app.scribe.Scribe";

fn main() {
    let app = adw::Application::builder()
        .application_id(APP_ID)
        .build();

    app.connect_activate(build_ui);
    app.run();
}

fn build_ui(app: &adw::Application) {
    let win = ScribeWindow::new(app);
    win.present();
}
