use gettextrs::gettext;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::ffi::OsStr;
use std::io::{IsTerminal, Read};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

mod arranque;
mod editor;
mod export_css;
mod export_pdf;
mod file_manager;
mod guardado;
mod markdown_render;
mod markdown_view;
mod preferences;
mod preview;
mod recovery;
mod settings;
mod tema;
mod window;

use settings::AppSettings;
use window::ScribeWindow;

const APP_ID: &str = "io.github.gnacho.Scribe";
const GETTEXT_DOMAIN: &str = "scribe";

/// Iconos de la app empaquetados como recurso (issue #111); se registran al
/// arrancar para que los simbolicos propios se resuelvan aunque el binario
/// se ejecute sin datos instalados en ningun tema de iconos.
const ICONOS_GRESOURCE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/icons.gresource"));

/// Locale dir relative to the binary: <prefix>/bin/scribe -> <prefix>/share/locale.
/// Works for /usr, /usr/local, ~/.local and Flatpak (/app) installs alike.
fn init_gettext() {
    use gettextrs::{
        bind_textdomain_codeset, bindtextdomain, setlocale, textdomain, LocaleCategory,
    };
    // SAFETY: gettext-rs marks setlocale unsafe because locale is global
    // state; main() runs before any thread spawns, so this is safe.
    unsafe { setlocale(LocaleCategory::LcAll, "") };
    let locale_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("../share/locale")))
        .unwrap_or_else(|| std::path::PathBuf::from("/usr/local/share/locale"));
    let _ = bindtextdomain(GETTEXT_DOMAIN, locale_dir);
    let _ = textdomain(GETTEXT_DOMAIN);
    let _ = bind_textdomain_codeset(GETTEXT_DOMAIN, "UTF-8");
}

// Registro de ventanas vivas: ScribeWindow es la ancla fuerte de sus
// documentos (los closures de la ventana los capturan en Weak), así que hay
// que retenerla mientras su ventana GTK exista. Las ventanas cerradas se
// podan al destruirse (connect_destroy en build_window) y, por redundancia,
// al consultar el registro.
thread_local! {
    static WINDOWS: RefCell<Vec<Rc<ScribeWindow>>> = const { RefCell::new(Vec::new()) };
}

/// Guarda de «recuperación ya ofrecida»: el diálogo de snapshots pendientes
/// se muestra una sola vez por proceso, en la primera ventana construida
/// (aunque luego se abran más ventanas).
static RECUPERACION_OFRECIDA: AtomicBool = AtomicBool::new(false);

fn main() -> glib::ExitCode {
    init_gettext();
    gio::resources_register(
        &gio::Resource::from_data(&glib::Bytes::from_static(ICONOS_GRESOURCE))
            .expect("el recurso de iconos empaquetado es valido"),
    );
    let app = adw::Application::builder()
        .application_id(APP_ID)
        // HANDLES_OPEN para `scribe fichero.md` y «Abrir con»; HANDLES_COMMAND_LINE
        // para leer los argumentos en crudo y poder separar `ruta:línea:columna`.
        .flags(gio::ApplicationFlags::HANDLES_OPEN | gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();

    app.connect_startup(|app| {
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
            ("win.reload", &["F5"]),
            ("win.find", &["<Ctrl>f"]),
            ("win.toggle-replace", &["<Ctrl>h"]),
            ("win.find-next", &["<Ctrl>g"]),
            ("win.find-prev", &["<Ctrl><Shift>g"]),
            ("win.close-tab", &["<Ctrl>w"]),
            ("win.tab-next", &["<Ctrl>Tab", "<Ctrl>Page_Down"]),
            ("win.tab-prev", &["<Ctrl><Shift>Tab", "<Ctrl>Page_Up"]),
            ("app.new-window", &["<Ctrl><Shift>n"]),
            ("app.quit", &["<Ctrl>q"]),
            ("win.bold", &["<Ctrl>b"]),
            ("win.italic", &["<Ctrl>i"]),
            ("win.code", &["<Ctrl>k"]),
            ("win.format-tables", &["<Ctrl><Alt>t"]),
            ("win.close-fences", &["<Ctrl><Alt>f"]),
            ("win.export-pdf", &["<Ctrl>p"]),
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

    app.connect_command_line(|app, cmdline| {
        // El primer argumento es el nombre del programa; las opciones
        // (empiezan por '-') se ignoran. El resto son ficheros, cada uno
        // resuelto a (ruta, posición de cursor opcional).
        let mut aperturas: Vec<(PathBuf, Option<(u32, u32)>)> = Vec::new();
        for arg in cmdline.arguments().iter().skip(1) {
            if arg.to_string_lossy().starts_with('-') {
                continue;
            }
            aperturas.push(resolver_argumento(arg));
        }

        match arranque::accion_arranque(!aperturas.is_empty(), std::io::stdin().is_terminal()) {
            arranque::AccionArranque::LeerStdin => {
                // stdin redirigido (p. ej. `git diff | scribe`): abrir el
                // texto como documento sin título, marcado sin guardar.
                // Con stdin VACÍO (los lanzamientos de escritorio dejan stdin
                // en /dev/null) la ventana es blanca: abrir un borrador vacío
                // lo marcaba modificado de nacimiento y rompía la
                // reutilización de la pestaña «Sin título» (issue #135).
                let mut texto = String::new();
                let _ = std::io::stdin().read_to_string(&mut texto);
                let win = active_or_new_window(app);
                if !texto.is_empty() {
                    win.open_untitled(texto);
                }
                win.present();
            }
            arranque::AccionArranque::AbrirFicheros => {
                let win = active_or_new_window(app);
                for (ruta, posicion) in aperturas {
                    match posicion {
                        Some((linea, columna)) => win.open_path_at(&ruta, linea, columna),
                        None => win.open_path(&ruta),
                    }
                }
                win.present();
            }
            arranque::AccionArranque::VentanaVacia => {
                active_or_new_window(app).present();
            }
        }
        0
    });

    let code = app.run();
    // Al terminar el bucle se sueltan las ventanas ANTES de que el proceso
    // entre en los dtors del thread_local WINDOWS (issue #141): si el drop
    // del registro disparara destroys de ventanas, el handler ya no podria
    // consultar un TLS en destruccion.
    WINDOWS.with(|list| list.borrow_mut().clear());
    code
}

fn build_window(app: &adw::Application) -> Rc<ScribeWindow> {
    // Los iconos propios viven tambien en un GResource (issue #111): se añade
    // su prefijo al tema del display una sola vez por proceso.
    static ICONOS_EN_TEMA: AtomicBool = AtomicBool::new(false);
    if !ICONOS_EN_TEMA.swap(true, Ordering::SeqCst) {
        if let Some(display) = gdk4::Display::default() {
            gtk4::IconTheme::for_display(&display)
                .add_resource_path("/io/github/gnacho/Scribe/icons");
        }
    }
    let settings = Rc::new(AppSettings::new());
    let win = Rc::new(ScribeWindow::new(app, &settings));
    WINDOWS.with(|list| list.borrow_mut().push(win.clone()));
    // Poda estructural (revision): al destruirse la ventana GTK sale del
    // registro. Antes solo se podaba al consultarlo (connect_open) y la
    // memoria de las ventanas cerradas se acumulaba toda la sesion. El
    // handler no captura nada: usar `dead` evita un autociclo.
    win.window.connect_destroy(|dead| {
        // El proceso puede estar destruyendo ya el thread_local WINDOWS
        // (issue #141: en exit, los dtors TLS sueltan el ultimo Rc, el
        // destroy de la ventana dispara este handler y with() paniquearia
        // con AccessError -> SIGABRT). try_with devuelve Err en ese caso y
        // no hay nada que podar: la lista esta desapareciendo entera.
        let _ = WINDOWS.try_with(|list| {
            list.borrow_mut().retain(|w| {
                w.window.upcast_ref::<gtk4::Window>() != dead.upcast_ref::<gtk4::Window>()
            });
        });
    });
    // Oferta de recuperación de snapshots pendientes (issue #23), una sola
    // vez por arranque.
    ofrecer_recuperacion(&win);
    win
}

/// Ofrece recuperar los snapshots pendientes al arrancar (issue #23): si
/// quedan documentos sin guardar de una ejecución anterior que murió, se
/// muestra un diálogo con tres acciones: recuperar todo, descartar todo o
/// decidir más tarde.
fn ofrecer_recuperacion(win: &Rc<ScribeWindow>) {
    if RECUPERACION_OFRECIDA.swap(true, Ordering::SeqCst) {
        return;
    }
    let dir = recovery::dir_recuperacion();
    let snapshots = recovery::listar_snapshots(&dir);
    if snapshots.is_empty() {
        return;
    }
    let titulos: Vec<String> = snapshots.iter().map(|s| s.titulo.clone()).collect();
    let cuerpo = titulos.join("\n");
    let dialog = adw::MessageDialog::new(
        Some(&win.window),
        Some(&gettext("Recover unsaved documents?")),
        Some(&cuerpo),
    );
    dialog.add_response("recover", &gettext("_Recover All"));
    dialog.add_response("discard", &gettext("_Discard All"));
    dialog.add_response("later", &gettext("Decide Later"));
    dialog.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
    dialog.set_response_appearance("recover", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("recover"));
    dialog.set_close_response("later");
    let win = win.clone();
    dialog.choose(gio::Cancellable::NONE, move |response| {
        match response.as_str() {
            "recover" => {
                for s in &snapshots {
                    win.open_recovered(s.texto.clone(), s.ruta.clone());
                }
                for s in &snapshots {
                    let _ = recovery::borrar_snapshot(&dir, &s.nombre);
                }
            }
            "discard" => {
                for s in &snapshots {
                    let _ = recovery::borrar_snapshot(&dir, &s.nombre);
                }
            }
            // "later" (o cierre del diálogo): no se tocan los snapshots; se
            // ofrecerán de nuevo en el próximo arranque.
            _ => {}
        }
    });
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

/// Resuelve un argumento de la línea de comandos a (ruta, posición opcional).
/// Si el sufijo `:N:N` es numérico pero el fichero no existe, se trata el
/// argumento completo como una ruta normal (no se inventan ficheros).
fn resolver_argumento(arg: &OsStr) -> (PathBuf, Option<(u32, u32)>) {
    let texto = arg.to_string_lossy();
    match arranque::parsear_argumento(&texto) {
        Ok((ruta, Some((linea, columna)))) if ruta.exists() => (ruta, Some((linea, columna))),
        _ => (PathBuf::from(arg), None),
    }
}
