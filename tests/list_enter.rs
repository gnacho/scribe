//! Regresión de #186: Enter en medio de un elemento de lista parte el texto
//! en dos líneas y la segunda conserva el marcador, en vez de quedar como
//! línea suelta fuera de la lista.
//!
//! Ejecutar: xvfb-run -a cargo test --test list_enter -- --ignored --test-threads=1

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

/// Pone el cursor en el offset indicado y pulsa Intro con el manejador real.
fn enter_en(ed: &editor::Editor, offset: i32) -> bool {
    let buffer = ed.buffer();
    buffer.place_cursor(&buffer.iter_at_offset(offset));
    editor::continuar_lista_en_buffer(&buffer)
}

/// Texto del buffer con el cursor representado como `|`.
fn texto_con_cursor(ed: &editor::Editor) -> String {
    let buffer = ed.buffer();
    let texto = buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), true)
        .to_string();
    let pos = buffer.cursor_position() as usize;
    let mut salida: String = texto.chars().take(pos).collect();
    salida.push('|');
    salida.extend(texto.chars().skip(pos));
    salida
}

#[test]
#[ignore = "requiere display: xvfb-run -a cargo test --test list_enter -- --ignored --test-threads=1"]
fn enter_en_listas_conserva_el_marcador() {
    let _guardia = markdown_view::tests::gtk_compartido();

    let ed = editor::Editor::new();
    let window = gtk4::Window::new();
    window.set_default_size(700, 500);
    window.set_child(Some(&ed.widget));
    window.present();

    // Enter en medio de un elemento: la segunda mitad sigue en la lista.
    let linea = "- Anarchy (no world government above sovereign states) produces uncertainty (the intentions of other states cannot be known for sure)";
    ed.set_text(linea);
    let corte = linea
        .find(" produces")
        .expect("el texto de prueba lo tiene") as i32;
    assert!(
        enter_en(&ed, corte),
        "Enter en medio de un elemento deberia consumir la tecla"
    );
    assert_eq!(
        texto_con_cursor(&ed),
        format!(
            "{}\n- | produces uncertainty (the intentions of other states cannot be known for sure)",
            &linea[..corte as usize]
        ),
        "la segunda mitad tiene que seguir siendo parte de la lista"
    );

    // Enter al final: el comportamiento de siempre no cambia.
    ed.set_text("- primero");
    assert!(enter_en(&ed, 9));
    assert_eq!(texto_con_cursor(&ed), "- primero\n- |");

    // Lista numerada con el cursor en medio: avanza el número.
    ed.set_text("1. uno dos");
    assert!(enter_en(&ed, 5));
    assert_eq!(texto_con_cursor(&ed), "1. un\n2. |o dos");

    // Elemento vacío: Enter cierra la lista.
    ed.set_text("- algo\n- ");
    assert!(enter_en(&ed, 9));
    assert_eq!(texto_con_cursor(&ed), "- algo\n|");

    // Fuera de una lista Enter no se toca.
    ed.set_text("texto normal");
    assert!(!enter_en(&ed, 3));

    window.destroy();
}
