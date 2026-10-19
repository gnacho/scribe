//! Presupuestos de rendimiento (issue #21) para abrir y decorar documentos.
//!
//! Mide el camino real de apertura: cargar el texto en el `Editor` (set_text,
//! que además ejecuta un `analyze` síncrono para detectar vallas sin cerrar) y
//! drenar la pasada de decoración diferida (análisis + tags + adornos) junto
//! con el layout del primer frame. El presupuesto solo se comprueba en release
//! (`!cfg!(debug_assertions)`); en debug el test siempre pasa e imprime los
//! números para compararlos entre versiones.
//!
//! Ejecutar: xvfb-run -a cargo test --release --test perf -- --ignored --nocapture

// Los módulos de la app se incluyen vía #[path] y el harness solo ejercita
// parte de su API: el resto aparece como código muerto aunque la app sí lo
// use (mismo criterio que tests/crash_stress.rs).
#![allow(dead_code)]

#[path = "../src/tema.rs"]
mod tema;

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

/// Presupuestos en milisegundos, fijados como mediana x 3 sobre 3 iteraciones
/// medidas en release bajo xvfb.
///
/// Máquina: ryzen-ai (AMD Ryzen AI 9 HX 370 w/ Radeon 890M), 2026-10-07.
/// Mediana medida (apertura / decorado / total, en ms):
///   fixture   0.91 /   9.53 /   10.37  (13.634 bytes)
///   completo 62.74 / 125.08 /  184.62  (80.674 bytes)
///   grande   61.03 / 990.52 / 1058.81  (1.049.664 bytes)
///
/// `apertura` = set_text síncrono (inserción + `analyze` para vallas);
/// `decorado` = pasada de decoración diferida + layout del primer frame
/// (en `grande`, que supera `MAX_LIVE_BYTES`, la decoración se omite y el
/// tiempo es todo layout de pango para calcular el alto del scroll);
/// `total` = suma de ambas. Si un presupuesto se dispara, re-medir y revisar
/// regresiones en `analyze`/`decorate`.
const BUDGETS: [(&str, f64, f64, f64); 3] = [
    ("fixture", 5.0, 30.0, 40.0),
    ("completo", 200.0, 400.0, 600.0),
    ("grande", 200.0, 3000.0, 3200.0),
];

fn presupuesto(nombre: &str) -> (f64, f64, f64) {
    BUDGETS
        .iter()
        .find(|(n, _, _, _)| *n == nombre)
        .map(|(_, a, d, t)| (*a, *d, *t))
        .expect("presupuesto definido para el caso")
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn mediana(mut v: Vec<Duration>) -> Duration {
    v.sort();
    v[v.len() / 2]
}

/// Drena el main context hasta dejarlo vacío: dispara el timeout de decoración
/// (0 ms) y las idles de validación/layout del primer frame. Devuelve el tiempo
/// empleado. El tope de 60 s es solo un guarda contra un bucle infinito.
fn drenar(ctx: &gtk4::glib::MainContext) -> Duration {
    let t = Instant::now();
    let mut guard = 0u32;
    while ctx.iteration(false) {
        guard += 1;
        if guard > 1_000_000 || t.elapsed() > Duration::from_secs(60) {
            break;
        }
    }
    t.elapsed()
}

/// El fixture mayor de `src/testdata/`: un documento de muestra exhaustivo.
fn cargar_fixture() -> String {
    let ruta = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/testdata/usuario-vallas-sin-cerrar.md");
    std::fs::read_to_string(&ruta).expect("leer el fixture mayor de testdata")
}

/// Documento exhaustivo generado en memoria que cubre todas las familias del
/// marcado que decora Scribe (cabeceras ATX y setext, énfasis, código inline,
/// enlaces, imágenes, citas anidadas, listas y casillas, tablas alineadas,
/// vallas, reglas, pies de nota y HTML en bruto). Sustituye a
/// `Full-Markdown.md`, que no existe en el repo; se repite hasta ~80 KB para
/// que la decoración completa entre en juego sobre un documento sustancial.
fn documento_completo() -> String {
    let bloque = |n: usize| -> String {
        format!(
            r#"
# Sección {n}

Párrafo con *cursiva*, **negrita**, ***ambas***, ~~tachado~~ y `código` inline.

Un enlace [inline](https://example.com) y otro auto <https://example.com>, más
una referencia [ref][1]. Imagen en bloque:

![texto alt](imagen-{n}.png)

> Cita de primer nivel con **negrita**.
>
> > Cita anidada con `código`.

1. Elemento ordenado uno
2. Elemento ordenado dos
   - anidado desordenado
   - otro anidado
3. Elemento ordenado tres

- [x] tarea hecha con **negrita**
- [ ] tarea pendiente con `código`
- [ ] otra tarea

| Izquierda | Centro | Derecha |
|:----------|:------:|--------:|
| `a`       | **b**  | c       |
| _d_       | e      | ~~f~~   |

```python
def saludar():
    return "hola {n}"  # comentario
```

```bash
echo "sección {n}"
```

Regla:

---

Pie de nota[^{n}] con su definición.

[^{n}]: texto del pie número {n}.

<div class="nota">HTML en bruto para la sección {n}.</div>

[1]: https://example.com/ref
"#
        )
    };

    let mut s = String::with_capacity(80_000);
    let mut n = 0usize;
    while s.len() < 80_000 {
        s.push_str(&bloque(n));
        n += 1;
    }
    s
}

/// Documento grande (~1 MiB) generado en memoria: secciones, tablas anchas,
/// bloques de código, citas y listas, con variación para que el parser no
/// ataje patrones repetidos. Supera `MAX_LIVE_BYTES` (400 KB), así que la
/// decoración en vivo se desactiva y el coste se concentra en la apertura
/// (inserción en el buffer + `analyze` síncrono para detectar vallas).
fn documento_grande() -> String {
    const META: usize = 1_048_576; // 1 MiB
    let words = [
        "lorem",
        "ipsum",
        "dolor",
        "sit",
        "amet",
        "consectetur",
        "adipiscing",
        "elit",
        "sed",
        "do",
        "eiusmod",
        "tempor",
    ];
    let mut s = String::with_capacity(META + 64_000);
    let mut section = 0usize;
    while s.len() < META {
        s.push_str(&format!("\n## Sección {section}\n\n"));
        s.push_str("| Col A | Col B | Col C | Col D |\n|---|---|---|---|\n");
        for r in 0..20 {
            let w = |i: usize| words[(section * 7 + r * 3 + i) % words.len()];
            s.push_str(&format!(
                "| `{}` | **{}** | _{}_ | {}<br>{} |\n",
                w(0),
                w(1),
                w(2),
                w(3),
                w(4)
            ));
        }
        s.push_str("\n> cita con **marcado** y `código` inline\n\n");
        s.push_str("```python\ndef f():\n    return 1  # comentario\n```\n\n");
        for i in 0..8 {
            let mark = if i % 3 == 0 { "[x]" } else { "[ ]" };
            s.push_str(&format!("- {mark} elemento **{i}** con `código`\n"));
        }
        s.push_str("\n---\n\n");
        section += 1;
    }
    s
}

fn casos() -> Vec<(&'static str, String)> {
    vec![
        ("fixture", cargar_fixture()),
        ("completo", documento_completo()),
        ("grande", documento_grande()),
    ]
}

#[test]
#[ignore = "requiere display y release: xvfb-run -a cargo test --release --test perf -- --ignored --nocapture"]
fn presupuestos_apertura_y_decorado() {
    gtk4::init().expect("GTK necesita un display; ejecuta bajo xvfb-run");
    libadwaita::init().expect("libadwaita init");

    let editor = Editor::new();
    let win = gtk4::Window::builder()
        .default_width(900)
        .default_height(700)
        .build();
    win.set_child(Some(&editor.widget));
    win.present();

    let ctx = gtk4::glib::MainContext::default();
    drenar(&ctx); // asienta el layout inicial de la ventana antes de medir

    for (nombre, doc) in casos() {
        let mut aperturas = Vec::with_capacity(3);
        let mut decorados = Vec::with_capacity(3);
        let mut totales = Vec::with_capacity(3);

        for _ in 0..3 {
            let t_total = Instant::now();
            let t_apertura = Instant::now();
            editor.set_text(&doc);
            let apertura = t_apertura.elapsed();
            let decorado = drenar(&ctx);
            let total = t_total.elapsed();

            aperturas.push(apertura);
            decorados.push(decorado);
            totales.push(total);
        }

        let apertura = mediana(aperturas);
        let decorado = mediana(decorados);
        let total = mediana(totales);

        println!(
            "{nombre}: apertura {:.2} ms, decorado {:.2} ms, total {:.2} ms ({} bytes)",
            ms(apertura),
            ms(decorado),
            ms(total),
            doc.len()
        );

        // Presupuesto SOLO en release: en debug se imprime y siempre pasa.
        if !cfg!(debug_assertions) {
            let (pa, pd, pt) = presupuesto(nombre);
            assert!(
                ms(apertura) <= pa,
                "{nombre}: apertura {:.2} ms supera el presupuesto de {pa:.2} ms",
                ms(apertura)
            );
            assert!(
                ms(decorado) <= pd,
                "{nombre}: decorado {:.2} ms supera el presupuesto de {pd:.2} ms",
                ms(decorado)
            );
            assert!(
                ms(total) <= pt,
                "{nombre}: total {:.2} ms supera el presupuesto de {pt:.2} ms",
                ms(total)
            );
        }
    }
}
