# Scribe landing: design system

Source of truth for the marketing site at https://scribe.cloudless.club.
Generated from the shipped code (`landing/styles.css`, `landing/index.html`,
`landing/app.js`) on 25 October 2026. When this document and the code disagree,
the code wins and this document gets fixed.

## World

Cinema white, product as protagonist. The page is a product page in the Apple
sense: generous white space, type large enough to be the layout, one blue action
colour, and the real app on screen doing the talking. No decorative gradients on
text, no cards pretending to be features, no stock photography.

Two atmospheres, one design: light is paper white, dark is near black. Both use
the same structure, spacing and type scale. Nothing is designed "for dark" as a
second thought.

## Tokens

Defined in `:root` and overridden in `html[data-theme="dark"]`. Colours are
literal hex, not RGB channel triples, because the palette is small and fixed.

| Token | Light | Dark | Use |
|---|---|---|---|
| `--bg` | `#ffffff` | `#0a0a0c` | Page background |
| `--text` | `#1d1d1f` | `#f5f5f7` | Body and headings |
| `--muted` | `#6e6e73` | `#86868b` | Secondary text (5.4:1 light, 6.9:1 dark) |
| `--accent` | `#0071e3` | `#2997ff` | Primary action, selection, focus ring |
| `--link` | `#0066cc` | `#2997ff` | Inline text links (5.6:1 light, 6.2:1 dark) |
| `--surface` | `#f5f5f7` | `#161617` | Cards, table headers, slider frame |
| `--hairline` | `rgba(0,0,0,.09)` | `rgba(255,255,255,.12)` | Borders and dividers |
| `--video-bg` | `#101012` | `#101012` | Letterbox behind the demo video |
| `--radius` | `18px` | `18px` | Product frames, video, large cards |

Shadows are two-layer, never a single blur: `--shadow-hero` is
`0 30px 90px rgba(0,0,0,.22), 0 6px 24px rgba(0,0,0,.08)` in light and much
denser in dark, `--shadow-card` is `0 4px 18px`. Nothing uses an offset hard
shadow, which belongs to other worlds.

## Type

One family: Cantarell, the GNOME interface face, self-hosted as a variable
subset (`landing/assets/fonts/Cantarell-VF.woff2`, 38 KB, weights 300 to 800,
OFL, subset with `pyftsubset`). No webfont request leaves the visitor's browser,
which is what makes the "no trackers" footer line true.

Monospace is reserved for things a user would type or read verbatim: install
commands, file names, `\newpage`. Stack is `Adwaita Mono`, `Fira Mono`,
`DejaVu Sans Mono`.

| Role | Size | Weight | Tracking | Notes |
|---|---|---|---|---|
| Hero `h1` | `clamp(52px, 7.2vw, 92px)` | 700 | `-0.045em` | Line height 1.02 |
| Section statement `h2` | `clamp(34px, 4.8vw, 56px)` | 700 | `-0.035em` | `text-wrap: balance`, max 20ch |
| Final CTA title | `clamp(34px, 4.8vw, 56px)` | 700 | `-0.035em` | Same scale as statements |
| Hero sub | `clamp(21px, 2.4vw, 27px)` | 400 | normal | Muted |
| Spec figure | `44px` | 700 | `-0.03em` | Tabular numerals |
| Spec title | `21px` | 700 | `-0.01em` | |
| Lead | `19px` | 400 | normal | Muted, max 62ch |
| Body | `17px` | 400 | normal | Line height 1.55 |
| Table header | `12.5px` | 700 | `0.05em` | Uppercase, muted |
| Metadata, captions | `13.5px` to `15px` | 400 | normal | Muted |

Prose measure stays between 56 and 62 characters. Nothing runs full width.

## Layout and rhythm

Content column is capped by the section wrappers, not by a single global
container, so tables and the slider can breathe wider than text.

- Sections: `padding: 96px 0 0`, `scroll-margin-top: 72px` for the sticky nav.
- Specs row: three equal columns, 44px gap, top padding 110px.
- Hero: centred, `padding: 108px 0 0`, video capped by its own max width.
- Final CTA: `padding: 128px 0 104px`, a deliberate pause before the footer.
- Install tabs: centred segmented control, panels capped at 800px.
- Breakpoint: a single `@media (max-width: 980px)`. Nav links hide, grids
  collapse to one column, tabs scroll horizontally, code boxes gain bottom
  padding so the copy button sits under the command instead of over it.

There is one breakpoint on purpose. Two would mean two layouts to verify by
hand and this page has one shape.

## Components

**Nav.** Sticky, 48px tall, translucent with `backdrop-filter: blur`, thin
hairline. Logo, four anchor links, theme switch, language switch, primary pill.
Gains a shadow and tighter background once scrolled (`is-scrolled`, driven by
`app.js`).

**Hero.** Centred `h1`, muted sub, primary pill with a download glyph, then a
row of package chips (`.deb`, `.rpm`, AppImage, and a dashed "Flathub" ghost
chip that is honest about not being available yet). Everything above the fold
answers what it is, what it costs and where to get it.

**Video.** The real app, recorded, looped, muted, `playsinline`, poster frame
extracted from the same file. 83 KB. This is the product demo, not a mockup, and
it is the only raster that is not a screenshot.

**Specs.** Three statements with one number that matters (2.6 MB), one mechanism
(the plain Markdown buffer, nothing to sync) and one capability that is hard to
fake (real tables in the exported PDF).

**Slider.** Eight real screenshots of the app (editor, split, outline, focus,
typewriter, find, preferences, shortcuts) in both languages and both app themes,
selected by arrows, a caption and clickable thumbnails. The gallery hides itself
if a view is missing, so a partial capture run degrades instead of showing holes.

**Comparison.** Four feature tables grouped by area, a legend of `check`, `cross`,
`half` and `n/a`, and the Scribe column tinted. One lead paragraph dated with the
day the claims were checked. A separate block, "What Scribe does not do", lists
the absences in the same voice as the rest of the page. Honesty is the hook, not
a disclaimer.

**Install.** Five tabs (deb, rpm, AppImage, Flathub, AUR) sharing one sliding
indicator. Each panel holds the exact asset name for that channel, a one-line
hint, the commands and a copy button inside the code frame. Keyboard support:
arrows, Home and End. Flathub shows a "Soon" badge and says so in the note.

**About.** Three short paragraphs in first person, a signature line with the
author's GitHub, and a star button. No team photos, no mission statement.

**Final CTA.** Restates the offer in six words and repeats the download action
with its own label, so the close does not read like a copy-paste of the hero.

**Footer.** Four links with 16px inline icons (GitHub, issues, releases, AGPL),
then a single line stating that the page loads no trackers and no analytics.

## Motion

Motion exists to make the page feel alive, not to decorate it. Only `transform`,
`opacity` and `filter` are animated.

- Hero and sections rise in with `rise-in` / `reveal-up`: 26px of travel, 6 to 8px
  of blur, 0.65s, `cubic-bezier(0.22, 1, 0.36, 1)`, `both` fill mode.
- Scroll reveals use an `IntersectionObserver` at 0.12 threshold, fired once, with
  a fixed duration. They never track scroll velocity.
- Theme change goes through the View Transitions API (`startViewTransition`),
  which is the page's signature moment: the whole page crossfades in 320ms in the
  same easing. Browsers without it switch instantly.
- The screenshot slider crossfades with a 250ms opacity swap.
- Hovers lift: cards by 3px, arrows and thumbnails by a few percent, the primary
  button by 1px with a soft glow. Presses go to `scale(0.97)`.
- Reading progress is a 2px accent bar scaled with `scaleX`, updated inside
  `requestAnimationFrame`. The hero video drifts up to 28px with scroll as
  parallax. The three specs tilt up to 5 degrees toward the pointer.
- `prefers-reduced-motion: reduce` turns all of it off: reveals appear, the video
  stops looping and exposes controls, parallax and tilt are disabled, transitions
  are reduced to none.

## Accessibility

Body text is at least 13:1 in both themes, muted text 5.4:1 or better, links
5.6:1 or better. The primary button keeps white on `#0071e3` (3.77:1), which is
below the 4.5:1 threshold for small text but above the 3:1 threshold for large
text and matches the system's own suggested-action pattern: the label is 16px
semibold on a 44px pill.

One `h1`. Section headings are `h2`, table group titles are `h3`. Table headers
carry `scope="col"` and every table has a screen-reader-only caption. Thumbnails
use `aria-pressed`, tabs use `role="tablist"` with `aria-selected` and
`tabindex` managed in script, the theme switch is a `role="switch"` with
`aria-checked`, and every icon-only control has an `aria-label` that is
translated. Focus rings are 2px `--accent` with offset.

## Assets and provenance

Every raster shipped in `landing/assets` can be traced to a source.

| Asset | Origin |
|---|---|
| `scribe-demo-v4.mp4` | Recorded by the project author, 1280x840, 31.6s, 5 fps, 83 KB |
| `demo-poster.jpg` | Frame at 2s extracted from the same video with ffmpeg |
| `shots/shot-<view>-<lang>-<theme>.webp` | Screenshots of the real application at 1440x900, captured on 25 October 2026 under Xvfb against master (build including the PDF table fix), 8 views x 2 languages x 2 app themes, 88% WebP |
| `favicon.png` | Rendered from the app's own `io.github.gnacho.Scribe.svg` icon |
| `og.png` | 1200x630 composition built with ImageMagick from a real screenshot, the hero headline and the primary action |
| `fonts/Cantarell-VF.woff2` | Subset of the system Cantarell variable font (OFL) |

No asset is a stock photo, a mockup, or a placeholder. If a screenshot stops
matching the app, it gets recaptured, not retouched.

## Language

Spanish and English, at parity: 78 dictionary keys each, checked before every
deploy. Spanish is written as Spanish from Spain, addresses the reader as "tú",
and avoids calques. English is written plainly, without marketing adjectives.
Neither language uses em or en dashes anywhere, in code or in prose.

## What this page does not do

No analytics, no cookie banner, no newsletter capture, no chat widget, no
testimonials, no pricing table (the software is free), no fake counters, no
"trusted by" logos. The absence is part of the design and the footer says it out
loud.
