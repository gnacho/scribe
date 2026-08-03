use gtk4::prelude::*;
use std::path::PathBuf;

pub struct FileManager;

impl FileManager {
    pub fn new() -> Self {
        Self
    }

    pub fn open<F: Fn(Option<(PathBuf, String)>) + 'static>(
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
            gio::Cancellable::NONE,
            move |result| {
                let content = result.ok()
                    .and_then(|file| file.path())
                    .and_then(|path| std::fs::read_to_string(&path).ok().map(|c| (path, c)));
                callback(content);
            },
        );
    }

    pub fn save<F: Fn(Option<PathBuf>) + 'static>(
        &self,
        parent: &impl IsA<gtk4::Window>,
        current: Option<&PathBuf>,
        content: &str,
        callback: F,
    ) {
        if let Some(path) = current {
            let _ = std::fs::write(path, content);
            callback(Some(path.clone()));
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
        dialog.save(
            Some(parent),
            gio::Cancellable::NONE,
            move |result| {
                let path = result.ok().and_then(|file| file.path());
                if let Some(ref p) = path {
                    let _ = std::fs::write(p, &content);
                }
                callback(path);
            },
        );
    }
}
