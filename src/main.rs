use gtk4::prelude::*;
use libadwaita as adw;
use std::rc::Rc;

mod editor;
mod file_manager;
mod markdown_render;
mod markdown_view;
mod preferences;
mod preview;
mod settings;
mod templates;
mod window;

use settings::AppSettings;
use window::ScribeWindow;

const APP_ID: &str = "app.scribe.Scribe";

fn main() -> glib::ExitCode {
    let app = adw::Application::builder()
        .application_id(APP_ID)
        // Necesario para que `scribe fichero.md` y «Abrir con» funcionen.
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();

    app.connect_startup(|app| {
        templates::ensure_defaults();

        // Las acciones app.* deben existir: si no, las entradas del menú
        // salen grises y los aceleradores no hacen nada.
        let quit = gio::SimpleAction::new("quit", None);
        let weak = app.downgrade();
        quit.connect_activate(move |_, _| {
            if let Some(app) = weak.upgrade() {
                app.quit();
            }
        });
        app.add_action(&quit);

        let new_window = gio::SimpleAction::new("new-window", None);
        let weak = app.downgrade();
        new_window.connect_activate(move |_, _| {
            if let Some(app) = weak.upgrade() {
                build_window(&app).present();
            }
        });
        app.add_action(&new_window);

        for (action, accels) in [
            ("win.new-document", &["<Ctrl>n"][..]),
            ("win.open", &["<Ctrl>o"]),
            ("win.save", &["<Ctrl>s"]),
            ("win.save-as", &["<Ctrl><Shift>s"]),
            ("app.new-window", &["<Ctrl><Shift>n"]),
            ("app.quit", &["<Ctrl>q"]),
            ("win.bold", &["<Ctrl>b"]),
            ("win.italic", &["<Ctrl>i"]),
            ("win.code", &["<Ctrl>k"]),
            ("win.format-tables", &["<Ctrl><Alt>t"]),
            ("win.toggle-sidebar", &["F9"]),
            ("win.toggle-preview", &["<Ctrl><Shift>p"]),
            ("win.focus-mode", &["<Ctrl><Shift>f"]),
            ("win.typewriter-mode", &["<Ctrl><Shift>t"]),
            (
                "win.zoom-in",
                &["<Ctrl>plus", "<Ctrl>equal", "<Ctrl>KP_Add"],
            ),
            ("win.zoom-out", &["<Ctrl>minus", "<Ctrl>KP_Subtract"]),
            ("win.zoom-reset", &["<Ctrl>0"]),
            ("win.preferences", &["<Ctrl>comma"]),
            ("win.show-help-overlay", &["<Ctrl>question"]),
        ] {
            app.set_accels_for_action(action, accels);
        }
    });

    app.connect_activate(|app| build_window(app).present());

    app.connect_open(|app, files, _hint| {
        for file in files {
            let win = build_window(app);
            if let Some(path) = file.path() {
                win.open_path(&path);
            }
            win.present();
        }
    });

    app.run()
}

fn build_window(app: &adw::Application) -> ScribeWindow {
    let settings = Rc::new(AppSettings::new());
    ScribeWindow::new(app, &settings)
}
