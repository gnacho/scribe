//! Pacto de bytes (issue #15): abrir y guardar sin editar debe reescribir
//! los bytes originales, y una única edición solo toca sus líneas.
//!
//! Todo headless: no hay `gtk::init` ni ventana. El covenant se apoya en la
//! función pura `guardado::bytes_a_escribir`, que es la conversión «texto del
//! buffer -> bytes del fichero» usada por `file_manager` al guardar. Así puede
//! vivir en el CI normal (sin display).

// Al incluir el módulo vía #[path] solo se ejercita una parte de su API; el
// resto aparecería como código muerto aunque la app sí lo use (mismo criterio
// que tests/crash_stress.rs).
#![allow(dead_code)]

#[path = "../src/guardado.rs"]
mod guardado;

use guardado::bytes_a_escribir;
use std::path::{Path, PathBuf};

/// Directorio `src/testdata` resuelto desde el manifiesto (independiente del
/// directorio de trabajo desde el que se lance cargo).
fn testdata_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/testdata")
}

/// Todos los ficheros `.md` de `src/testdata`, como bytes crudos.
fn fixtures() -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let dir = testdata_dir();
    let entries = std::fs::read_dir(&dir).expect("leer src/testdata");
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("md") {
            let bytes = std::fs::read(&path).expect("leer fixture");
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            out.push((name, bytes));
        }
    }
    // Orden estable para que los fallos sean reproducibles.
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Sustituye el contenido de la línea `indice` por `nuevo`, conservando el
/// terminador `\r` si la línea original lo tenía. Devuelve el texto editado.
fn sustituir_linea(texto: &str, indice: usize, nuevo: &str) -> String {
    let mut lineas: Vec<String> = texto.split('\n').map(|s| s.to_string()).collect();
    let terminador = if lineas[indice].ends_with('\r') {
        "\r"
    } else {
        ""
    };
    lineas[indice] = format!("{nuevo}{terminador}");
    lineas.join("\n")
}

/// Aserta que, salvo la línea `tocada`, todas las líneas de `editado` son
/// idénticas a las de `original`, incluido el `\r` de los finales CRLF.
fn lineas_intactas(original: &str, editado: &str, tocada: usize) {
    let o: Vec<&str> = original.split('\n').collect();
    let e: Vec<&str> = editado.split('\n').collect();
    assert_eq!(o.len(), e.len(), "mismo número de líneas");
    for (i, (a, b)) in o.iter().zip(e.iter()).enumerate() {
        if i != tocada {
            assert_eq!(a, b, "la línea {i} debe quedar intacta");
        }
    }
}

#[test]
fn abrir_y_guardar_sin_editar_conserva_los_bytes() {
    // Corpus: fixtures de src/testdata + sintéticos con finales LF, CRLF y
    // mixtos generados aquí mismo.
    let mut corpus: Vec<Vec<u8>> = fixtures().into_iter().map(|(_, b)| b).collect();
    corpus.push(b"linea uno\nlinea dos\n\nfin\n".to_vec()); // LF
    corpus.push(b"linea uno\r\nlinea dos\r\n\r\nfin\r\n".to_vec()); // CRLF
    corpus.push(b"lf aqui\ncrlf aqui\r\nmixto\nultima\r\n".to_vec()); // mixto

    for original in &corpus {
        let texto = String::from_utf8(original.clone()).expect("texto utf-8");
        let escritos = bytes_a_escribir(&texto);
        assert_eq!(
            &escritos, original,
            "abrir + guardar sin editar debe ser byte a byte"
        );
    }
}

#[test]
fn una_insercion_solo_toca_su_linea() {
    let original = "linea uno\nlinea dos\r\nlinea tres\nlinea cuatro\r\n";
    // Insertar una palabra al final de la línea 1 (índice 1).
    let editado = sustituir_linea(original, 1, "linea dos EDITADA");
    let guardado = String::from_utf8(bytes_a_escribir(&editado)).expect("utf-8");
    lineas_intactas(original, &guardado, 1);
    assert_eq!(guardado.split('\n').nth(1), Some("linea dos EDITADA\r"));
}

#[test]
fn un_borrado_solo_toca_su_linea() {
    let original = "cabecera\ncuerpo largo\npie de página\r\n";
    // Borrar la cola de la línea 1 (índice 1), dejando solo «cuerpo».
    let editado = sustituir_linea(original, 1, "cuerpo");
    let guardado = String::from_utf8(bytes_a_escribir(&editado)).expect("utf-8");
    lineas_intactas(original, &guardado, 1);
    assert_eq!(guardado.split('\n').nth(1), Some("cuerpo"));
}

#[test]
fn un_reemplazo_solo_toca_su_linea() {
    let original = "a\nb\r\nc\nd\r\n";
    // Reemplazar toda la línea 2 (índice 2) por otro texto.
    let editado = sustituir_linea(original, 2, "sustituida");
    let guardado = String::from_utf8(bytes_a_escribir(&editado)).expect("utf-8");
    lineas_intactas(original, &guardado, 2);
    assert_eq!(guardado.split('\n').nth(2), Some("sustituida"));
}
