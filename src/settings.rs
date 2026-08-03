use gio::prelude::*;
use gtk4::prelude::*;

pub struct AppSettings {
    settings: gio::Settings,
}

impl AppSettings {
    pub fn new() -> Self {
        let settings = gio::Settings::new("app.scribe.Scribe");
        Self { settings }
    }

    pub fn window_width(&self) -> i32 {
        self.settings.int("window-width")
    }

    pub fn set_window_width(&self, width: i32) {
        let _ = self.settings.set_int("window-width", width);
    }

    pub fn window_height(&self) -> i32 {
        self.settings.int("window-height")
    }

    pub fn set_window_height(&self, height: i32) {
        let _ = self.settings.set_int("window-height", height);
    }

    pub fn theme(&self) -> String {
        self.settings.string("theme").to_string()
    }

    pub fn set_theme(&self, theme: &str) {
        let _ = self.settings.set_string("theme", theme);
    }

    pub fn font_size(&self) -> i32 {
        self.settings.int("font-size")
    }

    pub fn line_spacing(&self) -> f64 {
        self.settings.double("line-spacing")
    }

    pub fn show_sidebar(&self) -> bool {
        self.settings.boolean("show-sidebar")
    }

    pub fn set_show_sidebar(&self, show: bool) {
        let _ = self.settings.set_boolean("show-sidebar", show);
    }

    pub fn autosave(&self) -> bool {
        self.settings.boolean("autosave")
    }

    pub fn connect_changed<F: Fn(&str) + 'static>(&self, callback: F) {
        self.settings.connect_changed(None, move |settings, key| {
            callback(key);
        });
    }
}
