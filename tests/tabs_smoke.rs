//! Smoke test headless del modelo de pestañas (AdwTabView): crea una ventana
//! bajo xvfb y ejercita el ciclo de vida completo —apertura vía acciones,
//! deduplicación por ruta canónica, cierre sin cambios, refresco del título
//! al cambiar de pestaña, punto «• » al modificar, guardado, y la regla D5
//! (al cerrar la última pestaña queda una en blanco).
//!
//! Ejecutar: xvfb-run -a cargo test --test tabs_smoke -- --ignored --nocapture

// Los módulos de la app se incluyen vía #[path] y el harness solo ejercita
// parte de su API: el resto aparece como código muerto aunque la app sí lo
// use (mismo criterio que tests/crash_stress.rs).
#![allow(dead_code)]

#[path = "../src/tema.rs"]
mod tema;

#[path = "../src/editor.rs"]
mod editor;
#[path = "../src/export_pdf.rs"]
mod export_pdf;
#[path = "../src/file_manager.rs"]
mod file_manager;
#[path = "../src/guardado.rs"]
mod guardado;
#[path = "../src/markdown_render.rs"]
mod markdown_render;
#[path = "../src/markdown_view.rs"]
mod markdown_view;
#[path = "../src/preferences.rs"]
mod preferences;
#[path = "../src/preview.rs"]
mod preview;
#[path = "../src/recovery.rs"]
mod recovery;
#[path = "../src/settings.rs"]
mod settings;
#[path = "../src/window.rs"]
mod window;

use gtk4::prelude::*;
use settings::AppSettings;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};
use window::ScribeWindow;

/// Bombea el main context durante `ms` milisegundos: drena los debounces de
/// decoración y los refrescos diferidos de título/TOC/preview.
fn pump(ctx: &gtk4::glib::MainContext, ms: u64) {
    let t = Instant::now();
    while t.elapsed() < Duration::from_millis(ms) {
        while ctx.iteration(false) {}
        std::thread::sleep(Duration::from_millis(4));
    }
}

/// Renderiza la ventana a PNG (misma técnica que tests/render_shot.rs) para
/// tener prueba visual de la barra de pestañas.
fn shot(win: &gtk4::Window, path: &std::path::Path) {
    let w = win.width() as f64;
    let h = win.height() as f64;
    let paintable = gtk4::WidgetPaintable::new(Some(win));
    let snapshot = gtk4::Snapshot::new();
    paintable.snapshot(&snapshot, w, h);
    let node = snapshot.to_node().expect("snapshot sin nodo raíz");
    let renderer = win
        .native()
        .expect("sin native")
        .renderer()
        .expect("sin renderer");
    let texture = renderer.render_texture(&node, None);
    texture.save_to_png(path).expect("guardar png");
}

/// Desambigua `activate_action`: en una ventana conviven ActionGroupExt y
/// WidgetExt; queremos el de Widget (acciones `win.*`).
fn act(
    win: &libadwaita::ApplicationWindow,
    name: &str,
    param: Option<&gtk4::glib::Variant>,
) -> Result<(), gtk4::glib::BoolError> {
    gtk4::prelude::WidgetExt::activate_action(win, name, param)
}
#[test]
#[ignore = "requiere display: xvfb-run -a cargo test --test tabs_smoke -- --ignored --nocapture"]
fn ciclo_de_vida_de_pestanas() {
    gtk4::init().expect("GTK necesita un display; ejecuta bajo xvfb-run");
    libadwaita::init().expect("libadwaita init");

    // Sin GSETTINGS_SCHEMA_DIR el esquema no se encuentra y AppSettings
    // trabaja con los valores por defecto: el test no toca el dconf real.
    let app = libadwaita::Application::builder()
        .application_id("io.github.gnacho.Scribe.TabsSmoke")
        .build();
    app.register(gtk4::gio::Cancellable::NONE)
        .expect("registrar la app de test");

    let settings = Rc::new(AppSettings::new());
    let win = ScribeWindow::new(&app, &settings);
    win.present();

    let ctx = gtk4::glib::MainContext::default();
    pump(&ctx, 300);

    // La ventana arranca con una pestaña en blanco.
    assert_eq!(win.page_count(), 1, "la ventana arranca con una en blanco");
    assert_eq!(win.document_count(), 1);

    // Tres documentos de muestra en el directorio temporal.
    let dir = std::env::temp_dir().join("scribe-tabs-smoke");
    std::fs::create_dir_all(&dir).expect("crear dir temporal");
    let paths: Vec<PathBuf> = ["alfa.md", "beta.md", "gamma.md"]
        .iter()
        .map(|n| dir.join(n))
        .collect();
    for (i, p) in paths.iter().enumerate() {
        std::fs::write(p, format!("# Doc {i}\n\ncontenido de prueba {i}\n"))
            .expect("escribir doc de prueba");
    }

    // Abrir los tres vía la acción win.open-recent (la misma vía que el menú
    // de recientes; no necesita diálogo). La primera apertura reutiliza la
    // pestaña virgen, así que quedan exactamente 3.
    for p in &paths {
        let target = p.to_string_lossy().to_string().to_variant();
        act(&win.window, "win.open-recent", Some(&target)).expect("activar win.open-recent");
        pump(&ctx, 150);
    }
    assert_eq!(win.page_count(), 3, "tres documentos, tres pestañas");
    assert_eq!(win.document_count(), 3);

    // Prueba visual: la barra de pestañas con los tres documentos.
    let shots_dir = std::path::Path::new("/mnt/agents/work/shots");
    if shots_dir.is_dir() {
        shot(
            &win.window.clone().upcast(),
            &shots_dir.join("pestanas.png"),
        );
    }

    // D6/R5: reabrir un path ya abierto no crea pestaña; selecciona la
    // existente (el título de la ventana pasa a ser el suyo).
    win.open_path(&paths[0]);
    pump(&ctx, 150);
    assert_eq!(win.page_count(), 3, "dedupe por ruta canónica");
    assert_eq!(win.document_count(), 3);
    let title = win
        .window
        .title()
        .map(|t| t.to_string())
        .unwrap_or_default();
    assert!(title.contains("alfa.md"), "título inesperado: {title}");

    // Cerrar la pestaña activa sin cambios: no hay diálogo.
    act(&win.window, "win.close-tab", None).expect("activar win.close-tab");
    pump(&ctx, 150);
    assert_eq!(win.page_count(), 2, "cerrar sin cambios cierra");
    assert_eq!(win.document_count(), 2);
    // Tras cerrar alfa queda seleccionada su vecina (beta).
    let title = win
        .window
        .title()
        .map(|t| t.to_string())
        .unwrap_or_default();
    assert!(title.contains("beta.md"), "título inesperado: {title}");

    // Modificar la activa marca el título con «• »…
    act(&win.window, "win.bold", None).expect("activar win.bold");
    pump(&ctx, 150);
    let title = win
        .window
        .title()
        .map(|t| t.to_string())
        .unwrap_or_default();
    assert!(
        title.starts_with('•'),
        "el título debe marcarse al modificar: {title}"
    );
    // …y win.save (tiene fichero, sin diálogo) lo quita.
    act(&win.window, "win.save", None).expect("activar win.save");
    pump(&ctx, 150);
    let title = win
        .window
        .title()
        .map(|t| t.to_string())
        .unwrap_or_default();
    assert!(
        !title.starts_with('•'),
        "el título debe limpiarse al guardar: {title}"
    );

    // Cambiar de pestaña refresca el título de la ventana.
    act(&win.window, "win.tab-next", None).expect("activar win.tab-next");
    pump(&ctx, 150);
    let title = win
        .window
        .title()
        .map(|t| t.to_string())
        .unwrap_or_default();
    assert!(title.contains("gamma.md"), "título inesperado: {title}");
    act(&win.window, "win.tab-prev", None).expect("activar win.tab-prev");
    pump(&ctx, 150);
    let title = win
        .window
        .title()
        .map(|t| t.to_string())
        .unwrap_or_default();
    assert!(title.contains("beta.md"), "título inesperado: {title}");

    // D5: al cerrar todas queda una pestaña en blanco; la ventana no se
    // cierra.
    act(&win.window, "win.close-tab", None).expect("cerrar beta");
    pump(&ctx, 150);
    act(&win.window, "win.close-tab", None).expect("cerrar gamma");
    pump(&ctx, 150);
    assert_eq!(win.page_count(), 1, "D5: queda una pestaña en blanco");
    assert_eq!(win.document_count(), 1);
    let title = win
        .window
        .title()
        .map(|t| t.to_string())
        .unwrap_or_default();
    // El titulo del borrador pasa por gettext desde el issue #14: se compara
    // con el mismo msgid que usa la ventana, para no depender de que el
    // catalogo este instalado (aqui no lo esta: el dominio y el idioma solo los
    // fija main).
    assert!(
        title.contains(gettextrs::gettext("Untitled").as_str()),
        "la pestaña nueva es un borrador: {title}"
    );

    // Los documentos cerrados sin cambios no dejaron diálogos colgados.
    assert_eq!(win.page_count(), win.document_count() as i32);

    // Cierre de ventana sin modificados: prospera sin diálogos.
    win.window.close();
    pump(&ctx, 100);

    std::fs::remove_dir_all(&dir).ok();
}

/// Issue #101: un borrador sin título cuyo buffer está vacío se cierra sin
/// el diálogo «Guardar/Descartar/Cancelar», aunque el flag de modificado
/// siga activo (escribir y borrar todo). Con un diálogo pendiente la
/// pestaña seguiría abierta, así que page_count discrimina el resultado.
#[test]
#[ignore = "requiere display: xvfb-run -a cargo test --test tabs_smoke -- --ignored --nocapture"]
fn cierre_borrador_vacio_sin_dialogo() {
    gtk4::init().expect("GTK necesita un display; ejecuta bajo xvfb-run");
    libadwaita::init().expect("libadwaita init");

    let app = libadwaita::Application::builder()
        .application_id("app.scribe.Scribe.UntitledCloseSmoke")
        .build();
    app.register(gtk4::gio::Cancellable::NONE)
        .expect("registrar la app de test");

    let settings = Rc::new(AppSettings::new());
    let win = ScribeWindow::new(&app, &settings);
    win.present();

    let ctx = gtk4::glib::MainContext::default();
    pump(&ctx, 300);

    // Un documento con fichero (la primera apertura reutiliza la pestaña
    // virgen del arranque) y un borrador sin título: dos pestañas.
    let dir = std::env::temp_dir().join("scribe-untitled-close-smoke");
    std::fs::create_dir_all(&dir).expect("crear dir temporal");
    let path = dir.join("alfa.md");
    std::fs::write(&path, "# Alfa\n\ncontenido de prueba\n").expect("escribir doc de prueba");
    win.open_path(&path);
    pump(&ctx, 150);
    assert_eq!(
        win.page_count(),
        1,
        "la apertura reutiliza la pestaña virgen"
    );

    act(&win.window, "win.new-document", None).expect("activar win.new-document");
    pump(&ctx, 150);
    assert_eq!(win.page_count(), 2, "borrador nuevo en segunda pestaña");

    // Escribir en el borrador activo…
    let buffer = win.active_buffer().expect("buffer del borrador activo");
    assert_eq!(buffer.char_count(), 0, "el borrador nuevo está vacío");
    let mut iter = buffer.end_iter();
    buffer.insert(&mut iter, "escrito y borrado\n");
    pump(&ctx, 150);
    let title = win
        .window
        .title()
        .map(|t| t.to_string())
        .unwrap_or_default();
    assert!(
        title.starts_with('•'),
        "escribir marca el título con «• »: {title}"
    );

    // …y borrarlo todo: el buffer queda vacío pero el documento sigue
    // marcado como modificado (el flag no se reevalúa al borrar).
    let mut start = buffer.start_iter();
    let mut end = buffer.end_iter();
    buffer.delete(&mut start, &mut end);
    pump(&ctx, 150);
    assert_eq!(buffer.char_count(), 0, "el buffer quedó vacío");

    // Cerrar el borrador: al estar vacío no hay nada que perder y cierra
    // directamente; si apareciera el diálogo la pestaña seguiría abierta.
    act(&win.window, "win.close-tab", None).expect("cerrar el borrador");
    pump(&ctx, 150);
    assert_eq!(
        win.page_count(),
        1,
        "el borrador vacío cierra sin diálogo (issue #101)"
    );
    assert_eq!(win.document_count(), 1);
    let title = win
        .window
        .title()
        .map(|t| t.to_string())
        .unwrap_or_default();
    assert!(
        title.contains("alfa.md"),
        "queda el documento con fichero: {title}"
    );

    win.window.close();
    pump(&ctx, 100);

    std::fs::remove_dir_all(&dir).ok();
}

/// Busca el primer AdwSplitButton del árbol de widgets (el «Abrir» de la
/// cabecera) sin asumir la estructura interna de la ToolbarView.
fn busca_split_button(w: &gtk4::Widget) -> Option<libadwaita::SplitButton> {
    if let Ok(split) = w.clone().downcast::<libadwaita::SplitButton>() {
        return Some(split);
    }
    let mut child = w.first_child();
    while let Some(c) = child {
        if let Some(found) = busca_split_button(&c) {
            return Some(found);
        }
        child = c.next_sibling();
    }
    None
}

#[test]
#[ignore = "requiere display: xvfb-run -a cargo test --test tabs_smoke recientes -- --ignored --nocapture"]
fn menu_recientes_nunca_vacio() {
    gtk4::init().expect("GTK necesita un display; ejecuta bajo xvfb-run");
    libadwaita::init().expect("libadwaita init");

    let app = libadwaita::Application::builder()
        .application_id("io.github.gnacho.Scribe.RecentSmoke")
        .build();
    app.register(gtk4::gio::Cancellable::NONE)
        .expect("registrar la app de test");

    let settings = Rc::new(AppSettings::new());
    let win = ScribeWindow::new(&app, &settings);
    win.present();
    let ctx = gtk4::glib::MainContext::default();
    pump(&ctx, 300);

    let split = busca_split_button(&win.window.clone().upcast())
        .expect("el SplitButton «Abrir» existe en la cabecera");
    let menu = split.menu_model().expect("el SplitButton tiene menu_model");
    assert!(
        menu.n_items() >= 1,
        "el menú de recientes nunca debe quedar vacío (issue #110)"
    );
    if menu.n_items() == 1 {
        // Recientes vacíos: el único ítem es el placeholder deshabilitado.
        let it = menu.iterate_item_attributes(0);
        let mut sensitive = None;
        while let Some((name, value)) = it.next() {
            if name.as_str() == "sensitive" {
                sensitive = value.get::<bool>();
            }
        }
        assert_eq!(
            sensitive,
            Some(false),
            "el placeholder sin recientes debe estar deshabilitado"
        );
    }

    win.window.close();
    pump(&ctx, 100);
}
