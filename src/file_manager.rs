use gtk4::prelude::*;
use std::path::PathBuf;

/// ¿Es la cancelación del usuario? El diálogo la entrega como un `gio::Error`
/// `G_IO_ERROR_CANCELLED`; hay que distinguirla de un fallo real para no
/// mostrar un toast de error cada vez que se cierra el diálogo sin elegir.
fn is_cancelled(e: &glib::Error) -> bool {
    e.kind::<gio::IOErrorEnum>() == Some(gio::IOErrorEnum::Cancelled)
}

pub enum Outcome {
    Ok(PathBuf),
    Cancelled,
    Error(String),
}

pub enum OpenOutcome {
    Ok((PathBuf, String)),
    Cancelled,
    Error(String),
}

/// Añade el sufijo `.md` si el nombre elegido no lleva una extensión
/// Markdown: el diálogo no lo pone solo y el documento acabaría sin
/// extensión reconocible.
fn with_md_suffix(path: PathBuf) -> PathBuf {
    let has_markdown_suffix = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| matches!(e.to_ascii_lowercase().as_str(), "md" | "markdown"))
        .unwrap_or(false);
    if has_markdown_suffix {
        path
    } else {
        let mut name = path.into_os_string();
        name.push(".md");
        PathBuf::from(name)
    }
}

pub struct FileManager;

impl FileManager {
    pub fn new() -> Self {
        Self
    }

    pub fn open<F: Fn(OpenOutcome) + 'static>(&self, parent: &impl IsA<gtk4::Window>, callback: F) {
        let dialog = gtk4::FileDialog::builder().title("Abrir documento").build();

        let filter = gtk4::FileFilter::new();
        filter.add_suffix("md");
        filter.add_suffix("markdown");
        filter.add_suffix("txt");
        filter.set_name(Some("Documentos Markdown"));

        let filters = gio::ListStore::new::<gtk4::FileFilter>();
        filters.append(&filter);
        dialog.set_filters(Some(&filters));

        dialog.open(Some(parent), gio::Cancellable::NONE, move |result| {
            let outcome = match result {
                Ok(file) => match file.path() {
                    Some(path) => match std::fs::read_to_string(&path) {
                        Ok(content) => OpenOutcome::Ok((path, content)),
                        Err(e) => OpenOutcome::Error(e.to_string()),
                    },
                    None => OpenOutcome::Cancelled,
                },
                Err(e) if is_cancelled(&e) => OpenOutcome::Cancelled,
                Err(e) => OpenOutcome::Error(e.to_string()),
            };
            callback(outcome);
        });
    }

    pub fn save<F: Fn(Outcome) + 'static>(
        &self,
        parent: &impl IsA<gtk4::Window>,
        current: Option<&PathBuf>,
        content: &str,
        callback: F,
    ) {
        if let Some(path) = current {
            match std::fs::write(path, content) {
                Ok(()) => callback(Outcome::Ok(path.clone())),
                Err(e) => callback(Outcome::Error(e.to_string())),
            }
            return;
        }

        let dialog = gtk4::FileDialog::builder()
            .title("Guardar documento")
            .build();

        let filter = gtk4::FileFilter::new();
        filter.add_suffix("md");
        filter.set_name(Some("Markdown"));

        let filters = gio::ListStore::new::<gtk4::FileFilter>();
        filters.append(&filter);
        dialog.set_filters(Some(&filters));

        let content = content.to_string();
        dialog.save(Some(parent), gio::Cancellable::NONE, move |result| {
            let outcome = match result {
                Ok(file) => match file.path() {
                    Some(path) => {
                        let path = with_md_suffix(path);
                        match std::fs::write(&path, &content) {
                            Ok(()) => Outcome::Ok(path),
                            Err(e) => Outcome::Error(e.to_string()),
                        }
                    }
                    None => Outcome::Cancelled,
                },
                Err(e) if is_cancelled(&e) => Outcome::Cancelled,
                Err(e) => Outcome::Error(e.to_string()),
            };
            callback(outcome);
        });
    }
}
