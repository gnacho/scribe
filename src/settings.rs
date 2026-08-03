use gio::prelude::*;

pub struct AppSettings {
    settings: Option<gio::Settings>,
}

impl AppSettings {
    pub fn new() -> Self {
        let settings = gio::SettingsSchemaSource::default()
            .and_then(|source| source.lookup("app.scribe.Scribe", false))
            .map(|_| gio::Settings::new("app.scribe.Scribe"));
        Self { settings }
    }

    fn get<T: Clone>(&self, key: &str, default: T, f: impl FnOnce(&gio::Settings) -> T) -> T {
        self.settings.as_ref().map(f).unwrap_or(default)
    }

    pub fn window_width(&self) -> i32 { self.get("window-width", 1100, |s| s.int("window-width")) }
    pub fn set_window_width(&self, v: i32) { if let Some(s) = &self.settings { let _ = s.set_int("window-width", v); } }

    pub fn window_height(&self) -> i32 { self.get("window-height", 800, |s| s.int("window-height")) }
    pub fn set_window_height(&self, v: i32) { if let Some(s) = &self.settings { let _ = s.set_int("window-height", v); } }

    pub fn show_sidebar(&self) -> bool { self.get("show-sidebar", true, |s| s.boolean("show-sidebar")) }
    pub fn set_show_sidebar(&self, v: bool) { if let Some(s) = &self.settings { let _ = s.set_boolean("show-sidebar", v); } }

    pub fn show_preview(&self) -> bool { self.get("show-preview", false, |s| s.boolean("show-preview")) }
    pub fn set_show_preview(&self, v: bool) { if let Some(s) = &self.settings { let _ = s.set_boolean("show-preview", v); } }

    pub fn autosave(&self) -> bool { self.get("autosave", true, |s| s.boolean("autosave")) }
    pub fn font_size(&self) -> i32 { self.get("font-size", 15, |s| s.int("font-size")) }
    pub fn line_spacing(&self) -> f64 { self.get("line-spacing", 1.7, |s| s.double("line-spacing")) }
}
