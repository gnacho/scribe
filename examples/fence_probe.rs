//! Sonda de eventos de pulldown-cmark: vuelca el flujo de eventos de un
//! fichero Markdown para diagnosticar cómo el parser interpreta vallas,
//! tablas, imágenes, etc. Útil cuando el renderizado difiere de lo esperado.
//!
//! Uso: cargo run --example fence_probe -- ruta/al/fichero.md

fn main() {
    let text = std::fs::read_to_string(std::env::args().nth(1).unwrap()).unwrap();
    let opts = pulldown_cmark::Options::ENABLE_TABLES
        | pulldown_cmark::Options::ENABLE_FOOTNOTES
        | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
        | pulldown_cmark::Options::ENABLE_TASKLISTS;
    use pulldown_cmark::Event::*;
    use pulldown_cmark::Tag;
    let mut n = 0;
    for ev in pulldown_cmark::Parser::new_ext(&text, opts) {
        n += 1;
        match ev {
            Start(Tag::CodeBlock(k)) => println!("FENCE OPEN {k:?}"),
            End(pulldown_cmark::TagEnd::CodeBlock) => println!("FENCE CLOSE"),
            Start(Tag::Image { dest_url, .. }) => println!("IMAGE dest={dest_url}"),
            Start(Tag::Heading { level, .. }) => println!("HEADING {level:?}"),
            Start(Tag::Table(_)) => println!("TABLE"),
            _ => {}
        }
    }
    println!("TOTAL EVENTOS: {n}");
}
