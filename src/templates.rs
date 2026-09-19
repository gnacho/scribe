//! Plantillas para documentos nuevos.
//!
//! Son ficheros `.md` sueltos en `$XDG_DATA_HOME/scribe/templates`, así que el
//! usuario puede añadir, editar o borrar las suyas sin tocar la aplicación.
//! Admiten un puñado de marcadores que se sustituyen al crear el documento.

use gettextrs::gettext;
use std::path::PathBuf;

pub struct Template {
    pub name: String,
    pub path: PathBuf,
}

impl Template {
    pub fn body(&self) -> Option<String> {
        std::fs::read_to_string(&self.path).ok()
    }
}

pub fn dir() -> PathBuf {
    glib::user_data_dir().join("scribe").join("templates")
}

/// Built-in template names and bodies, localized at first run. Existing
/// template files on disk are never overwritten, so upgrading keeps whatever
/// language they were created with.
fn built_in() -> [(String, String); 4] {
    [
        (gettext("Note"), gettext("# {{title}}\n\n*{{date}}*\n\n\n")),
        (
            gettext("Journal"),
            gettext("# {{date}}\n\n## What happened\n\n\n\n## What I learned\n\n\n\n## Tomorrow\n\n- \n"),
        ),
        (
            gettext("Meeting notes"),
            gettext("# {{title}}\n\n- **Date:** {{datetime}}\n- **Attendees:** \n\n## Topics\n\n1. \n\n## Agreements\n\n- \n\n## Tasks\n\n- [ ] \n"),
        ),
        (
            gettext("Article"),
            gettext("---\ntitle: {{title}}\ndate: {{date}}\ndraft: true\n---\n\n# {{title}}\n\n## Introduction\n\n\n\n## Development\n\n\n\n## Conclusion\n\n\n"),
        ),
    ]
}

/// Crea la carpeta y escribe las plantillas de ejemplo si aún no existen.
pub fn ensure_defaults() {
    let dir = dir();
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    for (name, body) in built_in() {
        let path = dir.join(format!("{name}.md"));
        if !path.exists() {
            let _ = std::fs::write(&path, body);
        }
    }
}

pub fn list() -> Vec<Template> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir()) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_markdown = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| matches!(e, "md" | "markdown" | "txt"));
        if !is_markdown {
            continue;
        }
        if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
            out.push(Template {
                name: name.to_string(),
                path: path.clone(),
            });
        }
    }
    out.sort_by_cached_key(|t| t.name.to_lowercase());
    out
}

pub fn find(name: &str) -> Option<Template> {
    list().into_iter().find(|t| t.name == name)
}

/// Sustituye los marcadores admitidos: `{{title}}`, `{{date}}`, `{{time}}`,
/// `{{datetime}}` y `{{year}}`.
pub fn render(body: &str, title: &str) -> String {
    let now = glib::DateTime::now_local().ok();
    let fmt = |pattern: &str| -> String {
        now.as_ref()
            .and_then(|d| d.format(pattern).ok())
            .map(|s| s.to_string())
            .unwrap_or_default()
    };
    body.replace("{{title}}", title)
        .replace("{{date}}", &fmt("%Y-%m-%d"))
        .replace("{{time}}", &fmt("%H:%M"))
        .replace("{{datetime}}", &fmt("%Y-%m-%d %H:%M"))
        .replace("{{year}}", &fmt("%Y"))
}

/// Abre la carpeta de plantillas en el gestor de archivos del escritorio.
pub fn open_dir(parent: &impl gtk4::prelude::IsA<gtk4::Window>) {
    ensure_defaults();
    let launcher = gtk4::FileLauncher::new(Some(&gio::File::for_path(dir())));
    launcher.launch(Some(parent), gio::Cancellable::NONE, |_| {});
}

/// Nombre de documento sugerido a partir de la primera cabecera del texto.
pub fn title_from(text: &str) -> Option<String> {
    for line in text.lines().take(30) {
        let trimmed = line.trim_start();
        let hashes = trimmed.bytes().take_while(|b| *b == b'#').count();
        if (1..=6).contains(&hashes) {
            let rest = trimmed[hashes..].trim();
            if !rest.is_empty() {
                return Some(rest.to_string());
            }
        }
    }
    None
}
