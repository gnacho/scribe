//! Regresión de #169: las viñetas se alinean con la primera línea de texto y
//! las continuaciones de un elemento empiezan a la misma X que esa primera
//! línea, sin el residuo del marcador Markdown encogido.
//!
//! Ejecutar: xvfb-run -a cargo test --test bullet_alignment -- --ignored --test-threads=1

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
use settings::FontFamily;
use std::time::{Duration, Instant};

const DOCUMENTO: &str = "- Texto llano\n- **Negrita** al inicio\n- Elemento suficientemente largo para envolver varias veces dentro de una ventana estrecha y comprobar la alineación del texto.\n  - Elemento anidado también suficientemente largo para envolver y comprobar que su continuación conserva la sangría.\n    - Tercer nivel con otro elemento largo que envuelve y permite medir la alineación de todo el texto.\n\nfuera de la lista\n";

fn pump(ctx: &gtk4::glib::MainContext, ms: u64) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(ms) {
        while ctx.iteration(false) {}
        std::thread::sleep(Duration::from_millis(4));
    }
}

fn editor_con_texto(text: &str, width: i32, size: i32) -> (gtk4::Window, Editor) {
    let css = gtk4::CssProvider::new();
    let display = gtk4::gdk::Display::default().expect("display bajo Xvfb");
    gtk4::style_context_add_provider_for_display(
        &display,
        &css,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    editor::install_app_css(css);

    let ed = Editor::new();
    ed.set_font(FontFamily::Sans, size, 1.7);
    let window = gtk4::Window::new();
    window.set_default_size(width, 500);
    window.set_child(Some(&ed.widget));
    window.present();
    ed.set_text(text);
    let last_line = text.lines().count().saturating_sub(1) as u32;
    ed.place_cursor_at(last_line + 1, 1);
    pump(&gtk4::glib::MainContext::default(), 700);
    (window, ed)
}

fn primer_rectangulo_visible(
    view: &markdown_view::MarkdownView,
    buffer: &impl IsA<gtk4::TextBuffer>,
    line: i32,
) -> gtk4::gdk::Rectangle {
    let hidden = buffer.tag_table().lookup("syn_shrink");
    let mut iter = buffer.iter_at_line(line).expect("inicio del elemento");
    let range = view.line_yrange(&iter);
    loop {
        let visible = match hidden.as_ref() {
            Some(tag) => !iter.has_tag(tag),
            None => true,
        };
        let rect = view.iter_location(&iter);
        if visible && rect.height() > 0 {
            return rect;
        }
        assert!(
            iter.forward_char(),
            "la línea {line} no tiene texto visible"
        );
        assert_eq!(
            view.line_yrange(&iter),
            range,
            "no hay glifo visible en la línea {line}"
        );
    }
}

#[test]
#[ignore = "requiere display"]
fn vinetas_y_continuaciones_se_alinean_con_el_texto() {
    let _guardia = markdown_view::tests::gtk_compartido();
    for size in [9, 16, 20, 24, 32, 40] {
        let (window, ed) = editor_con_texto(DOCUMENTO, 420, size);
        let view = &ed.view;
        let buffer = ed.buffer();

        for line in 0..5 {
            let body_rect = primer_rectangulo_visible(view, &buffer, line);
            let expected = body_rect.y() as f32 + body_rect.height() as f32 / 2.0;
            let (_, current) = view
                .bullet_center_for_test(line)
                .expect("la viñeta tiene geometría");
            assert!(
                (current - expected).abs() <= 1.0,
                "fuente {size}, línea {line}: centro de viñeta {current}, centro del texto {expected}, cuerpo={body_rect:?}"
            );
        }

        for line in 2..5 {
            let first_x = primer_rectangulo_visible(view, &buffer, line).x();
            let mut continuation = buffer.iter_at_line(line).expect("inicio del elemento");
            assert!(
                view.forward_display_line(&mut continuation),
                "fuente {size}, línea {line}: el elemento debe ocupar al menos dos líneas visuales"
            );
            let continuation_x = view.iter_location(&continuation).x();
            assert_eq!(
                first_x, continuation_x,
                "fuente {size}, línea {line}: la primera línea empieza en {first_x} y la continuación en {continuation_x}"
            );
        }

        window.destroy();
    }
}
