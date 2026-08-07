use gtk4::prelude::*;
use std::path::PathBuf;

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

        if let Some(path) = current {
            if let Some(parent_dir) = path.parent() {
                dialog.set_initial_folder(Some(&gio::File::for_path(parent_dir)));
            }
        }

        let content = content.to_string();
        dialog.save(Some(parent), gio::Cancellable::NONE, move |result| {
            let outcome = match result {
                Ok(file) => match file.path() {
                    Some(path) => match std::fs::write(&path, &content) {
                        Ok(()) => Outcome::Ok(path),
                        Err(e) => Outcome::Error(e.to_string()),
                    },
                    None => Outcome::Cancelled,
                },
                Err(e) => Outcome::Error(e.to_string()),
            };
            callback(outcome);
        });
    }
}
