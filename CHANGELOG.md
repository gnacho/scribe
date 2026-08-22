# Changelog

All notable changes to Scribe will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.4] - 2026-08-22

### Fixed

- **Contenido de tablas vuelve a monoespaciada** para que la rejilla visual
  cuadre. La rejilla (cabecera en negrita, separadores pintados, filete) se
  apoya en el padding del fuente que produce `format_tables`; con fuente
  proporcional las columnas no alineaban. El inline (negrita, código, color)
  sigue aplicando: son atributos independientes de la familia. Si una tabla no
  cuadra, Ctrl+Alt+T («Formatear tablas») la deja alineada.
- **Test de render headless**: `tests/render_shot.rs` renderiza el editor a PNG
  bajo xvfb (`SCRIBE_SHOTS_DIR` o temp dir) para verificar visualmente la vista
  enriquecida sin abrir ventanas. El camino de salida original apuntaba al
  sandbox del agente (`/mnt/agents/work/shots`) y fallaba en máquinas reales;
  ahora es configurable por variable de entorno.

## [0.3.2] - 2026-08-22

### Fixed

- **Crash «byte index off the end of the line» (SIGABRT) eliminado por
  construccion**: GTK aborta (GNOME/gtk#8346, `gtktextbtree.c:4012`) cuando el
  buffer tiene texto `invisible=true` y una conversion pixel-iter consulta
  geometria con el layout obsoleto. El editor ya no genera texto invisible en
  ningun camino; las marcas se atenuan o encogen (`syn_shrink`). Cuando GTK
  publique el fix, la puerta `gtk_hides_invisible_safely()` reactivara el
  ocultado real sin tocar mas nada. El canario `gtk_invisible_canary` detecta si
  el GTK del sistema sigue siendo vulnerable.
- **Perdida de datos al alinear tablas** (`format_tables`): absorbia lineas
  ajenas con `\|` y descartaba celdas de filas mas anchas que la cabecera. Ahora
  el cuerpo lo delimita pulldown-cmark y las columnas cubren la fila mas ancha.
  Conserva los finales CRLF por linea.
- **Fugas de memoria**: ciclos `Rc` en StyleManager, senales del buffer y
  acciones de ventana impedian que ninguna ventana ni editor se liberara. Todo a
  `downgrade()`/`upgrade()`.
- **Errores tragados en silencio**: el autoguardado y las preferencias ahora
  avisan cuando falla una escritura (toast una vez por racha).
- **Guardar sin extension** ahora anade `.md`.
- **Indexacion defensiva** del mapa byte-char (acceso a rango).
- **Manifest Flatpak** (issue #5): runtime GNOME 50, `cargo-sources.json` en el
  formato oficial de flatpak-cargo-generator, `Cargo.lock` commiteado, metainfo
  AppStream 1.0 conforme, CI endurecido (MSRV 1.83, `--locked`, appstreamcli).

### Added — vista de edicion enriquecida estilo GitHub

- **Bloques de codigo con caja de fondo** y el contenido en monoespaciada.
- **Tablas como tabla visual**: caja, cabecera en negrita, fila de guiones
  reducida a un filete y separadores de columna pintados.
- **Titulos**: jerarquia por escala/peso + filete inferior en H1/H2; las
  almohadillas se encogen en modo WYSIWYG.
- **Citas con barra lateral** dibujada y **listas** con vinetas y casillas en el
  canalon.
- **Imagenes locales en bloque**: si `![alt](ruta)` esta sola en su linea y el
  fichero existe (relativo al documento), se pinta escalada (max 144 px, limite
  20 MB, cache de 64). Remotas o ausentes, placeholder discreto. El documento no
  se contamina: nunca se insertan widgets ni caracteres.
- **Preferencias con sentido real**: «Atenuar» = vista cruda atenuada con
  adornos de ambiente; «Ocultar»/«Al enfocar» = WYSIWYG con marcas encogidas
  (etiquetado como «encoge las marcas; GTK aun no permite ocultarlas»).

### Changed

- El marcado sustituido por un adorno se **encoge** (escala 0.05 + alpha 0) en
  vez de ocultarse: sigue en la maquetacion de GTK, asi que el camino del aborto
  es inalcanzable por construccion.
- El cursor solo re-aplica el atenuado del modo foco (antes re-analizaba todo).
- Una sola copia del buffer para los contadores, en el timeout debounced.
- README reescrito: EN principal + `README.es.md`, con las features de la vista
  enriquecida.

## [0.3.1] - 2026-08-08

### Fixed

- **Una valla de codigo seguida de un tabulador no cerraba el bloque**, con lo
  que el resto del documento pasaba a verse como codigo. El spec de CommonMark
  dice que la valla de cierre «puede ir seguida solo de espacios o tabuladores,
  que se ignoran», pero pulldown-cmark no lo respeta: se reproduce con
  `` ```\ncodigo\n```\t\n `` tanto en 0.12 como en 0.13, asi que no es cosa
  nuestra ni esta arreglado aguas arriba. Como apano, `normalize_fences()`
  sustituye por espacios los tabuladores que van detras de una valla antes de
  parsear. Solo toca lineas de valla, y como espacio y tabulador ocupan un byte
  cada uno, todos los desplazamientos siguen valiendo sobre el texto original:
  los tabuladores de dentro del bloque no se tocan.
- Tres tests nuevos, incluido el de que la normalizacion conserva las
  posiciones (33 en total).

## [0.3.0] - 2026-08-08

### Added — tablas

- **Las tablas se renderizan como tablas**: la fila de guiones del fuente se
  oculta y en su hueco se dibuja la línea que separa la cabecera; el bloque
  lleva su propia caja redondeada, distinta de la de los bloques de código; y
  los pipes quedan atenuados, haciendo de separador de columna sin competir con
  el contenido.
- **Alinear tablas** (Ctrl+Alt+T, y en el menú principal): reformatea todas las
  tablas del documento para que sus columnas cuadren en el fuente. Como el
  bloque va en monoespaciada, alinear el fuente es lo que hace que la tabla se
  vea alineada en pantalla. Respeta los `:` de alineación, los pipes escapados
  (`\|`) y las tablas de ejemplo dentro de bloques de código, que no se tocan.
  Es una sola acción de usuario: se deshace de golpe con Ctrl+Z.
- Siete tests nuevos para el formateador y los adornos de tabla (29 en total).

### Changed

- El tag `tabledelim` desaparece: la fila de guiones ya no se atenúa, se oculta.

## [0.2.1] - 2026-08-08

### Fixed

- **Crash al editar** (`gtk_text_iter_set_visible_line_index`, «byte index off
  the end of the line»). Tres causas encadenadas, las tres arregladas:
  - `decorate()` se llamaba de forma sincrona desde
    `notify::cursor-position`, que es una senal del buffer. Aplicar tags de
    invisibilidad mientras GTK esta procesando una edicion descuadra su
    maquetacion. Ahora toda la decoracion pasa por `schedule_decoration()` y se
    difiere al bucle principal; no se toca el buffer desde ninguna senal suya.
  - Al dibujar, un bloque de codigo que abarcaba mucho mas que la pantalla hacia
    que se pidiera geometria de lineas muy lejanas, obligando a GTK a validar
    miles de lineas en mitad del `snapshot`. La geometria se limita ahora a la
    franja visible y la caja se alarga fuera de pantalla, para que las esquinas
    redondeadas no aparezcan cortadas a mitad del bloque.
  - Los adornos van por numero de linea; si el buffer cambiaba entre el calculo
    y el dibujado, apuntaban a sitios equivocados. `set_ornaments` guarda ahora
    el numero de lineas para el que se calcularon y el dibujado se salta el
    fotograma si ya no coincide.
- **La version en `Cargo.toml` seguia en 1.0.0** desde la pre-alpha, asi que la
  ventana «Acerca de» mentia. Ahora sale de `CARGO_PKG_VERSION` y coincide con
  el CHANGELOG. De paso: licencia como `AGPL-3.0-or-later`, `rust-version`
  declarada y fuera `serde`/`serde_json`, que no los usa nadie.
- **Tags de bloque que se extendian de mas**: el mismo problema de decorar
  dentro de una senal podia dejar el tag `codeblock` cubriendo texto posterior
  al cierre de la valla, con lo que el documento entero pasaba a verse como
  codigo despues de editar.

## [0.2.0] - 2026-08-07

## [0.2.0] - 2026-08-07

### Added — adornos dibujados

- **`src/markdown_view.rs`**: `MarkdownView`, un `GtkSourceView` subclaseado que
  implementa `snapshot_layer` para pintar lo que un `GtkTextTag` no puede
  expresar. Dibuja vinetas (disco, anillo y cuadrado segun el nivel), casillas
  de tarea con su marca de verificacion, reglas horizontales, la barra vertical
  de las citas y la caja redondeada de los bloques de codigo. Trabaja en
  coordenadas de buffer y solo calcula geometria de lo que esta en pantalla.
- **`Ornament` y `Analysis`** en `markdown_render`: el analizador devuelve ahora
  tramos *y* adornos referidos a numero de linea.
- **`SpanKind`**: distingue estilo permanente, marca de Markdown y marca
  sustituida por un adorno. Las sustituidas no se revelan al pasar el cursor,
  porque hacerlo desplazaria el texto en cada cambio de linea.
- **Notas al pie**: la referencia deja solo el numero en volado y la definicion
  se atenua.
- Tests: 16 -> 22.

### Changed

- Las citas y los bloques de codigo ya no usan `paragraph-background`: su caja y
  su barra se dibujan, lo que permite esquinas redondeadas y margenes reales.
- Las listas sin ordenar pierden la sangria francesa: la vineta va en el canalon
  y todas las lineas del elemento arrancan alineadas.
- En modo «atenuar siempre» los adornos se desactivan, para no duplicar
  informacion con las marcas visibles.

### Fixed

- **Sangria de listas anidadas**: pulldown-cmark empieza el rango del elemento en
  el marcador, no al principio de la linea. Como el tag no cubria el inicio del
  parrafo, GTK aplicaba el margen del nivel de arriba y la vineta quedaba
  alineada con la lista padre. Ahora el tramo llega hasta el inicio de linea y la
  sangria literal se oculta, para que el desplazamiento lo de solo el margen.
- **Espacio suelto tras las casillas**: el rango de `TaskListMarker` cubre `[x]`
  pero no el espacio siguiente, que quedaba como sangria fantasma.

### Added — interfaz HIG, plantillas y preferencias

- **Preferencias completas** (`src/preferences.rs`) con `AdwPreferencesWindow` y
  tres paginas: Apariencia (esquema de color, familia tipografica, tamano,
  interlineado, ancho de columna), Editor (visibilidad del marcado, continuar
  listas, modo foco, maquina de escribir, tabulacion, autoguardado e intervalo)
  y Plantillas.
- **Plantillas** (`src/templates.rs`): ficheros `.md` en
  `$XDG_DATA_HOME/scribe/templates`, sembrados con cuatro ejemplos la primera
  vez. Admiten `{{title}}`, `{{date}}`, `{{time}}`, `{{datetime}}` y `{{year}}`.
- **Visibilidad del marcado configurable**: ocultar siempre, revelar en la linea
  del cursor o atenuar siempre.
- **Modo foco** (Ctrl+Shift+F) y **maquina de escribir** (Ctrl+Shift+T).
- **Continuacion de listas** al pulsar Intro, con renumeracion de las ordenadas
  y cierre automatico al dejar un elemento vacio.
- **Zoom** (Ctrl +/-/0) con fila de control en el menu principal.
- **Ir a la linea** desde la barra de estado.
- **Marcado ampliado**: cabeceras setext, autoenlaces `<url>`, notas al pie,
  bloques HTML atenuados y tablas en monoespaciada para que las columnas cuadren.
  Tests: 13 -> 16.
- **Ventana de atajos** externalizada a `src/shortcuts.ui`.

### Changed

- **Cabecera reorganizada** igual que GNOME Text Editor: `AdwSplitButton`
  «Abrir» con desplegable de recientes y boton de documento nuevo a la
  izquierda, titulo al centro, menu principal a la derecha. La barra de estado
  lleva el contador de palabras a la izquierda y los botones de posicion y
  propiedades a la derecha.
- **Alternancias con estado** (`SimpleAction::new_stateful`) para barra lateral,
  vista dividida, modo foco y maquina de escribir: el menu muestra la marca.
- **Esquema GSettings** reescrito con enumeraciones y rangos.
- **`settings.rs`** reescrito con tipos propios (`MarkupVisibility`,
  `FontFamily`) en lugar de cadenas sueltas.

### Fixed

- Restauradas `app.quit` y `app.new-window`, que se habian perdido al rehacer
  `main.rs`: Ctrl+Q no hacia nada y dos entradas del menu salian grises.
- Restaurado `ApplicationFlags::HANDLES_OPEN`, sin el cual `scribe fichero.md` y
  «Abrir con» del gestor de archivos se ignoraban.
- `GtkTextTag:letter-spacing` no admite valores negativos: usarlos abortaba la
  aplicacion al construir los tags de cabecera.

### Added — render WYSIWYG en linea

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
