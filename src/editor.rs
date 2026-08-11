use gtk4::pango;
use gtk4::prelude::*;
use gtksourceview5::prelude::*;
use libadwaita as adw;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use crate::markdown_render::{analyze, Ornament, SpanKind, MAX_LIVE_BYTES};
use crate::markdown_view::{MarkdownView, OrnamentPalette};
use crate::settings::{FontFamily, MarkupVisibility};

type ChangedCallback = Rc<RefCell<Option<Box<dyn Fn(&str)>>>>;

const MIN_MARGIN: i32 = 24;

/// Márgenes extra (izquierdo, derecho) de cada tag de bloque respecto al margen
/// de la columna. El derecho importa en los bloques con caja dibujada: deja el
/// texto por dentro del recuadro en vez de pegado al borde.
const BLOCK_INDENTS: [(&str, i32, i32); 6] = [
    ("quote", 26, 0),
    ("codeblock", 24, 24),
    ("table", 24, 24),
    ("li1", 26, 0),
    ("li2", 52, 0),
    ("li3", 78, 0),
];

#[derive(Clone, Copy)]
struct Decoration {
    markup: MarkupVisibility,
    focus_mode: bool,
}

impl Default for Decoration {
    fn default() -> Self {
        Self {
            markup: MarkupVisibility::Focus,
            focus_mode: false,
        }
    }
}

pub struct Editor {
    pub widget: gtk4::ScrolledWindow,
    pub view: MarkdownView,
    buffer: gtksourceview5::Buffer,
    tags: gtk4::TextTagTable,
    on_changed: ChangedCallback,
    css: gtk4::CssProvider,
    last_line: Rc<Cell<i32>>,
    generation: Rc<Cell<u64>>,
    decoration: Rc<Cell<Decoration>>,
    column_width: Rc<Cell<i32>>,
    continue_lists: Rc<Cell<bool>>,
    typewriter: Rc<Cell<bool>>,
}

fn build_tags() -> gtk4::TextTagTable {
    let table = gtk4::TextTagTable::new();
    let add = |t: gtk4::TextTag| {
        table.add(&t);
    };

    // El orden fija la prioridad: lo añadido después gana.
    // Bloques → cabeceras → en línea → marcas → atenuado de foco.

    for (name, _, _) in BLOCK_INDENTS {
        let builder = gtk4::TextTag::builder().name(name);
        let tag = match name {
            "quote" => builder.style(pango::Style::Italic).build(),
            // `table` ya no es monoespaciada: deja que el inline (strong, em,
            // code, links) que genera `analyze` se interprete dentro de las
            // celdas. El padding del fuente (véase `format_tables`) queda como
            // separación natural; los pipes se atenúan por separado con
            // `tablepipe`.
            "codeblock" => builder.family("monospace").scale(0.9).build(),
            // Sin sangría francesa: la viñeta se dibuja en el canalón, así que
            // todas las líneas del elemento arrancan en el mismo sitio.
            _ => builder.build(),
        };
        add(tag);
    }

    // La fila de guiones va oculta: se encoge para que el hueco donde se pinta
    // la línea de cabecera no ocupe un renglón entero.
    add(gtk4::TextTag::builder()
        .name("tablerule")
        .scale(0.4)
        .pixels_above_lines(0)
        .pixels_below_lines(0)
        .build());
    // Los pipes siguen en monoespaciada para que cuadren con el padding del
    // fuente aunque el contenido de la celda sea proporcional.
    add(gtk4::TextTag::builder()
        .name("tablepipe")
        .family("monospace")
        .scale(0.9)
        .build());
    add(gtk4::TextTag::builder()
        .name("fence")
        .family("monospace")
        .scale(0.72)
        .build());
    add(gtk4::TextTag::builder()
        .name("html")
        .family("monospace")
        .scale(0.85)
        .build());

    // Nota: GtkTextTag:letter-spacing no admite valores negativos, así que el
    // ajuste óptico de las cabeceras se hace solo con escala y peso.
    for (name, scale, weight, above, below) in [
        ("h1", 1.9_f64, 800, 30, 12),
        ("h2", 1.5, 750, 26, 10),
        ("h3", 1.24, 700, 20, 8),
        ("h4", 1.1, 700, 16, 6),
        ("h5", 1.0, 700, 14, 5),
        ("h6", 1.0, 700, 14, 5),
    ] {
        add(gtk4::TextTag::builder()
            .name(name)
            .scale(scale)
            .weight(weight)
            .pixels_above_lines(above)
            .pixels_below_lines(below)
            .build());
    }

    add(gtk4::TextTag::builder().name("bold").weight(700).build());
    add(gtk4::TextTag::builder()
        .name("italic")
        .style(pango::Style::Italic)
        .build());
    add(gtk4::TextTag::builder()
        .name("strike")
        .strikethrough(true)
        .build());
    add(gtk4::TextTag::builder()
        .name("code")
        .family("monospace")
        .scale(0.9)
        .build());
    add(gtk4::TextTag::builder()
        .name("link")
        .underline(pango::Underline::Single)
        .build());
    add(gtk4::TextTag::builder()
        .name("footnote")
        .scale(0.8)
        .rise(4000)
        .build());
    add(gtk4::TextTag::builder()
        .name("footnotedef")
        .scale(0.88)
        .build());
    add(gtk4::TextTag::builder()
        .name("listmarker")
        .weight(700)
        .build());

    add(gtk4::TextTag::builder()
        .name("syn_hidden")
        .invisible(true)
        .build());
    add(gtk4::TextTag::builder().name("syn_shown").build());

    // El modo foco atenúa todo salvo el párrafo actual: va el último para pisar
    // el color de cualquier otro tag.
    add(gtk4::TextTag::builder().name("unfocused").build());

    table
}

/// GtkSourceView pinta el fondo desde su *style scheme*, no desde el tema GTK.
fn apply_scheme(buffer: &gtksourceview5::Buffer, dark: bool) {
    let manager = gtksourceview5::StyleSchemeManager::default();
    let candidates: [&str; 2] = if dark {
        ["Adwaita-dark", "classic-dark"]
    } else {
        ["Adwaita", "classic"]
    };
    for id in candidates {
        if let Some(scheme) = manager.scheme(id) {
            buffer.set_style_scheme(Some(&scheme));
            return;
        }
    }
}

fn rgba(hex: u32, alpha: f32) -> gtk4::gdk::RGBA {
    gtk4::gdk::RGBA::new(
        ((hex >> 16) & 0xff) as f32 / 255.0,
        ((hex >> 8) & 0xff) as f32 / 255.0,
        (hex & 0xff) as f32 / 255.0,
        alpha,
    )
}

fn apply_theme(tags: &gtk4::TextTagTable, view: &MarkdownView, dark: bool) {
    let set = |name: &str, fg: Option<&str>, para_bg: Option<&str>| {
        if let Some(tag) = tags.lookup(name) {
            tag.set_foreground(fg);
            tag.set_paragraph_background(para_bg);
        }
    };

    // Las citas y los bloques de código ya no llevan fondo de párrafo: su caja
    // y su barra se dibujan, que permite esquinas redondeadas y márgenes.
    if dark {
        set("code", Some("#f0a868"), None);
        set("codeblock", Some("#e4e4e4"), None);
        set("table", Some("#e0e0e0"), None);
        set("tablepipe", Some("#5f5f5f"), None);
        set("fence", Some("#8a8a8a"), None);
        set("quote", Some("#c2c2c2"), None);
        set("link", Some("#82b8f0"), None);
        set("footnote", Some("#82b8f0"), None);
        set("listmarker", Some("#82b8f0"), None);
        set("html", Some("#8f8f8f"), None);
        set("footnotedef", Some("#a0a0a0"), None);
        set("syn_shown", Some("#787878"), None);
        set("unfocused", Some("#5c5c5c"), None);
        view.set_palette(OrnamentPalette {
            accent: rgba(0x82b8f0, 1.0),
            muted: rgba(0x6f6f6f, 1.0),
            block: rgba(0xffffff, 0.07),
            table: rgba(0xffffff, 0.05),
            quote: rgba(0x6f6f6f, 1.0),
            on_accent: rgba(0x1b1b1b, 1.0),
        });
    } else {
        set("code", Some("#a34a00"), None);
        set("codeblock", Some("#1f1f1f"), None);
        set("table", Some("#1f1f1f"), None);
        set("tablepipe", Some("#c0bfbc"), None);
        set("fence", Some("#8b8a88"), None);
        set("quote", Some("#54535a"), None);
        set("link", Some("#1a6ed8"), None);
        set("footnote", Some("#1a6ed8"), None);
        set("listmarker", Some("#1a6ed8"), None);
        set("html", Some("#8b8a88"), None);
        set("footnotedef", Some("#77767b"), None);
        set("syn_shown", Some("#b5b4b1"), None);
        set("unfocused", Some("#bdbcba"), None);
        view.set_palette(OrnamentPalette {
            accent: rgba(0x1a6ed8, 1.0),
            muted: rgba(0xc0bfbc, 1.0),
            block: rgba(0x000000, 0.05),
            table: rgba(0x000000, 0.035),
            quote: rgba(0xc0bfbc, 1.0),
            on_accent: rgba(0xffffff, 1.0),
        });
    }
}

fn set_column_margins(tags: &gtk4::TextTagTable, base: i32) {
    for (name, left, right) in BLOCK_INDENTS {
        if let Some(tag) = tags.lookup(name) {
            tag.set_left_margin(base + left);
            tag.set_right_margin(base + right);
        }
    }
    for name in ["fence"] {
        if let Some(tag) = tags.lookup(name) {
            tag.set_left_margin(base + 24);
            tag.set_right_margin(base + 24);
        }
    }
}

/// Límites del párrafo (bloque entre líneas en blanco) que contiene `line`.
fn paragraph_bounds(buffer: &gtksourceview5::Buffer, line: i32) -> (i32, i32) {
    let is_blank = |n: i32| -> bool {
        match buffer.iter_at_line(n) {
            Some(start) => {
                let mut end = start;
                if !end.ends_line() {
                    end.forward_to_line_end();
                }
                buffer.text(&start, &end, true).trim().is_empty()
            }
            None => true,
        }
    };
    let mut first = line;
    while first > 0 && !is_blank(first - 1) {
        first -= 1;
    }
    let last_line = buffer.end_iter().line();
    let mut last = line;
    while last < last_line && !is_blank(last + 1) {
        last += 1;
    }
    (first, last)
}

/// Toda la decoracion pasa por aqui.
///
/// Nunca se aplican tags desde dentro de una senal del buffer: GTK no espera
/// que la invisibilidad de su texto cambie mientras esta procesando una
/// edicion, y hacerlo descuadra la maquetacion. El sintoma es un aborto en
/// `gtk_text_iter_set_visible_line_index` («byte index off the end of the
/// line») y, antes de llegar ahi, tags de bloque que se extienden mas alla de
/// su rango. Se difiere siempre al bucle principal.
fn schedule_decoration(
    view: &MarkdownView,
    buffer: &gtksourceview5::Buffer,
    decoration: &Rc<Cell<Decoration>>,
    generation: &Rc<Cell<u64>>,
    delay: Duration,
) {
    let current = generation.get().wrapping_add(1);
    generation.set(current);

    let view = view.clone();
    let buffer = buffer.clone();
    let decoration = decoration.clone();
    let generation = generation.clone();
    glib::timeout_add_local_once(delay, move || {
        // Si ha llegado otra peticion mientras esperabamos, esta sobra.
        if generation.get() != current {
            return;
        }
        decorate(&view, &buffer, decoration.get());
    });
}

fn decorate(view: &MarkdownView, buffer: &gtksourceview5::Buffer, config: Decoration) {
    let start = buffer.start_iter();
    let end = buffer.end_iter();
    buffer.remove_all_tags(&start, &end);

    let line_count = end.line() + 1;
    let text = buffer.text(&start, &end, true).to_string();
    if text.is_empty() || text.len() > MAX_LIVE_BYTES {
        view.set_ornaments(Vec::new(), line_count);
        return;
    }

    // GtkTextBuffer indexa por caracteres; pulldown-cmark, por bytes.
    let mut byte_to_char = vec![0i32; text.len() + 1];
    let mut char_index = 0i32;
    let mut byte_index = 0usize;
    for ch in text.chars() {
        for slot in byte_to_char.iter_mut().skip(byte_index).take(ch.len_utf8()) {
            *slot = char_index;
        }
        byte_index += ch.len_utf8();
        char_index += 1;
    }
    byte_to_char[text.len()] = char_index;

    let analysis = analyze(&text);
    let cursor_line = buffer.iter_at_offset(buffer.cursor_position()).line();
    let dim = config.markup == MarkupVisibility::Dim;

    for span in &analysis.spans {
        let (from, to) = (byte_to_char[span.start], byte_to_char[span.end]);
        if from >= to {
            continue;
        }
        let start_iter = buffer.iter_at_offset(from);
        let end_iter = buffer.iter_at_offset(to);
        let name = match span.kind {
            SpanKind::Style => span.tag,
            // Las marcas sustituidas por un adorno no se revelan al pasar el
            // cursor: hacerlo movería el texto de sitio en cada línea.
            SpanKind::Replaced => {
                if dim {
                    "syn_shown"
                } else {
                    "syn_hidden"
                }
            }
            SpanKind::Marker => {
                // invisible descuadra la maquetación de GTK combinado con
                // bloques de código y adornos. Se atenúa sin ocultar.
                "syn_shown"
            }
        };
        buffer.apply_tag_by_name(name, &start_iter, &end_iter);
    }

    // En modo «atenuar» las marcas están a la vista, así que dibujar encima
    // duplicaría la información.
    let mut ornaments = if dim { Vec::new() } else { analysis.ornaments };
    // Los adornos van por número de línea salvo `Break` y `CellSeparator`, que
    // guardan byte offsets: el widget indexa por caracteres, así que convertimos.
    for o in &mut ornaments {
        let cap = byte_to_char.len().saturating_sub(1);
        match o {
            Ornament::Break { offset } | Ornament::CellSeparator { offset } => {
                let at = (*offset).min(cap);
                *offset = byte_to_char[at] as usize;
            }
            _ => {}
        }
    }
    view.set_ornaments(ornaments, line_count);

    if config.focus_mode {
        let (first, last) = paragraph_bounds(buffer, cursor_line);
        if let Some(para_start) = buffer.iter_at_line(first) {
            buffer.apply_tag_by_name("unfocused", &buffer.start_iter(), &para_start);
        }
        let after = buffer
            .iter_at_line(last + 1)
            .unwrap_or_else(|| buffer.end_iter());
        buffer.apply_tag_by_name("unfocused", &after, &buffer.end_iter());
    }
}

/// Marcador que continúa una lista, o `None` si la línea no es un elemento.
/// El `bool` indica que el elemento está vacío, en cuyo caso hay que cerrarlo.
fn list_continuation(line: &str) -> Option<(String, bool)> {
    let indent: String = line
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect();
    let rest = &line[indent.len()..];
    let first = rest.chars().next()?;

    let (marker, body) = if matches!(first, '-' | '*' | '+') {
        let after = &rest[1..];
        let gap = after.len() - after.trim_start_matches(' ').len();
        if gap == 0 {
            return None;
        }
        (format!("{first}{}", " ".repeat(gap)), &after[gap..])
    } else if first.is_ascii_digit() {
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        let after = &rest[digits.len()..];
        let delim = after.chars().next()?;
        if delim != '.' && delim != ')' {
            return None;
        }
        let after = &after[1..];
        let gap = after.len() - after.trim_start_matches(' ').len();
        if gap == 0 {
            return None;
        }
        let next = digits.parse::<u64>().unwrap_or(1) + 1;
        (format!("{next}{delim}{}", " ".repeat(gap)), &after[gap..])
    } else {
        return None;
    };

    // Casillas de tarea: la siguiente empieza sin marcar.
    let (marker, body) = if let Some(stripped) = body
        .strip_prefix("[ ] ")
        .or_else(|| body.strip_prefix("[x] "))
        .or_else(|| body.strip_prefix("[X] "))
    {
        (format!("{marker}[ ] "), stripped)
    } else {
        (marker, body)
    };

    Some((format!("{indent}{marker}"), body.trim().is_empty()))
}

impl Editor {
    pub fn new() -> Self {
        let tags = build_tags();
        let buffer = gtksourceview5::Buffer::new(Some(&tags));
        buffer.set_highlight_syntax(false);
        buffer.set_highlight_matching_brackets(false);

        let view = MarkdownView::with_buffer(&buffer);
        view.set_wrap_mode(gtk4::WrapMode::Word);
        view.set_show_line_numbers(false);
        view.set_show_right_margin(false);
        view.set_highlight_current_line(false);
        view.set_indent_width(4);
        view.set_tab_width(4);
        view.set_insert_spaces_instead_of_tabs(true);
        view.set_smart_backspace(true);
        view.set_top_margin(52);
        view.set_bottom_margin(280);
        view.set_left_margin(MIN_MARGIN);
        view.set_right_margin(MIN_MARGIN);
        view.add_css_class("scribe-editor");

        let widget = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vexpand(true)
            .hexpand(true)
            .child(&view)
            .build();

        let css = gtk4::CssProvider::new();
        if let Some(display) = gdk4::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &css,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }

        let dark = adw::StyleManager::default().is_dark();
        apply_scheme(&buffer, dark);
        apply_theme(&tags, &view, dark);
        set_column_margins(&tags, MIN_MARGIN);

        let editor = Self {
            widget,
            view,
            buffer,
            tags,
            on_changed: Rc::new(RefCell::new(None)),
            css,
            last_line: Rc::new(Cell::new(-1)),
            generation: Rc::new(Cell::new(0)),
            decoration: Rc::new(Cell::new(Decoration::default())),
            column_width: Rc::new(Cell::new(720)),
            continue_lists: Rc::new(Cell::new(true)),
            typewriter: Rc::new(Cell::new(false)),
        };
        editor.connect_signals();
        editor
    }

    fn connect_signals(&self) {
        if let Some(hadj) = self.view.hadjustment() {
            let view = self.view.clone();
            let tags = self.tags.clone();
            let column_width = self.column_width.clone();
            hadj.connect_page_size_notify(move |adj| {
                let width = adj.page_size() as i32;
                if width <= 0 {
                    return;
                }
                let margin = ((width - column_width.get()) / 2).max(MIN_MARGIN);
                if view.left_margin() != margin {
                    view.set_left_margin(margin);
                    view.set_right_margin(margin);
                    set_column_margins(&tags, margin);
                }
            });
        }

        let tags = self.tags.clone();
        let buffer = self.buffer.clone();
        let view = self.view.clone();
        let decoration = self.decoration.clone();
        let generation = self.generation.clone();
        adw::StyleManager::default().connect_dark_notify(move |sm| {
            apply_scheme(&buffer, sm.is_dark());
            apply_theme(&tags, &view, sm.is_dark());
            schedule_decoration(&view, &buffer, &decoration, &generation, Duration::ZERO);
        });

        let on_changed = self.on_changed.clone();
        let generation = self.generation.clone();
        let last_line = self.last_line.clone();
        let decoration = self.decoration.clone();
        let view = self.view.clone();
        self.buffer.connect_changed(move |buf| {
            let text = buf.text(&buf.start_iter(), &buf.end_iter(), true);
            if let Some(cb) = on_changed.borrow().as_ref() {
                cb(&text);
            }
            last_line.set(buf.iter_at_offset(buf.cursor_position()).line());
            schedule_decoration(
                &view,
                buf,
                &decoration,
                &generation,
                Duration::from_millis(45),
            );
        });

        let last_line = self.last_line.clone();
        let decoration = self.decoration.clone();
        let generation = self.generation.clone();
        let typewriter = self.typewriter.clone();
        let view = self.view.clone();
        self.buffer.connect_cursor_position_notify(move |buf| {
            if typewriter.get() {
                let view = view.clone();
                let mark = buf.get_insert();
                glib::idle_add_local_once(move || {
                    view.scroll_to_mark(&mark, 0.0, true, 0.0, 0.5);
                });
            }
            let line = buf.iter_at_offset(buf.cursor_position()).line();
            if last_line.get() == line {
                return;
            }
            last_line.set(line);
            let config = decoration.get();
            // En «ocultar» y «atenuar» el marcado no depende del cursor; solo
            // hay que repintar si algo lo sigue.
            if config.markup == MarkupVisibility::Focus || config.focus_mode {
                schedule_decoration(&view, buf, &decoration, &generation, Duration::ZERO);
            }
        });

        // Continuar listas al pulsar Intro. Fase de captura para adelantarnos
        // al manejador propio del GtkTextView.
        let controller = gtk4::EventControllerKey::new();
        controller.set_propagation_phase(gtk4::PropagationPhase::Capture);
        let buffer = self.buffer.clone();
        let continue_lists = self.continue_lists.clone();
        controller.connect_key_pressed(move |_, key, _, state| {
            let plain = !state.intersects(
                gdk4::ModifierType::CONTROL_MASK
                    | gdk4::ModifierType::SHIFT_MASK
                    | gdk4::ModifierType::ALT_MASK,
            );
            if !continue_lists.get()
                || !plain
                || !matches!(key, gdk4::Key::Return | gdk4::Key::KP_Enter)
            {
                return glib::Propagation::Proceed;
            }

            let cursor = buffer.iter_at_offset(buffer.cursor_position());
            if !cursor.ends_line() || buffer.selection_bounds().is_some() {
                return glib::Propagation::Proceed;
            }
            let Some(line_start) = buffer.iter_at_line(cursor.line()) else {
                return glib::Propagation::Proceed;
            };
            let line = buffer.text(&line_start, &cursor, true).to_string();
            let Some((marker, empty)) = list_continuation(&line) else {
                return glib::Propagation::Proceed;
            };

            buffer.begin_user_action();
            if empty {
                let mut from = line_start;
                let mut to = cursor;
                buffer.delete(&mut from, &mut to);
            } else {
                let mut at = buffer.iter_at_offset(buffer.cursor_position());
                buffer.insert(&mut at, &format!("\n{marker}"));
            }
            buffer.end_user_action();
            glib::Propagation::Stop
        });
        self.view.add_controller(controller);
    }

    // --- contenido -------------------------------------------------------

    pub fn set_text(&self, text: &str) {
        self.buffer.set_text(text);
        self.buffer.set_modified(false);
        let start = self.buffer.start_iter();
        self.buffer.place_cursor(&start);
        self.last_line.set(0);
        self.refresh();
    }

    pub fn text(&self) -> String {
        self.buffer
            .text(&self.buffer.start_iter(), &self.buffer.end_iter(), true)
            .to_string()
    }

    pub fn refresh(&self) {
        schedule_decoration(
            &self.view,
            &self.buffer,
            &self.decoration,
            &self.generation,
            Duration::ZERO,
        );
    }

    pub fn connect_changed<F: Fn(&str) + 'static>(&self, callback: F) {
        *self.on_changed.borrow_mut() = Some(Box::new(callback));
    }

    pub fn connect_cursor_moved<F: Fn() + 'static>(&self, callback: F) {
        self.buffer
            .connect_cursor_position_notify(move |_| callback());
    }

    pub fn cursor_position(&self) -> (i32, i32) {
        let iter = self.buffer.iter_at_offset(self.buffer.cursor_position());
        (iter.line() + 1, iter.line_offset() + 1)
    }

    pub fn line_count(&self) -> i32 {
        self.buffer.end_iter().line() + 1
    }

    pub fn go_to_line(&self, line: i32) {
        self.scroll_to_line((line - 1).max(0));
    }

    pub fn scroll_to_line(&self, line: i32) {
        if let Some(mut iter) = self.buffer.iter_at_line(line) {
            self.buffer.place_cursor(&iter);
            self.view.scroll_to_iter(&mut iter, 0.0, true, 0.0, 0.3);
            self.view.grab_focus();
        }
    }

    /// Envuelve la selección con un marcador (`**`, `*`, `` ` ``…).
    pub fn wrap_selection(&self, marker: &str) {
        let (mut start, mut end) = match self.buffer.selection_bounds() {
            Some(bounds) => bounds,
            None => {
                let iter = self.buffer.iter_at_offset(self.buffer.cursor_position());
                (iter, iter)
            }
        };
        let selected = self.buffer.text(&start, &end, true).to_string();
        let offset = start.offset();
        self.buffer.begin_user_action();
        self.buffer.delete(&mut start, &mut end);
        let mut at = self.buffer.iter_at_offset(offset);
        self.buffer
            .insert(&mut at, &format!("{marker}{selected}{marker}"));
        self.buffer.end_user_action();
        let cursor = self
            .buffer
            .iter_at_offset(offset + marker.chars().count() as i32);
        self.buffer.place_cursor(&cursor);
        self.view.grab_focus();
    }

    // --- apariencia y comportamiento -------------------------------------

    pub fn set_font(&self, family: FontFamily, size: i32, line_spacing: f64) {
        let size = size.clamp(9, 40);
        self.css.load_from_string(&format!(
            "textview.scribe-editor {{ font-family: {}; font-size: {size}px; }}",
            family.css_stack()
        ));
        let extra = ((size as f64) * (line_spacing - 1.0)).round().max(0.0) as i32;
        self.view.set_pixels_above_lines(extra / 2);
        self.view.set_pixels_below_lines(extra / 2);
        self.view.set_pixels_inside_wrap(extra / 3);
    }

    pub fn set_column_width(&self, width: i32) {
        self.column_width.set(width.clamp(480, 1400));
        if let Some(hadj) = self.view.hadjustment() {
            let available = hadj.page_size() as i32;
            let margin = ((available - self.column_width.get()) / 2).max(MIN_MARGIN);
            self.view.set_left_margin(margin);
            self.view.set_right_margin(margin);
            set_column_margins(&self.tags, margin);
        }
    }

    pub fn set_markup_visibility(&self, visibility: MarkupVisibility) {
        let mut config = self.decoration.get();
        config.markup = visibility;
        self.decoration.set(config);
        self.refresh();
    }

    pub fn set_focus_mode(&self, enabled: bool) {
        let mut config = self.decoration.get();
        config.focus_mode = enabled;
        self.decoration.set(config);
        self.refresh();
    }

    pub fn set_typewriter_mode(&self, enabled: bool) {
        self.typewriter.set(enabled);
        if enabled {
            self.view
                .scroll_to_mark(&self.buffer.get_insert(), 0.0, true, 0.0, 0.5);
        }
    }

    pub fn set_continue_lists(&self, enabled: bool) {
        self.continue_lists.set(enabled);
    }

    /// Realinea todas las tablas del documento en el fuente. Es una sola
    /// accion de usuario, asi que se deshace de golpe con Ctrl+Z.
    pub fn format_tables(&self) -> bool {
        let text = self.text();
        let Some(formatted) = crate::markdown_render::format_tables(&text) else {
            return false;
        };
        let offset = self.buffer.cursor_position();
        self.buffer.begin_user_action();
        let (mut start, mut end) = (self.buffer.start_iter(), self.buffer.end_iter());
        self.buffer.delete(&mut start, &mut end);
        let mut at = self.buffer.start_iter();
        self.buffer.insert(&mut at, &formatted);
        self.buffer.end_user_action();
        let restored = self
            .buffer
            .iter_at_offset(offset.min(self.buffer.end_iter().offset()));
        self.buffer.place_cursor(&restored);
        true
    }

    pub fn set_tab_width(&self, width: i32) {
        let width = width.clamp(2, 8) as u32;
        self.view.set_tab_width(width);
        self.view.set_indent_width(width as i32);
    }
}
