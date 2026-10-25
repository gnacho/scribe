# Changelog

All notable changes to Scribe will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.21] - 2026-10-25

Thank you [@HelloKolla2004](https://github.com/HelloKolla2004) for the PDF
tables report and [@kem-a](https://github.com/kem-a) for two sharp reports:
the quit crash came with a stack trace that pointed straight at the culprit,
and the missing icons came with a screenshot that said the rest.

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

### Docs

- **The project has a website**: every feature is shown in action at
  [scribe.cloudless.club](https://scribe.cloudless.club). The READMEs invite
  the reader to it, and the AppStream homepage and the About dialog now use
  it instead of the GitHub repository (#144).

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

- **Vista dividida desde la cabecera** (#35): un icono en la cabecera muestra u
  oculta la previsualización, que se pinta a la derecha del editor con el mismo
  motor de `GtkTextTag`.
- **Esquemas de color en el menú** (#36): seguir el sistema, claro y oscuro, como
  acción con estado y sincronizada con la página de Apariencia de Preferencias.
  El menú deja de ofrecer lo que ya está en la cabecera (nueva ventana, abrir,
  recargar y cerrar pestaña).
- **Buscar en el documento** (#37): barra de búsqueda dentro del panel del
  editor, con contador de coincidencias, botones de anterior/siguiente y Ctrl+F,
  Intro y Mayús+Intro.
- **Imágenes remotas** (#38): las imágenes con URL se descargan y se cachean en
  `~/.cache/scribe/images` (se pregunta primero el tamaño, con límite de 20 MB),
  de modo que las siguientes veces se pintan sin salir a la red. Si la descarga
  falla queda el placeholder y no se reintenta en cada repintado.
- **Tarjetas de YouTube** (#39): un vídeo solo en su línea (enlace de Markdown,
  URL suelta o `<iframe>`) se dibuja como tarjeta 16:9 con su miniatura y botón
  de reproducción, sin tocar el buffer. No hay reproductor embebido: al pulsar
  la tarjeta se abre el navegador.
- **Ctrl+clic en un enlace** (#40): abre el destino con la aplicación
  predeterminada (solo `http`, `https` y `mailto`) y, con Ctrl pulsado encima de
  un enlace, el globo de ayuda dice a dónde va. El clic normal sigue siendo del
  texto: coloca el cursor y selecciona.
- **Exportar a PDF** (Ctrl+P) por el pipeline de impresión de GTK, con ajustes
  propios (esquema de color, fuentes, tamaño y orientación) y paginación
  cuidada, y **recarga automática** cuando el fichero cambia en disco (#31).
- **Pegado y arrastre de imágenes**: se guardan junto al documento y se enlazan
  con ruta relativa. **Recuperación del borrador** tras un cierre inesperado
  (#31).
- **Línea de órdenes**: `scribe fichero.md:12:4` abre en la línea y columna
  indicadas, y acepta contenido por tubería (#31).
- **Icono nuevo** (#44): un portanotas con una nota (cabecera, línea
  resaltada y lista) y una pluma estilográfica escribiendo en la esquina,
  dibujado en Impress y exportado a SVG; se lee de 16 px en adelante.

### Changed

- **Instalación de usuario** (`make install`) que incluye el binario, el esquema
  GSettings recompilado, el fichero .desktop y los iconos; **MSRV a Rust 1.85**
  (el árbol de dependencias exige edition2024) y **presupuestos de rendimiento**
  en modo release para abrir y decorar (#31).
- **Menú principal depurado** (#46): fuera la sección «Estilo» (el esquema de
  color se elige en Preferencias), la entrada «Vista dividida» (está en la
  cabecera y en Ctrl+Shift+P), la fila de zoom con el tamaño de fuente en px,
  y «Alinear tablas» y «Cerrar vallas». Las acciones y sus atajos siguen:
  Ctrl+Shift+P, Ctrl +/-/0, Ctrl+Alt+T y Ctrl+Alt+F.
- **Privacidad y documentación**: el aviso de privacidad pasa a decir la verdad
  sobre la red (las únicas peticiones son las que pide el documento, con su
  caché en disco) y el README (inglés y español) recoge las funciones nuevas con
  sus límites reales y capturas al día.

### Removed

- **Barra de estado** (#45): se quita el footer con el contador de palabras, el
  botón «Ln, Col» con su popover de ir a la línea, el botón «Markdown» con las
  propiedades y el aviso de vallas. «Cerrar vallas» queda en Ctrl+Alt+F y
  «Alinear tablas» en Ctrl+Alt+T.

### Fixed

- **Aborto al arrancar en Wayland** (#32, #34): los márgenes de columna se
  aplicaban dentro del ciclo de validación de GTK y disparaban el `g_assert` de
  `gtk_text_view_validate_onscreen` en cada arranque bajo un compositor (GTK
  4.22.5 y 4.24.1; con Xvfb no se reproducía). Ahora se aplican desde un idle.
- **Tablas** (#30, #34): las columnas se miden desde la fila de guiones en vez de
  desde los espacios que tenga cada fila, con una línea vertical continua por
  frontera, regla de cabecera y respeto de `:---`, `:---:` y `---:`. La fila
  donde está el cursor dentro de una tabla se muestra en su fuente para poder
  editarla. Una celda más ancha que su columna sigue desbordando: Ctrl+Alt+T
  («Formatear tablas») la deja alineada.
- **Test de pestañas** (#33, #34): el título del borrador se comparaba con el
  msgid sin traducir.

## [0.3.10] - 2026-09-19

### Added

- **Interfaz traducida (gettext)**: las cadenas de la interfaz pasan a inglés
  como idioma fuente, con catálogo completo en español (`po/es.po`), de modo
  que cada usuario ve Scribe en el idioma de su sistema. Nuevas traducciones en
  `po/` (ver `po/update-pot.sh`). Las plantillas integradas se crean en el
  idioma actual en el primer arranque y los atajos de teclado se traducen.
- **Detección y cierre de vallas de código sin cerrar** (#11): cuando una valla
  fenced no se cierra, la barra de estado avisa («Valla abierta (línea N)») y
  una acción (menú, Ctrl+Alt+F o pulsando el aviso) la cierra con una
  heurística best-effort (antes de la siguiente cabecera o regla, de la
  siguiente valla de apertura o al final del documento). Deshacer (Ctrl+Z)
  restaura el texto original.

## [0.3.8] - 2026-08-22

### Fixed

- **Caja de las tablas y regla de cabecera abrazan el contenido**: la caja de
  fondo se pintaba a todo el ancho de la columna de texto (sobraba un
  rectángulo pálido a la derecha, la «columna fantasma») y la regla bajo la
  cabecera cruzaba de margen a margen. Ahora miden el contenido real de las
  filas (con los pipes encogidos, el texto de las celdas); tablas de más de 200
  filas se miden solo en la ventana visible. Las celdas con pipe escapado
  (`\|`) muestran solo `|`; la barra invertida se encoge como marca de escape.

## [0.3.6] - 2026-08-22

### Added

- **Pestañas reales multi-documento** (`AdwTabView` + `AdwTabBar`, estilo
  gnome-text-editor). Cada documento tiene su editor, su undo, su estado de
  guardado y su pestaña, con botón de cierre, reordenado por arrastre y
  overflow. `Ctrl+N` nueva pestaña; `Ctrl+O`, recientes o `scribe fichero.md`
  abren pestañas en la ventana activa; `Ctrl+W` cierra; `Ctrl+Tab` /
  `Ctrl+Mayús+Tab` / `Ctrl+AvPág` / `Ctrl+RePág` navegan. Abrir un fichero ya
  abierto enfoca su pestaña (dedupe por ruta canónica). Al cerrar una pestaña
  con cambios: diálogo Guardar/Descartar/Cancelar; al cerrar la ventana con
  varios modificados: diálogos secuenciales. Punto «•» en la pestaña modificada
  + badge de atención en segundo plano; autoguardado por documento. La última
  pestaña no se cierra (queda una en blanco).

### Fixed

- **Caja de fondo continua en bloques largos**: la caja se alargaba fuera de
  pantalla para esconder las esquinas, y con origen muy negativo GSK no pintaba
  nada (la caja desaparecía a mitad de un bloque de código de 120 líneas). Ahora
  el rectángulo se limita a lo visible y las esquinas redondeadas solo se
  dibujan en los bordes reales del bloque; la barra de cita usa el mismo
  criterio.

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
