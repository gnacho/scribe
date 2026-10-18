//! Comprobación headless de la búsqueda en el documento (issue #37): recuento
//! de coincidencias, posición, saltos con vuelta al final, y que el salto deja
//! la coincidencia seleccionada (GtkSourceSearchContext solo devuelve el rango;
//! seleccionarlo es responsabilidad nuestra).
//!
//! Ejecutar: xvfb-run -a cargo test --test find_smoke -- --ignored --nocapture

// Los módulos de la app se incluyen vía #[path] y el harness solo ejercita
// parte de su API: el resto aparece como código muerto aunque la app sí lo
// use (mismo criterio que tests/render_shot.rs).
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
# Buscar

aguja en el pajar, otra aguja y una tercera aguja.
";

fn pump(ctx: &gtk4::glib::MainContext, ms: u64) {
    let t = Instant::now();
    while t.elapsed() < Duration::from_millis(ms) {
        while ctx.iteration(false) {}
        std::thread::sleep(Duration::from_millis(4));
    }
}

/// Lo que hay seleccionado en el buffer (lo que el usuario ve resaltado).
fn seleccion(editor: &Editor) -> Option<String> {
    let buffer = editor.view.buffer();
    buffer
        .selection_bounds()
        .map(|(start, end)| buffer.text(&start, &end, true).to_string())
}

#[test]
#[ignore = "requiere display: xvfb-run -a cargo test --test find_smoke -- --ignored --nocapture"]
fn recuento_saltos_y_vuelta_al_final() {
    gtk4::init().expect("GTK necesita un display; ejecuta bajo xvfb-run");
    libadwaita::init().expect("libadwaita init");
    let ctx = gtk4::glib::MainContext::default();

    let editor = Editor::new();
    editor.set_text(DOC);

    // Sin patrón no hay nada que contar ni que saltar.
    assert_eq!(
        editor.search_state(),
        (0, 0),
        "sin patrón, cero coincidencias"
    );
    assert!(!editor.search_next(), "sin patrón no se salta");

    // El recuento es asíncrono (llega por señal), así que se bombea.
    editor.set_search_text("aguja");
    pump(&ctx, 400);
    let (_, total) = editor.search_state();
    assert_eq!(total, 3, "tres agujas en el documento");
    assert!(
        !editor.view.buffer().has_selection(),
        "fijar el patrón no selecciona nada"
    );

    // Primero de tres.
    assert!(editor.search_next(), "hay una coincidencia a la que saltar");
    assert_eq!(seleccion(&editor).as_deref(), Some("aguja"));
    assert_eq!(editor.search_state().0, 1, "posición 1 de 3");

    assert!(editor.search_next());
    assert_eq!(editor.search_state().0, 2, "posición 2 de 3");
    assert!(editor.search_previous());
    assert_eq!(editor.search_state().0, 1, "vuelve a la posición 1");

    // Vuelta al final: desde la última, la siguiente es la primera.
    assert!(editor.search_next());
    assert!(editor.search_next());
    assert_eq!(editor.search_state().0, 3, "tercera y última");
    assert!(editor.search_next(), "da la vuelta");
    assert_eq!(editor.search_state().0, 1, "vuelve a empezar");

    // Del mismo modo hacia atrás: desde la primera, la anterior es la última.
    assert!(editor.search_previous(), "da la vuelta hacia atrás");
    assert_eq!(
        editor.search_state().0,
        3,
        "la anterior a la primera es la última"
    );

    // Un patrón que no está deja todo a cero.
    editor.set_search_text("ornitorrinco");
    pump(&ctx, 400);
    assert_eq!(editor.search_state(), (0, 0), "sin coincidencias");

    // El resaltado lo gobierna la ventana: encendido y apagado.
    editor.set_search_highlight(false);
    assert!(!editor.search_context().is_highlight());
    editor.set_search_highlight(true);
    assert!(editor.search_context().is_highlight());
}
