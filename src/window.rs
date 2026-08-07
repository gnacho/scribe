use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use crate::editor::Editor;
use crate::file_manager::{FileManager, Outcome, OpenOutcome};
use crate::preview::PreviewPanel;
use crate::settings::AppSettings;

const SHORTCUTS_UI: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<interface>
  <object class="GtkShortcutsWindow" id="help_overlay">
    <property name="modal">1</property>
    <child>
      <object class="GtkShortcutsSection">
        <property name="section-name">shortcuts</property>
        <property name="max-height">12</property>
        <child>
          <object class="GtkShortcutsGroup">
            <property name="title">Archivo</property>
            <child>
              <object class="GtkShortcutsShortcut">
                <property name="title">Abrir</property>
                <property name="accelerator">&lt;Control&gt;o</property>
              </object>
            </child>
            <child>
              <object class="GtkShortcutsShortcut">
                <property name="title">Guardar</property>
                <property name="accelerator">&lt;Control&gt;s</property>
              </object>
            </child>
            <child>
              <object class="GtkShortcutsShortcut">
                <property name="title">Guardar como</property>
                <property name="accelerator">&lt;Control&gt;&lt;Shift&gt;s</property>
              </object>
            </child>
            <child>
              <object class="GtkShortcutsShortcut">
                <property name="title">Nueva ventana</property>
                <property name="accelerator">&lt;Control&gt;n</property>
              </object>
            </child>
            <child>
              <object class="GtkShortcutsShortcut">
                <property name="title">Salir</property>
                <property name="accelerator">&lt;Control&gt;q</property>
              </object>
            </child>
          </object>
        </child>
        <child>
          <object class="GtkShortcutsGroup">
            <property name="title">Vista</property>
            <child>
              <object class="GtkShortcutsShortcut">
                <property name="title">Barra lateral</property>
                <property name="accelerator">F9</property>
              </object>
            </child>
            <child>
              <object class="GtkShortcutsShortcut">
                <property name="title">Previsualización</property>
                <property name="accelerator">&lt;Control&gt;&lt;Shift&gt;p</property>
              </object>
            </child>
            <child>
              <object class="GtkShortcutsShortcut">
                <property name="title">Preferencias</property>
                <property name="accelerator">&lt;Control&gt;comma</property>
              </object>
            </child>
          </object>
        </child>
      </object>
    </child>
  </object>
</interface>"#;

pub struct ScribeWindow {
    pub window: adw::ApplicationWindow,
    load_file: Rc<dyn Fn(&Path)>,
}

/// Sustituye el home del usuario por `~` para que el subtítulo no ocupe media barra.
fn shorten_home(path: &Path) -> String {
    let s = path.to_string_lossy().to_string();
    if let Some(home) = glib::home_dir().to_str() {
        if let Some(rest) = s.strip_prefix(home) {
            return format!("~{}", rest);
        }
    }
    s
}

/// Reconstruye el índice, saltándose las cabeceras que estén dentro de un bloque de código.
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
        if level == 0 || level > 6 {
            continue;
        }
        let rest = trimmed[level..].trim();
        if rest.is_empty() || !trimmed[level..].starts_with(' ') {
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

        let row = gtk4::ListBoxRow::builder()
            .child(&label)
            .activatable(true)
            .build();
        list.append(&row);
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
        let recents: Rc<RefCell<Vec<PathBuf>>> = Rc::new(RefCell::new(Vec::new()));
        let toc_lines: Rc<RefCell<Vec<i32>>> = Rc::new(RefCell::new(Vec::new()));

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .default_width(settings.window_width())
            .default_height(settings.window_height())
            .build();

        // === EDITOR ===
        let editor = Rc::new(Editor::new());
        editor.widget.set_hexpand(true);
        editor.widget.set_vexpand(true);
        editor.set_style(settings.font_size(), settings.line_spacing());

        // === PREVIEW ===
        let preview = Rc::new(PreviewPanel::new());
        preview.set_font_size(settings.font_size());
        let preview_visible = Rc::new(Cell::new(settings.show_preview()));

        // === SIDEBAR ===
        let sidebar_header = adw::HeaderBar::builder()
            .title_widget(&adw::WindowTitle::new("Documentos", ""))
            .show_end_title_buttons(false)
            .css_classes(vec!["flat".to_string()])
            .build();

        let search_entry = gtk4::SearchEntry::builder()
            .placeholder_text("Filtrar recientes…")
            .margin_top(12)
            .margin_bottom(6)
            .margin_start(12)
            .margin_end(12)
            .build();

        let notes_label = gtk4::Label::builder()
            .label("Recientes")
            .halign(gtk4::Align::Start)
            .css_classes(vec!["heading".to_string(), "dim-label".to_string()])
            .margin_start(12)
            .margin_top(12)
            .build();

        let notes_list = gtk4::ListBox::builder()
            .selection_mode(gtk4::SelectionMode::None)
            .css_classes(vec!["navigation-sidebar".to_string()])
            .build();

        let toc_label = gtk4::Label::builder()
            .label("Contenido")
            .halign(gtk4::Align::Start)
            .css_classes(vec!["heading".to_string(), "dim-label".to_string()])
            .margin_start(12)
            .margin_top(12)
            .build();

        let toc_list = gtk4::ListBox::builder()
            .selection_mode(gtk4::SelectionMode::None)
            .css_classes(vec!["navigation-sidebar".to_string()])
            .build();

        let sidebar_content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        sidebar_content.append(&search_entry);
        sidebar_content.append(&notes_label);
        sidebar_content.append(&notes_list);
        sidebar_content.append(&toc_label);
        sidebar_content.append(&toc_list);

        let sidebar_scrolled = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vexpand(true)
            .child(&sidebar_content)
            .build();

        let sidebar_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        sidebar_box.set_width_request(260);
        sidebar_box.add_css_class("sidebar");
        sidebar_box.append(&sidebar_header);
        sidebar_box.append(&sidebar_scrolled);

        // === PANED ===
        let paned = gtk4::Paned::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .start_child(&editor.widget)
            .wide_handle(true)
            .position(600)
            .hexpand(true)
            .vexpand(true)
            .shrink_start_child(false)
            .shrink_end_child(false)
            .build();
        if preview_visible.get() {
            paned.set_end_child(Some(&preview.widget));
        }

        // === HEADERBAR ===
        let header = adw::HeaderBar::new();

        let open_btn = gtk4::Button::builder()
            .icon_name("document-open-symbolic")
            .tooltip_text("Abrir (Ctrl+O)")
            .action_name("win.open")
            .build();
        header.pack_start(&open_btn);

        let save_btn = gtk4::Button::builder()
            .icon_name("document-save-symbolic")
            .tooltip_text("Guardar (Ctrl+S)")
            .action_name("win.save")
            .build();
        header.pack_start(&save_btn);

        let title_widget = adw::WindowTitle::new("Sin título", "");
        header.set_title_widget(Some(&title_widget));

        let menu_btn = gtk4::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .primary(true)
            .tooltip_text("Menú principal")
            .build();
        header.pack_end(&menu_btn);

        let preview_btn = gtk4::ToggleButton::builder()
            .icon_name("view-dual-symbolic")
            .tooltip_text("Previsualización (Ctrl+Shift+P)")
            .active(preview_visible.get())
            .build();
        header.pack_end(&preview_btn);

        // Estadísticas del documento, en un popover que se recalcula al abrirlo.
        let stats_label = gtk4::Label::builder()
            .halign(gtk4::Align::Start)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();
        let stats_popover = gtk4::Popover::builder().child(&stats_label).build();
        let stats_btn = gtk4::MenuButton::builder()
            .icon_name("info-symbolic")
            .tooltip_text("Estadísticas del documento")
            .popover(&stats_popover)
            .build();
        header.pack_end(&stats_btn);

        // === MENU ===
        let menu = gio::Menu::new();
        let file_section = gio::Menu::new();
        file_section.append(Some("Nueva ventana"), Some("app.new-window"));
        file_section.append(Some("Abrir…"), Some("win.open"));
        file_section.append(Some("Guardar"), Some("win.save"));
        file_section.append(Some("Guardar como…"), Some("win.save-as"));
        menu.append_section(None, &file_section);

        let view_section = gio::Menu::new();
        view_section.append(Some("Barra lateral"), Some("win.toggle-sidebar"));
        view_section.append(Some("Previsualización"), Some("win.toggle-preview"));
        menu.append_section(None, &view_section);

        let app_section = gio::Menu::new();
        app_section.append(Some("Preferencias"), Some("win.preferences"));
        app_section.append(Some("Atajos de teclado"), Some("win.show-help-overlay"));
        app_section.append(Some("Acerca de Scribe"), Some("win.about"));
        app_section.append(Some("Salir"), Some("app.quit"));
        menu.append_section(None, &app_section);
        menu_btn.set_menu_model(Some(&menu));

        // === CONTENT ===
        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&paned));

        let toasts = adw::ToastOverlay::new();
        toasts.set_child(Some(&toolbar_view));

        let overlay_split = adw::OverlaySplitView::builder()
            .sidebar(&sidebar_box)
            .content(&toasts)
            .show_sidebar(settings.show_sidebar())
            .enable_hide_gesture(true)
            .enable_show_gesture(true)
            .build();

        window.set_content(Some(&overlay_split));

        // Ventana de atajos -> registra automáticamente win.show-help-overlay.
        let builder = gtk4::Builder::from_string(SHORTCUTS_UI);
        if let Some(help) = builder.object::<gtk4::ShortcutsWindow>("help_overlay") {
            window.set_help_overlay(Some(&help));
        }

        // ---------------------------------------------------------------
        // Helpers compartidos
        // ---------------------------------------------------------------
        let toast = {
            let toasts = toasts.clone();
            Rc::new(move |msg: &str| {
                toasts.add_toast(adw::Toast::new(msg));
            })
        };

        let update_title: Rc<dyn Fn()> = {
            let current_file = current_file.clone();
            let is_modified = is_modified.clone();
            let title_widget = title_widget.clone();
            let window = window.clone();
            Rc::new(move || {
                let file = current_file.borrow();
                let (name, subtitle) = match file.as_ref() {
                    Some(p) => (
                        p.file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("Sin título")
                            .to_string(),
                        p.parent().map(shorten_home).unwrap_or_default(),
                    ),
                    None => ("Sin título".to_string(), "No guardado".to_string()),
                };
                let dot = if is_modified.get() { "• " } else { "" };
                title_widget.set_title(&format!("{dot}{name}"));
                title_widget.set_subtitle(&subtitle);
                window.set_title(Some(&format!("{dot}{name} — Scribe")));
            })
        };

        let refresh_recents: Rc<dyn Fn(&str)> = {
            let settings = settings.clone();
            let notes_list = notes_list.clone();
            let recents = recents.clone();
            Rc::new(move |filter: &str| {
                while let Some(child) = notes_list.first_child() {
                    notes_list.remove(&child);
                }
                let needle = filter.to_lowercase();
                let mut paths = Vec::new();
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
                    if !path.exists() {
                        row.set_subtitle("No encontrado");
                        row.set_sensitive(false);
                    }
                    notes_list.append(&row);
                    paths.push(path);
                }
                if paths.is_empty() {
                    let row = adw::ActionRow::builder()
                        .title(if needle.is_empty() {
                            "Sin documentos recientes"
                        } else {
                            "Ningún resultado"
                        })
                        .build();
                    row.set_sensitive(false);
                    notes_list.append(&row);
                }
                *recents.borrow_mut() = paths;
            })
        };
        refresh_recents("");

        let load_file: Rc<dyn Fn(&Path)> = {
            let editor = editor.clone();
            let current_file = current_file.clone();
            let is_modified = is_modified.clone();
            let settings = settings.clone();
            let update_title = update_title.clone();
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
                }
                Err(e) => toast(&format!("No se pudo abrir: {e}")),
            })
        };

        // ---------------------------------------------------------------
        // Acciones
        // ---------------------------------------------------------------
        let action_open = gio::SimpleAction::new("open", None);
        let action_save = gio::SimpleAction::new("save", None);
        let action_save_as = gio::SimpleAction::new("save-as", None);
        let action_toggle_sidebar = gio::SimpleAction::new("toggle-sidebar", None);
        let action_toggle_preview = gio::SimpleAction::new("toggle-preview", None);
        let action_preferences = gio::SimpleAction::new("preferences", None);
        let action_about = gio::SimpleAction::new("about", None);

        for a in [
            &action_open,
            &action_save,
            &action_save_as,
            &action_toggle_sidebar,
            &action_toggle_preview,
            &action_preferences,
            &action_about,
        ] {
            window.add_action(a);
        }

        // --- Abrir ---
        {
            let window = window.clone();
            let file_manager = file_manager.clone();
            let editor = editor.clone();
            let current_file = current_file.clone();
            let is_modified = is_modified.clone();
            let settings = settings.clone();
            let update_title = update_title.clone();
            let refresh_recents = refresh_recents.clone();
            let toast = toast.clone();
            action_open.connect_activate(move |_, _| {
                let editor = editor.clone();
                let current_file = current_file.clone();
                let is_modified = is_modified.clone();
                let settings = settings.clone();
                let update_title = update_title.clone();
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
                    }
                    OpenOutcome::Error(e) => toast(&e),
                    OpenOutcome::Cancelled => {}
                });
            });
        }

        // --- Guardar / Guardar como ---
        let make_save = |force_dialog: bool| {
            let window = window.clone();
            let file_manager = file_manager.clone();
            let editor = editor.clone();
            let current_file = current_file.clone();
            let is_modified = is_modified.clone();
            let settings = settings.clone();
            let update_title = update_title.clone();
            let refresh_recents = refresh_recents.clone();
            let toast = toast.clone();
            move |_: &gio::SimpleAction, _: Option<&glib::Variant>| {
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

        // --- Editor cambiado: preview + índice con debounce ---
        {
            let preview = preview.clone();
            let preview_visible = preview_visible.clone();
            let toc_list = toc_list.clone();
            let toc_lines = toc_lines.clone();
            let is_modified = is_modified.clone();
            let update_title = update_title.clone();
            let generation = Rc::new(Cell::new(0u64));
            editor.connect_changed(move |text| {
                let was_modified = is_modified.replace(true);
                if !was_modified {
                    update_title();
                }
                // Antes se re-renderizaba el preview entero en cada pulsación.
                let gen = generation.get().wrapping_add(1);
                generation.set(gen);

                let text = text.to_string();
                let generation = generation.clone();
                let preview = preview.clone();
                let preview_visible = preview_visible.clone();
                let toc_list = toc_list.clone();
                let toc_lines = toc_lines.clone();
                glib::timeout_add_local_once(Duration::from_millis(120), move || {
                    if generation.get() != gen {
                        return;
                    }
                    if preview_visible.get() {
                        preview.update(&text);
                    }
                    rebuild_toc(&toc_list, &toc_lines, &text);
                });
            });
        }

        // --- Barra lateral ---
        {
            let overlay_split = overlay_split.clone();
            let settings = settings.clone();
            action_toggle_sidebar.connect_activate(move |_, _| {
                let next = !overlay_split.shows_sidebar();
                overlay_split.set_show_sidebar(next);
                settings.set_show_sidebar(next);
            });
        }

        // --- Previsualización ---
        // La acción sólo conmuta el botón; el botón es quien aplica el cambio.
        // (Antes la acción leía el estado del botón, así que el atajo de teclado
        //  no hacía nada: releía el mismo valor y lo volvía a aplicar.)
        {
            let preview_btn = preview_btn.clone();
            action_toggle_preview.connect_activate(move |_, _| {
                preview_btn.set_active(!preview_btn.is_active());
            });
        }
        {
            let paned = paned.clone();
            let preview = preview.clone();
            let preview_visible = preview_visible.clone();
            let editor = editor.clone();
            let settings = settings.clone();
            preview_btn.connect_toggled(move |btn| {
                let active = btn.is_active();
                preview_visible.set(active);
                if active {
                    preview.update(&editor.text());
                    paned.set_end_child(Some(&preview.widget));
                } else {
                    paned.set_end_child(None::<&gtk4::Widget>);
                }
                settings.set_show_preview(active);
            });
        }

        // --- Recientes e índice clicables ---
        {
            let recents = recents.clone();
            let load_file = load_file.clone();
            notes_list.connect_row_activated(move |_, row| {
                let idx = row.index() as usize;
                let path = recents.borrow().get(idx).cloned();
                if let Some(path) = path {
                    load_file(&path);
                }
            });
        }
        {
            let toc_lines = toc_lines.clone();
            let editor = editor.clone();
            toc_list.connect_row_activated(move |_, row| {
                let idx = row.index() as usize;
                let line = toc_lines.borrow().get(idx).copied();
                if let Some(line) = line {
                    editor.scroll_to_line(line);
                }
            });
        }
        {
            let refresh_recents = refresh_recents.clone();
            search_entry.connect_search_changed(move |entry| {
                refresh_recents(entry.text().as_str());
            });
        }

        // --- Estadísticas ---
        {
            let editor = editor.clone();
            let stats_label = stats_label.clone();
            stats_btn.connect_active_notify(move |btn| {
                if !btn.is_active() {
                    return;
                }
                let text = editor.text();
                let words = text.split_whitespace().count();
                let chars = text.chars().count();
                let lines = text.lines().count();
                // ~200 palabras por minuto de lectura.
                let minutes = (words as f64 / 200.0).ceil().max(1.0) as usize;
                stats_label.set_label(&format!(
                    "Palabras: {words}\nCaracteres: {chars}\nLíneas: {lines}\nLectura: ~{minutes} min"
                ));
            });
        }

        // --- Preferencias (ahora sí escriben en GSettings) ---
        {
            let window = window.clone();
            let settings = settings.clone();
            let editor = editor.clone();
            let preview = preview.clone();
            action_preferences.connect_activate(move |_, _| {
                let prefs = adw::PreferencesWindow::builder()
                    .transient_for(&window)
                    .modal(true)
                    .title("Preferencias")
                    .build();

                let page = adw::PreferencesPage::builder()
                    .title("General")
                    .icon_name("preferences-system-symbolic")
                    .build();
                let group = adw::PreferencesGroup::builder().title("Editor").build();

                let font_row = adw::SpinRow::builder()
                    .title("Tamaño de fuente")
                    .subtitle("Píxeles")
                    .adjustment(&gtk4::Adjustment::new(
                        settings.font_size() as f64,
                        8.0,
                        48.0,
                        1.0,
                        1.0,
                        0.0,
                    ))
                    .build();
                group.add(&font_row);

                let spacing_row = adw::SpinRow::builder()
                    .title("Interlineado")
                    .adjustment(&gtk4::Adjustment::new(
                        settings.line_spacing(),
                        1.0,
                        3.0,
                        0.1,
                        0.1,
                        0.0,
                    ))
                    .digits(1)
                    .build();
                group.add(&spacing_row);

                let autosave_row = adw::SwitchRow::builder()
                    .title("Guardado automático")
                    .subtitle("Cada 30 segundos, si el documento ya tiene ruta")
                    .active(settings.autosave())
                    .build();
                group.add(&autosave_row);

                let apply = {
                    let settings = settings.clone();
                    let editor = editor.clone();
                    let preview = preview.clone();
                    let font_row = font_row.clone();
                    let spacing_row = spacing_row.clone();
                    Rc::new(move || {
                        let size = font_row.value() as i32;
                        let spacing = spacing_row.value();
                        settings.set_font_size(size);
                        settings.set_line_spacing(spacing);
                        editor.set_style(size, spacing);
                        preview.set_font_size(size);
                    })
                };
                {
                    let apply = apply.clone();
                    font_row.connect_value_notify(move |_| apply());
                }
                {
                    let apply = apply.clone();
                    spacing_row.connect_value_notify(move |_| apply());
                }
                {
                    let settings = settings.clone();
                    autosave_row.connect_active_notify(move |row| {
                        settings.set_autosave(row.is_active());
                    });
                }

                page.add(&group);
                prefs.add(&page);
                prefs.present();
            });
        }

        // --- Acerca de ---
        {
            let window = window.clone();
            action_about.connect_activate(move |_, _| {
                let about = adw::AboutWindow::builder()
                    .transient_for(&window)
                    .modal(true)
                    .application_name("Scribe")
                    .application_icon("app.scribe.Scribe")
                    .developer_name("gnacho")
                    .version(env!("CARGO_PKG_VERSION"))
                    .website("https://github.com/gnacho/scribe")
                    .issue_url("https://github.com/gnacho/scribe/issues")
                    // El proyecto es AGPL-3.0 (LICENSE y Cargo.toml); antes aquí
                    // ponía GPL-3.0 y no coincidía.
                    .license_type(gtk4::License::Agpl30)
                    .build();
                about.present();
            });
        }

        // --- Autoguardado ---
        {
            let settings = settings.clone();
            let editor = editor.clone();
            let current_file = current_file.clone();
            let is_modified = is_modified.clone();
            let update_title = update_title.clone();
            let alive = alive.clone();
            glib::timeout_add_seconds_local(30, move || {
                if !alive.get() {
                    return glib::ControlFlow::Break;
                }
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

        // --- Cierre: guardar tamaño y avisar de cambios sin guardar ---
        {
            let settings = settings.clone();
            let is_modified = is_modified.clone();
            let force_close = force_close.clone();
            let alive = alive.clone();
            let current_file = current_file.clone();
            let editor = editor.clone();
            let action_save_as = action_save_as.clone();
            window.connect_close_request(move |win| {
                let (w, h) = win.default_size();
                settings.set_window_width(w);
                settings.set_window_height(h);

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
                let action_save_as = action_save_as.clone();
                dialog.choose(gio::Cancellable::NONE, move |response| {
                    match response.as_str() {
                        "discard" => {
                            force_close.set(true);
                            win.close();
                        }
                        "save" => {
                            let path = current_file.borrow().clone();
                            match path {
                                Some(p) => {
                                    if std::fs::write(&p, editor.text()).is_ok() {
                                        force_close.set(true);
                                        win.close();
                                    }
                                }
                                // Sin ruta hace falta el diálogo de guardar,
                                // que es asíncrono: se deja la ventana abierta.
                                None => action_save_as.activate(None),
                            }
                        }
                        _ => {}
                    }
                });

                glib::Propagation::Stop
            });
        }

        update_title();

        Self { window, load_file }
    }

    pub fn open_path(&self, path: &Path) {
        (self.load_file)(path);
    }

    pub fn present(&self) {
        self.window.present();
    }
}
