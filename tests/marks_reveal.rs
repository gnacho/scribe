//! Regresión del ocultado por elemento (#117): los marcadores inline (`**`,
//! backticks) se revelan cuando el cursor está dentro del elemento que
//! delimitan o tocando sus bordes, y permanecen encogidos fuera, aunque sea en
//! la misma línea.
//!
//! Ejecutar: xvfb-run -a cargo test --test marks_reveal -- --ignored --test-threads=1

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

const DOC: &str = "un **texto** y mas\n";

/// Nombre del tag de visibilidad aplicado al carácter en `offset`.
fn tag_en(ed: &editor::Editor, offset: i32) -> String {
    let buffer = ed.buffer();
    let iter = buffer.iter_at_offset(offset);
    iter.tags()
        .iter()
        .filter_map(|t| t.name().map(|n| n.to_string()))
        .find(|n| n.starts_with("syn_"))
        .unwrap_or_default()
}

#[test]
#[ignore = "requiere display: xvfb-run -a cargo test --test marks_reveal -- --ignored --test-threads=1"]
fn la_marca_se_revela_dentro_del_elemento_y_fuera_no() {
    let _guardia = markdown_view::tests::gtk_compartido();

    let ed = editor::Editor::new();
    let window = gtk4::Window::new();
    window.set_default_size(700, 500);
    window.set_child(Some(&ed.widget));
    window.present();
    ed.set_text(DOC);

    let buffer = ed.buffer();
    // "un **texto** y mas": elemento de negrita en caracteres 3..=12.
    let casos: Vec<(i32, bool)> = vec![
        (0, false),  // antes de todo: encogida
        (3, true),   // `|**texto**`: borde de apertura
        (7, true),   // `**te|xto**`: dentro
        (12, true),  // `**texto**|`: borde de cierre
        (13, false), // espacio posterior, misma linea: encogida
        (18, false), // ultima linea: encogida
    ];
    for (offset, esperado) in casos {
        buffer.place_cursor(&buffer.iter_at_offset(offset));
        ed.refresh();
        let ctx = gtk4::glib::MainContext::default();
        let t = std::time::Instant::now();
        while t.elapsed() < std::time::Duration::from_millis(120) {
            while ctx.iteration(false) {}
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let tag = tag_en(&ed, 4); // el segundo `*` de la apertura
        let obtenido = tag == "syn_shown";
        assert_eq!(
            obtenido,
            esperado,
            "cursor en {offset}: se esperaba {} y hay {tag}",
            if esperado { "revelada" } else { "encogida" }
        );
    }

    window.destroy();
}
