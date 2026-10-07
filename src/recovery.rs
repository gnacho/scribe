//! Recuperación de documentos con cambios sin guardar (issue #23).
//!
//! Mientras un documento está modificado, Scribe escribe una copia de su
//! texto en el directorio de estado de XDG (`~/.local/state/scribe/
//! recuperacion/`). Si la aplicación muere sin cerrar con gracia (un crash o
//! un apagado), esos snapshots quedan en disco y se ofrecen al arrancar para
//! recuperarlos, descartarlos o dejarlos para más tarde.
//!
//! El módulo separa las funciones puras (nombre de fichero, serialización)
//! de la E/S (escribir, listar, borrar), de modo que las primeras se prueban
//! con unit tests headless y las segundas contra un directorio temporal.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Marca mágica de la primera línea de cada snapshot: identifica el formato y
/// permite descartar ficheros corruptos o ajenos al listar.
const MAGIA: &str = "scribe-recovery-v1";

/// Subdirectorio (relativo al state dir de XDG) donde viven los snapshots.
const SUBDIR: &str = "scribe/recuperacion";

/// Contador monotónico de ids de documento dentro de esta ejecución.
static SIGUIENTE_ID: AtomicU64 = AtomicU64::new(1);

/// Un snapshot de recuperación: el texto del documento y sus metadatos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// Nombre del fichero en disco (sin la ruta del directorio).
    pub nombre: String,
    /// Texto completo del documento en el momento del snapshot.
    pub texto: String,
    /// Ruta original del documento, si tenía fichero asociado.
    pub ruta: Option<PathBuf>,
    /// Título legible para mostrar en el diálogo de recuperación.
    pub titulo: String,
    /// Marca de tiempo UNIX (segundos) del último guardado.
    pub timestamp: u64,
}

/// Construye la ruta de los snapshots a partir del state dir base (pura, para
/// tests): `<base>/scribe/recuperacion`.
fn dir_desde_base(base: &Path) -> PathBuf {
    base.join(SUBDIR)
}

/// Directorio de estado de Scribe donde se guardan los snapshots.
/// Respeta `XDG_STATE_HOME`; si no está definida (o está vacía), usa
/// `~/.local/state`, el fallback de la especificación XDG Base Directory.
pub fn dir_recuperacion() -> PathBuf {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| glib::home_dir().join(".local/state"));
    dir_desde_base(&base)
}

/// Id único y estable para un documento durante esta ejecución. Se usa en el
/// nombre del snapshot para que sea estable por documento aunque cambie su
/// ruta (p. ej. con «guardar como»).
pub fn nuevo_id_doc() -> u64 {
    SIGUIENTE_ID.fetch_add(1, Ordering::Relaxed)
}

/// Nombre de fichero estable para el snapshot de un documento: combina el pid
/// y el id del documento. Es puro y determinista, lo que permite borrar el
/// snapshot del documento sin guardar su nombre.
pub fn nombre_snapshot(pid: u32, id_doc: u64) -> String {
    format!("{pid}-{id_doc}.scribe-recovery")
}

/// Marca de tiempo UNIX actual en segundos.
fn ahora() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// --- serialización ------------------------------------------------------

/// Escapa los bytes problemáticos de un campo de cabecera (ruta o título)
/// para que quepa en una sola línea: `%` → `%25`, `\n` → `%0A`, `\r` → `%0D`.
/// El texto del documento no pasa por aquí: va en el cuerpo, sin escapar.
fn codificar_campo(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '%' => out.push_str("%25"),
            '\n' => out.push_str("%0A"),
            '\r' => out.push_str("%0D"),
            _ => out.push(c),
        }
    }
    out
}

/// Inverso de [`codificar_campo`].
fn decodificar_campo(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let resto: String = chars.by_ref().take(2).collect();
        match resto.as_str() {
            "25" => out.push('%'),
            "0A" => out.push('\n'),
            "0D" => out.push('\r'),
            // Secuencia desconocida: se conserva tal cual (no se pierde nada
            // por no interpretar un «%» mal formado).
            _ => {
                out.push('%');
                out.push_str(&resto);
            }
        }
    }
    out
}

/// Serializa un snapshot a su representación en disco:
///
/// ```text
/// scribe-recovery-v1
/// <timestamp>
/// <ruta codificada (vacía si no hay fichero)>
/// <título codificado>
/// <texto (cuerpo, sin escapar)>
/// ```
///
/// Pura: no toca el sistema de ficheros.
pub fn serializar_snapshot(
    texto: &str,
    ruta: Option<&str>,
    titulo: &str,
    timestamp: u64,
) -> String {
    let ruta = codificar_campo(ruta.unwrap_or(""));
    let titulo = codificar_campo(titulo);
    format!("{MAGIA}\n{timestamp}\n{ruta}\n{titulo}\n{texto}")
}

/// Parsea un snapshot desde su contenido en disco. Devuelve `Err` si el
/// fichero no es un snapshot válido (marca mágica incorrecta o timestamp no
/// numérico), para que [`listar_snapshots`] pueda descartarlo en silencio.
pub fn parsear_snapshot(nombre: &str, contenido: &str) -> Result<Snapshot, String> {
    let mut partes = contenido.splitn(5, '\n');
    let magia = partes.next().unwrap_or("");
    if magia != MAGIA {
        return Err(format!("marca mágica desconocida: {magia}"));
    }
    let timestamp = partes
        .next()
        .ok_or_else(|| "falta el timestamp".to_string())?
        .parse::<u64>()
        .map_err(|e| format!("timestamp no válido: {e}"))?;
    let ruta_cod = partes.next().ok_or_else(|| "falta la ruta".to_string())?;
    let titulo_cod = partes.next().ok_or_else(|| "falta el título".to_string())?;
    // El resto (quinta parte de splitn) es el cuerpo íntegro, con sus saltos.
    let cuerpo = partes.next().unwrap_or("");
    let ruta = {
        let r = decodificar_campo(ruta_cod);
        if r.is_empty() {
            None
        } else {
            Some(PathBuf::from(r))
        }
    };
    Ok(Snapshot {
        nombre: nombre.to_string(),
        texto: cuerpo.to_string(),
        ruta,
        titulo: decodificar_campo(titulo_cod),
        timestamp,
    })
}

// --- E/S ----------------------------------------------------------------

/// Escribe el snapshot de un documento en `dir`. Crea el directorio si no
/// existe y escribe atómicamente (a un temporal y renombrando) para no dejar
/// un snapshot a medias si el proceso muere durante la escritura: el snapshot
/// anterior del documento se conserva hasta que el nuevo está completo.
pub fn escribir_snapshot(
    dir: &Path,
    nombre: &str,
    texto: &str,
    ruta: Option<&Path>,
    titulo: &str,
) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("crear {dir:?}: {e}"))?;
    let ruta_str = ruta.map(|p| p.to_string_lossy().into_owned());
    let contenido = serializar_snapshot(texto, ruta_str.as_deref(), titulo, ahora());
    let destino = dir.join(nombre);
    let tmp = dir.join(format!("{nombre}.tmp"));
    fs::write(&tmp, contenido).map_err(|e| format!("escribir {tmp:?}: {e}"))?;
    fs::rename(&tmp, &destino).map_err(|e| format!("renombrar a {destino:?}: {e}"))
}

/// Lista los snapshots pendientes en `dir`, ordenados del más reciente al más
/// antiguo. Ignora en silencio los ficheros ajenos o corruptos.
pub fn listar_snapshots(dir: &Path) -> Vec<Snapshot> {
    let Ok(entradas) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut snapshots: Vec<Snapshot> = Vec::new();
    for entrada in entradas.flatten() {
        let nombre = entrada.file_name().to_string_lossy().into_owned();
        if !nombre.ends_with(".scribe-recovery") {
            continue;
        }
        let Ok(contenido) = fs::read_to_string(entrada.path()) else {
            continue;
        };
        if let Ok(snap) = parsear_snapshot(&nombre, &contenido) {
            snapshots.push(snap);
        }
    }
    snapshots.sort_by_key(|s| std::cmp::Reverse(s.timestamp));
    snapshots
}

/// Borra el snapshot `nombre` de `dir`. Devuelve `Ok` también si no existía.
pub fn borrar_snapshot(dir: &Path, nombre: &str) -> Result<(), String> {
    let destino = dir.join(nombre);
    match fs::remove_file(&destino) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("borrar {destino:?}: {e}")),
    }
}

/// Borra todos los snapshots de `pid` (al cerrar con gracia). Devuelve el
/// número de ficheros borrados.
pub fn limpiar_de_este_pid(dir: &Path, pid: u32) -> usize {
    let Ok(entradas) = fs::read_dir(dir) else {
        return 0;
    };
    let prefijo = format!("{pid}-");
    let mut borrados = 0;
    for entrada in entradas.flatten() {
        let nombre = entrada.file_name().to_string_lossy().into_owned();
        if nombre.starts_with(&prefijo)
            && nombre.ends_with(".scribe-recovery")
            && fs::remove_file(entrada.path()).is_ok()
        {
            borrados += 1;
        }
    }
    borrados
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;

    static CONTADOR: AtomicU32 = AtomicU32::new(0);

    /// Crea un directorio temporal único por test y lo devuelve (el test
    /// debe borrarlo al final).
    fn tempdir() -> PathBuf {
        let n = CONTADOR.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("scribe-recovery-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn dir_recuperacion_cuelga_del_state_dir() {
        assert_eq!(
            dir_desde_base(Path::new("/estado")),
            PathBuf::from("/estado/scribe/recuperacion")
        );
    }

    #[test]
    fn nombre_snapshot_es_estable_y_unico() {
        assert_eq!(nombre_snapshot(42, 7), "42-7.scribe-recovery");
        // Mismo pid + id → mismo nombre (estable para un documento).
        assert_eq!(nombre_snapshot(42, 7), nombre_snapshot(42, 7));
        // Distinto id o distinto pid → distinto nombre.
        assert_ne!(nombre_snapshot(42, 7), nombre_snapshot(42, 8));
        assert_ne!(nombre_snapshot(42, 7), nombre_snapshot(43, 7));
    }

    #[test]
    fn serializacion_y_parseo_round_trip() {
        let texto = "# Título\n\nLínea con % y %0A literal\nfinal\n";
        let original = Snapshot {
            nombre: "x.scribe-recovery".to_string(),
            texto: texto.to_string(),
            ruta: Some(PathBuf::from("/ruta/con/espacio.md")),
            titulo: "Mi nota".to_string(),
            timestamp: 1_234_567_890,
        };
        let ruta = original.ruta.as_ref().and_then(|p| p.to_str());
        let ser = serializar_snapshot(&original.texto, ruta, &original.titulo, original.timestamp);
        let parsed = parsear_snapshot("x.scribe-recovery", &ser).unwrap();
        assert_eq!(parsed, original);
    }

    #[test]
    fn snapshot_sin_fichero_redondea_con_ruta_vacia() {
        let ser = serializar_snapshot("cuerpo", None, "Sin título", 5);
        let parsed = parsear_snapshot("x.scribe-recovery", &ser).unwrap();
        assert_eq!(parsed.ruta, None);
        assert_eq!(parsed.texto, "cuerpo");
        assert_eq!(parsed.titulo, "Sin título");
        assert_eq!(parsed.timestamp, 5);
    }

    #[test]
    fn ruta_y_titulo_con_saltos_se_codifican() {
        let ser = serializar_snapshot("cuerpo", Some("/a\nb.md"), "título\nmultilínea", 5);
        let parsed = parsear_snapshot("x.scribe-recovery", &ser).unwrap();
        assert_eq!(parsed.ruta, Some(PathBuf::from("/a\nb.md")));
        assert_eq!(parsed.titulo, "título\nmultilínea");
        assert_eq!(parsed.texto, "cuerpo");
    }

    #[test]
    fn parseo_descarta_marca_desconocida() {
        let err = parsear_snapshot("x.scribe-recovery", "magia rota\n1\n\n\n").unwrap_err();
        assert!(err.contains("marca mágica"), "error inesperado: {err}");
    }

    #[test]
    fn parseo_descarta_timestamp_no_valido() {
        let err = parsear_snapshot("x.scribe-recovery", "scribe-recovery-v1\nno-numérico\n\n\n")
            .unwrap_err();
        assert!(err.contains("timestamp"), "error inesperado: {err}");
    }

    #[test]
    fn escribe_lista_y_borra_con_tempdir() {
        let dir = tempdir();
        escribir_snapshot(&dir, "100-1.scribe-recovery", "texto uno", None, "Uno").unwrap();
        escribir_snapshot(
            &dir,
            "100-2.scribe-recovery",
            "texto dos",
            Some(Path::new("/b.md")),
            "Dos",
        )
        .unwrap();

        let listado = listar_snapshots(&dir);
        assert_eq!(listado.len(), 2);

        borrar_snapshot(&dir, "100-1.scribe-recovery").unwrap();
        let restantes = listar_snapshots(&dir);
        assert_eq!(restantes.len(), 1);
        assert_eq!(restantes[0].titulo, "Dos");

        // Borrar algo que no existe no es un error.
        assert!(borrar_snapshot(&dir, "100-1.scribe-recovery").is_ok());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn listar_ordena_por_timestamp_descendente() {
        let dir = tempdir();
        let viejo = serializar_snapshot("viejo", None, "Viejo", 100);
        let nuevo = serializar_snapshot("nuevo", None, "Nuevo", 200);
        std::fs::write(dir.join("100-1.scribe-recovery"), viejo).unwrap();
        std::fs::write(dir.join("100-2.scribe-recovery"), nuevo).unwrap();

        let listado = listar_snapshots(&dir);
        assert_eq!(listado.len(), 2);
        assert_eq!(listado[0].titulo, "Nuevo");
        assert_eq!(listado[1].titulo, "Viejo");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn listar_descarta_ficheros_corruptos() {
        let dir = tempdir();
        std::fs::write(dir.join("basura.txt"), "no es un snapshot").unwrap();
        std::fs::write(dir.join("100-1.scribe-recovery"), "magia rota").unwrap();
        escribir_snapshot(&dir, "100-2.scribe-recovery", "válido", None, "Válido").unwrap();

        let listado = listar_snapshots(&dir);
        assert_eq!(listado.len(), 1);
        assert_eq!(listado[0].titulo, "Válido");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn limpiar_de_este_pid_no_toca_otros() {
        let dir = tempdir();
        escribir_snapshot(&dir, "100-1.scribe-recovery", "a", None, "A").unwrap();
        escribir_snapshot(&dir, "200-1.scribe-recovery", "b", None, "B").unwrap();

        let borrados = limpiar_de_este_pid(&dir, 100);
        assert_eq!(borrados, 1);

        let restantes = listar_snapshots(&dir);
        assert_eq!(restantes.len(), 1);
        assert_eq!(restantes[0].nombre, "200-1.scribe-recovery");

        std::fs::remove_dir_all(&dir).ok();
    }
}
