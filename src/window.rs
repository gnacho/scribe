use gettextrs::gettext;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use crate::editor::Editor;
use crate::export_pdf;
use crate::file_manager::{FileManager, OpenOutcome, Outcome};
use crate::preferences;
use crate::preview::PreviewPanel;
use crate::recovery;
use crate::settings::AppSettings;

const SHORTCUTS_UI: &str = include_str!("shortcuts.ui");

/// Crea un documento nuevo en blanco.
type NewDocument = Rc<dyn Fn()>;

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
    /// true si el fichero cambió en disco desde la última carga/guardado.
    /// Lo marca el `FileMonitor`; la recarga en vivo lo consulta al enfocar.
    external_change: Cell<bool>,
    /// Monitor del fichero en disco; vive mientras viva la pestaña.
    monitor: RefCell<Option<gio::FileMonitor>>,
    /// Id estable del documento para el nombre de su snapshot de recuperación
    /// (issue #23). No cambia con «guardar como» ni con el pid.
    recovery_id: Cell<u64>,
}

/// Qué hacer cuando el fichero de un documento cambia en disco (issue #20).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccionRecarga {
    /// Releer el fichero y sustituir el buffer (limpio + fichero cambiado).
    Recargar,
    /// El buffer tiene cambios sin guardar: preguntar antes de descartarlos.
    Preguntar,
    /// No hay cambios externos que atender.
    Nada,
    /// El fichero se borró en disco: conservar el buffer, marcar sin guardar.
    MantenerSinFichero,
}

/// Decide la acción de recarga a partir del estado del buffer y del fichero.
///
/// Pura (sin GTK ni sistema de ficheros) para poder probarla en unit tests.
fn decidir_recarga(
    buffer_limpio: bool,
    hay_cambios_externos: bool,
    fichero_existe: bool,
) -> AccionRecarga {
    if !fichero_existe {
        AccionRecarga::MantenerSinFichero
    } else if hay_cambios_externos && buffer_limpio {
        AccionRecarga::Recargar
    } else if hay_cambios_externos {
        AccionRecarga::Preguntar
    } else {
        AccionRecarga::Nada
    }
}

/// Decide si cerrar un documento pide confirmación «Guardar/Descartar/
/// Cancelar». Un borrador sin título cuyo buffer está vacío no tiene nada
/// que perder: cierra directamente aunque el flag de modificado siga activo
/// (p. ej. el usuario escribió y borró todo). Pura (sin GTK) para poder
/// probarla en unit tests.
fn requiere_confirmacion_cierre(modificado: bool, tiene_fichero: bool, buffer_vacio: bool) -> bool {
    modificado && (tiene_fichero || !buffer_vacio)
}

/// Visibilidad de la barra de pestañas (issue #26): se oculta mientras haya
/// un único documento, de modo que la ventana se lea como un documento
/// simple, y se muestra a partir de dos pestañas.
fn barra_visible(n_paginas: usize) -> bool {
    n_paginas > 1
}

impl Document {
    /// Cancela el monitor del fichero al cerrar la pestaña.
    fn cancelar_monitor(&self) {
        if let Some(monitor) = self.monitor.borrow_mut().take() {
            monitor.cancel();
        }
    }

    /// Nombre del snapshot de recuperación de este documento: estable por
    /// documento mientras viva la pestaña (issue #23).
    fn nombre_snapshot(&self) -> String {
        recovery::nombre_snapshot(std::process::id(), self.recovery_id.get())
    }

    /// Borra el snapshot de recuperación de este documento (al guardar o al
    /// descartar). Idempotente: si no existía, no hace nada.
    fn borrar_snapshot(&self) {
        let dir = recovery::dir_recuperacion();
        if let Err(e) = recovery::borrar_snapshot(&dir, &self.nombre_snapshot()) {
            glib::g_warning!(
                "scribe",
                "no se pudo borrar el snapshot de recuperación: {e}"
            );
        }
    }

    /// Escribe el snapshot de recuperación de este documento (texto + ruta +
    /// título). Se llama con debounce tras cada edición, mientras el
    /// documento esté modificado. Un fallo no interrumpe al usuario: solo se
    /// registra (la recuperación queda degradada, no el editor).
    fn escribir_snapshot(&self) {
        let dir = recovery::dir_recuperacion();
        let nombre = self.nombre_snapshot();
        let texto = self.editor.text();
        let ruta = self.current_file.borrow().clone();
        let titulo = self.display_name.borrow().clone();
        if let Err(e) = recovery::escribir_snapshot(&dir, &nombre, &texto, ruta.as_deref(), &titulo)
        {
            glib::g_warning!(
                "scribe",
                "no se pudo escribir el snapshot de recuperación: {e}"
            );
        }
    }
}

/// Ata un `gio::FileMonitor` al fichero del documento para detectar cambios
/// en disco, sustituyendo al anterior si lo había. El handler solo marca el
/// flag `external_change` (nunca toca el buffer): la recarga real se decide al
/// recuperar el foco o al pulsar F5.
fn atar_monitor(doc: &Rc<Document>) {
    if let Some(previo) = doc.monitor.borrow_mut().take() {
        previo.cancel();
    }
    let Some(path) = doc.current_file.borrow().clone() else {
        return;
    };
    let doc_weak = Rc::downgrade(doc);
    // Sistema de ficheros sin monitorización: la recarga en vivo simplemente
    // no estará disponible (degradación graciosa).
    if let Ok(monitor) =
        gio::File::for_path(&path).monitor_file(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE)
    {
        monitor.connect_changed(move |_, _, _, evento| {
            if !matches!(
                evento,
                gio::FileMonitorEvent::Changed
                    | gio::FileMonitorEvent::Created
                    | gio::FileMonitorEvent::Deleted
                    | gio::FileMonitorEvent::Moved
                    | gio::FileMonitorEvent::Renamed
                    | gio::FileMonitorEvent::MovedIn
                    | gio::FileMonitorEvent::MovedOut
            ) {
                return;
            }
            let Some(doc) = doc_weak.upgrade() else {
                return;
            };
            doc.external_change.set(true);
        });
        *doc.monitor.borrow_mut() = Some(monitor);
    }
}

/// Relee el fichero en disco y sustituye el buffer, conservando la línea y la
/// columna del cursor (clampeadas a la longitud del contenido nuevo). Si el
/// contenido en disco coincide con el buffer (p. ej. una escritura propia del
/// guardado/autoguardado) no toca el buffer, para no resetear el cursor ni el
/// historial de deshacer.
fn recargar_desde_disco(doc: &Document, path: &Path) -> Result<(), String> {
    let contenido = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    if contenido == doc.editor.text() {
        doc.external_change.set(false);
        return Ok(());
    }
    let (linea, columna) = doc.editor.cursor_position();
    doc.editor.set_text(&contenido);
    doc.editor.place_cursor_at(linea as u32, columna as u32);
    doc.is_modified.set(false);
    doc.borrar_snapshot();
    doc.external_change.set(false);
    Ok(())
}

/// Remate del cierre de una pestaña: confirma/aborta y limpia (ver
/// `finish_close` en `ScribeWindow::new`).
type FinishCloseFn = Rc<dyn Fn(&adw::TabPage, bool)>;

/// Abre un fichero y coloca el cursor en línea/columna (1-based).
type OpenPathAt = Rc<dyn Fn(&Path, u32, u32)>;

pub struct ScribeWindow {
    pub window: adw::ApplicationWindow,
    load_file: Rc<dyn Fn(&Path)>,
    /// Abre un fichero y coloca el cursor en línea/columna (1-based).
    open_path_at: OpenPathAt,
    /// Abre texto como documento sin título, marcado sin guardar (stdin).
    open_untitled: Rc<dyn Fn(String)>,
    /// Abre un snapshot recuperado: texto + ruta original (si había), marcado
    /// modificado para forzar una decisión consciente (issue #23).
    open_recovered: Rc<dyn Fn(String, Option<PathBuf>)>,
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
    /// Superficie del test de la barra de pestañas (issue #26).
    #[allow(dead_code)]
    tab_bar: adw::TabBar,
}

impl ScribeWindow {
    /// Visibilidad actual de la barra de pestañas (superficie de tests, #26).
    #[allow(dead_code)]
    pub fn barra_pestanas_visible(&self) -> bool {
        self.tab_bar.is_visible()
    }
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
        .map(str::to_string)
        .unwrap_or_else(|| gettext("Untitled"))
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
    editor.set_spellcheck(settings.spellcheck());
    editor.reaplicar_tema(adw::StyleManager::default().is_dark());
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
        // Sin fichero, el borrador usa el nombre cacheado ("Untitled").
        None => (doc.display_name.borrow().clone(), gettext("Draft")),
    };
    let tooltip = match file.as_ref() {
        Some(p) => p.to_string_lossy().to_string(),
        None => gettext("Unsaved draft"),
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
        window.set_title(Some(&format!("{dot}{name} - Scribe")));
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
        Some(&gettext("Save changes to “{name}”?").replace("{name}", &name)),
        Some(&gettext(
            "If you close without saving, you will lose what you have written.",
        )),
    );
    dialog.add_response("cancel", &gettext("_Cancel"));
    dialog.add_response("discard", &gettext("_Discard"));
    dialog.add_response("save", &gettext("_Save"));
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
                            doc.borrar_snapshot();
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
                            toast(&gettext("Could not save: {e}").replace("{e}", &e.to_string()));
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
                                    doc.borrar_snapshot();
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
                                    toast2(
                                        &gettext("Could not save: {e}")
                                            .replace("{e}", &e.to_string()),
                                    );
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
            .title_widget(&adw::WindowTitle::new(&gettext("Documents"), ""))
            .show_end_title_buttons(false)
            .css_classes(vec!["flat".to_string()])
            .build();

        let search_entry = gtk4::SearchEntry::builder()
            .placeholder_text(gettext("Filter recents…"))
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
        sidebar_content.append(&section(&gettext("Recents")));
        sidebar_content.append(&recents_list);
        sidebar_content.append(&section(&gettext("Contents")));
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
        // Issue #26: la barra se oculta con un solo documento y se muestra
        // a partir de dos. Se usa la notificación de la propiedad «n-pages»:
        // conectar items-changed sobre pages() en línea dejaba el wrapper del
        // modelo efímero y la señal moría con él (la barra no reaparecía).
        tab_bar.set_visible(barra_visible(tab_view.n_pages() as usize));
        {
            let tab_bar = tab_bar.clone();
            tab_view.connect_n_pages_notify(move |view| {
                tab_bar.set_visible(barra_visible(view.n_pages() as usize));
            });
        }

        // ============================== BÚSQUEDA ==============================
        // Barra de búsqueda del documento activo (issue #37). Vive dentro del
        // panel del editor, no encima de la previsualización: al abrirla, el
        // editor y su barra se reparten el panel izquierdo, como en Typesetter.
        let search_entry = gtk4::SearchEntry::builder()
            .placeholder_text(gettext("Search"))
            .hexpand(true)
            .build();
        // Opciones de búsqueda (issue #50), como en gnome-text-editor:
        // «Aa» distingue mayúsculas y «.*» trata el patrón como regex.
        let case_toggle = gtk4::ToggleButton::builder()
            .label("Aa")
            .tooltip_text(gettext("Match Case"))
            .build();
        let regex_toggle = gtk4::ToggleButton::builder()
            .label(".*")
            .tooltip_text(gettext("Regular Expression"))
            .build();
        let search_count = gtk4::Label::builder()
            .css_classes(vec!["caption".to_string(), "dim-label".to_string()])
            .build();
        let search_prev_button = gtk4::Button::builder()
            .icon_name("go-up-symbolic")
            .tooltip_text(gettext("Find Previous"))
            .action_name("win.find-prev")
            .build();
        let search_next_button = gtk4::Button::builder()
            .icon_name("go-down-symbolic")
            .tooltip_text(gettext("Find Next"))
            .action_name("win.find-next")
            .build();
        // Botón para revelar la fila de reemplazar, como en gnome-text-editor:
        // un conmutador con el icono estándar edit-find-replace-symbolic.
        let replace_toggle = gtk4::ToggleButton::builder()
            .icon_name("edit-find-replace-symbolic")
            .tooltip_text(gettext("Search & Replace"))
            .build();
        let search_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        search_box.append(&search_entry);
        search_box.append(&case_toggle);
        search_box.append(&regex_toggle);
        search_box.append(&search_count);
        search_box.append(&search_prev_button);
        search_box.append(&search_next_button);
        search_box.append(&replace_toggle);

        // Fila de reemplazar (Ctrl+H), como en gnome-text-editor: aparece bajo
        // la de búsqueda al pedirla y se oculta al cerrar el modo.
        let replace_entry = gtk4::SearchEntry::builder()
            .placeholder_text(gettext("Replace"))
            .hexpand(true)
            .build();
        let replace_button = gtk4::Button::builder()
            .label(gettext("_Replace"))
            .use_underline(true)
            .build();
        let replace_all_button = gtk4::Button::builder()
            .label(gettext("Replace _All"))
            .use_underline(true)
            .build();
        let replace_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        replace_box.append(&replace_entry);
        replace_box.append(&replace_button);
        replace_box.append(&replace_all_button);
        replace_box.set_visible(false);

        let search_rows = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(6)
            .margin_top(6)
            .margin_bottom(6)
            .margin_start(6)
            .margin_end(6)
            .build();
        search_rows.append(&search_box);
        search_rows.append(&replace_box);
        let search_bar = gtk4::SearchBar::builder()
            .child(&search_rows)
            .css_classes(vec!["toolbar".to_string()])
            .build();
        search_bar.connect_entry(&search_entry);
        search_bar.set_key_capture_widget(Some(&window));

        // ============================== CONTENIDO ==============================
        let editor_pane = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        editor_pane.append(&search_bar);
        editor_pane.append(&tab_view);

        let paned = gtk4::Paned::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .start_child(&editor_pane)
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
            .label(gettext("_Open"))
            .use_underline(true)
            .tooltip_text(gettext("Open a document (Ctrl+O)"))
            .menu_model(&recents_menu)
            .build();
        open_button.set_action_name(Some("win.open"));
        header.pack_start(&open_button);

        let title_widget = adw::WindowTitle::new(&gettext("Untitled"), "");
        header.set_title_widget(Some(&title_widget));

        // Botón «nueva pestaña» (issue #28): icono plano que crea un
        // documento en blanco. Va a la IZQUIERDA, junto al de Abrir, como
        // estaba siempre (el usuario lo validó así).
        let new_tab_button = gtk4::Button::builder()
            .icon_name("tab-new-symbolic")
            .tooltip_text(gettext("New Tab (Ctrl+N)"))
            .action_name("win.new-document")
            .build();
        header.pack_start(&new_tab_button);

        let menu_button = gtk4::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .primary(true)
            .tooltip_text(gettext("Main menu"))
            .build();
        header.pack_end(&menu_button);

        // Botón de búsqueda (issue #37): sigue el estado de la barra, de modo
        // que al cerrarla con Escape el botón se suelta solo.
        let search_button = gtk4::ToggleButton::builder()
            .icon_name("edit-find-symbolic")
            .tooltip_text(gettext("Find in the document (Ctrl+F)"))
            .build();
        search_button
            .bind_property("active", &search_bar, "search-mode-enabled")
            .bidirectional()
            .sync_create()
            .build();
        header.pack_end(&search_button);

        // Botón de vista dividida (issue #35), como el de Typesetter: el icono
        // dice qué se va a ver y comparte estado con la acción (Ctrl+Shift+P).
        let preview_button = gtk4::ToggleButton::builder()
            .icon_name("scribe-preview-off-symbolic")
            .tooltip_text(gettext("Split View"))
            .action_name("win.toggle-preview")
            .build();
        header.pack_end(&preview_button);

        // ---- menú principal ----
        let menu = gio::Menu::new();

        let file_section = gio::Menu::new();
        file_section.append(Some(&gettext("_Save")), Some("win.save"));
        file_section.append(Some(&gettext("Save _As…")), Some("win.save-as"));
        file_section.append(Some(&gettext("Export to _PDF…")), Some("win.export-pdf"));
        menu.append_section(None, &file_section);

        let view_section = gio::Menu::new();
        view_section.append(Some(&gettext("Sidebar")), Some("win.toggle-sidebar"));
        view_section.append(Some(&gettext("Focus Mode")), Some("win.focus-mode"));
        view_section.append(
            Some(&gettext("Typewriter Mode")),
            Some("win.typewriter-mode"),
        );
        menu.append_section(None, &view_section);

        let app_section = gio::Menu::new();
        app_section.append(Some(&gettext("_Preferences")), Some("win.preferences"));
        app_section.append(
            Some(&gettext("_Keyboard Shortcuts")),
            Some("win.show-help-overlay"),
        );
        app_section.append(Some(&gettext("_About Scribe")), Some("win.about"));
        menu.append_section(None, &app_section);

        let popover = gtk4::PopoverMenu::from_model(Some(&menu));
        menu_button.set_popover(Some(&popover));

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.add_top_bar(&tab_bar);
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
        builder.set_translation_domain(Some("scribe"));
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
            // Debil: los refrescos de la pestaña activa lo capturan y están
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

        // ---- búsqueda del documento activo (issue #37) ----
        // La barra es única por ventana y siempre apunta a la pestaña activa;
        // cada documento conserva su propio contexto, así que cambiar de
        // pestaña no pierde el índice ya calculado.
        let update_find_count: Rc<dyn Fn()> = {
            let active_document = active_document.clone();
            let search_count = search_count.clone();
            Rc::new(move || {
                let (position, total) = match active_document() {
                    Some(doc) => doc.editor.search_state(),
                    None => (0, 0),
                };
                if total == 0 {
                    search_count.set_label("");
                    return;
                }
                // Traduccion: «{0}» es la coincidencia actual, «{1}» el total.
                search_count.set_label(
                    &gettext("{0} of {1}")
                        .replace("{0}", &position.to_string())
                        .replace("{1}", &total.to_string()),
                );
            })
        };

        // Reaplica el patrón de la barra al documento activo y sincroniza las
        // opciones con él: cada pestaña conserva sus propios ajustes.
        let apply_find: Rc<dyn Fn()> = {
            let active_document = active_document.clone();
            let search_entry = search_entry.clone();
            let case_toggle = case_toggle.clone();
            let regex_toggle = regex_toggle.clone();
            let update_find_count = update_find_count.clone();
            Rc::new(move || {
                let mut invalido = false;
                if let Some(doc) = active_document() {
                    doc.editor.set_search_text(&search_entry.text());
                    case_toggle.set_active(doc.editor.search_case_sensitive());
                    regex_toggle.set_active(doc.editor.search_regex());
                    invalido = doc.editor.search_pattern_has_error();
                }
                search_entry.set_css_classes(if invalido { &["error"] } else { &[] });
                update_find_count();
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

        // ---- recarga en vivo (issue #20): decisión por documento ----
        // Consulta el flag del monitor y decide con la función pura. Solo se
        // recarga de forma automática el buffer limpio; el sucio con cambios
        // externos deja el flag para que F5 ofrezca el diálogo (o se reconcilia
        // al guardar).
        let maybe_reload: Rc<dyn Fn(&Document)> = {
            let toast = toast.clone();
            let refresh_tabs = refresh_tabs.clone();
            Rc::new(move |doc| {
                let buffer_limpio = !doc.is_modified.get();
                let fichero_existe = doc
                    .current_file
                    .borrow()
                    .as_ref()
                    .is_some_and(|p| p.exists());
                match decidir_recarga(buffer_limpio, true, fichero_existe) {
                    AccionRecarga::Recargar => {
                        let Some(path) = doc.current_file.borrow().clone() else {
                            return;
                        };
                        match recargar_desde_disco(doc, &path) {
                            Ok(()) => refresh_tabs(),
                            Err(e) => toast(&gettext("Could not reload: {e}").replace("{e}", &e)),
                        }
                    }
                    AccionRecarga::MantenerSinFichero => {
                        doc.is_modified.set(true);
                        doc.external_change.set(false);
                        refresh_tabs();
                        toast(&gettext("File deleted on disk; save to recreate it"));
                    }
                    // Sucio con cambios externos: no se recarga solo.
                    AccionRecarga::Preguntar => {}
                    AccionRecarga::Nada => {
                        doc.external_change.set(false);
                    }
                }
            })
        };

        // ---- recarga manual (F5): guard de cambios sin guardar ----
        let manual_reload: Rc<dyn Fn()> = {
            let active_document = active_document.clone();
            let toast = toast.clone();
            let refresh_tabs = refresh_tabs.clone();
            let window = window.downgrade();
            Rc::new(move || {
                let Some(doc) = active_document() else {
                    return;
                };
                let Some(path) = doc.current_file.borrow().clone() else {
                    toast(&gettext("This document is not linked to a file"));
                    return;
                };
                if !path.exists() {
                    // Borrado externo: conservar el buffer, marcar sin guardar.
                    doc.is_modified.set(true);
                    doc.external_change.set(false);
                    refresh_tabs();
                    toast(&gettext("File deleted on disk; save to recreate it"));
                    return;
                }
                if !doc.is_modified.get() {
                    // Buffer limpio: recarga manual directa.
                    match recargar_desde_disco(&doc, &path) {
                        Ok(()) => refresh_tabs(),
                        Err(e) => toast(&gettext("Could not reload: {e}").replace("{e}", &e)),
                    }
                    return;
                }
                if !doc.external_change.get() {
                    // Sucio y sin cambios externos: no hay nada que recargar.
                    return;
                }
                // Sucio y con cambios externos: diálogo recargar/cancelar.
                let Some(win) = window.upgrade() else {
                    return;
                };
                let name = doc.display_name.borrow().clone();
                let dialog = adw::MessageDialog::new(
                    Some(&win),
                    Some(&gettext("Reload “{name}” and discard changes?").replace("{name}", &name)),
                    Some(&gettext(
                        "The file changed on disk. Reloading discards your unsaved edits.",
                    )),
                );
                dialog.add_response("cancel", &gettext("_Cancel"));
                dialog.add_response("reload", &gettext("_Reload"));
                dialog.set_response_appearance("reload", adw::ResponseAppearance::Destructive);
                dialog.set_default_response(Some("cancel"));
                dialog.set_close_response("cancel");
                // Diálogo one-shot y acotado: captura el documento en fuerte.
                let refresh_tabs = refresh_tabs.clone();
                let toast = toast.clone();
                dialog.choose(gio::Cancellable::NONE, move |response| {
                    if response.as_str() != "reload" {
                        return;
                    }
                    let Some(path) = doc.current_file.borrow().clone() else {
                        return;
                    };
                    match recargar_desde_disco(&doc, &path) {
                        Ok(()) => refresh_tabs(),
                        Err(e) => toast(&gettext("Could not reload: {e}").replace("{e}", &e)),
                    }
                });
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
                        row.set_subtitle(&gettext("Not found"));
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
                            gettext("No recent documents")
                        } else {
                            gettext("No results")
                        })
                        .build();
                    row.set_sensitive(false);
                    recents_list.append(&row);
                }
            })
        };
        refresh_recents("");

        // ---- aplicar preferencias ----
        // Ranura de la acción de estilos: `apply_settings` se construye antes
        // que las acciones, así que el estado del menú se sincroniza por aquí
        // cuando la preferencia cambia (menú ↔ Preferencias, issue #36).
        let action_color_scheme_slot: Rc<RefCell<Option<gio::SimpleAction>>> =
            Rc::new(RefCell::new(None));
        let apply_settings: Rc<dyn Fn()> = {
            let settings = settings.clone();
            let documents = Rc::downgrade(&documents);
            let preview = preview.clone();
            let refresh_recents = refresh_recents.clone();
            let action_color_scheme_slot = action_color_scheme_slot.clone();
            Rc::new(move || {
                let scheme = settings.color_scheme_nick();
                adw::StyleManager::default().set_color_scheme(match scheme.as_str() {
                    "light" => adw::ColorScheme::ForceLight,
                    "dark" => adw::ColorScheme::ForceDark,
                    _ => adw::ColorScheme::Default,
                });
                if let Some(action) = action_color_scheme_slot.borrow().as_ref() {
                    action.set_state(&scheme.to_variant());
                }
                // Las preferencias del editor son globales: se aplican a
                // todos los documentos abiertos.
                if let Some(docs) = documents.upgrade() {
                    for doc in docs.borrow().iter() {
                        apply_editor_settings(&settings, &doc.editor);
                    }
                }
                preview.set_font_size(settings.font_size());
                refresh_recents("");
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
            let toast = toast.clone();
            let update_find_count = update_find_count.clone();
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
                // Avisos transitorios del editor (p. ej. «guarda antes de pegar
                // imágenes») hacia el ToastOverlay de la ventana.
                {
                    let toast = toast.clone();
                    editor.connect_toast(move |msg| toast(msg));
                }

                // El contador de búsqueda sigue al documento activo; con
                // documentos grandes el recuento llega por señal, no al fijar
                // el patrón.
                {
                    let update_find_count = update_find_count.clone();
                    editor
                        .search_context()
                        .connect_occurrences_count_notify(move |_| update_find_count());
                }

                let page = tab_view.append(&editor.widget);
                let display_name = match &file {
                    Some(p) => file_name_of(p),
                    None => gettext("Untitled"),
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
                    external_change: Cell::new(false),
                    monitor: RefCell::new(None),
                    recovery_id: Cell::new(recovery::nuevo_id_doc()),
                });

                // Monitor del fichero (recarga en vivo, issue #20): vigila el
                // fichero recién abierto mientras la pestaña esté viva.
                atar_monitor(&doc);

                // ---- cableado por documento ----
                {
                    // Débil: Document → Editor → callback → Document sería un
                    // ciclo y ni la pestaña ni el editor se liberarían jamás.
                    let doc_weak = Rc::downgrade(&doc);
                    // Debil: este handler vive en el buffer, que cuelga del
                    // propio TabView; en fuerte seria un ciclo (R3).
                    let tab_view = tab_view.downgrade();
                    let refresh_tabs = refresh_tabs.clone();
                    let preview = preview.clone();
                    let preview_visible = preview_visible.clone();
                    let toc_list = toc_list.clone();
                    let toc_lines = toc_lines.clone();
                    let generation = Rc::new(Cell::new(0u64));
                    let rec_generation = Rc::new(Cell::new(0u64));
                    doc.editor.connect_changed(move |text| {
                        let Some(doc) = doc_weak.upgrade() else {
                            return;
                        };
                        if !doc.is_modified.replace(true) {
                            // Primera modificación: actualiza el «• » y el
                            // needs_attention de todas las pestañas.
                            refresh_tabs();
                        }
                        // ---- recuperación (issue #23): un snapshot por
                        // documento mientras esté modificado, con debounce
                        // corto para no escribir a disco en cada pulsación ----
                        let rec_gen = rec_generation.get().wrapping_add(1);
                        rec_generation.set(rec_gen);
                        let doc_weak_rec = doc_weak.clone();
                        let rec_generation_rec = rec_generation.clone();
                        glib::timeout_add_local_once(Duration::from_secs(3), move || {
                            if rec_generation_rec.get() != rec_gen {
                                return;
                            }
                            let Some(doc) = doc_weak_rec.upgrade() else {
                                return;
                            };
                            if doc.is_modified.get() {
                                doc.escribir_snapshot();
                            }
                        });
                        // Contadores, TOC y vista previa solo siguen a la
                        // pestaña activa.
                        let Some(tab_view) = tab_view.upgrade() else {
                            return;
                        };
                        if tab_view.selected_page().as_ref() != Some(&doc.page) {
                            return;
                        }

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
                            if preview_visible.get() {
                                preview.update(&text);
                            }
                            rebuild_toc(&toc_list, &toc_lines, &text);
                        });
                    });
                }
                if let Some(docs) = documents.upgrade() {
                    docs.borrow_mut().push(doc.clone());
                }
                tab_view.set_selected_page(&doc.page);
                refresh_tabs();
                let text = doc.editor.text();
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
                    doc.external_change.set(false);
                    *doc.display_name.borrow_mut() = file_name_of(&path);
                    atar_monitor(&doc);
                    tab_view.set_selected_page(&doc.page);
                } else {
                    create_document(content, Some(path.clone()));
                }
                settings.push_recent_file(&path.to_string_lossy());
                refresh_recents("");
                refresh_tabs();
            })
        };

        let load_file: Rc<dyn Fn(&Path)> = {
            let open_loaded = open_loaded.clone();
            let toast = toast.clone();
            Rc::new(move |path: &Path| match std::fs::read_to_string(path) {
                Ok(content) => open_loaded(path.to_path_buf(), content),
                Err(e) => toast(&gettext("Could not open: {e}").replace("{e}", &e.to_string())),
            })
        };

        // Abre un fichero y coloca el cursor: `load_file` lee y abre de forma
        // síncrona (set_text incluido), así que al volver el documento abierto
        // es la pestaña activa y el cursor se puede posicionar ya.
        let open_path_at: OpenPathAt = {
            let load_file = load_file.clone();
            let active_document = active_document.clone();
            Rc::new(move |path: &Path, line: u32, column: u32| {
                load_file(path);
                if let Some(doc) = active_document() {
                    doc.editor.place_cursor_at(line, column);
                }
            })
        };

        // Abre texto (stdin) como documento sin título, marcado sin guardar
        // para que «Guardar» abra «Guardar como». Reutiliza la pestaña virgen
        // si es la única, igual que `open_loaded`.
        let open_untitled: Rc<dyn Fn(String)> = {
            let documents = Rc::downgrade(&documents);
            let tab_view = tab_view.clone();
            let create_document = create_document.clone();
            let refresh_tabs = refresh_tabs.clone();
            Rc::new(move |texto: String| {
                let Some(docs) = documents.upgrade() else {
                    return;
                };
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
                    doc.editor.set_text(&texto);
                    doc.is_modified.set(true);
                    tab_view.set_selected_page(&doc.page);
                } else {
                    let doc = create_document(texto, None);
                    doc.is_modified.set(true);
                }
                refresh_tabs();
            })
        };

        // Abre un snapshot recuperado (issue #23): el texto se abre como
        // documento sin título, vinculado a su ruta original si la tenía,
        // pero MARCADO modificado para forzar una decisión consciente del
        // usuario (guardar o descartar). Reutiliza la pestaña virgen si es
        // la única, igual que `open_untitled`.
        let open_recovered: Rc<dyn Fn(String, Option<PathBuf>)> = {
            let documents = Rc::downgrade(&documents);
            let tab_view = tab_view.clone();
            let create_document = create_document.clone();
            let refresh_tabs = refresh_tabs.clone();
            Rc::new(move |texto: String, ruta: Option<PathBuf>| {
                let Some(docs) = documents.upgrade() else {
                    return;
                };
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
                    doc.editor.set_text(&texto);
                    doc.editor.set_base_dir(
                        ruta.as_ref()
                            .and_then(|p| p.parent().map(Path::to_path_buf)),
                    );
                    *doc.current_file.borrow_mut() = ruta.clone();
                    doc.is_modified.set(true);
                    *doc.display_name.borrow_mut() = match &ruta {
                        Some(p) => file_name_of(p),
                        None => gettext("Untitled"),
                    };
                    atar_monitor(&doc);
                    tab_view.set_selected_page(&doc.page);
                } else {
                    let doc = create_document(texto, ruta);
                    doc.is_modified.set(true);
                }
                refresh_tabs();
            })
        };

        let new_document: NewDocument = {
            let create_document = create_document.clone();
            Rc::new(move || {
                create_document(String::new(), None);
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
                    if let Some(cerrado) = docs.borrow().iter().find(|d| d.page == *page).cloned() {
                        cerrado.cancelar_monitor();
                        // Al cerrar (o descartar) la pestaña se borra su
                        // snapshot de recuperación (issue #23).
                        cerrado.borrar_snapshot();
                    }
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
        let action_zoom_in = gio::SimpleAction::new("zoom-in", None);
        let action_zoom_out = gio::SimpleAction::new("zoom-out", None);
        let action_zoom_reset = gio::SimpleAction::new("zoom-reset", None);
        let action_bold = gio::SimpleAction::new("bold", None);
        let action_italic = gio::SimpleAction::new("italic", None);
        let action_code = gio::SimpleAction::new("code", None);
        let action_format_tables = gio::SimpleAction::new("format-tables", None);
        let action_close_fences = gio::SimpleAction::new("close-fences", None);
        let action_close_tab = gio::SimpleAction::new("close-tab", None);
        let action_tab_next = gio::SimpleAction::new("tab-next", None);
        let action_tab_prev = gio::SimpleAction::new("tab-prev", None);
        let action_reload = gio::SimpleAction::new("reload", None);
        let action_export_pdf = gio::SimpleAction::new("export-pdf", None);
        let action_find = gio::SimpleAction::new("find", None);
        let action_find_next = gio::SimpleAction::new("find-next", None);
        let action_find_prev = gio::SimpleAction::new("find-prev", None);
        let action_find_replace = gio::SimpleAction::new("toggle-replace", None);
        // Estilos: una sola acción con estado de cadena («system»/«light»/
        // «dark»), para que GTK pinte los tres ítems como excluyentes.
        let action_color_scheme = gio::SimpleAction::new_stateful(
            "color-scheme",
            Some(glib::VariantTy::STRING),
            &settings.color_scheme_nick().to_variant(),
        );

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
            &action_close_fences,
            &action_close_tab,
            &action_tab_next,
            &action_tab_prev,
            &action_reload,
            &action_export_pdf,
            &action_find,
            &action_find_next,
            &action_find_prev,
            &action_find_replace,
        ] {
            window.add_action(a);
        }
        window.add_action(&action_open_recent);
        window.add_action(&action_color_scheme);
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
        {
            let active_document = active_document.clone();
            let toast = toast.clone();
            action_close_fences.connect_activate(move |_, _| {
                let Some(doc) = active_document() else {
                    return;
                };
                if doc.editor.close_unclosed_fences() {
                    toast(&gettext("Code fences closed"));
                } else {
                    toast(&gettext("No unclosed fences"));
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

        // ---- recarga manual (F5) ----
        {
            let manual_reload = manual_reload.clone();
            action_reload.connect_activate(move |_, _| {
                manual_reload();
            });
        }

        // ---- exportar a PDF ----
        {
            let active_document = active_document.clone();
            let window = window.downgrade();
            let settings = settings.clone();
            action_export_pdf.connect_activate(move |_, _| {
                let Some(window) = window.upgrade() else {
                    return;
                };
                let Some(doc) = active_document() else {
                    return;
                };
                let config = export_pdf::ExportConfig::desde_settings(&settings);
                let texto = doc.editor.text();
                let titulo = doc.display_name.borrow().clone();
                export_pdf::exportar(
                    &window,
                    &config,
                    &texto,
                    &titulo,
                    adw::StyleManager::default().is_dark(),
                );
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
            action_new_document.connect_activate(move |_, _| {
                new_document();
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
                                    toast(&gettext("That file is already open in another tab"));
                                    return;
                                }
                            }
                            // Tras «guardar como» el directorio base de las
                            // imágenes puede haber cambiado.
                            doc.editor.set_base_dir(p.parent().map(Path::to_path_buf));
                            *doc.current_file.borrow_mut() = Some(p.clone());
                            doc.is_modified.set(false);
                            doc.borrar_snapshot();
                            // El guardado reconcilia buffer y disco; se limpia
                            // el flag y se re-ata el monitor a la ruta nueva
                            // (puede haber cambiado con «guardar como»).
                            doc.external_change.set(false);
                            atar_monitor(&doc);
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
        // El icono del botón dice qué se va a ver: panel derecho atenuado
        // (previsualización oculta) o relleno (visible).
        {
            let preview_button = preview_button.clone();
            let sync_icon = move |on: bool| {
                preview_button.set_icon_name(if on {
                    "scribe-preview-on-symbolic"
                } else {
                    "scribe-preview-off-symbolic"
                });
            };
            sync_icon(preview_visible.get());
            action_preview.connect_state_notify(move |action| {
                sync_icon(
                    action
                        .state()
                        .and_then(|s| s.get::<bool>())
                        .unwrap_or(false),
                );
            });
        }

        // ---- estilos (issue #36) ----
        {
            // El estado inicial del menú sale del ajuste; a partir de aquí menú
            // y Preferencias se sincronizan solos vía `apply_settings`.
            *action_color_scheme_slot.borrow_mut() = Some(action_color_scheme.clone());
            let settings = settings.clone();
            let apply_settings = apply_settings.clone();
            action_color_scheme.connect_activate(move |action, param| {
                let Some(nick) = param.and_then(|p| p.get::<String>()) else {
                    return;
                };
                action.set_state(&nick.to_variant());
                settings.set_color_scheme_nick(&nick);
                apply_settings();
            });
        }

        // ---- búsqueda (issue #37) ----
        {
            let apply_find = apply_find.clone();
            search_entry.connect_search_changed(move |_| apply_find());
        }
        // ---- opciones y reemplazo (issue #50) ----
        {
            let active_document = active_document.clone();
            let update_find_count = update_find_count.clone();
            case_toggle.connect_toggled(move |t| {
                if let Some(doc) = active_document() {
                    doc.editor.set_search_case_sensitive(t.is_active());
                }
                update_find_count();
            });
        }
        {
            // Al cambiar la regex hay que revalidar el patrón: aplica de nuevo
            // el texto de la entrada (pinta el error si la regex no compila).
            let active_document = active_document.clone();
            let apply_find = apply_find.clone();
            regex_toggle.connect_toggled(move |t| {
                if let Some(doc) = active_document() {
                    doc.editor.set_search_regex(t.is_active());
                }
                apply_find();
            });
        }
        {
            let active_document = active_document.clone();
            let replace_entry = replace_entry.clone();
            let update_find_count = update_find_count.clone();
            replace_button.connect_clicked(move |_| {
                if let Some(doc) = active_document() {
                    doc.editor.replace_current(&replace_entry.text());
                }
                update_find_count();
            });
        }
        {
            let active_document = active_document.clone();
            let replace_entry = replace_entry.clone();
            let toast = toast.clone();
            replace_all_button.connect_clicked(move |_| {
                if let Some(doc) = active_document() {
                    let toast = toast.clone();
                    doc.editor
                        .replace_all_matches(&replace_entry.text(), move |n| {
                            if n > 0 {
                                toast(
                                    &gettext("{n} occurrences replaced")
                                        .replace("{n}", &n.to_string()),
                                );
                            }
                        });
                }
            });
        }
        {
            // Ctrl+H: muestra u oculta la fila de reemplazar.
            let search_bar = search_bar.clone();
            let search_entry = search_entry.clone();
            let replace_box = replace_box.clone();
            let replace_entry = replace_entry.clone();
            action_find_replace.connect_activate(move |_, _| {
                search_bar.set_search_mode(true);
                let visible = !replace_box.is_visible();
                replace_box.set_visible(visible);
                if visible {
                    replace_entry.grab_focus();
                    replace_entry.select_region(0, -1);
                } else {
                    search_entry.grab_focus();
                }
            });
        }
        {
            // El conmutador con icono de la barra activa la misma acción que
            // Ctrl+H, y su estado visual sigue a la visibilidad de la fila,
            // así el botón y el atajo siempre coinciden.
            let replace_box = replace_box.clone();
            let replace_toggle = replace_toggle.clone();
            replace_box
                .bind_property("visible", &replace_toggle, "active")
                .sync_create()
                .build();
            replace_toggle.connect_clicked(move |_| action_find_replace.activate(None));
        }
        {
            // Intros en captura: se adelantan al GtkText interno, que si no
            // emitiría su «activate» (siguiente) también con Mayús pulsado.
            let update_find_count = update_find_count.clone();
            let active_document = active_document.clone();
            let keys = gtk4::EventControllerKey::new();
            keys.set_propagation_phase(gtk4::PropagationPhase::Capture);
            keys.connect_key_pressed(move |_, key, _, state| {
                if key != gtk4::gdk::Key::Return && key != gtk4::gdk::Key::KP_Enter {
                    return glib::Propagation::Proceed;
                }
                if let Some(doc) = active_document() {
                    if state.contains(gtk4::gdk::ModifierType::SHIFT_MASK) {
                        doc.editor.search_previous();
                    } else {
                        doc.editor.search_next();
                    }
                }
                update_find_count();
                glib::Propagation::Stop
            });
            search_entry.add_controller(keys);
        }
        for (action, forward) in [(&action_find_next, true), (&action_find_prev, false)] {
            let update_find_count = update_find_count.clone();
            let active_document = active_document.clone();
            action.connect_activate(move |_, _| {
                if let Some(doc) = active_document() {
                    if forward {
                        doc.editor.search_next();
                    } else {
                        doc.editor.search_previous();
                    }
                }
                update_find_count();
            });
        }
        {
            let update_find_count = update_find_count.clone();
            let apply_find = apply_find.clone();
            let active_document = active_document.clone();
            search_bar.connect_search_mode_enabled_notify(move |bar| {
                if bar.is_search_mode() {
                    // El patrón de la barra se reaplica al documento que pasa a
                    // estar activo, para no buscar con el texto de otra pestaña.
                    apply_find();
                    if let Some(doc) = active_document() {
                        doc.editor.set_search_highlight(true);
                    }
                } else if let Some(doc) = active_document() {
                    doc.editor.set_search_highlight(false);
                }
                update_find_count();
            });
        }
        {
            let search_bar = search_bar.clone();
            let search_entry = search_entry.clone();
            action_find.connect_activate(move |_, _| {
                search_bar.set_search_mode(true);
                search_entry.select_region(0, -1);
                search_entry.grab_focus();
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
        // ---- cambio de pestaña: refresca título, TOC y vista previa, y quita
        // el needs_attention de la nueva activa ----
        {
            let documents = Rc::downgrade(&documents);
            let refresh_tabs = refresh_tabs.clone();
            let preview = preview.clone();
            let preview_visible = preview_visible.clone();
            let toc_list = toc_list.clone();
            let toc_lines = toc_lines.clone();
            let search_bar = search_bar.clone();
            let apply_find = apply_find.clone();
            let update_find_count = update_find_count.clone();
            tab_view.connect_selected_page_notify(move |tv| {
                refresh_tabs();
                let (Some(docs), Some(page)) = (documents.upgrade(), tv.selected_page()) else {
                    return;
                };
                let Some(doc) = docs.borrow().iter().find(|d| d.page == page).cloned() else {
                    return;
                };
                let text = doc.editor.text();
                if preview_visible.get() {
                    preview.update(&text);
                }
                rebuild_toc(&toc_list, &toc_lines, &text);
                // Con la barra de búsqueda abierta, el patrón se traslada a la
                // pestaña que pasa a estar activa.
                if search_bar.is_search_mode() {
                    apply_find();
                } else {
                    update_find_count();
                }
            });
        }

        // ---- recarga en vivo al recuperar el foco (issue #20) ----
        {
            // Débiles: la señal vive en la ventana. Al volver el foco se
            // consulta el flag del monitor de cada documento y se decide con
            // la función pura.
            let documents = Rc::downgrade(&documents);
            let maybe_reload = maybe_reload.clone();
            window.connect_is_active_notify(move |win| {
                if !win.is_active() {
                    return;
                }
                let Some(docs) = documents.upgrade() else {
                    return;
                };
                for doc in docs.borrow().iter() {
                    if doc.external_change.get() {
                        maybe_reload(doc);
                    }
                }
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
                if !requiere_confirmacion_cierre(
                    doc.is_modified.get(),
                    doc.current_file.borrow().is_some(),
                    doc.editor.text().is_empty(),
                ) {
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
                    Some(&gettext("Save changes to “{name}”?").replace("{name}", &name)),
                    Some(&gettext(
                        "If you close without saving, you will lose what you have written.",
                    )),
                );
                dialog.add_response("cancel", &gettext("_Cancel"));
                dialog.add_response("discard", &gettext("_Discard"));
                dialog.add_response("save", &gettext("_Save"));
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
                                        doc.borrar_snapshot();
                                        finish_close(&page, true);
                                    }
                                    // Un guardado fallido aborta el cierre:
                                    // el usuario decide qué hacer después.
                                    Err(e) => {
                                        toast(
                                            &gettext("Could not save: {e}")
                                                .replace("{e}", &e.to_string()),
                                        );
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
                                                doc.borrar_snapshot();
                                                finish_close(&page, true);
                                            }
                                            Outcome::Error(e) => {
                                                toast(
                                                    &gettext("Could not save: {e}")
                                                        .replace("{e}", &e.to_string()),
                                                );
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
                            doc.borrar_snapshot();
                            doc.external_change.set(false);
                            // También quita el «•»/needs_attention de las
                            // pestañas no activas (D4).
                            refresh_tabs();
                            doc.autosave_failed.set(false);
                        }
                        // Sin este aviso el usuario confía en una copia
                        // automática que no existe.
                        Err(e) => {
                            if !doc.autosave_failed.replace(true) {
                                toast(
                                    &gettext("Could not autosave: {e}")
                                        .replace("{e}", &e.to_string()),
                                );
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
                    .filter(|d| {
                        requiere_confirmacion_cierre(
                            d.is_modified.get(),
                            d.current_file.borrow().is_some(),
                            d.editor.text().is_empty(),
                        )
                    })
                    .cloned()
                    .rev()
                    .collect();
                if modified.is_empty() {
                    persist_geometry(win, &settings);
                    // Cierre con gracia: no quedan docs modificados, así que
                    // se limpian los snapshots de este proceso por si alguno
                    // quedó huérfano (issue #23).
                    recovery::limpiar_de_este_pid(
                        &recovery::dir_recuperacion(),
                        std::process::id(),
                    );
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
                        // Cierre con gracia tras resolver todos los docs
                        // modificados: se limpian los snapshots de este
                        // proceso (issue #23).
                        recovery::limpiar_de_este_pid(
                            &recovery::dir_recuperacion(),
                            std::process::id(),
                        );
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
            open_path_at,
            open_untitled,
            open_recovered,
            documents,
            tab_view,
            tab_bar,
        }
    }

    pub fn open_path(&self, path: &Path) {
        (self.load_file)(path);
    }

    pub fn open_path_at(&self, path: &Path, line: u32, column: u32) {
        (self.open_path_at)(path, line, column);
    }

    pub fn open_untitled(&self, text: String) {
        (self.open_untitled)(text);
    }

    pub fn open_recovered(&self, text: String, ruta: Option<PathBuf>) {
        (self.open_recovered)(text, ruta);
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

    /// Buffer del documento activo (smoke tests; el binario no lo usa).
    #[allow(dead_code)]
    pub fn active_buffer(&self) -> Option<gtksourceview5::Buffer> {
        self.documents
            .borrow()
            .iter()
            .find(|d| self.tab_view.selected_page().as_ref() == Some(&d.page))
            .map(|d| d.editor.buffer())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn barra_de_pestanas_visible_solo_desde_dos() {
        // Con cero o una pestaña la barra se oculta; a partir de dos, aparece.
        assert!(!barra_visible(0));
        assert!(!barra_visible(1));
        assert!(barra_visible(2));
        assert!(barra_visible(3));
    }

    #[test]
    fn reload_limpio_con_cambios_recarga() {
        // Buffer limpio + fichero cambiado en disco: recarga automática.
        assert_eq!(decidir_recarga(true, true, true), AccionRecarga::Recargar);
    }

    #[test]
    fn reload_sucio_con_cambios_pregunta() {
        // Buffer con cambios + fichero cambiado: preguntar antes de descartar.
        assert_eq!(decidir_recarga(false, true, true), AccionRecarga::Preguntar);
    }

    #[test]
    fn reload_limpio_sin_cambios_no_hace_nada() {
        // Sin cambios externos no hay nada que atender.
        assert_eq!(decidir_recarga(true, false, true), AccionRecarga::Nada);
        assert_eq!(decidir_recarga(false, false, true), AccionRecarga::Nada);
    }

    #[test]
    fn reload_fichero_borrado_mantiene_el_buffer() {
        // Fichero borrado en disco: conservar el buffer y marcar sin guardar,
        // sin importar si el buffer estaba limpio o sucio.
        assert_eq!(
            decidir_recarga(true, true, false),
            AccionRecarga::MantenerSinFichero
        );
        assert_eq!(
            decidir_recarga(false, true, false),
            AccionRecarga::MantenerSinFichero
        );
    }

    #[test]
    fn cierre_borrador_vacio_no_pregunta() {
        // Borrador sin título con el buffer vacío: no hay nada que perder, se
        // cierra directamente aunque el flag de modificado siga activo (issue
        // #101: escribir y borrar todo no debe pedir confirmación).
        assert!(!requiere_confirmacion_cierre(true, false, true));
        assert!(!requiere_confirmacion_cierre(false, false, true));
    }

    #[test]
    fn cierre_borrador_con_texto_pregunta() {
        // Borrador sin título con contenido: perdería texto, confirmar.
        assert!(requiere_confirmacion_cierre(true, false, false));
    }

    #[test]
    fn cierre_fichero_modificado_pregunta() {
        // Documento con fichero: el vaciado del buffer es un cambio real
        // sobre el fichero en disco; confirmar. Limpio: cerrar directo.
        assert!(requiere_confirmacion_cierre(true, true, true));
        assert!(requiere_confirmacion_cierre(true, true, false));
        assert!(!requiere_confirmacion_cierre(false, true, true));
        assert!(!requiere_confirmacion_cierre(false, true, false));
    }
}
