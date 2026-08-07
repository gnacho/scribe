# Scribe

<p align="center">
  <a href="README.es.md">Español</a> |
  <a href="README.md">English</a>
</p>

<p align="center">
  <a href="https://github.com/gnacho/scribe/releases"><img alt="Release" src="https://img.shields.io/github/v/release/gnacho/scribe"></a>
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/github/license/gnacho/scribe"></a>
</p>

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/hero-es-dark.png">
    <source media="(prefers-color-scheme: light)" srcset="assets/hero-es-light.png">
    <img alt="Ventana del editor Scribe mostrando texto Markdown con render WYSIWYG en vivo" src="assets/hero-es-light.png" width="800">
  </picture>
</p>

Scribe es un editor Markdown nativo para GNOME, escrito en Rust con GTK4 y
libadwaita. El Markdown se renderiza en vivo sobre el propio buffer de
edición: las cabeceras se ven con su tamaño, la negrita en negrita y las
marcas de sintaxis se ocultan, sin cambiar a una previsualización aparte ni
usar un motor de navegador.

## Por que existe esto

Quería un editor Markdown que se sintiera parte de GNOME, no un porte de una
aplicación web. Los que probé o bien metían un motor de navegador
(ProseMirror, Milkdown dentro de WebKit) y consumían cientos de megabytes, o
mostraban el código fuente a un lado y la previsualización al otro. Acababa
con los dos paneles abiertos, yendo y viniendo entre ellos en vez de
escribir.

La idea es un editor donde el texto fuente ES la previsualización: el buffer
sigue siendo Markdown sin más, pero el renderizado se pinta encima con
`GtkTextTag`. Así no hay nada que sincronizar, no hay HTML por debajo ni que
empaquetar un motor de navegador. Empezó como maquetas para explorar el
diseño con GTK4 y ha terminado siendo algo que uso de verdad para borradores
y notas.

## Por que este stack

- **Rust + GTK4 + libadwaita** &mdash; toolkit nativo, sin Electron. El
  binario ocupa unos 8 MB y en reposo consume unas docenas de megabytes de
  RAM. Un editor basado en WebKit pesaría diez veces eso antes de abrir un
  archivo.
- **GtkSourceView 5** para el buffer de texto y **pulldown-cmark** para
  parsear Markdown. Luego se aplican tramos con `GtkTextTag` para el render.
  Sin HTML ni CSS en la previsualización. El editor es un único
  `GtkTextView` con decoraciones sobre el buffer.
- **Sin base de datos, sin servidor, sin JavaScript.** Es una aplicación de
  escritorio que abre, edita y guarda archivos. Las preferencias van por
  GSettings, no en un archivo de configuración ni en una interfaz web.

## Características

- **Render WYSIWYG en vivo** sobre el buffer de edición: cabeceras a escala
  real, negrita/cursiva/tachado, código en línea y en bloque, citas, listas
  anidadas con sangría colgante, tareas, enlaces, imágenes, notas al pie,
  reglas, tablas en monoespaciada y bloques HTML atenuados
- **Visibilidad del marcado configurable**: ocultar las marcas (`**`, `#`,
  backticks) siempre, revelarlas en la línea del cursor o mantenerlas
  atenuadas en todo momento
- **Modo foco** (Ctrl+Shift+F) atenúa todo salvo el párrafo actual
- **Máquina de escribir** (Ctrl+Shift+T) mantiene el cursor centrado
  verticalmente
- **Plantillas**: archivos `.md` en `~/.local/share/scribe/templates` con
  marcadores `{{title}}`, `{{date}}`, `{{time}}`, `{{datetime}}` y
  `{{year}}`. Cuatro ejemplos se crean la primera vez
- **Vista dividida** (Ctrl+Shift+P) con render completo en un panel lateral
  usando el mismo motor de `GtkTextTag`. Útil para tablas e imágenes
- **Ctrl+B/I/K** envuelve la selección en `**`, `*` o backticks
- **Continuación de listas**: al pulsar Intro se crea el siguiente elemento y
  se renumeran las listas ordenadas. Dejar un elemento vacío cierra la lista
- **Zoom** (Ctrl +/&minus;/0) con controles en el menú principal
- **Ir a la línea** desde la barra de estado
- **Cabecera** al estilo de GNOME Text Editor: botón de abrir con
  desplegable de recientes, botón de documento nuevo, título centrado y menú
  principal con fila de zoom a la derecha
- **Preferencias** en tres páginas: Apariencia, Editor y Plantillas
- **Archivos**: abrir, guardar y guardar como con `GtkFileDialog`, escritura
  atómica (temporal + rename), aviso de cambios sin guardar y autoguardado
  configurable
- **Barra lateral** (F9) con documentos recientes filtrables e índice del
  documento navegable
- **Tema claro y oscuro** que sigue el style scheme de GtkSourceView
- **Integración**: `scribe fichero.md` y "Abrir con" del gestor de archivos

## Límites conocidos del render en vivo

`GtkTextTag` cambia cómo se ve el texto pero no puede sustituirlo ni dibujar
encima, así que algunas cosas se ven como texto estilizado en vez de widgets
reales:

- Las viñetas se quedan como `-` o `*`, coloreadas y con sangría colgante,
  en vez de `&bull;`
- Las tareas muestran `[x]` y `[ ]` estilizados, no checkboxes
- Las reglas (`---`) se atenúan y centran, no son una línea dibujada
- Las tablas se renderizan en monoespaciada para que las columnas cuadren al
  alinear los pipes en el fuente; no son una rejilla real
- Las imágenes no se incrustan

Dibujar con `snapshot()` propio o empotrar widgets con anclas en el buffer
alteraría el texto fuente. Queda fuera de esta fase.

## Capturas

**Ventana principal con render Markdown en vivo**

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/hero-es-dark.png">
  <source media="(prefers-color-scheme: light)" srcset="assets/hero-es-light.png">
  <img alt="Ventana principal del editor con cabeceras, negrita, cursiva, código y una cita" src="assets/hero-es-light.png" width="800">
</picture>

**Menú principal con controles de zoom**

<p align="center">
  <img alt="Menú con las opciones de abrir, guardar, zoom y vista" src="assets/screenshot-menu-es-light.png" width="800">
</p>

**Ventana de preferencias**

<p align="center">
  <img alt="Ventana de preferencias con las páginas de Apariencia, Editor y Plantillas" src="assets/screenshot-preferences-es-light.png" width="800">
</p>

**Modo foco (todo atenuado salvo el párrafo actual)**

<p align="center">
  <img alt="Modo foco en el editor con el documento atenuado y el párrafo actual resaltado" src="assets/screenshot-focus-es-light.png" width="800">
</p>

## Qué falta

- Pestañas / varios documentos (`AdwTabView`)
- Buscar y reemplazar
- Exportar a HTML o PDF
- Corrección ortográfica
- Traducción de la interfaz al inglés y otros idiomas

## Requisitos de compilación

- Rust 1.80 o superior
- GTK4 (&ge; 4.14), libadwaita (&ge; 1.5), GtkSourceView 5 y GLib con sus
  ficheros de desarrollo

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

## Compilar y ejecutar

```sh
cargo build --release
cargo run
```

Para que las preferencias funcionen hace falta instalar el esquema
GSettings. En desarrollo:

```sh
glib-compile-schemas data/
GSETTINGS_SCHEMA_DIR=$PWD/data cargo run
```

Si el esquema no está disponible, la aplicación arranca igual con los
valores por defecto y avisa por stderr.

## Flatpak

Hay un manifiesto en [build-aux/flatpak](build-aux/flatpak). En progreso. El
módulo compila con `cargo --offline`, así que primero hay que generar
`cargo-sources.json` con
[flatpak-cargo-generator](https://github.com/flatpak/flatpak-builder-tools/tree/master/cargo)
y tener `Cargo.lock` en el repositorio:

```sh
cargo generate-lockfile
python3 flatpak-cargo-generator.py Cargo.lock -o build-aux/flatpak/cargo-sources.json
cd build-aux/flatpak
flatpak-builder --user --install build-dir app.scribe.Scribe.json --force-clean
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

AGPL-3.0. Ver [LICENSE](LICENSE).
