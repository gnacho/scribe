use gio::prelude::*;

/// Cuándo se muestran las marcas de Markdown (`**`, `#`, backticks, URLs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkupVisibility {
    /// Siempre ocultas. Mientras la puerta [`gtk_hides_invisible_safely`] esté
    /// cerrada, las marcas sustituidas se encogen (`syn_shrink`) en vez de
    /// ocultarse, y las demás se atenúan.
    Hidden,
    /// Ocultas salvo en la línea del cursor. Sin la puerta equivale a
    /// [`Self::Hidden`]: ya no hay revelado por línea.
    Focus,
    /// Siempre visibles, pero atenuadas.
    Dim,
}

/// ¿Puede el GTK del sistema ocultar texto sin riesgo de aborto?
///
/// GTK aborta en `gtk_text_iter_set_visible_line_index` («byte index off the
/// end of the line») cuando un buffer contiene texto invisible y su
/// maquetación perezosa conserva índices calculados con otra visibilidad
/// (GNOME/gtk#8346; el fix, MR !10228, aún no está publicado en ninguna
/// versión). Mientras no exista una versión con el parche — verificable con
/// el test canario `tests/gtk_invisible_canary.rs` — esto devuelve `false`
/// y nada en el editor se oculta.
///
/// Al publicarse el fix en GTK, hay que comparar aquí
/// `gtk4::major_version()`/`minor_version()`/`micro_version()` contra la
/// primera versión que lo incluya.
pub fn gtk_hides_invisible_safely() -> bool {
    // SIGUE CERRADA (19-Oct-2026): con la puerta abierta sobre GTK 4.24.1 la
    // bateria de display aborta (SIGABRT, gtk#8346: texto invisible + indices
    // de linea visibles). El ocultado seguro es `syn_shrink` (escala minima),
    // que no excluye texto del layout.
    false
}

impl MarkupVisibility {
    fn from_nick(nick: &str) -> Self {
        match nick {
            "hidden" => Self::Hidden,
            "dim" => Self::Dim,
            _ => Self::Focus,
        }
    }
    pub fn nick(self) -> &'static str {
        match self {
            Self::Hidden => "hidden",
            Self::Focus => "focus",
            Self::Dim => "dim",
        }
    }
    pub fn index(self) -> u32 {
        match self {
            Self::Hidden => 0,
            Self::Focus => 1,
            Self::Dim => 2,
        }
    }
    pub fn from_index(i: u32) -> Self {
        match i {
            0 => Self::Hidden,
            2 => Self::Dim,
            _ => Self::Focus,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontFamily {
    Sans,
    Serif,
    Mono,
}

impl FontFamily {
    fn from_nick(nick: &str) -> Self {
        match nick {
            "serif" => Self::Serif,
            "mono" => Self::Mono,
            _ => Self::Sans,
        }
    }
    pub fn nick(self) -> &'static str {
        match self {
            Self::Sans => "sans",
            Self::Serif => "serif",
            Self::Mono => "mono",
        }
    }
    pub fn index(self) -> u32 {
        match self {
            Self::Sans => 0,
            Self::Serif => 1,
            Self::Mono => 2,
        }
    }
    pub fn from_index(i: u32) -> Self {
        match i {
            1 => Self::Serif,
            2 => Self::Mono,
            _ => Self::Sans,
        }
    }
    /// Pila de familias CSS. El cuerpo va en proporcional; la monoespaciada
    /// queda reservada para el código aunque se elija "mono" como cuerpo.
    pub fn css_stack(self) -> &'static str {
        match self {
            Self::Sans => "Cantarell, 'Adwaita Sans', 'Noto Sans', sans-serif",
            Self::Serif => "'Source Serif 4', 'Noto Serif', 'Liberation Serif', serif",
            Self::Mono => "'Adwaita Mono', 'Source Code Pro', 'Fira Mono', monospace",
        }
    }
}

/// Tema de color del PDF exportado. Independiente del tema de lectura.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportColorScheme {
    /// Sigue el tema del editor (claro/oscuro del sistema).
    System,
    Light,
    Dark,
    /// Blanco y negro: sin color, para imprimir.
    Mono,
}

impl ExportColorScheme {
    fn from_nick(nick: &str) -> Self {
        match nick {
            "dark" => Self::Dark,
            "mono" => Self::Mono,
            "system" => Self::System,
            _ => Self::Light,
        }
    }
    pub fn nick(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
            Self::Mono => "mono",
        }
    }
    pub fn index(self) -> u32 {
        match self {
            Self::System => 0,
            Self::Light => 1,
            Self::Dark => 2,
            Self::Mono => 3,
        }
    }
    pub fn from_index(i: u32) -> Self {
        match i {
            0 => Self::System,
            2 => Self::Dark,
            3 => Self::Mono,
            _ => Self::Light,
        }
    }
}

/// Fuente monoespaciada del código en el PDF exportado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportMonoFont {
    AdwaitaMono,
    SourceCodePro,
    FiraMono,
}

impl ExportMonoFont {
    fn from_nick(nick: &str) -> Self {
        match nick {
            "source-code-pro" => Self::SourceCodePro,
            "fira-mono" => Self::FiraMono,
            _ => Self::AdwaitaMono,
        }
    }
    pub fn nick(self) -> &'static str {
        match self {
            Self::AdwaitaMono => "adwaita-mono",
            Self::SourceCodePro => "source-code-pro",
            Self::FiraMono => "fira-mono",
        }
    }
    pub fn index(self) -> u32 {
        match self {
            Self::AdwaitaMono => 0,
            Self::SourceCodePro => 1,
            Self::FiraMono => 2,
        }
    }
    pub fn from_index(i: u32) -> Self {
        match i {
            1 => Self::SourceCodePro,
            2 => Self::FiraMono,
            _ => Self::AdwaitaMono,
        }
    }
    /// Nombre de familia Pango/Fontconfig para cairo.
    pub fn family(self) -> &'static str {
        match self {
            Self::AdwaitaMono => "Adwaita Mono",
            Self::SourceCodePro => "Source Code Pro",
            Self::FiraMono => "Fira Mono",
        }
    }
}

/// Tamaño de página del PDF exportado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportPageSize {
    A4,
    Letter,
}

impl ExportPageSize {
    fn from_nick(nick: &str) -> Self {
        if nick == "letter" {
            Self::Letter
        } else {
            Self::A4
        }
    }
    pub fn nick(self) -> &'static str {
        match self {
            Self::A4 => "a4",
            Self::Letter => "letter",
        }
    }
    pub fn index(self) -> u32 {
        match self {
            Self::A4 => 0,
            Self::Letter => 1,
        }
    }
    pub fn from_index(i: u32) -> Self {
        if i == 1 {
            Self::Letter
        } else {
            Self::A4
        }
    }
    /// Nombre de papel para `gtk::PaperSize::new`.
    pub fn paper_name(self) -> &'static str {
        match self {
            Self::A4 => "iso_a4",
            Self::Letter => "na_letter",
        }
    }
}

/// Orientación de la página del PDF exportado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportOrientation {
    Portrait,
    Landscape,
}

impl ExportOrientation {
    fn from_nick(nick: &str) -> Self {
        if nick == "landscape" {
            Self::Landscape
        } else {
            Self::Portrait
        }
    }
    pub fn nick(self) -> &'static str {
        match self {
            Self::Portrait => "portrait",
            Self::Landscape => "landscape",
        }
    }
    pub fn index(self) -> u32 {
        match self {
            Self::Portrait => 0,
            Self::Landscape => 1,
        }
    }
    pub fn from_index(i: u32) -> Self {
        if i == 1 {
            Self::Landscape
        } else {
            Self::Portrait
        }
    }
}

/// Plantilla de página del PDF exportado. `Custom` usa los ajustes
/// granulares; el resto fija sus propios valores de maquetación.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportTemplate {
    Custom,
    Article,
    Manuscript,
    Letter,
    Compact,
}

impl ExportTemplate {
    fn from_nick(nick: &str) -> Self {
        match nick {
            "article" => Self::Article,
            "manuscript" => Self::Manuscript,
            "letter" => Self::Letter,
            "compact" => Self::Compact,
            _ => Self::Custom,
        }
    }
    pub fn nick(self) -> &'static str {
        match self {
            Self::Custom => "custom",
            Self::Article => "article",
            Self::Manuscript => "manuscript",
            Self::Letter => "letter",
            Self::Compact => "compact",
        }
    }
    pub fn index(self) -> u32 {
        match self {
            Self::Custom => 0,
            Self::Article => 1,
            Self::Manuscript => 2,
            Self::Letter => 3,
            Self::Compact => 4,
        }
    }
    pub fn from_index(i: u32) -> Self {
        match i {
            1 => Self::Article,
            2 => Self::Manuscript,
            3 => Self::Letter,
            4 => Self::Compact,
            _ => Self::Custom,
        }
    }
}

pub struct AppSettings {
    settings: Option<gio::Settings>,
}

/// Registra un fallo al escribir una clave (p. ej. bloqueada por dconf):
/// sin esto, la preferencia parecería aplicarse y no se habría guardado.
fn log_set_error(key: &str, e: glib::BoolError) {
    glib::g_warning!(
        "scribe",
        "no se pudo guardar la clave «{key}» en GSettings: {e}"
    );
}

macro_rules! prop {
    ($get:ident, $set:ident, $key:literal, i32, $default:expr) => {
        pub fn $get(&self) -> i32 {
            self.get($default, |s| s.int($key))
        }
        pub fn $set(&self, v: i32) {
            self.set(|s| {
                if let Err(e) = s.set_int($key, v) {
                    log_set_error($key, e);
                }
            });
        }
    };
    ($get:ident, $set:ident, $key:literal, f64, $default:expr) => {
        pub fn $get(&self) -> f64 {
            self.get($default, |s| s.double($key))
        }
        pub fn $set(&self, v: f64) {
            self.set(|s| {
                if let Err(e) = s.set_double($key, v) {
                    log_set_error($key, e);
                }
            });
        }
    };
    ($get:ident, $set:ident, $key:literal, bool, $default:expr) => {
        pub fn $get(&self) -> bool {
            self.get($default, |s| s.boolean($key))
        }
        pub fn $set(&self, v: bool) {
            self.set(|s| {
                if let Err(e) = s.set_boolean($key, v) {
                    log_set_error($key, e);
                }
            });
        }
    };
    ($get:ident, $set:ident, $key:literal, String, $default:expr) => {
        pub fn $get(&self) -> String {
            self.get($default.to_string(), |s| s.string($key).to_string())
        }
        pub fn $set(&self, v: &str) {
            self.set(|s| {
                if let Err(e) = s.set_string($key, v) {
                    log_set_error($key, e);
                }
            });
        }
    };
}

impl AppSettings {
    pub fn new() -> Self {
        let settings = gio::SettingsSchemaSource::default()
            .and_then(|source| source.lookup("io.github.gnacho.Scribe", true))
            .map(|_| gio::Settings::new("io.github.gnacho.Scribe"));
        if settings.is_none() {
            eprintln!(
                "scribe: no se encontró el esquema GSettings 'io.github.gnacho.Scribe'. \
                 Se usan los valores por defecto y no se guardarán las preferencias. \
                 En desarrollo: glib-compile-schemas data/ && \
                 GSETTINGS_SCHEMA_DIR=$PWD/data cargo run"
            );
        }
        Self { settings }
    }

    fn get<T>(&self, default: T, f: impl FnOnce(&gio::Settings) -> T) -> T {
        self.settings.as_ref().map(f).unwrap_or(default)
    }

    fn set(&self, f: impl FnOnce(&gio::Settings)) {
        if let Some(s) = &self.settings {
            f(s);
        }
    }

    prop!(window_width, set_window_width, "window-width", i32, 1100);
    prop!(window_height, set_window_height, "window-height", i32, 800);
    prop!(
        window_maximized,
        set_window_maximized,
        "window-maximized",
        bool,
        false
    );
    prop!(show_sidebar, set_show_sidebar, "show-sidebar", bool, false);
    prop!(show_preview, set_show_preview, "show-preview", bool, false);
    prop!(font_size, set_font_size, "font-size", i32, 16);
    prop!(line_spacing, set_line_spacing, "line-spacing", f64, 1.7);
    prop!(column_width, set_column_width, "column-width", i32, 720);
    prop!(spellcheck, set_spellcheck, "spellcheck", bool, true);
    prop!(focus_mode, set_focus_mode, "focus-mode", bool, false);
    prop!(
        typewriter_mode,
        set_typewriter_mode,
        "typewriter-mode",
        bool,
        false
    );
    prop!(
        continue_lists,
        set_continue_lists,
        "continue-lists",
        bool,
        true
    );
    prop!(tab_width, set_tab_width, "tab-width", i32, 4);
    prop!(autosave, set_autosave, "autosave", bool, true);
    prop!(
        autosave_interval,
        set_autosave_interval,
        "autosave-interval",
        i32,
        30
    );

    /// Id del tema semántico (`auto`, `light`, `dark`, `sepia`,
    /// `solarized-dark`, `gruvbox-dark` o el id de un tema de usuario).
    pub fn theme(&self) -> String {
        self.get("auto".to_string(), |s| s.string("theme").to_string())
    }
    pub fn set_theme(&self, v: &str) {
        self.set(|s| {
            if let Err(e) = s.set_string("theme", v) {
                log_set_error("theme", e);
            }
        });
    }

    pub fn markup_visibility(&self) -> MarkupVisibility {
        MarkupVisibility::from_nick(&self.get("focus".to_string(), |s| {
            s.string("markup-visibility").to_string()
        }))
    }
    pub fn set_markup_visibility(&self, v: MarkupVisibility) {
        self.set(|s| {
            if let Err(e) = s.set_string("markup-visibility", v.nick()) {
                log_set_error("markup-visibility", e);
            }
        });
    }

    pub fn font_family(&self) -> FontFamily {
        FontFamily::from_nick(
            &self.get("sans".to_string(), |s| s.string("font-family").to_string()),
        )
    }
    pub fn set_font_family(&self, v: FontFamily) {
        self.set(|s| {
            if let Err(e) = s.set_string("font-family", v.nick()) {
                log_set_error("font-family", e);
            }
        });
    }

    /// 0 = sistema, 1 = claro, 2 = oscuro.
    pub fn color_scheme_index(&self) -> u32 {
        match self.color_scheme_nick().as_str() {
            "light" => 1,
            "dark" => 2,
            _ => 0,
        }
    }

    /// Nick del ajuste `color-scheme`: «system», «light» o «dark».
    pub fn color_scheme_nick(&self) -> String {
        let nick = self.get("system".to_string(), |s| {
            s.string("color-scheme").to_string()
        });
        match nick.as_str() {
            "light" | "dark" => nick,
            _ => "system".to_string(),
        }
    }
    pub fn set_color_scheme_index(&self, index: u32) {
        self.set_color_scheme_nick(match index {
            1 => "light",
            2 => "dark",
            _ => "system",
        });
    }

    /// Fija el esquema por nick («system», «light», «dark»). Un valor
    /// desconocido cae en «system», igual que al leerlo.
    pub fn set_color_scheme_nick(&self, nick: &str) {
        let nick = match nick {
            "light" | "dark" => nick,
            _ => "system",
        };
        self.set(|s| {
            if let Err(e) = s.set_string("color-scheme", nick) {
                log_set_error("color-scheme", e);
            }
        });
    }

    pub fn recent_files(&self) -> Vec<String> {
        self.get(Vec::new(), |s| {
            s.strv("recent-files")
                .iter()
                .map(|g| g.to_string())
                .collect()
        })
    }

    pub fn push_recent_file(&self, path: &str) {
        let mut list = self.recent_files();
        list.retain(|p| p != path);
        list.insert(0, path.to_string());
        list.truncate(20);
        self.set(|s| {
            let refs: Vec<&str> = list.iter().map(|s| s.as_str()).collect();
            if let Err(e) = s.set_strv("recent-files", refs) {
                log_set_error("recent-files", e);
            }
        });
    }

    pub fn clear_recent_files(&self) {
        self.set(|s| {
            if let Err(e) = s.set_strv("recent-files", Vec::<&str>::new()) {
                log_set_error("recent-files", e);
            }
        });
    }

    // ------------------------------------------------------------------
    // Ajustes de exportación a PDF: independientes de los de lectura.
    // ------------------------------------------------------------------

    prop!(
        export_font_size,
        set_export_font_size,
        "export-font-size",
        i32,
        11
    );

    pub fn export_color_scheme(&self) -> ExportColorScheme {
        ExportColorScheme::from_nick(&self.get("light".to_string(), |s| {
            s.string("export-color-scheme").to_string()
        }))
    }
    pub fn set_export_color_scheme(&self, v: ExportColorScheme) {
        self.set(|s| {
            if let Err(e) = s.set_string("export-color-scheme", v.nick()) {
                log_set_error("export-color-scheme", e);
            }
        });
    }

    pub fn export_font_family(&self) -> FontFamily {
        FontFamily::from_nick(&self.get("sans".to_string(), |s| {
            s.string("export-font-family").to_string()
        }))
    }
    pub fn set_export_font_family(&self, v: FontFamily) {
        self.set(|s| {
            if let Err(e) = s.set_string("export-font-family", v.nick()) {
                log_set_error("export-font-family", e);
            }
        });
    }

    pub fn export_mono_font(&self) -> ExportMonoFont {
        ExportMonoFont::from_nick(&self.get("adwaita-mono".to_string(), |s| {
            s.string("export-mono-font").to_string()
        }))
    }
    pub fn set_export_mono_font(&self, v: ExportMonoFont) {
        self.set(|s| {
            if let Err(e) = s.set_string("export-mono-font", v.nick()) {
                log_set_error("export-mono-font", e);
            }
        });
    }

    pub fn export_page_size(&self) -> ExportPageSize {
        ExportPageSize::from_nick(&self.get("a4".to_string(), |s| {
            s.string("export-page-size").to_string()
        }))
    }
    pub fn set_export_page_size(&self, v: ExportPageSize) {
        self.set(|s| {
            if let Err(e) = s.set_string("export-page-size", v.nick()) {
                log_set_error("export-page-size", e);
            }
        });
    }

    pub fn export_orientation(&self) -> ExportOrientation {
        ExportOrientation::from_nick(&self.get("portrait".to_string(), |s| {
            s.string("export-orientation").to_string()
        }))
    }
    pub fn set_export_orientation(&self, v: ExportOrientation) {
        self.set(|s| {
            if let Err(e) = s.set_string("export-orientation", v.nick()) {
                log_set_error("export-orientation", e);
            }
        });
    }

    pub fn export_template(&self) -> ExportTemplate {
        ExportTemplate::from_nick(&self.get("custom".to_string(), |s| {
            s.string("export-template").to_string()
        }))
    }
    pub fn set_export_template(&self, v: ExportTemplate) {
        self.set(|s| {
            if let Err(e) = s.set_string("export-template", v.nick()) {
                log_set_error("export-template", e);
            }
        });
    }

    /// Ruta de la hoja de estilo del usuario. Vacía significa "ninguna".
    pub fn export_css_path(&self) -> Option<std::path::PathBuf> {
        let ruta = self.get(String::new(), |s| s.string("export-css-path").to_string());
        if ruta.trim().is_empty() {
            None
        } else {
            Some(std::path::PathBuf::from(ruta))
        }
    }
    pub fn set_export_css_path(&self, ruta: Option<&std::path::Path>) {
        let valor = ruta
            .map(|r| r.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.set(|s| {
            if let Err(e) = s.set_string("export-css-path", &valor) {
                log_set_error("export-css-path", e);
            }
        });
    }
}
