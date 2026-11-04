# Changelog

All notable changes to Scribe will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.35] - 2026-10-28

Thank you [@HelloKolla2004](https://github.com/HelloKolla2004) for the Fedora
report. The crash notification appeared right after a normal close, which is
the kind of clue that only comes from watching the app where it fails.

### Fixed

- **Abort when closing the window** (#166): shutting down with the window
  close button, or with Alt+F4, ended in an abort and, on systems that notify
  crashes, in a crash dialog right after a clean close. The window cleanup ran
  while the internal window registry was still borrowed, so the internal panic
  could not unwind. The previous release fixed the same abort for Ctrl+Q, this
  one covers the window itself, on any desktop.

## [0.3.30] - 2026-10-27

Thank you [@HelloKolla2004](https://github.com/HelloKolla2004) for the PDF
tables report and [@kem-a](https://github.com/kem-a) for two sharp reports:
the quit crash came with a stack trace that pointed straight at the culprit,
and the missing icons came with a screenshot that said the rest.

### Added

- **Named page templates for PDF export** (#153): the export controls were a
  flat list and nobody could save a layout, so anyone printing often re-tuned
  the same values every time. Five named layouts now group the whole page:
  Custom, Article, Manuscript, Letter and Compact. Each one decides margins,
  leading, heading scale, running header and footer, and the title block built
  from the document name. The choice persists across restarts, and Custom
  reproduces the output of previous versions.
- **Print preview** (#153): `Print Preview…` renders the real pages through the
  export pipeline, so what you see is the PDF and not an approximation.
- **Your own stylesheet** (#156): a CSS file can be chosen in
  `Preferences > Export` and is applied on top of the theme, so it wins over it.
  The parser is a mapped subset, not a CSS engine: selectors for the page, body,
  headings, paragraphs, code, quotes, tables and links are translated into
  values the cairo layout already understands, anything outside the table is
  ignored with one warning, and removing the file restores the previous output
  byte for byte.
- **Four bundled themes** (#163): Sepia, High contrast, Technical and Academic,
  written with our own values and compiled into the binary, so a Flatpak or
  AppImage install gets them without installing anything. A theme changes type,
  sizes, leading and colour, and combines with any template. The stylesheet
  always applies last.
- **Typewriter mode in the toolbar** (#151, #152): the split view toggle is
  replaced by a typewriter mode toggle that shares its state with the menu item
  and the preferences switch. Split view stays available on Ctrl+Shift+P, and a
  symbolic typewriter icon ships with the app.

### Fixed

- **Tables export to PDF as real columns** (#138, reported by
  @HelloKolla2004): the PDF painted tables as a monospaced box with the raw
  source lines. The exporter now parses the table: column widths from their
  natural content, `:--`/`:-:`/`--:` alignments, escaped pipes, inline
  bold/code in cells, a bold header band with a strong rule, light grid
  lines, and cells that wrap inside their own column (#139).
- **Quitting no longer aborts the process** (#141, reported by @kem-a): at
  exit, the window registry thread-local was consulted from its own
  destructor, the panic could not unwind through C and every clean quit
  ended in SIGABRT. The destroy handler now tolerates a registry that is
  already going away and main releases the windows before returning, so
  both destruction orders are covered (#142).
- **Missing action icons on some systems** (#140, reported by @kem-a): the
  preferences pages and two header buttons relied on theme icons absent
  from recent Fedora images, and an AppImage or Flatpak installs nothing
  into any icon theme. The action icons now ship with the app as resource
  fallbacks; the system theme still wins when it has them (#143).

### Changed

- **The export preferences are split by job** (#163): the PDF group keeps the
  layout controls and a new Typography and colour group holds the PDF theme and
  the stylesheet. The theme row is called PDF theme so it is not confused with
  the text theme of the editor, and loading a stylesheet has its own row: it
  used to live inside the theme combo and got stuck when the file chooser was
  cancelled.

### Docs

- **The project has a website**: every feature is shown in action at
  [scribe.cloudless.club](https://scribe.cloudless.club). The READMEs invite
  the reader to it, and the AppStream homepage and the About dialog now use
  it instead of the GitHub repository (#144).
- **Flathub comes first in the install instructions** (#158, #159): both
  READMEs now lead with `flatpak install flathub io.github.gnacho.Scribe` and
  keep the distribution packages below.
- **One Flatpak manifest instead of two** (#160, #161): the stale copy under
  `packaging/flatpak` is gone. The published manifest lives in the Flathub
  repository, and the local one under `build-aux` is only for building against
  the GNOME runtime.
- **The landing page offers Flathub first** (#162): the install section opens
  with the Flathub tab, the hero links to it, and the structured data declares
  it as the install URL.

## [0.3.20] - 2026-10-24

The first release carrying external code. Thank you
[@somepaulo](https://github.com/somepaulo): the redesigned icon, the symbolic
variant and the catch of the `index.theme` regression are all yours - and
[@mihierkansal](https://github.com/mihierkansal) reported the install bug that
v0.3.18 shipped.

### Fixed

- **The installer no longer writes `index.theme` into hicolor** (#128,
  reported by @mihierkansal): since v0.3.18 `make install` dropped its own
  theme index into the user's hicolor directory, which redefined the whole
  theme for that user and left other apps without icons. The file is gone from
  the repo and the install target; the copy those two releases left behind is
  removed when it matches ours (an unrelated one is left alone), the cache is
  refreshed only when a theme index exists at the destination and a stale user
  cache is removed otherwise (#130).
- **Opening a document no longer adds a second tab next to the untouched
  "Untitled" draft** (#135): a desktop launch leaves stdin at /dev/null, the
  startup treated it as piped text and the empty draft was born marked as
  modified, so the pristine-tab reuse never fired. Empty stdin now opens a
  clean, unmodified window (#136).
- **The recents empty state is no longer a dead button** (#131): the dropdown
  swaps to a centered inert label and the sidebar shows the same centered
  placeholder instead of an insensitive action row that looked clickable
  (#134).
- **Saving no longer pops a hardcoded Spanish toast** (#132): "Guardado"
  showed up regardless of the interface language; the toast is gone, the tab
  state already tells the story (#134).

### Added

- **Redesigned application icon to Adwaita standards plus a symbolic icon**
  (@somepaulo, PR #130): size, position and corner radii fixed, unnecessary
  layers removed, and the missing symbolic variant for GNOME Shell added.
- **Tab close buttons follow the window button layout** (#133): with the
  controls on the left the closes move left, exactly like GNOME TextEditor
  does it - watch `gtk-decoration-layout` and invert the tab bar (#134).
- **Markdown syntax conceals per element, Obsidian-style** (#117, requested by
  @JustSimpleUser52): the markers around the element the cursor is in - or
  touching either edge of - reveal; everything else stays concealed, even on
  the same line. Heading, list and quote markers keep the line-wide reveal.
  Every inline marker now carries its element's range, and the three visibility
  modes share the same rule. Thanks to @JustSimpleUser52 for the precise
  examples (`**bo|ld**`, `|**bold**`, `**bold**|`) - they became the spec
  (#127).

## [0.3.19] - 2026-10-22

The first release shaped by external bug reports. Thank you,
[@JustSimpleUser52](https://github.com/JustSimpleUser52): you are the first
user this project ever had, three of your reports landed in this changelog, and
the editor is better for every one of them.

### Fixed

- **Tables with long cells no longer break** (#124, reported by
  @JustSimpleUser52 as "Markdown tables are borked" - and they were): a row
  longer than its column rendered wider than the table and, once it passed the
  visible width, wrapped with its continuation landing outside the table. Any
  table with an overflowing row now renders as the painted canvas, which wraps
  cell text inside its own column (#125).
- **A heading no longer jumps in size when the document opens** (#115, reported
  by @JustSimpleUser52, with a video): the decoration was deferred past the
  first layout, so headings painted at body size and grew one cycle later.
  Loading a document is now decorated before the first paint (#120).
- **The rule under H1 and H2 headings is gone** (#115, #117 - "It doesn't look
  good"; fair, it is gone) (#120).
- **Heading, list and blockquote markers reveal on the cursor line** in "Hide"
  mode (#117, reported by @JustSimpleUser52): the `#`, `- ` and `> ` now show on
  the line being edited, so those markers can be edited at all in that mode. The
  task checkbox also aligns with its text baseline now (#122).
- **Remote image downloads no longer register GIO async callbacks** (#99): the
  download runs on a dedicated thread and only the decoded texture crosses
  back, so no GLib closure is ever consumed from the wrong thread. The ignored
  UI tests now serialize around the shared `MainContext` and run
  deterministically in parallel (#123).

### Added

- **Pointer cursor over task checkboxes** (#117, requested by
  @JustSimpleUser52): hovering a `- [ ]` box turns the cursor into a hand, so
  the box is clearly clickable (#121).
- **Larger task checkboxes** (#117, same report): from 14 to 20 px, with the
  border and check stroke scaled up and the click hit-test following (#121).

### Internal

- **CI: the package-attach job points `gh` at the repository** (#119): without
  a checkout, `gh release upload` failed with "not a git repository" and the
  v0.3.18 packages had to be attached by hand.

## [0.3.18] - 2026-10-22

### Fixed

- **The About window shows the Scribe icon again** (#114): the app icon and the
  symbolic ones are embedded as a GResource and registered on the display icon
  theme, so `AdwAboutWindow` no longer depends on the installed theme. Builds
  without an icon theme, or with a stale `icon-theme.cache`, already get the
  right icon from `d7a81a5`; this release is the first one that ships it.
- **`make install` no longer depends on a cached icon directory** (#114): it
  installs a `hicolor/index.theme` and refreshes the icon cache, so the desktop
  entry resolves even on machines where `gtk-update-icon-cache` had refused to
  rebuild a stale cache ("No theme index file").

### Changed

- **README limitations match the current feature set** (#97): both README files
  no longer claim that replacing is not implemented. Ctrl+H reveals the replace
  row, with case sensitivity and regular expressions as options.

## [0.3.17] - 2026-10-21

### Added

- **AppImage** (#95): built in CI on Ubuntu 24.04 and attached to every
  release, alongside the .deb and the .rpm.
- **Find & replace shortcuts** (#98): Ctrl+H toggles the replace row and
  Ctrl+G / Ctrl+Shift+G jump between matches; the toggle gets the standard
  replace icon.

### Changed

- **App ID renamed to `io.github.gnacho.Scribe`** (#103): desktop file,
  metainfo, gschema, icons, Flatpak manifests and packaging scripts all move
  to the GitHub-based ID so the Flathub submission can be verified. The
  Flatpak manifests are now YAML and build fully offline from a generated
  `cargo-sources.json`.

### Fixed

- **Closing an empty untitled draft no longer prompts** (#101): a buffer with
  no file and no content closes directly; typed-then-cleared text still asks,
  and modified saved files still ask.

## [0.3.16] - 2026-10-19

### Added

- **Spell-check context menu** (#62): right-click a misspelt word for up to
  six dictionary suggestions, ignore it for the session or add it to the
  persistent personal dictionary.
- **Clickable task checkboxes** (#67): click the box of a `- [ ]` task to
  toggle it in the source.
- **Semantic themes** (#25): native theme files paint the buffer background,
  text and ornaments; five built-ins (Light, Dark, Sepia, Solarized Dark,
  Gruvbox Dark) selectable in Preferences > Appearance, applied live, with
  user themes supported in `$XDG_DATA_HOME/scribe/themes/*.ini`.
- **Syntax marks only on the cursor line** (#70): `**`, `#`, `-` markers show
  on the line being edited and collapse everywhere else.

### Fixed

- Painted canvas tables: wrapped cell text keeps 2-3 px of air before the
  vertical lines (#66) and the collapsed source rows no longer leave faint
  dashes over the first column.
- The spell-check underline no longer recolors the text.
- Substitute ornaments (bullet, checkbox, heading rule) stay off the cursor
  line so what you edit matches what is in the buffer (#41).

## [0.3.14] - 2026-10-19

### Added

- **Spell checking** (#51): words the system dictionary does not validate are
  underlined as you type; it can be turned off in Preferences > Writing. The
  dictionary follows the system locale (enchant).
- **Clickable task checkboxes** (#67): click the box of a `- [ ]` task to
  toggle it in the source.
- **Adaptive column width** (#63): on wide windows the side margins stop
  growing without limit and the text column takes advantage of the space.
- **Syntax marks only on the cursor line** (#70): `**`, `#`, `-` markers show
  on the line being edited and collapse everywhere else.

### Fixed

- Painted canvas tables: collapsed source rows no longer leave faint dashes
  over the first column (#54), and wrapped cell text keeps 2-3 px of air
  before the vertical lines (#66).
- Spell-check marks underline without recoloring the text (#65).
- Opening a document no longer shows the width reflow flash (#72).

## [0.3.12] - 2026-10-18

### Added

- **Split view from the header** (#35): a header icon shows or hides the preview, painted to the right of the editor with the same `GtkTextTag` engine.
- **Colour schemes in the menu** (#36): follow the system, light and dark, as a stateful action kept in sync with the Appearance page in Preferences. The menu stops offering what the header already has (new window, open, reload and close tab).
- **Find in the document** (#37): a search bar inside the editor pane with a match counter, previous/next buttons and Ctrl+F, Enter and Shift+Enter.
- **Remote images** (#38): images with a URL are downloaded and cached under `~/.cache/scribe/images` (the size is checked first, with a 20 MB limit), so later paints happen without going to the network. A failed download leaves the placeholder and is not retried on every repaint.
- **YouTube cards** (#39): a video alone on its line (Markdown link, bare URL or `<iframe>`) is drawn as a 16:9 card with its thumbnail and a play badge, without touching the buffer. There is no embedded player: clicking the card opens the browser.
- **Ctrl+click on a link** (#40): opens the destination with the default application (`http`, `https` and `mailto` only) and, with Ctrl held over a link, the tooltip says where it goes. A normal click still belongs to the text: it places the cursor and selects.
- **PDF export** (Ctrl+P) through the GTK print pipeline, with its own settings (colour scheme, fonts, size and orientation) and careful pagination, and **automatic reload** when the file changes on disk (#31).
- **Pasting and dropping images**: they are saved next to the document and linked with a relative path. **Draft recovery** after an unexpected close (#31).
- **Command line**: `scribe file.md:12:4` opens at the given line and column, and accepts piped content (#31).
- **New icon** (#44): a clipboard holding a note (heading, highlighted line and a list) with a fountain pen writing in the corner, drawn in Impress and exported to SVG; it reads from 16 px up.

### Changed

- **User installation** (`make install`) that ships the binary, the recompiled GSettings schema, the .desktop file and the icons; **MSRV moved to Rust 1.85** (the dependency tree requires edition2024) and **performance budgets** in release mode for opening and decorating (#31).
- **Main menu trimmed** (#46): out go the "Style" section (the colour scheme lives in Preferences), the "Split view" entry (it is in the header and on Ctrl+Shift+P), the zoom row with the font size in px, and "Align tables" and "Close fences". The actions and their shortcuts stay: Ctrl+Shift+P, Ctrl +/-/0, Ctrl+Alt+T and Ctrl+Alt+F.
- **Privacy and documentation**: the privacy notice now tells the truth about the network (the only requests are the ones the document asks for, cached on disk) and the README (English and Spanish) covers the new features with their real limits and up-to-date screenshots.

### Removed

- **Status bar** (#45): the footer with the word counter, the "Ln, Col" button with its go-to-line popover, the "Markdown" button with the properties and the fence warning are gone. "Close fences" stays on Ctrl+Alt+F and "Align tables" on Ctrl+Alt+T.

### Fixed

- **Abort on startup under Wayland** (#32, #34): column margins were applied inside GTK's validation cycle and tripped the `g_assert` in `gtk_text_view_validate_onscreen` on every start under a compositor (GTK 4.22.5 and 4.24.1, not reproducible under Xvfb). They are applied from an idle now.
- **Tables** (#30, #34): columns are measured from the delimiter row instead of from the spaces each row happens to have, with a continuous vertical line per border, a header rule and respect for `:---`, `:---:` and `---:`. The row the cursor is in inside a table is shown as source so it can be edited. A cell wider than its column still overflows: Ctrl+Alt+T ("Format tables") aligns it.
- **Tab test** (#33, #34): the draft title was compared against the untranslated msgid.

## [0.3.10] - 2026-09-19

Thank you [@sewnie](https://github.com/sewnie) for asking for an English
interface (#12).

### Added

- **Translated interface (gettext)**: the interface strings move to English as
  the source language, with a full Spanish catalog (`po/es.po`), so each user
  sees Scribe in the language of their system. New translations live in `po/`
  (see `po/update-pot.sh`). Built-in templates are seeded in the current locale
  on first launch and the keyboard shortcuts are translated.
- **Unterminated code fences are detected and closed** (#11): when a fenced
  block is not closed, the status bar warns ("Unclosed fence (line N)") and an
  action (menu, Ctrl+Alt+F or clicking the warning) closes it with a best-effort
  heuristic (before the next heading or rule, the next opening fence, or the end
  of the document). Undo (Ctrl+Z) restores the original text.

## [0.3.8] - 2026-08-22

### Fixed

- **The table box and the header rule hug the content**: the background box was
  painted at the full width of the text column (a pale rectangle was left on the
  right, the "ghost column") and the rule under the header crossed from margin
  to margin. They now measure the real content of the rows (shrunk pipes, cell
  text); tables with more than 200 rows are measured only in the visible window.
  Cells with an escaped pipe (`\|`) show just `|`; the backslash shrinks like an
  escape mark.

## [0.3.6] - 2026-08-22

### Added

- **Real multi-document tabs** (`AdwTabView` + `AdwTabBar`, gnome-text-editor
  style). Each document has its own editor, undo, saved state and tab, with a
  close button, drag reordering and overflow. `Ctrl+N` opens a tab. `Ctrl+O`,
  recents or `scribe file.md` open tabs in the active window. `Ctrl+W` closes.
  `Ctrl+Tab` / `Ctrl+Shift+Tab` / `Ctrl+PageDown` / `Ctrl+PageUp` navigate.
  Opening an already open file focuses its tab (dedupe by canonical path).
  Closing a tab with changes shows a Save/Discard/Cancel dialog. Closing the
  window with several modified documents shows sequential dialogs. A "•" dot
  marks the modified tab, with an attention badge in the background, and
  autosave is per document. The last tab never closes (a blank one stays).

### Fixed

- **Continuous background box in long blocks**: the box was stretched off screen
  to hide the corners, and with a very negative origin GSK painted nothing (the
  box vanished halfway through a 120-line code block). The rectangle is now
  limited to the visible area and rounded corners are drawn only at the real
  edges of the block; the blockquote bar uses the same rule.

## [0.3.4] - 2026-08-22

### Fixed

- **Table content is monospaced again** so the visual grid lines up. The grid
  (bold header, painted separators, rule) relies on the font padding that
  `format_tables` produces; with a proportional font the columns did not align.
  Inline styling (bold, code, colour) still applies: those are attributes
  independent of the family. If a table does not line up, Ctrl+Alt+T ("Format
  tables") aligns it.
- **Headless render test**: `tests/render_shot.rs` renders the editor to PNG
  under xvfb (`SCRIBE_SHOTS_DIR` or a temp dir) to check the rich view visually
  without opening windows. The original output path pointed at the agent sandbox
  (`/mnt/agents/work/shots`) and failed on real machines; it is configurable
  through an environment variable now.

## [0.3.2] - 2026-08-22

### Fixed

- **The "byte index off the end of the line" crash (SIGABRT) is eliminated by
  construction**: GTK aborts (GNOME/gtk#8346, `gtktextbtree.c:4012`) when the
  buffer holds `invisible=true` text and a pixel-iter conversion asks for
  geometry with a stale layout. The editor no longer generates invisible text on
  any path: marks are dimmed or shrunk (`syn_shrink`). When GTK ships the fix,
  the `gtk_hides_invisible_safely()` gate will re-enable real hiding without
  touching anything else. The `gtk_invisible_canary` canary detects whether the
  system GTK is still vulnerable.
- **Data loss when aligning tables** (`format_tables`): it absorbed foreign lines
  containing `\|` and dropped cells from rows wider than the header. The body is
  now delimited by pulldown-cmark and the columns cover the widest row. CRLF
  line endings are preserved.
- **Memory leaks**: `Rc` cycles in StyleManager, buffer signals and window
  actions kept every window and editor alive. All moved to
  `downgrade()`/`upgrade()`.
- **Silently swallowed errors**: autosave and preferences now warn when a write
  fails (one toast per streak).
- **Saving without an extension** now appends `.md`.
- **Defensive indexing** of the byte-char map (range access).
- **Flatpak manifest** (issue #5): GNOME 50 runtime, `cargo-sources.json` in the
  official flatpak-cargo-generator format, committed `Cargo.lock`, compliant
  AppStream 1.0 metainfo, hardened CI (MSRV 1.83, `--locked`, appstreamcli).

### Added - GitHub-style rich editing view

- **Code blocks with a background box** and monospaced content.
- **Tables as a visual table**: box, bold header, delimiter row reduced to a
  rule and painted column separators.
- **Headings**: hierarchy by scale/weight plus an underline rule on H1/H2; the
  hashes shrink in WYSIWYG mode.
- **Blockquotes with a painted side bar** and **lists** with bullets and
  checkboxes in the gutter.
- **Local images in a block**: if `![alt](path)` is alone on its line and the
  file exists (relative to the document), it is painted scaled (max 144 px,
  20 MB limit, cache of 64). Remote or missing, a discreet placeholder. The
  document is never polluted: no widgets or characters are ever inserted.
- **Preferences that mean something**: "Dim" is the raw view dimmed with ambient
  ornaments; "Hide"/"On focus" is WYSIWYG with shrunk marks (labelled "shrinks
  the marks, GTK does not allow hiding them yet").

### Changed

- Markup replaced by an ornament now **shrinks** (scale 0.05 + alpha 0) instead
  of hiding, so it stays in GTK's layout and the abort path is unreachable by
  construction.
- The cursor only re-applies the focus-mode dimming (it used to re-analyse
  everything).
- A single copy of the buffer for the counters, in the debounced timeout.
- README rewritten: EN main + `README.es.md`, with the rich view features.

## [0.3.1] - 2026-08-08

### Fixed

- **A code fence followed by a tab did not close the block**, so the rest of the
  document started to render as code. The CommonMark spec says the closing fence
  "may be followed only by spaces or tabs, which are ignored", but pulldown-cmark
  does not honour it: it reproduces with `` ```\ncode\n```\t\n `` both in 0.12 and
  in 0.13, so it is neither ours nor fixed upstream. As a workaround,
  `normalize_fences()` replaces the tabs after a fence with spaces before
  parsing. It only touches fence lines, and since a space and a tab are one byte
  each, all offsets remain valid against the original text: tabs inside the
  block are not touched.
- Three new tests, including the one proving normalisation preserves positions
  (33 in total).

## [0.3.0] - 2026-08-08

### Added - tables

- **Tables are rendered as tables**: the delimiter row from the source is hidden
  and the header separator line is drawn in its place, the block gets its own
  rounded box, different from code blocks, and the pipes are dimmed, acting as
  column separators without competing with the content.
- **Align tables** (Ctrl+Alt+T, and in the main menu): reformats every table in
  the document so its columns line up in the source. Since the block is
  monospaced, aligning the source is what makes the table look aligned on screen.
  It respects the alignment `:` marks, escaped pipes (`\|`) and sample tables
  inside code blocks, which are left alone. It is a single user action: one
  Ctrl+Z undoes it.
- Seven new tests for the formatter and the table ornaments (29 in total).

### Changed

- The `tabledelim` tag is gone: the delimiter row is no longer dimmed, it is
  hidden.

## [0.2.1] - 2026-08-08

### Fixed

- **Crash while editing** (`gtk_text_iter_set_visible_line_index`, "byte index
  off the end of the line"). Three chained causes, all fixed:
  - `decorate()` was called synchronously from `notify::cursor-position`, which
    is a buffer signal. Applying invisibility tags while GTK is processing an
    edit throws its layout off. All decoration now goes through
    `schedule_decoration()` and is deferred to the main loop, so the buffer is
    never touched from one of its own signals.
  - When drawing, a code block spanning far beyond the screen requested
    geometry for very distant lines, forcing GTK to validate thousands of lines
    in the middle of `snapshot`. Geometry is now limited to the visible strip and
    the box is stretched off screen so the rounded corners do not appear cut in
    the middle of the block.
  - Ornaments are keyed by line number, so if the buffer changed between
    computing and drawing they pointed at the wrong places. `set_ornaments` now
    stores the line count they were computed for and drawing skips the frame when
    it no longer matches.
- **The version in `Cargo.toml` was still 1.0.0** since the pre-alpha, so the
  About window lied. It now comes from `CARGO_PKG_VERSION` and matches the
  CHANGELOG. While at it: licence as `AGPL-3.0-or-later`, `rust-version`
  declared and `serde`/`serde_json` dropped, nobody used them.
- **Block tags extending too far**: the same problem of decorating inside a
  signal could leave the `codeblock` tag covering text past the closing fence,
  so the whole document started to look like code after editing.

## [0.2.0] - 2026-08-07

### Added - drawn ornaments

- **`src/markdown_view.rs`**: `MarkdownView`, a `GtkSourceView` subclass
  implementing `snapshot_layer` to paint what a `GtkTextTag` cannot express. It
  draws bullets (disc, ring and square by level), task checkboxes with their
  check mark, horizontal rules, the vertical bar of blockquotes and the rounded
  box of code blocks. It works in buffer coordinates and only computes geometry
  for what is on screen.
- **`Ornament` and `Analysis`** in `markdown_render`: the analyser now returns
  spans *and* ornaments keyed by line number.
- **`SpanKind`**: distinguishes permanent style, Markdown mark and a mark
  replaced by an ornament. Replaced marks do not reveal on hover, since doing so
  would shift the text on every line change.
- **Footnotes**: the reference keeps only the superscript number and the
  definition is dimmed.
- Tests: 16 -> 22.

### Changed

- Blockquotes and code blocks no longer use `paragraph-background`: their box and
  bar are drawn, which allows rounded corners and real margins.
- Unordered lists lose the hanging indent: the bullet goes in the gutter and
  every line of the item starts aligned.
- In "always dim" mode the ornaments are disabled, so information is not
  duplicated with the visible marks.

### Fixed

- **Nested list indentation**: pulldown-cmark starts the item range at the
  marker, not at the start of the line. Since the tag did not cover the
  paragraph start, GTK applied the parent level margin and the bullet lined up
  with the parent list. The span now reaches the start of the line and the
  literal indent is hidden, so the margin alone provides the offset.
- **Stray space after checkboxes**: the `TaskListMarker` range covers `[x]` but
  not the following space, which was left as a phantom indent.

### Added - HIG interface, templates and preferences

- **Complete preferences** (`src/preferences.rs`) with `AdwPreferencesWindow` and
  three pages: Appearance (colour scheme, font family, size, line spacing,
  column width), Editor (markup visibility, continue lists, focus mode,
  typewriter, tab width, autosave and interval) and Templates.
- **Templates** (`src/templates.rs`): `.md` files in
  `$XDG_DATA_HOME/scribe/templates`, seeded with four examples the first time.
  They support `{{title}}`, `{{date}}`, `{{time}}`, `{{datetime}}` and
  `{{year}}`.
- **Configurable markup visibility**: hide always, reveal on the cursor line or
  dim always.
- **Focus mode** (Ctrl+Shift+F) and **typewriter** (Ctrl+Shift+T).
- **List continuation** on Enter, with renumbering of ordered lists and automatic
  closing when an item is left empty.
- **Zoom** (Ctrl +/-/0) with a control row in the main menu.
- **Go to line** from the status bar.
- **Extended markup**: setext headings, autolinks `<url>`, footnotes, dimmed HTML
  blocks and monospaced tables so the columns line up. Tests: 13 -> 16.
- **Shortcuts window** moved out to `src/shortcuts.ui`.

### Changed

- **Header reorganised** like GNOME Text Editor: `AdwSplitButton` "Open" with a
  recents dropdown and a new document button on the left, title in the centre,
  main menu on the right. The status bar carries the word counter on the left
  and the position and properties buttons on the right.
- **Stateful toggles** (`SimpleAction::new_stateful`) for the sidebar, split
  view, focus mode and typewriter: the menu shows the check mark.
- **GSettings schema** rewritten with enumerations and ranges.
- **`settings.rs`** rewritten with its own types (`MarkupVisibility`,
  `FontFamily`) instead of loose strings.

### Fixed

- Restored `app.quit` and `app.new-window`, lost when `main.rs` was redone:
  Ctrl+Q did nothing and two menu entries came out greyed.
- Restored `ApplicationFlags::HANDLES_OPEN`, without which `scribe file.md` and
  "Open with" from the file manager were ignored.
- `GtkTextTag:letter-spacing` does not accept negative values: using them
  aborted the application while building the heading tags.

### Added - inline WYSIWYG rendering

- **Inline WYSIWYG rendering**: the editor buffer is now decorated with `GtkTextTag`
  spans computed from the Markdown source. Headings render at their actual scale,
  bold/italic/strikethrough/code appear styled inline, and the Markdown syntax
  markers (`**`, `##`, backticks, link URLs) are hidden everywhere except on the
  cursor line, which is what makes editing feel WYSIWYG.
- **New `src/markdown_render.rs`** module: pure-logic span calculator backed by
  pulldown-cmark. No GTK dependency: returns byte ranges and tag names so it can
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
  and autosave toggle, all persisted to GSettings with setter methods.
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

## [Pre-alpha] - 2026-08-03

Initial public commit (`1932282`). Proof-of-concept editor with GtkSourceView 5,
sidebar, file open/save dialogs, GSettings persistence, and basic Markdown
preview via `pulldown-cmark` → HTML → `GtkLabel::set_markup`.
