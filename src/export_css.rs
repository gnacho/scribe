//! Subconjunto de CSS para el export a PDF.
//!
//! El export dibuja y pagina con cairo, así que no interpreta CSS de verdad:
//! este módulo lee un conjunto reducido de reglas y devuelve los valores que la
//! maquetación ya sabe aplicar (márgenes de página, tipografía, interlineado,
//! espaciado entre bloques y colores). Todo lo que no encaja en ese modelo se
//! ignora sin abortar la exportación, y los nombres de propiedad descartados se
//! acumulan en [`Ajustes::ignoradas`] para avisar una sola vez.

use crate::settings::{ExportMonoFont, FontFamily};

/// Color en componentes 0..1.
pub type Color = (f64, f64, f64);

/// Puntos por milímetro.
const PT_POR_MM: f64 = 72.0 / 25.4;
/// Puntos por centímetro.
const PT_POR_CM: f64 = 72.0 / 2.54;
/// Puntos por pulgada.
const PT_POR_PX: f64 = 0.75;

/// Márgenes de página en puntos. Un `None` deja el margen que traiga el papel.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MargenesCss {
    pub superior: Option<f64>,
    pub inferior: Option<f64>,
    pub izquierda: Option<f64>,
    pub derecha: Option<f64>,
}

impl MargenesCss {
    const fn uniforme(margen: f64) -> Self {
        Self {
            superior: Some(margen),
            inferior: Some(margen),
            izquierda: Some(margen),
            derecha: Some(margen),
        }
    }
}

/// Colores que la hoja impone sobre la paleta resuelta.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Colores {
    pub fondo: Option<Color>,
    pub texto: Option<Color>,
    pub cabecera: Option<Color>,
    pub codigo: Option<Color>,
    pub codigo_fondo: Option<Color>,
    pub cita: Option<Color>,
    pub enlace: Option<Color>,
    pub tabla_fondo: Option<Color>,
}

/// Valores que una hoja de estilo impone a la maquetación.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Ajustes {
    pub margenes: MargenesCss,
    pub familia: Option<FontFamily>,
    pub fuente_mono: Option<ExportMonoFont>,
    pub tamano_cuerpo: Option<f64>,
    /// Tamaño absoluto pedido para cada cabecera (índices 1..=4).
    pub tamanos_cabecera: [Option<f64>; 5],
    pub interlinea: Option<f64>,
    pub espacio_bloque: Option<f64>,
    pub colores: Colores,
    /// Propiedades que el export no sabe aplicar (sin repetir).
    pub ignoradas: Vec<String>,
}

impl Ajustes {
    /// ¿La hoja no impone nada? Entonces el export no cambia de aspecto.
    pub fn es_vacio(&self) -> bool {
        *self == Self::default()
    }
}

/// Lee y analiza una hoja de estilo.
pub fn cargar(ruta: &std::path::Path) -> std::io::Result<Ajustes> {
    let css = std::fs::read_to_string(ruta)?;
    Ok(analizar(&css))
}

/// Analiza una hoja de estilo y devuelve los valores aplicables.
pub fn analizar(css: &str) -> Ajustes {
    let mut ajustes = Ajustes::default();
    for (preludio, cuerpo) in reglas(&sin_comentarios(css)) {
        let preludio = preludio.trim();
        if preludio.is_empty() {
            continue;
        }
        if preludio.starts_with('@') {
            if preludio.eq_ignore_ascii_case("@page") {
                for (prop, valor) in declaraciones(&cuerpo) {
                    aplicar_pagina(&mut ajustes, &prop, &valor);
                }
            }
            continue;
        }
        let elementos: Vec<Elemento> = preludio.split(',').filter_map(elemento).collect();
        if elementos.is_empty() {
            continue;
        }
        for (prop, valor) in declaraciones(&cuerpo) {
            let mut aplicada = false;
            for el in &elementos {
                aplicada |= aplicar_regla(&mut ajustes, *el, &prop, &valor);
            }
            if !aplicada && !ajustes.ignoradas.contains(&prop) {
                ajustes.ignoradas.push(prop);
            }
        }
    }
    ajustes
}

/// Elementos del documento que la hoja puede estilizar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Elemento {
    Raiz,
    Parrafo,
    Cabecera(u8),
    Codigo,
    Cita,
    Enlace,
    Tabla,
}

/// Quita los comentarios `/* ... */` y devuelve el resto como caracteres.
fn sin_comentarios(css: &str) -> Vec<char> {
    let mut salida = Vec::with_capacity(css.len());
    let mut i = 0;
    let chars: Vec<char> = css.chars().collect();
    while i < chars.len() {
        if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                i += 1;
            }
            i = (i + 2).min(chars.len());
            continue;
        }
        salida.push(chars[i]);
        i += 1;
    }
    salida
}

/// Trocea la hoja en pares (preludio, cuerpo de la regla).
fn reglas(css: &[char]) -> Vec<(String, String)> {
    let mut salida = Vec::new();
    let mut preludio = String::new();
    let mut i = 0;
    while i < css.len() {
        match css[i] {
            '{' => {
                let (cuerpo, fin) = hasta_llave(css, i + 1);
                salida.push((preludio.trim().to_string(), cuerpo));
                preludio.clear();
                i = fin;
            }
            '}' | ';' => {
                preludio.clear();
                i += 1;
            }
            c => {
                preludio.push(c);
                i += 1;
            }
        }
    }
    salida
}

/// Cuerpo de la regla que empieza en `desde`, respetando llaves anidadas.
fn hasta_llave(css: &[char], desde: usize) -> (String, usize) {
    let mut nivel = 1;
    let mut cuerpo = String::new();
    let mut i = desde;
    while i < css.len() {
        match css[i] {
            '{' => {
                nivel += 1;
                cuerpo.push('{');
            }
            '}' => {
                nivel -= 1;
                if nivel == 0 {
                    return (cuerpo, i + 1);
                }
                cuerpo.push('}');
            }
            c => cuerpo.push(c),
        }
        i += 1;
    }
    (cuerpo, i)
}

/// Declaraciones `propiedad: valor` de un cuerpo de regla.
fn declaraciones(cuerpo: &str) -> Vec<(String, String)> {
    cuerpo
        .split(';')
        .filter_map(|d| d.split_once(':'))
        .map(|(p, v)| (p.trim().to_ascii_lowercase(), v.trim().to_string()))
        .filter(|(p, _)| !p.is_empty())
        .collect()
}

/// Quita un `!important` final del valor.
fn sin_importante(valor: &str) -> String {
    let mut v = valor.trim().to_string();
    const MARCA: &str = "!important";
    if v.to_ascii_lowercase().ends_with(MARCA) {
        v.truncate(v.len() - MARCA.len());
    }
    v.trim().to_string()
}

/// Elemento al que apunta un selector. Un selector que no se reconoce se
/// descarta, y con él sus declaraciones.
fn elemento(selector: &str) -> Option<Elemento> {
    let ultimo = selector.split_whitespace().next_back()?;
    if ultimo.eq_ignore_ascii_case(":root") || ultimo == "*" {
        return Some(Elemento::Raiz);
    }
    // Se queda con la parte anterior a clases, ids, atributos y pseudoclases.
    let base: String = ultimo
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '-')
        .collect();
    match base.to_ascii_lowercase().as_str() {
        "html" | "body" => Some(Elemento::Raiz),
        "p" => Some(Elemento::Parrafo),
        "h1" => Some(Elemento::Cabecera(1)),
        "h2" => Some(Elemento::Cabecera(2)),
        "h3" => Some(Elemento::Cabecera(3)),
        "h4" => Some(Elemento::Cabecera(4)),
        "pre" | "code" => Some(Elemento::Codigo),
        "blockquote" => Some(Elemento::Cita),
        "a" => Some(Elemento::Enlace),
        "table" => Some(Elemento::Tabla),
        _ => None,
    }
}

/// Aplica una declaración de una regla normal. Devuelve si se ha usado.
fn aplicar_regla(ajustes: &mut Ajustes, el: Elemento, prop: &str, valor: &str) -> bool {
    let valor = sin_importante(valor);
    if prop == "--block-spacing" {
        if let Some(v) = longitud(&valor) {
            ajustes.espacio_bloque = Some(v);
            return true;
        }
        return false;
    }
    match (el, prop) {
        (Elemento::Raiz, "font-family") => {
            if let Some(f) = familia(&valor) {
                ajustes.familia = Some(f);
            }
        }
        (Elemento::Codigo, "font-family") => {
            if let Some(f) = fuente_mono(&valor) {
                ajustes.fuente_mono = Some(f);
            }
        }
        (Elemento::Raiz, "font-size") => {
            if let Some(v) = longitud(&valor).filter(|v| *v > 0.0) {
                ajustes.tamano_cuerpo = Some(v);
            }
        }
        (Elemento::Cabecera(n), "font-size") => {
            if let Some(v) = longitud(&valor).filter(|v| *v > 0.0) {
                ajustes.tamanos_cabecera[n as usize] = Some(v);
            }
        }
        (Elemento::Raiz | Elemento::Parrafo, "line-height") => {
            if let Some(v) = interlinea(&valor) {
                ajustes.interlinea = Some(v);
            }
        }
        (Elemento::Raiz | Elemento::Parrafo, "margin-bottom") => {
            if let Some(v) = longitud(&valor) {
                ajustes.espacio_bloque = Some(v);
            }
        }
        (Elemento::Raiz, "background-color") => {
            if let Some(c) = color(&valor) {
                ajustes.colores.fondo = Some(c);
            }
        }
        (Elemento::Codigo, "background-color") => {
            if let Some(c) = color(&valor) {
                ajustes.colores.codigo_fondo = Some(c);
            }
        }
        (Elemento::Tabla, "background-color") => {
            if let Some(c) = color(&valor) {
                ajustes.colores.tabla_fondo = Some(c);
            }
        }
        (_, "color") => {
            if let Some(c) = color(&valor) {
                *campo_color(&mut ajustes.colores, el) = Some(c);
            }
        }
        _ => return false,
    }
    true
}

/// Campo de la paleta que alimenta el `color` de cada elemento.
fn campo_color(colores: &mut Colores, el: Elemento) -> &mut Option<Color> {
    match el {
        Elemento::Raiz | Elemento::Parrafo | Elemento::Tabla => &mut colores.texto,
        Elemento::Cabecera(_) => &mut colores.cabecera,
        Elemento::Codigo => &mut colores.codigo,
        Elemento::Cita => &mut colores.cita,
        Elemento::Enlace => &mut colores.enlace,
    }
}

/// Aplica una declaración de `@page`.
fn aplicar_pagina(ajustes: &mut Ajustes, prop: &str, valor: &str) {
    let valor = sin_importante(valor);
    match prop {
        "margin" => {
            let valores: Vec<f64> = valor.split_whitespace().filter_map(longitud).collect();
            let lados = valor.split_whitespace().count();
            if valores.len() != lados {
                return;
            }
            ajustes.margenes = match valores.len() {
                1 => MargenesCss::uniforme(valores[0]),
                2 => MargenesCss {
                    superior: Some(valores[0]),
                    inferior: Some(valores[0]),
                    izquierda: Some(valores[1]),
                    derecha: Some(valores[1]),
                },
                3 => MargenesCss {
                    superior: Some(valores[0]),
                    inferior: Some(valores[2]),
                    izquierda: Some(valores[1]),
                    derecha: Some(valores[1]),
                },
                4 => MargenesCss {
                    superior: Some(valores[0]),
                    inferior: Some(valores[2]),
                    izquierda: Some(valores[3]),
                    derecha: Some(valores[1]),
                },
                _ => return,
            };
        }
        "margin-top" => {
            if let Some(v) = longitud(&valor) {
                ajustes.margenes.superior = Some(v);
            }
        }
        "margin-bottom" => {
            if let Some(v) = longitud(&valor) {
                ajustes.margenes.inferior = Some(v);
            }
        }
        "margin-left" => {
            if let Some(v) = longitud(&valor) {
                ajustes.margenes.izquierda = Some(v);
            }
        }
        "margin-right" => {
            if let Some(v) = longitud(&valor) {
                ajustes.margenes.derecha = Some(v);
            }
        }
        _ => {}
    }
}

/// Longitud CSS convertida a puntos. Se aceptan pt, mm, cm, in y px, además de
/// números sin unidad (como el 0 de CSS). Los valores negativos no encajan en
/// la maquetación y se descartan.
fn longitud(valor: &str) -> Option<f64> {
    let v = valor.trim().to_ascii_lowercase();
    if v.is_empty() {
        return None;
    }
    let (numero, factor) = if let Some(n) = v.strip_suffix("mm") {
        (n, PT_POR_MM)
    } else if let Some(n) = v.strip_suffix("cm") {
        (n, PT_POR_CM)
    } else if let Some(n) = v.strip_suffix("in") {
        (n, 72.0)
    } else if let Some(n) = v.strip_suffix("px") {
        (n, PT_POR_PX)
    } else if let Some(n) = v.strip_suffix("pt") {
        (n, 1.0)
    } else {
        (v.as_str(), 1.0)
    };
    let n: f64 = numero.trim().parse().ok()?;
    if !n.is_finite() || n < 0.0 {
        return None;
    }
    Some(n * factor)
}

/// Interlineado: número sin unidad o porcentaje. Fuera de un rango razonable se
/// descarta para no romper la paginación.
fn interlinea(valor: &str) -> Option<f64> {
    let v = valor.trim();
    let n = if let Some(pct) = v.strip_suffix('%') {
        pct.trim().parse::<f64>().ok()? / 100.0
    } else {
        v.parse::<f64>().ok()?
    };
    if !n.is_finite() || !(0.5..=5.0).contains(&n) {
        return None;
    }
    Some(n)
}

/// Color CSS: `#rgb`, `#rrggbb`, `rgb()`, `rgba()` o uno de los nombres básicos.
fn color(valor: &str) -> Option<Color> {
    let v = valor.trim().to_ascii_lowercase();
    if let Some(resto) = v.strip_prefix('#') {
        return color_hex(resto);
    }
    let rgb = v
        .strip_prefix("rgba(")
        .or_else(|| v.strip_prefix("rgb("))
        .and_then(|r| r.strip_suffix(')'));
    if let Some(resto) = rgb {
        let partes: Vec<&str> = resto
            .split([',', ' '])
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .collect();
        if partes.len() < 3 {
            return None;
        }
        let canal = |p: &str| -> Option<f64> {
            if let Some(pct) = p.strip_suffix('%') {
                pct.parse::<f64>().ok().map(|n| (n / 100.0).clamp(0.0, 1.0))
            } else {
                p.parse::<f64>().ok().map(|n| (n / 255.0).clamp(0.0, 1.0))
            }
        };
        return Some((canal(partes[0])?, canal(partes[1])?, canal(partes[2])?));
    }
    let (r, g, b) = nombre_color(&v)?;
    Some((r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0))
}

/// Color a partir de los dígitos hexadecimales (sin la almohadilla).
fn color_hex(hex: &str) -> Option<Color> {
    let digitos: Vec<u32> = hex
        .chars()
        .map(|c| c.to_digit(16))
        .collect::<Option<Vec<u32>>>()?;
    let (r, g, b) = match digitos.len() {
        3 | 4 => (digitos[0] * 17, digitos[1] * 17, digitos[2] * 17),
        6 | 8 => (
            digitos[0] * 16 + digitos[1],
            digitos[2] * 16 + digitos[3],
            digitos[4] * 16 + digitos[5],
        ),
        _ => return None,
    };
    Some((r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0))
}

/// Nombres de color admitidos (RGB de 8 bits).
fn nombre_color(nombre: &str) -> Option<(u32, u32, u32)> {
    let rgb = match nombre {
        "black" => (0, 0, 0),
        "white" => (255, 255, 255),
        "gray" | "grey" => (128, 128, 128),
        "silver" => (192, 192, 192),
        "maroon" => (128, 0, 0),
        "red" => (255, 0, 0),
        "orange" => (255, 165, 0),
        "green" => (0, 128, 0),
        "teal" => (0, 128, 128),
        "navy" => (0, 0, 128),
        "blue" => (0, 0, 255),
        "purple" => (128, 0, 128),
        _ => return None,
    };
    Some(rgb)
}

/// Familia tipográfica del cuerpo a partir de un `font-family`. El export solo
/// maneja tres familias, así que se busca la primera del listado que encaje.
fn familia(valor: &str) -> Option<FontFamily> {
    for candidata in valor.split(',') {
        let nombre = candidata
            .trim()
            .trim_matches(|c| c == '"' || c == '\'')
            .to_ascii_lowercase();
        let familia = match nombre.as_str() {
            "serif" | "source serif 4" | "georgia" | "times" | "times new roman" | "garamond"
            | "libertinus serif" => FontFamily::Serif,
            "sans-serif" | "sans" | "system-ui" | "cantarell" | "helvetica" | "arial"
            | "adwaita sans" | "inter" => FontFamily::Sans,
            "monospace" | "mono" | "adwaita mono" | "source code pro" | "fira mono"
            | "fira code" | "jetbrains mono" | "iosevka" | "courier" => FontFamily::Mono,
            _ => continue,
        };
        return Some(familia);
    }
    None
}

/// Fuente monoespaciada del código a partir de un `font-family`.
fn fuente_mono(valor: &str) -> Option<ExportMonoFont> {
    for candidata in valor.split(',') {
        let nombre = candidata
            .trim()
            .trim_matches(|c| c == '"' || c == '\'')
            .to_ascii_lowercase();
        match nombre.as_str() {
            "adwaita mono" | "monospace" | "mono" => return Some(ExportMonoFont::AdwaitaMono),
            "source code pro" => return Some(ExportMonoFont::SourceCodePro),
            "fira mono" | "fira code" => return Some(ExportMonoFont::FiraMono),
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_las_longitudes_se_convierten_a_puntos() {
        assert_eq!(longitud("72pt"), Some(72.0));
        assert_eq!(longitud("1in"), Some(72.0));
        assert_eq!(longitud("25.4mm"), Some(72.0));
        assert_eq!(longitud("2.54cm"), Some(72.0));
        assert_eq!(longitud("16px"), Some(12.0));
        assert_eq!(longitud("0"), Some(0.0));
        assert_eq!(longitud(" 12 PT "), Some(12.0));
    }

    #[test]
    fn css_las_longitudes_invalidas_se_descartan() {
        assert_eq!(longitud(""), None);
        assert_eq!(longitud("auto"), None);
        assert_eq!(longitud("1em"), None);
        assert_eq!(longitud("-4pt"), None);
        assert_eq!(longitud("inf"), None);
        assert_eq!(longitud("nan"), None);
    }

    #[test]
    fn css_el_margen_de_pagina_acepta_atajos_de_uno_a_cuatro_valores() {
        let uno = analizar("@page { margin: 1in }");
        assert_eq!(uno.margenes, MargenesCss::uniforme(72.0));

        let dos = analizar("@page { margin: 10mm 20mm }");
        assert_eq!(
            dos.margenes,
            MargenesCss {
                superior: Some(10.0 * PT_POR_MM),
                inferior: Some(10.0 * PT_POR_MM),
                izquierda: Some(20.0 * PT_POR_MM),
                derecha: Some(20.0 * PT_POR_MM),
            }
        );

        let tres = analizar("@page { margin: 10pt 20pt 30pt }");
        assert_eq!(tres.margenes.superior, Some(10.0));
        assert_eq!(tres.margenes.inferior, Some(30.0));
        assert_eq!(tres.margenes.izquierda, Some(20.0));
        assert_eq!(tres.margenes.derecha, Some(20.0));

        let cuatro = analizar("@page { margin: 10pt 20pt 30pt 40pt }");
        assert_eq!(cuatro.margenes.superior, Some(10.0));
        assert_eq!(cuatro.margenes.derecha, Some(20.0));
        assert_eq!(cuatro.margenes.inferior, Some(30.0));
        assert_eq!(cuatro.margenes.izquierda, Some(40.0));
    }

    #[test]
    fn css_los_margenes_por_lado_pisan_al_atajo() {
        let a = analizar("@page { margin: 1in; margin-left: 2in }");
        assert_eq!(a.margenes.izquierda, Some(144.0));
        assert_eq!(a.margenes.superior, Some(72.0));
    }

    #[test]
    fn css_la_tipografia_sale_del_cuerpo_y_de_las_cabeceras() {
        let a = analizar(
            "body { font-family: \"Source Serif 4\", serif; font-size: 12pt; line-height: 1.8 }
             h1 { font-size: 24pt; color: #123 }
             h3 { font-size: 14pt }",
        );
        assert_eq!(a.familia, Some(FontFamily::Serif));
        assert_eq!(a.tamano_cuerpo, Some(12.0));
        assert_eq!(a.interlinea, Some(1.8));
        assert_eq!(a.tamanos_cabecera[1], Some(24.0));
        assert_eq!(a.tamanos_cabecera[3], Some(14.0));
        assert_eq!(a.tamanos_cabecera[2], None);
        assert_eq!(
            a.colores.cabecera,
            Some((17.0 / 255.0, 34.0 / 255.0, 51.0 / 255.0))
        );
    }

    #[test]
    fn css_el_interlineado_admite_porcentaje_y_descarta_absurdos() {
        assert_eq!(analizar("p { line-height: 150% }").interlinea, Some(1.5));
        assert_eq!(analizar("p { line-height: 0 }").interlinea, None);
        assert_eq!(analizar("p { line-height: 12 }").interlinea, None);
        assert_eq!(analizar("p { line-height: auto }").interlinea, None);
    }

    #[test]
    fn css_el_espaciado_entre_bloques_sale_de_margin_bottom_o_de_la_variable() {
        assert_eq!(
            analizar("p { margin-bottom: 6pt }").espacio_bloque,
            Some(6.0)
        );
        assert_eq!(
            analizar("body { --block-spacing: 4mm }").espacio_bloque,
            Some(4.0 * PT_POR_MM)
        );
    }

    #[test]
    fn css_los_colores_por_elemento_van_a_su_campo() {
        let a = analizar(
            "body { color: #222; background-color: white }
             p { color: rgb(10, 20, 30) }
             pre { color: #333; background-color: #f5f5f5 }
             blockquote { color: #444 }
             a { color: blue }
             table { background-color: #eee }",
        );
        assert_eq!(a.colores.fondo, Some((1.0, 1.0, 1.0)));
        assert_eq!(
            a.colores.texto,
            Some((10.0 / 255.0, 20.0 / 255.0, 30.0 / 255.0))
        );
        assert_eq!(a.colores.codigo, Some((0.2, 0.2, 0.2)));
        assert_eq!(
            a.colores.codigo_fondo,
            Some((245.0 / 255.0, 245.0 / 255.0, 245.0 / 255.0))
        );
        assert_eq!(
            a.colores.cita,
            Some((68.0 / 255.0, 68.0 / 255.0, 68.0 / 255.0))
        );
        assert_eq!(a.colores.enlace, Some((0.0, 0.0, 1.0)));
        assert_eq!(
            a.colores.tabla_fondo,
            Some((238.0 / 255.0, 238.0 / 255.0, 238.0 / 255.0))
        );
    }

    #[test]
    fn css_los_colores_invalidos_se_descartan() {
        assert_eq!(color("#zzz"), None);
        assert_eq!(color("#12345"), None);
        assert_eq!(color("rgb(1, 2)"), None);
        assert_eq!(color("chartreuse"), None);
        assert_eq!(color(""), None);
        assert_eq!(color("#fff"), Some((1.0, 1.0, 1.0)));
        assert_eq!(color("rgba(255, 0, 0, 0.5)"), Some((1.0, 0.0, 0.0)));
    }

    #[test]
    fn css_la_familia_monoespaciada_elige_la_fuente_del_codigo() {
        assert_eq!(
            analizar("code { font-family: \"Source Code Pro\", monospace }").fuente_mono,
            Some(ExportMonoFont::SourceCodePro)
        );
        assert_eq!(
            analizar("pre { font-family: Fira Code }").fuente_mono,
            Some(ExportMonoFont::FiraMono)
        );
        assert_eq!(analizar("pre { font-family: Papyrus }").fuente_mono, None);
    }

    #[test]
    fn css_los_selectores_con_clase_o_pseudoclase_cuentan_por_su_elemento() {
        let a = analizar("p.nota:hover { color: black } h2:first-child { font-size: 18pt }");
        assert_eq!(a.colores.texto, Some((0.0, 0.0, 0.0)));
        assert_eq!(a.tamanos_cabecera[2], Some(18.0));
    }

    #[test]
    fn css_los_comentarios_y_las_arrobas_se_ignoran() {
        let a = analizar(
            "/* body { color: red } */
             @charset \"utf-8\";
             @media print { body { color: red } }
             body { color: black }",
        );
        assert_eq!(a.colores.texto, Some((0.0, 0.0, 0.0)));
        assert_eq!(a.colores.fondo, None);
    }

    #[test]
    fn css_las_propiedades_desconocidas_se_apuntan_una_sola_vez() {
        let a = analizar(
            "p { padding: 4pt; padding: 8pt; font-variant: small-caps }
             body { color: black }",
        );
        assert_eq!(
            a.ignoradas,
            vec!["padding".to_string(), "font-variant".to_string()]
        );
    }

    #[test]
    fn css_un_fichero_vacio_no_impone_nada() {
        assert!(analizar("").es_vacio());
        assert!(analizar("   /* nada */  ").es_vacio());
        assert!(analizar("section { color: red }").es_vacio());
        assert!(!analizar("body { color: black }").es_vacio());
    }

    #[test]
    fn css_los_selectores_compuestos_usan_el_ultimo_elemento() {
        assert_eq!(elemento("body > p"), Some(Elemento::Parrafo));
        assert_eq!(elemento("html body"), Some(Elemento::Raiz));
        assert_eq!(elemento("*"), Some(Elemento::Raiz));
        assert_eq!(elemento("table td"), None);
        assert_eq!(elemento(""), None);
    }
}
