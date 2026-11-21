//! Regresión de #210: Tab indenta el elemento de lista entero (cuatro
//! espacios por nivel) y Shift+Tab lo desindenta, en vez de meter espacios o
//! un tabulador dentro del contenido del elemento.
//!
//! Ejecutar: xvfb-run -a cargo test --test list_indent -- --ignored --test-threads=1

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

/// Pone el cursor en el offset indicado y aplica la sangría con el manejador
/// real.
fn sangrar_en(ed: &editor::Editor, offset: i32, desindentar: bool) -> bool {
    let buffer = ed.buffer();
    buffer.place_cursor(&buffer.iter_at_offset(offset));
    editor::sangrar_lista_en_buffer(&buffer, desindentar)
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
#[ignore = "requiere display: xvfb-run -a cargo test --test list_indent -- --ignored --test-threads=1"]
fn tab_indenta_y_shift_tab_desindenta_el_elemento() {
    let _guardia = markdown_view::tests::gtk_compartido();

    let ed = editor::Editor::new();
    let window = gtk4::Window::new();
    window.set_default_size(700, 500);
    window.set_child(Some(&ed.widget));
    window.present();

    // Tab con el cursor en el texto: sangra la línea y el cursor la sigue.
    ed.set_text("- algo");
    assert!(sangrar_en(&ed, 2, false));
    assert_eq!(texto_con_cursor(&ed), "    - |algo");

    // Tab con el cursor al principio: la sangría entra delante y el cursor se
    // queda pegado al mismo carácter, como al insertar una tabulación.
    ed.set_text("- algo");
    assert!(sangrar_en(&ed, 0, false));
    assert_eq!(texto_con_cursor(&ed), "    |- algo");

    // Un segundo Tab añade otro nivel.
    ed.set_text("    - algo");
    assert!(sangrar_en(&ed, 6, false));
    assert_eq!(texto_con_cursor(&ed), "        - |algo");

    // Shift+Tab quita un nivel de cuatro espacios.
    ed.set_text("    - algo");
    assert!(sangrar_en(&ed, 6, true));
    assert_eq!(texto_con_cursor(&ed), "- |algo");

    // Shift+Tab quita también un tabulador de sangría.
    ed.set_text("\t- algo");
    assert!(sangrar_en(&ed, 3, true));
    assert_eq!(texto_con_cursor(&ed), "- |algo");

    // Con el elemento al ras la tecla se consume sin tocar el texto (si no, el
    // buffer se llevaría el foco al widget siguiente).
    ed.set_text("- algo");
    assert!(sangrar_en(&ed, 2, true));
    assert_eq!(texto_con_cursor(&ed), "- |algo");

    // Fuera de una lista la tecla no es nuestra.
    ed.set_text("texto normal");
    assert!(!sangrar_en(&ed, 3, false));
    assert!(!sangrar_en(&ed, 3, true));
    assert_eq!(texto_con_cursor(&ed), "tex|to normal");

    window.destroy();
}
