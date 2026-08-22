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

/// Factoría de documentos: texto inicial y fichero asociado (si lo hay).
/// Devuelve el documento creado, ya insertado en el TabView y seleccionado.
type CreateDocument = Rc<dyn Fn(String, Option<PathBuf>) -> Rc<Document>>;

/// Un documento abierto: su editor, su estado de guardado y su pestaña.
/// La ventana guarda `RefCell<Vec<Rc<Document>>>` y localiza cada documento
/// por el puntero de su `page`.
struct Document {
    editor: Rc<Editor>,
    current_file: RefCell<Option<PathBuf>>,
    is_modified: Cell<bool>,
    /// Segundos acumulados hacia el próximo autoguardado (D4: por documento).
    autosave_elapsed: Cell<i32>,
    /// true tras un fallo de autoguardado ya notificado (un aviso por racha).
    autosave_failed: Cell<bool>,
    /// Nombre base cacheado (fichero o primera cabecera del borrador); solo
    /// se recalcula para la pestaña activa, para no leer el texto de todas
    /// en cada pulsación.
    display_name: RefCell<String>,
    /// true mientras hay un dialogo de cierre pendiente para esta pestaña:
    /// evita dialogos duplicados si se pide cerrarla dos veces.
    closing: Cell<bool>,
    page: adw::TabPage,
}

/// Remate del cierre de una pestaña: confirma/aborta y limpia (ver
/// `finish_close` en `ScribeWindow::new`).
type FinishCloseFn = Rc<dyn Fn(&adw::TabPage, bool)>;

pub struct ScribeWindow {
    pub window: adw::ApplicationWindow,
    load_file: Rc<dyn Fn(&Path)>,
    /// Ancla fuerte de la lista de documentos: los closures registrados en
    /// objetos de la ventana la capturan en Weak (R3), así que alguien tiene
    /// que poseerla. La poseen este struct (que main retiene en su registro
    /// de ventanas) y el temporizador de autoguardado. El binario no la lee
    /// directamente (solo los smoke tests via page_count/document_count),
    /// pero su presencia es estructural.
    #[allow(dead_code)]
    documents: Rc<RefCell<Vec<Rc<Document>>>>,
    /// Igual que `documents`: ancla del TabView y superficie de los tests.
    #[allow(dead_code)]
    tab_view: adw::TabView,
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

/// Ruta canónica para deduplicar aperturas (R5/D6); si no se puede
/// canonicalizar (aún no existe) se usa la ruta tal cual.
fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn file_name_of(path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("Sin título")
        .to_string()
}

/// La geometría solo se persiste cuando el cierre de la ventana prospera
/// (antes se guardaba aunque el usuario cancelara el diálogo de cambios).
fn persist_geometry(win: &adw::ApplicationWindow, settings: &AppSettings) {
    let (w, h) = win.default_size();
    settings.set_window_width(w);
    settings.set_window_height(h);
    settings.set_window_maximized(win.is_maximized());
}

/// Aplica las preferencias a un editor concreto. La usan tanto
/// `apply_settings` (iterando todos los documentos) como la factoría de
/// documentos (solo el editor recién creado).
fn apply_editor_settings(settings: &AppSettings, editor: &Editor) {
    let size = settings.font_size();
    editor.set_font(settings.font_family(), size, settings.line_spacing());
    editor.set_column_width(settings.column_width());
    editor.set_markup_visibility(settings.markup_visibility());
    editor.set_focus_mode(settings.focus_mode());
    editor.set_typewriter_mode(settings.typewriter_mode());
    editor.set_continue_lists(settings.continue_lists());
    editor.set_tab_width(settings.tab_width());
}

/// Refresca título y tooltip de la pestaña de `doc` (D3: «• » si está
/// modificado; needs_attention solo en las pestañas NO activas) y, si es la
/// activa, el título de la cabecera y de la ventana.
fn refresh_doc_tab(
    doc: &Document,
    window: &adw::ApplicationWindow,
    title_widget: &adw::WindowTitle,
    is_active: bool,
) {
    let file = doc.current_file.borrow();
    let (name, subtitle) = match file.as_ref() {
        Some(p) => (
            file_name_of(p),
            p.parent().map(shorten_home).unwrap_or_default(),
        ),
        // Sin fichero, el borrador se identifica por su primera cabecera,
        // que es lo que el usuario tiene en la cabeza. El texto solo se lee
        // para la pestaña activa; el resto usa el nombre cacheado.
        None if is_active => (
            templates::title_from(&doc.editor.text()).unwrap_or_else(|| "Sin título".to_string()),
            "Borrador".to_string(),
        ),
        None => (doc.display_name.borrow().clone(), "Borrador".to_string()),
    };
    let tooltip = match file.as_ref() {
        Some(p) => p.to_string_lossy().to_string(),
        None => "Borrador sin guardar".to_string(),
    };
    drop(file);
    if is_active {
        *doc.display_name.borrow_mut() = name.clone();
    }
    let dot = if doc.is_modified.get() { "• " } else { "" };
    doc.page.set_title(&format!("{dot}{name}"));
    doc.page.set_tooltip(&tooltip);
    doc.page
        .set_needs_attention(!is_active && doc.is_modified.get());
    if is_active {
        title_widget.set_title(&format!("{dot}{name}"));
        title_widget.set_subtitle(&subtitle);
        window.set_title(Some(&format!("{dot}{name} — Scribe")));
    }
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

/// Un paso de la cadena de diálogos «Guardar/Descartar/Cancelar» del cierre
/// de ventana (como GNOME Text Editor): se muestra uno por documento
/// modificado. Cancelar —o un guardado fallido— aborta toda la secuencia;
/// al resolverlos todos se invoca `on_done` (que persiste la geometría y
/// cierra de verdad).
fn ask_save_step(
    win: adw::ApplicationWindow,
    pending: Rc<RefCell<Vec<Rc<Document>>>>,
    file_manager: Rc<FileManager>,
    toast: Rc<dyn Fn(&str)>,
    on_done: Rc<dyn Fn(&adw::ApplicationWindow)>,
    // Se invoca cuando la cadena se aborta (Cancelar o guardado fallido):
    // libera la guarda de cierre de la ventana.
    on_abort: Rc<dyn Fn()>,
) {
    let Some(doc) = pending.borrow_mut().pop() else {
        on_done(&win);
        return;
    };
    let name = doc.display_name.borrow().clone();
    let dialog = adw::MessageDialog::new(
        Some(&win),
        Some(&format!("¿Guardar los cambios en «{name}»?")),
        Some("Si cierras sin guardar, perderás lo que hayas escrito."),
    );
    dialog.add_response("cancel", "Cancelar");
    dialog.add_response("discard", "Descartar");
    dialog.add_response("save", "Guardar");
    dialog.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
    dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("save"));
    dialog.set_close_response("cancel");

    // Los diálogos son one-shot y acotados: pueden capturar en fuerte (R3).
    dialog.choose(gio::Cancellable::NONE, move |response| {
        match response.as_str() {
            "discard" => {
                ask_save_step(win, pending, file_manager, toast, on_done, on_abort.clone())
            }
            "save" => {
                let path = doc.current_file.borrow().clone();
                match path {
                    Some(p) => match std::fs::write(&p, doc.editor.text()) {
                        Ok(()) => {
                            doc.is_modified.set(false);
                            ask_save_step(
                                win,
                                pending,
                                file_manager,
                                toast,
                                on_done,
                                on_abort.clone(),
                            );
                        }
                        Err(e) => {
                            toast(&format!("No se pudo guardar: {e}"));
                            on_abort();
                        }
                    },
                    // Documento sin fichero: se abre «Guardar como»; si el
                    // usuario cancela ahí, se aborta todo el cierre.
                    None => {
                        // El callback de «Guardar como» es otro closure: hay
                        // que clonar lo que ambos necesitan (no se puede
                        // mover lo capturado por el closure exterior, Fn).
                        let win2 = win.clone();
                        let pending2 = pending.clone();
                        let fm2 = file_manager.clone();
                        let toast2 = toast.clone();
                        let on_done2 = on_done.clone();
                        let on_abort2 = on_abort.clone();
                        file_manager.save(&win, None, &doc.editor.text(), move |outcome| {
                            match outcome {
                                Outcome::Ok(p) => {
                                    doc.editor.set_base_dir(p.parent().map(Path::to_path_buf));
                                    *doc.current_file.borrow_mut() = Some(p);
                                    doc.is_modified.set(false);
                                    ask_save_step(
                                        win2,
                                        pending2,
                                        fm2,
                                        toast2,
                                        on_done2,
                                        on_abort2.clone(),
                                    );
                                }
                                Outcome::Error(e) => {
                                    toast2(&format!("No se pudo guardar: {e}"));
                                    on_abort2();
                                }
                                // Cancelar el «Guardar como» aborta toda la
                                // cadena de cierre.
                                Outcome::Cancelled => on_abort2(),
                            }
                        });
                    }
                }
            }
            // Cancelar: se aborta toda la cadena; la ventana sigue abierta.
            _ => on_abort(),
        }
    });
}

impl ScribeWindow {
    pub fn new(app: &adw::Application, settings: &Rc<AppSettings>) -> Self {
        let settings = settings.clone();
        let file_manager = Rc::new(FileManager::new());
        let documents: Rc<RefCell<Vec<Rc<Document>>>> = Rc::new(RefCell::new(Vec::new()));
        let force_close = Rc::new(Cell::new(false));
        let alive = Rc::new(Cell::new(true));
        // Guarda de «cierre de ventana en curso»: sin ella, pulsar dos veces
        // el boton de cerrar con dialogos pendientes arrancaba dos cadenas
        // de «Guardar/Descartar/Cancelar» sobre los mismos documentos.
        let closing_window = Rc::new(Cell::new(false));
        let toc_lines: Rc<RefCell<Vec<i32>>> = Rc::new(RefCell::new(Vec::new()));

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .default_width(settings.window_width())
            .default_height(settings.window_height())
            .build();
        if settings.window_maximized() {
            window.maximize();
        }

        // Vista previa compartida (D1/D2): hay UNA por ventana, a la derecha
        // de todas las pestañas, no dentro de ninguna página.
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

        // ============================ PESTAÑAS ===============================
        // Patrón gnome-text-editor: cada página del AdwTabView es el editor
        // de un documento; el AdwTabBar va como top bar del ToolbarView.
        let tab_view = adw::TabView::new();
        tab_view.set_hexpand(true);
        tab_view.set_vexpand(true);
        let tab_bar = adw::TabBar::builder()
            .view(&tab_view)
            .autohide(false)
            .expand_tabs(true)
            .build();

        // ============================== CONTENIDO ==============================
        let paned = gtk4::Paned::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .start_child(&tab_view)
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
        file_section.append(Some("Cerrar pestaña"), Some("win.close-tab"));
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
        toolbar_view.add_top_bar(&tab_bar);
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

        // Resuelve el documento de la pestaña activa. Las acciones de
        // formato/guardado lo llaman en cada activación (D10/R7): ninguna
        // captura un editor concreto. Débil hacia la lista de documentos:
        // este closure acaba registrado en acciones y señales de la propia
        // ventana y en fuerte sería un ciclo (R3).
        let active_document: Rc<dyn Fn() -> Option<Rc<Document>>> = {
            let documents = Rc::downgrade(&documents);
            // Debil: update_status/update_counts lo capturan y estan
            // registrados en el propio TabView y en los buffers (R3).
            let tab_view = tab_view.downgrade();
            Rc::new(move || {
                let docs = documents.upgrade()?;
                let tab_view = tab_view.upgrade()?;
                let page = tab_view.selected_page()?;
                // En una expresion de cola el Ref temporal vivira mas que
                // `docs`; se fuerza su caida antes con una variable local.
                let found = docs.borrow().iter().find(|d| d.page == page).cloned();
                found
            })
        };

        // Refresca el título de TODAS las pestañas (punto «• », tooltip y
        // needs_attention, también en las no activas) y el título de la
        // cabecera y la ventana con la pestaña activa.
        let refresh_tabs: Rc<dyn Fn()> = {
            // Débiles: véase active_document. tab_view tambien va en debil:
            // este closure lo capturan las senales DEL PROPIO TabView y en
            // fuerte seria un ciclo (R3, revision de pestañas).
            let window = window.downgrade();
            let documents = Rc::downgrade(&documents);
            let title_widget = title_widget.clone();
            let tab_view = tab_view.downgrade();
            Rc::new(move || {
                let (Some(window), Some(docs), Some(tab_view)) =
                    (window.upgrade(), documents.upgrade(), tab_view.upgrade())
                else {
                    return;
                };
                let selected = tab_view.selected_page();
                for doc in docs.borrow().iter() {
                    let is_active = selected.as_ref() == Some(&doc.page);
                    refresh_doc_tab(doc, &window, &title_widget, is_active);
                }
            })
        };

        // Lo barato (posición del cursor) se actualiza en cada evento; los
        // contadores de palabras/caracteres son O(n) y se calculan dentro del
        // timeout ya debounced de connect_changed, reutilizando la copia del
        // texto que se hace para la vista previa y el índice.
        let update_counts: Rc<dyn Fn(&str)> = {
            let active_document = active_document.clone();
            let words_label = words_label.clone();
            let props_label = props_label.clone();
            Rc::new(move |text: &str| {
                let Some(doc) = active_document() else {
                    return;
                };
                let words = text.split_whitespace().count();
                let chars = text.chars().count();
                let lines = doc.editor.line_count();
                words_label.set_label(&format!("{words} palabras"));
                let minutes = (words as f64 / 200.0).ceil().max(1.0) as usize;
                props_label.set_label(&format!(
                    "Palabras: {words}\nCaracteres: {chars}\nLíneas: {lines}\nLectura: ~{minutes} min"
                ));
            })
        };

        let update_status: Rc<dyn Fn()> = {
            let active_document = active_document.clone();
            let position_button = position_button.clone();
            let goto_spin = goto_spin.clone();
            let goto_popover = goto_popover.clone();
            Rc::new(move || {
                let Some(doc) = active_document() else {
                    return;
                };
                let lines = doc.editor.line_count();
                let (line, column) = doc.editor.cursor_position();
                position_button.set_label(&format!("Ln {line}, Col {column}"));
                // Con el popover «Ir a la línea» abierto el spin es del
                // usuario: no pisar lo que está escribiendo.
                if !goto_popover.is_visible() {
                    goto_spin.set_range(1.0, lines.max(1) as f64);
                    goto_spin.set_value(line as f64);
                }
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

        // ---- aplicar preferencias ----
        let apply_settings: Rc<dyn Fn()> = {
            let settings = settings.clone();
            let documents = Rc::downgrade(&documents);
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
                // Las preferencias del editor son globales: se aplican a
                // todos los documentos abiertos.
                if let Some(docs) = documents.upgrade() {
                    for doc in docs.borrow().iter() {
                        apply_editor_settings(&settings, &doc.editor);
                    }
                }
                preview.set_font_size(settings.font_size());
                zoom_label.set_label(&format!("{}px", settings.font_size()));
                refresh_recents("");
                refresh_templates();
            })
        };
        apply_settings();

        // ======================= FACTORÍA DE DOCUMENTOS ======================
        // Crea el editor, le aplica las preferencias, monta su pestaña y
        // cablea sus señales. Todo closure registrado en el editor o en la
        // ventana captura Weak del Document o de la lista (R3); los timeouts
        // one-shot acotados pueden capturar en fuerte.
        let create_document: CreateDocument = {
            let settings = settings.clone();
            let documents = Rc::downgrade(&documents);
            // Debil: finish_close (que lo captura) vive en una senal del
            // propio TabView; en fuerte seria un ciclo (R3).
            let tab_view = tab_view.downgrade();
            let preview = preview.clone();
            let preview_visible = preview_visible.clone();
            let toc_list = toc_list.clone();
            let toc_lines = toc_lines.clone();
            let refresh_tabs = refresh_tabs.clone();
            let update_status = update_status.clone();
            let update_counts = update_counts.clone();
            Rc::new(move |body: String, file: Option<PathBuf>| {
                // Solo puede fallar con la ventana destruyendose, cuando
                // nadie deberia crear documentos.
                let tab_view = tab_view.upgrade().expect("ventana viva al crear documento");
                let editor = Rc::new(Editor::new());
                apply_editor_settings(&settings, &editor);
                if !body.is_empty() {
                    editor.set_text(&body);
                }
                // Documento sin fichero: las imágenes relativas no se
                // resuelven y se pinta el placeholder.
                editor.set_base_dir(
                    file.as_ref()
                        .and_then(|p| p.parent().map(Path::to_path_buf)),
                );

                let page = tab_view.append(&editor.widget);
                let display_name = match &file {
                    Some(p) => file_name_of(p),
                    None => {
                        templates::title_from(&body).unwrap_or_else(|| "Sin título".to_string())
                    }
                };
                let doc = Rc::new(Document {
                    editor,
                    current_file: RefCell::new(file),
                    is_modified: Cell::new(false),
                    autosave_elapsed: Cell::new(0),
                    autosave_failed: Cell::new(false),
                    display_name: RefCell::new(display_name),
                    closing: Cell::new(false),
                    page,
                });

                // ---- cableado por documento ----
                {
                    // Débil: Document → Editor → callback → Document sería un
                    // ciclo y ni la pestaña ni el editor se liberarían jamás.
                    let doc_weak = Rc::downgrade(&doc);
                    // Debil: este handler vive en el buffer, que cuelga del
                    // propio TabView; en fuerte seria un ciclo (R3).
                    let tab_view = tab_view.downgrade();
                    let refresh_tabs = refresh_tabs.clone();
                    let update_status = update_status.clone();
                    let update_counts = update_counts.clone();
                    let preview = preview.clone();
                    let preview_visible = preview_visible.clone();
                    let toc_list = toc_list.clone();
                    let toc_lines = toc_lines.clone();
                    let generation = Rc::new(Cell::new(0u64));
                    doc.editor.connect_changed(move |text| {
                        let Some(doc) = doc_weak.upgrade() else {
                            return;
                        };
                        if !doc.is_modified.replace(true) {
                            // Primera modificación: actualiza el «• » y el
                            // needs_attention de todas las pestañas.
                            refresh_tabs();
                        }
                        // Contadores, TOC y vista previa solo siguen a la
                        // pestaña activa.
                        let Some(tab_view) = tab_view.upgrade() else {
                            return;
                        };
                        if tab_view.selected_page().as_ref() != Some(&doc.page) {
                            return;
                        }
                        update_status();

                        let current = generation.get().wrapping_add(1);
                        generation.set(current);
                        let text = text.to_string();
                        let generation = generation.clone();
                        let doc_weak = doc_weak.clone();
                        let tab_view = tab_view.clone();
                        let preview = preview.clone();
                        let preview_visible = preview_visible.clone();
                        let toc_list = toc_list.clone();
                        let toc_lines = toc_lines.clone();
                        let update_counts = update_counts.clone();
                        glib::timeout_add_local_once(Duration::from_millis(120), move || {
                            if generation.get() != current {
                                return;
                            }
                            let Some(doc) = doc_weak.upgrade() else {
                                return;
                            };
                            // Si mientras tanto se cambió de pestaña, este
                            // refresco ya no interesa.
                            if tab_view.selected_page().as_ref() != Some(&doc.page) {
                                return;
                            }
                            update_counts(&text);
                            if preview_visible.get() {
                                preview.update(&text);
                            }
                            rebuild_toc(&toc_list, &toc_lines, &text);
                        });
                    });
                }
                {
                    let update_status = update_status.clone();
                    doc.editor.connect_cursor_moved(move || update_status());
                }

                if let Some(docs) = documents.upgrade() {
                    docs.borrow_mut().push(doc.clone());
                }
                tab_view.set_selected_page(&doc.page);
                refresh_tabs();
                update_status();
                let text = doc.editor.text();
                update_counts(&text);
                if preview_visible.get() {
                    preview.update(&text);
                }
                rebuild_toc(&toc_list, &toc_lines, &text);
                doc
            })
        };

        // Inserta un fichero ya leído: deduplica por ruta canónica (D6/R5),
        // reutiliza la pestaña virgen si es la única y crea pestaña nueva en
        // cualquier otro caso.
        let open_loaded: Rc<dyn Fn(PathBuf, String)> = {
            let documents = Rc::downgrade(&documents);
            let tab_view = tab_view.clone();
            let settings = settings.clone();
            let create_document = create_document.clone();
            let refresh_recents = refresh_recents.clone();
            let refresh_tabs = refresh_tabs.clone();
            let update_status = update_status.clone();
            Rc::new(move |path: PathBuf, content: String| {
                let Some(docs) = documents.upgrade() else {
                    return;
                };
                let canon = canonical(&path);
                let existing = docs
                    .borrow()
                    .iter()
                    .find(|d| {
                        d.current_file
                            .borrow()
                            .as_ref()
                            .is_some_and(|p| canonical(p) == canon)
                    })
                    .cloned();
                if let Some(existing) = existing {
                    tab_view.set_selected_page(&existing.page);
                    return;
                }

                // Si solo hay una pestaña virgen (sin fichero, sin cambios y
                // vacía) se reutiliza en vez de abrir otra, como hace gedit.
                let pristine = {
                    let list = docs.borrow();
                    match list.as_slice() {
                        [d] if d.current_file.borrow().is_none()
                            && !d.is_modified.get()
                            && d.editor.text().is_empty() =>
                        {
                            Some(d.clone())
                        }
                        _ => None,
                    }
                };
                if let Some(doc) = pristine {
                    doc.editor.set_text(&content);
                    doc.editor
                        .set_base_dir(path.parent().map(Path::to_path_buf));
                    *doc.current_file.borrow_mut() = Some(path.clone());
                    doc.is_modified.set(false);
                    *doc.display_name.borrow_mut() = file_name_of(&path);
                    tab_view.set_selected_page(&doc.page);
                } else {
                    create_document(content, Some(path.clone()));
                }
                settings.push_recent_file(&path.to_string_lossy());
                refresh_recents("");
                refresh_tabs();
                update_status();
            })
        };

        let load_file: Rc<dyn Fn(&Path)> = {
            let open_loaded = open_loaded.clone();
            let toast = toast.clone();
            Rc::new(move |path: &Path| match std::fs::read_to_string(path) {
                Ok(content) => open_loaded(path.to_path_buf(), content),
                Err(e) => toast(&format!("No se pudo abrir: {e}")),
            })
        };

        let new_document: NewDocument = {
            let create_document = create_document.clone();
            Rc::new(move |template_name: Option<&str>| {
                let body = template_name
                    .and_then(templates::find)
                    .and_then(|t| t.body())
                    .map(|b| templates::render(&b, "Sin título"))
                    .unwrap_or_default();
                create_document(body, None);
            })
        };

        // Remata el cierre de una pestaña pedido con close_page: confirma o
        // aborta, limpia la lista de documentos y aplica D5 (al cerrar la
        // última pestaña se abre una en blanco; la ventana solo se cierra
        // por cierre de ventana).
        let finish_close: FinishCloseFn = {
            let documents = Rc::downgrade(&documents);
            // Debil: este closure lo captura la senal close-page del propio
            // TabView; en fuerte seria un ciclo (R3).
            let tab_view = tab_view.downgrade();
            let create_document = create_document.clone();
            Rc::new(move |page, confirm| {
                let Some(tab_view) = tab_view.upgrade() else {
                    return;
                };
                tab_view.close_page_finish(page, confirm);
                if !confirm {
                    return;
                }
                if let Some(docs) = documents.upgrade() {
                    docs.borrow_mut().retain(|d| d.page != *page);
                }
                if tab_view.n_pages() == 0 {
                    create_document(String::new(), None);
                }
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
        let action_close_tab = gio::SimpleAction::new("close-tab", None);
        let action_tab_next = gio::SimpleAction::new("tab-next", None);
        let action_tab_prev = gio::SimpleAction::new("tab-prev", None);

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
            &action_close_tab,
            &action_tab_next,
            &action_tab_prev,
        ] {
            window.add_action(a);
        }
        window.add_action(&action_open_recent);
        window.add_action(&action_new_template);
        window.add_action(&action_sidebar);
        window.add_action(&action_preview);
        window.add_action(&action_focus);
        window.add_action(&action_typewriter);

        // ---- formato: siempre sobre la pestaña activa, resuelta al vuelo ----
        for (action, marker) in [
            (&action_bold, "**"),
            (&action_italic, "*"),
            (&action_code, "`"),
        ] {
            let active_document = active_document.clone();
            action.connect_activate(move |_, _| {
                if let Some(doc) = active_document() {
                    doc.editor.wrap_selection(marker);
                }
            });
        }

        {
            let active_document = active_document.clone();
            let toast = toast.clone();
            action_format_tables.connect_activate(move |_, _| {
                let Some(doc) = active_document() else {
                    return;
                };
                if doc.editor.format_tables() {
                    toast("Tablas alineadas");
                } else {
                    toast("No hay tablas que alinear");
                }
            });
        }

        // ---- pestañas ----
        {
            let tab_view = tab_view.clone();
            action_close_tab.connect_activate(move |_, _| {
                if let Some(page) = tab_view.selected_page() {
                    // close_page dispara la señal close-page, donde se
                    // confirma o se veta según haya cambios.
                    tab_view.close_page(&page);
                }
            });
        }
        {
            let tab_view = tab_view.clone();
            action_tab_next.connect_activate(move |_, _| {
                tab_view.select_next_page();
            });
        }
        {
            let tab_view = tab_view.clone();
            action_tab_prev.connect_activate(move |_, _| {
                tab_view.select_previous_page();
            });
        }

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
            let open_loaded = open_loaded.clone();
            let toast = toast.clone();
            action_open.connect_activate(move |_, _| {
                let Some(window) = window.upgrade() else {
                    return;
                };
                let open_loaded = open_loaded.clone();
                let toast = toast.clone();
                file_manager.open(&window, move |outcome| match outcome {
                    OpenOutcome::Ok((path, content)) => open_loaded(path, content),
                    OpenOutcome::Error(e) => toast(&e),
                    OpenOutcome::Cancelled => {}
                });
            });
        }

        let make_save = |force_dialog: bool| {
            // Débil: véase action_preferences.
            let window = window.downgrade();
            let file_manager = file_manager.clone();
            let active_document = active_document.clone();
            let documents = Rc::downgrade(&documents);
            let tab_view = tab_view.downgrade();
            let settings = settings.clone();
            let refresh_recents = refresh_recents.clone();
            let refresh_tabs = refresh_tabs.clone();
            let toast = toast.clone();
            move |_: &gio::SimpleAction, _: Option<&glib::Variant>| {
                let Some(window) = window.upgrade() else {
                    return;
                };
                // D10/R7: el documento se resuelve en cada activación.
                let Some(doc) = active_document() else {
                    return;
                };
                let content = doc.editor.text();
                let path = if force_dialog {
                    None
                } else {
                    doc.current_file.borrow().clone()
                };
                let settings = settings.clone();
                let refresh_recents = refresh_recents.clone();
                let refresh_tabs = refresh_tabs.clone();
                let toast = toast.clone();
                // Clones del Weak para el callback one-shot (no se puede
                // mover lo capturado por este closure Fn).
                let documents = documents.clone();
                let tab_view = tab_view.clone();
                file_manager.save(
                    &window,
                    path.as_ref(),
                    &content,
                    // Callback one-shot y acotado: puede capturar el
                    // documento en fuerte.
                    move |outcome| match outcome {
                        Outcome::Ok(p) => {
                            // R5 ampliado (revision): «Guardar como» sobre la
                            // ruta de OTRA pestaña abierta crearia dos
                            // buffers para el mismo fichero y el
                            // autoguardado los haria pisarse. Se aborta y se
                            // enfoca la pestaña que ya lo tiene.
                            if let (Some(docs), Some(tv)) =
                                (documents.upgrade(), tab_view.upgrade())
                            {
                                let canon_new = canonical(&p);
                                let otro = docs
                                    .borrow()
                                    .iter()
                                    .find(|d| {
                                        !Rc::ptr_eq(d, &doc)
                                            && d.current_file
                                                .borrow()
                                                .as_ref()
                                                .map(|q| canonical(q))
                                                .as_ref()
                                                == Some(&canon_new)
                                    })
                                    .cloned();
                                if let Some(otro) = otro {
                                    // Limitacion conocida: FileManager::save
                                    // ya escribio el fichero al llegar aqui
                                    // (el usuario confirmo la sobrescritura
                                    // en el dialogo); lo que se evita es el
                                    // estado persistente de dos buffers para
                                    // un mismo fichero.
                                    tv.set_selected_page(&otro.page);
                                    toast("Ese fichero ya está abierto en otra pestaña");
                                    return;
                                }
                            }
                            // Tras «guardar como» el directorio base de las
                            // imágenes puede haber cambiado.
                            doc.editor.set_base_dir(p.parent().map(Path::to_path_buf));
                            *doc.current_file.borrow_mut() = Some(p.clone());
                            doc.is_modified.set(false);
                            *doc.display_name.borrow_mut() = file_name_of(&p);
                            settings.push_recent_file(&p.to_string_lossy());
                            refresh_recents("");
                            // Actualiza título y tooltip (ruta) de la pestaña.
                            refresh_tabs();
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
            let active_document = active_document.clone();
            let settings = settings.clone();
            action_preview.connect_activate(move |action, _| {
                let next = !preview_visible.get();
                preview_visible.set(next);
                if next {
                    // Vista previa compartida: se alimenta de la activa.
                    if let Some(doc) = active_document() {
                        preview.update(&doc.editor.text());
                    }
                    paned.set_end_child(Some(&preview.widget));
                } else {
                    paned.set_end_child(None::<&gtk4::Widget>);
                }
                action.set_state(&next.to_variant());
                settings.set_show_preview(next);
            });
        }
        for (action, is_focus) in [(&action_focus, true), (&action_typewriter, false)] {
            // Débil: estos modos se aplican a todos los editores.
            let documents = Rc::downgrade(&documents);
            let settings = settings.clone();
            action.connect_activate(move |action, _| {
                let next = !action
                    .state()
                    .and_then(|s| s.get::<bool>())
                    .unwrap_or(false);
                action.set_state(&next.to_variant());
                if is_focus {
                    settings.set_focus_mode(next);
                } else {
                    settings.set_typewriter_mode(next);
                }
                if let Some(docs) = documents.upgrade() {
                    for doc in docs.borrow().iter() {
                        if is_focus {
                            doc.editor.set_focus_mode(next);
                        } else {
                            doc.editor.set_typewriter_mode(next);
                        }
                    }
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
        // ---- cambio de pestaña: refresca título, contadores, posición,
        // TOC, vista previa y quita el needs_attention de la nueva activa ----
        {
            let documents = Rc::downgrade(&documents);
            let refresh_tabs = refresh_tabs.clone();
            let update_status = update_status.clone();
            let update_counts = update_counts.clone();
            let preview = preview.clone();
            let preview_visible = preview_visible.clone();
            let toc_list = toc_list.clone();
            let toc_lines = toc_lines.clone();
            tab_view.connect_selected_page_notify(move |tv| {
                refresh_tabs();
                update_status();
                let (Some(docs), Some(page)) = (documents.upgrade(), tv.selected_page()) else {
                    return;
                };
                let Some(doc) = docs.borrow().iter().find(|d| d.page == page).cloned() else {
                    return;
                };
                let text = doc.editor.text();
                update_counts(&text);
                if preview_visible.get() {
                    preview.update(&text);
                }
                rebuild_toc(&toc_list, &toc_lines, &text);
            });
        }

        // ---- cierre de pestaña (botón de la TabBar o win.close-tab) ----
        {
            // Débiles: esta señal vive en el TabView, que es de la ventana.
            let documents = Rc::downgrade(&documents);
            let window_weak = window.downgrade();
            let file_manager = file_manager.clone();
            let finish_close = finish_close.clone();
            let toast = toast.clone();
            tab_view.connect_close_page(move |_, page| {
                // Veto siempre (Stop): el cierre y su limpieza los rematamos
                // nosotros con close_page_finish. Solo se deja pasar el
                // cierre por defecto si el documento ya no está localizable
                // (ventana destruyéndose), pues entonces no hay nada que
                // limpiar.
                let Some(docs) = documents.upgrade() else {
                    // Inalcanzable en la practica: el TabView no emite
                    // close-page durante la destruccion de la ventana. Si
                    // ocurriera, el cierre por defecto dejaria el Document
                    // huerfano en la lista (no hay retain aqui).
                    return glib::Propagation::Proceed;
                };
                let Some(doc) = docs.borrow().iter().find(|d| &d.page == page).cloned() else {
                    return glib::Propagation::Proceed;
                };
                if !doc.is_modified.get() {
                    finish_close(page, true);
                    return glib::Propagation::Stop;
                }
                if doc.closing.replace(true) {
                    // Ya hay un dialogo de cierre pendiente para esta
                    // pestaña: veto sin abrir otro (el primero remata).
                    return glib::Propagation::Stop;
                }
                let Some(win) = window_weak.upgrade() else {
                    return glib::Propagation::Proceed;
                };

                let name = doc.display_name.borrow().clone();
                let dialog = adw::MessageDialog::new(
                    Some(&win),
                    Some(&format!("¿Guardar los cambios en «{name}»?")),
                    Some("Si cierras sin guardar, perderás lo que hayas escrito."),
                );
                dialog.add_response("cancel", "Cancelar");
                dialog.add_response("discard", "Descartar");
                dialog.add_response("save", "Guardar");
                dialog.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
                dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
                dialog.set_default_response(Some("save"));
                dialog.set_close_response("cancel");

                // El diálogo es one-shot y acotado: capturas en fuerte (R3).
                // Todo lo que el callback mueve debe ser local de esta
                // invocacion: lo capturado por el closure Fn exterior no se
                // puede mover, asi que se clona antes (sombras).
                let page = page.clone();
                let finish_close = finish_close.clone();
                let toast = toast.clone();
                let file_manager = file_manager.clone();
                dialog.choose(gio::Cancellable::NONE, move |response| {
                    match response.as_str() {
                        "discard" => finish_close(&page, true),
                        "save" => {
                            let path = doc.current_file.borrow().clone();
                            match path {
                                Some(p) => match std::fs::write(&p, doc.editor.text()) {
                                    Ok(()) => {
                                        doc.is_modified.set(false);
                                        finish_close(&page, true);
                                    }
                                    // Un guardado fallido aborta el cierre:
                                    // el usuario decide qué hacer después.
                                    Err(e) => {
                                        toast(&format!("No se pudo guardar: {e}"));
                                        // Aborto: liberar la guarda de
                                        // cierre de esta pestaña.
                                        doc.closing.set(false);
                                        finish_close(&page, false);
                                    }
                                },
                                // Documento sin fichero: «Guardar como»; si
                                // se cancela ahí, la pestaña no se cierra.
                                None => {
                                    file_manager.save(
                                        &win,
                                        None,
                                        &doc.editor.text(),
                                        move |outcome| match outcome {
                                            Outcome::Ok(p) => {
                                                doc.editor.set_base_dir(
                                                    p.parent().map(Path::to_path_buf),
                                                );
                                                *doc.current_file.borrow_mut() = Some(p);
                                                doc.is_modified.set(false);
                                                finish_close(&page, true);
                                            }
                                            Outcome::Error(e) => {
                                                toast(&format!("No se pudo guardar: {e}"));
                                                doc.closing.set(false);
                                                finish_close(&page, false);
                                            }
                                            Outcome::Cancelled => {
                                                doc.closing.set(false);
                                                finish_close(&page, false);
                                            }
                                        },
                                    );
                                }
                            }
                        }
                        // Cancelar: se aborta el cierre de la pestaña.
                        _ => {
                            doc.closing.set(false);
                            finish_close(&page, false);
                        }
                    }
                });
                glib::Propagation::Stop
            });
        }

        {
            let toc_lines = toc_lines.clone();
            let active_document = active_document.clone();
            toc_list.connect_row_activated(move |_, row| {
                let line = toc_lines.borrow().get(row.index() as usize).copied();
                if let (Some(line), Some(doc)) = (line, active_document()) {
                    doc.editor.scroll_to_line(line);
                }
            });
        }
        {
            let refresh_recents = refresh_recents.clone();
            search_entry
                .connect_search_changed(move |entry| refresh_recents(entry.text().as_str()));
        }
        {
            let active_document = active_document.clone();
            let goto_spin = goto_spin.clone();
            let goto_popover = goto_popover.clone();
            goto_button.connect_clicked(move |_| {
                if let Some(doc) = active_document() {
                    doc.editor.go_to_line(goto_spin.value() as i32);
                }
                goto_popover.popdown();
            });
        }

        // ---- autoguardado (D4/R1): UN temporizador de ventana que itera
        // todos los documentos; el elapsed es por documento ----
        {
            let settings = settings.clone();
            // Fuerte a propósito: el temporizador NO es un objeto de la
            // ventana (vive en el main context) y junto con ScribeWindow es
            // el ancla de la lista; termina cuando la ventana muere (alive).
            let documents = documents.clone();
            let refresh_tabs = refresh_tabs.clone();
            let toast = toast.clone();
            let alive = alive.clone();
            glib::timeout_add_seconds_local(5, move || {
                if !alive.get() {
                    return glib::ControlFlow::Break;
                }
                if !settings.autosave() {
                    return glib::ControlFlow::Continue;
                }
                // R1: clonar el Vec antes de iterar: puede cambiar durante
                // el tick (guardar → toast → señales → alta/baja de
                // documentos).
                let docs = documents.borrow().clone();
                for doc in docs {
                    doc.autosave_elapsed.set(doc.autosave_elapsed.get() + 5);
                    if doc.autosave_elapsed.get() < settings.autosave_interval() {
                        continue;
                    }
                    doc.autosave_elapsed.set(0);
                    if !doc.is_modified.get() {
                        continue;
                    }
                    let Some(path) = doc.current_file.borrow().clone() else {
                        continue;
                    };
                    match std::fs::write(&path, doc.editor.text()) {
                        Ok(()) => {
                            doc.is_modified.set(false);
                            // También quita el «•»/needs_attention de las
                            // pestañas no activas (D4).
                            refresh_tabs();
                            doc.autosave_failed.set(false);
                        }
                        // Sin este aviso el usuario confía en una copia
                        // automática que no existe.
                        Err(e) => {
                            if !doc.autosave_failed.replace(true) {
                                toast(&format!("No se pudo autoguardar: {e}"));
                            }
                        }
                    }
                }
                glib::ControlFlow::Continue
            });
        }

        // ---- cierre de la ventana con N documentos modificados ----
        {
            let settings = settings.clone();
            let documents = Rc::downgrade(&documents);
            let force_close = force_close.clone();
            let alive = alive.clone();
            let file_manager = file_manager.clone();
            let toast = toast.clone();
            window.connect_close_request(move |win| {
                if force_close.get() {
                    alive.set(false);
                    return glib::Propagation::Proceed;
                }
                if closing_window.get() {
                    // Ya hay una cadena de dialogos en curso.
                    return glib::Propagation::Stop;
                }
                let Some(docs) = documents.upgrade() else {
                    alive.set(false);
                    return glib::Propagation::Proceed;
                };
                // En orden de pestaña (ask_save_step saca por el final).
                let modified: Vec<Rc<Document>> = docs
                    .borrow()
                    .iter()
                    .filter(|d| d.is_modified.get())
                    .cloned()
                    .rev()
                    .collect();
                if modified.is_empty() {
                    persist_geometry(win, &settings);
                    alive.set(false);
                    return glib::Propagation::Proceed;
                }

                // Diálogos secuenciales por documento (como GNOME Text
                // Editor): Cancelar aborta todo; al resolverlos todos se
                // persiste la geometría y se cierra de verdad.
                let on_done = {
                    let settings = settings.clone();
                    let force_close = force_close.clone();
                    let alive = alive.clone();
                    Rc::new(move |win: &adw::ApplicationWindow| {
                        persist_geometry(win, &settings);
                        alive.set(false);
                        force_close.set(true);
                        win.close();
                    })
                };
                closing_window.set(true);
                let on_abort = {
                    let closing_window = closing_window.clone();
                    Rc::new(move || closing_window.set(false))
                };
                ask_save_step(
                    win.clone(),
                    Rc::new(RefCell::new(modified)),
                    file_manager.clone(),
                    toast.clone(),
                    on_done,
                    on_abort,
                );
                glib::Propagation::Stop
            });
        }

        // Documento inicial en blanco, como el editor mono-documento de
        // antes y como gnome-text-editor al arrancar sin ficheros.
        create_document(String::new(), None);

        Self {
            window,
            load_file,
            documents,
            tab_view,
        }
    }

    pub fn open_path(&self, path: &Path) {
        (self.load_file)(path);
    }

    pub fn present(&self) {
        self.window.present();
    }

    /// Número de páginas del TabView (smoke tests; el binario no lo usa).
    #[allow(dead_code)]
    pub fn page_count(&self) -> i32 {
        self.tab_view.n_pages()
    }

    /// Número de documentos vivos en la lista interna (smoke tests).
    #[allow(dead_code)]
    pub fn document_count(&self) -> usize {
        self.documents.borrow().len()
    }
}
