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
use std::rc::Rc;
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

    // ---- opciones y reemplazo (issue #50), sobre un editor nuevo ----
    let editor2 = Editor::new();
    editor2.set_text("Casi todo y nada; TODO dos veces: todo.");

    // Por defecto no distingue mayúsculas: las tres variantes cuentan.
    editor2.set_search_text("todo");
    pump(&ctx, 400);
    assert_eq!(editor2.search_state().1, 3, "sin distinguir mayúsculas");

    // Con «Aa» solo queda la minúscula exacta.
    editor2.set_search_case_sensitive(true);
    pump(&ctx, 400);
    assert_eq!(
        editor2.search_state().1,
        2,
        "distinguiendo mayúsculas quedan las dos minúsculas"
    );
    assert!(editor2.search_case_sensitive());
    editor2.set_search_case_sensitive(false);

    // Con regex el patrón es un patrón: «TODO+» con mayúsculas distinguidas
    // solo casa el grito.
    editor2.set_search_case_sensitive(true);
    editor2.set_search_regex(true);
    editor2.set_search_text("TODO+");
    pump(&ctx, 400);
    assert_eq!(editor2.search_state().1, 1, "la regex solo casa TODO");
    assert!(editor2.search_regex());
    editor2.set_search_regex(false);
    editor2.set_search_case_sensitive(false);

    // Reemplazar la selección actual y saltar a la siguiente coincidencia.
    editor2.set_search_text("todo");
    pump(&ctx, 400);
    assert!(editor2.search_next(), "salta a la primera");
    assert_eq!(seleccion(&editor2).as_deref(), Some("todo"));
    assert!(
        editor2.replace_current("nada"),
        "reemplaza la coincidencia seleccionada"
    );
    // El salto a la siguiente corre en un idle (tras la recomputación de
    // gtksourceview), así que hay que bombearlo como en la UI real.
    pump(&ctx, 400);
    assert_eq!(
        seleccion(&editor2).as_deref(),
        Some("TODO"),
        "tras reemplazar salta a la siguiente"
    );
    assert_eq!(
        editor2.text().matches("nada").count(),
        2,
        "quedan dos «nada» (el original y el reemplazo)"
    );

    // Reemplazar todas las que quedan de una vez. El recuento llega por el
    // callback tras el idle que hace el trabajo.
    let reemplazadas = Rc::new(std::cell::Cell::new(usize::MAX));
    {
        let reemplazadas = reemplazadas.clone();
        editor2.replace_all_matches("x", move |n| reemplazadas.set(n));
    }
    pump(&ctx, 400);
    assert_eq!(reemplazadas.get(), 2, "quedaban TODO y todo por reemplazar");
    assert!(
        !editor2.text().to_lowercase().contains("todo"),
        "sin «todo» (en ninguna capitalización) después de reemplazar todas"
    );

    // Sobre texto seleccionado a mano (no coincidencia) no se reemplaza nada.
    let editor3 = Editor::new();
    editor3.set_text("algo de texto");
    editor3.set_search_text("texto");
    pump(&ctx, 400);
    let buffer3 = editor3.view.buffer();
    let a = buffer3.start_iter();
    let mut b = a;
    b.forward_chars(4); // «algo»
    buffer3.select_range(&a, &b);
    assert!(
        !editor3.replace_current("x"),
        "sin reemplazo sobre una selección que no es coincidencia"
    );
    assert_eq!(editor3.text(), "algo de texto", "el texto queda intacto");
}
