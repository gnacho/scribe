use gtk4::pango;
use gtk4::prelude::*;
use gtksourceview5::prelude::*;
use libadwaita as adw;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use crate::markdown_render::{spans, MAX_LIVE_BYTES};

type ChangedCallback = Rc<RefCell<Option<Box<dyn Fn(&str)>>>>;

/// Ancho máximo de la columna de texto, al estilo Typora.
const COLUMN_WIDTH: i32 = 720;
const MIN_MARGIN: i32 = 24;

/// Sangría extra (en px) de cada tag de bloque respecto al margen de la columna.
const BLOCK_INDENTS: [(&str, i32); 5] = [
    ("quote", 24),
    ("codeblock", 20),
    ("li1", 26),
    ("li2", 52),
    ("li3", 78),
];

pub struct Editor {
    /// El `ScrolledWindow` es el widget que se mete en la ventana.
    /// Sin él el documento no se podía desplazar.
    pub widget: gtk4::ScrolledWindow,
    pub view: gtksourceview5::View,
    buffer: gtksourceview5::Buffer,
    tags: gtk4::TextTagTable,
    on_changed: ChangedCallback,
    css: gtk4::CssProvider,
    last_line: Rc<Cell<i32>>,
    generation: Rc<Cell<u64>>,
}

fn build_tags() -> gtk4::TextTagTable {
    let table = gtk4::TextTagTable::new();
    let add = |t: gtk4::TextTag| {
        table.add(&t);
    };

    // El orden importa: en GTK, el tag añadido más tarde tiene más prioridad.
    // Bloques primero, luego cabeceras, luego elementos en línea y al final
    // las marcas de sintaxis, que deben ganar siempre.

    for (name, indent) in BLOCK_INDENTS {
        let builder = gtk4::TextTag::builder().name(name);
        let tag = match name {
            "quote" => builder.style(pango::Style::Italic).build(),
            "codeblock" => builder.family("monospace").scale(0.92).build(),
            _ => builder.indent(-indent).build(),
        };
        add(tag);
    }

    add(gtk4::TextTag::builder()
        .name("fence")
        .family("monospace")
        .scale(0.75)
        .build());
    add(gtk4::TextTag::builder()
        .name("rule")
        .scale(0.8)
        .justification(gtk4::Justification::Center)
        .build());

    for (name, scale, weight, above, below) in [
        ("h1", 1.85_f64, 800, 26, 10),
        ("h2", 1.45, 700, 22, 8),
        ("h3", 1.2, 700, 18, 6),
        ("h4", 1.05, 700, 14, 4),
        ("h5", 1.0, 700, 12, 4),
        ("h6", 1.0, 700, 12, 4),
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
        .scale(0.92)
        .build());
    add(gtk4::TextTag::builder()
        .name("link")
        .underline(pango::Underline::Single)
        .build());
    add(gtk4::TextTag::builder()
        .name("listmarker")
        .weight(700)
        .build());
    add(gtk4::TextTag::builder()
        .name("task")
        .family("monospace")
        .weight(700)
        .build());

    // Marcas de Markdown: ocultas por defecto, atenuadas en la línea del cursor.
    add(gtk4::TextTag::builder()
        .name("syn_hidden")
        .invisible(true)
        .build());
    add(gtk4::TextTag::builder().name("syn_shown").build());

    table
}

/// GtkSourceView pinta el fondo del buffer desde su *style scheme*, no desde el
/// tema GTK: sin esto la ventana se pone oscura pero el texto se queda en claro.
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

fn apply_theme(tags: &gtk4::TextTagTable, dark: bool) {
    let set = |name: &str, fg: Option<&str>, para_bg: Option<&str>| {
        if let Some(tag) = tags.lookup(name) {
            tag.set_foreground(fg);
            tag.set_paragraph_background(para_bg);
        }
    };

    if dark {
        set("code", Some("#f0a868"), None);
        set("codeblock", Some("#e0e0e0"), Some("#343434"));
        set("fence", Some("#7c7c7c"), Some("#343434"));
        set("quote", Some("#c2c2c2"), Some("#2e2e2e"));
        set("link", Some("#82b8f0"), None);
        set("listmarker", Some("#82b8f0"), None);
        set("task", Some("#82b8f0"), None);
        set("rule", Some("#6f6f6f"), None);
        set("syn_shown", Some("#7a7a7a"), None);
    } else {
        set("code", Some("#a34a00"), None);
        set("codeblock", Some("#1f1f1f"), Some("#f4f3f2"));
        set("fence", Some("#9a9996"), Some("#f4f3f2"));
        set("quote", Some("#54535a"), Some("#f6f5f4"));
        set("link", Some("#1a6ed8"), None);
        set("listmarker", Some("#1a6ed8"), None);
        set("task", Some("#1a6ed8"), None);
        set("rule", Some("#9a9996"), None);
        set("syn_shown", Some("#a9a8a5"), None);
    }
}

/// Los tags de bloque llevan margen absoluto, así que hay que recalcularlos
/// cada vez que cambia el ancho de la columna centrada.
fn set_column_margins(tags: &gtk4::TextTagTable, base: i32) {
    for (name, extra) in BLOCK_INDENTS {
        if let Some(tag) = tags.lookup(name) {
            tag.set_left_margin(base + extra);
            tag.set_right_margin(base);
        }
    }
    for name in ["fence", "rule"] {
        if let Some(tag) = tags.lookup(name) {
            tag.set_left_margin(base + 20);
            tag.set_right_margin(base);
        }
    }
}

/// Vuelve a decorar todo el buffer. Se llama con debounce al escribir y
/// al cambiar el cursor de línea.
fn decorate(buffer: &gtksourceview5::Buffer) {
    let start = buffer.start_iter();
    let end = buffer.end_iter();
    buffer.remove_all_tags(&start, &end);

    let text = buffer.text(&start, &end, true).to_string();
    if text.is_empty() || text.len() > MAX_LIVE_BYTES {
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

    let cursor_line = buffer.iter_at_offset(buffer.cursor_position()).line();

    for span in spans(&text) {
        let (from, to) = (byte_to_char[span.start], byte_to_char[span.end]);
        if from >= to {
            continue;
        }
        let start_iter = buffer.iter_at_offset(from);
        let end_iter = buffer.iter_at_offset(to);
        let name = if span.syntax {
            if start_iter.line() <= cursor_line && end_iter.line() >= cursor_line {
                "syn_shown"
            } else {
                "syn_hidden"
            }
        } else {
            span.tag
        };
        buffer.apply_tag_by_name(name, &start_iter, &end_iter);
    }
}

impl Editor {
    pub fn new() -> Self {
        let tags = build_tags();
        let buffer = gtksourceview5::Buffer::new(Some(&tags));
        // La decoración la ponemos nosotros; el resaltado de GtkSourceView
        // pintaría por encima y duplicaría el trabajo.
        buffer.set_highlight_syntax(false);
        buffer.set_highlight_matching_brackets(false);

        let view = gtksourceview5::View::builder()
            .buffer(&buffer)
            .wrap_mode(gtk4::WrapMode::Word)
            .show_line_numbers(false)
            .show_right_margin(false)
            .highlight_current_line(false)
            .indent_width(4)
            .tab_width(4)
            .insert_spaces_instead_of_tabs(true)
            .smart_backspace(true)
            .top_margin(48)
            .bottom_margin(240)
            .left_margin(MIN_MARGIN)
            .right_margin(MIN_MARGIN)
            .build();
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
        apply_theme(&tags, dark);
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
        };
        editor.connect_signals();
        editor
    }

    fn connect_signals(&self) {
        // Columna centrada: se recalcula con el ancho real de la vista.
        if let Some(hadj) = self.view.hadjustment() {
            let view = self.view.clone();
            let tags = self.tags.clone();
            hadj.connect_page_size_notify(move |adj| {
                let width = adj.page_size() as i32;
                if width <= 0 {
                    return;
                }
                let margin = ((width - COLUMN_WIDTH) / 2).max(MIN_MARGIN);
                if view.left_margin() != margin {
                    view.set_left_margin(margin);
                    view.set_right_margin(margin);
                    set_column_margins(&tags, margin);
                }
            });
        }

        // Tema claro/oscuro: los colores de un GtkTextTag son fijos.
        let tags = self.tags.clone();
        let buffer = self.buffer.clone();
        adw::StyleManager::default().connect_dark_notify(move |sm| {
            apply_scheme(&buffer, sm.is_dark());
            apply_theme(&tags, sm.is_dark());
            decorate(&buffer);
        });

        // Al escribir: avisar y redecorar con debounce.
        let on_changed = self.on_changed.clone();
        let generation = self.generation.clone();
        let last_line = self.last_line.clone();
        self.buffer.connect_changed(move |buf| {
            let text = buf.text(&buf.start_iter(), &buf.end_iter(), true);
            if let Some(cb) = on_changed.borrow().as_ref() {
                cb(&text);
            }

            let current = generation.get().wrapping_add(1);
            generation.set(current);
            let generation = generation.clone();
            let last_line = last_line.clone();
            let buf = buf.clone();
            glib::timeout_add_local_once(Duration::from_millis(45), move || {
                if generation.get() != current {
                    return;
                }
                last_line.set(buf.iter_at_offset(buf.cursor_position()).line());
                decorate(&buf);
            });
        });

        // Al mover el cursor de línea: revelar u ocultar las marcas.
        let last_line = self.last_line.clone();
        self.buffer.connect_cursor_position_notify(move |buf| {
            let line = buf.iter_at_offset(buf.cursor_position()).line();
            if last_line.get() == line {
                return;
            }
            last_line.set(line);
            decorate(buf);
        });
    }

    pub fn set_text(&self, text: &str) {
        self.buffer.set_text(text);
        self.buffer.set_modified(false);
        let start = self.buffer.start_iter();
        self.buffer.place_cursor(&start);
        self.last_line.set(0);
        decorate(&self.buffer);
    }

    pub fn text(&self) -> String {
        self.buffer
            .text(&self.buffer.start_iter(), &self.buffer.end_iter(), true)
            .to_string()
    }

    pub fn connect_changed<F: Fn(&str) + 'static>(&self, callback: F) {
        *self.on_changed.borrow_mut() = Some(Box::new(callback));
    }

    /// Posición del cursor como (línea, columna), en base 1.
    pub fn cursor_position(&self) -> (i32, i32) {
        let iter = self.buffer.iter_at_offset(self.buffer.cursor_position());
        (iter.line() + 1, iter.line_offset() + 1)
    }

    pub fn connect_cursor_moved<F: Fn() + 'static>(&self, callback: F) {
        self.buffer
            .connect_cursor_position_notify(move |_| callback());
    }

    pub fn set_style(&self, font_size: i32, line_spacing: f64) {
        let size = font_size.clamp(9, 40);
        self.css.load_from_string(&format!(
            "textview.scribe-editor {{ \
                font-family: Cantarell, 'Adwaita Sans', 'Noto Sans', sans-serif; \
                font-size: {size}px; \
             }}"
        ));
        let extra = ((size as f64) * (line_spacing - 1.0)).round().max(0.0) as i32;
        self.view.set_pixels_above_lines(extra / 2);
        self.view.set_pixels_below_lines(extra / 2);
        self.view.set_pixels_inside_wrap(extra / 3);
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
        let cursor = self.buffer.iter_at_offset(offset + marker.len() as i32);
        self.buffer.place_cursor(&cursor);
        self.view.grab_focus();
    }

    pub fn scroll_to_line(&self, line: i32) {
        if let Some(mut iter) = self.buffer.iter_at_line(line) {
            self.buffer.place_cursor(&iter);
            self.view.scroll_to_iter(&mut iter, 0.0, true, 0.0, 0.25);
            self.view.grab_focus();
        }
    }
}
