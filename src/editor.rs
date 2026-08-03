use gtk4::prelude::*;
use gtksourceview5::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub struct Editor {
    pub widget: gtksourceview5::View,
    buffer: gtksourceview5::Buffer,
    on_changed: Rc<RefCell<Option<Box<dyn Fn(&str)>>>>,
}

impl Editor {
    pub fn new() -> Self {
        let buffer = gtksourceview5::Buffer::new(None);
        buffer.set_highlight_matching_brackets(true);

        // Load Markdown language spec if available
        let manager = gtksourceview5::LanguageManager::default();
        if let Some(lang) = manager.language("markdown") {
            buffer.set_language(Some(&lang));
        }

        let view = gtksourceview5::View::builder()
            .buffer(&buffer)
            .wrap_mode(gtk4::WrapMode::WordChar)
            .show_line_numbers(true)
            .show_right_margin(false)
            .indent_width(4)
            .tab_width(4)
            .insert_spaces_instead_of_tabs(true)
            .smart_backspace(true)
            .build();

        view.set_monospace(true);
        view.add_css_class("view");

        let on_changed = Rc::new(RefCell::new(None::<Box<dyn Fn(&str)>>));

        let on_changed_clone = on_changed.clone();
        buffer.connect_changed(move |buf| {
            let text = buf.text(&buf.start_iter(), &buf.end_iter(), true);
            if let Some(cb) = on_changed_clone.borrow().as_ref() {
                cb(&text);
            }
        });

        Self {
            widget: view,
            buffer,
            on_changed,
        }
    }

    pub fn set_text(&self, text: &str) {
        self.buffer.set_text(text);
    }

    pub fn text(&self) -> String {
        self.buffer.text(&self.buffer.start_iter(), &self.buffer.end_iter(), true).to_string()
    }

    pub fn connect_changed<F: Fn(&str) + 'static>(&self, callback: F) {
        *self.on_changed.borrow_mut() = Some(Box::new(callback));
    }

    pub fn set_font_size(&self, size: i32) {
        let css = format!(
            "textview {{ font-size: {}px; }}",
            size
        );
        let provider = gtk4::CssProvider::new();
        provider.load_from_string(&css);
        self.widget.style_context().add_provider(&provider, gtk4::STYLE_PROVIDER_PRIORITY_USER);
    }
}
