//! Captura visual del editor: renderiza un documento a PNGs paginados para
//! verificar la vista enriquecida (tablas, cajas de código, filetes de
//! título, citas, listas, imágenes) sin abrir ventanas interactivas.
//!
//! Ejecutar: xvfb-run -a cargo test --test render_shot -- --ignored --nocapture
//! Entrada:  MD_PATH (por defecto, documento de muestra integrado)
//! Salida:   /mnt/agents/work/shots/pagina-NN.png

// Los módulos de la app se incluyen vía #[path] y el harness solo ejercita
// parte de su API: el resto aparece como código muerto aunque la app sí lo
// use (mismo criterio que tests/crash_stress.rs).
#![allow(dead_code)]

#[path = "../src/editor.rs"]
mod editor;
#[path = "../src/markdown_render.rs"]
mod markdown_render;
#[path = "../src/markdown_view.rs"]
mod markdown_view;
#[path = "../src/settings.rs"]
mod settings;

use editor::Editor;
use gtk4::prelude::*;
use std::time::{Duration, Instant};

const DOC: &str = "\
# Título principal

| Command    | Description                      | Precio |
| ---------- | -------------------------------- | -----: |
| git status | List all **new** or modified     |    $12 |
| git diff   | Show file `differences`          |  $1600 |
";

/// PNG de 64x48 generado con un codificador real (bicolor rojo/azul), para
/// probar el pintado de imágenes locales.
const TEST_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 64, 0, 0, 0, 48, 8, 2,
    0, 0, 0, 46, 41, 235, 72, 0, 0, 0, 91, 73, 68, 65, 84, 120, 156, 237, 209, 65, 13, 192, 48, 16,
    3, 193, 166, 184, 14, 73, 17, 7, 86, 65, 228, 49, 138, 180, 131, 192, 43, 175, 249, 246, 115,
    179, 87, 15, 56, 85, 128, 86, 128, 86, 128, 86, 128, 86, 128, 86, 128, 86, 128, 86, 128, 86,
    128, 86, 128, 86, 128, 182, 246, 140, 222, 112, 228, 250, 7, 10, 208, 10, 208, 10, 208, 10,
    208, 10, 208, 10, 208, 10, 208, 10, 208, 10, 208, 10, 208, 10, 208, 126, 242, 14, 2, 253, 217,
    136, 163, 109, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

fn pump(ctx: &gtk4::glib::MainContext, ms: u64) {
    let t = Instant::now();
    while t.elapsed() < Duration::from_millis(ms) {
        while ctx.iteration(false) {}
        std::thread::sleep(Duration::from_millis(4));
    }
}

/// Renderiza el contenido visible de la ventana a un PNG usando el renderer
/// de la superficie ya realizada.
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

#[test]
#[ignore = "requiere display: xvfb-run -a cargo test --test render_shot -- --ignored --nocapture"]
fn captura_vista_enriquecida() {
    gtk4::init().expect("GTK necesita un display; ejecuta bajo xvfb-run");
    libadwaita::init().expect("libadwaita init");

    let out = std::env::var("SCRIBE_SHOTS_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("scribe-shots"));
    std::fs::create_dir_all(&out).expect("crear dir de salida");
    std::fs::write(out.join("test.png"), TEST_PNG).expect("escribir imagen de prueba");

    // Documento: el indicado por MD_PATH o la muestra integrada.
    let doc = std::env::var("MD_PATH")
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_else(|| DOC.to_string());

    let editor = Editor::new();
    editor.set_base_dir(Some(out.to_path_buf()));
    let win = gtk4::Window::builder()
        .default_width(760)
        .default_height(1000)
        .build();
    win.set_child(Some(&editor.widget));
    win.present();

    let ctx = gtk4::glib::MainContext::default();
    editor.set_text(&doc);
    pump(&ctx, 800); // drena el debounce de decoración y el layout

    let total = editor.line_count().max(1);
    // Filas de texto por página (~1000 px de ventana): paginar con solape de
    // una línea para no cortar cajas ni tablas a la mitad sin contexto.
    let per_page = 38usize;
    let mut first = 0usize;
    let mut page = 0usize;
    while first < total as usize {
        editor.scroll_to_line(first as i32);
        pump(&ctx, 350);
        shot(&win, &out.join(format!("pagina-{page:02}.png")));
        page += 1;
        first += per_page;
    }
    eprintln!("{page} paginas escritas en {}", out.display());
}
