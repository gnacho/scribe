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

- **Render en vivo (WYSIWYG en linea)**: se decora el buffer del editor con
  `GtkTextTag`. Las marcas de Markdown llevan la propiedad `invisible` y se revelan
  atenuadas en la linea del cursor. Cubre cabeceras, negrita, cursiva, tachado,
  codigo en linea y en bloque, citas, listas con sangria colgante, listas anidadas,
  tareas, enlaces y reglas.
- **Tipografia**: columna centrada de 720 px, cuerpo en Cantarell, monoespaciada solo
  para codigo. Tema claro y oscuro con el *style scheme* de GtkSourceView.
- **Previsualizacion dividida** (opcional, Ctrl+Shift+P): render completo en un panel
  aparte, util para tablas. Desactivada por defecto.
- **Ficheros**: abrir, guardar y guardar como con `GtkFileDialog`. Escritura atomica
  (temporal + rename). Aviso al cerrar si hay cambios sin guardar.
- **Integracion**: `scribe fichero.md` y "Abrir con" del gestor de archivos funcionan.
- **Barra lateral**: recientes reales (persistidos en GSettings) con filtro, e indice
  del documento navegable.
- **Preferencias**: tamano de fuente, interlineado y autoguardado, persistidos en GSettings.
- **Formato**: Ctrl+B, Ctrl+I y Ctrl+K envuelven la seleccion.
- **Barra de estado** al estilo de GNOME Text Editor: palabras, caracteres y Ln/Col.

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

- Modo foco y modo maquina de escribir.
- Exportar a HTML o PDF.
- Tabla de estilos configurable.

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
