//! Analiza Markdown y decide cómo debe *verse* mientras se edita.
//!
//! Devuelve dos cosas y ninguna toca GTK, así que todo esto se prueba sin
//! display y se puede reutilizar fuera de Scribe:
//!
//! - **Tramos** ([`Span`]): rangos en bytes con el nombre del `GtkTextTag` que
//!   les corresponde. Los tramos de tipo [`SpanKind::Marker`] son las marcas del
//!   Markdown (`**`, `#`, backticks, la URL de un enlace) y se ocultan según la
//!   preferencia del usuario.
//! - **Adornos** ([`Ornament`]): elementos que no se pueden expresar con un tag
//!   porque hay que *dibujarlos* — viñetas, casillas, reglas, la barra de las
//!   citas y la caja de los bloques de código. Van referidos a número de línea
//!   para que el widget solo tenga que preguntar por su geometría.
//!
//! Las marcas que un adorno sustituye se marcan como [`SpanKind::Replaced`]:
//! se ocultan siempre (salvo en modo «atenuar»), porque revelarlas movería el
//! texto de sitio cada vez que el cursor cambia de línea.

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, LinkType, Options, Parser, Tag, TagEnd};

/// Por encima de este tamaño se deja de decorar en vivo, para no bloquear la UI.
pub const MAX_LIVE_BYTES: usize = 400_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanKind {
    /// Estilo permanente: negrita, cabecera, color…
    Style,
    /// Marca de Markdown. Su visibilidad la decide el usuario.
    Marker,
    /// Marca sustituida por un adorno dibujado. Se oculta salvo en modo «atenuar».
    Replaced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub tag: &'static str,
    pub kind: SpanKind,
}

impl Span {
    /// Parte de la superficie pública del módulo: la usa quien reutilice el
    /// analizador sin replicar la distinción entre marca y sustitución.
    #[allow(dead_code)]
    pub fn is_syntax(&self) -> bool {
        matches!(self.kind, SpanKind::Marker | SpanKind::Replaced)
    }
}

/// Elemento que hay que pintar a mano. Las líneas son lógicas y empiezan en 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ornament {
    /// Viñeta de lista sin ordenar. `depth` empieza en 1.
    Bullet { line: usize, depth: usize },
    /// Casilla de tarea.
    Checkbox { line: usize, checked: bool },
    /// Regla horizontal (`---`), en una línea que queda vacía a propósito.
    Rule { line: usize },
    /// Barra vertical de cita, de `first` a `last` inclusive.
    Quote { first: usize, last: usize },
    /// Caja de fondo de un bloque de código, de `first` a `last` inclusive.
    CodeBlock { first: usize, last: usize },
}

#[derive(Debug, Default)]
pub struct Analysis {
    pub spans: Vec<Span>,
    pub ornaments: Vec<Ornament>,
}

fn style(start: usize, end: usize, tag: &'static str) -> Span {
    Span {
        start,
        end,
        tag,
        kind: SpanKind::Style,
    }
}

fn marker(start: usize, end: usize) -> Span {
    Span {
        start,
        end,
        tag: "syntax",
        kind: SpanKind::Marker,
    }
}

fn replaced(start: usize, end: usize) -> Span {
    Span {
        start,
        end,
        tag: "syntax",
        kind: SpanKind::Replaced,
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

fn list_tag(depth: usize) -> &'static str {
    match depth {
        0 | 1 => "li1",
        2 => "li2",
        _ => "li3",
    }
}

/// Índice de comienzos de línea, para traducir desplazamientos a números de línea.
struct LineIndex {
    starts: Vec<usize>,
}

impl LineIndex {
    fn new(text: &str) -> Self {
        let mut starts = vec![0];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                starts.push(i + 1);
            }
        }
        Self { starts }
    }

    fn line_of(&self, offset: usize) -> usize {
        match self.starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        }
    }
}

fn trim_nl(text: &str, mut end: usize) -> usize {
    while end > 0 && matches!(text.as_bytes()[end - 1], b'\n' | b'\r') {
        end -= 1;
    }
    end
}

/// Marca los delimitadores de un elemento en línea (`**`, `_`, `~~`, backticks).
fn mark_delims(out: &mut Vec<Span>, start: usize, end: usize, len: usize) {
    if len == 0 || end < start + len * 2 {
        return;
    }
    out.push(marker(start, start + len));
    out.push(marker(end - len, end));
}

fn line_ranges(text: &str, start: usize, end: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut cursor = start;
    while cursor < end {
        let stop = match text[cursor..end].find('\n') {
            Some(i) => cursor + i,
            None => end,
        };
        out.push((cursor, stop));
        cursor = stop + 1;
    }
    out
}

/// Longitud de la marca `>` (con su espacio) al principio de una línea de cita.
fn quote_marker_len(line: &str) -> usize {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() && i < 3 && bytes[i] == b' ' {
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

/// Marcador de un elemento de lista: longitud y si la lista es sin ordenar.
fn item_marker(rest: &str) -> Option<(usize, bool)> {
    let bytes = rest.as_bytes();
    let first = *bytes.first()?;
    let mut i;
    let unordered = matches!(first, b'-' | b'*' | b'+');
    if unordered {
        i = 1;
    } else {
        i = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
        if i == 0 || i >= bytes.len() || !matches!(bytes[i], b'.' | b')') {
            return None;
        }
        i += 1;
    }
    let spaces = bytes[i..].iter().take_while(|b| **b == b' ').count();
    if spaces == 0 {
        return None;
    }
    Some((i + spaces, unordered))
}

pub fn analyze(text: &str) -> Analysis {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_FOOTNOTES);

    let lines = LineIndex::new(text);
    let mut spans: Vec<Span> = Vec::new();
    let mut ornaments: Vec<Ornament> = Vec::new();
    let mut list_depth = 0usize;

    for (event, range) in Parser::new_ext(text, opts).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                let end = trim_nl(text, range.end);
                if end <= range.start {
                    continue;
                }
                spans.push(style(range.start, end, heading_tag(level)));
                let src = &text[range.start..end];
                let hashes = src.bytes().take_while(|b| *b == b'#').count();
                if hashes > 0 {
                    let gap = src[hashes..].bytes().take_while(|b| *b == b' ').count();
                    spans.push(marker(range.start, range.start + hashes + gap));
                } else {
                    // Cabecera setext: se oculta el subrayado `===` / `---` con
                    // su salto de línea, para no dejar un renglón en blanco.
                    if let Some(&(ls, le)) = line_ranges(text, range.start, end).last() {
                        let underline = text[ls..le].trim();
                        let is_rule = !underline.is_empty()
                            && (underline.bytes().all(|b| b == b'=')
                                || underline.bytes().all(|b| b == b'-'));
                        if is_rule && ls > range.start {
                            spans.push(replaced(ls - 1, le));
                        }
                    }
                }
            }

            Event::Start(Tag::Strong) => {
                spans.push(style(range.start, range.end, "bold"));
                mark_delims(&mut spans, range.start, range.end, 2);
            }
            Event::Start(Tag::Emphasis) => {
                spans.push(style(range.start, range.end, "italic"));
                mark_delims(&mut spans, range.start, range.end, 1);
            }
            Event::Start(Tag::Strikethrough) => {
                spans.push(style(range.start, range.end, "strike"));
                mark_delims(&mut spans, range.start, range.end, 2);
            }

            Event::Code(_) => {
                spans.push(style(range.start, range.end, "code"));
                let ticks = text[range.start..range.end]
                    .bytes()
                    .take_while(|b| *b == b'`')
                    .count();
                mark_delims(&mut spans, range.start, range.end, ticks);
            }

            Event::Start(Tag::Link { link_type, .. }) => {
                spans.push(style(range.start, range.end, "link"));
                let src = &text[range.start..range.end];
                match link_type {
                    LinkType::Autolink | LinkType::Email => {
                        if src.starts_with('<') && src.ends_with('>') && src.len() > 2 {
                            spans.push(marker(range.start, range.start + 1));
                            spans.push(marker(range.end - 1, range.end));
                        }
                    }
                    _ => {
                        if src.starts_with('[') {
                            if let Some(idx) = src.rfind("](") {
                                spans.push(marker(range.start, range.start + 1));
                                spans.push(marker(range.start + idx, range.end));
                            }
                        }
                    }
                }
            }
            Event::Start(Tag::Image { .. }) => {
                spans.push(style(range.start, range.end, "link"));
                let src = &text[range.start..range.end];
                if src.starts_with("![") {
                    if let Some(idx) = src.rfind("](") {
                        spans.push(marker(range.start, range.start + 2));
                        spans.push(marker(range.start + idx, range.end));
                    }
                }
            }

            Event::Start(Tag::BlockQuote(_)) => {
                let end = trim_nl(text, range.end);
                spans.push(style(range.start, end, "quote"));
                // Los `>` se ocultan siempre: la cita se marca con una barra
                // dibujada, y revelarlos desplazaría el texto al pasar el cursor.
                for (ls, le) in line_ranges(text, range.start, end) {
                    let n = quote_marker_len(&text[ls..le]);
                    if n > 0 {
                        spans.push(replaced(ls, ls + n));
                    }
                }
                ornaments.push(Ornament::Quote {
                    first: lines.line_of(range.start),
                    last: lines.line_of(end.saturating_sub(1).max(range.start)),
                });
            }

            Event::Start(Tag::List(_)) => list_depth += 1,
            Event::End(TagEnd::List(_)) => list_depth = list_depth.saturating_sub(1),
            Event::Start(Tag::Item) => {
                // En las listas anidadas pulldown empieza el rango en el
                // marcador, no en la línea. Si el tag no cubre el principio del
                // párrafo, GTK aplica el margen del nivel de arriba y la viñeta
                // se queda alineada con la lista padre.
                let line_start = text[..range.start].rfind('\n').map(|i| i + 1).unwrap_or(0);
                spans.push(style(
                    line_start,
                    trim_nl(text, range.end),
                    list_tag(list_depth),
                ));
                let src = &text[range.start..range.end];
                let lead = src.len() - src.trim_start_matches([' ', '\t']).len();
                if let Some((mlen, unordered)) = item_marker(&src[lead..]) {
                    let marker_start = range.start + lead;
                    let marker_end = marker_start + mlen;
                    if unordered {
                        // La viñeta se dibuja, así que sobran el guion y toda la
                        // sangría literal: el desplazamiento lo da el margen del
                        // tag y dejarla la sumaría dos veces.
                        spans.push(replaced(line_start, marker_end));
                        ornaments.push(Ornament::Bullet {
                            line: lines.line_of(marker_start),
                            depth: list_depth.max(1),
                        });
                    } else {
                        // En las ordenadas el número lleva información: se deja.
                        if line_start < marker_start {
                            spans.push(replaced(line_start, marker_start));
                        }
                        spans.push(style(marker_start, marker_end, "listmarker"));
                    }
                }
            }

            Event::TaskListMarker(checked) => {
                // El rango de pulldown cubre `[x]` pero no el espacio que sigue;
                // sin él quedaría una sangría suelta delante del texto.
                let mut end = range.end;
                while text.as_bytes().get(end) == Some(&b' ') {
                    end += 1;
                }
                spans.push(replaced(range.start, end));
                ornaments.push(Ornament::Checkbox {
                    line: lines.line_of(range.start),
                    checked,
                });
            }

            Event::Start(Tag::CodeBlock(kind)) => {
                let end = trim_nl(text, range.end);
                spans.push(style(range.start, end, "codeblock"));
                let first = lines.line_of(range.start);
                let last = lines.line_of(end.saturating_sub(1).max(range.start));
                ornaments.push(Ornament::CodeBlock { first, last });

                if matches!(kind, CodeBlockKind::Fenced(_)) {
                    let block_lines = line_ranges(text, range.start, end);
                    // Valla de apertura: se ocultan las comillas y se deja el
                    // nombre del lenguaje como etiqueta pequeña.
                    if let Some(&(ls, le)) = block_lines.first() {
                        let line = &text[ls..le];
                        let fence = line
                            .bytes()
                            .take_while(|b| matches!(b, b'`' | b'~'))
                            .count();
                        if fence >= 3 {
                            spans.push(replaced(ls, ls + fence));
                            spans.push(style(ls + fence, le, "fence"));
                        }
                    }
                    // Valla de cierre: la línea entera se oculta y el hueco que
                    // deja sirve de margen inferior dentro de la caja.
                    if block_lines.len() > 1 {
                        if let Some(&(ls, le)) = block_lines.last() {
                            let line = text[ls..le].trim_start();
                            if line.starts_with("```") || line.starts_with("~~~") {
                                spans.push(replaced(ls, le));
                            }
                        }
                    }
                }
            }

            Event::Start(Tag::Table(_)) => {
                // Monoespaciada en todo el bloque: si el usuario alinea los
                // pipes en el fuente, las columnas cuadran en pantalla.
                let end = trim_nl(text, range.end);
                spans.push(style(range.start, end, "table"));
                for (ls, le) in line_ranges(text, range.start, end) {
                    let line = text[ls..le].trim();
                    let is_delimiter = !line.is_empty()
                        && line.bytes().all(|b| matches!(b, b'|' | b'-' | b':' | b' '))
                        && line.contains('-');
                    if is_delimiter {
                        spans.push(style(ls, le, "tabledelim"));
                        break;
                    }
                }
            }

            Event::Html(_) | Event::InlineHtml(_) => {
                spans.push(style(range.start, trim_nl(text, range.end), "html"))
            }
            Event::FootnoteReference(_) => {
                // Queda solo el número, en volado: `[^` y `]` sobran.
                spans.push(style(range.start, range.end, "footnote"));
                let src = &text[range.start..range.end];
                if src.starts_with("[^") && src.ends_with(']') && src.len() > 3 {
                    spans.push(marker(range.start, range.start + 2));
                    spans.push(marker(range.end - 1, range.end));
                }
            }
            Event::Start(Tag::FootnoteDefinition(_)) => {
                spans.push(style(range.start, trim_nl(text, range.end), "footnotedef"))
            }
            Event::HardBreak => spans.push(marker(range.start, trim_nl(text, range.end))),

            Event::Rule => {
                // La línea queda vacía y la regla se pinta encima del hueco.
                let end = trim_nl(text, range.end);
                spans.push(replaced(range.start, end));
                ornaments.push(Ornament::Rule {
                    line: lines.line_of(range.start),
                });
            }

            _ => {}
        }
    }

    // Un elemento de tarea genera viñeta y casilla; la casilla manda.
    let checkbox_lines: Vec<usize> = ornaments
        .iter()
        .filter_map(|o| match o {
            Ornament::Checkbox { line, .. } => Some(*line),
            _ => None,
        })
        .collect();
    ornaments.retain(|o| match o {
        Ornament::Bullet { line, .. } => !checkbox_lines.contains(line),
        _ => true,
    });

    Analysis { spans, ornaments }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans(text: &str) -> Vec<Span> {
        analyze(text).spans
    }

    fn find<'a>(spans: &'a [Span], tag: &str) -> Vec<&'a Span> {
        spans.iter().filter(|s| s.tag == tag).collect()
    }

    /// Reconstruye lo que se vería con todas las marcas ocultas.
    fn hidden(text: &str) -> String {
        let mut keep = vec![true; text.len()];
        for s in spans(text).iter().filter(|s| s.is_syntax()) {
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
        assert_eq!(find(&spans("## Título\n"), "h2").len(), 1);
        assert_eq!(hidden("## Título\n"), "Título\n");
    }

    #[test]
    fn cabecera_setext_oculta_el_subrayado() {
        assert_eq!(find(&spans("Título\n======\n"), "h1").len(), 1);
        assert_eq!(hidden("Título\n======\n"), "Título\n");
    }

    #[test]
    fn negrita_y_cursiva() {
        assert_eq!(hidden("un **texto** y *otro*\n"), "un texto y otro\n");
        assert_eq!(find(&spans("un **texto**\n"), "bold").len(), 1);
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
    fn enlace_automatico_oculta_los_angulos() {
        assert_eq!(hidden("ver <https://ej.com> ya"), "ver https://ej.com ya");
    }

    #[test]
    fn imagen_oculta_marcas() {
        assert_eq!(hidden("![gato](/tmp/g.png)"), "gato");
    }

    #[test]
    fn cita_oculta_el_mayor_que_y_pide_barra() {
        assert_eq!(hidden("> una cita\n> y otra\n"), "una cita\ny otra\n");
        let a = analyze("> una cita\n> y otra\n");
        assert_eq!(a.ornaments, vec![Ornament::Quote { first: 0, last: 1 }]);
    }

    #[test]
    fn lista_sin_ordenar_cambia_el_guion_por_una_vineta() {
        let a = analyze("- uno\n- dos\n");
        assert_eq!(hidden("- uno\n- dos\n"), "uno\ndos\n");
        assert_eq!(
            a.ornaments,
            vec![
                Ornament::Bullet { line: 0, depth: 1 },
                Ornament::Bullet { line: 1, depth: 1 },
            ]
        );
    }

    #[test]
    fn la_sangria_literal_de_una_lista_anidada_se_oculta() {
        // La sangría la pone el margen del tag; el texto no debe llevarla dos veces.
        assert_eq!(hidden("- uno\n    - dos\n"), "uno\ndos\n");
    }

    #[test]
    fn lista_ordenada_conserva_el_numero() {
        let a = analyze("1. uno\n2. dos\n");
        assert_eq!(hidden("1. uno\n2. dos\n"), "1. uno\n2. dos\n");
        assert_eq!(find(&a.spans, "listmarker").len(), 2);
        assert!(a.ornaments.is_empty());
    }

    #[test]
    fn lista_anidada_usa_mas_sangria_y_otra_vineta() {
        let a = analyze("- uno\n    - dos\n");
        assert_eq!(find(&a.spans, "li2").len(), 1);
        assert!(a
            .ornaments
            .contains(&Ornament::Bullet { line: 1, depth: 2 }));
    }

    #[test]
    fn tareas_producen_casillas() {
        let a = analyze("- [x] hecho\n- [ ] pendiente\n");
        assert_eq!(
            hidden("- [x] hecho\n- [ ] pendiente\n"),
            "hecho\npendiente\n"
        );
        assert!(a.ornaments.contains(&Ornament::Checkbox {
            line: 0,
            checked: true
        }));
        assert!(a.ornaments.contains(&Ornament::Checkbox {
            line: 1,
            checked: false
        }));
    }

    #[test]
    fn una_tarea_no_lleva_ademas_vineta() {
        let a = analyze("- [x] hecho\n");
        assert!(!a
            .ornaments
            .iter()
            .any(|o| matches!(o, Ornament::Bullet { .. })));
    }

    #[test]
    fn bloque_de_codigo_pide_caja_y_oculta_las_vallas() {
        let a = analyze("```rust\nfn main() {}\n```\n");
        assert_eq!(find(&a.spans, "codeblock").len(), 1);
        assert!(a
            .ornaments
            .contains(&Ornament::CodeBlock { first: 0, last: 2 }));
        // Queda el nombre del lenguaje y el hueco de la valla de cierre.
        assert_eq!(
            hidden("```rust\nfn main() {}\n```\n"),
            "rust\nfn main() {}\n\n"
        );
    }

    #[test]
    fn el_markdown_dentro_de_codigo_no_se_decora() {
        assert!(find(&spans("```\nesto **no** es negrita\n```\n"), "bold").is_empty());
    }

    #[test]
    fn la_regla_deja_la_linea_vacia_para_dibujarla() {
        let a = analyze("uno\n\n---\n\ndos\n");
        assert!(a.ornaments.contains(&Ornament::Rule { line: 2 }));
        assert_eq!(hidden("uno\n\n---\n\ndos\n"), "uno\n\n\n\ndos\n");
    }

    #[test]
    fn las_notas_al_pie_dejan_solo_el_numero() {
        let text = "Texto[^1].\n\n[^1]: La nota.\n";
        assert_eq!(hidden(text), "Texto1.\n\n[^1]: La nota.\n");
        assert_eq!(find(&spans(text), "footnotedef").len(), 1);
    }

    #[test]
    fn tabla_va_en_monoespaciada() {
        let a = analyze("| a | b |\n|---|---|\n| 1 | 2 |\n");
        assert_eq!(find(&a.spans, "table").len(), 1);
        assert_eq!(find(&a.spans, "tabledelim").len(), 1);
    }

    #[test]
    fn los_tramos_caen_en_fronteras_de_caracter() {
        let text = "# Ñandú **café** y `añejo`\n- ítem\n> cita\n";
        for s in spans(text) {
            assert!(text.is_char_boundary(s.start), "{s:?}");
            assert!(text.is_char_boundary(s.end), "{s:?}");
        }
    }

    #[test]
    fn los_adornos_apuntan_a_lineas_existentes() {
        let text = "# T\n\n- uno\n- [x] dos\n\n> cita\n\n```\nx\n```\n\n---\n";
        let total = text.lines().count();
        for o in analyze(text).ornaments {
            let max = match o {
                Ornament::Bullet { line, .. }
                | Ornament::Checkbox { line, .. }
                | Ornament::Rule { line } => line,
                Ornament::Quote { last, .. } | Ornament::CodeBlock { last, .. } => last,
            };
            assert!(max < total, "{o:?} fuera de rango ({total} líneas)");
        }
    }

    #[test]
    fn texto_vacio_no_revienta() {
        let a = analyze("");
        assert!(a.spans.is_empty() && a.ornaments.is_empty());
    }
}
