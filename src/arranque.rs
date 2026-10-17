//! Interpretación de la línea de comandos (issue #17).
//!
//! Scribe acepta `scribe fichero.md:42:7` para abrir el fichero con el cursor
//! en la línea 42, columna 7 (1-based), y `git diff | scribe` para abrir el
//! texto entubado como documento sin título. Aquí vive la parte pura del
//! arranque: el parser de `ruta:línea[:columna]` y la decisión de qué hacer
//! (ficheros, stdin o ventana vacía), sin tocar el sistema de ficheros ni GTK.

use std::path::PathBuf;

/// Error al interpretar un argumento de la línea de comandos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorArgumento {
    /// Columna sin línea: el penúltimo segmento está vacío mientras el último
    /// es numérico (p. ej. `fichero.md::7`).
    ColumnaSinLinea,
}

/// Qué debe hacer Scribe al arrancar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccionArranque {
    /// Abrir los ficheros indicados (ya resueltos, con posición opcional).
    AbrirFicheros,
    /// Leer stdin y abrirlo como documento sin título.
    LeerStdin,
    /// Arranque normal: una ventana en blanco.
    VentanaVacia,
}

/// Devuelve el valor de `s` si es un número de línea/columna válido:
/// totalmente numérico y mayor que cero. `None` en caso contrario (vacío,
/// no numérico, cero, negativo o demasiado grande para `u32`).
fn numero_positivo(s: &str) -> Option<u32> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    match s.parse::<u32>() {
        Ok(n) if n > 0 => Some(n),
        _ => None,
    }
}

/// Separa `ruta:línea[:columna]` respetando que la ruta puede contener dos
/// puntos: solo el último (y penúltimo) segmento se trata como número si es
/// totalmente numérico y mayor que cero; si no, es una ruta normal.
///
/// - `fichero.md:7` → línea 7, columna 1 implícita.
/// - `fichero.md:7:3` → línea 7, columna 3.
/// - `ruta:con:dos:puntos.md` → ruta normal.
/// - `fichero.md::7` → `Err(ColumnaSinLinea)` (columna sin línea no es válida).
pub fn parsear_argumento(arg: &str) -> Result<(PathBuf, Option<(u32, u32)>), ErrorArgumento> {
    if !arg.contains(':') {
        return Ok((PathBuf::from(arg), None));
    }
    let segments: Vec<&str> = arg.split(':').collect();
    let ultimo = segments[segments.len() - 1];
    let penultimo = segments[segments.len() - 2];

    // Ambos numéricos y mayores que cero: ruta:línea:columna.
    if let (Some(linea), Some(columna)) = (numero_positivo(penultimo), numero_positivo(ultimo)) {
        let ruta = segments[..segments.len() - 2].join(":");
        return Ok((PathBuf::from(ruta), Some((linea, columna))));
    }
    // Solo el último numérico: ruta:línea (columna 1 implícita).
    if let Some(linea) = numero_positivo(ultimo) {
        if penultimo.is_empty() {
            return Err(ErrorArgumento::ColumnaSinLinea);
        }
        let ruta = segments[..segments.len() - 1].join(":");
        return Ok((PathBuf::from(ruta), Some((linea, 1))));
    }
    // Sin sufijo numérico válido: ruta normal.
    Ok((PathBuf::from(arg), None))
}

/// Decide la acción de arranque según haya ficheros y si stdin es un
/// terminal. Los ficheros mandan; sin ellos, un stdin no terminal significa
/// leer texto entubado.
pub fn accion_arranque(hay_ficheros: bool, stdin_es_tty: bool) -> AccionArranque {
    if hay_ficheros {
        AccionArranque::AbrirFicheros
    } else if !stdin_es_tty {
        AccionArranque::LeerStdin
    } else {
        AccionArranque::VentanaVacia
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argumento_con_line_col_abre_en_posicion() {
        let (ruta, posicion) = parsear_argumento("notas.md:42:7").unwrap();
        assert_eq!(ruta, PathBuf::from("notas.md"));
        assert_eq!(posicion, Some((42, 7)));
    }

    #[test]
    fn argumento_con_linea_sin_columna_usa_columna_uno() {
        let (ruta, posicion) = parsear_argumento("fichero.md:7").unwrap();
        assert_eq!(ruta, PathBuf::from("fichero.md"));
        assert_eq!(posicion, Some((7, 1)));
    }

    #[test]
    fn argumento_ruta_con_dos_puntos_es_ruta_normal() {
        let (ruta, posicion) = parsear_argumento("ruta:con:dos:puntos.md").unwrap();
        assert_eq!(ruta, PathBuf::from("ruta:con:dos:puntos.md"));
        assert_eq!(posicion, None);
    }

    #[test]
    fn argumento_sin_dos_puntos_es_ruta_normal() {
        let (ruta, posicion) = parsear_argumento("notas.md").unwrap();
        assert_eq!(ruta, PathBuf::from("notas.md"));
        assert_eq!(posicion, None);
    }

    #[test]
    fn argumento_con_numero_no_valido_es_ruta_normal() {
        let (ruta, posicion) = parsear_argumento("notas.md:abc").unwrap();
        assert_eq!(ruta, PathBuf::from("notas.md:abc"));
        assert_eq!(posicion, None);
    }

    #[test]
    fn argumento_con_cero_o_negativo_es_ruta_normal() {
        assert_eq!(
            parsear_argumento("notas.md:0").unwrap(),
            (PathBuf::from("notas.md:0"), None)
        );
        assert_eq!(
            parsear_argumento("notas.md:-3").unwrap(),
            (PathBuf::from("notas.md:-3"), None)
        );
    }

    #[test]
    fn argumento_numero_demasiado_grande_es_ruta_normal() {
        let (ruta, posicion) = parsear_argumento("notas.md:99999999999999999999").unwrap();
        assert_eq!(ruta, PathBuf::from("notas.md:99999999999999999999"));
        assert_eq!(posicion, None);
    }

    #[test]
    fn argumento_columna_sin_linea_devuelve_error() {
        let error = parsear_argumento("fichero.md::7").unwrap_err();
        assert_eq!(error, ErrorArgumento::ColumnaSinLinea);
    }

    #[test]
    fn stdin_se_lee_si_no_hay_ficheros_y_no_es_terminal() {
        assert_eq!(accion_arranque(false, false), AccionArranque::LeerStdin);
        assert_eq!(accion_arranque(false, true), AccionArranque::VentanaVacia);
        assert_eq!(accion_arranque(true, false), AccionArranque::AbrirFicheros);
        assert_eq!(accion_arranque(true, true), AccionArranque::AbrirFicheros);
    }
}
