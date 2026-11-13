//! Regresión de #185: con el cursor en la línea de una imagen o de una
//! tarjeta de vídeo en bloque, el fuente se ve crudo y el hueco donde se pinta
//! el adorno colapsa. Fuera de esa línea el adorno vuelve.
//!
//! Ejecutar: xvfb-run -a cargo test --test image_reveal -- --ignored --test-threads=1

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
        .filter(|n| n.starts_with("syn_") || n == "imagegap" || n == "youtubegap")
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
#[ignore = "requiere display: xvfb-run -a cargo test --test image_reveal -- --ignored --test-threads=1"]
fn la_imagen_en_bloque_revela_su_fuente_en_la_linea_del_cursor() {
    let _guardia = markdown_view::tests::gtk_compartido();

    let ed = editor::Editor::new();
    let window = gtk4::Window::new();
    window.set_default_size(700, 500);
    window.set_child(Some(&ed.widget));
    window.present();

    let doc = "![](images/foto.png)\n\ntexto final\n";
    ed.set_text(doc);
    let apertura = 0; // el `!` de `![`
    let cierre = doc.find("](").expect("el texto de prueba lo tiene") as i32;

    // Cursor en la línea de la imagen: fuente crudo y sin hueco reservado.
    pon_cursor(&ed, apertura);
    let tags = tags_en(&ed, apertura);
    assert!(
        tags.iter().any(|t| t == "syn_shown"),
        "la marca de imagen deberia revelarse, tags: {tags:?}"
    );
    assert!(
        !tags.iter().any(|t| t == "imagegap"),
        "el hueco de la imagen deberia colapsar, tags: {tags:?}"
    );
    let tags_cierre = tags_en(&ed, cierre);
    assert!(
        tags_cierre.iter().any(|t| t == "syn_shown"),
        "el cierre de la imagen deberia revelarse, tags: {tags_cierre:?}"
    );

    // Cursor fuera: el hueco vuelve y las marcas se encogen.
    pon_cursor(&ed, (doc.len() - 2) as i32);
    let tags = tags_en(&ed, apertura);
    assert!(
        tags.iter().any(|t| t == "imagegap"),
        "fuera de la linea el hueco tiene que volver, tags: {tags:?}"
    );
    assert!(
        tags.iter().any(|t| t == "syn_shrink"),
        "fuera de la linea la marca se encoge, tags: {tags:?}"
    );

    // Misma regla para la tarjeta de vídeo, que reserva su propio hueco.
    let doc_video = "[video](https://www.youtube.com/watch?v=dQw4w9WgXcQ)\n\nfinal\n";
    ed.set_text(doc_video);
    pon_cursor(&ed, 0);
    let tags = tags_en(&ed, 0);
    assert!(
        tags.iter().any(|t| t == "syn_shown"),
        "el enlace del video deberia revelarse, tags: {tags:?}"
    );
    assert!(
        !tags.iter().any(|t| t == "youtubegap"),
        "el hueco del video deberia colapsar, tags: {tags:?}"
    );
    pon_cursor(&ed, (doc_video.len() - 2) as i32);
    let tags = tags_en(&ed, 0);
    assert!(
        tags.iter().any(|t| t == "youtubegap"),
        "fuera de la linea el hueco del video tiene que volver, tags: {tags:?}"
    );

    window.destroy();
}
