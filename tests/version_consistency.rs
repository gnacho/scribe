//! Test de consistencia de versiones (issue #16): la versión se sube a mano
//! en varios sitios y pueden descuadrarse entre sí. Este test falla si:
//! - la versión de `Cargo.toml` difiere de la release más reciente del
//!   metainfo (`data/io.github.gnacho.Scribe.metainfo.xml`);
//! - la entrada más reciente del CHANGELOG no menciona esa versión;
//! - el fichero `.desktop` o el metainfo no pasan sus validadores (cuando las
//!   herramientas `appstreamcli` y `desktop-file-validate` están instaladas).
//!
//! Es standalone: solo lee ficheros del repo (vía `env!("CARGO_MANIFEST_DIR")`)
//! y no importa módulos de `src`, así que corre en el CI normal sin display.

use std::io::ErrorKind;
use std::path::Path;
use std::process::Command;

/// Ruta raíz del repo, independiente del directorio de trabajo desde el que
/// se lance cargo.
fn raiz() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// Lee un fichero del repo como texto.
fn leer(rel: &str) -> String {
    std::fs::read_to_string(raiz().join(rel))
        .unwrap_or_else(|e| panic!("no se pudo leer {rel}: {e}"))
}

/// Versión declarada en `Cargo.toml` (línea `version = "x.y.z"`).
fn version_cargo() -> String {
    let cargo = leer("Cargo.toml");
    for linea in cargo.lines() {
        let linea = linea.trim();
        if let Some(resto) = linea.strip_prefix("version = \"") {
            let fin = resto.find('"').expect("comilla de cierre de version");
            return resto[..fin].to_string();
        }
    }
    panic!("no se encontró `version` en Cargo.toml");
}

/// Versión de la release más reciente del metainfo: la primera aparición de
/// `<release version="...">` (la lista va de más nueva a más antigua).
fn version_metainfo() -> String {
    let xml = leer("data/io.github.gnacho.Scribe.metainfo.xml");
    for linea in xml.lines() {
        if let Some(idx) = linea.find("<release ") {
            let resto = &linea[idx..];
            let inicio = resto.find("version=\"").expect("atributo version") + "version=\"".len();
            let fin = resto[inicio..]
                .find('"')
                .expect("cierre del atributo version");
            return resto[inicio..inicio + fin].to_string();
        }
    }
    panic!("no se encontró `<release version=...>` en el metainfo");
}

#[test]
fn cargo_toml_coincide_con_la_release_mas_reciente_del_metainfo() {
    let de_cargo = version_cargo();
    let de_metainfo = version_metainfo();
    assert_eq!(
        de_cargo, de_metainfo,
        "Cargo.toml dice {de_cargo} pero la release más nueva del metainfo es {de_metainfo}"
    );
}

#[test]
fn el_changelog_menciona_la_version_de_cargo() {
    let version = version_cargo();
    let changelog = leer("CHANGELOG.md");
    assert!(
        changelog.contains(&format!("[{version}]")),
        "el CHANGELOG no menciona la versión {version} como encabezado"
    );
}

#[test]
fn el_desktop_y_el_metainfo_pasan_sus_validadores() {
    let metainfo = raiz().join("data/io.github.gnacho.Scribe.metainfo.xml");
    let desktop = raiz().join("data/io.github.gnacho.Scribe.desktop");

    // `appstreamcli validate <metainfo>`; salto silencioso si no está instalado.
    match Command::new("appstreamcli")
        .arg("validate")
        .arg(&metainfo)
        .output()
    {
        Ok(salida) => assert!(salida.status.success(), "appstreamcli rechazó el metainfo"),
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => panic!("no se pudo ejecutar appstreamcli: {e}"),
    }

    // `desktop-file-validate <desktop>`; salto silencioso si no está instalado.
    match Command::new("desktop-file-validate").arg(&desktop).output() {
        Ok(salida) => assert!(
            salida.status.success(),
            "desktop-file-validate rechazó el .desktop"
        ),
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => panic!("no se pudo ejecutar desktop-file-validate: {e}"),
    }
}
