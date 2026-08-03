use gio::prelude::*;
use gtk4::prelude::*;

pub struct AppSettings {
    settings: Option<gio::Settings>,
}

impl AppSettings {
    pub fn new() -> Self {
        // Try to load schema; if not installed, use defaults
        let settings = gio::SettingsSchemaSource::default()
            .and_then(|source| source.lookup("app.scribe.Scribe", false))
            .map(|_| gio::Settings::new("app.scribe.Scribe"));

        Self { settings }
    }

    fn with_settings<F, T>(&self, default: T, f: F) -> T
    where
        F: FnOnce(&gio::Settings) -> T,
    {
        match &self.settings {
            Some(s) => f(s),
            None => default,
        }
    }

    pub fn window_width(&self) -> i32 {
        self.with_settings(1100, |s| s.int("window-width"))
    }

    pub fn set_window_width(&self, width: i32) {
        if let Some(s) = &self.settings {
            let _ = s.set_int("window-width", width);
        }
    }

    pub fn window_height(&self) -> i32 {
        self.with_settings(800, |s| s.int("window-height"))
    }

    pub fn set_window_height(&self, height: i32) {
        if let Some(s) = &self.settings {
            let _ = s.set_int("window-height", height);
        }
    }

    pub fn theme(&self) -> String {
        self.with_settings("system".to_string(), |s| s.string("theme").to_string())
    }

    pub fn set_theme(&self, theme: &str) {
        if let Some(s) = &self.settings {
            let _ = s.set_string("theme", theme);
        }
    }

    pub fn font_size(&self) -> i32 {
        self.with_settings(15, |s| s.int("font-size"))
    }

    pub fn line_spacing(&self) -> f64 {
        self.with_settings(1.7, |s| s.double("line-spacing"))
    }

    pub fn show_sidebar(&self) -> bool {
        self.with_settings(true, |s| s.boolean("show-sidebar"))
    }

    pub fn set_show_sidebar(&self, show: bool) {
        if let Some(s) = &self.settings {
            let _ = s.set_boolean("show-sidebar", show);
        }
    }

    pub fn autosave(&self) -> bool {
        self.with_settings(true, |s| s.boolean("autosave"))
    }
}
