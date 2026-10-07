//! Exportación real a PDF: genera un fichero con la operación de impresión de
//! GTK y comprueba sus propiedades básicas (páginas, texto extraíble).
//!
//! Ejecutar: xvfb-run -a cargo test --test export_pdf -- --ignored --nocapture

// Los módulos de la app se incluyen vía #[path] y el harness solo ejercita
// parte de su API: el resto aparece como código muerto aunque la app sí lo
// use (mismo criterio que tests/render_shot.rs).
#![allow(dead_code)]

#[path = "../src/export_pdf.rs"]
mod export_pdf;
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

\\newpage

## Page Break Section

This text lands on a new page because of the explicit `\\newpage` above.
";

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
    };
    export_pdf::exportar_a_fichero(&config, DOC, "Prueba", &ruta, false).expect("exportar a PDF");

    assert!(ruta.exists(), "el PDF no se escribió en {}", ruta.display());
    assert!(
        std::fs::metadata(&ruta).map(|m| m.len()).unwrap_or(0) > 0,
        "el PDF está vacío"
    );
    eprintln!("PDF escrito en {}", ruta.display());
}
