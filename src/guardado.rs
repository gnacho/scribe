//! Conversión del texto del buffer a los bytes finales del fichero.
//!
//! Esta función es el único punto por el que el texto del editor se convierte
//! en lo que acaba escrito en disco. Es deliberadamente pura (sin GTK) para
//! que el pacto de bytes (tests/covenant.rs) pueda probarla headless, sin
//! display, en el CI normal.

use std::path::Path;

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

/// ¿Se puede escribir sobre esta ruta con la cuenta actual?
///
/// Se pregunta antes de guardar sobre un fichero ya conocido. Cuando Scribe
/// abre un documento que le presta el portal de documentos (Flatpak, p. ej.
/// desde la búsqueda del overview), la ruta vive en un montaje temporal que el
/// portal puede haber exportado de solo lectura: escribir fallaría con EACCES
/// en el primer Ctrl+S. Preguntarlo antes permite ofrecer guardar una copia en
/// vez de un error. Un fichero que ya no existe se considera escribible: el
/// guardado lo vuelve a crear, como antes (issue #211).
pub fn es_escribible(path: &Path) -> bool {
    match std::fs::OpenOptions::new().write(true).open(path) {
        Ok(_) => true,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => true,
        Err(_) => false,
    }
}

/// ¿La ruta la sirve el portal de documentos (`/run/user/<uid>/doc/...`)?
///
/// Ese montaje solo existe mientras dura la concesión del portal y no dice
/// dónde vive el documento de verdad, así que no aporta nada en el subtítulo
/// de la cabecera (issue #212).
pub fn ruta_de_portal(path: &Path) -> bool {
    let texto = path.to_string_lossy();
    let Some(resto) = texto.strip_prefix("/run/user/") else {
        return false;
    };
    let mut partes = resto.splitn(2, '/');
    let uid = partes.next().unwrap_or("");
    let cola = partes.next().unwrap_or("");
    !uid.is_empty() && uid.chars().all(|c| c.is_ascii_digit()) && cola.starts_with("doc/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn reconoce_las_rutas_del_portal_de_documentos() {
        assert!(ruta_de_portal(Path::new(
            "/run/user/1000/doc/1Wbk_g27S0XU8mlT1GDnsA/nota.md"
        )));
        assert!(ruta_de_portal(Path::new("/run/user/0/doc/a/b.md")));
        assert!(!ruta_de_portal(Path::new("/run/user/1000/doc")));
        assert!(!ruta_de_portal(Path::new("/run/user/1000/gvfs/x.md")));
        assert!(!ruta_de_portal(Path::new("/home/nacho/notas/nota.md")));
        assert!(!ruta_de_portal(Path::new("/run/media/doc/x.md")));
    }

    #[test]
    fn un_fichero_de_solo_lectura_no_es_escribible() {
        let dir = std::env::temp_dir().join(format!("scribe-guardado-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("directorio temporal");
        let editable = dir.join("editable.md");
        let fijo = dir.join("fijo.md");
        std::fs::write(&editable, "# hola\n").expect("escribir editable");
        std::fs::write(&fijo, "# hola\n").expect("escribir fijo");
        let mut permisos = std::fs::metadata(&fijo).expect("metadata").permissions();
        permisos.set_mode(0o444);
        std::fs::set_permissions(&fijo, permisos).expect("chmod");

        assert!(es_escribible(&editable));
        assert!(!es_escribible(&fijo));
        // Un fichero que ya no está se puede volver a crear al guardar.
        assert!(es_escribible(&dir.join("todavia-no-existe.md")));

        let mut permisos = std::fs::metadata(&fijo).expect("metadata").permissions();
        permisos.set_mode(0o644);
        let _ = std::fs::set_permissions(&fijo, permisos);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
