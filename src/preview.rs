use gtk4::pango;
use gtk4::prelude::*;
use libadwaita as adw;
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// Panel de previsualización.
///
/// Renderiza el Markdown sobre un GtkTextView usando GtkTextTags.
/// (Antes se metía HTML en un GtkLabel::set_markup, que espera markup Pango:
/// pango_parse_markup fallaba con "Unknown tag 'style'" y el panel salía vacío.)
pub struct PreviewPanel {
    pub widget: gtk4::ScrolledWindow,
    view: gtk4::TextView,
    buffer: gtk4::TextBuffer,
    tags: gtk4::TextTagTable,
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
        0 | 1 => "list1",
        2 => "list2",
        _ => "list3",
    }
}

#[derive(Default)]
struct Doc {
    text: String,
    chars: i32,
    spans: Vec<(i32, i32, &'static str)>,
    open: Vec<(&'static str, i32)>,
    list_stack: Vec<Option<u64>>,
    in_code_block: bool,
    suppress_gap: bool,
}

impl Doc {
    fn push(&mut self, s: &str) {
        self.text.push_str(s);
        self.chars += s.chars().count() as i32;
    }

    fn open_tag(&mut self, name: &'static str) {
        self.open.push((name, self.chars));
    }

    fn close_tag(&mut self, name: &'static str) {
        if let Some(pos) = self.open.iter().rposition(|(n, _)| *n == name) {
            let (n, start) = self.open.remove(pos);
            if start < self.chars {
                self.spans.push((start, self.chars, n));
            }
        }
    }

    fn block_gap(&mut self) {
        if self.suppress_gap {
            self.suppress_gap = false;
            return;
        }
        if !self.text.is_empty() && !self.text.ends_with('\n') {
            self.push("\n");
        }
    }
}

fn render(markdown: &str) -> (String, Vec<(i32, i32, &'static str)>) {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_FOOTNOTES);

    let mut d = Doc::default();

    for event in Parser::new_ext(markdown, opts) {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => d.block_gap(),
                Tag::Heading { level, .. } => {
                    d.block_gap();
                    d.open_tag(heading_tag(level));
                }
                Tag::BlockQuote(_) => {
                    d.block_gap();
                    d.open_tag("quote");
                }
                Tag::CodeBlock(kind) => {
                    d.block_gap();
                    if let CodeBlockKind::Fenced(lang) = &kind {
                        if !lang.is_empty() {
                            d.open_tag("dim");
                            d.push(&format!("{}\n", lang));
                            d.close_tag("dim");
                        }
                    }
                    d.open_tag("codeblock");
                    d.in_code_block = true;
                }
                Tag::List(start) => {
                    d.block_gap();
                    d.list_stack.push(start);
                }
                Tag::Item => {
                    d.block_gap();
                    let depth = d.list_stack.len();
                    d.open_tag(list_tag(depth));
                    let marker = match d.list_stack.last_mut() {
                        Some(Some(n)) => {
                            let m = format!("{}. ", n);
                            *n += 1;
                            m
                        }
                        _ => "• ".to_string(),
                    };
                    d.open_tag("dim");
                    d.push(&marker);
                    d.close_tag("dim");
                    d.suppress_gap = true;
                }
                Tag::Emphasis => d.open_tag("italic"),
                Tag::Strong => d.open_tag("bold"),
                Tag::Strikethrough => d.open_tag("strike"),
                Tag::Link { .. } => d.open_tag("link"),
                Tag::Image { .. } => {
                    d.open_tag("dim");
                    d.push("[imagen: ");
                }
                Tag::Table(_) => d.block_gap(),
                Tag::TableHead => d.open_tag("bold"),
                _ => {}
            },

            Event::End(tag) => match tag {
                TagEnd::Paragraph => d.push("\n"),
                TagEnd::Heading(level) => {
                    d.close_tag(heading_tag(level));
                    d.push("\n");
                }
                TagEnd::BlockQuote(_) => {
                    d.close_tag("quote");
                    d.push("\n");
                }
                TagEnd::CodeBlock => {
                    d.in_code_block = false;
                    d.close_tag("codeblock");
                    if !d.text.ends_with('\n') {
                        d.push("\n");
                    }
                }
                TagEnd::List(_) => {
                    d.list_stack.pop();
                }
                TagEnd::Item => {
                    let depth = d.list_stack.len();
                    d.close_tag(list_tag(depth));
                    if !d.text.ends_with('\n') {
                        d.push("\n");
                    }
                    d.suppress_gap = true;
                }
                TagEnd::Emphasis => d.close_tag("italic"),
                TagEnd::Strong => d.close_tag("bold"),
                TagEnd::Strikethrough => d.close_tag("strike"),
                TagEnd::Link => d.close_tag("link"),
                TagEnd::Image => {
                    d.push("]");
                    d.close_tag("dim");
                }
                TagEnd::TableHead => {
                    d.close_tag("bold");
                    d.push("\n");
                }
                TagEnd::TableRow => d.push("\n"),
                TagEnd::TableCell => d.push("\t"),
                _ => {}
            },

            Event::Text(t) => d.push(&t),
            Event::Code(t) => {
                d.open_tag("code");
                d.push(&t);
                d.close_tag("code");
            }
            Event::InlineMath(t) | Event::DisplayMath(t) => {
                d.open_tag("code");
                d.push(&t);
                d.close_tag("code");
            }
            Event::SoftBreak => {
                if d.in_code_block {
                    d.push("\n");
                } else {
                    d.push(" ");
                }
            }
            Event::HardBreak => d.push("\n"),
            Event::Rule => {
                d.block_gap();
                d.open_tag("dim");
                d.push("────────────────────");
                d.close_tag("dim");
                d.push("\n");
            }
            Event::TaskListMarker(done) => {
                d.push(if done { "☑ " } else { "☐ " });
            }
            Event::FootnoteReference(name) => {
                d.open_tag("dim");
                d.push(&format!("[{}]", name));
                d.close_tag("dim");
            }
            // El HTML embebido se ignora a propósito: no hay motor HTML aquí.
            Event::Html(_) | Event::InlineHtml(_) => {}
        }
    }

    // Cierra lo que se haya quedado abierto por Markdown malformado.
    while let Some((n, start)) = d.open.pop() {
        if start < d.chars {
            d.spans.push((start, d.chars, n));
        }
    }

    (std::mem::take(&mut d.text), std::mem::take(&mut d.spans))
}

impl PreviewPanel {
    pub fn new() -> Self {
        let tags = gtk4::TextTagTable::new();

        let add = |t: gtk4::TextTag| {
            tags.add(&t);
        };

        for (name, scale, above, below) in [
            ("h1", 1.9_f64, 20, 8),
            ("h2", 1.5, 18, 6),
            ("h3", 1.25, 16, 5),
            ("h4", 1.1, 14, 4),
            ("h5", 1.0, 12, 4),
            ("h6", 1.0, 12, 4),
        ] {
            add(gtk4::TextTag::builder()
                .name(name)
                .scale(scale)
                .weight(if name == "h1" { 800 } else { 700 })
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
            .name("codeblock")
            .family("monospace")
            .scale(0.92)
            .left_margin(36)
            .pixels_above_lines(6)
            .pixels_below_lines(6)
            .build());
        add(gtk4::TextTag::builder()
            .name("quote")
            .style(pango::Style::Italic)
            .left_margin(36)
            .build());
        add(gtk4::TextTag::builder()
            .name("link")
            .underline(pango::Underline::Single)
            .build());
        add(gtk4::TextTag::builder().name("dim").build());
        add(gtk4::TextTag::builder()
            .name("list1")
            .left_margin(28)
            .indent(-14)
            .build());
        add(gtk4::TextTag::builder()
            .name("list2")
            .left_margin(56)
            .indent(-14)
            .build());
        add(gtk4::TextTag::builder()
            .name("list3")
            .left_margin(84)
            .indent(-14)
            .build());

        let buffer = gtk4::TextBuffer::new(Some(&tags));

        let view = gtk4::TextView::builder()
            .buffer(&buffer)
            .editable(false)
            .cursor_visible(false)
            .wrap_mode(gtk4::WrapMode::WordChar)
            .left_margin(28)
            .right_margin(28)
            .top_margin(24)
            .bottom_margin(24)
            .pixels_above_lines(6)
            .pixels_inside_wrap(2)
            .build();

        let scrolled = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .child(&view)
            .build();

        let panel = Self {
            widget: scrolled,
            view,
            buffer,
            tags,
        };
        panel.apply_theme();

        // Los colores de un GtkTextTag son fijos, así que hay que repintarlos
        // cuando el usuario cambia entre claro y oscuro.
        let tags_clone = panel.tags.clone();
        adw::StyleManager::default().connect_dark_notify(move |sm| {
            apply_theme_to(&tags_clone, sm.is_dark());
        });

        panel
    }

    fn apply_theme(&self) {
        apply_theme_to(&self.tags, adw::StyleManager::default().is_dark());
    }

    pub fn update(&self, markdown: &str) {
        let (text, spans) = render(markdown);
        self.buffer.set_text(&text);
        for (start, end, name) in spans {
            let s = self.buffer.iter_at_offset(start);
            let e = self.buffer.iter_at_offset(end);
            self.buffer.apply_tag_by_name(name, &s, &e);
        }
    }

    pub fn set_font_size(&self, size: i32) {
        // Escala relativa al tamaño base del tema.
        self.view
            .set_pixels_above_lines((size as f32 * 0.4).round() as i32);
    }
}

fn apply_theme_to(tags: &gtk4::TextTagTable, dark: bool) {
    let set = |name: &str, fg: Option<&str>, bg: Option<&str>| {
        if let Some(tag) = tags.lookup(name) {
            tag.set_foreground(fg);
            tag.set_background(bg);
        }
    };

    if dark {
        set("code", Some("#f6c177"), Some("#3a3a3a"));
        set("codeblock", None, Some("#2c2c2c"));
        set("quote", Some("#b5b5b5"), None);
        set("link", Some("#78aeed"), None);
        set("dim", Some("#9a9a9a"), None);
    } else {
        set("code", Some("#a04000"), Some("#f2f0ef"));
        set("codeblock", None, Some("#f6f5f4"));
        set("quote", Some("#5e5c64"), None);
        set("link", Some("#1c71d8"), None);
        set("dim", Some("#77767b"), None);
    }
}
