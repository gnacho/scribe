//! Conversión del texto del buffer a los bytes finales del fichero.
//!
//! Esta función es el único punto por el que el texto del editor se convierte
//! en lo que acaba escrito en disco. Es deliberadamente pura (sin GTK) para
//! que el pacto de bytes (tests/covenant.rs) pueda probarla headless, sin
//! display, en el CI normal.

/// Devuelve los bytes que hay que escribir al guardar el documento.
///
/// La conversión es la identidad UTF-8, sin normalizar finales de línea y sin
/// añadir salto final: el `GtkTextBuffer` conserva los `\r` de los ficheros
/// CRLF como parte del texto de línea (véase el comentario y el test
/// `alinear_conserva_los_finales_crlf` en markdown_render.rs) y `editor.text()`
/// devuelve exactamente ese texto. Escribir esos bytes tal cual es lo que
/// garantiza que abrir y guardar sin editar sea un round-trip byte a byte.
pub fn bytes_a_escribir(texto: &str) -> Vec<u8> {
    texto.as_bytes().to_vec()
}
