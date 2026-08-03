use gtk4::prelude::*;
use gtk4::gio;
use std::path::PathBuf;
use std::fs;

pub struct FileManager;

impl FileManager {
    pub fn new() -> Self {
        Self
    }

    pub fn open_file_dialog<F: Fn(Option<PathBuf>, Option<String>) + 'static>(
        &self,
        parent: &impl IsA<gtk4::Window>,
        callback: F,
    ) {
        let dialog = gtk4::FileDialog::builder()
            .title("Abrir documento")
            .build();

        let filter = gtk4::FileFilter::new();
        filter.add_suffix("md");
        filter.add_suffix("markdown");
        filter.add_suffix("txt");
        filter.set_name(Some("Documentos Markdown"));

        let filters = gio::ListStore::new::<gtk4::FileFilter>();
        filters.append(&filter);
        dialog.set_filters(Some(&filters));

        dialog.open(
            Some(parent),
            gtk4::gio::Cancellable::NONE,
            move |result| {
                match result {
                    Ok(file) => {
                        if let Some(path) = file.path() {
                            match fs::read_to_string(&path) {
                                Ok(content) => callback(Some(path), Some(content)),
                                Err(_) => callback(Some(path), None),
                            }
                        } else {
                            callback(None, None);
                        }
                    }
                    Err(_) => callback(None, None),
                }
            },
        );
    }

    pub fn save_file_dialog<F: Fn(Option<PathBuf>) + 'static>(
        &self,
        parent: &impl IsA<gtk4::Window>,
        current_path: Option<&PathBuf>,
        callback: F,
    ) {
        let dialog = gtk4::FileDialog::builder()
            .title("Guardar documento")
            .build();

        let filter = gtk4::FileFilter::new();
        filter.add_suffix("md");
        filter.set_name(Some("Markdown"));

        let filters = gio::ListStore::new::<gtk4::FileFilter>();
        filters.append(&filter);
        dialog.set_filters(Some(&filters));

        if let Some(path) = current_path {
            if let Some(file) = gtk4::gio::File::for_path(path).parent() {
                dialog.set_initial_folder(Some(&file));
            }
        }

        dialog.save(
            Some(parent),
            gtk4::gio::Cancellable::NONE,
            move |result| {
                match result {
                    Ok(file) => {
                        callback(file.path());
                    }
                    Err(_) => callback(None),
                }
            },
        );
    }

    pub fn save_to_path(&self, path: &PathBuf, content: &str) -> Result<(), std::io::Error> {
        fs::write(path, content)
    }
}
