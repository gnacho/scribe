use gtk4::prelude::*;
use webkit6::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub struct EditorBridge {
    webview: webkit6::WebView,
    title_callbacks: RefCell<Vec<Box<dyn Fn(&str)>>>,
}

impl EditorBridge {
    pub fn new(webview: &webkit6::WebView) -> Rc<Self> {
        let bridge = Rc::new(Self {
            webview: webview.clone(),
            title_callbacks: RefCell::new(Vec::new()),
        });

        let user_content = webview
            .user_content_manager()
            .expect("WebView debe tener UserContentManager");

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

        user_content.register_script_message_handler("scribe", None);
        let bridge_weak = Rc::downgrade(&bridge);
        user_content.connect_script_message_received(Some("scribe"), move |_ucm, value| {
            let s = value.to_str();
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
                            "saveRequested" => {
                                if let Some(content) = json.get("content").and_then(|v| v.as_str()) {
                                    println!("Save content ({} chars)", content.len());
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        });

        bridge
    }

    pub fn exec_command(&self, command: &str) {
        let js = format!(
            r#"window.dispatchEvent(new CustomEvent('scribe-command', {{ detail: {{ command: '{}' }} }}));"#,
            command
        );
        self.webview.evaluate_javascript(
            &js,
            None,
            None,
            None::<&gtk4::gio::Cancellable>,
            move |result| {
                if let Err(e) = result {
                    eprintln!("JS eval error: {:?}", e);
                }
            },
        );
    }

    pub fn set_theme(&self, theme: &str) {
        let js = format!(
            r#"window.dispatchEvent(new CustomEvent('scribe-theme', {{ detail: {{ theme: '{}' }} }}));"#,
            theme
        );
        self.webview
            .evaluate_javascript(&js, None, None, None::<&gtk4::gio::Cancellable>, |_| {});
    }

    pub fn request_save(&self) {
        let js = r#"window.dispatchEvent(new CustomEvent('scribe-save'));"#;
        self.webview
            .evaluate_javascript(js, None, None, None::<&gtk4::gio::Cancellable>, |_| {});
    }

    pub fn connect_title_changed<F: Fn(&str) + 'static>(&self, callback: F) {
        self.title_callbacks.borrow_mut().push(Box::new(callback));
    }
}
