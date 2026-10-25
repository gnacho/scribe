# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Stack

delegated: static HTML/CSS/JS with no build step, following the landing-page skill conventions already used by NetPulse (web/) and EasyZFS (landing/). One Google Fonts link plus index.html, styles.css, i18n.js, app.js. i18n ES/EN with a language switcher; light/dark theme with a toggle. Deployed behind Nginx Proxy Manager at scribe.cloudless.club.

## Users

Primary (user-confirmed): desktop Linux users, GNOME-leaning, who resent Electron/WebKit Markdown editors and want a native, light app. ASSUMPTION (unconfirmed): a secondary audience of writers and note-takers arriving from Obsidian/Typora looking for a simpler single-surface editor.

## Product Purpose

Scribe is a native GNOME Markdown editor (Rust + GTK4 + libadwaita + GtkSourceView 5 + pulldown-cmark) that renders Markdown live on the editing buffer: the source text IS the preview. No preview pane, no WebKit, no Electron. Success for this landing: a visitor understands the mechanism within seconds, installs a release (deb/rpm/AppImage from GitHub), and keeps using the app daily.

## Positioning

The mechanism a neighbor cannot truthfully copy without rewriting the editor: Markdown rendering applied as GtkTextTag spans plus snapshot_layer ornaments on a plain GtkTextSourceView buffer. Nothing to sync, no HTML under the hood, no browser engine bundled. Claimed footprint (from README, to be re-measured before publishing numbers): binary around 8 MB, idles at a few dozen MB of RAM, versus hundreds of MB for WebKit-based editors.

## Operating Context

- Install channels: GitHub releases assets (deb, rpm, AppImage). Flathub submission pending human review (do not promise a date). Arch: AUR scribe-bin/scribe-git maintained by somepaulo.
- Use case: plain Markdown files for notes, docs, journals. Files stay local; no account, no cloud, no telemetry.
- License AGPL-3.0-or-later; repo github.com/gnacho/scribe; App ID io.github.gnacho.Scribe.
- The product is pre-1.0 (v0.3.20). Honest about it; do not claim maturity it does not have.

## Capabilities and Constraints

Verified in code (repo ~/Documentos/Repos/scribe-git, master):
- Live WYSIWYG rendering via GtkTextTag spans; ornaments drawn with snapshot_layer (rounded code blocks, drawn bullets, task checkboxes, quote bars, rules).
- Tables: grid mode or canvas mode with per-cell wrapping, bold header, alignments; row under cursor shows raw source.
- Export to PDF via GTK print pipeline: themed headings/code/quotes/tables, per-export settings (scheme, fonts, page size/orientation), careful pagination, \newpage support.
- Spell checking with suggestions and dictionary; locale-following.
- Focus mode, typewriter mode, split preview (same rendering engine), sidebar with recents and outline, find/replace with regex, zoom, tabs, recents menu.
- Local images painted in-block; remote images downloaded with size precheck and disk cache; YouTube cards (16:9 thumbnail card, opens browser).
- Configurable markup visibility (dim / hide-shrunk / reveal on cursor line/element).
- List continuation with renumbering; format-tables action; Ctrl+B/I/K wraps; autosave configurable; light/dark color scheme; UI translated EN/ES (gettext).
Constraints: Linux GTK only (no Windows/macOS/mobile), no sync/cloud, no plugin ecosystem, no math/Mermaid, pre-1.0. Templates feature was removed from the codebase: never mention it.

## Brand Commitments

- Name: Scribe. Icon and hero PNGs exist in assets/ (hero-en/es x dark/light).
- Voice: honest engineering, no hype, zero em/en dashes in all content, every claim verifiable against the repo, an honest "what it does not do" section is mandatory (landing-page skill).
- Both languages are first-class: ES/EN paridad total in landing copy.

## Evidence on Hand

- Demo video: ~/Documentos/Mi Nube/Proyectos/scribe/scribe-demo-v4.mp4 (31.6 s, h264 1280x840, 83 KB, 5 fps; shows live typing of a field journal with headings, bold, list, table, quote; EN UI, light theme). Also attached to GitHub release v0.3.16+.
- Hero PNGs in repo assets/ (en/es x light/dark). Screenshots (light, en/es): find, focus, preferences, split.
- Comparison data: must be contrasted against public sources of Obsidian/Typora/Apostrophe/Mark Text with date noted; nothing fabricated.
- No testimonials, no user counts, no benchmarks beyond self-measured ones. Do not invent any.

## Product Principles

1. Show the mechanism, do not describe it: live demo first (video in the hero).
2. Measured facts over adjectives: verify binary size and idle RAM before quoting numbers.
3. GNOME-native pride: HIG header bar, system theme, translations, shortcuts.
4. Honesty as the hook: the comparison matrix includes what Scribe does NOT do.
5. Two languages, one product: ES/EN parity at all times.

## Accessibility & Inclusion

Landing: WCAG AA contrast in both themes, prefers-reduced-motion respected, aria-labels on all non-text controls, single h1, descriptive alts. App: EN/ES UI; spellcheck follows locale.
