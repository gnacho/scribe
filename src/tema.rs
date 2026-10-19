//! Temas semánticos nativos de la vista de texto (issue #25).
//!
//! Un tema es un conjunto de colores con nombre en formato keyfile
//! (`.ini`, grupo `[theme]`), aplicado a los tags del buffer, a la paleta
//! de adornos y al fondo de la vista. Los valores que falten caen en los
//! del tema base (claro u oscuro según el esquema de color activo).

use gtk4::gdk;

/// Colores semánticos de un tema. Todos en `#rrggbb`.
#[derive(Debug, Clone)]
pub struct Tema {
    /// Nombre visible (se usa en el selector de Preferencias, PR del picker).
    #[allow(dead_code)]
    pub nombre: String,
    /// Fondo de la vista (CSS del widget).
    pub fondo: String,
    /// Texto principal.
    pub primer_plano: String,
    /// Encabezados (h1-h6).
    pub encabezado: String,
    /// Enlaces, viñetas y marcadores de lista.
    pub acento: String,
    /// Marcas de sintaxis visibles y texto técnico (html, fence).
    pub atenuado: String,
    /// Marcas de sintaxis cuando están visibles (`syn_shown`).
    pub marcas: String,
    /// Código en línea.
    pub codigo: String,
    /// Bloques de código.
    pub bloque_codigo: String,
    /// Citas.
    pub cita: String,
    /// Tablas (texto).
    pub tabla: String,
    /// Bordes y líneas de los adornos.
    pub borde: String,
    /// Relleno de bloques (`#rrggbb` + canal alfa en `bloque_alfa`).
    pub bloque: String,
    pub bloque_alfa: f32,
    /// Barra de las citas.
    pub barra_cita: String,
    /// Texto sobre el acento (p. ej. la marca de la casilla marcada).
    pub sobre_acento: String,
    /// Texto atenuado del modo foco.
    pub desenfocado: String,
}

impl Tema {
    /// Tema claro por defecto (los valores que ya llevaba la app).
    pub fn claro() -> Self {
        Self {
            nombre: "Light".into(),
            fondo: "#ffffff".into(),
            primer_plano: "#1f1f1f".into(),
            encabezado: "#1f1f1f".into(),
            acento: "#1a6ed8".into(),
            atenuado: "#8b8a88".into(),
            marcas: "#b5b4b1".into(),
            codigo: "#a34a00".into(),
            bloque_codigo: "#1f1f1f".into(),
            cita: "#54535a".into(),
            tabla: "#1f1f1f".into(),
            borde: "#c0bfbc".into(),
            bloque: "#000000".into(),
            bloque_alfa: 0.05,
            barra_cita: "#c0bfbc".into(),
            sobre_acento: "#ffffff".into(),
            desenfocado: "#bdbcba".into(),
        }
    }

    /// Tema oscuro por defecto.
    pub fn oscuro() -> Self {
        Self {
            nombre: "Dark".into(),
            fondo: "#1e1e1e".into(),
            primer_plano: "#e4e4e4".into(),
            encabezado: "#e4e4e4".into(),
            acento: "#82b8f0".into(),
            atenuado: "#8f8f8f".into(),
            marcas: "#787878".into(),
            codigo: "#f0a868".into(),
            bloque_codigo: "#e4e4e4".into(),
            cita: "#c2c2c2".into(),
            tabla: "#e0e0e0".into(),
            borde: "#6f6f6f".into(),
            bloque: "#ffffff".into(),
            bloque_alfa: 0.07,
            barra_cita: "#6f6f6f".into(),
            sobre_acento: "#1b1b1b".into(),
            desenfocado: "#5c5c5c".into(),
        }
    }

    /// Sepia: lectura prolongada sobre papel cálido.
    pub fn sepia() -> Self {
        Self {
            nombre: "Sepia".into(),
            fondo: "#f5efe0".into(),
            primer_plano: "#43392a".into(),
            encabezado: "#3a2f21".into(),
            acento: "#8a5a2b".into(),
            atenuado: "#9c8d74".into(),
            marcas: "#b3a488".into(),
            codigo: "#8a4a1f".into(),
            bloque_codigo: "#43392a".into(),
            cita: "#6b5b44".into(),
            tabla: "#43392a".into(),
            borde: "#c9bb9d".into(),
            bloque: "#43392a".into(),
            bloque_alfa: 0.06,
            barra_cita: "#c9bb9d".into(),
            sobre_acento: "#fdf8ec".into(),
            desenfocado: "#bdae8f".into(),
        }
    }

    /// Temas integrados, en el orden del selector.
    #[allow(dead_code)] // el selector de Preferencias llega en la siguiente unidad
    pub fn integrados() -> Vec<Self> {
        vec![Self::claro(), Self::oscuro(), Self::sepia()]
    }

    /// Busca un tema integrado por id (`light`, `dark`, `sepia`).
    pub fn integrado_por_id(id: &str) -> Option<Self> {
        match id {
            "light" => Some(Self::claro()),
            "dark" => Some(Self::oscuro()),
            "sepia" => Some(Self::sepia()),
            _ => None,
        }
    }

    /// Carga un tema desde un keyfile (`.ini` con grupo `[theme]`). Las
    /// claves ausentes heredan del tema base indicado. Devuelve `None` si
    /// el fichero no existe o no tiene grupo `[theme]`.
    #[allow(dead_code)] // los temas de usuario llegan en la siguiente unidad
    pub fn cargar(ruta: &std::path::Path, base: &Tema) -> Option<Self> {
        let texto = std::fs::read_to_string(ruta).ok()?;
        let mut tema = base.clone();
        let mut nombre = ruta
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Tema".into());
        for linea in texto.lines() {
            let linea = linea.trim();
            if linea == "[theme]"
                || linea.is_empty()
                || linea.starts_with(';')
                || linea.starts_with('#')
            {
                continue;
            }
            let Some((clave, valor)) = linea.split_once('=') else {
                continue;
            };
            let clave = clave.trim();
            let valor = valor.trim().trim_matches('"');
            if clave.eq_ignore_ascii_case("name") {
                nombre = valor.to_string();
                continue;
            }
            if clave.eq_ignore_ascii_case("block-alpha") {
                tema.bloque_alfa = valor.parse().unwrap_or(tema.bloque_alfa);
                continue;
            }
            let destino = match clave.to_ascii_lowercase().as_str() {
                "background" => Some(&mut tema.fondo),
                "foreground" => Some(&mut tema.primer_plano),
                "heading" => Some(&mut tema.encabezado),
                "accent" => Some(&mut tema.acento),
                "muted" => Some(&mut tema.atenuado),
                "marks" => Some(&mut tema.marcas),
                "code" => Some(&mut tema.codigo),
                "codeblock" => Some(&mut tema.bloque_codigo),
                "quote" => Some(&mut tema.cita),
                "table" => Some(&mut tema.tabla),
                "border" => Some(&mut tema.borde),
                "quote-bar" => Some(&mut tema.barra_cita),
                "on-accent" => Some(&mut tema.sobre_acento),
                "unfocused" => Some(&mut tema.desenfocado),
                _ => None,
            };
            if let Some(slot) = destino {
                *slot = valor.to_string();
            }
        }
        tema.nombre = nombre;
        Some(tema)
    }

    /// Color RGBA a partir de un `#rrggbb` del tema.
    pub fn rgba(hex: &str, alfa: f32) -> gdk::RGBA {
        let v = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0);
        gdk::RGBA::new(
            ((v >> 16) & 0xff) as f32 / 255.0,
            ((v >> 8) & 0xff) as f32 / 255.0,
            (v & 0xff) as f32 / 255.0,
            alfa,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tema_keyfile_hereda_los_faltantes() {
        let dir = std::env::temp_dir().join("scribe-tema-test.ini");
        std::fs::write(
            &dir,
            "[theme]\nname=Prueba\nbackground=#112233\naccent=#445566\n",
        )
        .unwrap();
        let base = Tema::claro();
        let tema = Tema::cargar(&dir, &base).expect("tema cargado");
        assert_eq!(tema.nombre, "Prueba");
        assert_eq!(tema.fondo, "#112233");
        assert_eq!(tema.acento, "#445566");
        // Hereda lo que no declara.
        assert_eq!(tema.codigo, base.codigo);
        assert_eq!(tema.cita, base.cita);
        std::fs::remove_file(&dir).ok();
    }

    #[test]
    fn tema_keyfile_tolerante_sin_grupo_explicito() {
        let dir = std::env::temp_dir().join("scribe-tema-vacio.ini");
        std::fs::write(&dir, "background=#101010\n").unwrap();
        let tema = Tema::cargar(&dir, &Tema::oscuro()).expect("carga igual");
        assert_eq!(tema.fondo, "#101010");
        std::fs::remove_file(&dir).ok();
    }

    #[test]
    fn tema_keyfile_inexistente_devuelve_none() {
        assert!(Tema::cargar(std::path::Path::new("/no/existe.ini"), &Tema::claro()).is_none());
    }
}
