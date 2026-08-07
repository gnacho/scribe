# Scribe

<p align="center">
  <a href="README.md">English</a> |
  <a href="README.es.md">Español</a>
</p>

<p align="center">
  <a href="https://github.com/gnacho/scribe/releases"><img alt="Release" src="https://img.shields.io/github/v/release/gnacho/scribe"></a>
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/github/license/gnacho/scribe"></a>
</p>

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/hero-es-dark.png">
    <source media="(prefers-color-scheme: light)" srcset="assets/hero-es-light.png">
    <img alt="Scribe editor window showing Markdown text with live WYSIWYG rendering" src="assets/hero-es-light.png" width="800">
  </picture>
</p>

Scribe is a native GNOME Markdown editor built with Rust, GTK4 and
libadwaita. It renders Markdown live on the editing buffer so headings look
like headings and bold text looks bold, without switching to a separate
preview or browser engine.

## Why does this exist?

I wanted a Markdown editor that felt like part of GNOME, not a port of a web
app. The ones I tried either wrapped a browser engine (ProseMirror, Milkdown
inside WebKit) and consumed hundreds of megabytes, or they showed a plain
source buffer next to a rendered preview. I kept both panes open, going back
and forth between them instead of just writing.

The idea is an editor where the source text IS the preview: the buffer stays
plain Markdown, but the rendering happens on top with `GtkTextTag`. That way
there is nothing to sync, no HTML under the hood, no browser engine to
bundle. It started as mockups for a GTK4 design exploration and evolved into
something I actually use to draft notes and docs.

## Why this stack?

- **Rust + GTK4 + libadwaita** &mdash; native toolkit, no Electron. The binary
  is around 8 MB and idles at a few dozen megabytes of RAM. A WebKit-based
  editor would be ten times that before opening a file.
- **GtkSourceView 5** for the text buffer and **pulldown-cmark** for Markdown
  parsing, then custom `GtkTextTag` spans for rendering. No HTML or CSS
  involved in the preview. The editor is a single `GtkTextView` with
  decorations applied to the buffer.
- **No database, no server, no JavaScript.** It is a desktop application that
  opens, edits and saves files. Preferences go through GSettings, not a
  config file or a web UI.

## Features

- **Live WYSIWYG rendering** on the editing buffer: headings at real scale,
  bold/italic/strikethrough, inline and fenced code, quotes, nested lists
  with hanging indent, task lists, links, images, footnotes, rules, tables in
  monospace, and dimmed HTML blocks
- **Configurable markup visibility**: hide syntax markers like `**`, `#` and
  backticks entirely, reveal them on the cursor line, or keep them dimmed
  everywhere
- **Focus mode** (Ctrl+Shift+F) dims everything except the current paragraph
- **Typewriter mode** (Ctrl+Shift+T) keeps the cursor vertically centered
- **Templates**: Markdown files in `~/.local/share/scribe/templates` with
  `{{title}}`, `{{date}}`, `{{time}}`, `{{datetime}}` and `{{year}}` markers.
  Four examples are seeded on first launch
- **Split preview** (Ctrl+Shift+P) renders the full document in a side panel
  using the same `GtkTextTag` engine. Useful for tables and images
- **Ctrl+B/I/K** wraps the selection in `**`, `*` or backticks
- **List continuation**: pressing Enter starts the next item and re-numbers
  ordered lists. Leaving an item empty closes the list
- **Zoom** (Ctrl +/&minus;/0) with controls in the main menu
- **Go to line** from the status bar
- **Header bar** follows GNOME Text Editor: open button with recent files
  dropdown, new document button, centered title, main menu with zoom row on
  the right
- **Preferences** window with three pages: Appearance, Editor and Templates
- **Files**: open, save and save-as with `GtkFileDialog`, atomic writes
  (temp + rename), unsaved-changes warning, configurable autosave
- **Sidebar** (F9) with filterable recent files and a navigable document
  outline
- **Light and dark theme** follows the GtkSourceView style scheme
- **Integration**: `scribe file.md` and "Open with" from the file manager

## Known limits of live rendering

`GtkTextTag` changes how text looks but cannot replace or overlay it, so
some things are rendered as styled text rather than true widgets:

- Bullet markers stay as `-` or `*`, colored and with hanging indent, rather
  than `&bull;`
- Task lists show styled `[x]` and `[ ]` instead of checkboxes
- Rules (`---`) are dimmed and centered, not a drawn line
- Tables are rendered in monospace so columns align when pipes are lined up
  in the source; they are not a grid widget
- Images are not embedded

Drawing with a custom `snapshot()` or embedding widgets with text anchors
would alter the source buffer. That is out of scope for this phase.

## Screenshots

The interface is in Spanish. English localization is not done yet.

**Main window with live Markdown rendering**

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/hero-es-dark.png">
  <source media="(prefers-color-scheme: light)" srcset="assets/hero-es-light.png">
  <img alt="Main editor window with headings, bold, italic, code and a quote block" src="assets/hero-es-light.png" width="800">
</picture>

**Main menu with zoom controls**

<p align="center">
  <img alt="Menu showing open, save, zoom and view options" src="assets/screenshot-menu-es-light.png" width="800">
</p>

**Preferences window**

<p align="center">
  <img alt="Preferences window with Appearance, Editor and Templates pages" src="assets/screenshot-preferences-es-light.png" width="800">
</p>

**Focus mode (everything dimmed except the current paragraph)**

<p align="center">
  <img alt="Focus mode in the editor showing a dimmed document with the current paragraph highlighted" src="assets/screenshot-focus-es-light.png" width="800">
</p>

## What's missing

- Tabs / multiple documents (`AdwTabView`)
- Find and replace
- Export to HTML or PDF
- Spell checking
- English and other UI translations

## Build requirements

- Rust 1.80 or later
- GTK4 (&ge; 4.14), libadwaita (&ge; 1.5), GtkSourceView 5 and GLib
  development files

Arch / CachyOS:

```sh
sudo pacman -S rust gtk4 libadwaita gtksourceview5 glib2
```

Debian / Ubuntu:

```sh
sudo apt install libgtk-4-dev libadwaita-1-dev libgtksourceview-5-dev libglib2.0-dev
```

Fedora:

```sh
sudo dnf install gtk4-devel libadwaita-devel gtksourceview5-devel glib2-devel
```

## Build and run

```sh
cargo build --release
cargo run
```

GSettings schema needs to be installed for preferences to work. During
development:

```sh
glib-compile-schemas data/
GSETTINGS_SCHEMA_DIR=$PWD/data cargo run
```

The application starts with defaults if the schema is not found, logging a
warning to stderr.

## Flatpak

A Flatpak manifest is at [build-aux/flatpak](build-aux/flatpak). Work in
progress. The module builds with `cargo --offline`, so generate
`cargo-sources.json` first with
[flatpak-cargo-generator](https://github.com/flatpak/flatpak-builder-tools/tree/master/cargo)
and keep `Cargo.lock` in the repository:

```sh
cargo generate-lockfile
python3 flatpak-cargo-generator.py Cargo.lock -o build-aux/flatpak/cargo-sources.json
cd build-aux/flatpak
flatpak-builder --user --install build-dir app.scribe.Scribe.json --force-clean
```

## Development

```sh
git clone https://github.com/gnacho/scribe.git
cd scribe
cargo build
cargo test
cargo run
```

## License

AGPL-3.0. See [LICENSE](LICENSE).
