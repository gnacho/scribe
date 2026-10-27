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
