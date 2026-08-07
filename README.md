# Scribe

Editor Markdown nativo para GNOME, escrito en Rust con GTK4, libadwaita y GtkSourceView 5.

**Estado: pre-alpha.** Edicion Markdown con render en vivo sobre el propio texto:
las cabeceras se ven con su tamano, la negrita en negrita y los `**`, `#`, backticks
y URLs se ocultan, reapareciendo solo en la linea donde esta el cursor. Sin WebKit
ni segundo documento que sincronizar.

Este repositorio continua la exploracion de diseno de las maquetas de Gnome-MD
(ahora en [docs/mockups](docs/mockups)): un editor Markdown sin distracciones que
sigue las GNOME Human Interface Guidelines.

## Que hace ahora mismo

- **Render en vivo (WYSIWYG en linea)**: el buffer del editor se decora con
  `GtkTextTag`. Las marcas de Markdown llevan la propiedad `invisible` y su
  visibilidad es configurable: ocultas siempre, reveladas en la linea del cursor,
  o atenuadas siempre. Cubre cabeceras ATX y setext, negrita, cursiva, tachado,
  codigo en linea y en bloque, citas, listas con sangria colgante, listas
  anidadas, tareas, enlaces, autoenlaces, notas al pie, tablas, HTML y reglas.
- **Tipografia**: columna centrada de ancho configurable, familia sans/serif/mono
  a elegir y monoespaciada reservada para codigo y tablas. Tema claro y oscuro
  con el style scheme de GtkSourceView.
- **Modo foco y maquina de escribir**: atenuar todo salvo el parrafo actual y
  mantener la linea del cursor centrada verticalmente.
- **Plantillas**: ficheros `.md` en `~/.local/share/scribe/templates`, con
  marcadores `{{title}}`, `{{date}}`, `{{time}}`, `{{datetime}}` y `{{year}}`.
  Se eligen desde el boton de documento nuevo o se fija una por defecto.
- **Interfaz segun las HIG**, con el mismo reparto que GNOME Text Editor: boton
  «Abrir» con desplegable de recientes y boton de documento nuevo a la izquierda,
  titulo al centro, menu principal con fila de zoom a la derecha, y barra de
  estado inferior con posicion del cursor e «Ir a la linea».
- **Preferencias** en tres paginas: apariencia, editor y plantillas.
- **Ficheros**: abrir, guardar y guardar como con `GtkFileDialog`, escritura
  atomica, aviso de cambios sin guardar y autoguardado con intervalo ajustable.
- **Integracion**: `scribe fichero.md` y «Abrir con» del gestor de archivos.
- **Barra lateral** (F9) con recientes filtrables e indice del documento navegable.
- **Previsualizacion dividida** (opcional, Ctrl+Shift+P) para tablas e imagenes.

## Limites conocidos del render en vivo

`GtkTextTag` puede cambiar como se ve un texto, pero no sustituirlo por otro ni
dibujar encima. De ahi que:

- Las vinetas siguen siendo `-` o `*`, coloreados y con sangria colgante, en vez de `-`.
- Las casillas se ven como `[x]` y `[ ]` estilizados, no como checkboxes.
- Las reglas `---` se atenuan y centran, pero no son una linea real.
- Las tablas no se alinean en columnas; para eso esta la vista dividida.
- Las imagenes no se incrustan.

Todo eso necesitaria dibujar en un `snapshot()` propio o incrustar widgets con
anclas en el buffer, que altera el texto fuente. Queda fuera de esta fase.

## Que falta

- Pestanas / varios documentos por ventana (`AdwTabView`).
- Buscar y reemplazar.
- Exportar a HTML o PDF.
- Correccion ortografica.

## Requisitos de compilacion

- Rust 1.78+
- GTK4, libadwaita, GtkSourceView 5 y GLib development files

Arch/CachyOS:

```bash
sudo pacman -S rust gtk4 libadwaita gtksourceview5 glib2
```

Debian/Ubuntu:

```bash
sudo apt install libgtk-4-dev libadwaita-1-dev libgtksourceview-5-dev libglib2.0-dev
```

Fedora:

```bash
sudo dnf install gtk4-devel libadwaita-devel gtksourceview5-devel glib2-devel
```

## Build and run

```bash
cargo build --release
cargo run
```

## Flatpak

A Flatpak manifest is available at [build-aux/flatpak](build-aux/flatpak/) (work in progress):

```bash
cd build-aux/flatpak
flatpak-builder --user --install build-dir app.scribe.Scribe.json --force-clean
```

## License

AGPL-3.0. See [LICENSE](LICENSE).
