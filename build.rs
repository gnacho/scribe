//! Compila los iconos de la app en un GResource (issue #111).
//!
//! El binario registra el recurso al arrancar y asi los iconos propios se
//! resuelven aunque no se haya instalado nada en ningun tema de iconos
//! (cargo build, copia manual del binario). Requiere glib-compile-resources
//! en el PATH: viene con glib2 en las distros del CI, el SDK de GNOME del
//! Flatpak y el entorno de makepkg (glib2 ya es dependencia).

use std::process::Command;

fn main() {
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR definido por cargo");
    let target = format!("{out_dir}/icons.gresource");
    let status = Command::new("glib-compile-resources")
        .args([
            "--sourcedir=data/icons",
            "--target",
            &target,
            "data/icons/icons.gresource.xml",
        ])
        .status()
        .expect("glib-compile-resources es necesario para empaquetar los iconos");
    assert!(status.success(), "glib-compile-resources fallo");

    println!("cargo:rerun-if-changed=data/icons/icons.gresource.xml");
    for icon in [
        "hicolor/scalable/apps/io.github.gnacho.Scribe.svg",
        "hicolor/scalable/actions/io.github.gnacho.Scribe-preview-on-symbolic.svg",
        "hicolor/scalable/actions/io.github.gnacho.Scribe-preview-off-symbolic.svg",
    ] {
        println!("cargo:rerun-if-changed=data/icons/{icon}");
    }
}
