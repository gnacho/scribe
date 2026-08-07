//! Cálculo de los tramos que hay que decorar sobre el buffer del editor.
//!
//! No toca GTK: recibe el texto Markdown y devuelve rangos en bytes con el
//! nombre del `GtkTextTag` que les corresponde. Así se puede probar sin display.
//!
//! Los tramos marcados como `syntax` son las marcas del Markdown (`**`, `#`,
//! backticks, la URL de un enlace…). El editor las oculta salvo cuando el cursor
//! está en su línea, que es lo que hace que la edición se sienta WYSIWYG.

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, LinkType, Options, Parser, Tag, TagEnd};

/// Por encima de este tamaño se deja de decorar en vivo, para no bloquear la UI.
pub const MAX_LIVE_BYTES: usize = 400_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub tag: &'static str,
    pub syntax: bool,
}

fn style(start: usize, end: usize, tag: &'static str) -> Span {
    Span {
        start,
        end,
        tag,
        syntax: false,
    }
}

fn syn(start: usize, end: usize) -> Span {
    Span {
        start,
        end,
        tag: "syntax",
        syntax: true,
    }
}

fn heading_tag(level: HeadingLevel) -> &'static str {
    match level {
        HeadingLevel::H1 => "h1",
        HeadingLevel::H2 => "h2",
        HeadingLevel::H3 => "h3",
        HeadingLevel::H4 => "h4",
        HeadingLevel::H5 => "h5",
        HeadingLevel::H6 => "h6",
    }
}

/// Recorta el salto de línea final de un rango de bloque.
fn trim_nl(text: &str, mut end: usize) -> usize {
    while end > 0 && (text.as_bytes()[end - 1] == b'\n' || text.as_bytes()[end - 1] == b'\r') {
        end -= 1;
    }
    end
}

/// Marca los delimitadores de un elemento en línea (`**`, `_`, `~~`, backticks).
fn mark_delims(out: &mut Vec<Span>, start: usize, end: usize, len: usize) {
    if len == 0 || end < start + len * 2 {
        return;
    }
    out.push(syn(start, start + len));
    out.push(syn(end - len, end));
}

fn line_ranges(text: &str, start: usize, end: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut cursor = start;
    while cursor < end {
        let rel = text[cursor..end].find('\n');
        let stop = match rel {
            Some(i) => cursor + i,
            None => end,
        };
        out.push((cursor, stop));
        cursor = stop + 1;
    }
    out
}

/// Longitud de la marca `>` (con sus espacios) al principio de una línea de cita.
fn quote_marker_len(line: &str) -> usize {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i] == b' ' && i < 3 {
        i += 1;
    }
    if i >= bytes.len() || bytes[i] != b'>' {
        return 0;
    }
    i += 1;
    if i < bytes.len() && bytes[i] == b' ' {
        i += 1;
    }
    i
}

/// Longitud del marcador de un elemento de lista (`- `, `* `, `1. `…).
fn item_marker_len(rest: &str) -> usize {
    let bytes = rest.as_bytes();
    if bytes.is_empty() {
        return 0;
    }
    let mut i = 0;
    if bytes[0] == b'-' || bytes[0] == b'*' || bytes[0] == b'+' {
        i = 1;
    } else {
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == 0 || i >= bytes.len() || (bytes[i] != b'.' && bytes[i] != b')') {
            return 0;
        }
        i += 1;
    }
    while i < bytes.len() && bytes[i] == b' ' {
        i += 1;
    }
    i
}

pub fn spans(text: &str) -> Vec<Span> {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_TASKLISTS);

    let mut out: Vec<Span> = Vec::new();
    let mut list_depth = 0usize;

    for (event, range) in Parser::new_ext(text, opts).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                let end = trim_nl(text, range.end);
                if end <= range.start {
                    continue;
                }
                out.push(style(range.start, end, heading_tag(level)));
                let src = &text[range.start..end];
                let hashes = src.bytes().take_while(|b| *b == b'#').count();
                if hashes > 0 {
                    let spaces = src[hashes..].bytes().take_while(|b| *b == b' ').count();
                    out.push(syn(range.start, range.start + hashes + spaces));
                } else {
                    // Cabecera setext: se oculta el subrayado `===` / `---`
                    // junto con el salto de línea que lo separa del título,
                    // para que no quede un renglón en blanco.
                    let lines = line_ranges(text, range.start, end);
                    if let Some(&(ls, le)) = lines.last() {
                        let underline = text[ls..le].trim();
                        let is_rule = !underline.is_empty()
                            && (underline.bytes().all(|b| b == b'=')
                                || underline.bytes().all(|b| b == b'-'));
                        if is_rule && ls > range.start {
                            out.push(syn(ls - 1, le));
                        }
                    }
                }
            }

            Event::Start(Tag::Strong) => {
                out.push(style(range.start, range.end, "bold"));
                mark_delims(&mut out, range.start, range.end, 2);
            }
            Event::Start(Tag::Emphasis) => {
                out.push(style(range.start, range.end, "italic"));
                mark_delims(&mut out, range.start, range.end, 1);
            }
            Event::Start(Tag::Strikethrough) => {
                out.push(style(range.start, range.end, "strike"));
                mark_delims(&mut out, range.start, range.end, 2);
            }

            Event::Code(_) => {
                out.push(style(range.start, range.end, "code"));
                let ticks = text[range.start..range.end]
                    .bytes()
                    .take_while(|b| *b == b'`')
                    .count();
                mark_delims(&mut out, range.start, range.end, ticks);
            }

            Event::Start(Tag::Link { link_type, .. }) => {
                out.push(style(range.start, range.end, "link"));
                let src = &text[range.start..range.end];
                match link_type {
                    // `<https://…>` y `<a@b.com>`: solo sobran los ángulos.
                    LinkType::Autolink | LinkType::Email => {
                        if src.starts_with('<') && src.ends_with('>') && src.len() > 2 {
                            out.push(syn(range.start, range.start + 1));
                            out.push(syn(range.end - 1, range.end));
                        }
                    }
                    _ => {
                        if src.starts_with('[') {
                            if let Some(idx) = src.rfind("](") {
                                out.push(syn(range.start, range.start + 1));
                                out.push(syn(range.start + idx, range.end));
                            }
                        }
                    }
                }
            }
            Event::Start(Tag::Image { .. }) => {
                out.push(style(range.start, range.end, "link"));
                let src = &text[range.start..range.end];
                if src.starts_with("![") {
                    if let Some(idx) = src.rfind("](") {
                        out.push(syn(range.start, range.start + 2));
                        out.push(syn(range.start + idx, range.end));
                    }
                }
            }

            Event::Start(Tag::BlockQuote(_)) => {
                let end = trim_nl(text, range.end);
                out.push(style(range.start, end, "quote"));
                for (ls, le) in line_ranges(text, range.start, end) {
                    let n = quote_marker_len(&text[ls..le]);
                    if n > 0 {
                        out.push(syn(ls, ls + n));
                    }
                }
            }

            Event::Start(Tag::List(_)) => list_depth += 1,
            Event::End(TagEnd::List(_)) => list_depth = list_depth.saturating_sub(1),
            Event::Start(Tag::Item) => {
                let tag = match list_depth {
                    0 | 1 => "li1",
                    2 => "li2",
                    _ => "li3",
                };
                out.push(style(range.start, trim_nl(text, range.end), tag));
                let src = &text[range.start..range.end];
                let lead = src.len() - src.trim_start_matches([' ', '\t']).len();
                let mlen = item_marker_len(&src[lead..]);
                if mlen > 0 {
                    out.push(style(
                        range.start + lead,
                        range.start + lead + mlen,
                        "listmarker",
                    ));
                }
            }

            Event::Start(Tag::CodeBlock(kind)) => {
                let end = trim_nl(text, range.end);
                out.push(style(range.start, end, "codeblock"));
                if matches!(kind, CodeBlockKind::Fenced(_)) {
                    let lines = line_ranges(text, range.start, end);
                    for (ls, le) in [lines.first(), lines.last()].into_iter().flatten() {
                        let l = text[*ls..*le].trim_start();
                        if l.starts_with("```") || l.starts_with("~~~") {
                            out.push(style(*ls, *le, "fence"));
                        }
                    }
                }
            }

            Event::Start(Tag::Table(_)) => {
                // Monoespaciada en todo el bloque: si el usuario alinea los
                // pipes en el fuente, las columnas cuadran también en pantalla.
                let end = trim_nl(text, range.end);
                out.push(style(range.start, end, "table"));
                for (ls, le) in line_ranges(text, range.start, end) {
                    let line = text[ls..le].trim();
                    let is_delimiter = !line.is_empty()
                        && line.bytes().all(|b| matches!(b, b'|' | b'-' | b':' | b' '))
                        && line.contains('-');
                    if is_delimiter {
                        out.push(style(ls, le, "tabledelim"));
                        break;
                    }
                }
            }

            Event::Html(_) | Event::InlineHtml(_) => {
                out.push(style(range.start, trim_nl(text, range.end), "html"))
            }
            Event::FootnoteReference(_) => out.push(style(range.start, range.end, "footnote")),
            Event::HardBreak => out.push(syn(range.start, trim_nl(text, range.end))),

            Event::Rule => out.push(style(range.start, trim_nl(text, range.end), "rule")),
            Event::TaskListMarker(_) => out.push(style(range.start, range.end, "task")),

            _ => {}
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find<'a>(spans: &'a [Span], tag: &str) -> Vec<&'a Span> {
        spans.iter().filter(|s| s.tag == tag).collect()
    }

    fn hidden(text: &str) -> String {
        // Reconstruye lo que se vería con todas las marcas ocultas.
        let mut keep = vec![true; text.len()];
        for s in spans(text).iter().filter(|s| s.syntax) {
            for k in keep.iter_mut().take(s.end).skip(s.start) {
                *k = false;
            }
        }
        text.char_indices()
            .filter(|(i, _)| keep[*i])
            .map(|(_, c)| c)
            .collect()
    }

    #[test]
    fn cabecera_oculta_las_almohadillas() {
        let s = spans("## Título\n");
        assert_eq!(find(&s, "h2").len(), 1);
        assert_eq!(hidden("## Título\n"), "Título\n");
    }

    #[test]
    fn negrita_y_cursiva() {
        assert_eq!(hidden("un **texto** y *otro*\n"), "un texto y otro\n");
        let s = spans("un **texto**\n");
        assert_eq!(find(&s, "bold").len(), 1);
    }

    #[test]
    fn codigo_en_linea_con_varios_backticks() {
        assert_eq!(hidden("esto es ``a ` b`` fin"), "esto es a ` b fin");
    }

    #[test]
    fn enlace_oculta_la_url() {
        assert_eq!(hidden("ver [la web](https://ej.com) ya"), "ver la web ya");
    }

    #[test]
    fn imagen_oculta_marcas() {
        assert_eq!(hidden("![gato](/tmp/g.png)"), "gato");
    }

    #[test]
    fn cita_oculta_el_mayor_que() {
        assert_eq!(hidden("> una cita\n> y otra\n"), "una cita\ny otra\n");
        assert_eq!(find(&spans("> una cita\n"), "quote").len(), 1);
    }

    #[test]
    fn lista_conserva_el_marcador_pero_lo_estiliza() {
        let s = spans("- uno\n- dos\n");
        assert_eq!(find(&s, "listmarker").len(), 2);
        assert_eq!(find(&s, "li1").len(), 2);
        // El marcador se ve: no es un tramo de sintaxis.
        assert_eq!(hidden("- uno\n"), "- uno\n");
    }

    #[test]
    fn lista_anidada_usa_mas_sangria() {
        let s = spans("- uno\n    - dos\n");
        assert_eq!(find(&s, "li2").len(), 1);
    }

    #[test]
    fn bloque_de_codigo_marca_las_vallas() {
        let s = spans("```rust\nfn main() {}\n```\n");
        assert_eq!(find(&s, "codeblock").len(), 1);
        assert_eq!(find(&s, "fence").len(), 2);
    }

    #[test]
    fn el_markdown_dentro_de_codigo_no_se_decora() {
        let s = spans("```\nesto **no** es negrita\n```\n");
        assert!(find(&s, "bold").is_empty());
    }

    #[test]
    fn regla_y_tareas() {
        assert_eq!(find(&spans("---\n"), "rule").len(), 1);
        assert_eq!(find(&spans("- [x] hecho\n"), "task").len(), 1);
    }

    #[test]
    fn los_tramos_caen_en_fronteras_de_caracter() {
        let text = "# Ñandú **café** y `añejo`\n";
        for s in spans(text) {
            assert!(text.is_char_boundary(s.start), "{s:?}");
            assert!(text.is_char_boundary(s.end), "{s:?}");
        }
    }

    #[test]
    fn cabecera_setext_oculta_el_subrayado() {
        let s = spans("Título\n======\n");
        assert_eq!(find(&s, "h1").len(), 1);
        assert_eq!(hidden("Título\n======\n"), "Título\n");
    }

    #[test]
    fn enlace_automatico_oculta_los_angulos() {
        assert_eq!(hidden("ver <https://ej.com> ya"), "ver https://ej.com ya");
    }

    #[test]
    fn tabla_va_en_monoespaciada() {
        let s = spans("| a | b |\n|---|---|\n| 1 | 2 |\n");
        assert_eq!(find(&s, "table").len(), 1);
        assert_eq!(find(&s, "tabledelim").len(), 1);
    }

    #[test]
    fn texto_vacio_no_revienta() {
        assert!(spans("").is_empty());
    }
}
