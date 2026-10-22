//! Regresión de #115: al abrir un documento, la cabecera tiene que salir ya
//! con su tag aplicado. La decoración se difería al bucle principal, así que
//! el primer layout se pintaba con el texto a tamaño normal y el tag lo
//! agrandaba un ciclo después: eso es el salto de tamaño que se veía cuando
//! había texto encima. Lo que se comprueba aquí es que tras cargar el texto
//! (sin iterar el bucle principal) el tag ya está puesto.
//!
//! Ejecutar: xvfb-run -a cargo test --test heading_no_reflow -- --ignored --test-threads=1

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

use gtk4::prelude::*;

const DOC: &str = "\
Texto normal encima de la cabecera.

# Cabecera grande

Texto normal debajo.
";

#[test]
#[ignore = "requiere display: xvfb-run -a cargo test --test heading_no_reflow -- --ignored --test-threads=1"]
fn la_cabecera_ya_lleva_su_tag_al_cargar() {
    gtk4::init().expect("GTK necesita un display; ejecuta bajo xvfb-run");
    libadwaita::init().expect("libadwaita init");

    let ed = editor::Editor::new();
    let window = gtk4::Window::new();
    window.set_default_size(700, 600);
    window.set_child(Some(&ed.widget));
    window.present();

    // Sin `iteration` a proposito: se mide lo que hay antes de que corra el
    // bucle principal, que es donde antes llegaba la decoracion diferida.
    ed.set_text(DOC);

    let buffer = ed.buffer();
    let h1 = buffer.tag_table().lookup("h1").expect("el tag h1 existe");
    let iter = buffer.iter_at_line(2).expect("linea de la cabecera");
    assert!(
        iter.has_tag(&h1),
        "la cabecera no lleva el tag h1 nada mas cargar: la decoracion sigue siendo diferida (#115)"
    );

    // La primera linea es texto normal y no debe llevarlo.
    let normal = buffer.iter_at_line(0).expect("linea 0");
    assert!(
        !normal.has_tag(&h1),
        "el texto normal no deberia llevar el tag de cabecera"
    );

    window.destroy();
}
