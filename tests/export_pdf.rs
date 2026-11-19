//! Exportación real a PDF: genera un fichero con la operación de impresión de
//! GTK y comprueba sus propiedades básicas (páginas, texto extraíble).
//!
//! Ejecutar: xvfb-run -a cargo test --test export_pdf -- --ignored --nocapture

// Los módulos de la app se incluyen vía #[path] y el harness solo ejercita
// parte de su API: el resto aparece como código muerto aunque la app sí lo
// use (mismo criterio que tests/render_shot.rs).
#![allow(dead_code)]

#[path = "../src/export_css.rs"]
mod export_css;
#[path = "../src/export_pdf.rs"]
mod export_pdf;
#[path = "../src/export_themes.rs"]
mod export_themes;
#[path = "../src/markdown_render.rs"]
mod markdown_render;
#[path = "../src/settings.rs"]
mod settings;

/// Documento de muestra con cabeceras, código, tabla, cita, lista y un salto
/// de página explícito: suficiente para varias páginas.
const DOC: &str = "\
# Heading Level One

This is the first paragraph of the sample document. It is long enough that,
when printed at eleven points on A4 paper, it wraps onto several visual lines.

## Heading Level Two

Some **bold text** and *italic text* and `inline code` and a [link](https://example.com).

- First list item
- Second list item
- Third list item

1. Ordered one
2. Ordered two

> A block quote that spans
> more than one line.

```rust
fn main() {
    println!(\"Hello from a code block\");
}
```

| Command | Description |
| ------- | ----------- |
| git init | Create a repo |
| git add | Stage changes |

| Feature | Status | Notes |
| :------ | :----: | ----: |
| **Real columns** | done | widths grow with content |
| Escaped pipe \\| in cell | done | the pipe must not split the cell |
| A very long cell whose text does not fit its column and therefore wraps onto several lines inside the cell | done | wrapped text stays inside its own column |

\\newpage

## Page Break Section

This text lands on a new page because of the explicit `\\newpage` above.
";

/// Vuelca a PNG la primera página del PDF. `None` si no hay poppler.
fn raster(pdf: &std::path::Path, prefijo: &std::path::Path) -> Option<Vec<u8>> {
    let salida = std::process::Command::new("pdftoppm")
        .arg("-png")
        .arg("-r")
        .arg("60")
        .arg("-f")
        .arg("1")
        .arg("-l")
        .arg("1")
        .arg(pdf)
        .arg(prefijo)
        .output()
        .ok()?;
    if !salida.status.success() {
        return None;
    }
    std::fs::read(format!("{}-1.png", prefijo.display())).ok()
}

#[test]
#[ignore = "requiere display: xvfb-run -a cargo test --test export_pdf -- --ignored --nocapture"]
fn la_hoja_de_estilo_cambia_el_pdf_y_quitarla_lo_deja_igual() {
    gtk4::init().expect("GTK necesita un display; ejecuta bajo xvfb-run");
    libadwaita::init().expect("libadwaita init");

    let dir = std::env::temp_dir().join("scribe-css-prueba");
    std::fs::create_dir_all(&dir).expect("crear directorio de prueba");
    let css = dir.join("estilo.css");
    std::fs::write(
        &css,
        "body { font-size: 14pt; color: #003366; background-color: #fffdf5 }\n@page { margin: 48pt }\n",
    )
    .expect("escribir css");

    let base = export_pdf::ExportConfig {
        esquema: settings::ExportColorScheme::Light,
        familia_cuerpo: settings::FontFamily::Sans,
        tamano_cuerpo: 11.0,
        fuente_mono: settings::ExportMonoFont::AdwaitaMono,
        papel: settings::ExportPageSize::A4,
        orientacion: settings::ExportOrientation::Portrait,
        template: settings::ExportTemplate::Custom,
        theme: settings::ExportTheme::None,
        css_path: None,
        indice: false,
    };
    let con_hoja = export_pdf::ExportConfig {
        css_path: Some(css.clone()),
        ..base.clone()
    };

    let sin = dir.join("sin-hoja.pdf");
    let con = dir.join("con-hoja.pdf");
    let otra_vez = dir.join("otra-vez.pdf");
    export_pdf::exportar_a_fichero(&base, DOC, "Prueba", &sin, false).expect("exportar sin hoja");
    export_pdf::exportar_a_fichero(&con_hoja, DOC, "Prueba", &con, false)
        .expect("exportar con hoja");
    export_pdf::exportar_a_fichero(&base, DOC, "Prueba", &otra_vez, false).expect("reexportar");

    let Some(a) = raster(&sin, &dir.join("a")) else {
        eprintln!("pdftoppm no disponible: se omite la comparación visual");
        return;
    };
    let b = raster(&con, &dir.join("b")).expect("rasterizar el PDF con hoja");
    let c = raster(&otra_vez, &dir.join("c")).expect("rasterizar el PDF sin hoja");
    assert_eq!(a, c, "quitar la hoja debe dejar la página como estaba");
    assert_ne!(a, b, "la hoja de estilo debe cambiar la página");
    eprintln!("PDFs de prueba en {}", dir.display());
}

#[test]
#[ignore = "requiere GTK y pdftoppm; ejecutar con xvfb-run y --ignored"]
fn un_tema_cambia_el_pdf_y_quitarlo_lo_deja_igual() {
    gtk4::init().expect("GTK necesita un display; ejecuta bajo xvfb-run");
    libadwaita::init().expect("libadwaita init");

    let dir = std::env::temp_dir().join("scribe-tema-prueba");
    std::fs::create_dir_all(&dir).expect("crear directorio de prueba");

    let base = export_pdf::ExportConfig {
        esquema: settings::ExportColorScheme::Light,
        familia_cuerpo: settings::FontFamily::Sans,
        tamano_cuerpo: 11.0,
        fuente_mono: settings::ExportMonoFont::AdwaitaMono,
        papel: settings::ExportPageSize::A4,
        orientacion: settings::ExportOrientation::Portrait,
        template: settings::ExportTemplate::Article,
        theme: settings::ExportTheme::None,
        css_path: None,
        indice: false,
    };
    let con_tema = export_pdf::ExportConfig {
        theme: settings::ExportTheme::Sepia,
        ..base.clone()
    };

    let sin = dir.join("sin-tema.pdf");
    let con = dir.join("con-tema.pdf");
    let otra_vez = dir.join("otra-vez.pdf");
    export_pdf::exportar_a_fichero(&base, DOC, "Prueba", &sin, false).expect("exportar sin tema");
    export_pdf::exportar_a_fichero(&con_tema, DOC, "Prueba", &con, false)
        .expect("exportar con tema");
    export_pdf::exportar_a_fichero(&base, DOC, "Prueba", &otra_vez, false).expect("reexportar");

    let Some(a) = raster(&sin, &dir.join("a")) else {
        eprintln!("pdftoppm no disponible: se omite la comparación visual");
        return;
    };
    let b = raster(&con, &dir.join("b")).expect("rasterizar el PDF con tema");
    let c = raster(&otra_vez, &dir.join("c")).expect("rasterizar el PDF sin tema");
    assert_eq!(a, c, "quitar el tema debe dejar la página como estaba");
    assert_ne!(a, b, "el tema debe cambiar la página");
    eprintln!("PDFs de prueba en {}", dir.display());
}

#[test]
#[ignore = "requiere display: xvfb-run -a cargo test --test export_pdf -- --ignored --nocapture"]
fn exporta_un_pdf_real_de_prueba() {
    gtk4::init().expect("GTK necesita un display; ejecuta bajo xvfb-run");
    libadwaita::init().expect("libadwaita init");

    let ruta = std::env::temp_dir().join("export-prueba.pdf");
    // Configuración explícita: no depende del schema GSettings instalado.
    let config = export_pdf::ExportConfig {
        esquema: settings::ExportColorScheme::Light,
        familia_cuerpo: settings::FontFamily::Sans,
        tamano_cuerpo: 11.0,
        fuente_mono: settings::ExportMonoFont::AdwaitaMono,
        papel: settings::ExportPageSize::A4,
        orientacion: settings::ExportOrientation::Portrait,
        template: settings::ExportTemplate::Custom,
        theme: settings::ExportTheme::None,
        css_path: None,
        indice: false,
    };
    export_pdf::exportar_a_fichero(&config, DOC, "Prueba", &ruta, false).expect("exportar a PDF");

    assert!(ruta.exists(), "el PDF no se escribió en {}", ruta.display());
    assert!(
        std::fs::metadata(&ruta).map(|m| m.len()).unwrap_or(0) > 0,
        "el PDF está vacío"
    );
    eprintln!("PDF escrito en {}", ruta.display());

    // La tabla debe salir como tabla real: las filas del fuente no pueden
    // aparecer como texto plano con sus pipes de separación. Un `|` suelto
    // proveniente de `\|` escapado sí es válido. Best-effort: poppler-utils.
    if let Ok(salida) = std::process::Command::new("pdftotext")
        .arg(&ruta)
        .arg("-")
        .output()
    {
        if salida.status.success() {
            let texto = String::from_utf8_lossy(&salida.stdout);
            assert!(
                !texto.contains("Command |") && !texto.contains("| git"),
                "el PDF conserva las filas de la tabla como texto con pipes"
            );
            assert!(texto.contains("Status"), "falta la cabecera de la tabla");
            assert!(
                texto.contains("in cell"),
                "falta la celda con pipe escapado"
            );
        }
    }
    // Volcado de páginas a PNG para inspección visual manual (best-effort).
    let prefijo = std::env::temp_dir().join("export-prueba");
    let _ = std::process::Command::new("pdftoppm")
        .arg("-png")
        .arg("-r")
        .arg("110")
        .arg("-f")
        .arg("1")
        .arg("-l")
        .arg("3")
        .arg(&ruta)
        .arg(&prefijo)
        .output();
    eprintln!("PNGs de inspección: {}-N.png", prefijo.display());
}

// ---------------------------------------------------------------------------
// Índice con números de página reales (issue #195)
// ---------------------------------------------------------------------------

/// Cabeceras del documento de prueba, en el orden en que aparecen.
const TITULOS_INDICE: [&str; 6] = [
    "Overview",
    "Installation",
    "Requirements",
    "Usage",
    "Reference",
    "Limits",
];

/// Documento de varias páginas con las cabeceras de [`TITULOS_INDICE`] y
/// párrafos de relleno entre ellas. `BODYSTART` marca la primera página del
/// cuerpo para localizar dónde acaba el índice.
fn documento_con_indice() -> String {
    let relleno = "The quick brown fox jumps over the lazy dog while the printer \
                   paginates this paragraph at eleven points on A4 paper.\n\n";
    let mut doc = String::from("BODYSTART\n\n");
    for (i, titulo) in TITULOS_INDICE.iter().enumerate() {
        let nivel = "#".repeat(1 + i % 3);
        doc.push_str(&format!("{nivel} {titulo}\n\n"));
        for _ in 0..8 {
            doc.push_str(relleno);
        }
    }
    doc
}

/// Documento largo: 70 cabeceras no caben en una sola página de índice.
fn documento_con_indice_largo() -> (String, Vec<String>) {
    let mut doc = String::from("BODYSTART\n\n");
    let mut titulos = Vec::new();
    for i in 1..=70 {
        let titulo = format!("Section {i:02}");
        doc.push_str(&format!("# {titulo}\n\n"));
        doc.push_str("A short paragraph so each section takes a little vertical space.\n\n");
        titulos.push(titulo);
    }
    (doc, titulos)
}

/// Texto de cada página del PDF (pdftotext separa las páginas con `\f`).
/// `-layout` conserva la línea de cada entrada del índice junto a su número.
/// `None` si poppler-utils no está instalado.
fn paginas_de_texto(pdf: &std::path::Path) -> Option<Vec<String>> {
    let salida = std::process::Command::new("pdftotext")
        .arg("-layout")
        .arg(pdf)
        .arg("-")
        .output()
        .ok()?;
    if !salida.status.success() {
        return None;
    }
    let texto = String::from_utf8_lossy(&salida.stdout).to_string();
    let mut paginas: Vec<String> = texto.split('\u{c}').map(|p| p.trim().to_string()).collect();
    if paginas.last().map(|p| p.is_empty()).unwrap_or(false) {
        paginas.pop();
    }
    Some(paginas)
}

/// Número impreso al final de la línea de `titulo` en una página del índice.
fn numero_del_indice(pagina: &str, titulo: &str) -> Option<usize> {
    let lineas: Vec<&str> = pagina.lines().map(str::trim).collect();
    let i = lineas.iter().position(|linea| {
        linea
            .strip_prefix(titulo)
            .map(|resto| {
                let resto = resto.trim();
                resto.is_empty() || resto.parse::<usize>().is_ok()
            })
            .unwrap_or(false)
    })?;
    let resto = lineas[i][titulo.len()..].trim();
    if let Ok(n) = resto.parse::<usize>() {
        return Some(n);
    }
    // Si pdftotext parte el número a la línea siguiente.
    lineas.get(i + 1)?.parse::<usize>().ok()
}

/// Comprueba que cada título del índice lleva impresa la página donde de
/// verdad aparece su cabecera.
fn comprobar_numeros(paginas: &[String], n_indice: usize, titulos: &[String]) -> Vec<usize> {
    let mut numeros = Vec::new();
    for titulo in titulos {
        let en_cuerpo = paginas[n_indice..]
            .iter()
            .position(|p| p.contains(titulo.as_str()))
            .unwrap_or_else(|| panic!("el título {titulo} no aparece en el cuerpo del PDF"));
        let esperada = n_indice + en_cuerpo + 1;
        let impresa = paginas[..n_indice]
            .iter()
            .find_map(|p| numero_del_indice(p, titulo))
            .unwrap_or_else(|| panic!("el índice no lista {titulo}"));
        assert_eq!(
            impresa, esperada,
            "{titulo}: el índice dice {impresa} y la cabecera está en la {esperada}"
        );
        numeros.push(impresa);
    }
    assert!(
        titulos.len() >= 3,
        "el documento de prueba debe tener varias cabeceras"
    );
    numeros
}

#[test]
#[ignore = "requiere display y pdftotext: xvfb-run -a cargo test --test export_pdf -- --ignored --nocapture --test-threads=1"]
fn el_indice_numera_la_pagina_real_de_cada_titulo() {
    gtk4::init().expect("GTK necesita un display; ejecuta bajo xvfb-run");
    libadwaita::init().expect("libadwaita init");

    let dir = std::env::temp_dir().join("scribe-indice-prueba");
    std::fs::create_dir_all(&dir).expect("crear directorio de prueba");

    let base = export_pdf::ExportConfig {
        esquema: settings::ExportColorScheme::Light,
        familia_cuerpo: settings::FontFamily::Sans,
        tamano_cuerpo: 11.0,
        fuente_mono: settings::ExportMonoFont::AdwaitaMono,
        papel: settings::ExportPageSize::A4,
        orientacion: settings::ExportOrientation::Portrait,
        template: settings::ExportTemplate::Article,
        theme: settings::ExportTheme::None,
        css_path: None,
        indice: false,
    };
    let con_indice = export_pdf::ExportConfig {
        indice: true,
        ..base.clone()
    };

    // Escenario 1: documento de varias páginas e índice de una página.
    let doc = documento_con_indice();
    let sin = dir.join("sin-indice.pdf");
    let con = dir.join("con-indice.pdf");
    export_pdf::exportar_a_fichero(&base, &doc, "Indice", &sin, false)
        .expect("exportar sin índice");
    export_pdf::exportar_a_fichero(&con_indice, &doc, "Indice", &con, false)
        .expect("exportar con índice");

    let Some(paginas_sin) = paginas_de_texto(&sin) else {
        eprintln!("pdftotext no disponible: se omite la comprobación del índice");
        return;
    };
    let paginas = paginas_de_texto(&con).expect("texto del PDF con índice");

    assert!(
        !paginas_sin[0].contains("Table of contents"),
        "sin el ajuste el PDF no debe llevar índice"
    );
    assert!(
        paginas[0].contains("Table of contents"),
        "el índice debe abrir el PDF: {}",
        paginas[0]
    );
    let n_indice = paginas
        .iter()
        .position(|p| p.contains("BODYSTART"))
        .expect("el cuerpo debe empezar tras el índice");
    assert_eq!(n_indice, 1, "seis cabeceras caben en una página de índice");
    assert_eq!(
        paginas.len(),
        paginas_sin.len() + n_indice,
        "el índice solo añade páginas delante del cuerpo"
    );
    let titulos: Vec<String> = TITULOS_INDICE.iter().map(|t| t.to_string()).collect();
    let numeros = comprobar_numeros(&paginas, n_indice, &titulos);
    let mut distintos = numeros.clone();
    distintos.sort_unstable();
    distintos.dedup();
    assert!(
        distintos.len() >= 3,
        "las cabeceras deben caer en páginas distintas: {numeros:?}"
    );

    // Escenario 2: índice de dos páginas, que desplaza todos los números.
    let (doc_largo, titulos_largo) = documento_con_indice_largo();
    let largo = dir.join("indice-largo.pdf");
    export_pdf::exportar_a_fichero(&con_indice, &doc_largo, "Largo", &largo, false)
        .expect("exportar el documento largo");
    let paginas_largo = paginas_de_texto(&largo).expect("texto del PDF largo");
    let n_indice_largo = paginas_largo
        .iter()
        .position(|p| p.contains("BODYSTART"))
        .expect("el cuerpo debe empezar tras el índice");
    assert!(
        n_indice_largo >= 2,
        "70 entradas no caben en una página de índice (fueron {n_indice_largo})"
    );
    let numeros_largo = comprobar_numeros(&paginas_largo, n_indice_largo, &titulos_largo);
    // El desplazamiento del índice se nota: la primera cabecera no cae en la
    // página 1 sino tras las páginas del índice.
    assert_eq!(numeros_largo[0], n_indice_largo + 1);
    // Escenario 3: con la plantilla Manuscrito el pie numera "n / total" y el
    // total tiene que contar las páginas del índice.
    let manuscrito = export_pdf::ExportConfig {
        template: settings::ExportTemplate::Manuscript,
        ..con_indice.clone()
    };
    let con_pie = dir.join("manuscrito.pdf");
    export_pdf::exportar_a_fichero(&manuscrito, &doc, "Indice", &con_pie, false)
        .expect("exportar con la plantilla Manuscrito");
    let paginas_pie = paginas_de_texto(&con_pie).expect("texto del PDF con pie");
    let total = paginas_pie.len();
    // pdftotext pega el pie ("1/8"): se compara sin espacios.
    let sin_espacios = |texto: &str| texto.replace(' ', "");
    assert!(
        sin_espacios(&paginas_pie[0]).contains(&format!("1/{total}")),
        "el índice es la página 1 de {total}: {}",
        paginas_pie[0]
    );
    let ultima = &paginas_pie[total - 1];
    assert!(
        sin_espacios(ultima).contains(&format!("{total}/{total}")),
        "la última página debe cerrar la numeración: {ultima}"
    );

    eprintln!(
        "PDFs de prueba en {} (índice de {n_indice} y de {n_indice_largo} páginas)",
        dir.display()
    );
}
