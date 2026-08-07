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

- **Render en vivo (WYSIWYG en linea)**. El buffer del editor se decora con
  `GtkTextTag` y lo que un tag no puede expresar se **dibuja**: vinetas segun el
  nivel de anidamiento, casillas de tarea marcables, reglas horizontales, la
  barra vertical de las citas y la caja redondeada de los bloques de codigo.
- **Visibilidad del marcado configurable**: ocultar siempre, revelar solo en la
  linea del cursor, o atenuar siempre (que ensena el fuente tal cual y desactiva
  los adornos, para cuando quieres ver lo que hay de verdad).
- **Cobertura**: cabeceras ATX y setext, negrita, cursiva, tachado, codigo en
  linea y en bloque, citas, listas ordenadas y sin ordenar con anidamiento,
  tareas, enlaces, autoenlaces, imagenes, notas al pie, tablas, HTML y reglas.
- **Tipografia**: columna centrada de ancho configurable, familia sans/serif/mono
  a elegir y monoespaciada reservada para codigo y tablas. Tema claro y oscuro
  con el style scheme de GtkSourceView.
- **Modo foco y maquina de escribir**: atenuar todo salvo el parrafo actual y
  mantener la linea del cursor centrada verticalmente.
- **Plantillas**: ficheros `.md` en `~/.local/share/scribe/templates`, con
  marcadores `{{title}}`, `{{date}}`, `{{time}}`, `{{datetime}}` y `{{year}}`.
- **Interfaz segun las HIG**, con el mismo reparto que GNOME Text Editor: boton
  «Abrir» con desplegable de recientes y boton de documento nuevo a la izquierda,
  titulo al centro, menu principal con fila de zoom a la derecha, y barra de
  estado inferior con posicion del cursor e «Ir a la linea».
- **Preferencias** en tres paginas: apariencia, editor y plantillas.
- **Ficheros**: abrir, guardar y guardar como con `GtkFileDialog`, escritura
  atomica, aviso de cambios sin guardar y autoguardado con intervalo ajustable.
- **Integracion**: `scribe fichero.md` y «Abrir con» del gestor de archivos.
- **Barra lateral** (F9) con recientes filtrables e indice del documento navegable.

## Como funciona el render

Dos modulos, ninguno con tipos de la aplicacion dentro:

- **`src/markdown_render.rs`** analiza el Markdown con pulldown-cmark y devuelve
  dos listas: *tramos* (rangos en bytes con el nombre del `GtkTextTag`) y
  *adornos* (elementos referidos a numero de linea que hay que pintar). No
  depende de GTK, asi que se prueba sin display: 22 tests unitarios.
- **`src/markdown_view.rs`** es un `GtkSourceView` subclaseado que implementa
  `snapshot_layer`, el vfunc que GTK expone precisamente para pintar debajo o
  encima del texto. Trabaja en coordenadas de buffer, asi que el desplazamiento
  lo resuelve GTK y aqui no hay que compensar nada. Recibe la lista de adornos y
  una paleta de colores, y dibuja con `gsk::PathBuilder`.

Las marcas que un adorno sustituye (`- `, `[x] `, `---`, `>`, las vallas de los
bloques) se ocultan **siempre**, no solo fuera de la linea del cursor: revelarlas
moveria el texto de sitio cada vez que el cursor cambia de linea.

## Reutilizar el render en otro proyecto

Los dos modulos de arriba forman una pieza autocontenida y estan escritos para
poder extraerse tal cual a un crate. Todavia no se publican: la API de
`Ornament` y `OrnamentPalette` seguira moviendose mientras se anadan elementos, y
versionar en crates.io algo que va a cambiar cada semana es una carga sin
beneficio hasta que aparezca un segundo consumidor. Si quieres usarlo, copia los
dos ficheros; si acabas manteniendolo, hablamos de sacarlo a un crate del
workspace.

## Limites conocidos

- **Las imagenes no se incrustan**: se ve el texto alternativo. `snapshot_layer`
  puede pintar, pero no reservar altura de linea. Incrustarlas de verdad exige
  `insert_paintable`, que mete un caracter en el buffer y ensucia el fuente.
- **Las tablas cuadran solo si el fuente esta alineado**: el bloque va en
  monoespaciada, que hace que los pipes coincidan, pero no se reformatean solas.
- En modo «atenuar siempre» los adornos se desactivan a proposito, para no
  duplicar la informacion con las marcas ya visibles.


## Que falta

- Pestanas / varios documentos por ventana (`AdwTabView`).
- Buscar y reemplazar.
- Alineado automatico de tablas.
- Exportar a HTML o PDF.


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
