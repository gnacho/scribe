//! Captura visual del editor: renderiza un documento de muestra a PNG para
//! verificar la vista enriquecida (tablas, cajas de código, filetes de
//! título, citas, listas, imágenes) sin abrir ventanas interactivas.
//!
//! Ejecutar: xvfb-run -a cargo test --test render_shot -- --ignored --nocapture
//! Salida:   /mnt/agents/work/shots/*.png

// Los módulos de la app se incluyen vía #[path] y el harness solo ejercita
// parte de su API: el resto aparece como código muerto aunque la app sí lo
// use (mismo criterio que tests/crash_stress.rs).
#![allow(dead_code)]

#[path = "../src/editor.rs"]
mod editor;
#[path = "../src/markdown_render.rs"]
mod markdown_render;
#[path = "../src/markdown_view.rs"]
mod markdown_view;
#[path = "../src/settings.rs"]
mod settings;

use editor::Editor;
use gtk4::prelude::*;
use std::time::{Duration, Instant};

const DOC: &str = "\
# Título principal

## Sección con *énfasis* y `código`

Texto normal con **negrita** y un [enlace](https://ejemplo.com).

| Command    | Description                      | Precio |
| ---------- | -------------------------------- | -----: |
| git status | List all **new** or modified     |    $12 |
| git diff   | Show file `differences`          |  $1600 |

> Una cita con **marcado**
> y dos líneas.

- [x] Tarea hecha
- [ ] Tarea pendiente
- Elemento normal

---

```python
def f():
    return 1  # comentario
```

![gatito](test.png)
";

/// PNG de 64x48 generado con un codificador real (bicolor rojo/azul), para
/// probar el pintado de imágenes locales.
const TEST_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 64, 0, 0, 0, 48, 8, 2,
    0, 0, 0, 46, 41, 235, 72, 0, 0, 0, 91, 73, 68, 65, 84, 120, 156, 237, 209, 65, 13, 192, 48, 16,
    3, 193, 166, 184, 14, 73, 17, 7, 86, 65, 228, 49, 138, 180, 131, 192, 43, 175, 249, 246, 115,
    179, 87, 15, 56, 85, 128, 86, 128, 86, 128, 86, 128, 86, 128, 86, 128, 86, 128, 86, 128, 86,
    128, 86, 128, 86, 128, 182, 246, 140, 222, 112, 228, 250, 7, 10, 208, 10, 208, 10, 208, 10,
    208, 10, 208, 10, 208, 10, 208, 10, 208, 10, 208, 10, 208, 10, 208, 126, 242, 14, 2, 253, 217,
    136, 163, 109, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

fn pump(ctx: &gtk4::glib::MainContext, ms: u64) {
    let t = Instant::now();
    while t.elapsed() < Duration::from_millis(ms) {
        while ctx.iteration(false) {}
        std::thread::sleep(Duration::from_millis(4));
    }
}

/// Renderiza el contenido de un widget a un PNG usando el renderer de la
/// ventana ya realizada.
fn shot(win: &gtk4::Window, path: &str) {
    let w = win.width() as f64;
    let h = win.height() as f64;
    let paintable = gtk4::WidgetPaintable::new(Some(win));
    let snapshot = gtk4::Snapshot::new();
    paintable.snapshot(&snapshot, w, h);
    let node = snapshot.to_node().expect("snapshot sin nodo raíz");
    let renderer = win
        .native()
        .expect("sin native")
        .renderer()
        .expect("sin renderer");
    let texture = renderer.render_texture(&node, None);
    texture.save_to_png(path).expect("guardar png");
}

#[test]
#[ignore = "requiere display: xvfb-run -a cargo test --test render_shot -- --ignored --nocapture"]
fn captura_vista_enriquecida() {
    gtk4::init().expect("GTK necesita un display; ejecuta bajo xvfb-run");
    libadwaita::init().expect("libadwaita init");

    let out = std::path::Path::new("/mnt/agents/work/shots");
    std::fs::create_dir_all(out).expect("crear dir de salida");
    std::fs::write(out.join("test.png"), TEST_PNG).expect("escribir imagen de prueba");

    let editor = Editor::new();
    editor.set_base_dir(Some(out.to_path_buf()));
    let win = gtk4::Window::builder()
        .default_width(760)
        .default_height(1000)
        .build();
    win.set_child(Some(&editor.widget));
    win.present();

    let ctx = gtk4::glib::MainContext::default();
    editor.set_text(DOC);
    pump(&ctx, 600); // drena el debounce de decoración y el layout

    shot(&win, &out.join("vista.png").display().to_string());
    eprintln!("captura escrita en {}", out.join("vista.png").display());
}
