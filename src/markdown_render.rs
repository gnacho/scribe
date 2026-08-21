//! Analiza Markdown y decide cómo debe *verse* mientras se edita.
//!
//! Devuelve dos cosas y ninguna toca GTK, así que todo esto se prueba sin
//! display y se puede reutilizar fuera de Scribe:
//!
//! - **Tramos** ([`Span`]): rangos en bytes con el nombre del `GtkTextTag` que
//!   les corresponde. Los tramos de tipo [`SpanKind::Marker`] son las marcas del
//!   Markdown (`**`, `#`, backticks, la URL de un enlace); su visibilidad la
//!   decide el editor según la preferencia del usuario y la salud del GTK del
//!   sistema (véase `settings::gtk_hides_invisible_safely`).
//! - **Adornos** ([`Ornament`]): elementos que no se pueden expresar con un tag
//!   porque hay que *dibujarlos* — viñetas, casillas, reglas, la barra de las
//!   citas y la caja de los bloques de código. Van referidos a número de línea
//!   para que el widget solo tenga que preguntar por su geometría.
//!
//! Las marcas que un adorno sustituye se marcan como [`SpanKind::Replaced`]:
//! no deben revelarse nunca al pasar el cursor, porque hacerlo movería el
//! texto de sitio en cada línea. Con la mitigación de GNOME/gtk#8346 activa
//! no se ocultan: en los modos WYSIWYG se encogen (tag `syn_shrink`) y en
//! «Atenuar» se atenúan como cualquier otra marca.

use crate::settings::MarkupVisibility;
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, LinkType, Options, Parser, Tag, TagEnd};
use std::borrow::Cow;

/// Por encima de este tamaño se deja de decorar en vivo, para no bloquear la UI.
pub const MAX_LIVE_BYTES: usize = 400_000;

/// Alto (px) del hueco que el tag `imagegap` reserva bajo la línea de una
/// imagen en bloque; ahí el widget pinta la textura. Única fuente: lo usan el
/// tag (editor.rs) y el pintado (markdown_view.rs).
pub const IMAGE_GAP_HEIGHT: i32 = 150;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanKind {
    /// Estilo permanente: negrita, cabecera, color…
    Style,
    /// Marca de Markdown. Su visibilidad la decide el usuario (y, de momento,
    /// la mitigación de GNOME/gtk#8346: véase `settings::gtk_hides_invisible_safely`).
    Marker,
    /// Marca sustituida por un adorno dibujado. No se revela con el cursor;
    /// se encoge, se oculta o se atenúa según la preferencia del usuario y
    /// `settings::gtk_hides_invisible_safely` (véase `decorate` en editor.rs).
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
///
/// No es `Copy`: algunos adornos llevan texto (la ruta de una imagen).
#[derive(Debug, Clone, PartialEq)]
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
    /// Línea que separa la cabecera de una tabla del resto. Ocupa el hueco
    /// que deja la fila de guiones del fuente, que se encoge.
    TableRule { line: usize },
    /// Salto de línea dentro de una celda de tabla, producido por `<br>`.
    /// `offset` es el byte donde iba el `<br>` (ya encogido); el widget dibuja
    /// en su sitio un glifo de retorno. Fuera de tablas no se genera: ahí el
    /// `<br>` se deja con su tag `html` como hasta ahora.
    Break { offset: usize },
    /// Separador de columna de tabla (un `|` del fuente). El widget dibuja una
    /// línea vertical en su posición, para dotar a la tabla de rejilla visual
    /// sin necesidad de un widget compuesto.
    CellSeparator { offset: usize },
    /// Filete fino bajo un título H1/H2, estilo GitHub. Aditivo: no sustituye
    /// ninguna marca.
    HeadingRule { line: usize },
    /// Imagen local que es el único contenido de su línea: se pinta en el
    /// hueco que reserva el span `imagegap` bajo esa línea. Aditiva: la marca
    /// `![alt](src)` sigue visible y editable en todos los modos.
    Image {
        line: usize,
        dest: String,
        alt: String,
    },
}

/// Familia de un adorno, según cómo convive con el texto del buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrnamentFamily {
    /// Se pinta siempre: convive con el texto sin taparlo (cajas, barras).
    Ambient,
    /// Sustituye a una marca del fuente (`- `, `[ ]`, `---`, pipes…). Solo se
    /// pinta cuando la marca está oculta o encogida; si la marca se ve, el
    /// adorno la duplicaría.
    Substitute,
    /// No sustituye nada (filete de título, imagen): se pinta en todos los
    /// modos sin riesgo de duplicar.
    Additive,
}

impl Ornament {
    /// Familia a la que pertenece el adorno; decide en qué modos se pinta.
    pub fn family(&self) -> OrnamentFamily {
        match self {
            Ornament::CodeBlock { .. } | Ornament::Table { .. } | Ornament::Quote { .. } => {
                OrnamentFamily::Ambient
            }
            Ornament::Bullet { .. }
            | Ornament::Checkbox { .. }
            | Ornament::Rule { .. }
            | Ornament::TableRule { .. }
            | Ornament::Break { .. }
            | Ornament::CellSeparator { .. } => OrnamentFamily::Substitute,
            Ornament::HeadingRule { .. } | Ornament::Image { .. } => OrnamentFamily::Additive,
        }
    }
}

/// Devuelve los adornos a pintar según la visibilidad elegida por el usuario.
/// `Dim` = vista cruda atenuada: solo ambientales y aditivos. `Hidden`/`Focus`
/// = WYSIWYG (hoy con marcas encogidas, no ocultas, por GNOME/gtk#8346): todos.
pub fn ornaments_for(ornaments: &[Ornament], vis: MarkupVisibility) -> Vec<Ornament> {
    ornaments
        .iter()
        .filter(|o| match o.family() {
            OrnamentFamily::Ambient | OrnamentFamily::Additive => true,
            OrnamentFamily::Substitute => vis != MarkupVisibility::Dim,
        })
        .cloned()
        .collect()
}

/// Imagen detectada en el fuente. Los offsets son en bytes; `line` es la
/// línea lógica (empieza en 0) que la contiene.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageRef {
    pub line: usize,
    pub start: usize,
    pub end: usize,
    pub dest: String,
    pub alt: String,
}

#[derive(Debug, Default)]
pub struct Analysis {
    pub spans: Vec<Span>,
    pub ornaments: Vec<Ornament>,
    /// Imágenes en bloque del documento. El pintado las recibe vía
    /// `Ornament::Image`; este listado es superficie pública para consumidores
    /// que quieran los metadatos sin reinterpretar ornamentos.
    #[allow(dead_code)]
    pub images: Vec<ImageRef>,
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
    // Dentro de una tabla los delimitadores (`**`, `_`, backticks) se marcan
    // como Replaced: el modo `focus` solo los trata distinto en la línea del
    // cursor, así que fuera de ella aparecerían los asteriscos literalmente
    // encima del texto en negrita. Su visibilidad no depende del cursor.
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

/// Segmentos de texto (offsets relativos a la fila, ya sin espacios a los
/// lados) de cada celda. Los pipes — incluidos los exteriores — quedan fuera;
/// los escapados (`\|`) no cortan.
fn cell_text_ranges(row: &str) -> Vec<(usize, usize)> {
    let bytes = row.as_bytes();
    let mut cuts: Vec<usize> = Vec::new();
    for (i, b) in bytes.iter().enumerate() {
        if *b == b'|' && (i == 0 || bytes[i - 1] != b'\\') {
            cuts.push(i);
        }
    }
    cuts.push(row.len());
    let mut out = Vec::new();
    let mut prev = 0;
    for cut in cuts {
        let seg = &row[prev..cut];
        let trimmed = seg.trim();
        if !trimmed.is_empty() {
            let lead = seg.len() - seg.trim_start().len();
            out.push((prev + lead, prev + lead + trimmed.len()));
        }
        prev = cut + 1;
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
    let mut images: Vec<ImageRef> = Vec::new();
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
                // Filete fino bajo los títulos H1/H2, estilo GitHub.
                if matches!(level, HeadingLevel::H1 | HeadingLevel::H2) {
                    ornaments.push(Ornament::HeadingRule {
                        line: lines.line_of(range.start),
                    });
                }
                let src = &text[range.start..end];
                let hashes = src.bytes().take_while(|b| *b == b'#').count();
                if hashes > 0 {
                    let gap = src[hashes..].bytes().take_while(|b| *b == b' ').count();
                    // Las almohadillas (y el espacio que las sigue) son marca
                    // sustituida: en WYSIWYG se encogen y el título queda
                    // limpio; en «Atenuar» siguen visibles, atenuadas.
                    spans.push(replaced(range.start, range.start + hashes + gap));
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
            Event::Start(Tag::Image { dest_url, .. }) => {
                spans.push(style(range.start, range.end, "link"));
                let src = &text[range.start..range.end];
                if src.starts_with("![") {
                    if let Some(idx) = src.rfind("](") {
                        spans.push(marker(range.start, range.start + 2));
                        spans.push(marker(range.start + idx, range.end));
                    }
                    // v1: solo imágenes en bloque — las que son el único
                    // contenido de su línea (salvo espacios). Con texto
                    // alrededor no se genera nada nuevo. La marca `![alt](src)`
                    // sigue visible y editable en todos los modos (la imagen
                    // pintada es aditiva, no sustituye).
                    let line_start = text[..range.start].rfind('\n').map(|i| i + 1).unwrap_or(0);
                    let line_end = text[range.end..]
                        .find('\n')
                        .map(|i| range.end + i)
                        .unwrap_or(text.len());
                    let sola = text[line_start..range.start].trim().is_empty()
                        && text[range.end..line_end].trim().is_empty();
                    if sola {
                        let alt = src
                            .find(']')
                            .map(|i| &src[2..i])
                            .unwrap_or_default()
                            .to_string();
                        // pulldown-cmark ya ha resuelto las reference-style.
                        let dest = dest_url.to_string();
                        let line = lines.line_of(range.start);
                        images.push(ImageRef {
                            line,
                            start: range.start,
                            end: range.end,
                            dest: dest.clone(),
                            alt: alt.clone(),
                        });
                        ornaments.push(Ornament::Image { line, dest, alt });
                        // Reserva bajo la línea el hueco donde se pinta.
                        spans.push(style(line_start, line_end, "imagegap"));
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
                // La tabla no es monoespaciada (véase editor.rs): aquí solo
                // marcamos el bloque para el indent y la caja de fondo, y los
                // pipes se sustituyen por separado. El inline (strong, em,
                // code) fluye dentro de las celdas.
                table_depth += 1;
                let end = trim_nl(text, range.end);
                spans.push(style(range.start, end, "table"));
                let first = lines.line_of(range.start);
                let last = lines.line_of(end.saturating_sub(1).max(range.start));
                ornaments.push(Ornament::Table { first, last });

                let rows = line_ranges(text, range.start, end);
                // La primera línea del rango es la fila de cabecera.
                let header_start = rows.first().map(|&(ls, _)| ls);
                for (ls, le) in rows {
                    if is_delimiter_row(&text[ls..le]) {
                        // La fila de guiones se sustituye y en su hueco se
                        // pinta la línea que separa la cabecera.
                        spans.push(replaced(ls, le));
                        spans.push(style(ls, le, "tablerule"));
                        ornaments.push(Ornament::TableRule {
                            line: lines.line_of(ls),
                        });
                        continue;
                    }
                    // Los pipes son marcas sustituidas: en WYSIWYG se encogen
                    // y en su sitio el widget dibuja un separador vertical
                    // (`CellSeparator`), para dotar a la tabla de rejilla
                    // visual sin necesidad de un widget compuesto.
                    let row = &text[ls..le];
                    let bytes = row.as_bytes();
                    for (i, b) in bytes.iter().enumerate() {
                        if *b == b'|' && (i == 0 || bytes[i - 1] != b'\\') {
                            spans.push(replaced(ls + i, ls + i + 1));
                            ornaments.push(Ornament::CellSeparator { offset: ls + i });
                        }
                    }
                    // Cabecera: el texto de las celdas en negrita (span
                    // `tablehead`); los pipes quedan fuera.
                    if Some(ls) == header_start {
                        for (s, e) in cell_text_ranges(row) {
                            spans.push(style(ls + s, ls + e, "tablehead"));
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

    Analysis {
        spans,
        ornaments,
        images,
    }
}

// ---------------------------------------------------------------------------
// Alineado de tablas
// ---------------------------------------------------------------------------

/// ¿Es este fragmento un `<br>` de HTML? Acepta `<br>`, `<br/>`, `<br />` y
/// cualquier combinación de mayúsculas/minúsculas. No acepta atributos
/// (`<br class="x">`) porque no aparecen en markdown a mano.
fn is_br(content: &str) -> bool {
    let lower = content.to_ascii_lowercase();
    let Some(inner) = lower.strip_prefix('<').and_then(|s| s.strip_suffix('>')) else {
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

/// Rangos de línea (primera, última inclusive) que pulldown-cmark reconoce
/// como tablas. Acotar con el parser evita absorber líneas ajenas que solo
/// contienen un `|` (una cabecera, una regla con pipes…) después de la tabla.
fn table_line_ranges(text: &str) -> Vec<(usize, usize)> {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    let lines = LineIndex::new(text);
    let mut out = Vec::new();
    for (event, range) in Parser::new_ext(text, opts).into_offset_iter() {
        if let Event::Start(Tag::Table(_)) = event {
            let first = lines.line_of(range.start);
            let last = lines.line_of(trim_nl(text, range.end).saturating_sub(1).max(range.start));
            out.push((first, last));
        }
    }
    out
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
    let tables = table_line_ranges(text);
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

        // El cuerpo lo acota el parser: solo las líneas que forman la tabla
        // de verdad. Antes se absorbía cualquier línea posterior con un `|`,
        // aunque fuera una cabecera o un párrafo ajeno a la tabla.
        let Some(&(_, last)) = tables.iter().find(|(first, _)| *first == i) else {
            // La heurística local vio una tabla que el parser no reconoce:
            // no se toca nada.
            out.push(line.to_string());
            i += 1;
            continue;
        };
        let end = last + 1;

        let header = split_row(lines[i]);
        let aligns: Vec<Align> = split_row(lines[i + 1])
            .iter()
            .map(|c| alignment_of(c))
            .collect();
        let body: Vec<Vec<String>> = lines[i + 2..end].iter().map(|l| split_row(l)).collect();

        // Nunca se suelta una celda: las columnas cubren la cabecera Y la
        // fila más ancha del cuerpo (las celdas sobrantes antes se perdían).
        let columns = header
            .len()
            .max(body.iter().map(|row| row.len()).max().unwrap_or(0));
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

        // Conserva el EOL original de cada línea: en un documento CRLF las
        // líneas rehechas deben seguir llevando su "\r" (out se une con
        // "\n"), si no el fichero acaba con finales de línea mezclados.
        let table: Vec<String> = table
            .into_iter()
            .enumerate()
            .map(|(k, l)| {
                if lines[i + k].ends_with('\r') {
                    format!("{l}\r")
                } else {
                    l
                }
            })
            .collect();

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

    /// Tramos de sintaxis (marca o sustitución) ordenados por posición:
    /// (inicio, fin, clase). Son los rangos cuya visibilidad decide el editor;
    /// los tests asertan sobre ellos, no sobre una visibilidad simulada.
    fn syntax(text: &str) -> Vec<(usize, usize, SpanKind)> {
        let mut v: Vec<_> = spans(text)
            .iter()
            .filter(|s| s.is_syntax())
            .map(|s| (s.start, s.end, s.kind))
            .collect();
        v.sort_by_key(|s| (s.0, s.1));
        v
    }

    #[test]
    fn cabecera_marca_las_almohadillas() {
        let text = "## Título\n";
        assert_eq!(find(&spans(text), "h2").len(), 1);
        // Las almohadillas (con el espacio) son marca sustituida: en WYSIWYG
        // se encogen y el título queda limpio; en «Atenuar» se atenúan.
        assert_eq!(syntax(text), vec![(0, 3, SpanKind::Replaced)]);
    }

    #[test]
    fn h1_y_h2_piden_filete_bajo_el_titulo_y_h3_no() {
        let a = analyze("# Uno\n\n## Dos\n\n### Tres\n");
        let reglas: Vec<usize> = a
            .ornaments
            .iter()
            .filter_map(|o| match o {
                Ornament::HeadingRule { line } => Some(*line),
                _ => None,
            })
            .collect();
        assert_eq!(reglas, vec![0, 2]);
        // La cabecera setext (H1) también lo pide.
        let setext = analyze("Título\n======\n");
        assert!(setext
            .ornaments
            .contains(&Ornament::HeadingRule { line: 0 }));
    }

    #[test]
    fn cabecera_setext_marca_el_subrayado() {
        let text = "Título\n======\n";
        assert_eq!(find(&spans(text), "h1").len(), 1);
        // El subrayado y el salto que lo precede quedan como marca sustituida
        // (ojo: «í» ocupa dos bytes, el subrayado empieza en el byte 8).
        assert_eq!(syntax(text), vec![(7, 14, SpanKind::Replaced)]);
    }

    #[test]
    fn negrita_y_cursiva() {
        assert_eq!(find(&spans("un **texto**\n"), "bold").len(), 1);
        assert_eq!(
            syntax("un **texto** y *otro*\n"),
            vec![
                (3, 5, SpanKind::Marker),
                (10, 12, SpanKind::Marker),
                (15, 16, SpanKind::Marker),
                (20, 21, SpanKind::Marker),
            ]
        );
    }

    #[test]
    fn codigo_en_linea_con_varios_backticks() {
        assert_eq!(
            syntax("esto es ``a ` b`` fin"),
            vec![(8, 10, SpanKind::Marker), (15, 17, SpanKind::Marker)]
        );
    }

    #[test]
    fn enlace_marca_la_url() {
        assert_eq!(
            syntax("ver [la web](https://ej.com) ya"),
            vec![(4, 5, SpanKind::Marker), (11, 28, SpanKind::Marker)]
        );
    }

    #[test]
    fn enlace_automatico_marca_los_angulos() {
        assert_eq!(
            syntax("ver <https://ej.com> ya"),
            vec![(4, 5, SpanKind::Marker), (19, 20, SpanKind::Marker)]
        );
    }

    #[test]
    fn imagen_marca_las_marcas() {
        assert_eq!(
            syntax("![gato](/tmp/g.png)"),
            vec![(0, 2, SpanKind::Marker), (6, 19, SpanKind::Marker)]
        );
    }

    #[test]
    fn cita_marca_el_mayor_que_y_pide_barra() {
        let a = analyze("> una cita\n> y otra\n");
        assert_eq!(
            syntax("> una cita\n> y otra\n"),
            vec![(0, 2, SpanKind::Replaced), (11, 13, SpanKind::Replaced)]
        );
        assert_eq!(a.ornaments, vec![Ornament::Quote { first: 0, last: 1 }]);
    }

    #[test]
    fn lista_sin_ordenar_cambia_el_guion_por_una_vineta() {
        let a = analyze("- uno\n- dos\n");
        assert_eq!(
            syntax("- uno\n- dos\n"),
            vec![(0, 2, SpanKind::Replaced), (6, 8, SpanKind::Replaced)]
        );
        assert_eq!(
            a.ornaments,
            vec![
                Ornament::Bullet { line: 0, depth: 1 },
                Ornament::Bullet { line: 1, depth: 1 },
            ]
        );
    }

    #[test]
    fn la_sangria_literal_de_una_lista_anidada_es_sustituida() {
        // La sangría la pone el margen del tag; el texto no debe llevarla dos veces.
        assert_eq!(
            syntax("- uno\n    - dos\n"),
            vec![(0, 2, SpanKind::Replaced), (6, 12, SpanKind::Replaced)]
        );
    }

    #[test]
    fn lista_ordenada_conserva_el_numero() {
        let a = analyze("1. uno\n2. dos\n");
        assert!(syntax("1. uno\n2. dos\n").is_empty());
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
        // Viñeta y casilla de cada elemento, sustituidas por adornos.
        assert_eq!(
            syntax("- [x] hecho\n- [ ] pendiente\n"),
            vec![
                (0, 2, SpanKind::Replaced),
                (2, 6, SpanKind::Replaced),
                (12, 14, SpanKind::Replaced),
                (14, 18, SpanKind::Replaced),
            ]
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
    fn bloque_de_codigo_pide_caja_y_marca_las_vallas() {
        let text = "```rust\nfn main() {}\n```\n";
        let a = analyze(text);
        assert_eq!(find(&a.spans, "codeblock").len(), 1);
        assert!(a
            .ornaments
            .contains(&Ornament::CodeBlock { first: 0, last: 2 }));
        // Se sustituyen las comillas de apertura (el nombre del lenguaje queda
        // como etiqueta) y la línea entera de la valla de cierre.
        assert_eq!(
            syntax(text),
            vec![(0, 3, SpanKind::Replaced), (21, 24, SpanKind::Replaced)]
        );
        assert_eq!(find(&a.spans, "fence").len(), 1);
    }

    #[test]
    fn el_markdown_dentro_de_codigo_no_se_decora() {
        assert!(find(&spans("```\nesto **no** es negrita\n```\n"), "bold").is_empty());
    }

    #[test]
    fn la_regla_sustituye_la_linea_para_dibujarla() {
        let a = analyze("uno\n\n---\n\ndos\n");
        assert!(a.ornaments.contains(&Ornament::Rule { line: 2 }));
        assert_eq!(
            syntax("uno\n\n---\n\ndos\n"),
            vec![(5, 8, SpanKind::Replaced)]
        );
    }

    #[test]
    fn las_notas_al_pie_marcan_los_corchetes() {
        let text = "Texto[^1].\n\n[^1]: La nota.\n";
        assert_eq!(
            syntax(text),
            vec![(5, 7, SpanKind::Marker), (8, 9, SpanKind::Marker)]
        );
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
                | Ornament::TableRule { line }
                | Ornament::HeadingRule { line }
                | Ornament::Image { line, .. } => line,
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
    fn la_fila_de_guiones_se_sustituye_y_pide_una_linea() {
        let text = "| a | b |\n|---|---|\n| 1 | 2 |\n";
        let a = analyze(text);
        assert!(a.ornaments.contains(&Ornament::TableRule { line: 1 }));
        assert!(a.ornaments.contains(&Ornament::Table { first: 0, last: 2 }));
        // Los pipes también son marcas sustituidas (se encogen en WYSIWYG).
        assert_eq!(
            syntax(text),
            vec![
                (0, 1, SpanKind::Replaced),
                (4, 5, SpanKind::Replaced),
                (8, 9, SpanKind::Replaced),
                (10, 19, SpanKind::Replaced),
                (20, 21, SpanKind::Replaced),
                (24, 25, SpanKind::Replaced),
                (28, 29, SpanKind::Replaced),
            ]
        );
    }

    #[test]
    fn los_pipes_de_tabla_son_marcas_sustituidas() {
        // Tres por fila de contenido, dos filas: se encogen en WYSIWYG y el
        // widget dibuja un separador vertical en su sitio.
        let a = analyze("| a | b |\n|---|---|\n| 1 | 2 |\n");
        let pipes = a
            .spans
            .iter()
            .filter(|s| s.kind == SpanKind::Replaced && s.end == s.start + 1)
            .count();
        assert_eq!(pipes, 6);
    }

    #[test]
    fn la_cabecera_de_tabla_lleva_tablehead_sobre_el_texto_de_las_celdas() {
        let text = "| a | b |\n|---|---|\n| 1 | 2 |\n";
        let a = analyze(text);
        let heads = find(&a.spans, "tablehead");
        assert_eq!(heads.len(), 2);
        assert_eq!(&text[heads[0].start..heads[0].end], "a");
        assert_eq!(&text[heads[1].start..heads[1].end], "b");
        // Solo la cabecera: el cuerpo no lleva tablehead.
        assert!(heads.iter().all(|s| s.end <= 9));
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
    fn br_dentro_de_tabla_se_sustituye_y_genera_adorno() {
        let text = "| a | b |\n|---|---|\n| x<br>y | z |\n";
        let a = analyze(text);
        // El `<br>` debe quedar como marca sustituida (Replaced), no como html.
        let brs = a
            .spans
            .iter()
            .filter(|s| s.kind == SpanKind::Replaced && text[s.start..s.end].contains("br"))
            .count();
        assert_eq!(brs, 1, "debe haber exactamente un <br> sustituido");
        assert!(a
            .ornaments
            .iter()
            .any(|o| matches!(o, Ornament::Break { .. })));
        // Y nada con tag `html` dentro de la tabla.
        assert_eq!(find(&a.spans, "html").len(), 0);
    }

    #[test]
    fn br_fuera_de_tabla_queda_como_html() {
        let text = "línea uno<br>línea dos\n";
        let a = analyze(text);
        assert_eq!(find(&a.spans, "html").len(), 1);
        assert!(!a
            .ornaments
            .iter()
            .any(|o| matches!(o, Ornament::Break { .. })));
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
    fn los_delimitadores_inline_de_tabla_son_siempre_sustituidos() {
        // En las tablas los delimitadores (`**`, `*`, backticks) se marcan como
        // Replaced, no como Marker: su visibilidad no puede depender de la
        // línea del cursor, porque revelarlos sobre el texto en negrita
        // descuadraría la celda.
        let text = "| **a** | `b` |\n|---|---|\n| *c* | d |\n";
        let a = analyze(text);
        assert!(
            !a.spans.iter().any(|s| s.kind == SpanKind::Marker),
            "ninguna marca `Marker` debe quedar dentro de la tabla: {a:?}"
        );
        // Los pipes, los dos delimitadores de `**a**`, los dos de `` `b` ``,
        // la fila de guiones y los dos de `*c*`, por orden de posición.
        assert_eq!(
            syntax(text),
            vec![
                (0, 1, SpanKind::Replaced),
                (2, 4, SpanKind::Replaced),
                (5, 7, SpanKind::Replaced),
                (8, 9, SpanKind::Replaced),
                (10, 11, SpanKind::Replaced),
                (12, 13, SpanKind::Replaced),
                (14, 15, SpanKind::Replaced),
                (16, 25, SpanKind::Replaced),
                (26, 27, SpanKind::Replaced),
                (28, 29, SpanKind::Replaced),
                (30, 31, SpanKind::Replaced),
                (32, 33, SpanKind::Replaced),
                (36, 37, SpanKind::Replaced),
            ]
        );
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
        // y debe coincidir con el número de pipes sustituidos (spans
        // Replaced de un byte; la fila de guiones es uno solo y más largo).
        let pipes = a
            .spans
            .iter()
            .filter(|s| s.kind == SpanKind::Replaced && s.end == s.start + 1)
            .count();
        assert_eq!(seps, pipes);
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
    fn alinear_no_absorbe_lineas_ajenas_con_pipes() {
        // La cabecera `#` cierra la tabla aunque contenga un `|`: antes se
        // absorbía como si fuera una fila más y se reformateaba.
        let entrada = "| a | b |\n|---|---|\n| 1 | 2 |\n# Título | con pipe\n";
        let esperado = "| a   | b   |\n| --- | --- |\n| 1   | 2   |\n# Título | con pipe\n";
        assert_eq!(format_tables(entrada).unwrap(), esperado);
    }

    #[test]
    fn alinear_conserva_las_celdas_sobrantes() {
        // La fila es más ancha que la cabecera: la celda extra no se pierde,
        // la tabla crece a tres columnas.
        let entrada = "| a | b |\n|---|---|\n| 1 | 2 | 3 |\n";
        let esperado = "| a   | b   |     |\n| --- | --- | --- |\n| 1   | 2   | 3   |\n";
        assert_eq!(format_tables(entrada).unwrap(), esperado);
    }

    #[test]
    fn alinear_rellena_las_celdas_que_faltan() {
        let entrada = "| a | b | c |\n|---|---|---|\n| 1 | 2 |\n";
        let esperado = "| a   | b   | c   |\n| --- | --- | --- |\n| 1   | 2   |     |\n";
        assert_eq!(format_tables(entrada).unwrap(), esperado);
    }

    #[test]
    fn alinear_conserva_los_finales_crlf() {
        // GtkTextBuffer conserva los \r al abrir ficheros CRLF: las líneas
        // rehechas no pueden salir con LF o el documento queda con EOL mixto.
        let entrada = "| a | b |\r\n|---|---|\r\n| 1 | 22 |\r\n\r\nTexto\r\n";
        let esperado = "| a   | b   |\r\n| --- | --- |\r\n| 1   | 22  |\r\n\r\nTexto\r\n";
        assert_eq!(format_tables(entrada).unwrap(), esperado);
    }

    #[test]
    fn alinear_una_tabla_crlf_ya_alineada_devuelve_none() {
        let entrada = "| a   | b   |\r\n| --- | --- |\r\n| 1   | 2   |\r\n";
        assert_eq!(format_tables(entrada), None);
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

    #[test]
    fn cada_adorno_tiene_su_familia() {
        assert_eq!(
            Ornament::CodeBlock { first: 0, last: 1 }.family(),
            OrnamentFamily::Ambient
        );
        assert_eq!(
            Ornament::Table { first: 0, last: 1 }.family(),
            OrnamentFamily::Ambient
        );
        assert_eq!(
            Ornament::Quote { first: 0, last: 1 }.family(),
            OrnamentFamily::Ambient
        );
        assert_eq!(
            Ornament::Bullet { line: 0, depth: 1 }.family(),
            OrnamentFamily::Substitute
        );
        assert_eq!(
            Ornament::Checkbox {
                line: 0,
                checked: false
            }
            .family(),
            OrnamentFamily::Substitute
        );
        assert_eq!(
            Ornament::Rule { line: 0 }.family(),
            OrnamentFamily::Substitute
        );
        assert_eq!(
            Ornament::TableRule { line: 0 }.family(),
            OrnamentFamily::Substitute
        );
        assert_eq!(
            Ornament::Break { offset: 0 }.family(),
            OrnamentFamily::Substitute
        );
        assert_eq!(
            Ornament::CellSeparator { offset: 0 }.family(),
            OrnamentFamily::Substitute
        );
        assert_eq!(
            Ornament::HeadingRule { line: 0 }.family(),
            OrnamentFamily::Additive
        );
        assert_eq!(
            Ornament::Image {
                line: 0,
                dest: String::new(),
                alt: String::new()
            }
            .family(),
            OrnamentFamily::Additive
        );
    }

    #[test]
    fn en_atenuar_solo_se_pintan_ambientales_y_aditivos() {
        // Documento con un adorno de cada familia disponible hoy: cita y caja
        // de código (ambientales), viñeta y regla (sustitutivos).
        let a = analyze("> cita\n\n- uno\n\n---\n\n```\nx\n```\n");
        let filtrados = ornaments_for(&a.ornaments, MarkupVisibility::Dim);
        assert!(
            filtrados
                .iter()
                .all(|o| o.family() != OrnamentFamily::Substitute),
            "en «Atenuar» no debe pintarse ningún sustitutivo: {filtrados:?}"
        );
        assert!(filtrados
            .iter()
            .any(|o| matches!(o, Ornament::Quote { .. })));
        assert!(filtrados
            .iter()
            .any(|o| matches!(o, Ornament::CodeBlock { .. })));
    }

    #[test]
    fn imagen_sola_en_su_linea_genera_referencia_adorno_y_hueco() {
        let text = "![gato](/tmp/g.png)\n";
        let a = analyze(text);
        assert_eq!(
            a.images,
            vec![ImageRef {
                line: 0,
                start: 0,
                end: 19,
                dest: "/tmp/g.png".to_string(),
                alt: "gato".to_string(),
            }]
        );
        assert!(a.ornaments.contains(&Ornament::Image {
            line: 0,
            dest: "/tmp/g.png".to_string(),
            alt: "gato".to_string(),
        }));
        // El hueco lo reserva el span de estilo `imagegap`, sobre la línea.
        assert_eq!(find(&a.spans, "imagegap").len(), 1);
        // La marca sigue siendo eso, una marca: visible y editable.
        assert_eq!(
            syntax(text),
            vec![(0, 2, SpanKind::Marker), (6, 19, SpanKind::Marker)]
        );
    }

    #[test]
    fn imagen_con_texto_alrededor_no_genera_nada_nuevo() {
        let text = "ver ![gato](/tmp/g.png) ya\n";
        let a = analyze(text);
        assert!(a.images.is_empty());
        assert!(!a
            .ornaments
            .iter()
            .any(|o| matches!(o, Ornament::Image { .. })));
        assert!(find(&a.spans, "imagegap").is_empty());
        // Pero la imagen sigue decorada como hasta ahora.
        assert_eq!(
            syntax(text),
            vec![(4, 6, SpanKind::Marker), (10, 23, SpanKind::Marker)]
        );
    }

    #[test]
    fn imagen_por_referencia_resuelve_el_destino() {
        let text = "![gato][id]\n\n[id]: /tmp/g.png\n";
        let a = analyze(text);
        assert_eq!(a.images.len(), 1);
        assert_eq!(a.images[0].dest, "/tmp/g.png");
        assert_eq!(a.images[0].alt, "gato");
        assert!(a
            .ornaments
            .iter()
            .any(|o| matches!(o, Ornament::Image { dest, .. } if dest == "/tmp/g.png")));
    }

    #[test]
    fn los_adornos_aditivos_se_pintan_tambien_en_atenuar() {
        let a = analyze("# T\n\n![gato](/tmp/g.png)\n");
        let filtrados = ornaments_for(&a.ornaments, MarkupVisibility::Dim);
        assert!(filtrados
            .iter()
            .any(|o| matches!(o, Ornament::HeadingRule { .. })));
        assert!(filtrados
            .iter()
            .any(|o| matches!(o, Ornament::Image { .. })));
    }

    #[test]
    fn en_ocultar_y_al_enfocar_se_pintan_todos_los_adornos() {
        let a = analyze("> cita\n\n- uno\n\n---\n");
        assert_eq!(
            ornaments_for(&a.ornaments, MarkupVisibility::Hidden),
            a.ornaments
        );
        assert_eq!(
            ornaments_for(&a.ornaments, MarkupVisibility::Focus),
            a.ornaments
        );
    }
}
