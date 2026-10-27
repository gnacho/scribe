# Scribe

<p align="center">
  <a href="README.es.md">Español</a> |
  <a href="README.md">English</a>
</p>

<p align="center">
  <a href="https://flathub.org/apps/io.github.gnacho.Scribe"><img alt="Flathub" src="https://img.shields.io/badge/Flathub-io.github.gnacho.Scribe-4a90d9"></a>
  <a href="https://github.com/gnacho/scribe/releases"><img alt="Release" src="https://img.shields.io/github/v/release/gnacho/scribe"></a>
  <a href="LICENSE"><img alt="License: AGPL-3.0-or-later" src="https://img.shields.io/badge/license-AGPL--3.0--or--later-blue"></a>
</p>

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/hero-es-dark.png">
    <source media="(prefers-color-scheme: light)" srcset="assets/hero-es-light.png">
    <img alt="Ventana del editor Scribe mostrando texto Markdown con render WYSIWYG en vivo" src="assets/hero-es-light.png" width="800">
  </picture>
</p>

Scribe es un editor Markdown nativo para GNOME, escrito en Rust con GTK4 y
libadwaita. El Markdown se renderiza en vivo sobre el propio buffer de edición:
las cabeceras se ven con su tamaño, la negrita en negrita y las marcas de
sintaxis se atenúan o encogen, sin cambiar a una previsualización aparte ni
usar un motor de navegador.

Míralo en acción en [scribe.cloudless.club](https://scribe.cloudless.club): todas
las funciones explicadas y funcionando, capturas en claro y oscuro, una demo de
escritura y la instalación para Flathub, .deb, .rpm, AppImage y AUR.

Disponible en Flathub:

```sh
flatpak install flathub io.github.gnacho.Scribe
```

## Por qué existe esto

Quería un editor Markdown que se sintiera parte de GNOME, no un porte de una
aplicación web. Los que probé o bien metían un motor de navegador (ProseMirror,
Milkdown dentro de WebKit) y consumían cientos de megabytes, o mostraban el
código fuente a un lado y la previsualización al otro. Acababa con los dos
paneles abiertos, yendo y viniendo entre ellos en vez de escribir.

La idea es un editor donde el texto fuente ES la previsualización: el buffer
sigue siendo Markdown sin más, pero el renderizado se pinta encima con
`GtkTextTag`. Así no hay nada que sincronizar, no hay HTML por debajo ni que
empaquetar un motor de navegador. Empezó como maquetas para explorar el diseño
con GTK4 y ha terminado siendo algo que uso de verdad para borradores y notas.

## Por qué este stack

- **Rust + GTK4 + libadwaita** &mdash; toolkit nativo, sin Electron. El binario
  ocupa unos 8 MB y en reposo consume unas docenas de megabytes de RAM. Un
  editor basado en WebKit pesaría diez veces eso antes de abrir un archivo.
- **GtkSourceView 5** para el buffer de texto y **pulldown-cmark** para parsear
  Markdown. Luego se aplican tramos con `GtkTextTag` para el render. Sin HTML
  ni CSS en la previsualización. El editor es un único `GtkTextView` con
  decoraciones sobre el buffer.
- **Sin base de datos, sin servidor, sin JavaScript.** Es una aplicación de
  escritorio que abre, edita y guarda archivos. Las preferencias van por
  GSettings.

## Características

- **Render WYSIWYG en vivo** sobre el buffer de edición: cabeceras a escala
  real, negrita/cursiva/tachado, código en línea y en bloque, citas, listas,
  enlaces, imágenes, notas al pie, tablas y bloques HTML vía tramos de
  `GtkTextTag`.
- **Vista enriquecida** pintada sobre el buffer (`snapshot_layer`): bloques de
  código con caja redondeada, tablas como rejilla visual (cabecera en negrita,
  separadores de columna pintados y un filete donde estaba la fila de guiones),
  filete bajo H1/H2, barra lateral de citas, viñetas dibujadas por nivel,
  casillas de tarea marcables y reglas horizontales. La marca que sustituye un
  adorno se **encoge** en vez de ocultarse.
- **Corrector ortográfico**: las palabras mal escritas se subrayan al
  escribir, con sugerencias e «ignorar/añadir al diccionario» en el menú
  contextual; el idioma sigue al sistema y se puede apagar en Preferencias.
- **Tablas de prosa como tablas de verdad**: cuando las celdas de prosa no
  caben en la geometría del delimitador (la tabla larga de toda la vida), las
  filas fuente colapsan y la tabla se pinta como lienzo: cada celda envuelve
  su texto dentro de su columna, la cabecera va en negrita con filete y se
  respetan las alineaciones. La fila bajo el cursor muestra su fuente crudo
  para editarla.
- **Imágenes locales en bloque**: cuando un `![alt](ruta)` está solo en su
  línea y el fichero existe (relativo al documento), se pinta escalado bajo la
  marca (máx. 144 px, límite de 20 MB, caché de 64). Los ficheros ausentes
  muestran un placeholder discreto. El buffer nunca se modifica: no se insertan
  widgets ni caracteres.
- **Imágenes remotas**: cuando el destino es una URL, Scribe pregunta primero su
  tamaño (límite de 20 MB), la descarga y la cachea en disco en
  `~/.cache/scribe/images`, de modo que las siguientes veces se pinta sin salir
  a la red. Una URL que falla deja el placeholder y no se reintenta en cada
  repintado.
- **Tarjetas de YouTube**: un vídeo que está solo en su línea, escrito como
  enlace de Markdown, URL suelta o `<iframe>`, se dibuja bajo la línea como una
  tarjeta 16:9 con su miniatura y un botón de reproducción. La línea sigue
  visible y editable, y no hay reproductor embebido: al pulsar la tarjeta se
  abre el navegador.
- **Ctrl+clic en un enlace** abre el destino con la aplicación predeterminada;
  con Ctrl pulsado encima de uno, el globo de ayuda dice a dónde va. Solo se
  ofrecen `http`, `https` y `mailto`, así que un clic normal sigue siendo un
  clic de texto (cursor, selección) cuando no hay nada que abrir.
- **Visibilidad del marcado configurable**: atenuar las marcas de sintaxis
  (`**`, `#`, backticks) siempre, ocultarlas o revelarlas en la línea del
  cursor. Por un bug de GTK
  ([gtk#8346](https://gitlab.gnome.org/GNOME/gtk/-/issues/8346)), "ocultar"
  actualmente *encoge* las marcas (escala ~0) en vez de quitarlas; cuando GTK
  publique el fix, el ocultado real vuelve solo.
- **Modo foco** (Ctrl+Shift+F) atenúa todo salvo el párrafo actual.
- **Máquina de escribir** (Ctrl+Shift+T) mantiene el cursor centrado
  verticalmente.
- **Vista dividida a la derecha** (Ctrl+Shift+P o el icono de la cabecera) con
  render completo en un panel lateral usando el mismo motor de `GtkTextTag`, de
  modo que puedes seguir el índice mientras escribes.
- **Buscar en el documento** (Ctrl+F): barra de búsqueda dentro del panel del
  editor, con contador de coincidencias, botones de anterior/siguiente e
  Intro/Mayús+Intro.
- **Ctrl+B/I/K** envuelve la selección en `**`, `*` o backticks.
- **Alinear tablas** (Ctrl+Alt+T) reformatea todas las tablas del documento
  para que las columnas cuadren en el fuente.
- **Continuación de listas**: al pulsar Intro se crea el siguiente elemento y
  se renumeran las listas ordenadas. Dejar un elemento vacío cierra la lista.
- **Zoom** (Ctrl +/&minus;/0).
- **Cabecera** al estilo de GNOME Text Editor: botón de abrir con desplegable
  de recientes, botón de nueva pestaña, título centrado, botón de vista
  dividida y menú principal.
- **Preferencias** en tres páginas: Apariencia, Editor y Exportación.
- **Exportar a PDF** (Ctrl+P): imprime o exporta el documento a través del
  pipeline de impresión de GTK, reutilizando el mismo análisis de tramos y
  adornos del editor. Las cabeceras, los bloques de código, las citas y las
  tablas se tematizan, y la maquetación se elige con plantillas de página
  (Artículo, Manuscrito, Carta, Compacta o Personalizada), temas de hoja de
  estilo y, si hace falta, un CSS propio. La paginación es cuidadosa (sin
  cabeceras huérfanas, soporte de `\newpage`).
- **Vista previa de impresión**: la misma maquetación del PDF, paginada, antes
  de imprimir.
- **Archivos**: abrir, guardar y guardar como con `GtkFileDialog`, escritura
  atómica, aviso de cambios sin guardar y autoguardado configurable.
- **Barra lateral** (F9) con documentos recientes filtrables e índice del
  documento navegable.
- **Tema claro y oscuro** que sigue el style scheme de GtkSourceView; las
  Preferencias eligen el esquema de color explícitamente (seguir el sistema,
  claro, oscuro).
- **Integración**: `scribe fichero.md` y "Abrir con" del gestor de archivos.
- **Interfaz traducida** (gettext): inglés y español. Nuevas traducciones en
  `po/` (ver `po/update-pot.sh`).

## Cómo funciona el render

Dos módulos, ninguno con tipos de la aplicación dentro:

- **`src/markdown_render.rs`** analiza el Markdown con pulldown-cmark y
  devuelve dos listas: *tramos* (rangos en bytes con el nombre del
  `GtkTextTag`) y *adornos* (elementos referidos a número de línea que hay que
  pintar). No depende de GTK y se prueba sin display.
- **`src/markdown_view.rs`** es un `GtkSourceView` subclaseado que implementa
  `snapshot_layer`, el vfunc que GTK expone para pintar debajo o encima del
  texto. Trabaja en coordenadas de buffer (GTK resuelve el desplazamiento) y
  dibuja con `gsk::PathBuilder`.

Las marcas que sustituye un adorno (viñetas, `[x]`, `---`, `>`, vallas, pipes)
se encogen en vez de ocultarse: el glifo diminuto sigue en la maquetación de
GTK, así que el camino del aborto por texto invisible de GNOME/gtk#8346 queda
inalcanzable por construcción.

## Plantillas, temas y hojas de estilo

El aspecto del PDF se decide en dos capas que se suman. La **plantilla** fija la
geometría: márgenes, interlineado, escala de cabeceras, cabecera y pie con
numeración, y bloque de título a partir del nombre del documento. El **tema**
fija tipografía y color. Cualquier tema se combina con cualquier plantilla.

| Plantilla | Para qué |
| --- | --- |
| Personalizada | Usa los ajustes granulares de Preferencias |
| Artículo | La maquetación de siempre |
| Manuscrito | Serif, interlineado doble, márgenes de una pulgada, cabecera con el título y pie con número de página |
| Carta | Sin bloque de título, márgenes ajustados y pie con número de página |
| Compacta | Notas técnicas, espaciado apretado |

| Tema | Qué hace |
| --- | --- |
| Ninguno | Deja la plantilla tal cual |
| Sepia | Papel crema, serif cálida y espaciado amplio para leer mucho rato |
| Alto contraste | Negro sobre blanco y cuerpo más grande, para imprimir |
| Técnico | Cuerpo monoespaciado y colores apagados, para notas y código |
| Académico | Serif sin color y leading amplio, en la línea de un paper |

Elegir `Fichero propio…` carga un CSS del disco que se aplica **encima** del
tema, así que puede corregir un solo detalle. El analizador es un subconjunto
mapeado, no un motor CSS: el export pinta y pagina con cairo, de modo que cada
declaración se traduce a un valor que la maquetación ya entiende y lo que queda
fuera se ignora con un aviso. Quitar el fichero o elegir Ninguno devuelve la
salida anterior tal cual.

| Selector | Propiedades |
| --- | --- |
| `@page` | `margin`, `margin-top`, `margin-right`, `margin-bottom`, `margin-left` (`pt`, `mm`, `cm`, `in`, `px`) |
| `body`, `html`, `:root` | `font-family`, `font-size`, `line-height`, `color`, `background-color`, `margin-bottom`, `--block-spacing`, `--muted-color` |
| `p` | `line-height`, `color`, `margin-bottom`, `--block-spacing` |
| `h1` a `h4` | `font-size`, `color` |
| `pre`, `code` | `font-family`, `color`, `background-color` |
| `blockquote` | `color`, `border-color` (la barra de la cita) |
| `a` | `color` |
| `table` | `color`, `background-color` |
| `th` | `color`, `background-color` |
| `hr` | `color` |

## Limitaciones

- **Solo UTF-8.** Los ficheros se leen como UTF-8 y se escriben de vuelta byte
  a byte. Un fichero en otra codificación (Latin-1, UTF-16, etc.) no se abre.
- **El fuente es la previsualización.** El render es un conjunto de tramos
  `GtkTextTag` y adornos pintados sobre el buffer, no un motor de maquetación.
  El HTML del fuente no se interpreta: queda como texto atenuado en el editor y
  se descarta en la vista dividida. El resultado es una aproximación del
  Markdown, no una copia del render de un navegador.
- **Sin exportar a HTML y sin corrección ortográfica.** La barra de buscar
  (Ctrl+F) busca en el documento activo, con sensibilidad a mayúsculas y
  expresiones regulares como opciones. Ctrl+H despliega una segunda fila con
  Reemplazar y Reemplazar todo, así que el reemplazo se hace en el sitio, no en
  un diálogo aparte.
- **La exportación a PDF no lleva índice ni marcadores.** `GtkPrintOperation`
  no expone marcadores de PDF, así que el PDF exportado no tiene tabla de
  contenidos ni outline.
- **Imágenes**: los ficheros locales de hasta 20 MB se pintan, como máximo
  144 px de alto, y las URLs remotas se descargan con los mismos límites. Las
  imágenes inline (no solas en su línea), los ficheros ausentes y las descargas
  que fallan muestran el placeholder.
- **La caché de imágenes no caduca.** Una imagen editada en disco no se
  refresca hasta reabrir el documento, y la descargada se reutiliza desde
  `~/.cache/scribe/images` hasta que borres ese directorio.
- **Los vídeos no se reproducen dentro del editor.** La tarjeta se dibuja a
  partir del id del vídeo y de una miniatura que se pide a `i.ytimg.com`, y al
  pulsarla se abre el navegador. Un enlace cuyo id no sea un id válido de 11
  caracteres se queda como texto.
- En el modo «ocultar» las marcas se ocultan por elemento, al estilo Obsidian:
  se revelan las del elemento donde esta el cursor (o tocando sus bordes) y el
  resto sigue encogido. Las marcas de cabecera, lista y cita se revelan por
  linea. Las marcas se encogen, no se quitan (ver arriba).

## Privacidad

Scribe no recopila nada y no tiene cuentas, telemetría ni nube. Las únicas
peticiones que hace son las que le pide un documento: las imágenes remotas y las
miniaturas de vídeo referenciadas por el texto se descargan para pintarlas y se
cachean en `~/.cache/scribe/images`. Nunca se sube nada. Ver
[PRIVACY.md](PRIVACY.md).

## Capturas

**Ventana principal con render Markdown en vivo**

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/hero-es-dark.png">
  <source media="(prefers-color-scheme: light)" srcset="assets/hero-es-light.png">
  <img alt="Ventana principal del editor con cabeceras, negrita, cursiva, una cita, una lista, una tabla alineada y una imagen pintada desde una URL" src="assets/hero-es-light.png" width="800">
</picture>

**Vista dividida (con el icono de la cabecera) y buscar en el documento**

<p align="center">
  <img alt="Editor con la vista dividida a la derecha renderizando el mismo documento" src="assets/screenshot-split-es-light.png" width="800">
</p>

<p align="center">
  <img alt="Barra de buscar dentro del panel del editor, con anterior y siguiente" src="assets/screenshot-find-es-light.png" width="800">
</p>

**Ventana de preferencias**

<p align="center">
  <img alt="Ventana de preferencias con las páginas de Apariencia, Editor y Exportación" src="assets/screenshot-preferences-es-light.png" width="800">
</p>

**Modo foco (todo atenuado salvo el párrafo actual)**

<p align="center">
  <img alt="Modo foco en el editor con el documento atenuado y el párrafo actual resaltado" src="assets/screenshot-focus-es-light.png" width="800">
</p>

## Requisitos de compilación

- Rust 1.85 o superior
- GTK4 (&ge; 4.14), libadwaita (&ge; 1.5), GtkSourceView 5 y GLib con sus
  ficheros de desarrollo

Arch / CachyOS:

```sh
sudo pacman -S rust gtk4 libadwaita gtksourceview5 glib2
```

Debian / Ubuntu:

```sh
sudo apt install libgtk-4-dev libadwaita-1-dev libgtksourceview-5-dev libenchant-2-dev libglib2.0-dev
```

Fedora:

```sh
sudo dnf install gtk4-devel libadwaita-devel gtksourceview5-devel enchant2-devel glib2-devel
```

## Instalar desde Flathub

Flathub es la forma más sencilla de instalar Scribe, y se actualiza junto con el
resto del sistema:

```sh
flatpak install flathub io.github.gnacho.Scribe
flatpak run io.github.gnacho.Scribe
```

La ficha está en [flathub.org/apps/io.github.gnacho.Scribe](https://flathub.org/apps/io.github.gnacho.Scribe).
Hay builds para x86_64 y aarch64. Los paquetes `.deb`, `.rpm` y AppImage se
siguen generando en cada versión, por si prefieres instalarlos a mano.

## Compilar e instalar en local

Instala en `~/.local` (sin root) con:

```sh
make install
```

Esto compila el binario en modo release e instala el binario en `~/.local/bin`,
el esquema GSettings en `~/.local/share/glib-2.0/schemas` (recompilado con
`glib-compile-schemas`), el fichero .desktop y los iconos. `make build` compila
en modo release y `make check` ejecuta fmt, clippy y los tests.

El esquema GSettings es obligatorio, no opcional: cuando una versión añade una
preferencia nueva, la aplicación aborta si el esquema instalado está
desactualizado (ya no contiene la clave nueva). `make install` lo recompila
para que eso no ocurra.

## Compilar y ejecutar

```sh
cargo build --release
cargo run
```

En desarrollo puedes apuntar al esquema local en vez de instalarlo:

```sh
glib-compile-schemas data/
GSETTINGS_SCHEMA_DIR=$PWD/data cargo run
```

Si el esquema no está disponible, la aplicación arranca igual con los valores
por defecto (no guarda preferencias) y avisa por stderr.

## Flatpak

El manifiesto que compila Flathub vive en
[flathub/io.github.gnacho.Scribe](https://github.com/flathub/io.github.gnacho.Scribe),
y ahí se actualiza cada versión: ese repositorio es la fuente de verdad y este no
guarda ninguna copia.

[build-aux/flatpak](build-aux/flatpak) es un manifiesto aparte que compila el
directorio de trabajo contra el runtime GNOME 51, que es lo que quieres mientras
trabajas en la aplicación. Su `cargo-sources.json` se genera con
[flatpak-cargo-generator](https://github.com/flatpak/flatpak-builder-tools/tree/master/cargo)
a partir del `Cargo.lock` commiteado:

```sh
cargo generate-lockfile
python3 flatpak-cargo-generator.py Cargo.lock -o build-aux/flatpak/cargo-sources.json
cd build-aux/flatpak
flatpak-builder --user --install build-dir io.github.gnacho.Scribe.yml --force-clean
```

## Desarrollo

```sh
git clone https://github.com/gnacho/scribe.git
cd scribe
cargo build
cargo test
cargo run
```

## Licencia

AGPL-3.0-or-later. Ver [LICENSE](LICENSE).
