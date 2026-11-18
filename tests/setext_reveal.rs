//! Regresión de #200: el subrayado setext se ve mientras el cursor está en su
//! línea, para poder editarlo. Antes se encogía siempre porque el tramo
//! empieza en el salto de línea anterior.
//!
//! Ejecutar: xvfb-run -a cargo test --test setext_reveal -- --ignored --test-threads=1

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

/// Nombres de los tags de sintaxis aplicados al carácter en `offset`.
fn tags_en(ed: &editor::Editor, offset: i32) -> Vec<String> {
    let buffer = ed.buffer();
    buffer
        .iter_at_offset(offset)
        .tags()
        .iter()
        .filter_map(|t| t.name().map(|n| n.to_string()))
        .filter(|n| n.starts_with("syn_"))
        .collect()
}

/// Bombea el bucle principal hasta que el decorado perezoso se aplica.
fn espera_decorado() {
    let ctx = gtk4::glib::MainContext::default();
    let t = std::time::Instant::now();
    while t.elapsed() < std::time::Duration::from_millis(200) {
        while ctx.iteration(false) {}
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

fn pon_cursor(ed: &editor::Editor, offset: i32) {
    let buffer = ed.buffer();
    buffer.place_cursor(&buffer.iter_at_offset(offset));
    ed.refresh();
    espera_decorado();
}

#[test]
#[ignore = "requiere display: xvfb-run -a cargo test --test setext_reveal -- --ignored --test-threads=1"]
fn el_subrayado_setext_se_ve_con_el_cursor_en_su_linea() {
    let _guardia = markdown_view::tests::gtk_compartido();

    let ed = editor::Editor::new();
    let window = gtk4::Window::new();
    window.set_default_size(700, 500);
    window.set_child(Some(&ed.widget));
    window.present();

    let doc = "Titulo\n===\n";
    ed.set_text(doc);
    let subrayado = doc.find("===").expect("el texto de prueba lo tiene") as i32;

    // Con el cursor en la línea del subrayado, el fuente se ve crudo.
    pon_cursor(&ed, subrayado);
    let tags = tags_en(&ed, subrayado);
    assert!(
        tags.iter().any(|t| t == "syn_shown"),
        "el subrayado deberia revelarse en su linea, tags: {tags:?}"
    );

    // Con el cursor en la cabecera, el subrayado vuelve a esconderse.
    pon_cursor(&ed, 0);
    let tags = tags_en(&ed, subrayado);
    assert!(
        !tags.iter().any(|t| t == "syn_shown"),
        "fuera de su linea el subrayado se esconde, tags: {tags:?}"
    );
}
