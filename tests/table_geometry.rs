//! Geometría de las tablas, medida en la vista real: el contenido de cada
//! celda tiene que caer en la columna que marcan los guiones de la fila
//! delimitadora, sea cual sea el ancho del contenido de cada fila.
//!
//! Necesita display: `xvfb-run -a cargo test --test table_geometry -- --ignored
//! --nocapture`. La regla del parser (`cell_text_ranges`, `TablePlan::pads`) se
//! comprueba aparte en los tests unitarios de `markdown_render`; aquí se
//! comprueba que el render obedece esos rellenos, que es donde GTK tiene la
//! última palabra (aplica la mitad de un `letter-spacing` que empieza una
//! línea).

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

use gtk4::prelude::*;

/// Tabla escrita como la escribe un usuario (filas sin rellenar, la
/// delimitadora es la más ancha) precedida de un párrafo: el cursor se queda en
/// la línea 0, fuera de la tabla, así que no hay relleno revelado por el
/// cursor. Es el caso que el issue pide: la columna de cada celda sale de la
/// fila de guiones, no del ancho del contenido de cada fila.
const DOC: &str = "texto\n\n| Command | Descripcion larga |\n| ---------- | ----------------- |\n| git status | Todo el arbol |\n| ls | Corto |\n";

fn pump() {
    let ctx = gtk4::glib::MainContext::default();
    for _ in 0..120 {
        while ctx.iteration(false) {}
    }
}

/// Inicio (byte, dentro de la línea) del contenido de cada celda de una fila,
/// entendida como lo hace el parser: segmentos entre pipes, recortados.
fn celdas(line: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut prev = 0usize;
    for cut in line
        .char_indices()
        .filter(|&(_, c)| c == '|')
        .map(|(i, _)| i)
        .chain(std::iter::once(line.len()))
    {
        let segment = &line[prev..cut];
        if !segment.trim().is_empty() {
            out.push(prev + (segment.len() - segment.trim_start().len()));
        }
        prev = cut + 1;
    }
    out
}

#[test]
#[ignore = "requiere display"]
fn el_contenido_de_cada_celda_cae_en_la_columna_de_los_guiones() {
    gtk4::init().unwrap();
    libadwaita::init().unwrap();
    let ed = editor::Editor::new();
    let win = gtk4::Window::builder()
        .default_width(760)
        .default_height(600)
        .build();
    win.set_child(Some(&ed.widget));
    win.present();
    ed.set_text(DOC);
    pump();

    let buf = ed.view.buffer();
    let advance = ed.view.cell_advance();
    let lineas: Vec<&str> = DOC.lines().collect();
    // Fila delimitadora y filas de contenido (la primera fila es la cabecera).
    let delim = 3;
    assert!(lineas[delim].starts_with("| -"), "doc de prueba cambiado");
    // Columna canónica de cada celda: dónde empieza el texto de la celda en la
    // fila de guiones, en celdas de carácter desde su inicio.
    let columnas = celdas(lineas[delim]);

    let origen = {
        let it = buf.iter_at_line(delim as i32).unwrap();
        ed.view.iter_location(&it).x() as f32
    };
    eprintln!("[GEO] avance={advance:.3} origen={origen:.1} columnas={columnas:?}");

    let mut medidas = 0;
    for (fila, texto) in lineas.iter().enumerate().skip(2) {
        if fila == delim {
            continue;
        }
        let inicio = buf.iter_at_line(fila as i32).unwrap().offset();
        for (k, byte) in celdas(texto).into_iter().enumerate() {
            let esperado = origen + columnas[k] as f32 * advance;
            let it = buf.iter_at_offset(inicio + byte as i32);
            let medido = ed.view.iter_location(&it).x() as f32;
            eprintln!(
                "[GEO] linea {fila} celda {k}: esperado {esperado:.1}, medido {medido:.1}, \
                 desvio {:+.1}",
                medido - esperado
            );
            assert!(
                (medido - esperado).abs() <= 1.0,
                "linea {fila} celda {k}: medido {medido:.1} frente a {esperado:.1}"
            );
            medidas += 1;
        }
    }
    assert!(medidas >= 6, "se esperaban al menos 6 celdas medidas");
}
