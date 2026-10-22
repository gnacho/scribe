//! Capturas de tablas para el diagnóstico de #124: renderiza documentos con
//! tablas en los tres modos de visibilidad y con el cursor en filas distintas,
//! y guarda un PNG por combinación en `SCRIBE_SHOTS_DIR`.
//!
//! Ejecutar:
//!   SCRIBE_SHOTS_DIR=/tmp/shots124 \
//!   xvfb-run -a cargo test --test tables_shot -- --ignored --test-threads=1

#![allow(dead_code)]

#[path = "../src/tema.rs"]
mod tema;

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
use settings::MarkupVisibility;
use std::time::{Duration, Instant};

const TABLA_SIMPLE: &str = "\
# Titulo

| Col A | Col B |
|-------|-------|
| uno   | dos   |
| tres  | cuatro|
";

const TABLA_ALINEADA: &str = "\
| Izq | Centro | Der |
|:----|:------:|----:|
| a   |   b    |   c |
";

const TABLA_PROSA: &str = "\
| Command    | Description                      |
| ---------- | -------------------------------- |
| git status | List all **new** or modified files with a fairly long description that wraps |
| git diff   | Show `differences` with code     |
";

fn pump(ctx: &gtk4::glib::MainContext, ms: u64) {
    let t = Instant::now();
    while t.elapsed() < Duration::from_millis(ms) {
        while ctx.iteration(false) {}
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Fuerza el repintado de la ventana y espera, con iteracion bloqueante pero
/// acotada por un timeout, a que el frame llegue a procesarse. Sin el
/// `queue_draw` + espera bloqueante, el snapshot captura el arbol antes de que
/// GTK haya pintado el texto (probado: con `pump` pasivo salen capturas
/// vacias).
fn pintar_y_esperar(win: &gtk4::Window, ctx: &gtk4::glib::MainContext) {
    win.queue_draw();
    let hecho = std::rc::Rc::new(std::cell::Cell::new(false));
    let despierta = hecho.clone();
    glib::timeout_add_local_once(Duration::from_millis(150), move || {
        despierta.set(true);
    });
    while !hecho.get() {
        ctx.iteration(true);
    }
    let t = Instant::now();
    while t.elapsed() < Duration::from_millis(150) {
        while ctx.iteration(false) {}
        std::thread::sleep(Duration::from_millis(4));
    }
}

fn shot(win: &gtk4::Window, path: &std::path::Path) {
    let w = win.width() as f64;
    let h = win.height() as f64;
    let paintable = gtk4::WidgetPaintable::new(Some(win));
    let snapshot = gtk4::Snapshot::new();
    paintable.snapshot(&snapshot, w, h);
    let node = snapshot.to_node().expect("snapshot sin nodo");
    let renderer = win
        .native()
        .expect("sin native")
        .renderer()
        .expect("sin renderer");
    renderer
        .render_texture(&node, None)
        .save_to_png(path)
        .expect("guardar png");
}

#[test]
#[ignore = "diagnostico 124: mide si el layout de la tabla de prosa se estabiliza"]
fn prosa_se_estabiliza() {
    let _guardia = markdown_view::tests::gtk_compartido();
    let ctx = gtk4::glib::MainContext::default();
    let editor = Editor::new();
    editor.set_markup_visibility(settings::MarkupVisibility::Hidden);
    editor.set_text(TABLA_PROSA);
    let win = gtk4::Window::new();
    win.set_default_size(700, 400);
    win.set_child(Some(&editor.widget));
    win.present();
    let t0 = Instant::now();
    let mut ticks = 0u64;
    while t0.elapsed() < Duration::from_secs(5) {
        while ctx.iteration(false) {}
        std::thread::sleep(Duration::from_millis(10));
        ticks += 1;
    }
    println!("PROSA: {} ticks en 5s (sin carga deberian ser ~500)", ticks);
    win.destroy();
}

#[test]
#[ignore = "requiere display: xvfb-run -a cargo test --test tables_shot -- --ignored --test-threads=1"]
fn capturas_de_tablas_para_124() {
    let _guardia = markdown_view::tests::gtk_compartido();
    let ctx = gtk4::glib::MainContext::default();

    let out = std::env::var("SCRIBE_SHOTS_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("scribe-shots-124"));
    std::fs::create_dir_all(&out).expect("crear dir de salida");

    let docs: Vec<(&str, &str)> = vec![
        ("simple", TABLA_SIMPLE),
        ("alineada", TABLA_ALINEADA),
        ("prosa", TABLA_PROSA),
    ];
    let modos: Vec<(&str, MarkupVisibility)> = vec![
        ("hidden", MarkupVisibility::Hidden),
        ("dim", MarkupVisibility::Dim),
        ("focus", MarkupVisibility::Focus),
    ];
    // Filas de la tabla simple: 0 titulo, 3 cabecera, 4 delimitador, 5 dato.
    let cursores: Vec<(&str, i32)> = vec![
        ("fuera", 0),
        ("cabecera", 3),
        ("delimitador", 4),
        ("dato", 5),
    ];

    for (nombre_doc, texto) in &docs {
        for (nombre_modo, modo) in &modos {
            for (nombre_cursor, linea) in &cursores {
                let editor = Editor::new();
                editor.set_markup_visibility(*modo);
                editor.set_text(texto);
                let win = gtk4::Window::new();
                win.set_default_size(700, 400);
                win.set_child(Some(&editor.widget));
                win.present();
                pump(&ctx, 400);

                let buffer = editor.buffer();
                if let Some(it) = buffer.iter_at_line(*linea) {
                    buffer.place_cursor(&it);
                }
                editor.refresh();
                pump(&ctx, 400);
                pintar_y_esperar(&win, &ctx);

                let ruta = out.join(format!(
                    "t124-{}-{}-{}.png",
                    nombre_doc, nombre_modo, nombre_cursor
                ));
                shot(&win, &ruta);
                win.destroy();
                pump(&ctx, 50);
            }
        }
    }
    println!("capturas en {}", out.display());
}
