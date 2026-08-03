use gtk4::prelude::*;
use webkit6::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub struct EditorBridge {
    webview: webkit6::WebView,
    title_callbacks: RefCell<Vec<Box<dyn Fn(&str)>>>,
    stats_callbacks: RefCell<Vec<Box<dyn Fn(u32, u32, u32)>>>,
    save_callbacks: RefCell<Vec<Box<dyn Fn(&str)>>>,
    toc_callbacks: RefCell<Vec<Box<dyn Fn(Vec<(u8, String)>)>>>,
}

impl EditorBridge {
    pub fn new(webview: &webkit6::WebView) -> Rc<Self> {
        let bridge = Rc::new(Self {
            webview: webview.clone(),
            title_callbacks: RefCell::new(Vec::new()),
            stats_callbacks: RefCell::new(Vec::new()),
            save_callbacks: RefCell::new(Vec::new()),
            toc_callbacks: RefCell::new(Vec::new()),
        });

        let user_content = webview.user_content_manager()
            .expect("WebView must have a UserContentManager");

        let script = webkit6::UserScript::new(
            r#"
            window.scribe = {
                post: function(type, data) {
                    if (window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.scribe) {
                        window.webkit.messageHandlers.scribe.postMessage(JSON.stringify({type: type, ...data}));
                    }
                }
            };
            "#,
            webkit6::UserContentInjectedFrames::AllFrames,
            webkit6::UserScriptInjectionTime::Start,
            &[],
            &[],
        );
        user_content.add_script(&script);

        let bridge_weak = Rc::downgrade(&bridge);
        user_content.connect_script_message_received(Some("scribe"), move |_ucm, value| {
            let s = value.to_str();
            {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&s) {
                    if let Some(type_val) = json.get("type").and_then(|v| v.as_str()) {
                        if let Some(strong) = bridge_weak.upgrade() {
                            match type_val {
                                "titleChanged" => {
                                    if let Some(title) = json.get("title").and_then(|v| v.as_str()) {
                                        for cb in strong.title_callbacks.borrow().iter() {
                                            cb(title);
                                        }
                                    }
                                }
                                "statsChanged" => {
                                    let words = json.get("words").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                                    let lines = json.get("lines").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                                    let chars = json.get("chars").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                                    for cb in strong.stats_callbacks.borrow().iter() {
                                        cb(words, lines, chars);
                                    }
                                }
                                "saveRequested" => {
                                    if let Some(content) = json.get("content").and_then(|v| v.as_str()) {
                                        for cb in strong.save_callbacks.borrow().iter() {
                                            cb(content);
                                        }
                                    }
                                }
                                "tocChanged" => {
                                    if let Some(toc) = json.get("toc").and_then(|v| v.as_array()) {
                                        let headings: Vec<(u8, String)> = toc.iter().filter_map(|item| {
                                            let level = item.get("level").and_then(|v| v.as_u64())? as u8;
                                            let text = item.get("text").and_then(|v| v.as_str())?.to_string();
                                            Some((level, text))
                                        }).collect();
                                        for cb in strong.toc_callbacks.borrow().iter() {
                                            cb(headings.clone());
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        });

        bridge
    }

    pub fn exec_command(&self, command: &str) {
        let event = serde_json::json!({ "command": command });
        let js = format!(
            "window.dispatchEvent(new CustomEvent('scribe-command', {{ detail: {} }}));",
            event
        );
        self.webview.evaluate_javascript(
            &js,
            None,
            None,
            gtk4::gio::Cancellable::NONE,
            move |result| {
                if let Err(e) = result {
                    eprintln!("JS eval error: {:?}", e);
                }
            },
        );
    }

    pub fn set_theme(&self, theme: &str) {
        let event = serde_json::json!({ "theme": theme });
        let js = format!(
            "window.dispatchEvent(new CustomEvent('scribe-theme', {{ detail: {} }}));",
            event
        );
        self.webview.evaluate_javascript(
            &js,
            None,
            None,
            gtk4::gio::Cancellable::NONE,
            |_| {},
        );
    }

    pub fn set_content(&self, content: &str) {
        let event = serde_json::json!({ "content": content });
        let js = format!(
            "window.dispatchEvent(new CustomEvent('scribe-set-content', {{ detail: {} }}));",
            event
        );
        self.webview.evaluate_javascript(
            &js,
            None,
            None,
            gtk4::gio::Cancellable::NONE,
            |_| {},
        );
    }

    pub fn request_save(&self) {
        let js = "window.dispatchEvent(new CustomEvent('scribe-save'));";
        self.webview.evaluate_javascript(
            js,
            None,
            None,
            gtk4::gio::Cancellable::NONE,
            |_| {},
        );
    }

    pub fn request_toc(&self) {
        let js = "window.dispatchEvent(new CustomEvent('scribe-get-toc'));";
        self.webview.evaluate_javascript(
            js,
            None,
            None,
            gtk4::gio::Cancellable::NONE,
            |_| {},
        );
    }

    pub fn connect_title_changed<F: Fn(&str) + 'static>(&self, callback: F) {
        self.title_callbacks.borrow_mut().push(Box::new(callback));
    }

    pub fn connect_stats_changed<F: Fn(u32, u32, u32) + 'static>(&self, callback: F) {
        self.stats_callbacks.borrow_mut().push(Box::new(callback));
    }

    pub fn connect_save_requested<F: Fn(&str) + 'static>(&self, callback: F) {
        self.save_callbacks.borrow_mut().push(Box::new(callback));
    }

    pub fn connect_toc_changed<F: Fn(Vec<(u8, String)>) + 'static>(&self, callback: F) {
        self.toc_callbacks.borrow_mut().push(Box::new(callback));
    }
}
