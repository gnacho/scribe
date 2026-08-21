//! Estrés del escenario del crash (bug upstream GNOME/gtk#8346) a nivel de
//! `Editor` real: abrir un documento grande con marcado, drenar la decoración
//! diferida y forzar conversiones píxel→iter (el mismo mecanismo que un clic
//! o scroll del usuario). Con el ocultado de texto activo GTK aborta de forma
//! estadística (gtktextbtree.c:4012); con la opción B (nada invisible) debe
//! completar siempre.
//!
//! Ejecutar: xvfb-run -a cargo test --test crash_stress -- --ignored --nocapture

#[path = "../src/settings.rs"]
mod settings;
#[path = "../src/markdown_render.rs"]
mod markdown_render;
#[path = "../src/markdown_view.rs"]
mod markdown_view;
#[path = "../src/editor.rs"]
mod editor;

use editor::Editor;
use gtk4::prelude::*;
use std::time::{Duration, Instant};

/// Documento tortura generado en Rust: tablas anchas con formato inline en
/// celdas, vallas, listas con casillas, citas, reglas… >150 KB (< límite del
/// modo vivo) para que la decoración entre en juego.
fn torture_doc() -> String {
    let words = [
        "lorem", "ipsum", "dolor", "sit", "amet", "consectetur", "adipiscing",
        "elit", "sed", "do", "eiusmod", "tempor",
    ];
    let mut s = String::with_capacity(220_000);
    let mut section = 0;
    while s.len() < 160_000 {
        s.push_str(&format!("\n## Sección {section}\n\n"));
        s.push_str("| Col A | Col B | Col C | Col D |\n|---|---|---|---|\n");
        for r in 0..24 {
            let w = |i: usize| words[(section * 7 + r * 3 + i) % words.len()];
            s.push_str(&format!(
                "| `{}` | **{}** | _{}_ {} | {}<br>{} |\n",
                w(0), w(1), w(2), w(3), w(4), w(5)
            ));
        }
        s.push_str("\n> cita con **marcado** y `código`\n\n");
        s.push_str("```python\ndef f():\n    return 1  # comentario\n```\n\n");
        for i in 0..8 {
            let mark = if i % 3 == 0 { "[x]" } else { "[ ]" };
            s.push_str(&format!("- {mark} elemento **{i}** con `código`\n"));
        }
        s.push_str("\n---\n\n");
        section += 1;
    }
    s
}

/// Bombea el main context durante `ms` milisegundos: drena el timeout de
/// decoración (0/45 ms) y las idles de validación de GTK (prioridades 108/125).
fn pump(ctx: &gtk4::glib::MainContext, ms: u64) {
    let t = Instant::now();
    while t.elapsed() < Duration::from_millis(ms) {
        while ctx.iteration(false) {}
        std::thread::sleep(Duration::from_millis(4));
    }
}

#[test]
#[ignore = "requiere display: xvfb-run -a cargo test --test crash_stress -- --ignored --nocapture"]
fn abrir_documento_grande_no_aborta() {
    gtk4::init().expect("GTK necesita un display; ejecuta bajo xvfb-run");
    libadwaita::init();

    let editor = Editor::new();
    let win = gtk4::Window::builder()
        .default_width(900)
        .default_height(700)
        .build();
    win.set_child(Some(&editor.widget));
    win.present();

    let doc = torture_doc();
    assert!(doc.len() > 150_000, "el doc debe estar bajo el límite pero ser grande");
    let ctx = gtk4::glib::MainContext::default();

    for i in 0..60 {
        editor.set_text(&doc);
        pump(&ctx, 100);
        // Acelerante: conversiones píxel→iter sobre todo el alto visible.
        let height = editor.view.height().max(1);
        let mut y = 0;
        while y < height {
            let _ = editor.view.iter_at_position(10, y);
            y += 37;
        }
        let line = (i * 13) % editor.line_count().max(1) as usize;
        editor.scroll_to_line(line as i32);
        pump(&ctx, 30);
    }
    // Con el bug activo este test muere con SIGABRT antes de llegar aquí.
}
