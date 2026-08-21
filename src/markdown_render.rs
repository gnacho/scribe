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
use std::borrow::Cow;

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
    /// Caja de fondo de una tabla, de `first` a `last` inclusive.
    Table { first: usize, last: usize },
    /// Línea que separa la cabecera de una tabla del resto. Ocupa el hueco que
    /// deja la fila de guiones del fuente, que se oculta.
    TableRule { line: usize },
    /// Salto de línea dentro de una celda de tabla, producido por `<br>`.
    /// `offset` es el byte donde iba el `<br>` (ya oculto); el widget dibuja en
    /// su sitio un glifo de retorno. Fuera de tablas no se genera: ahí el
    /// `<br>` se deja con su tag `html` como hasta ahora.
    Break { offset: usize },
    /// Separador de columna de tabla (un `|` del fuente). El widget dibuja una
    /// línea vertical en su posición, para dotar a la tabla de rejilla visual
    /// sin necesidad de un widget compuesto.
    CellSeparator { offset: usize },
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
fn mark_delims(out: &mut Vec<Span>, start: usize, end: usize, len: usize, inside_table: bool) {
    if len == 0 || end < start + len * 2 {
        return;
    }
    // Dentro de una tabla ocultamos siempre los delimitadores (`**`, `_`,
    // backticks): el modo `focus` solo los oculta en la línea del cursor, así
    // que fuera de ella aparecerían los asteriscos literalmente encima del
    // texto en negrita. Marcándolos como Replaced se ocultan siempre.
    let mk = if inside_table { replaced } else { marker };
    out.push(mk(start, start + len));
    out.push(mk(end - len, end));
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

/// ¿Es la fila de guiones que separa la cabecera de una tabla?
fn is_delimiter_row(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty()
        && trimmed.contains('-')
        && trimmed
            .bytes()
            .all(|b| matches!(b, b'|' | b'-' | b':' | b' '))
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

/// Sustituye por espacios los tabuladores que van detras de una valla de codigo.
///
/// El spec de CommonMark dice que la valla de cierre «puede ir seguida solo de
/// espacios o tabuladores, que se ignoran», pero pulldown-cmark no cierra el
/// bloque si lo que sigue es un tabulador: se traga el resto del documento como
/// codigo. Se sanea solo en las lineas de valla y, como espacio y tabulador
/// ocupan un byte cada uno, todos los desplazamientos siguen siendo validos
/// sobre el texto original.
fn normalize_fences(text: &str) -> Cow<'_, str> {
    if !text.contains('\t') {
        return Cow::Borrowed(text);
    }

    let mut out: Option<Vec<u8>> = None;
    let bytes = text.as_bytes();
    let mut line_start = 0usize;

    while line_start <= bytes.len() {
        let line_end = match text[line_start..].find('\n') {
            Some(i) => line_start + i,
            None => bytes.len(),
        };
        let line = &text[line_start..line_end];

        // Hasta tres espacios de sangria, luego la valla.
        let indent = line.len() - line.trim_start_matches(' ').len();
        if indent <= 3 {
            let rest = &line[indent..];
            let marker = rest.as_bytes().first().copied();
            if matches!(marker, Some(b'`') | Some(b'~')) {
                let fence = rest.bytes().take_while(|b| Some(*b) == marker).count();
                let tail_start = line_start + indent + fence;
                if fence >= 3 && text[tail_start..line_end].contains('\t') {
                    let buffer = out.get_or_insert_with(|| bytes.to_vec());
                    for byte in &mut buffer[tail_start..line_end] {
                        if *byte == b'\t' {
                            *byte = b' ';
                        }
                    }
                }
            }
        }

        if line_end == bytes.len() {
            break;
        }
        line_start = line_end + 1;
    }

    match out {
        // Solo se han cambiado tabuladores por espacios, ambos ASCII.
        Some(buffer) => Cow::Owned(String::from_utf8(buffer).expect("solo ASCII sustituido")),
        None => Cow::Borrowed(text),
    }
}

pub fn analyze(text: &str) -> Analysis {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_FOOTNOTES);

    let normalized = normalize_fences(text);
    let text: &str = &normalized;

    let lines = LineIndex::new(text);
    let mut spans: Vec<Span> = Vec::new();
    let mut ornaments: Vec<Ornament> = Vec::new();
    let mut list_depth = 0usize;
    let mut table_depth = 0usize;

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
                mark_delims(&mut spans, range.start, range.end, 2, table_depth > 0);
            }
            Event::Start(Tag::Emphasis) => {
                spans.push(style(range.start, range.end, "italic"));
                mark_delims(&mut spans, range.start, range.end, 1, table_depth > 0);
            }
            Event::Start(Tag::Strikethrough) => {
                spans.push(style(range.start, range.end, "strike"));
                mark_delims(&mut spans, range.start, range.end, 2, table_depth > 0);
            }

            Event::Code(_) => {
                spans.push(style(range.start, range.end, "code"));
                let ticks = text[range.start..range.end]
                    .bytes()
                    .take_while(|b| *b == b'`')
                    .count();
                mark_delims(&mut spans, range.start, range.end, ticks, table_depth > 0);
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
            Event::End(TagEnd::Table) => table_depth = table_depth.saturating_sub(1),
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
                // La tabla ya no es monoespaciada (véase editor.rs): aquí solo
                // marcamos el bloque para el indent y la caja de fondo, y los
                // pipes se atenúan por separado. El inline (strong, em, code)
                // fluye dentro de las celdas.
                table_depth += 1;
                let end = trim_nl(text, range.end);
                spans.push(style(range.start, end, "table"));
                let first = lines.line_of(range.start);
                let last = lines.line_of(end.saturating_sub(1).max(range.start));
                ornaments.push(Ornament::Table { first, last });

                for (ls, le) in line_ranges(text, range.start, end) {
                    if is_delimiter_row(&text[ls..le]) {
                        // La fila de guiones se oculta y en su hueco se pinta
                        // la línea que separa la cabecera.
                        spans.push(replaced(ls, le));
                        spans.push(style(ls, le, "tablerule"));
                        ornaments.push(Ornament::TableRule {
                            line: lines.line_of(ls),
                        });
                        continue;
                    }
                    // Los pipes se quedan, pero atenuados: hacen de separador
                    // de columna sin competir con el contenido. Además emitimos
                    // un `CellSeparator` para que el widget dibuje una línea
                    // vertical y la tabla gane estructura visual.
                    let row = &text[ls..le];
                    let bytes = row.as_bytes();
                    for (i, b) in bytes.iter().enumerate() {
                        if *b == b'|' && (i == 0 || bytes[i - 1] != b'\\') {
                            spans.push(style(ls + i, ls + i + 1, "tablepipe"));
                            ornaments.push(Ornament::CellSeparator { offset: ls + i });
                        }
                    }
                }
            }

            Event::Html(_) | Event::InlineHtml(_) => {
                // Dentro de una tabla, `<br>` (la única forma estándar de meter
                // un salto en una celda) se oculta y se sustituye por un glifo
                // de retorno pintado por `MarkdownView`. Fuera de tablas lo
                // dejamos como cualquier otro HTML inline.
                if table_depth > 0 && is_br(&text[range.start..range.end]) {
                    spans.push(replaced(range.start, range.end));
                    ornaments.push(Ornament::Break {
                        offset: range.start,
                    });
                } else {
                    spans.push(style(range.start, trim_nl(text, range.end), "html"))
                }
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

// ---------------------------------------------------------------------------
// Alineado de tablas
// ---------------------------------------------------------------------------

/// ¿Es este fragmento un `<br>` de HTML? Acepta `<br>`, `<br/>`, `<br />` y
/// cualquier combinación de mayúsculas/minúsculas. No acepta atributos
/// (`<br class="x">`) porque no aparecen en markdown a mano.
fn is_br(content: &str) -> bool {
    let lower = content.to_ascii_lowercase();
    let Some(inner) = lower
        .strip_prefix('<')
        .and_then(|s| s.strip_suffix('>'))
    else {
        return false;
    };
    let inner = inner.trim_matches(|c: char| c == '/' || c.is_whitespace());
    inner == "br"
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Align {
    None,
    Left,
    Center,
    Right,
}

/// Parte una fila en celdas respetando los pipes escapados (`\|`).
fn split_row(row: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut escaped = false;
    for ch in row.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
        } else if ch == '\\' {
            current.push(ch);
            escaped = true;
        } else if ch == '|' {
            cells.push(current.trim().to_string());
            current = String::new();
        } else {
            current.push(ch);
        }
    }
    cells.push(current.trim().to_string());

    // Los pipes exteriores son opcionales: si están, dejan una celda vacía a
    // cada lado que no representa ninguna columna.
    if row.trim_start().starts_with('|') && !cells.is_empty() {
        cells.remove(0);
    }
    if row.trim_end().ends_with('|') && !cells.is_empty() {
        cells.pop();
    }
    cells
}

fn alignment_of(cell: &str) -> Align {
    let c = cell.trim();
    match (c.starts_with(':'), c.ends_with(':')) {
        (true, true) => Align::Center,
        (true, false) => Align::Left,
        (false, true) => Align::Right,
        _ => Align::None,
    }
}

fn pad(cell: &str, width: usize, align: Align) -> String {
    let len = cell.chars().count();
    let missing = width.saturating_sub(len);
    match align {
        Align::Right => format!("{}{cell}", " ".repeat(missing)),
        Align::Center => {
            let left = missing / 2;
            format!("{}{cell}{}", " ".repeat(left), " ".repeat(missing - left))
        }
        _ => format!("{cell}{}", " ".repeat(missing)),
    }
}

fn delimiter_cell(width: usize, align: Align) -> String {
    let w = width.max(3);
    match align {
        Align::None => "-".repeat(w),
        Align::Left => format!(":{}", "-".repeat(w - 1)),
        Align::Right => format!("{}:", "-".repeat(w - 1)),
        Align::Center => format!(":{}:", "-".repeat(w - 2)),
    }
}

/// Reformatea todas las tablas del documento para que sus columnas queden
/// alineadas en el fuente. Devuelve `None` si no hay nada que cambiar.
///
/// Las tablas dentro de un bloque de código se dejan intactas: en el fuente son
/// texto de ejemplo, no tablas.
pub fn format_tables(text: &str) -> Option<String> {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut i = 0;
    let mut in_fence = false;
    let mut changed = false;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            out.push(line.to_string());
            i += 1;
            continue;
        }

        let starts_table = !in_fence
            && line.contains('|')
            && i + 1 < lines.len()
            && is_delimiter_row(lines[i + 1])
            && split_row(lines[i + 1]).len() == split_row(line).len();

        if !starts_table {
            out.push(line.to_string());
            i += 1;
            continue;
        }

        // Cuerpo: todo lo que siga conteniendo un pipe.
        let mut end = i + 2;
        while end < lines.len() && lines[end].contains('|') && !lines[end].trim().is_empty() {
            end += 1;
        }

        let header = split_row(lines[i]);
        let aligns: Vec<Align> = split_row(lines[i + 1])
            .iter()
            .map(|c| alignment_of(c))
            .collect();
        let body: Vec<Vec<String>> = lines[i + 2..end].iter().map(|l| split_row(l)).collect();

        let columns = header.len();
        let mut widths = vec![3usize; columns];
        for (width, cell) in widths.iter_mut().zip(header.iter()) {
            *width = (*width).max(cell.chars().count());
        }
        for row in &body {
            for (width, cell) in widths.iter_mut().zip(row.iter()) {
                *width = (*width).max(cell.chars().count());
            }
        }

        let render = |cells: &[String]| -> String {
            let mut s = String::from("|");
            for (c, width) in widths.iter().enumerate() {
                let cell = cells.get(c).map(|s| s.as_str()).unwrap_or("");
                let align = aligns.get(c).copied().unwrap_or(Align::None);
                s.push(' ');
                s.push_str(&pad(cell, *width, align));
                s.push_str(" |");
            }
            s
        };

        let mut table = vec![render(&header)];
        let mut delim = String::from("|");
        for (c, width) in widths.iter().enumerate() {
            delim.push(' ');
            delim.push_str(&delimiter_cell(
                *width,
                aligns.get(c).copied().unwrap_or(Align::None),
            ));
            delim.push_str(" |");
        }
        table.push(delim);
        table.extend(body.iter().map(|row| render(row)));

        if table != lines[i..end] {
            changed = true;
        }
        out.extend(table);
        i = end;
    }

    changed.then(|| out.join("\n"))
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
    fn tabla_tiene_tramo_de_bloque() {
        let a = analyze("| a | b |\n|---|---|\n| 1 | 2 |\n");
        assert_eq!(find(&a.spans, "table").len(), 1);
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
                | Ornament::Rule { line }
                | Ornament::TableRule { line } => line,
                Ornament::Quote { last, .. }
                | Ornament::CodeBlock { last, .. }
                | Ornament::Table { last, .. } => last,
                // Break y CellSeparator van por offset, no por línea: se
                // comprueban aparte en sus tests propios.
                Ornament::Break { .. } | Ornament::CellSeparator { .. } => continue,
            };
            assert!(max < total, "{o:?} fuera de rango ({total} líneas)");
        }
    }

    #[test]
    fn la_fila_de_guiones_se_oculta_y_pide_una_linea() {
        let text = "| a | b |\n|---|---|\n| 1 | 2 |\n";
        let a = analyze(text);
        assert!(a.ornaments.contains(&Ornament::TableRule { line: 1 }));
        assert!(a.ornaments.contains(&Ornament::Table { first: 0, last: 2 }));
        assert_eq!(hidden(text), "| a | b |\n\n| 1 | 2 |\n");
    }

    #[test]
    fn los_pipes_se_atenuan() {
        let a = analyze("| a | b |\n|---|---|\n| 1 | 2 |\n");
        // Tres por fila de contenido, dos filas.
        assert_eq!(find(&a.spans, "tablepipe").len(), 6);
    }

    #[test]
    fn is_br_reconoce_las_variantes_comunes() {
        for ok in ["<br>", "<br/>", "<br />", "<BR>", "<Br/>", "<BR />"] {
            assert!(is_br(ok), "{ok:?} debería contar como <br>");
        }
        for ko in ["<br class=\"x\">", "<b>", "<brown>", "br", ""] {
            assert!(!is_br(ko), "{ko:?} NO debería contar como <br>");
        }
    }

    #[test]
    fn br_dentro_de_tabla_se_oculta_y_genera_adorno() {
        let text = "| a | b |\n|---|---|\n| x<br>y | z |\n";
        let a = analyze(text);
        // El `<br>` debe quedar como marca reemplazada (oculta), no como html.
        let brs = a
            .spans
            .iter()
            .filter(|s| s.kind == SpanKind::Replaced && text[s.start..s.end].contains("br"))
            .count();
        assert_eq!(brs, 1, "debe haber exactamente un <br> oculto");
        assert!(a.ornaments.iter().any(|o| matches!(o, Ornament::Break { .. })));
        // Y nada con tag `html` dentro de la tabla.
        assert_eq!(find(&a.spans, "html").len(), 0);
    }

    #[test]
    fn br_fuera_de_tabla_queda_como_html() {
        let text = "línea uno<br>línea dos\n";
        let a = analyze(text);
        assert_eq!(find(&a.spans, "html").len(), 1);
        assert!(!a.ornaments.iter().any(|o| matches!(o, Ornament::Break { .. })));
    }

    #[test]
    fn inline_dentro_de_celda_genera_tramos_propios() {
        // El cambio clave de esta iteración: el interior de las celdas ya no
        // va solo en el span `table`; el inline (strong, code) genera sus
        // propios tramos, que el editor aplicará encima.
        let text = "| a | b |\n|---|---|\n| **x** | `y` |\n";
        let a = analyze(text);
        assert!(!find(&a.spans, "bold").is_empty(), "bold dentro de celda");
        assert!(!find(&a.spans, "code").is_empty(), "code dentro de celda");
    }

    #[test]
    fn los_delimitadores_inline_de_tabla_se_ocultan_siempre() {
        // En las tablas los delimitadores (`**`, `*`, backticks) se marcan como
        // Replaced, de modo que se ocultan aunque el cursor esté en la línea:
        // el modo `focus` solo los revela en la línea del cursor, y encimarlos
        // sobre el texto en negrita descuadraría la celda.
        let text = "| **a** | `b` |\n|---|---|\n| *c* | d |\n";
        let a = analyze(text);
        let visible_markers = a
            .spans
            .iter()
            .filter(|s| s.kind == SpanKind::Marker)
            .count();
        assert_eq!(
            visible_markers, 0,
            "ninguna marca visible debe quedar dentro de la tabla: {a:?}"
        );
        // El texto visto por el usuario queda limpio: solo pipes, texto y el
        // hueco de la fila de guiones.
        assert_eq!(hidden(text), "| a | b |\n\n| c | d |\n");
    }

    #[test]
    fn cada_pipe_de_tabla_genera_un_separador() {
        // Cada `|` del fuente (en cabecera y cuerpo; la fila de guiones va
        // oculta y se salta) debe emitir un `CellSeparator` para que el widget
        // dibuje las líneas verticales.
        let text = "| a | b |\n|---|---|\n| 1 | 2 |\n";
        let a = analyze(text);
        let seps = a
            .ornaments
            .iter()
            .filter(|o| matches!(o, Ornament::CellSeparator { .. }))
            .count();
        // 2 filas (cabecera + 1 body) × 3 pipes por fila = 6.
        assert_eq!(seps, 6, "una tubería por pipe de cabecera/cuerpo");
        // y debe coincidir con el número de spans `tablepipe`.
        assert_eq!(seps, find(&a.spans, "tablepipe").len());
    }

    #[test]
    fn alinear_una_tabla_desalineada() {
        let entrada = "Markdown | Less | Pretty\n--- | --- | ---\n*Still* | `renders` | **nicely**\n1 | 2 | 3\n";
        let esperado = [
            "| Markdown | Less      | Pretty     |",
            "| -------- | --------- | ---------- |",
            "| *Still*  | `renders` | **nicely** |",
            "| 1        | 2         | 3          |",
            "",
        ]
        .join("\n");
        assert_eq!(format_tables(entrada).unwrap(), esperado);
    }

    #[test]
    fn alinear_respeta_los_dos_puntos_de_alineacion() {
        let entrada = "| a | b | c |\n| :--- | :---: | ---: |\n| 1 | 2 | 3 |\n";
        let salida = format_tables(entrada).unwrap();
        // Ancho mínimo de columna: tres guiones.
        assert!(salida.contains("| :-- | :-: | --: |"), "{salida}");
        // La celda derecha se rellena por la izquierda.
        assert!(salida.contains("|   3 |"), "{salida}");
    }

    #[test]
    fn alinear_no_toca_las_tablas_dentro_de_un_bloque_de_codigo() {
        let entrada = "```\na | b\n--- | ---\n1 | 2\n```\n";
        assert_eq!(format_tables(entrada), None);
    }

    #[test]
    fn alinear_devuelve_none_si_ya_esta_alineada() {
        let entrada = "| a   | b   |\n| --- | --- |\n| 1   | 2   |\n";
        assert_eq!(format_tables(entrada), None);
    }

    #[test]
    fn alinear_respeta_los_pipes_escapados() {
        let entrada = "| Name | Character |\n| --- | --- |\n| Pipe | \\| |\n";
        let salida = format_tables(entrada).unwrap();
        assert!(salida.contains("\\|"));
        assert_eq!(salida.lines().count(), 3);
    }

    #[test]
    fn una_valla_seguida_de_tabulador_cierra_el_bloque() {
        // El spec de CommonMark lo permite; pulldown-cmark, no. Sin esto el
        // bloque se traga el resto del documento.
        let text = "```\ncodigo\n```\t\n\ntexto normal\n";
        let a = analyze(text);
        assert!(a
            .ornaments
            .contains(&Ornament::CodeBlock { first: 0, last: 2 }));
        assert!(find(&a.spans, "codeblock").len() == 1);
    }

    #[test]
    fn normalizar_vallas_conserva_las_posiciones() {
        let text = "```rust\tx\ncodigo\ttabulado\n```\t\n";
        let normalizado = normalize_fences(text);
        assert_eq!(normalizado.len(), text.len());
        // El tabulador de dentro del bloque no se toca.
        assert!(normalizado.contains("codigo\ttabulado"));
        assert!(!normalizado.lines().next_back().unwrap().contains('\t'));
    }

    #[test]
    fn sin_tabuladores_no_se_copia_el_texto() {
        assert!(matches!(
            normalize_fences("```\nx\n```\n"),
            Cow::Borrowed(_)
        ));
    }

    #[test]
    fn texto_vacio_no_revienta() {
        let a = analyze("");
        assert!(a.spans.is_empty() && a.ornaments.is_empty());
    }
}

#[cfg(test)]
mod probe4 {
    use super::*;
    #[test]
    fn probe_tab13() {
        let a = analyze("```\ncodigo\n```\t\n\ntexto\n");
        let b: Vec<_> = a
            .ornaments
            .iter()
            .filter(|o| matches!(o, Ornament::CodeBlock { .. }))
            .collect();
        println!("CON 0.13 tabulador: {b:?}");
    }
}
