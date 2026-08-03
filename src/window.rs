use gtk4::prelude::*;
use libadwaita::prelude::*;
use libadwaita as adw;
use std::rc::Rc;
use std::cell::RefCell;
use std::path::PathBuf;

use crate::editor::Editor;
use crate::preview::PreviewPanel;
use crate::settings::AppSettings;
use crate::file_manager::FileManager;

pub struct ScribeWindow {
    pub window: adw::ApplicationWindow,
}

impl ScribeWindow {
    pub fn new(app: &adw::Application, settings: &AppSettings) -> Self {
        let settings_rc = Rc::new(AppSettings::new());
        let file_manager = Rc::new(FileManager::new());
        let current_file = Rc::new(RefCell::new(None::<PathBuf>));
        let is_modified = Rc::new(RefCell::new(false));

        // === WINDOW ===
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .default_width(settings_rc.window_width())
            .default_height(settings_rc.window_height())
            .build();

        // === EDITOR ===
        let editor = Rc::new(Editor::new());
        editor.widget.set_hexpand(true);
        editor.widget.set_vexpand(true);

        // === PREVIEW ===
        let preview = Rc::new(PreviewPanel::new());

        // === SIDEBAR ===
        let sidebar_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        sidebar_box.set_width_request(260);
        sidebar_box.add_css_class("sidebar");

        let sidebar_header = adw::HeaderBar::new();
        sidebar_header.set_title_widget(Some(&adw::WindowTitle::new("Notas", "")));
        sidebar_header.add_css_class("flat");

        let search_entry = gtk4::SearchEntry::builder()
            .placeholder_text("Buscar notas...")
            .margin_top(12)
            .margin_bottom(6)
            .margin_start(12)
            .margin_end(12)
            .build();

        let notes_label = gtk4::Label::new(Some("Notas recientes"));
        notes_label.add_css_class("caption-heading");
        notes_label.set_halign(gtk4::Align::Start);
        notes_label.set_margin_start(12);
        notes_label.set_margin_top(12);

        let notes_list = gtk4::ListBox::builder()
            .selection_mode(gtk4::SelectionMode::Single)
            .css_classes(vec!["navigation-sidebar".to_string()])
            .build();

        let toc_label = gtk4::Label::new(Some("Contenido"));
        toc_label.add_css_class("caption-heading");
        toc_label.set_halign(gtk4::Align::Start);
        toc_label.set_margin_start(12);
        toc_label.set_margin_top(12);

        let toc_list = gtk4::ListBox::builder()
            .selection_mode(gtk4::SelectionMode::None)
            .css_classes(vec!["navigation-sidebar".to_string()])
            .build();

        for (title, subtitle) in [("Bienvenida.md", "~/Documentos"), ("Proyecto.md", "~/Documentos"), ("Ideas.md", "~/Notas")] {
            let row = adw::ActionRow::builder()
                .title(title)
                .subtitle(subtitle)
                .build();
            notes_list.append(&row);
        }

        let sidebar_content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        sidebar_content.append(&search_entry);
        sidebar_content.append(&notes_label);
        sidebar_content.append(&notes_list);
        sidebar_content.append(&toc_label);
        sidebar_content.append(&toc_list);

        let sidebar_scrolled = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .child(&sidebar_content)
            .build();

        sidebar_box.append(&sidebar_header);
        sidebar_box.append(&sidebar_scrolled);

        // === CENTER PANED (Editor + Preview) ===
        let paned = gtk4::Paned::new(gtk4::Orientation::Horizontal);
        paned.set_start_child(Some(&editor.widget));
        paned.set_end_child(Some(&preview.widget));
        paned.set_wide_handle(true);
        paned.set_position(600);
        paned.set_hexpand(true);
        paned.set_vexpand(true);

        // Show/hide preview based on settings
        if !settings_rc.show_preview() {
            paned.set_end_child(None::<&gtk4::Widget>);
        }

        // === HEADERBAR (GNOME Text Editor style) ===
        let header = adw::HeaderBar::new();

        // Left: New tab / Open
        let new_tab_btn = gtk4::Button::from_icon_name("tab-new-symbolic");
        new_tab_btn.set_tooltip_text(Some("Nueva pestaña (Ctrl+T)"));
        header.pack_start(&new_tab_btn);

        let open_btn = gtk4::Button::from_icon_name("document-open-symbolic");
        open_btn.set_tooltip_text(Some("Abrir (Ctrl+O)"));
        header.pack_start(&open_btn);

        // Center: Title
        let title_widget = adw::WindowTitle::new("Sin título", "Scribe");
        header.set_title_widget(Some(&title_widget));

        // Right: Preview toggle / Info / Menu
        let preview_btn = gtk4::ToggleButton::builder()
            .icon_name("document-preview-symbolic")
            .tooltip_text("Mostrar previsualización (Ctrl+Shift+P)")
            .active(settings_rc.show_preview())
            .build();
        header.pack_end(&preview_btn);

        let info_btn = gtk4::Button::from_icon_name("info-symbolic");
        info_btn.set_tooltip_text(Some("Estadísticas"));
        header.pack_end(&info_btn);

        let menu_btn = gtk4::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .primary(true)
            .build();
        header.pack_end(&menu_btn);

        // === MENU ===
        let menu = gio::Menu::new();
        menu.append(Some("Nueva ventana"), Some("app.new-window"));
        menu.append(Some("Abrir..."), Some("win.open"));
        menu.append(Some("Guardar"), Some("win.save"));
        menu.append(Some("Guardar como..."), Some("win.save-as"));
        menu.append(None, None);
        menu.append(Some("Mostrar barra lateral"), Some("win.toggle-sidebar"));
        menu.append(Some("Mostrar previsualización"), Some("win.toggle-preview"));
        menu.append(None, None);
        menu.append(Some("Preferencias"), Some("win.preferences"));
        menu.append(Some("Atajos de teclado"), Some("win.show-help-overlay"));
        menu.append(Some("Acerca de Scribe"), Some("win.about"));
        menu.append(None, None);
        menu.append(Some("Salir"), Some("app.quit"));
        menu_btn.set_menu_model(Some(&menu));

        // === CONTENT ===
        let content_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        content_box.append(&paned);

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&content_box));

        // === OVERLAY SPLIT (Sidebar + Content) ===
        let overlay_split = adw::OverlaySplitView::builder()
            .sidebar(&sidebar_box)
            .content(&toolbar_view)
            .show_sidebar(settings_rc.show_sidebar())
            .pin_sidebar(false)
            .enable_hide_gesture(true)
            .build();

        window.set_content(Some(&overlay_split));

        // === ACTIONS ===
        let action_open = gio::SimpleAction::new("open", None);
        let action_save = gio::SimpleAction::new("save", None);
        let action_save_as = gio::SimpleAction::new("save-as", None);
        let action_toggle_sidebar = gio::SimpleAction::new("toggle-sidebar", None);
        let action_toggle_preview = gio::SimpleAction::new("toggle-preview", None);
        let action_preferences = gio::SimpleAction::new("preferences", None);
        let action_about = gio::SimpleAction::new("about", None);

        window.add_action(&action_open);
        window.add_action(&action_save);
        window.add_action(&action_save_as);
        window.add_action(&action_toggle_sidebar);
        window.add_action(&action_toggle_preview);
        window.add_action(&action_preferences);
        window.add_action(&action_about);

        // === CONNECT: Open ===
        let window_clone = window.clone();
        let file_manager_clone = file_manager.clone();
        let current_file_clone = current_file.clone();
        let editor_clone = editor.clone();
        let title_widget_clone = title_widget.clone();
        let is_modified_clone = is_modified.clone();
        action_open.connect_activate(move |_, _| {
            let editor_inner = editor_clone.clone();
            let current_file_inner = current_file_clone.clone();
            let title_widget_inner = title_widget_clone.clone();
            let is_modified_inner = is_modified_clone.clone();
            file_manager_clone.open(&window_clone, move |result| {
                if let Some((path, content)) = result {
                    *current_file_inner.borrow_mut() = Some(path.clone());
                    editor_inner.set_text(&content);
                    let title = path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("Sin título");
                    title_widget_inner.set_title(title);
                    *is_modified_inner.borrow_mut() = false;
                }
            });
        });

        open_btn.connect_clicked(glib::clone!(@weak action_open => move |_| {
            action_open.activate(None);
        }));

        // === CONNECT: Save ===
        let window_clone = window.clone();
        let file_manager_clone = file_manager.clone();
        let current_file_clone = current_file.clone();
        let editor_clone = editor.clone();
        let title_widget_clone = title_widget.clone();
        let is_modified_clone = is_modified.clone();
        action_save.connect_activate(move |_, _| {
            let content = editor_clone.text();
            let path = current_file_clone.borrow().clone();
            let current_file_inner = current_file_clone.clone();
            let title_widget_inner = title_widget_clone.clone();
            let is_modified_inner = is_modified_clone.clone();
            file_manager_clone.save(&window_clone, path.as_ref(), &content, move |saved_path| {
                if let Some(p) = saved_path {
                    *current_file_inner.borrow_mut() = Some(p.clone());
                    let title = p.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("Sin título");
                    title_widget_inner.set_title(title);
                    *is_modified_inner.borrow_mut() = false;
                }
            });
        });

        // === CONNECT: Save As ===
        let window_clone = window.clone();
        let file_manager_clone = file_manager.clone();
        let current_file_clone = current_file.clone();
        let editor_clone = editor.clone();
        let title_widget_clone = title_widget.clone();
        let is_modified_clone = is_modified.clone();
        action_save_as.connect_activate(move |_, _| {
            let content = editor_clone.text();
            let current_file_inner = current_file_clone.clone();
            let title_widget_inner = title_widget_clone.clone();
            let is_modified_inner = is_modified_clone.clone();
            file_manager_clone.save(&window_clone, None, &content, move |saved_path| {
                if let Some(p) = saved_path {
                    *current_file_inner.borrow_mut() = Some(p.clone());
                    let title = p.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("Sin título");
                    title_widget_inner.set_title(title);
                    *is_modified_inner.borrow_mut() = false;
                }
            });
        });

        // === CONNECT: Editor changed → update preview & stats ===
        let preview_clone = preview.clone();
        let toc_list_clone = toc_list.clone();
        let title_widget_clone = title_widget.clone();
        let is_modified_clone = is_modified.clone();
        editor.connect_changed(move |text| {
            preview_clone.update(text);
            *is_modified_clone.borrow_mut() = true;

            // Update title from first heading
            let lines = text.lines();
            let mut title = "Sin título";
            for line in lines {
                if let Some(t) = line.strip_prefix("# ") {
                    title = t.trim();
                    break;
                }
            }
            title_widget_clone.set_title(title);

            // Update TOC
            while let Some(child) = toc_list_clone.first_child() {
                toc_list_clone.remove(&child);
            }
            for line in text.lines() {
                if let Some(rest) = line.strip_prefix("# ") {
                    let label = gtk4::Label::new(Some(rest.trim()));
                    label.set_halign(gtk4::Align::Start);
                    label.set_margin_start(12);
                    label.set_margin_top(2);
                    label.set_margin_bottom(2);
                    label.add_css_class("heading");
                    toc_list_clone.append(&label);
                } else if let Some(rest) = line.strip_prefix("## ") {
                    let label = gtk4::Label::new(Some(rest.trim()));
                    label.set_halign(gtk4::Align::Start);
                    label.set_margin_start(24);
                    label.set_margin_top(2);
                    label.set_margin_bottom(2);
                    toc_list_clone.append(&label);
                } else if let Some(rest) = line.strip_prefix("### ") {
                    let label = gtk4::Label::new(Some(rest.trim()));
                    label.set_halign(gtk4::Align::Start);
                    label.set_margin_start(36);
                    label.set_margin_top(2);
                    label.set_margin_bottom(2);
                    toc_list_clone.append(&label);
                }
            }
        });

        // === CONNECT: Toggle sidebar ===
        let overlay_split_clone = overlay_split.clone();
        let settings_clone = settings_rc.clone();
        action_toggle_sidebar.connect_activate(move |_, _| {
            let current = overlay_split_clone.shows_sidebar();
            overlay_split_clone.set_show_sidebar(!current);
            let _ = settings_clone.set_show_sidebar(!current);
        });

        // === CONNECT: Toggle preview ===
        let paned_clone = paned.clone();
        let preview_clone = preview.clone();
        let preview_btn_clone = preview_btn.clone();
        let settings_clone = settings_rc.clone();
        action_toggle_preview.connect_activate(move |_, _| {
            let active = preview_btn_clone.is_active();
            if active {
                paned_clone.set_end_child(Some(&preview_clone.widget));
            } else {
                paned_clone.set_end_child(None::<&gtk4::Widget>);
            }
            let _ = settings_clone.set_show_preview(active);
        });

        preview_btn.connect_toggled(glib::clone!(@weak action_toggle_preview => move |btn| {
            action_toggle_preview.activate(None);
        }));

        // === CONNECT: Preferences ===
        let window_clone = window.clone();
        let settings_clone = settings_rc.clone();
        action_preferences.connect_activate(move |_, _| {
            let prefs = adw::PreferencesWindow::builder()
                .transient_for(&window_clone)
                .modal(true)
                .title("Preferencias")
                .build();

            let page = adw::PreferencesPage::builder()
                .title("General")
                .icon_name("preferences-system-symbolic")
                .build();

            let group = adw::PreferencesGroup::builder()
                .title("Editor")
                .build();

            let font_row = adw::ActionRow::builder()
                .title("Fuente del editor")
                .subtitle("Monospace 15px")
                .build();
            group.add(&font_row);

            let spacing_row = adw::SpinRow::builder()
                .title("Espaciado entre líneas")
                .adjustment(&gtk4::Adjustment::new(1.7, 1.0, 3.0, 0.1, 0.1, 0.0))
                .digits(1)
                .build();
            group.add(&spacing_row);

            let autosave_row = adw::SwitchRow::builder()
                .title("Guardado automático")
                .subtitle("Guardar cada 30 segundos")
                .active(settings_clone.autosave())
                .build();
            group.add(&autosave_row);

            page.add(&group);
            prefs.add(&page);
            prefs.present();
        });

        // === CONNECT: About ===
        let window_clone = window.clone();
        action_about.connect_activate(move |_, _| {
            let about = adw::AboutWindow::builder()
                .transient_for(&window_clone)
                .application_name("Scribe")
                .application_icon("app.scribe.Scribe")
                .developer_name("Tu nombre")
                .version("1.0.0")
                .website("https://github.com/gnacho/scribe")
                .issue_url("https://github.com/gnacho/scribe/issues")
                .license_type(gtk4::License::Gpl30)
                .build();
            about.present();
        });

        // === Keyboard shortcuts ===
        let controller = gtk4::EventControllerKey::new();
        controller.connect_key_pressed(move |_ctrl, keyval, _keycode, state| {
            if state.contains(gdk4::ModifierType::CONTROL_MASK) {
                match keyval {
                    gdk4::Key::t => {
                        // New tab - placeholder
                        return glib::Propagation::Stop;
                    }
                    _ => {}
                }
            }
            glib::Propagation::Proceed
        });
        window.add_controller(controller);

        // Window size persistence
        let settings_clone = settings_rc.clone();
        window.connect_close_request(move |win| {
            let (width, height) = win.default_size();
            settings_clone.set_window_width(width);
            settings_clone.set_window_height(height);
            glib::Propagation::Proceed
        });

        Self { window }
    }

    pub fn present(&self) {
        self.window.present();
    }
}
