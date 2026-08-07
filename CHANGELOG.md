# Changelog

All notable changes to Scribe will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Inline WYSIWYG rendering**: the editor buffer is now decorated with `GtkTextTag`
  spans computed from the Markdown source. Headings render at their actual scale,
  bold/italic/strikethrough/code appear styled inline, and the Markdown syntax
  markers (`**`, `##`, backticks, link URLs) are hidden everywhere except on the
  cursor line — which is what makes editing feel WYSIWYG.
- **New `src/markdown_render.rs`** module: pure-logic span calculator backed by
  pulldown-cmark. No GTK dependency — returns byte ranges and tag names so it can
  be tested without a display. Ships with 13 unit tests (headings, emphasis, code,
  links, images, blockquotes, lists, nested lists, code blocks, rules, task lists,
  Unicode boundaries, empty input).
- **Status bar** at the bottom of the window, GNOME Text Editor style: word count,
  character count, and cursor position (`Ln N, Col M`), updated on every change
  and cursor move.
- **Format shortcuts**: `Ctrl+B` for bold, `Ctrl+I` for italic, `Ctrl+K` for inline
  code. Each wraps the current selection (or inserts markers at the cursor).
- **Recents persistence**: recently opened files are stored in GSettings
  (`recent-files` key as a string array), deduplicated, capped at 20 entries,
  and filterable from the sidebar search entry.
- **Rich preferences window** with font size spin button, line spacing adjustment,
  and autosave toggle — all persisted to GSettings with setter methods.
- **`Outcome` / `OpenOutcome` enums** in `FileManager`: file open and save
  operations now distinguish success, user cancellation, and I/O errors, with
  toasts shown for failures.

### Changed

- **`editor.rs` rewritten** (75 → 435 lines): GtkSourceView syntax highlighting
  is disabled; all decoration is applied manually via `GtkTextTagTable`. The
  editor uses proportional Cantarell (not monospace), with monospace reserved
  for code spans and blocks. A centered 720 px column with dynamic margins
  gives a Typora-like writing experience. Rendering is throttled with a 45 ms
  debounce. Light/dark themes toggle automatically via `libadwaita::StyleManager`
  and update both the GtkSourceView style scheme and the custom text tags.
- **`preview.rs` rewritten** (68 → 392 lines): the split preview panel now
  renders Markdown into a `GtkTextView` with `GtkTextTag` spans instead of
  pushing HTML into a `GtkLabel` (which was broken: Pango markup rejected
  `<style>` tags). Covers headings, bold, italic, strikethrough, inline code,
  code blocks with language labels, blockquotes, unordered and ordered lists
  with bullet/number prefixes, task lists with `☑`/`☐` glyphs, links,
  images (shown as `[image: alt]`), tables (tab-separated), horizontal rules,
  footnotes, and math (rendered as code). Adapts colours for light and dark
  themes.
- **`window.rs` rewritten** (450 → 970 lines): extraction of the shortcuts
  overlay into a `SHORTCUTS_UI` XML constant, shared helpers (`update_title`,
  `update_status`, `toast`, `refresh_recents`, `load_file`),
  `Rc<dyn Fn()>`-based closures for title and status updates, format actions
  wired to `editor.wrap_selection`, proper close-request handling with unsaved-
  changes dialog, and `open_path` for command-line file arguments.
- **`file_manager.rs` reworked**: `open` and `save` now take typed outcome
  callbacks (`OpenOutcome` and `Outcome`) instead of raw `Option` tuples.
- **`settings.rs` expanded**: added `set_font_size`, `set_line_spacing`,
  `set_autosave`, `recent_files` (returns `Vec<String>` from GSettings strv),
  and `push_recent_file` (dedup + truncate to 20).
- **GSettings schema defaults** changed: sidebar hidden by default, font size
  16 px (was 15).
- **README rewritten** to describe the inline WYSIWYG approach, known
  limitations of `GtkTextTag`-based rendering, current features, and what
  remains to be done.

### Removed

- **WebKitGTK / Milkdown stack**: the v1.3 rewrite already dropped the JS
  bridge; this version completes the transition by removing all HTML-based
  preview code (`GtkLabel::set_markup`) and the CDN dependency.
- **Monospace editor body**: the editor now uses proportional Cantarell.
  Monospace is applied only to code spans and blocks via text tags.
- **Line numbers**: disabled in the editor (`show_line_numbers: false`).
  Cursor position is available in the status bar instead.
- **GtkSourceView syntax highlighting**: disabled to avoid painting over the
  custom WYSIWYG tags.

### Fixed

- Preview panel was blank because `GtkLabel::set_markup` rejected the `<style>`
  tag in the HTML output. Replaced with `GtkTextView` + `GtkTextTag`.
- File open/save errors are now surfaced as toasts instead of failing silently.

## [Pre-alpha] — 2026-08-03

Initial public commit (`1932282`). Proof-of-concept editor with GtkSourceView 5,
sidebar, file open/save dialogs, GSettings persistence, and basic Markdown
preview via `pulldown-cmark` → HTML → `GtkLabel::set_markup`.
