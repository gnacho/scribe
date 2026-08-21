use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use crate::editor::Editor;
use crate::file_manager::{FileManager, OpenOutcome, Outcome};
use crate::preferences;
use crate::preview::PreviewPanel;
use crate::settings::AppSettings;
use crate::templates;

const SHORTCUTS_UI: &str = include_str!("shortcuts.ui");

/// Crea un documento nuevo, opcionalmente a partir de una plantilla.
type NewDocument = Rc<dyn Fn(Option<&str>)>;

pub struct ScribeWindow {
    pub window: adw::ApplicationWindow,
    load_file: Rc<dyn Fn(&Path)>,
}

fn shorten_home(path: &Path) -> String {
    let s = path.to_string_lossy().to_string();
    if let Some(home) = glib::home_dir().to_str() {
        if let Some(rest) = s.strip_prefix(home) {
            return format!("~{rest}");
        }
    }
    s
}

fn rebuild_toc(list: &gtk4::ListBox, lines_out: &Rc<RefCell<Vec<i32>>>, text: &str) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    let mut lines = Vec::new();
    let mut in_fence = false;

    for (idx, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let level = trimmed.chars().take_while(|c| *c == '#').count();
        if level == 0 || level > 6 || !trimmed[level..].starts_with(' ') {
            continue;
        }
        let rest = trimmed[level..].trim();
        if rest.is_empty() {
            continue;
        }

        let label = gtk4::Label::builder()
            .label(rest)
            .halign(gtk4::Align::Start)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .margin_start(12 + (level as i32 - 1) * 12)
            .margin_end(12)
            .margin_top(3)
            .margin_bottom(3)
            .build();
        if level == 1 {
            label.add_css_class("heading");
        } else if level >= 3 {
            label.add_css_class("dim-label");
        }
        list.append(
            &gtk4::ListBoxRow::builder()
                .child(&label)
                .activatable(true)
                .build(),
        );
        lines.push(idx as i32);
    }
    *lines_out.borrow_mut() = lines;
}

impl ScribeWindow {
    pub fn new(app: &adw::Application, settings: &Rc<AppSettings>) -> Self {
        let settings = settings.clone();
        let file_manager = Rc::new(FileManager::new());
        let current_file: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));
        let is_modified = Rc::new(Cell::new(false));
        let force_close = Rc::new(Cell::new(false));
        let alive = Rc::new(Cell::new(true));
        let toc_lines: Rc<RefCell<Vec<i32>>> = Rc::new(RefCell::new(Vec::new()));

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .default_width(settings.window_width())
            .default_height(settings.window_height())
            .build();
        if settings.window_maximized() {
            window.maximize();
        }

        let editor = Rc::new(Editor::new());
        let preview = Rc::new(PreviewPanel::new());
        let preview_visible = Rc::new(Cell::new(settings.show_preview()));

        // ============================ BARRA LATERAL ============================
        let sidebar_header = adw::HeaderBar::builder()
            .title_widget(&adw::WindowTitle::new("Documentos", ""))
            .show_end_title_buttons(false)
            .css_classes(vec!["flat".to_string()])
            .build();

        let search_entry = gtk4::SearchEntry::builder()
            .placeholder_text("Filtrar recientes…")
            .margin_top(6)
            .margin_bottom(6)
            .margin_start(12)
            .margin_end(12)
            .build();

        let recents_list = gtk4::ListBox::builder()
            .selection_mode(gtk4::SelectionMode::None)
            .css_classes(vec!["navigation-sidebar".to_string()])
            .build();
        let toc_list = gtk4::ListBox::builder()
            .selection_mode(gtk4::SelectionMode::None)
            .css_classes(vec!["navigation-sidebar".to_string()])
            .build();

        let section = |text: &str| {
            gtk4::Label::builder()
                .label(text)
                .halign(gtk4::Align::Start)
                .css_classes(vec!["heading".to_string(), "dim-label".to_string()])
                .margin_start(12)
                .margin_top(12)
                .margin_bottom(2)
                .build()
        };

        let sidebar_content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        sidebar_content.append(&search_entry);
        sidebar_content.append(&section("Recientes"));
        sidebar_content.append(&recents_list);
        sidebar_content.append(&section("Contenido"));
        sidebar_content.append(&toc_list);

        let sidebar_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        sidebar_box.set_width_request(280);
        sidebar_box.add_css_class("sidebar");
        sidebar_box.append(&sidebar_header);
        sidebar_box.append(
            &gtk4::ScrolledWindow::builder()
                .hscrollbar_policy(gtk4::PolicyType::Never)
                .vexpand(true)
                .child(&sidebar_content)
                .build(),
        );

        // ============================== CONTENIDO ==============================
        let paned = gtk4::Paned::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .start_child(&editor.widget)
            .wide_handle(true)
            .position(680)
            .hexpand(true)
            .vexpand(true)
            .shrink_start_child(false)
            .shrink_end_child(false)
            .build();
        if preview_visible.get() {
            paned.set_end_child(Some(&preview.widget));
        }

        // ============================== CABECERA ===============================
        // Mismo reparto que GNOME Text Editor: abrir (con recientes) y nuevo a la
        // izquierda, título al centro, menú principal a la derecha.
        let header = adw::HeaderBar::new();

        let recents_menu = gio::Menu::new();
        let open_button = adw::SplitButton::builder()
            .label("Abrir")
            .tooltip_text("Abrir un documento (Ctrl+O)")
            .menu_model(&recents_menu)
            .build();
        open_button.set_action_name(Some("win.open"));
        header.pack_start(&open_button);

        let templates_menu = gio::Menu::new();
        let new_button = gtk4::MenuButton::builder()
            .icon_name("document-new-symbolic")
            .tooltip_text("Documento nuevo (Ctrl+N)")
            .menu_model(&templates_menu)
            .build();
        header.pack_start(&new_button);

        let title_widget = adw::WindowTitle::new("Sin título", "");
        header.set_title_widget(Some(&title_widget));

        let menu_button = gtk4::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .primary(true)
            .tooltip_text("Menú principal")
            .build();
        header.pack_end(&menu_button);

        // ---- menú principal, con la fila de zoom como en las apps de GNOME ----
        let menu = gio::Menu::new();

        let zoom_section = gio::Menu::new();
        let zoom_item = gio::MenuItem::new(None, None);
        zoom_item.set_attribute_value("custom", Some(&"zoom".to_variant()));
        zoom_section.append_item(&zoom_item);
        menu.append_section(None, &zoom_section);

        let file_section = gio::Menu::new();
        file_section.append(Some("Nueva ventana"), Some("app.new-window"));
        file_section.append(Some("Abrir…"), Some("win.open"));
        file_section.append(Some("Guardar"), Some("win.save"));
        file_section.append(Some("Guardar como…"), Some("win.save-as"));
        menu.append_section(None, &file_section);

        let edit_section = gio::Menu::new();
        edit_section.append(Some("Alinear tablas"), Some("win.format-tables"));
        menu.append_section(None, &edit_section);

        let view_section = gio::Menu::new();
        view_section.append(Some("Barra lateral"), Some("win.toggle-sidebar"));
        view_section.append(Some("Vista dividida"), Some("win.toggle-preview"));
        view_section.append(Some("Modo foco"), Some("win.focus-mode"));
        view_section.append(Some("Máquina de escribir"), Some("win.typewriter-mode"));
        menu.append_section(None, &view_section);

        let app_section = gio::Menu::new();
        app_section.append(Some("Preferencias"), Some("win.preferences"));
        app_section.append(Some("Atajos de teclado"), Some("win.show-help-overlay"));
        app_section.append(Some("Acerca de Scribe"), Some("win.about"));
        menu.append_section(None, &app_section);

        let zoom_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .css_classes(vec!["linked".to_string()])
            .margin_top(6)
            .margin_bottom(6)
            .margin_start(12)
            .margin_end(12)
            .build();
        let zoom_out = gtk4::Button::builder()
            .icon_name("zoom-out-symbolic")
            .action_name("win.zoom-out")
            .tooltip_text("Reducir")
            .build();
        let zoom_label = gtk4::Button::builder()
            .action_name("win.zoom-reset")
            .hexpand(true)
            .tooltip_text("Restablecer")
            .build();
        let zoom_in = gtk4::Button::builder()
            .icon_name("zoom-in-symbolic")
            .action_name("win.zoom-in")
            .tooltip_text("Ampliar")
            .build();
        zoom_box.append(&zoom_out);
        zoom_box.append(&zoom_label);
        zoom_box.append(&zoom_in);

        let popover = gtk4::PopoverMenu::from_model(Some(&menu));
        popover.add_child(&zoom_box, "zoom");
        menu_button.set_popover(Some(&popover));

        // ============================ BARRA DE ESTADO ==========================
        let words_label = gtk4::Label::builder()
            .css_classes(vec!["caption".to_string(), "dim-label".to_string()])
            .build();

        let goto_spin = gtk4::SpinButton::with_range(1.0, 1.0, 1.0);
        goto_spin.set_numeric(true);
        let goto_button = gtk4::Button::builder()
            .label("Ir")
            .css_classes(vec!["suggested-action".to_string()])
            .build();
        let goto_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(6)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(10)
            .margin_end(10)
            .build();
        goto_box.append(&gtk4::Label::new(Some("Línea")));
        goto_box.append(&goto_spin);
        goto_box.append(&goto_button);
        let goto_popover = gtk4::Popover::builder().child(&goto_box).build();

        let position_button = gtk4::MenuButton::builder()
            .label("Ln 1, Col 1")
            .tooltip_text("Ir a la línea")
            .css_classes(vec!["flat".to_string()])
            .popover(&goto_popover)
            .build();

        let props_label = gtk4::Label::builder()
            .halign(gtk4::Align::Start)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();
        let props_button = gtk4::MenuButton::builder()
            .label("Markdown")
            .tooltip_text("Propiedades del documento")
            .css_classes(vec!["flat".to_string()])
            .popover(&gtk4::Popover::builder().child(&props_label).build())
            .build();

        let status_end = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        status_end.append(&position_button);
        status_end.append(&props_button);

        let status_bar = gtk4::CenterBox::builder()
            .css_classes(vec!["toolbar".to_string()])
            .margin_start(12)
            .margin_end(6)
            .build();
        status_bar.set_start_widget(Some(&words_label));
        status_bar.set_end_widget(Some(&status_end));

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.add_bottom_bar(&status_bar);
        toolbar_view.set_content(Some(&paned));

        let toasts = adw::ToastOverlay::new();
        toasts.set_child(Some(&toolbar_view));

        let split_view = adw::OverlaySplitView::builder()
            .sidebar(&sidebar_box)
            .content(&toasts)
            .show_sidebar(settings.show_sidebar())
            .enable_hide_gesture(true)
            .enable_show_gesture(true)
            .build();
        window.set_content(Some(&split_view));

        let builder = gtk4::Builder::from_string(SHORTCUTS_UI);
        if let Some(help) = builder.object::<gtk4::ShortcutsWindow>("help_overlay") {
            window.set_help_overlay(Some(&help));
        }

        // ============================== AYUDANTES ==============================
        let toast = {
            let toasts = toasts.clone();
            Rc::new(move |msg: &str| toasts.add_toast(adw::Toast::new(msg)))
        };

        let update_title: Rc<dyn Fn()> = {
            let current_file = current_file.clone();
            let is_modified = is_modified.clone();
            let title_widget = title_widget.clone();
            // Débil: este closure acaba dentro de las señales del editor, que
            // cuelga de la propia ventana; en fuerte sería un ciclo y la
            // ventana no se liberaría jamás.
            let window = window.downgrade();
            let editor = editor.clone();
            Rc::new(move || {
                let Some(window) = window.upgrade() else {
                    return;
                };
                let file = current_file.borrow();
                let (name, subtitle) = match file.as_ref() {
                    Some(p) => (
                        p.file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("Sin título")
                            .to_string(),
                        p.parent().map(shorten_home).unwrap_or_default(),
                    ),
                    // Sin fichero, el borrador se identifica por su primera
                    // cabecera, que es lo que el usuario tiene en la cabeza.
                    None => (
                        templates::title_from(&editor.text())
                            .unwrap_or_else(|| "Sin título".to_string()),
                        "Borrador".to_string(),
                    ),
                };
                let dot = if is_modified.get() { "• " } else { "" };
                title_widget.set_title(&format!("{dot}{name}"));
                title_widget.set_subtitle(&subtitle);
                window.set_title(Some(&format!("{dot}{name} — Scribe")));
            })
        };

        let update_status: Rc<dyn Fn()> = {
            let editor = editor.clone();
            let words_label = words_label.clone();
            let position_button = position_button.clone();
            let props_label = props_label.clone();
            let goto_spin = goto_spin.clone();
            Rc::new(move || {
                let text = editor.text();
                let words = text.split_whitespace().count();
                let chars = text.chars().count();
                let lines = editor.line_count();
                words_label.set_label(&format!("{words} palabras"));
                let (line, column) = editor.cursor_position();
                position_button.set_label(&format!("Ln {line}, Col {column}"));
                goto_spin.set_range(1.0, lines.max(1) as f64);
                goto_spin.set_value(line as f64);
                let minutes = (words as f64 / 200.0).ceil().max(1.0) as usize;
                props_label.set_label(&format!(
                    "Palabras: {words}\nCaracteres: {chars}\nLíneas: {lines}\nLectura: ~{minutes} min"
                ));
            })
        };

        let refresh_recents: Rc<dyn Fn(&str)> = {
            let settings = settings.clone();
            let recents_list = recents_list.clone();
            let recents_menu = recents_menu.clone();
            Rc::new(move |filter: &str| {
                while let Some(child) = recents_list.first_child() {
                    recents_list.remove(&child);
                }
                recents_menu.remove_all();

                let needle = filter.to_lowercase();
                let mut shown = 0;
                for entry in settings.recent_files() {
                    let path = PathBuf::from(&entry);
                    let name = path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_string();
                    if !needle.is_empty() && !name.to_lowercase().contains(&needle) {
                        continue;
                    }
                    let row = adw::ActionRow::builder()
                        .title(&name)
                        .subtitle(path.parent().map(shorten_home).unwrap_or_default())
                        .activatable(true)
                        .build();
                    row.set_action_name(Some("win.open-recent"));
                    row.set_action_target_value(Some(&entry.to_variant()));
                    if !path.exists() {
                        row.set_subtitle("No encontrado");
                        row.set_sensitive(false);
                    }
                    recents_list.append(&row);

                    if shown < 10 && path.exists() {
                        let item = gio::MenuItem::new(Some(&name), None);
                        item.set_action_and_target_value(
                            Some("win.open-recent"),
                            Some(&entry.to_variant()),
                        );
                        recents_menu.append_item(&item);
                    }
                    shown += 1;
                }
                if shown == 0 {
                    let row = adw::ActionRow::builder()
                        .title(if needle.is_empty() {
                            "Sin documentos recientes"
                        } else {
                            "Ningún resultado"
                        })
                        .build();
                    row.set_sensitive(false);
                    recents_list.append(&row);
                }
            })
        };
        refresh_recents("");

        let refresh_templates: Rc<dyn Fn()> = {
            let templates_menu = templates_menu.clone();
            Rc::new(move || {
                templates_menu.remove_all();
                let blank = gio::Menu::new();
                blank.append(Some("Documento en blanco"), Some("win.new-document"));
                templates_menu.append_section(None, &blank);

                let list = templates::list();
                if !list.is_empty() {
                    let section = gio::Menu::new();
                    for template in list {
                        let item = gio::MenuItem::new(Some(&template.name), None);
                        item.set_action_and_target_value(
                            Some("win.new-from-template"),
                            Some(&template.name.to_variant()),
                        );
                        section.append_item(&item);
                    }
                    templates_menu.append_section(Some("Plantillas"), &section);
                }
            })
        };
        refresh_templates();

        let load_file: Rc<dyn Fn(&Path)> = {
            let editor = editor.clone();
            let current_file = current_file.clone();
            let is_modified = is_modified.clone();
            let settings = settings.clone();
            let update_title = update_title.clone();
            let update_status = update_status.clone();
            let refresh_recents = refresh_recents.clone();
            let toast = toast.clone();
            Rc::new(move |path: &Path| match std::fs::read_to_string(path) {
                Ok(content) => {
                    editor.set_text(&content);
                    *current_file.borrow_mut() = Some(path.to_path_buf());
                    is_modified.set(false);
                    settings.push_recent_file(&path.to_string_lossy());
                    refresh_recents("");
                    update_title();
                    update_status();
                }
                Err(e) => toast(&format!("No se pudo abrir: {e}")),
            })
        };

        let new_document: NewDocument = {
            let editor = editor.clone();
            let current_file = current_file.clone();
            let is_modified = is_modified.clone();
            let update_title = update_title.clone();
            let update_status = update_status.clone();
            Rc::new(move |template_name: Option<&str>| {
                let body = template_name
                    .and_then(templates::find)
                    .and_then(|t| t.body())
                    .map(|b| templates::render(&b, "Sin título"))
                    .unwrap_or_default();
                editor.set_text(&body);
                *current_file.borrow_mut() = None;
                is_modified.set(false);
                update_title();
                update_status();
            })
        };

        // ============================== ACCIONES ===============================
        let action_open = gio::SimpleAction::new("open", None);
        let action_save = gio::SimpleAction::new("save", None);
        let action_save_as = gio::SimpleAction::new("save-as", None);
        let action_preferences = gio::SimpleAction::new("preferences", None);
        let action_about = gio::SimpleAction::new("about", None);
        let action_new_document = gio::SimpleAction::new("new-document", None);
        let action_open_recent =
            gio::SimpleAction::new("open-recent", Some(glib::VariantTy::STRING));
        let action_new_template =
            gio::SimpleAction::new("new-from-template", Some(glib::VariantTy::STRING));
        let action_zoom_in = gio::SimpleAction::new("zoom-in", None);
        let action_zoom_out = gio::SimpleAction::new("zoom-out", None);
        let action_zoom_reset = gio::SimpleAction::new("zoom-reset", None);
        let action_bold = gio::SimpleAction::new("bold", None);
        let action_italic = gio::SimpleAction::new("italic", None);
        let action_code = gio::SimpleAction::new("code", None);
        let action_format_tables = gio::SimpleAction::new("format-tables", None);

        // Las alternancias son con estado, para que el menú muestre la marca.
        let action_sidebar = gio::SimpleAction::new_stateful(
            "toggle-sidebar",
            None,
            &settings.show_sidebar().to_variant(),
        );
        let action_preview = gio::SimpleAction::new_stateful(
            "toggle-preview",
            None,
            &settings.show_preview().to_variant(),
        );
        let action_focus = gio::SimpleAction::new_stateful(
            "focus-mode",
            None,
            &settings.focus_mode().to_variant(),
        );
        let action_typewriter = gio::SimpleAction::new_stateful(
            "typewriter-mode",
            None,
            &settings.typewriter_mode().to_variant(),
        );

        for a in [
            &action_open,
            &action_save,
            &action_save_as,
            &action_preferences,
            &action_about,
            &action_new_document,
            &action_zoom_in,
            &action_zoom_out,
            &action_zoom_reset,
            &action_bold,
            &action_italic,
            &action_code,
            &action_format_tables,
        ] {
            window.add_action(a);
        }
        window.add_action(&action_open_recent);
        window.add_action(&action_new_template);
        window.add_action(&action_sidebar);
        window.add_action(&action_preview);
        window.add_action(&action_focus);
        window.add_action(&action_typewriter);

        for (action, marker) in [
            (&action_bold, "**"),
            (&action_italic, "*"),
            (&action_code, "`"),
        ] {
            let editor = editor.clone();
            action.connect_activate(move |_, _| editor.wrap_selection(marker));
        }

        {
            let editor = editor.clone();
            let toast = toast.clone();
            action_format_tables.connect_activate(move |_, _| {
                if editor.format_tables() {
                    toast("Tablas alineadas");
                } else {
                    toast("No hay tablas que alinear");
                }
            });
        }

        // ---- aplicar preferencias ----
        let apply_settings: Rc<dyn Fn()> = {
            let settings = settings.clone();
            let editor = editor.clone();
            let preview = preview.clone();
            let zoom_label = zoom_label.clone();
            let refresh_recents = refresh_recents.clone();
            let refresh_templates = refresh_templates.clone();
            Rc::new(move || {
                adw::StyleManager::default().set_color_scheme(
                    match settings.color_scheme_index() {
                        1 => adw::ColorScheme::ForceLight,
                        2 => adw::ColorScheme::ForceDark,
                        _ => adw::ColorScheme::Default,
                    },
                );
                let size = settings.font_size();
                editor.set_font(settings.font_family(), size, settings.line_spacing());
                editor.set_column_width(settings.column_width());
                editor.set_markup_visibility(settings.markup_visibility());
                editor.set_focus_mode(settings.focus_mode());
                editor.set_typewriter_mode(settings.typewriter_mode());
                editor.set_continue_lists(settings.continue_lists());
                editor.set_tab_width(settings.tab_width());
                preview.set_font_size(size);
                zoom_label.set_label(&format!("{}px", size));
                refresh_recents("");
                refresh_templates();
            })
        };
        apply_settings();

        {
            // Débil: la ventana posee sus acciones; una captura fuerte aquí
            // sería un ciclo ventana↔acción.
            let window_clone = window.downgrade();
            let settings = settings.clone();
            let apply_settings = apply_settings.clone();
            let action_focus = action_focus.clone();
            let action_typewriter = action_typewriter.clone();
            action_preferences.connect_activate(move |_, _| {
                let Some(window_clone) = window_clone.upgrade() else {
                    return;
                };
                let action_focus = action_focus.clone();
                let action_typewriter = action_typewriter.clone();
                let settings_inner = settings.clone();
                let apply_inner = apply_settings.clone();
                // Las preferencias y el menú comparten estado: hay que
                // sincronizar las alternancias en los dos sentidos.
                let sync: Rc<dyn Fn()> = Rc::new(move || {
                    action_focus.set_state(&settings_inner.focus_mode().to_variant());
                    action_typewriter.set_state(&settings_inner.typewriter_mode().to_variant());
                    apply_inner();
                });
                preferences::present(&window_clone, &settings, sync);
            });
        }

        // ---- zoom ----
        for (action, delta) in [(&action_zoom_in, 1i32), (&action_zoom_out, -1)] {
            let settings = settings.clone();
            let apply_settings = apply_settings.clone();
            action.connect_activate(move |_, _| {
                settings.set_font_size((settings.font_size() + delta).clamp(9, 40));
                apply_settings();
            });
        }
        {
            let settings = settings.clone();
            let apply_settings = apply_settings.clone();
            action_zoom_reset.connect_activate(move |_, _| {
                settings.set_font_size(16);
                apply_settings();
            });
        }

        // ---- documentos nuevos ----
        {
            let new_document = new_document.clone();
            let settings = settings.clone();
            action_new_document.connect_activate(move |_, _| {
                let name = settings.default_template();
                new_document(if name.is_empty() { None } else { Some(&name) });
            });
        }
        {
            let new_document = new_document.clone();
            action_new_template.connect_activate(move |_, param| {
                if let Some(name) = param.and_then(|p| p.str()) {
                    new_document(Some(name));
                }
            });
        }
        {
            let load_file = load_file.clone();
            action_open_recent.connect_activate(move |_, param| {
                if let Some(path) = param.and_then(|p| p.str()) {
                    load_file(Path::new(path));
                }
            });
        }

        // ---- abrir / guardar ----
        {
            // Débil: véase action_preferences.
            let window = window.downgrade();
            let file_manager = file_manager.clone();
            let editor = editor.clone();
            let current_file = current_file.clone();
            let is_modified = is_modified.clone();
            let settings = settings.clone();
            let update_title = update_title.clone();
            let update_status = update_status.clone();
            let refresh_recents = refresh_recents.clone();
            let toast = toast.clone();
            action_open.connect_activate(move |_, _| {
                let Some(window) = window.upgrade() else {
                    return;
                };
                let editor = editor.clone();
                let current_file = current_file.clone();
                let is_modified = is_modified.clone();
                let settings = settings.clone();
                let update_title = update_title.clone();
                let update_status = update_status.clone();
                let refresh_recents = refresh_recents.clone();
                let toast = toast.clone();
                file_manager.open(&window, move |outcome| match outcome {
                    OpenOutcome::Ok((path, content)) => {
                        editor.set_text(&content);
                        *current_file.borrow_mut() = Some(path.clone());
                        is_modified.set(false);
                        settings.push_recent_file(&path.to_string_lossy());
                        refresh_recents("");
                        update_title();
                        update_status();
                    }
                    OpenOutcome::Error(e) => toast(&e),
                    OpenOutcome::Cancelled => {}
                });
            });
        }

        let make_save = |force_dialog: bool| {
            // Débil: véase action_preferences.
            let window = window.downgrade();
            let file_manager = file_manager.clone();
            let editor = editor.clone();
            let current_file = current_file.clone();
            let is_modified = is_modified.clone();
            let settings = settings.clone();
            let update_title = update_title.clone();
            let refresh_recents = refresh_recents.clone();
            let toast = toast.clone();
            move |_: &gio::SimpleAction, _: Option<&glib::Variant>| {
                let Some(window) = window.upgrade() else {
                    return;
                };
                let content = editor.text();
                let path = if force_dialog {
                    None
                } else {
                    current_file.borrow().clone()
                };
                let current_file = current_file.clone();
                let is_modified = is_modified.clone();
                let settings = settings.clone();
                let update_title = update_title.clone();
                let refresh_recents = refresh_recents.clone();
                let toast = toast.clone();
                file_manager.save(
                    &window,
                    path.as_ref(),
                    &content,
                    move |outcome| match outcome {
                        Outcome::Ok(p) => {
                            *current_file.borrow_mut() = Some(p.clone());
                            is_modified.set(false);
                            settings.push_recent_file(&p.to_string_lossy());
                            refresh_recents("");
                            update_title();
                            toast("Guardado");
                        }
                        Outcome::Error(e) => toast(&e),
                        Outcome::Cancelled => {}
                    },
                );
            }
        };
        action_save.connect_activate(make_save(false));
        action_save_as.connect_activate(make_save(true));

        // ---- alternancias con estado ----
        {
            let split_view = split_view.clone();
            let settings = settings.clone();
            action_sidebar.connect_activate(move |action, _| {
                let next = !split_view.shows_sidebar();
                split_view.set_show_sidebar(next);
                action.set_state(&next.to_variant());
                settings.set_show_sidebar(next);
            });
        }
        {
            let paned = paned.clone();
            let preview = preview.clone();
            let preview_visible = preview_visible.clone();
            let editor = editor.clone();
            let settings = settings.clone();
            action_preview.connect_activate(move |action, _| {
                let next = !preview_visible.get();
                preview_visible.set(next);
                if next {
                    preview.update(&editor.text());
                    paned.set_end_child(Some(&preview.widget));
                } else {
                    paned.set_end_child(None::<&gtk4::Widget>);
                }
                action.set_state(&next.to_variant());
                settings.set_show_preview(next);
            });
        }
        for (action, is_focus) in [(&action_focus, true), (&action_typewriter, false)] {
            let editor = editor.clone();
            let settings = settings.clone();
            action.connect_activate(move |action, _| {
                let next = !action
                    .state()
                    .and_then(|s| s.get::<bool>())
                    .unwrap_or(false);
                action.set_state(&next.to_variant());
                if is_focus {
                    settings.set_focus_mode(next);
                    editor.set_focus_mode(next);
                } else {
                    settings.set_typewriter_mode(next);
                    editor.set_typewriter_mode(next);
                }
            });
        }

        // ---- acerca de ----
        {
            // Débil: la ventana posee la acción; en fuerte era un ciclo
            // ventana↔acción y ninguna ScribeWindow llegaba a liberarse.
            let window = window.downgrade();
            action_about.connect_activate(move |_, _| {
                let Some(window) = window.upgrade() else {
                    return;
                };
                adw::AboutWindow::builder()
                    .transient_for(&window)
                    .modal(true)
                    .application_name("Scribe")
                    .application_icon("app.scribe.Scribe")
                    .developer_name("gnacho")
                    .version(env!("CARGO_PKG_VERSION"))
                    .website("https://github.com/gnacho/scribe")
                    .issue_url("https://github.com/gnacho/scribe/issues")
                    .license_type(gtk4::License::Agpl30)
                    .build()
                    .present();
            });
        }

        // ============================== SEÑALES ================================
        {
            let preview = preview.clone();
            let preview_visible = preview_visible.clone();
            let toc_list = toc_list.clone();
            let toc_lines = toc_lines.clone();
            let is_modified = is_modified.clone();
            let update_title = update_title.clone();
            let update_status = update_status.clone();
            let generation = Rc::new(Cell::new(0u64));
            editor.connect_changed(move |text| {
                if !is_modified.replace(true) {
                    update_title();
                }
                update_status();

                let current = generation.get().wrapping_add(1);
                generation.set(current);
                let text = text.to_string();
                let generation = generation.clone();
                let preview = preview.clone();
                let preview_visible = preview_visible.clone();
                let toc_list = toc_list.clone();
                let toc_lines = toc_lines.clone();
                glib::timeout_add_local_once(Duration::from_millis(120), move || {
                    if generation.get() != current {
                        return;
                    }
                    if preview_visible.get() {
                        preview.update(&text);
                    }
                    rebuild_toc(&toc_list, &toc_lines, &text);
                });
            });
        }
        {
            let update_status = update_status.clone();
            editor.connect_cursor_moved(move || update_status());
        }
        {
            let toc_lines = toc_lines.clone();
            let editor = editor.clone();
            toc_list.connect_row_activated(move |_, row| {
                let line = toc_lines.borrow().get(row.index() as usize).copied();
                if let Some(line) = line {
                    editor.scroll_to_line(line);
                }
            });
        }
        {
            let refresh_recents = refresh_recents.clone();
            search_entry
                .connect_search_changed(move |entry| refresh_recents(entry.text().as_str()));
        }
        {
            let editor = editor.clone();
            let goto_spin = goto_spin.clone();
            let goto_popover = goto_popover.clone();
            goto_button.connect_clicked(move |_| {
                editor.go_to_line(goto_spin.value() as i32);
                goto_popover.popdown();
            });
        }

        // ---- autoguardado ----
        {
            let settings = settings.clone();
            let editor = editor.clone();
            let current_file = current_file.clone();
            let is_modified = is_modified.clone();
            let update_title = update_title.clone();
            let alive = alive.clone();
            let elapsed = Cell::new(0i32);
            glib::timeout_add_seconds_local(5, move || {
                if !alive.get() {
                    return glib::ControlFlow::Break;
                }
                elapsed.set(elapsed.get() + 5);
                if elapsed.get() < settings.autosave_interval() {
                    return glib::ControlFlow::Continue;
                }
                elapsed.set(0);
                if settings.autosave() && is_modified.get() {
                    let path = current_file.borrow().clone();
                    if let Some(path) = path {
                        if std::fs::write(&path, editor.text()).is_ok() {
                            is_modified.set(false);
                            update_title();
                        }
                    }
                }
                glib::ControlFlow::Continue
            });
        }

        // ---- cierre ----
        {
            let settings = settings.clone();
            let is_modified = is_modified.clone();
            let force_close = force_close.clone();
            let alive = alive.clone();
            let current_file = current_file.clone();
            let editor = editor.clone();
            let file_manager = file_manager.clone();
            let toast = toast.clone();
            window.connect_close_request(move |win| {
                let (w, h) = win.default_size();
                settings.set_window_width(w);
                settings.set_window_height(h);
                settings.set_window_maximized(win.is_maximized());

                if force_close.get() || !is_modified.get() {
                    alive.set(false);
                    return glib::Propagation::Proceed;
                }

                let dialog = adw::MessageDialog::new(
                    Some(win),
                    Some("¿Guardar los cambios?"),
                    Some("Si cierras sin guardar, perderás lo que hayas escrito."),
                );
                dialog.add_response("cancel", "Cancelar");
                dialog.add_response("discard", "Descartar");
                dialog.add_response("save", "Guardar");
                dialog.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
                dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
                dialog.set_default_response(Some("save"));
                dialog.set_close_response("cancel");

                let win = win.clone();
                let force_close = force_close.clone();
                let current_file = current_file.clone();
                let editor = editor.clone();
                let file_manager = file_manager.clone();
                let toast = toast.clone();
                dialog.choose(gio::Cancellable::NONE, move |response| {
                    match response.as_str() {
                        "discard" => {
                            force_close.set(true);
                            win.close();
                        }
                        "save" => {
                            let path = current_file.borrow().clone();
                            match path {
                                Some(p) => match std::fs::write(&p, editor.text()) {
                                    Ok(()) => {
                                        force_close.set(true);
                                        win.close();
                                    }
                                    Err(e) => toast(&format!("No se pudo guardar: {e}")),
                                },
                                // Documento sin fichero: se abre «Guardar como» y
                                // la ventana debe cerrarse al terminar; si no, el
                                // usuario se queda con ella abierta tras pedir
                                // guardar y cerrar.
                                None => {
                                    let parent = win.clone();
                                    let win = win.clone();
                                    let force_close = force_close.clone();
                                    let toast = toast.clone();
                                    file_manager.save(
                                        &parent,
                                        None,
                                        &editor.text(),
                                        move |outcome| match outcome {
                                            Outcome::Ok(_) => {
                                                force_close.set(true);
                                                win.close();
                                            }
                                            Outcome::Error(e) => {
                                                toast(&format!("No se pudo guardar: {e}"));
                                            }
                                            Outcome::Cancelled => {}
                                        },
                                    );
                                }
                            }
                        }
                        _ => {}
                    }
                });
                glib::Propagation::Stop
            });
        }

        update_title();
        update_status();
        Self { window, load_file }
    }

    pub fn open_path(&self, path: &Path) {
        (self.load_file)(path);
    }

    pub fn present(&self) {
        self.window.present();
    }
}
