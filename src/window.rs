use gtk4::prelude::*;
use gtk4::gio;
use libadwaita::prelude::*;
use webkit6::prelude::*;
use libadwaita as adw;
use std::rc::Rc;
use std::cell::RefCell;
use std::path::PathBuf;

use crate::editor::EditorBridge;
use crate::sidebar::Sidebar;
use crate::settings::AppSettings;
use crate::file_manager::FileManager;

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
    settings: Rc<AppSettings>,
    file_manager: Rc<FileManager>,
    current_file: Rc<RefCell<Option<PathBuf>>>,
    current_content: Rc<RefCell<String>>,
    title_widget: adw::WindowTitle,
    code_toggle: gtk4::ToggleButton,
    split_view: adw::NavigationSplitView,
    header: adw::HeaderBar,
}

impl ScribeWindow {
    pub fn new(app: &adw::Application, settings: &AppSettings) -> Self {
        let settings_rc = Rc::new(AppSettings::new());
        let file_manager = Rc::new(FileManager::new());
        let current_file = Rc::new(RefCell::new(None::<PathBuf>));
        let current_content = Rc::new(RefCell::new(String::new()));

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .default_width(settings_rc.window_width())
            .default_height(settings_rc.window_height())
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

        let wsettings = webkit6::prelude::WebViewExt::settings(&webview).expect("WebView debe tener settings");
        wsettings.set_enable_javascript(true);
        wsettings.set_enable_developer_extras(true);
        wsettings.set_javascript_can_access_clipboard(true);

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

        // === INFO POPOVER ===
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

        // === HEADERBAR (estilo GNOME Text Editor) ===
        let header = adw::HeaderBar::new();

        // Menu hamburguesa
        let menu = gio::Menu::new();
        menu.append(Some("Nueva ventana"), Some("app.new-window"));
        menu.append(Some("Abrir..."), Some("win.open"));
        menu.append(Some("Guardar"), Some("win.save"));
        menu.append(Some("Guardar como..."), Some("win.save-as"));
        menu.append(None, None);
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

        // Right side buttons
        let search_btn = gtk4::ToggleButton::builder()
            .icon_name("system-search-symbolic")
            .tooltip_text("Buscar (Ctrl+F)")
            .build();
        header.pack_end(&search_btn);

        let info_button = gtk4::MenuButton::builder()
            .icon_name("info-symbolic")
            .tooltip_text("Estadísticas del documento")
            .popover(&info_popover)
            .build();
        header.pack_end(&info_button);

        let code_toggle = gtk4::ToggleButton::builder()
            .icon_name("code-symbolic")
            .tooltip_text("Mostrar código fuente (Ctrl+Shift+C)")
            .build();
        header.pack_end(&code_toggle);

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
            .collapsed(true)
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
        let action_toggle_search = gio::SimpleAction::new("toggle-search", None);
        let action_toggle_code = gio::SimpleAction::new("toggle-code", None);
        let action_preferences = gio::SimpleAction::new("preferences", None);
        let action_help = gio::SimpleAction::new("help", None);
        let action_about = gio::SimpleAction::new("about", None);

        window.add_action(&action_open);
        window.add_action(&action_save);
        window.add_action(&action_save_as);
        window.add_action(&action_focus_mode);
        window.add_action(&action_toggle_sidebar);
        window.add_action(&action_toggle_search);
        window.add_action(&action_toggle_code);
        window.add_action(&action_preferences);
        window.add_action(&action_help);
        window.add_action(&action_about);

        // === CONNECT: Open file ===
        let window_clone = window.clone();
        let file_manager_clone = file_manager.clone();
        let current_file_clone = current_file.clone();
        let bridge_clone = bridge.clone();
        let title_widget_clone = title_widget.clone();
        action_open.connect_activate(move |_, _| {
            let current_file_inner = current_file_clone.clone();
            let bridge_inner = bridge_clone.clone();
            let title_inner = title_widget_clone.clone();
            file_manager_clone.open_file_dialog(&window_clone, move |path, content| {
                if let (Some(p), Some(c)) = (path, content) {
                    *current_file_inner.borrow_mut() = Some(p.clone());
                    bridge_inner.set_content(&c);
                    title_inner.set_title(p.file_stem().unwrap_or_default().to_str().unwrap_or("Sin título"));
                }
            });
        });

        // === CONNECT: Save ===
        let window_clone = window.clone();
        let file_manager_clone = file_manager.clone();
        let current_file_clone = current_file.clone();
        let current_content_clone = current_content.clone();
        let bridge_clone = bridge.clone();
        action_save.connect_activate(move |_, _| {
            let path = current_file_clone.borrow().clone();
            if let Some(p) = path {
                bridge_clone.request_save();
            } else {
                // No file yet, do save-as
                let current_file_inner = current_file_clone.clone();
                let bridge_inner = bridge_clone.clone();
                file_manager_clone.save_file_dialog(&window_clone, None, move |path| {
                    if let Some(p) = path {
                        *current_file_inner.borrow_mut() = Some(p);
                        bridge_inner.request_save();
                    }
                });
            }
        });

        // === CONNECT: Save As ===
        let window_clone = window.clone();
        let file_manager_clone = file_manager.clone();
        let current_file_clone = current_file.clone();
        let bridge_clone = bridge.clone();
        action_save_as.connect_activate(move |_, _| {
            let current = current_file_clone.borrow().clone();
            let current_file_inner = current_file_clone.clone();
            let bridge_inner = bridge_clone.clone();
            file_manager_clone.save_file_dialog(&window_clone, current.as_ref(), move |path| {
                if let Some(p) = path {
                    *current_file_inner.borrow_mut() = Some(p);
                    bridge_inner.request_save();
                }
            });
        });

        // === CONNECT: Save callback from JS ===
        let current_file_clone = current_file.clone();
        let current_content_clone = current_content.clone();
        let file_manager_clone = file_manager.clone();
        bridge.connect_save_requested(move |content| {
            *current_content_clone.borrow_mut() = content.to_string();
            if let Some(path) = current_file_clone.borrow().as_ref() {
                let _ = file_manager_clone.save_to_path(path, content);
            }
        });

        // === CONNECT: Title changed ===
        let title_widget_clone = title_widget.clone();
        bridge.connect_title_changed(move |title| {
            title_widget_clone.set_title(title);
        });

        // === CONNECT: Stats changed ===
        let word_label_clone = word_label.clone();
        let line_label_clone = line_label.clone();
        let char_label_clone = char_label.clone();
        bridge.connect_stats_changed(move |words, lines, chars| {
            word_label_clone.set_text(&format!("Palabras: {}", words));
            line_label_clone.set_text(&format!("Líneas: {}", lines));
            char_label_clone.set_text(&format!("Caracteres: {}", chars));
        });

        // === CONNECT: Code toggle ===
        let webview_clone = webview.clone();
        let is_source_clone = is_source_mode.clone();
        let bridge_clone = bridge.clone();
        code_toggle.connect_clicked(move |btn| {
            let source = *is_source_clone.borrow();
            if source {
                webview_clone.load_html(EDITOR_HTML, Some("file:///"));
                btn.set_icon_name("code-symbolic");
                btn.set_tooltip_text(Some("Mostrar código fuente (Ctrl+Shift+C)"));
            } else {
                bridge_clone.request_save();
                webview_clone.load_html(SOURCE_HTML, Some("file:///"));
                btn.set_icon_name("document-edit-symbolic");
                btn.set_tooltip_text(Some("Mostrar editor WYSIWYG (Ctrl+Shift+C)"));
            }
            *is_source_clone.borrow_mut() = !source;
        });

        let code_toggle_clone = code_toggle.clone();
        action_toggle_code.connect_activate(move |_, _| {
            code_toggle_clone.set_active(!code_toggle_clone.is_active());
        });

        // === CONNECT: Search ===
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

        // === CONNECT: Focus mode ===
        let header_clone = header.clone();
        let toolbar_view_clone = toolbar_view.clone();
        action_focus_mode.connect_activate(move |_, _| {
            let is_focus = !header_clone.is_visible();
            header_clone.set_visible(is_focus);
            toolbar_view_clone.set_top_bar_style(if is_focus {
                adw::ToolbarStyle::Raised
            } else {
                adw::ToolbarStyle::Flat
            });
        });

        // === CONNECT: Toggle sidebar ===
        let split_view_clone = split_view.clone();
        let settings_clone = settings_rc.clone();
        action_toggle_sidebar.connect_activate(move |_, _| {
            let current = split_view_clone.shows_content();
            split_view_clone.set_show_content(!current);
            let _ = settings_clone.set_show_sidebar(current);
        });

        // Restore sidebar state
        if settings_rc.show_sidebar() {
            split_view.set_show_content(false);
        }

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
                .version("0.1.0")
                .website("https://github.com/gnacho/scribe")
                .issue_url("https://github.com/gnacho/scribe/issues")
                .license_type(gtk4::License::Gpl30)
                .build();
            about.present();
        });

        // === Keyboard shortcuts ===
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
                    _ => {}
                }
            }
            glib::Propagation::Proceed
        });
        window.add_controller(controller);

        // === Window size persistence ===
        let settings_clone = settings_rc.clone();
        window.connect_close_request(move |win| {
            let (width, height) = win.default_size();
            settings_clone.set_window_width(width);
            settings_clone.set_window_height(height);
            glib::Propagation::Proceed
        });

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
            settings: settings_rc,
            file_manager,
            current_file,
            current_content,
            title_widget,
            code_toggle,
            split_view,
            header,
        }
    }

    pub fn present(&self) {
        self.window.present();
    }
}
