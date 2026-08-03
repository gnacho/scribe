use gtk4::prelude::*;
use gtk4::gio;
use libadwaita::prelude::*;
use webkit6::prelude::*;
use libadwaita as adw;
use std::rc::Rc;
use std::cell::RefCell;

use crate::editor::EditorBridge;
use crate::sidebar::Sidebar;

const EDITOR_HTML: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/editor/index.html"));
const SOURCE_HTML: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/editor/source.html"));

pub struct ScribeWindow {
    pub window: adw::ApplicationWindow,
    bridge: Rc<EditorBridge>,
    webview: webkit6::WebView,
    sidebar: Rc<Sidebar>,
    is_source_mode: Rc<RefCell<bool>>,
    search_bar: gtk4::SearchBar,
    search_entry: gtk4::SearchEntry,
    info_popover: gtk4::Popover,
    word_label: gtk4::Label,
    line_label: gtk4::Label,
    char_label: gtk4::Label,
}

impl ScribeWindow {
    pub fn new(app: &adw::Application) -> Self {
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .default_width(1100)
            .default_height(800)
            .build();

        // === SIDEBAR ===
        let sidebar = Rc::new(Sidebar::new());
        sidebar.add_note("Bienvenida.md", "~/Documentos");
        sidebar.add_note("Proyecto.md", "~/Documentos");
        sidebar.add_note("Ideas.md", "~/Notas");

        // === WEBVIEW ===
        let webview = webkit6::WebView::new();
        webview.set_hexpand(true);
        webview.set_vexpand(true);
        webview.set_background_color(&gdk4::RGBA::new(0.0, 0.0, 0.0, 0.0));

        let settings = webkit6::prelude::WebViewExt::settings(&webview).expect("WebView debe tener settings");
        settings.set_enable_javascript(true);
        settings.set_enable_developer_extras(true);
        settings.set_javascript_can_access_clipboard(true);

        webview.load_html(EDITOR_HTML, Some("file:///"));

        let bridge = EditorBridge::new(&webview);

        // === SEARCH BAR ===
        let search_entry = gtk4::SearchEntry::builder()
            .placeholder_text("Buscar en el documento...")
            .width_request(300)
            .build();
        let search_bar = gtk4::SearchBar::builder()
            .child(&search_entry)
            .search_mode_enabled(false)
            .build();
        search_bar.set_key_capture_widget(Some(&window));

        // === INFO POPOVER (words/lines/chars) ===
        let word_label = gtk4::Label::new(Some("Palabras: 0"));
        word_label.set_halign(gtk4::Align::Start);
        let line_label = gtk4::Label::new(Some("Líneas: 0"));
        line_label.set_halign(gtk4::Align::Start);
        let char_label = gtk4::Label::new(Some("Caracteres: 0"));
        char_label.set_halign(gtk4::Align::Start);

        let info_box = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
        info_box.set_margin_top(12);
        info_box.set_margin_bottom(12);
        info_box.set_margin_start(12);
        info_box.set_margin_end(12);
        info_box.append(&word_label);
        info_box.append(&line_label);
        info_box.append(&char_label);

        let info_popover = gtk4::Popover::builder()
            .child(&info_box)
            .build();

        // === HEADERBAR ===
        let header = adw::HeaderBar::new();

        // Menu button (hamburger)
        let menu = gio::Menu::new();
        menu.append(Some("Nueva ventana"), Some("app.new-window"));
        menu.append(Some("Abrir..."), Some("win.open"));
        menu.append(Some("Guardar"), Some("win.save"));
        menu.append(Some("Guardar como..."), Some("win.save-as"));
        menu.append(None, None); // separator
        menu.append(Some("Modo foco"), Some("win.focus-mode"));
        menu.append(Some("Mostrar barra lateral"), Some("win.toggle-sidebar"));
        menu.append(None, None);
        menu.append(Some("Preferencias"), Some("win.preferences"));
        menu.append(Some("Atajos de teclado"), Some("win.show-help-overlay"));
        menu.append(Some("Ayuda"), Some("win.help"));
        menu.append(Some("Acerca de Scribe"), Some("win.about"));
        menu.append(None, None);
        menu.append(Some("Salir"), Some("app.quit"));

        let menu_button = gtk4::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .menu_model(&menu)
            .primary(true)
            .build();
        header.pack_start(&menu_button);

        // Title
        let title_widget = adw::WindowTitle::new("Sin título", "Scribe");
        header.set_title_widget(Some(&title_widget));

        // Code toggle
        let code_toggle = gtk4::ToggleButton::builder()
            .icon_name("code-symbolic")
            .tooltip_text("Mostrar código fuente (Ctrl+Shift+C)")
            .build();
        header.pack_end(&code_toggle);

        // Info button
        let info_button = gtk4::MenuButton::builder()
            .icon_name("info-symbolic")
            .tooltip_text("Estadísticas del documento")
            .popover(&info_popover)
            .build();
        header.pack_end(&info_button);

        // Save button
        let save_btn = gtk4::Button::from_icon_name("document-save-symbolic");
        save_btn.set_tooltip_text(Some("Guardar (Ctrl+S)"));
        header.pack_end(&save_btn);

        // Search button
        let search_btn = gtk4::ToggleButton::builder()
            .icon_name("system-search-symbolic")
            .tooltip_text("Buscar (Ctrl+F)")
            .build();
        header.pack_end(&search_btn);

        // === CONTENT LAYOUT ===
        let content_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        content_box.append(&search_bar);
        content_box.append(&webview);

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&content_box));

        let content_page = adw::NavigationPage::builder()
            .child(&toolbar_view)
            .title("Editor")
            .build();

        // === SPLIT VIEW ===
        let split_view = adw::NavigationSplitView::builder()
            .sidebar(&sidebar.page)
            .content(&content_page)
            .show_content(true)
            .build();

        window.set_content(Some(&split_view));

        // === STATE ===
        let is_source_mode = Rc::new(RefCell::new(false));

        // === ACTIONS ===
        let action_open = gio::SimpleAction::new("open", None);
        let action_save = gio::SimpleAction::new("save", None);
        let action_save_as = gio::SimpleAction::new("save-as", None);
        let action_focus_mode = gio::SimpleAction::new("focus-mode", None);
        let action_toggle_sidebar = gio::SimpleAction::new("toggle-sidebar", None);
        let action_preferences = gio::SimpleAction::new("preferences", None);
        let action_help = gio::SimpleAction::new("help", None);
        let action_about = gio::SimpleAction::new("about", None);

        window.add_action(&action_open);
        window.add_action(&action_save);
        window.add_action(&action_save_as);
        window.add_action(&action_focus_mode);
        window.add_action(&action_toggle_sidebar);
        window.add_action(&action_preferences);
        window.add_action(&action_help);
        window.add_action(&action_about);

        // === CONNECT SIGNALS ===
        let bridge_clone = bridge.clone();
        save_btn.connect_clicked(move |_| {
            bridge_clone.request_save();
        });

        let bridge_clone = bridge.clone();
        action_save.connect_activate(move |_, _| {
            bridge_clone.request_save();
        });

        let webview_clone = webview.clone();
        let is_source_clone = is_source_mode.clone();
        let bridge_clone = bridge.clone();
        code_toggle.connect_clicked(move |btn| {
            let source = *is_source_clone.borrow();
            if source {
                // Switch to WYSIWYG
                webview_clone.load_html(EDITOR_HTML, Some("file:///"));
                btn.set_icon_name("code-symbolic");
                btn.set_tooltip_text(Some("Mostrar código fuente (Ctrl+Shift+C)"));
            } else {
                // Switch to source
                webview_clone.load_html(SOURCE_HTML, Some("file:///"));
                btn.set_icon_name("document-edit-symbolic");
                btn.set_tooltip_text(Some("Mostrar editor WYSIWYG (Ctrl+Shift+C)"));
            }
            *is_source_clone.borrow_mut() = !source;
        });

        let webview_clone = webview.clone();
        let is_source_clone = is_source_mode.clone();
        let code_toggle_clone = code_toggle.clone();
        let action_code = gio::SimpleAction::new("toggle-code", None);
        window.add_action(&action_code);
        action_code.connect_activate(move |_, _| {
            code_toggle_clone.set_active(!code_toggle_clone.is_active());
        });

        let search_bar_clone = search_bar.clone();
        let search_btn_clone = search_btn.clone();
        search_btn.connect_toggled(move |btn| {
            search_bar_clone.set_search_mode(btn.is_active());
        });
        search_bar.connect_notify_local(Some("search-mode-enabled"), move |bar, _| {
            let enabled = bar.property::<bool>("search-mode-enabled");
            search_btn_clone.set_active(enabled);
        });

        let search_entry_clone = search_entry.clone();
        let webview_clone = webview.clone();
        search_entry.connect_search_changed(move |entry| {
            let text = entry.text();
            if !text.is_empty() {
                let js = format!(r#"window.find("{}", false, false, true, false, true, false);"#, text);
                webview_clone.evaluate_javascript(&js, None, None, None::<&gtk4::gio::Cancellable>, |_| {});
            }
        });

        let title_widget_clone = title_widget.clone();
        bridge.connect_title_changed(move |title| {
            title_widget_clone.set_title(title);
        });

        let word_label_clone = word_label.clone();
        let line_label_clone = line_label.clone();
        let char_label_clone = char_label.clone();
        bridge.connect_stats_changed(move |words, lines, chars| {
            word_label_clone.set_text(&format!("Palabras: {}", words));
            line_label_clone.set_text(&format!("Líneas: {}", lines));
            char_label_clone.set_text(&format!("Caracteres: {}", chars));
        });

        let bridge_clone = bridge.clone();
        action_open.connect_activate(move |_, _| {
            // TODO: GtkFileDialog
            println!("Open file dialog");
        });

        let bridge_clone = bridge.clone();
        action_save_as.connect_activate(move |_, _| {
            // TODO: GtkFileDialog
            println!("Save as dialog");
        });

        let header_clone = header.clone();
        let toolbar_view_clone = toolbar_view.clone();
        let action_focus = action_focus_mode.clone();
        action_focus_mode.connect_activate(move |_, _| {
            let is_focus = header_clone.is_visible();
            header_clone.set_visible(!is_focus);
            toolbar_view_clone.set_top_bar_style(if is_focus {
                adw::ToolbarStyle::Flat
            } else {
                adw::ToolbarStyle::Raised
            });
        });

        let split_view_clone = split_view.clone();
        let sidebar_clone = sidebar.clone();
        action_toggle_sidebar.connect_activate(move |_, _| {
            // Toggle sidebar visibility
            let current = split_view_clone.shows_content();
            split_view_clone.set_show_content(!current);
        });

        let window_clone = window.clone();
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
                .subtitle("Cantarell 15px")
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
                .active(true)
                .build();
            group.add(&autosave_row);

            page.add(&group);
            prefs.add(&page);
            prefs.present();
        });

        let window_clone = window.clone();
        action_about.connect_activate(move |_, _| {
            let about = adw::AboutWindow::builder()
                .transient_for(&window_clone)
                .application_name("Scribe")
                .application_icon("app.scribe.Scribe")
                .developer_name("Tu nombre")
                .version("0.1.0")
                .website("https://github.com/gnacho/scribe")
                .issue_url("https://github.com/gnacho/scribe/issues")
                .license_type(gtk4::License::Gpl30)
                .build();
            about.present();
        });

        // Keyboard shortcuts
        let bridge_clone = bridge.clone();
        let controller = gtk4::EventControllerKey::new();
        controller.connect_key_pressed(move |_ctrl, keyval, _keycode, state| {
            if state.contains(gdk4::ModifierType::CONTROL_MASK) {
                match keyval {
                    gdk4::Key::b => {
                        bridge_clone.exec_command("toggleBold");
                        return glib::Propagation::Stop;
                    }
                    gdk4::Key::i => {
                        bridge_clone.exec_command("toggleItalic");
                        return glib::Propagation::Stop;
                    }
                    gdk4::Key::k => {
                        bridge_clone.exec_command("toggleCode");
                        return glib::Propagation::Stop;
                    }
                    gdk4::Key::f => {
                        search_btn.set_active(!search_btn.is_active());
                        return glib::Propagation::Stop;
                    }
                    gdk4::Key::s => {
                        bridge_clone.request_save();
                        return glib::Propagation::Stop;
                    }
                    _ => {}
                }
            }
            if state.contains(gdk4::ModifierType::CONTROL_MASK | gdk4::ModifierType::SHIFT_MASK) {
                if keyval == gdk4::Key::c {
                    code_toggle.set_active(!code_toggle.is_active());
                    return glib::Propagation::Stop;
                }
            }
            glib::Propagation::Proceed
        });
        window.add_controller(controller);

        Self {
            window,
            bridge,
            webview,
            sidebar,
            is_source_mode,
            search_bar,
            search_entry,
            info_popover,
            word_label,
            line_label,
            char_label,
        }
    }

    pub fn present(&self) {
        self.window.present();
    }
}
