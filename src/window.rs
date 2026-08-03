use gtk4::prelude::*;
use libadwaita::prelude::*;
use webkit6::prelude::*;
use libadwaita as adw;
use std::rc::Rc;

use crate::editor::EditorBridge;

const EDITOR_HTML: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/editor/index.html"));

pub struct ScribeWindow {
    pub window: adw::ApplicationWindow,
    bridge: Rc<EditorBridge>,
}

impl ScribeWindow {
    pub fn new(app: &adw::Application) -> Self {
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .default_width(1000)
            .default_height(750)
            .build();

        let header = adw::HeaderBar::new();
        let title_widget = adw::WindowTitle::new("Sin título", "Inkwell");
        header.set_title_widget(Some(&title_widget));

        let format_btn = gtk4::Button::from_icon_name("format-text-bold-symbolic");
        format_btn.set_tooltip_text(Some("Formato (Ctrl+B)"));
        header.pack_start(&format_btn);

        let theme_btn = gtk4::Button::from_icon_name("weather-clear-night-symbolic");
        theme_btn.set_tooltip_text(Some("Cambiar tema"));
        header.pack_end(&theme_btn);

        let export_btn = gtk4::Button::from_icon_name("document-save-symbolic");
        export_btn.set_tooltip_text(Some("Guardar"));
        header.pack_end(&export_btn);

        let webview = webkit6::WebView::new();
        webview.set_hexpand(true);
        webview.set_vexpand(true);
        webview.set_background_color(&gdk4::RGBA::new(0.0, 0.0, 0.0, 0.0));

        let settings = webkit6::prelude::WebViewExt::settings(&webview).expect("WebView debe tener settings");
        settings.set_enable_javascript(true);
        settings.set_enable_developer_extras(true);
        settings.set_javascript_can_access_clipboard(true);

        // Cargar HTML inline en lugar de resource:///
        webview.load_html(EDITOR_HTML, Some("file:///"));

        let bridge = EditorBridge::new(&webview);

        let bridge_clone = bridge.clone();
        format_btn.connect_clicked(move |_| {
            bridge_clone.exec_command("toggleBold");
        });

        let bridge_clone = bridge.clone();
        theme_btn.connect_clicked(move |_| {
            let style = adw::StyleManager::default();
            let is_dark = style.is_dark();
            style.set_color_scheme(if is_dark {
                adw::ColorScheme::ForceLight
            } else {
                adw::ColorScheme::ForceDark
            });
            bridge_clone.set_theme(if is_dark { "light" } else { "dark" });
        });

        let bridge_clone = bridge.clone();
        export_btn.connect_clicked(move |_| {
            bridge_clone.request_save();
        });

        let title_widget_clone = title_widget.clone();
        bridge.connect_title_changed(move |title| {
            title_widget_clone.set_title(title);
        });

        let scrolled = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .child(&webview)
            .build();

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&scrolled));

        window.set_content(Some(&toolbar_view));

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

        Self { window, bridge }
    }

    pub fn present(&self) {
        self.window.present();
    }
}
