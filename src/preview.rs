use gtk4::prelude::*;
use pulldown_cmark::{Parser, Options, html};

pub struct PreviewPanel {
    pub widget: gtk4::ScrolledWindow,
    label: gtk4::Label,
}

impl PreviewPanel {
    pub fn new() -> Self {
        let label = gtk4::Label::builder()
            .wrap(true)
            .wrap_mode(gtk4::pango::WrapMode::WordChar)
            .xalign(0.0)
            .yalign(0.0)
            .margin_top(24)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .build();

        let scrolled = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .child(&label)
            .build();

        Self {
            widget: scrolled,
            label,
        }
    }

    pub fn update(&self, markdown: &str) {
        let mut options = Options::empty();
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_TASKLISTS);

        let parser = Parser::new_ext(markdown, options);
        let mut html_output = String::new();
        html::push_html(&mut html_output, parser);

        // Wrap in basic styling
        let styled = format!(
            r#"<style>
            body {{ font-family: Cantarell, sans-serif; font-size: 15px; line-height: 1.7; color: #1c1c1c; }}
            h1 {{ font-size: 2em; font-weight: 800; margin: 0.8em 0 0.4em; }}
            h2 {{ font-size: 1.5em; font-weight: 700; margin: 0.7em 0 0.35em; }}
            h3 {{ font-size: 1.25em; font-weight: 700; margin: 0.6em 0 0.3em; }}
            p {{ margin: 0.6em 0; }}
            code {{ background: #f6f6f6; padding: 2px 6px; border-radius: 4px; font-family: monospace; }}
            pre {{ background: #f0f0f0; padding: 16px; border-radius: 8px; overflow-x: auto; }}
            blockquote {{ border-left: 3px solid #1c71d8; padding-left: 16px; margin: 1em 0; color: #5e5e5e; font-style: italic; }}
            ul, ol {{ padding-left: 1.5em; }}
            li {{ margin: 0.25em 0; }}
            a {{ color: #1c71d8; text-decoration: none; }}
            img {{ max-width: 100%; border-radius: 8px; }}
            table {{ width: 100%; border-collapse: collapse; margin: 1em 0; }}
            th, td {{ border: 1px solid rgba(0,0,0,0.08); padding: 8px 12px; text-align: left; }}
            th {{ background: #f0f0f0; font-weight: 600; }}
            </style>
            <div style="max-width: 720px; margin: 0 auto;">{}</div>"#,
            html_output
        );

        self.label.set_markup(&styled);
    }
}
