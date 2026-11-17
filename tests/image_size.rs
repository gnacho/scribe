//! Regresión de #194: el tamaño pedido en el alt (`![foto|480](x)`) cambia el
//! hueco que se reserva bajo la línea, y la acción del menú contextual lo
//! escribe en el buffer sin tocar el resto de la imagen.
//!
//! Un solo test a propósito: GTK solo se inicializa una vez por proceso y dos
//! tests con ventana en el mismo binario abortan.
//!
//! Ejecutar: xvfb-run -a cargo test --test image_size -- --ignored --test-threads=1

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

use gtk4::glib;
use gtk4::prelude::*;

/// Bombea el bucle principal hasta que el decorado perezoso se aplica.
fn espera_decorado() {
    let ctx = glib::MainContext::default();
    let t = std::time::Instant::now();
    while t.elapsed() < std::time::Duration::from_millis(250) {
        while ctx.iteration(false) {}
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// PNG de 1x1 transparente, para tener una textura real sin depender de
/// ningún fichero del repo.
fn png_temporal(nombre: &str) -> std::path::PathBuf {
    const UNO_X_UNO: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
    let bytes = glib::base64_decode(UNO_X_UNO);
    let ruta = std::env::temp_dir().join(nombre);
    std::fs::write(&ruta, bytes).expect("escribir el PNG de prueba");
    ruta
}

/// Alto (`pixels_below_lines`) del tag de hueco aplicado al offset dado.
fn alto_del_hueco(ed: &editor::Editor, offset: i32) -> Option<i32> {
    ed.buffer()
        .iter_at_offset(offset)
        .tags()
        .iter()
        .map(|t| t.pixels_below_lines())
        .find(|px| *px > 0)
}

/// Deja el cursor al final del documento: en la línea de la imagen el hueco
/// colapsa a propósito (#185), así que hay que salir para poder medirlo.
fn cursor_al_final(ed: &editor::Editor) {
    let buffer = ed.buffer();
    buffer.place_cursor(&buffer.end_iter());
    ed.refresh();
    espera_decorado();
}

#[test]
#[ignore = "requiere display: xvfb-run -a cargo test --test image_size -- --ignored --test-threads=1"]
fn el_tamano_del_alt_manda_en_el_hueco_y_el_menu_lo_escribe() {
    let _guardia = markdown_view::tests::gtk_compartido();

    let ed = editor::Editor::new();
    let window = gtk4::Window::new();
    window.set_default_size(700, 500);
    window.set_child(Some(&ed.widget));
    window.present();
    // El ancho real decide cuánto se puede escalar: sin él la columna mide 0.
    let ctx = glib::MainContext::default();
    let t = std::time::Instant::now();
    while ed.widget.width() < 100 && t.elapsed() < std::time::Duration::from_secs(2) {
        while ctx.iteration(false) {}
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    let png = png_temporal("scribe-194.png");
    // Ancho y alto explícitos: el hueco no depende del ratio del fichero.
    ed.set_text(&format!(
        "![foto|480x320]({})\n\ntexto final\n",
        png.display()
    ));
    cursor_al_final(&ed);
    let alto = alto_del_hueco(&ed, 0).expect("la imagen tiene que reservar su hueco");
    assert!(
        (320..=330).contains(&alto),
        "el hueco deberia medir lo que la imagen pedida, mide {alto}"
    );

    // Sin tamaño pedido, la 1x1 no se agranda y el hueco se ajusta a ella.
    ed.set_text(&format!("![foto]({})\n\ntexto final\n", png.display()));
    cursor_al_final(&ed);
    let alto = alto_del_hueco(&ed, 0).expect("la imagen sigue reservando hueco");
    assert!(
        (1..20).contains(&alto),
        "sin tamaño pedido el hueco mide la imagen, mide {alto}"
    );

    // Acciones del menú contextual: solo tocan el alt. Basta con que la línea
    // sea una imagen en bloque, el fichero no necesita existir.
    ed.set_text("![foto](/tmp/no-existe-194.png \"Titulo\")\n");
    ed.refresh();
    espera_decorado();
    ed.buffer().place_cursor(&ed.buffer().start_iter());

    let _ = ed
        .view()
        .activate_action("imagen.tamano", Some(&glib::Variant::from(480u32)));
    assert_eq!(
        texto(&ed),
        "![foto|480](/tmp/no-existe-194.png \"Titulo\")\n",
        "la accion solo debe tocar el alt y conservar destino y titulo"
    );

    let _ = ed
        .view()
        .activate_action("imagen.alinear", Some(&glib::Variant::from("right")));
    assert_eq!(
        texto(&ed),
        "![foto|480|right](/tmp/no-existe-194.png \"Titulo\")\n"
    );

    // Original size quita el tamaño y deja la alineación.
    let _ = ed
        .view()
        .activate_action("imagen.tamano", Some(&glib::Variant::from(0u32)));
    assert_eq!(
        texto(&ed),
        "![foto|right](/tmp/no-existe-194.png \"Titulo\")\n"
    );

    window.destroy();
}

fn texto(ed: &editor::Editor) -> String {
    let buffer = ed.buffer();
    buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), true)
        .to_string()
}
