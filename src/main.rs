use gtk4::prelude::*;
use libadwaita as adw;
use std::cell::RefCell;
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

// Registro de ventanas vivas: ScribeWindow es la ancla fuerte de sus
// documentos (los closures de la ventana los capturan en Weak), así que hay
// que retenerla mientras su ventana GTK exista. Las ventanas cerradas se
// podan al destruirse (connect_destroy en build_window) y, por redundancia,
// al consultar el registro.
thread_local! {
    static WINDOWS: RefCell<Vec<Rc<ScribeWindow>>> = const { RefCell::new(Vec::new()) };
}

fn main() -> glib::ExitCode {
    let app = adw::Application::builder()
        .application_id(APP_ID)
        // Necesario para que `scribe fichero.md` y «Abrir con» funcionen.
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();

    app.connect_startup(|app| {
        templates::ensure_defaults();

        // CSS a nivel de aplicación (D9): antes cada Editor instalaba su
        // propio provider en el display y nunca lo retiraba; ahora hay uno
        // solo, registrado una vez, que Editor::set_font recarga.
        let css = gtk4::CssProvider::new();
        if let Some(display) = gdk4::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &css,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
        editor::install_app_css(css);

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
            ("win.close-tab", &["<Ctrl>w"]),
            ("win.tab-next", &["<Ctrl>Tab", "<Ctrl>Page_Down"]),
            ("win.tab-prev", &["<Ctrl><Shift>Tab", "<Ctrl>Page_Up"]),
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

    app.connect_activate(|app| {
        build_window(app).present();
    });

    app.connect_open(|app, files, _hint| {
        // Los ficheros se abren como pestañas en la ventana activa; si no
        // hay ninguna, se crea una.
        let win = active_or_new_window(app);
        for file in files {
            if let Some(path) = file.path() {
                win.open_path(&path);
            }
        }
        win.present();
    });

    app.run()
}

fn build_window(app: &adw::Application) -> Rc<ScribeWindow> {
    let settings = Rc::new(AppSettings::new());
    let win = Rc::new(ScribeWindow::new(app, &settings));
    WINDOWS.with(|list| list.borrow_mut().push(win.clone()));
    // Poda estructural (revision): al destruirse la ventana GTK sale del
    // registro. Antes solo se podaba al consultarlo (connect_open) y la
    // memoria de las ventanas cerradas se acumulaba toda la sesion. El
    // handler no captura nada: usar `dead` evita un autociclo.
    win.window.connect_destroy(|dead| {
        WINDOWS.with(|list| {
            list.borrow_mut().retain(|w| {
                w.window.upcast_ref::<gtk4::Window>() != dead.upcast_ref::<gtk4::Window>()
            });
        });
    });
    win
}

/// La ventana activa de Scribe, o una nueva si no hay ninguna. Poda del
/// registro las ventanas que la aplicación ya ha olvidado (cerradas).
fn active_or_new_window(app: &adw::Application) -> Rc<ScribeWindow> {
    let found = WINDOWS.with(|list| {
        let open = app.windows();
        list.borrow_mut()
            .retain(|w| open.contains(w.window.upcast_ref::<gtk4::Window>()));
        app.active_window().and_then(|active| {
            list.borrow()
                .iter()
                .find(|w| w.window.upcast_ref::<gtk4::Window>() == &active)
                .cloned()
        })
    });
    found.unwrap_or_else(|| build_window(app))
}
