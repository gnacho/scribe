use gtk4::prelude::*;
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;

pub struct Sidebar {
    pub page: adw::NavigationPage,
    pub toc_list: gtk4::ListBox,
    pub notes_list: gtk4::ListBox,
    pub search_entry: gtk4::SearchEntry,
    pub on_toc_clicked: Rc<RefCell<Option<Box<dyn Fn(&str)>>>>,
}

impl Sidebar {
    pub fn new() -> Self {
        // Search
        let search_entry = gtk4::SearchEntry::builder()
            .placeholder_text("Buscar notas...")
            .margin_top(12)
            .margin_bottom(6)
            .margin_start(12)
            .margin_end(12)
            .build();

        // Notes list
        let notes_list = gtk4::ListBox::builder()
            .selection_mode(gtk4::SelectionMode::Single)
            .css_classes(vec!["navigation-sidebar".to_string()])
            .build();

        // TOC list
        let toc_list = gtk4::ListBox::builder()
            .selection_mode(gtk4::SelectionMode::None)
            .css_classes(vec!["navigation-sidebar".to_string()])
            .build();

        // Sections
        let notes_label = gtk4::Label::new(Some("Notas recientes"));
        notes_label.add_css_class("caption-heading");
        notes_label.set_halign(gtk4::Align::Start);
        notes_label.set_margin_start(12);
        notes_label.set_margin_top(12);

        let toc_label = gtk4::Label::new(Some("Contenido"));
        toc_label.add_css_class("caption-heading");
        toc_label.set_halign(gtk4::Align::Start);
        toc_label.set_margin_start(12);
        toc_label.set_margin_top(12);

        let box_layout = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        box_layout.append(&search_entry);
        box_layout.append(&notes_label);
        box_layout.append(&notes_list);
        box_layout.append(&toc_label);
        box_layout.append(&toc_list);

        let scrolled = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .child(&box_layout)
            .build();

        let toolbar_view = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(&adw::WindowTitle::new("Notas", "")));
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&scrolled));

        let page = adw::NavigationPage::builder()
            .child(&toolbar_view)
            .title("Notas")
            .build();

        let sidebar = Self {
            page,
            toc_list,
            notes_list,
            search_entry,
            on_toc_clicked: Rc::new(RefCell::new(None)),
        };

        sidebar
    }

    pub fn update_toc(&self, headings: &[(u8, String)]) {
        // Clear existing
        while let Some(child) = self.toc_list.first_child() {
            self.toc_list.remove(&child);
        }

        for (level, text) in headings {
            let label = gtk4::Label::new(Some(text));
            label.set_halign(gtk4::Align::Start);
            label.set_margin_start(12 + (*level as i32 - 1) * 12);
            label.set_margin_top(4);
            label.set_margin_bottom(4);
            label.add_css_class("body");
            if *level == 1 {
                label.add_css_class("heading");
            }
            self.toc_list.append(&label);
        }
    }

    pub fn add_note(&self, title: &str, subtitle: &str) {
        let row = adw::ActionRow::builder()
            .title(title)
            .subtitle(subtitle)
            .build();
        self.notes_list.append(&row);
    }

    pub fn connect_toc_clicked<F: Fn(&str) + 'static>(&self, callback: F) {
        *self.on_toc_clicked.borrow_mut() = Some(Box::new(callback));
    }
}
