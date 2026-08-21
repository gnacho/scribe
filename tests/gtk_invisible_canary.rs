//! Canario del bug de GTK que obliga a Scribe a no usar texto invisible.
//!
//! GTK aborta en `gtk_text_iter_set_visible_line_index` («byte index off the
//! end of the line») cuando un buffer contiene texto invisible y un índice de
//! la maquetación perezosa apunta más allá del contenido visible de una línea
//! (GNOME/gtk#8346). El fix es el MR !10228, aún sin publicar en ninguna
//! versión de GTK.
//!
//! Este test reproduce el mecanismo mínimo: **si el GTK del sistema es
//! vulnerable, el proceso muere con SIGABRT** (un `g_error` dentro de GTK no
//! se puede capturar desde Rust). Mientras eso siga pasando,
//! `settings::gtk_hides_invisible_safely()` debe devolver `false`.
//!
//! Necesita display, así que está marcado `#[ignore]`. Para ejecutarlo:
//!
//! ```sh
//! xvfb-run -a cargo test --test gtk_invisible_canary -- --ignored
//! ```

use gtk4::prelude::*;

#[test]
#[ignore]
fn visible_line_index_no_se_sale_con_texto_invisible() {
    gtk4::init().expect("GTK necesita un display; ejecuta bajo xvfb-run");
    let buffer = gtk4::TextBuffer::new(None::<&gtk4::TextTagTable>);
    let tag = gtk4::TextTag::builder()
        .name("hide")
        .invisible(true)
        .build();
    buffer.tag_table().add(&tag);
    buffer.set_text("abcdef\nghijkl\n");
    // Oculta "cd": la longitud visible de la línea 0 pasa de 6 a 4 bytes.
    let s = buffer.iter_at_offset(2);
    let e = buffer.iter_at_offset(4);
    buffer.apply_tag(&tag, &s, &e);
    // Pide el índice visible 6 (> visible real). En un GTK vulnerable el
    // escaneo cruza a la línea 1 y aborta con g_error (el test muere con
    // SIGABRT). En un GTK con !10228, el iterador queda dentro del buffer.
    let mut it = buffer.iter_at_line(0).unwrap();
    it.set_visible_line_index(6);
    assert!(it.line() <= 1);
}
