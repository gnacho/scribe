# Scribe

A native GNOME Markdown editor with inline WYSIWYG, built with Rust, GTK4, libadwaita and WebKitGTK.

**Status: pre-alpha.** It opens, renders and lets you write Markdown with live WYSIWYG, but core features (real file saving, open dialogs, preferences) are still missing. Expect rough edges.

This repository continues the design exploration started in the old Gnome-MD mockups (now in [docs/mockups](docs/mockups/)): a distraction-free Markdown editor that follows the GNOME Human Interface Guidelines, inspired by Typora.

## Features (so far)

- Inline WYSIWYG: type `# Title` and it becomes an editable heading instantly.
- GNOME integration: libadwaita, automatic light/dark themes, adaptive headerbar.
- Editor engine: Milkdown (ProseMirror) inside WebKitGTK 6.0.
- Extensions: GFM tables, task lists, KaTeX math, emojis.
- Keyboard shortcuts: Ctrl+B bold, Ctrl+I italic, Ctrl+K inline code.

## Known gaps

- The Save button only prints to stdout (no file dialog yet).
- Milkdown loads from the jsDelivr CDN, so an internet connection is required. Vendoring the bundles is planned.

## Build requirements

- Rust 1.78+
- GTK4, libadwaita, WebKitGTK 6.0 and GLib development files
- `glib-compile-resources`

Arch/CachyOS:

```bash
sudo pacman -S rust gtk4 libadwaita webkitgtk-6.0 glib2
```

Debian/Ubuntu:

```bash
sudo apt install libgtk-4-dev libadwaita-1-dev libwebkitgtk-6.0-dev libglib2.0-dev
```

Fedora:

```bash
sudo dnf install gtk4-devel libadwaita-devel webkitgtk6.0-devel glib2-devel
```

## Build and run

```bash
cargo build --release
cargo run
```

## Flatpak

A Flatpak manifest is available at [build-aux/flatpak](build-aux/flatpak/) (work in progress):

```bash
cd build-aux/flatpak
flatpak-builder --user --install build-dir app.scribe.Scribe.json --force-clean
```

## License

AGPL-3.0. See [LICENSE](LICENSE).
