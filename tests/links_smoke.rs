//! Comprobación headless de los enlaces y las tarjetas de vídeo (issues #39 y
//! #40): que un punto del widget se traduzca a la URL correcta, que solo se
//! ofrezcan los destinos que se pueden abrir (http, https, mailto) y que el
//! hueco de una tarjeta de YouTube responda con la URL del vídeo.
//!
//! Ejecutar: nohup mutter --wayland --headless --wayland-display=scribe-links \
//!            --virtual-monitor 1000x800 & \
//!          GDK_BACKEND=wayland WAYLAND_DISPLAY=scribe-links \
//!            cargo test --test links_smoke -- --ignored --nocapture

// Los módulos de la app se incluyen vía #[path] y el harness solo ejercita
// parte de su API: el resto aparece como código muerto aunque la app sí lo
// use (mismo criterio que tests/render_shot.rs).
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
use markdown_view::MarkdownView;
use std::time::{Duration, Instant};

const DOC: &str = "\
# Enlaces y vídeos

Áéíóú ñÑ üÜ: acentos antes del enlace a la [web de GNOME](https://www.gnome.org) y después.

[fichero local](/etc/hostname)

https://youtu.be/dQw4w9WgXcQ
";

const URL_VIDEO: &str = "https://youtu.be/dQw4w9WgXcQ";
const URL_VIDEO_CANONICA: &str = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";

fn pump(ctx: &gtk4::glib::MainContext, ms: u64) {
    let t = Instant::now();
    while t.elapsed() < Duration::from_millis(ms) {
        while ctx.iteration(false) {}
        std::thread::sleep(Duration::from_millis(4));
    }
}

/// Offset de carácter correspondiente a un offset en bytes (el buffer indexa
/// por caracteres; el analizador devuelve bytes).
fn offset_de_caracter(texto: &str, byte: usize) -> i32 {
    texto[..byte].chars().count() as i32
}

/// Punto en mitad del carácter anterior a `byte`, en coordenadas del widget.
fn punto_en(view: &MarkdownView, texto: &str, byte: usize) -> (f64, f64) {
    let buffer = view.buffer();
    let iter = buffer.iter_at_offset(offset_de_caracter(texto, byte));
    // `iter_location` da el rectángulo del carácter anterior al iter, que sigue
    // estando dentro del enlace si se apunta a su mitad.
    let rect = view.iter_location(&iter);
    let bx = rect.x() + rect.width().max(2) / 2;
    let by = rect.y() + rect.height() / 2;
    let (wx, wy) = view.buffer_to_window_coords(gtk4::TextWindowType::Widget, bx, by);
    (wx as f64, wy as f64)
}

/// Y (coordenadas del widget) del principio de la línea que contiene `byte`:
/// el hueco donde se pinta la tarjeta va justo después del texto de esa línea.
fn inicio_de_linea(view: &MarkdownView, texto: &str, byte: usize) -> i32 {
    let buffer = view.buffer();
    let iter = buffer.iter_at_offset(offset_de_caracter(texto, byte));
    let rect = view.iter_location(&iter);
    let (_, wy) = view.buffer_to_window_coords(gtk4::TextWindowType::Widget, 0, rect.y());
    wy
}

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
    renderer
        .render_texture(&node, None)
        .save_to_png(path)
        .expect("guardar png");
}

#[test]
#[ignore = "requiere display: ejecutar bajo un compositor (véase la cabecera)"]
fn los_enlaces_y_las_tarjetas_responden_a_las_coordenadas() {
    gtk4::init().expect("GTK necesita un display; ejecuta bajo un compositor");
    libadwaita::init().expect("libadwaita init");
    let ctx = gtk4::glib::MainContext::default();

    let out = std::env::var("SCRIBE_SHOTS_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("scribe-links"));
    std::fs::create_dir_all(&out).expect("crear dir de salida");

    let editor = Editor::new();
    let win = gtk4::Window::builder()
        .default_width(900)
        .default_height(700)
        .build();
    win.set_child(Some(&editor.widget));
    win.present();
    editor.set_text(DOC);
    // Margen de sobra para el debounce de decoración, la maquetación y la
    // descarga de la miniatura de la tarjeta.
    pump(&ctx, 5000);

    let view = &editor.view;
    let analisis = markdown_render::analyze(DOC);

    // El enlace de markdown se abre; el de un fichero local, no. El punto es
    // el primer carácter del texto del enlace (y no su mitad) a propósito: el
    // documento lleva acentos antes, así que un desajuste entre los offsets en
    // bytes del analizador y los offsets en caracteres del buffer se nota aquí.
    let web = analisis
        .links
        .iter()
        .find(|l| l.dest == "https://www.gnome.org")
        .expect("el enlace a GNOME debería estar en el análisis");
    let (x, y) = punto_en(view, DOC, web.start + 1);
    assert_eq!(
        view.link_at_position(x, y).as_deref(),
        Some("https://www.gnome.org"),
        "el punto sobre el texto del enlace debería dar su URL"
    );

    let fichero = analisis
        .links
        .iter()
        .find(|l| l.dest == "/etc/hostname")
        .expect("el enlace al fichero debería estar en el análisis");
    let (x, y) = punto_en(view, DOC, fichero.start + 1);
    assert_eq!(
        view.link_at_position(x, y),
        None,
        "un destino que no es http(s) ni mailto no se ofrece"
    );

    // La decisión del clic: el texto solo se abre con Ctrl; sin Ctrl el clic
    // sigue siendo del texto (colocar el cursor, seleccionar).
    let (x, y) = punto_en(view, DOC, web.start + 1);
    assert_eq!(
        view.dest_under_pointer(x, y, false),
        None,
        "un clic normal sobre un enlace no debe abrirlo"
    );
    assert_eq!(
        view.dest_under_pointer(x, y, true).as_deref(),
        Some("https://www.gnome.org"),
        "Ctrl+clic sobre el enlace sí lo abre"
    );

    // Texto normal: no hay enlace que abrir. El documento lleva acentos a
    // propósito: si los offsets no se tradujeran de bytes a caracteres, este
    // punto caería sobre otro sitio del texto.
    let medio = DOC.find("acentos").expect("el texto tiene 'acentos'");
    let (x, y) = punto_en(view, DOC, medio + 2);
    assert_eq!(
        view.link_at_position(x, y),
        None,
        "el texto normal no es enlace"
    );

    // La URL suelta de YouTube no es un enlace de markdown (pulldown no crea
    // autolinks sin GFM): lo que se abre es la tarjeta.
    let video = DOC.find(URL_VIDEO).expect("el documento trae la URL");
    assert!(
        !analisis.links.iter().any(|l| l.dest == URL_VIDEO),
        "la URL suelta no debería ser un enlace"
    );
    let (x, y) = punto_en(view, DOC, video + URL_VIDEO.len() - 2);
    assert_eq!(
        view.link_at_position(x, y),
        None,
        "el texto del vídeo no es enlace"
    );
    let tarjeta = analisis.ornaments.iter().find_map(|o| match o {
        markdown_render::Ornament::Youtube { url, .. } => Some(url.clone()),
        _ => None,
    });
    assert_eq!(
        tarjeta.as_deref(),
        Some(URL_VIDEO_CANONICA),
        "la URL suelta debería producir la tarjeta del vídeo"
    );

    // El hueco de la tarjeta, en cambio, sí responde: se recorre su área
    // (por debajo de la línea) y todos los aciertos apuntan al mismo vídeo.
    let alto_linea = inicio_de_linea(view, DOC, video);
    let mut aciertos = 0;
    let mut dentro = None;
    for dy in (alto_linea..alto_linea + 320).step_by(6) {
        for dx in (40..860).step_by(20) {
            if let Some(url) = view.card_at_position(dx as f64, dy as f64) {
                assert_eq!(url, URL_VIDEO_CANONICA);
                aciertos += 1;
                dentro.get_or_insert((dx as f64 + 1.0, dy as f64 + 1.0));
            }
        }
    }
    assert!(
        aciertos > 100,
        "el área de la tarjeta debería ser grande; {aciertos} aciertos"
    );

    // El clic sobre la tarjeta la abre, con Ctrl o sin él.
    let (cx, cy) = dentro.expect("la tarjeta debería tener área");
    assert_eq!(
        view.dest_under_pointer(cx, cy, false).as_deref(),
        Some(URL_VIDEO_CANONICA),
        "el clic sobre la tarjeta abre el vídeo"
    );
    assert_eq!(
        view.dest_under_pointer(cx, cy, true).as_deref(),
        Some(URL_VIDEO_CANONICA),
        "con Ctrl también"
    );

    // Y fuera de ella no hay tarjeta que abrir.
    assert_eq!(
        view.card_at_position(400.0, 10.0),
        None,
        "arriba del documento no hay tarjeta"
    );

    shot(&win, &out.join("enlaces-01.png"));
    eprintln!("captura en {}", out.display());
}
