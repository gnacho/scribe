use gio::prelude::*;

/// Cuándo se muestran las marcas de Markdown (`**`, `#`, backticks, URLs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkupVisibility {
    /// Siempre ocultas.
    Hidden,
    /// Ocultas salvo en la línea del cursor.
    Focus,
    /// Siempre visibles, pero atenuadas.
    Dim,
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

pub struct AppSettings {
    settings: Option<gio::Settings>,
}

macro_rules! prop {
    ($get:ident, $set:ident, $key:literal, i32, $default:expr) => {
        pub fn $get(&self) -> i32 {
            self.get($default, |s| s.int($key))
        }
        pub fn $set(&self, v: i32) {
            self.set(|s| {
                let _ = s.set_int($key, v);
            });
        }
    };
    ($get:ident, $set:ident, $key:literal, f64, $default:expr) => {
        pub fn $get(&self) -> f64 {
            self.get($default, |s| s.double($key))
        }
        pub fn $set(&self, v: f64) {
            self.set(|s| {
                let _ = s.set_double($key, v);
            });
        }
    };
    ($get:ident, $set:ident, $key:literal, bool, $default:expr) => {
        pub fn $get(&self) -> bool {
            self.get($default, |s| s.boolean($key))
        }
        pub fn $set(&self, v: bool) {
            self.set(|s| {
                let _ = s.set_boolean($key, v);
            });
        }
    };
    ($get:ident, $set:ident, $key:literal, String, $default:expr) => {
        pub fn $get(&self) -> String {
            self.get($default.to_string(), |s| s.string($key).to_string())
        }
        pub fn $set(&self, v: &str) {
            self.set(|s| {
                let _ = s.set_string($key, v);
            });
        }
    };
}

impl AppSettings {
    pub fn new() -> Self {
        let settings = gio::SettingsSchemaSource::default()
            .and_then(|source| source.lookup("app.scribe.Scribe", true))
            .map(|_| gio::Settings::new("app.scribe.Scribe"));
        if settings.is_none() {
            eprintln!(
                "scribe: no se encontró el esquema GSettings 'app.scribe.Scribe'. \
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
    prop!(
        default_template,
        set_default_template,
        "default-template",
        String,
        ""
    );

    pub fn markup_visibility(&self) -> MarkupVisibility {
        MarkupVisibility::from_nick(&self.get("focus".to_string(), |s| {
            s.string("markup-visibility").to_string()
        }))
    }
    pub fn set_markup_visibility(&self, v: MarkupVisibility) {
        self.set(|s| {
            let _ = s.set_string("markup-visibility", v.nick());
        });
    }

    pub fn font_family(&self) -> FontFamily {
        FontFamily::from_nick(
            &self.get("sans".to_string(), |s| s.string("font-family").to_string()),
        )
    }
    pub fn set_font_family(&self, v: FontFamily) {
        self.set(|s| {
            let _ = s.set_string("font-family", v.nick());
        });
    }

    /// 0 = sistema, 1 = claro, 2 = oscuro.
    pub fn color_scheme_index(&self) -> u32 {
        match self
            .get("system".to_string(), |s| {
                s.string("color-scheme").to_string()
            })
            .as_str()
        {
            "light" => 1,
            "dark" => 2,
            _ => 0,
        }
    }
    pub fn set_color_scheme_index(&self, index: u32) {
        let nick = match index {
            1 => "light",
            2 => "dark",
            _ => "system",
        };
        self.set(|s| {
            let _ = s.set_string("color-scheme", nick);
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
            let _ = s.set_strv("recent-files", refs);
        });
    }

    pub fn clear_recent_files(&self) {
        self.set(|s| {
            let _ = s.set_strv("recent-files", Vec::<&str>::new());
        });
    }
}
