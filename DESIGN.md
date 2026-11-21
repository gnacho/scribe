# Design decisions

Scribe has a small number of deliberate decisions that come back as feature
requests under new issue numbers. This file states each one with its reason so
an issue can be answered with a link instead of a new paragraph.

It is not a roadmap. If a decision changes, the paragraph changes in the same
commit that changes the code, and the old paragraph does not stay around as
history: the changelog keeps that.

## The buffer is plain Markdown

The editor edits the Markdown source and the decoration happens on top, with
`GtkTextTag` spans and a `snapshot_layer`. There is no HTML, no web engine and
no JavaScript in the rendering path. The file on disk is always valid Markdown
that any other tool can read, and nothing has to be converted back from a rich
document model. Every feature that would need a real layout engine is out
almost by definition.

## Replaced marks are shrunk, not hidden

Marks that an ornament replaces (list bullets, checkboxes, heading rules,
fences) are scaled down to almost nothing and left transparent instead of
toggled invisible. GNOME/gtk#8346 makes GTK abort on some layouts when a tag
switches `invisible` on and off, so hiding is behind
`settings::gtk_hides_invisible_safely()`, which is false today. When GTK fixes
it, hiding can return on its own, without touching the model.

## GtkSourceView syntax highlighting is off on purpose

Scribe paints its own WYSIWYG tags. GtkSourceView's highlighter paints over
them with its own colours, so `set_highlight_syntax(false)` is applied when the
buffer is created, and the unused `markdown.lang` was deleted because nothing
referenced it. Syntax highlighting inside code fences has the same conflict,
which is why it is a design question and not a switch.

## Images live next to the document

A pasted image is written into `images/` next to the `.md` file and linked
relatively (`![](images/name.png)`). A document and its assets can be moved,
zipped or synced together and keep working, and the Markdown stays portable.
Absolute paths and URLs are accepted when the user wrote them.

## Image size and alignment live in the alt text

`![alt|480](path)` and `![alt|480x320|center](path)` are the convention for
size and alignment, the same shape Obsidian uses. It rewrites only the alt text
of the image, so the buffer keeps being ordinary Markdown and readers that do
not know the convention ignore it. There is no sidecar metadata and no custom
syntax.

## The PDF comes from GTK and cairo

Export runs a `GtkPrintOperation` over the same measured layout the print
preview uses, painted with cairo, with a mapped subset of CSS. No Chromium, no
WebKit and no TeX distribution is bundled. That is why the PDF is predictable
and small, and also why anything that needs a real typesetting engine is out of
scope.

## The table of contents is a printed page, not PDF bookmarks

With `export-toc` on, the PDF starts with a table of contents page that carries
real page numbers. The body is paginated first and the index pages are
prepended, so the body layout does not change. PDF bookmarks are a different
thing: they need post-processing the PDF, which the cairo print pipeline does
not expose, so they are not part of the index feature.

## Links open with Ctrl+click, and only http, https and mailto

A Markdown file can come from anywhere. Scribe refuses `file:`, `javascript:`
and every other scheme so a document cannot make the editor launch something
local, and a plain click stays a click in the text. Relative paths are not
opened from the editor.

## A leading BOM is preserved

Some editors write a UTF-8 BOM. Scribe keeps it byte for byte, never adds one,
and skips it when parsing so the first block of the file renders like any
other. Inside the editor it is an ordinary character in the buffer.

## Non-goals

Accounts, telemetry, cloud sync, a plugin marketplace and an AI assistant
inside the app. Scribe is a local editor: the file is the product, nothing
leaves the machine, and there is no service to depend on.

## When a request comes back

If a feature was decided against, this file should answer why. Link to the
paragraph instead of writing the argument again, and if the decision really
changes, change it here in the same commit as the code.
