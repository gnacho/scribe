//! Exportación del documento a PDF reutilizando el análisis de tramos y
//! adornos de `markdown_render` (issue #24).
//!
//! La salida se produce a través del pipeline de impresión de GTK
//! ([`gtk4::PrintOperation`]): el usuario puede imprimir en papel o «imprimir
//! en fichero» (PDF) desde el mismo diálogo. El render es texto temático: cada
//! bloque del documento (cabeceras, bloques de código, tablas, citas, listas,
//! reglas, imágenes) se pinta con su tipografía y color según el tema de
//! exportación elegido, reutilizando los tramos (`Span`) y ornamentos
//! (`Ornament`) que ya calcula el editor para su vista en vivo.
//!
//! # Diseño de paginación
//!
//! La paginación es una **función pura** ([`planificar_paginas`]) que trabaja
//! sobre una lista de [`Bloque`]s con su altura ya medida. Las reglas:
//!
//! - una cabecera no puede quedar como último bloque de una página (huerfana):
//!   baja a la siguiente junto con su contenido;
//! - un bloque de código, tabla, cita o imagen no se parte si cabe entero en
//!   la página restante;
//! - una línea `\newpage` (o un `<div>` con `page-break-after: always`)
//!   fuerza un salto de página.
//!
//! Las alturas se miden con el contexto de cairo del `PrintContext` en
//! `begin_print` (una sola pasada) y se cortan con la función pura sobre las
//! alturas reales. `draw_page` solo pinta la página ya planificada.
//!
//! # Limitaciones conocidas (v1)
//!
//! - **Sin índice/outline del PDF**: `GtkPrintOperation` no expone marcadores
//!   PDF, así que el PDF no lleva tabla de contenidos ni outline.
//! - **Render con la API de texto de cairo**, no con `PangoLayout`: dibujar un
//!   `PangoLayout` sobre cairo exige el crate `pangocairo` (no incluido y no
//!   añadido por la restricción «sin dependencias nuevas»). El ajuste de
//!   línea es un word-wrap simple y no hay ligaduras ni shaping de scripts
//!   complejos; para texto latino es suficiente.
//! - Un bloque de código más alto que una página entera se dibuja recortado:
//!   su continuación a la siguiente página se pierde (caso patológico).
//! - Las listas conservan el marcador del fuente (`-`, `1.`, `- [ ]`); las
//!   tablas se pintan en monoespaciada con la fila de cabecera en negrita y
//!   sin rejilla de celdas (el ancho fijo conserva la alineación).

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use gtk4::cairo;
use gtk4::prelude::*;

use crate::markdown_render::{analyze, Analysis, Ornament, SpanKind};
use crate::settings::{
    AppSettings, ExportColorScheme, ExportMonoFont, ExportOrientation, ExportPageSize, FontFamily,
};

// ---------------------------------------------------------------------------
// Constantes de maquetación
// ---------------------------------------------------------------------------

/// Altura de línea relativa al tamaño del bloque (cuerpo = 1.4).
const INTERLINEA: f64 = 1.4;
/// Separación vertical entre bloques consecutivos (puntos).
const ESPACIO_BLOQUE: f64 = 10.0;
/// Padding interior de las cajas (bloques de código y tablas).
const PADDING_CAJA: f64 = 6.0;
/// Alto de un bloque de regla horizontal.
const REGLA_ALTO: f64 = 14.0;
/// Alto del placeholder de una imagen.
const IMAGEN_ALTO: f64 = 42.0;
/// Sangrado horizontal por nivel de lista (puntos).
const INDENT_LISTA: f64 = 18.0;
/// Sangrado de la barra de cita (puntos).
const INDENT_CITA: f64 = 14.0;

// ---------------------------------------------------------------------------
// Configuración de exportación
// ---------------------------------------------------------------------------

/// Ajustes de exportación ya resueltos, independientes de GSettings.
#[derive(Debug, Clone)]
pub struct ExportConfig {
    pub esquema: ExportColorScheme,
    pub familia_cuerpo: FontFamily,
    /// Tamaño del cuerpo en puntos.
    pub tamano_cuerpo: f64,
    pub fuente_mono: ExportMonoFont,
    pub papel: ExportPageSize,
    pub orientacion: ExportOrientation,
}

impl ExportConfig {
    /// Construye la configuración a partir de las claves de exportación de
    /// GSettings (independientes de las de lectura).
    pub fn desde_settings(s: &AppSettings) -> Self {
        Self {
            esquema: s.export_color_scheme(),
            familia_cuerpo: s.export_font_family(),
            tamano_cuerpo: s.export_font_size() as f64,
            fuente_mono: s.export_mono_font(),
            papel: s.export_page_size(),
            orientacion: s.export_orientation(),
        }
    }
}

// ---------------------------------------------------------------------------
// Paleta de colores
// ---------------------------------------------------------------------------

/// Paleta resuelta (claro, oscuro o monocromo). Colores en RGB 0..1.
#[derive(Debug, Clone, Copy)]
struct Paleta {
    fondo: (f64, f64, f64),
    texto: (f64, f64, f64),
    cabecera: (f64, f64, f64),
    codigo: (f64, f64, f64),
    codigo_fondo: (f64, f64, f64),
    cita: (f64, f64, f64),
    cita_barra: (f64, f64, f64),
    enlace: (f64, f64, f64),
    regla: (f64, f64, f64),
    tabla_fondo: (f64, f64, f64),
    atenuado: (f64, f64, f64),
}

fn hex(hex: u32) -> (f64, f64, f64) {
    let r = ((hex >> 16) & 0xff) as f64 / 255.0;
    let g = ((hex >> 8) & 0xff) as f64 / 255.0;
    let b = (hex & 0xff) as f64 / 255.0;
    (r, g, b)
}

/// Resuelve la paleta a partir del esquema elegido y del tema del sistema.
/// Los colores de claro/oscuro siguen los de la vista del editor
/// (`editor::apply_theme`).
fn resolver_paleta(esquema: ExportColorScheme, sistema_oscuro: bool) -> Paleta {
    let oscuro = match esquema {
        ExportColorScheme::System => sistema_oscuro,
        ExportColorScheme::Dark => true,
        ExportColorScheme::Light | ExportColorScheme::Mono => false,
    };
    if esquema == ExportColorScheme::Mono {
        let negro = (0.0, 0.0, 0.0);
        let blanco = (1.0, 1.0, 1.0);
        Paleta {
            fondo: blanco,
            texto: negro,
            cabecera: negro,
            codigo: negro,
            codigo_fondo: (0.95, 0.95, 0.95),
            cita: negro,
            cita_barra: negro,
            enlace: negro,
            regla: negro,
            tabla_fondo: (0.95, 0.95, 0.95),
            atenuado: (0.4, 0.4, 0.4),
        }
    } else if oscuro {
        Paleta {
            fondo: hex(0x1e1e1e),
            texto: hex(0xe6e6e6),
            cabecera: hex(0xffffff),
            codigo: hex(0xf0a868),
            codigo_fondo: hex(0x2d2d2d),
            cita: hex(0xc2c2c2),
            cita_barra: hex(0x6f6f6f),
            enlace: hex(0x82b8f0),
            regla: hex(0x4a4a4a),
            tabla_fondo: hex(0x282828),
            atenuado: hex(0x8f8f8f),
        }
    } else {
        Paleta {
            fondo: hex(0xffffff),
            texto: hex(0x1a1a1a),
            cabecera: hex(0x0f0f0f),
            codigo: hex(0xa34a00),
            codigo_fondo: hex(0xf6f8fa),
            cita: hex(0x54535a),
            cita_barra: hex(0xc0bfbc),
            enlace: hex(0x1a6ed8),
            regla: hex(0xc0bfbc),
            tabla_fondo: hex(0xf8f8f8),
            atenuado: hex(0x8b8a88),
        }
    }
}

// ---------------------------------------------------------------------------
// Estilos inline y bloques
// ---------------------------------------------------------------------------

/// Estilo inline de un tramo (combinable por flags).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Estilo {
    pub negrita: bool,
    pub cursiva: bool,
    pub tachado: bool,
    pub codigo: bool,
    pub enlace: bool,
}

/// Frase estilada de una línea (texto + estilo inline).
#[derive(Debug, Clone)]
pub struct Frase {
    pub texto: String,
    pub estilo: Estilo,
}

/// Tipo de bloque, para las reglas de paginación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TipoBloque {
    /// Párrafo (una línea de texto, quizá un elemento de lista).
    #[default]
    Parrafo,
    Cabecera {
        nivel: u8,
    },
    Codigo,
    Tabla,
    Cita,
    Regla,
    Imagen,
    /// Salto de página forzado (`\newpage`): sin contenido.
    Salto,
}

/// Palabra ya medida dentro de una línea visual.
#[derive(Debug, Clone, Default)]
pub struct Palabra {
    pub texto: String,
    pub estilo: Estilo,
    pub ancho: f64,
}

/// Fragmento del documento listo para paginar y dibujar. La función pura de
/// paginación solo lee `tipo` y `alto`; el resto es el contenido para el
/// dibujado.
#[derive(Debug, Clone, Default)]
pub struct Bloque {
    pub tipo: TipoBloque,
    /// Alto total en puntos, incluida su separación inferior.
    pub alto: f64,
    /// Líneas de contenido ya limpias (marcas de bloque eliminadas).
    pub lineas: Vec<String>,
    /// Frases estiladas (para párrafos y cabeceras de una línea).
    pub frases: Vec<Frase>,
    /// Nivel de lista (0 = no es lista; 1..3 = anidamiento).
    pub nivel_lista: u8,
    /// (alt, dest) si el bloque es una imagen.
    pub imagen: Option<(String, String)>,
    /// Líneas visuales ya envueltas y medidas (rellenado en `begin_print`).
    pub envuelto: Vec<Vec<Palabra>>,
}

impl Bloque {
    /// Constructor mínimo para los tests de paginación.
    #[cfg(test)]
    pub fn nuevo(tipo: TipoBloque, alto: f64) -> Self {
        Self {
            tipo,
            alto,
            ..Default::default()
        }
    }
}

/// Una página: lista de índices de bloques del documento.
#[derive(Debug, Clone, PartialEq)]
pub struct Pagina {
    pub bloques: Vec<usize>,
}

/// ¿Es esta línea un salto de página explícito?
///
/// Reconoce `\newpage` (con posibles espacios alrededor) y el div
/// `page-break-after: always` (insensible a mayúsculas), que es la forma que
/// usan algunos exportadores HTML.
fn es_salto_pagina(linea: &str) -> bool {
    let t = linea.trim();
    if t == "\\newpage" {
        return true;
    }
    let lower = t.to_ascii_lowercase();
    lower.contains("page-break-after") && lower.contains("always")
}

// ---------------------------------------------------------------------------
// Análisis inline (escáner simple, no anidado)
// ---------------------------------------------------------------------------

/// Separa el marcador de un elemento de lista del resto de la línea.
/// Devuelve (marcador, resto) o `None` si la línea no es un elemento de lista.
/// El marcador conserva el texto del fuente (`- `, `1. `, `- [x] `).
fn separar_marcador_lista(linea: &str) -> Option<(&str, &str)> {
    let t = linea.trim_start();
    let lead = linea.len() - t.len();
    let (marcador, resto) = if let Some(r) = t
        .strip_prefix("- [ ] ")
        .or_else(|| t.strip_prefix("- [x] "))
    {
        (&t[..6], r)
    } else if let Some(r) = t
        .strip_prefix("- ")
        .or_else(|| t.strip_prefix("* "))
        .or_else(|| t.strip_prefix("+ "))
    {
        (&t[..2], r)
    } else if let Some((n, r)) = prefijo_numero(t) {
        (&t[..n], r)
    } else {
        return None;
    };
    // El sangrado del fuente se descarta: lo da `nivel_lista`.
    Some((&linea[lead..lead + marcador.len()], resto))
}

/// Prefijo `N. ` o `N) ` de una lista ordenada. Devuelve (longitud, resto).
fn prefijo_numero(t: &str) -> Option<(usize, &str)> {
    let n = t.bytes().take_while(|b| b.is_ascii_digit()).count();
    if n == 0 {
        return None;
    }
    let resto = &t[n..];
    if resto.starts_with(". ") || resto.starts_with(") ") {
        Some((n + 2, &resto[2..]))
    } else {
        None
    }
}

/// Busca un cierre delimitado: dado `s` que empieza por `abrir`, devuelve el
/// interior y el avance hasta justo después del `cerrar`.
fn parsear_delimitado(s: &str, abrir: &str, cerrar: &str) -> Option<(String, usize)> {
    let resto = s.strip_prefix(abrir)?;
    let idx = resto.find(cerrar)?;
    Some((resto[..idx].to_string(), abrir.len() + idx + cerrar.len()))
}

/// Enlace inline `[texto](url)`. Solo enlaces con URL (`http`/`https`); el
/// resto (referencias, autolinks) se deja como texto plano en v1.
fn parsear_enlace(s: &str) -> Option<(String, usize)> {
    let cierre = s[1..].find(']')? + 1;
    let etiqueta = &s[1..cierre];
    let resto = &s[cierre + 1..];
    if !resto.starts_with("(http") {
        return None;
    }
    let cierre_paren = resto.find(')')?;
    Some((etiqueta.to_string(), cierre + 1 + cierre_paren + 1))
}

fn flush(frases: &mut Vec<Frase>, actual: &mut String, estilo: Estilo) {
    if !actual.is_empty() {
        frases.push(Frase {
            texto: std::mem::take(actual),
            estilo,
        });
    }
}

/// Escanea una línea en busca de estilos inline (`**negrita**`, `*cursiva*`,
/// `` `código` ``, `~~tachado~~`, `[texto](url)`) y devuelve frases estiladas.
/// Escáner simple, no anidado: la primera construcción que cierra gana.
fn segmentar_inline(texto: &str) -> Vec<Frase> {
    let mut frases = Vec::new();
    let mut actual = String::new();
    let b = texto.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'[' {
            if let Some((etiqueta, avance)) = parsear_enlace(&texto[i..]) {
                flush(&mut frases, &mut actual, Estilo::default());
                frases.push(Frase {
                    texto: etiqueta,
                    estilo: Estilo {
                        enlace: true,
                        ..Default::default()
                    },
                });
                i += avance;
                continue;
            }
        }
        if b[i] == b'`' {
            if let Some((cod, avance)) = parsear_delimitado(&texto[i..], "`", "`") {
                flush(&mut frases, &mut actual, Estilo::default());
                frases.push(Frase {
                    texto: cod,
                    estilo: Estilo {
                        codigo: true,
                        ..Default::default()
                    },
                });
                i += avance;
                continue;
            }
        }
        if texto[i..].starts_with("**") {
            if let Some((txt, avance)) = parsear_delimitado(&texto[i..], "**", "**") {
                flush(&mut frases, &mut actual, Estilo::default());
                frases.push(Frase {
                    texto: txt,
                    estilo: Estilo {
                        negrita: true,
                        ..Default::default()
                    },
                });
                i += avance;
                continue;
            }
        }
        if texto[i..].starts_with("~~") {
            if let Some((txt, avance)) = parsear_delimitado(&texto[i..], "~~", "~~") {
                flush(&mut frases, &mut actual, Estilo::default());
                frases.push(Frase {
                    texto: txt,
                    estilo: Estilo {
                        tachado: true,
                        ..Default::default()
                    },
                });
                i += avance;
                continue;
            }
        }
        if b[i] == b'*' || b[i] == b'_' {
            if let Some((txt, avance)) =
                parsear_delimitado(&texto[i..], &texto[i..i + 1], &texto[i..i + 1])
            {
                flush(&mut frases, &mut actual, Estilo::default());
                frases.push(Frase {
                    texto: txt,
                    estilo: Estilo {
                        cursiva: true,
                        ..Default::default()
                    },
                });
                i += avance;
                continue;
            }
        }
        let ch = texto[i..].chars().next().unwrap();
        actual.push(ch);
        i += ch.len_utf8();
    }
    flush(&mut frases, &mut actual, Estilo::default());
    frases
}

// ---------------------------------------------------------------------------
// Construcción de bloques a partir del análisis
// ---------------------------------------------------------------------------

/// Líneas del documento como rangos de bytes (sin `\n` ni `\r`).
fn lineas_doc(texto: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut inicio = 0;
    for linea in texto.split('\n') {
        let contenido = linea.strip_suffix('\r').unwrap_or(linea);
        out.push((inicio, inicio + contenido.len()));
        inicio += linea.len() + 1;
    }
    if texto.ends_with('\n') {
        out.pop();
    }
    if out.is_empty() {
        out.push((0, 0));
    }
    out
}

/// Línea (índice) que contiene el byte `offset`.
fn linea_de(lineas: &[(usize, usize)], offset: usize) -> Option<usize> {
    lineas.iter().position(|&(a, b)| offset >= a && offset < b)
}

/// Quita de una línea de cabecera las almohadillas (`#`) y el espacio que las
/// sigue. Si no hay almohadillas (setext) devuelve la línea tal cual.
fn limpiar_cabecera(linea: &str) -> &str {
    let t = linea.trim_start();
    let hashes = t.bytes().take_while(|b| *b == b'#').count();
    if hashes > 0 && hashes <= 6 {
        let mut rest = &t[hashes..];
        rest = rest.trim_start();
        return rest;
    }
    linea.trim_end()
}

/// ¿Es esta línea el subrayado `===` o `---` de una cabecera setext?
fn es_subrayado_setext(linea: &str) -> bool {
    let t = linea.trim();
    !t.is_empty() && (t.bytes().all(|b| b == b'=') || t.bytes().all(|b| b == b'-'))
}

fn construir_bloques(texto: &str, analisis: &Analysis) -> Vec<Bloque> {
    let lineas = lineas_doc(texto);
    let n = lineas.len();

    // Rangos de bloques de varias líneas, ordenados.
    let mut codigo: Vec<(usize, usize)> = Vec::new();
    let mut tablas: Vec<(usize, usize)> = Vec::new();
    let mut citas: Vec<(usize, usize)> = Vec::new();
    let mut reglas: Vec<usize> = Vec::new();
    let mut imagenes: std::collections::HashMap<usize, (String, String)> =
        std::collections::HashMap::new();
    let mut cabeceras: std::collections::HashMap<usize, u8> = std::collections::HashMap::new();
    let mut listas: std::collections::HashMap<usize, u8> = std::collections::HashMap::new();

    for o in &analisis.ornaments {
        match o {
            Ornament::CodeBlock { first, last } => codigo.push((*first, *last)),
            Ornament::Table { first, last } => tablas.push((*first, *last)),
            Ornament::Quote { first, last } => citas.push((*first, *last)),
            Ornament::Rule { line } => reglas.push(*line),
            Ornament::Image { line, dest, alt } => {
                imagenes.insert(*line, (alt.clone(), dest.clone()));
            }
            _ => {}
        }
    }
    codigo.sort_unstable();
    tablas.sort_unstable();
    citas.sort_unstable();

    for s in &analisis.spans {
        let nivel_cabecera = match s.tag {
            "h1" => Some(1u8),
            "h2" => Some(2),
            "h3" => Some(3),
            "h4" => Some(4),
            "h5" => Some(5),
            "h6" => Some(6),
            _ => None,
        };
        let nivel_lista = match s.tag {
            "li1" => Some(1u8),
            "li2" => Some(2),
            "li3" => Some(3),
            _ => None,
        };
        if s.kind == SpanKind::Style && (nivel_cabecera.is_some() || nivel_lista.is_some()) {
            if let Some(line) = linea_de(&lineas, s.start) {
                if let Some(nv) = nivel_cabecera {
                    cabeceras.entry(line).or_insert(nv);
                }
                if let Some(nv) = nivel_lista {
                    listas.entry(line).or_insert(nv);
                }
            }
        }
    }

    let cubre = |rangos: &[(usize, usize)], i: usize| -> Option<(usize, usize)> {
        rangos.iter().copied().find(|&(a, b)| i >= a && i <= b)
    };

    let mut bloques: Vec<Bloque> = Vec::new();
    let mut i = 0;
    while i < n {
        let (ls, le) = lineas[i];
        let linea = &texto[ls..le];

        // Saltos de página forzados.
        if es_salto_pagina(linea) {
            bloques.push(Bloque {
                tipo: TipoBloque::Salto,
                ..Default::default()
            });
            i += 1;
            continue;
        }

        // Bloques de varias líneas (código, tabla, cita).
        if let Some((a, b)) = cubre(&codigo, i) {
            let contenido: Vec<String> = lineas[a..=b.min(n - 1)]
                .iter()
                .map(|&(lsj, lej)| texto[lsj..lej].to_string())
                .collect();
            // Quita las vallas de apertura y cierre.
            let contenido = quitar_vallas(&contenido);
            bloques.push(Bloque {
                tipo: TipoBloque::Codigo,
                lineas: contenido,
                ..Default::default()
            });
            i = b + 1;
            continue;
        }
        if let Some((a, b)) = cubre(&tablas, i) {
            let lineas_bloque: Vec<String> = lineas[a..=b.min(n - 1)]
                .iter()
                .map(|&(lsj, lej)| texto[lsj..lej].to_string())
                .filter(|l| !es_fila_delimitadora(l))
                .collect();
            bloques.push(Bloque {
                tipo: TipoBloque::Tabla,
                lineas: lineas_bloque,
                ..Default::default()
            });
            i = b + 1;
            continue;
        }
        if let Some((a, b)) = cubre(&citas, i) {
            let lineas_bloque: Vec<String> = lineas[a..=b.min(n - 1)]
                .iter()
                .map(|&(lsj, lej)| quitar_marca_cita(&texto[lsj..lej]))
                .collect();
            bloques.push(Bloque {
                tipo: TipoBloque::Cita,
                lineas: lineas_bloque,
                ..Default::default()
            });
            i = b + 1;
            continue;
        }

        // Regla horizontal.
        if reglas.contains(&i) {
            bloques.push(Bloque {
                tipo: TipoBloque::Regla,
                ..Default::default()
            });
            i += 1;
            continue;
        }

        // Imagen.
        if let Some((alt, dest)) = imagenes.get(&i).cloned() {
            bloques.push(Bloque {
                tipo: TipoBloque::Imagen,
                imagen: Some((alt, dest)),
                ..Default::default()
            });
            i += 1;
            continue;
        }

        // Cabecera.
        if let Some(&nivel) = cabeceras.get(&i) {
            let texto_cabecera = limpiar_cabecera(linea).to_string();
            let frases = segmentar_inline(&texto_cabecera);
            let setext = !linea.trim_start().starts_with('#');
            bloques.push(Bloque {
                tipo: TipoBloque::Cabecera { nivel },
                frases,
                ..Default::default()
            });
            // La cabecera setext consume su subrayado en la línea siguiente.
            if setext && i + 1 < n {
                let (lsn, len) = lineas[i + 1];
                if es_subrayado_setext(&texto[lsn..len]) {
                    i += 1;
                }
            }
            i += 1;
            continue;
        }

        // Párrafo (quizá elemento de lista).
        let nivel_lista = listas.get(&i).copied().unwrap_or(0);
        let frases = if nivel_lista > 0 {
            match separar_marcador_lista(linea) {
                Some((marcador, resto)) => {
                    let mut frases = vec![Frase {
                        texto: marcador.to_string(),
                        estilo: Estilo::default(),
                    }];
                    frases.extend(segmentar_inline(resto));
                    frases
                }
                None => segmentar_inline(linea),
            }
        } else {
            segmentar_inline(linea)
        };
        bloques.push(Bloque {
            tipo: TipoBloque::Parrafo,
            nivel_lista,
            frases,
            ..Default::default()
        });
        i += 1;
    }

    bloques
}

/// Quita las vallas de apertura y cierre de un bloque de código.
fn quitar_vallas(lineas: &[String]) -> Vec<String> {
    let mut out = lineas.to_vec();
    if out.first().map(|l| es_valla(l)).unwrap_or(false) {
        out.remove(0);
    }
    if out
        .last()
        .map(|l| es_valla(l.trim_start()))
        .unwrap_or(false)
    {
        out.pop();
    }
    out
}

fn es_valla(linea: &str) -> bool {
    let t = linea.trim_start();
    t.starts_with("```") || t.starts_with("~~~")
}

/// Quita el marcador `>` (y su espacio) del principio de una línea de cita.
fn quitar_marca_cita(linea: &str) -> String {
    let t = linea.trim_start();
    let lead = linea.len() - t.len();
    let t = t
        .strip_prefix('>')
        .map(|r| r.strip_prefix(' ').unwrap_or(r))
        .unwrap_or(t);
    let mut out = String::new();
    out.push_str(&linea[..lead]);
    out.push_str(t);
    out
}

/// ¿Es la fila de guiones que separa la cabecera de una tabla?
fn es_fila_delimitadora(linea: &str) -> bool {
    let t = linea.trim();
    !t.is_empty() && t.contains('-') && t.bytes().all(|b| matches!(b, b'|' | b'-' | b':' | b' '))
}

// ---------------------------------------------------------------------------
// Paginación (función pura)
// ---------------------------------------------------------------------------

/// Reparte los bloques en páginas respetando la etiqueta de paginación.
///
/// Reglas:
/// - un [`TipoBloque::Salto`] cierra la página actual y no aporta contenido;
/// - una cabecera no puede quedar como último bloque de una página (huerfana):
///   baja a la siguiente junto con su contenido;
/// - un bloque de código/tabla/cita/imagen no se parte si cabe entero en el
///   espacio restante (si es más alto que una página, se dibuja recortado).
pub fn planificar_paginas(bloques: &[Bloque], alto_util: f64) -> Vec<Pagina> {
    let mut paginas: Vec<Pagina> = Vec::new();
    let mut actual: Vec<usize> = Vec::new();
    let mut usado = 0.0f64;

    for (i, b) in bloques.iter().enumerate() {
        if b.tipo == TipoBloque::Salto {
            if !actual.is_empty() {
                paginas.push(Pagina {
                    bloques: std::mem::take(&mut actual),
                });
                usado = 0.0;
            }
            continue;
        }

        let cabe = b.alto <= alto_util - usado;
        let es_cabecera = matches!(b.tipo, TipoBloque::Cabecera { .. });
        // Cabecera huerfana: cabe, pero el bloque siguiente no cabría tras
        // ella, así que la cabecera bajaría sola al pie de página.
        let huerfana = es_cabecera
            && i + 1 < bloques.len()
            && cabe
            && b.alto + bloques[i + 1].alto > alto_util - usado;

        if (!cabe || huerfana) && !actual.is_empty() {
            paginas.push(Pagina {
                bloques: std::mem::take(&mut actual),
            });
            usado = 0.0;
        }
        actual.push(i);
        usado += b.alto;
    }

    if !actual.is_empty() {
        paginas.push(Pagina { bloques: actual });
    }
    if paginas.is_empty() {
        paginas.push(Pagina {
            bloques: Vec::new(),
        });
    }
    paginas
}

// ---------------------------------------------------------------------------
// Medición y dibujado con cairo
// ---------------------------------------------------------------------------

fn familia_cuerpo(ff: FontFamily) -> &'static str {
    match ff {
        FontFamily::Sans => "Cantarell",
        FontFamily::Serif => "Source Serif 4",
        FontFamily::Mono => "Adwaita Mono",
    }
}

/// Escala de las cabeceras (misma proporción que la vista del editor).
fn escala_cabecera(nivel: u8) -> f64 {
    match nivel {
        1 => 1.9,
        2 => 1.5,
        3 => 1.25,
        4 => 1.1,
        _ => 1.0,
    }
}

/// Familia y tamaño de fuente para un estilo dado, a partir del tamaño del
/// bloque. El código usa la monoespaciada al 90% del cuerpo (no del bloque).
fn fuente_para(config: &ExportConfig, estilo: Estilo, tamano_bloque: f64) -> (&'static str, f64) {
    if estilo.codigo {
        (config.fuente_mono.family(), config.tamano_cuerpo * 0.9)
    } else {
        (familia_cuerpo(config.familia_cuerpo), tamano_bloque)
    }
}

/// Tamaño y estilo base de un bloque.
fn tamano_y_estilo(tipo: TipoBloque, config: &ExportConfig) -> (f64, Estilo) {
    match tipo {
        TipoBloque::Cabecera { nivel } => (
            config.tamano_cuerpo * escala_cabecera(nivel),
            Estilo {
                negrita: true,
                ..Default::default()
            },
        ),
        TipoBloque::Codigo => (
            config.tamano_cuerpo * 0.9,
            Estilo {
                codigo: true,
                ..Default::default()
            },
        ),
        TipoBloque::Tabla => (
            config.tamano_cuerpo * 0.9,
            Estilo {
                codigo: true,
                ..Default::default()
            },
        ),
        TipoBloque::Cita => (
            config.tamano_cuerpo,
            Estilo {
                cursiva: true,
                ..Default::default()
            },
        ),
        _ => (config.tamano_cuerpo, Estilo::default()),
    }
}

fn seleccionar_fuente(
    cr: &cairo::Context,
    config: &ExportConfig,
    estilo: Estilo,
    tamano_base: f64,
) {
    let (familia, tamano) = fuente_para(config, estilo, tamano_base);
    let inclinacion = if estilo.cursiva {
        cairo::FontSlant::Italic
    } else {
        cairo::FontSlant::Normal
    };
    let peso = if estilo.negrita {
        cairo::FontWeight::Bold
    } else {
        cairo::FontWeight::Normal
    };
    cr.select_font_face(familia, inclinacion, peso);
    cr.set_font_size(tamano);
}

fn medir_ancho(
    cr: &cairo::Context,
    config: &ExportConfig,
    estilo: Estilo,
    tamano_base: f64,
    texto: &str,
) -> f64 {
    seleccionar_fuente(cr, config, estilo, tamano_base);
    cr.text_extents(texto).map(|e| e.x_advance()).unwrap_or(0.0)
}

/// Envuelve una lista de frases en líneas visuales de palabras, cada una de
/// ancho <= `ancho_max`.
fn envolver_frases(
    frases: &[Frase],
    tamano_base: f64,
    config: &ExportConfig,
    cr: &cairo::Context,
    ancho_max: f64,
) -> Vec<Vec<Palabra>> {
    let mut palabras: Vec<Palabra> = Vec::new();
    for f in frases {
        for word in f.texto.split_whitespace() {
            let ancho = medir_ancho(cr, config, f.estilo, tamano_base, word);
            palabras.push(Palabra {
                texto: word.to_string(),
                estilo: f.estilo,
                ancho,
            });
        }
    }
    let espacio = medir_ancho(cr, config, Estilo::default(), tamano_base, " ");
    let mut lineas: Vec<Vec<Palabra>> = Vec::new();
    let mut actual: Vec<Palabra> = Vec::new();
    let mut ancho_actual = 0.0;
    for p in palabras {
        let extra = if actual.is_empty() { 0.0 } else { espacio };
        let ancho = p.ancho;
        if ancho_actual + extra + ancho <= ancho_max || actual.is_empty() {
            actual.push(p);
            ancho_actual += extra + ancho;
        } else {
            lineas.push(std::mem::take(&mut actual));
            actual.push(p);
            ancho_actual = ancho;
        }
    }
    if !actual.is_empty() {
        lineas.push(actual);
    }
    lineas
}

/// Espaciado extra (arriba + abajo) de una cabecera.
fn espaciado_cabecera(nivel: u8, config: &ExportConfig) -> f64 {
    match nivel {
        1 => config.tamano_cuerpo * 1.0,
        2 => config.tamano_cuerpo * 0.8,
        3 => config.tamano_cuerpo * 0.6,
        _ => config.tamano_cuerpo * 0.4,
    }
}

/// Alto del contenido de un bloque (sin la separación inferior).
fn alto_contenido(b: &Bloque, config: &ExportConfig) -> f64 {
    match b.tipo {
        TipoBloque::Regla => REGLA_ALTO,
        TipoBloque::Imagen => IMAGEN_ALTO,
        TipoBloque::Salto => 0.0,
        _ => {
            let (tamano, _) = tamano_y_estilo(b.tipo, config);
            let alto_linea = tamano * INTERLINEA;
            let mut alto = b.envuelto.len() as f64 * alto_linea;
            if let TipoBloque::Cabecera { nivel } = b.tipo {
                alto += espaciado_cabecera(nivel, config);
            }
            if matches!(b.tipo, TipoBloque::Codigo | TipoBloque::Tabla) {
                alto += PADDING_CAJA * 2.0;
            }
            alto
        }
    }
}

/// Mide un bloque (rellena `envuelto` y `alto`) con el contexto de cairo dado.
fn medir_bloque(b: &mut Bloque, config: &ExportConfig, cr: &cairo::Context, ancho_util: f64) {
    let (tamano, estilo_base) = tamano_y_estilo(b.tipo, config);
    let ancho_texto = ancho_util - sangrado_x(b, ancho_util);

    let mut envuelto: Vec<Vec<Palabra>> = Vec::new();
    match b.tipo {
        TipoBloque::Cabecera { .. } | TipoBloque::Parrafo => {
            envuelto = envolver_frases(&b.frases, tamano, config, cr, ancho_texto);
        }
        TipoBloque::Codigo | TipoBloque::Tabla | TipoBloque::Cita => {
            for line in &b.lineas {
                let frases = vec![Frase {
                    texto: line.clone(),
                    estilo: estilo_base,
                }];
                envuelto.extend(envolver_frases(&frases, tamano, config, cr, ancho_texto));
            }
        }
        _ => {}
    }
    b.envuelto = envuelto;

    let contenido = alto_contenido(b, config);
    b.alto = contenido
        + if b.tipo == TipoBloque::Salto {
            0.0
        } else {
            ESPACIO_BLOQUE
        };
}

/// Sangrado horizontal izquierdo de un bloque (puntos).
fn sangrado_x(b: &Bloque, _ancho_util: f64) -> f64 {
    match b.tipo {
        TipoBloque::Parrafo => INDENT_LISTA * b.nivel_lista as f64,
        TipoBloque::Cita => INDENT_CITA,
        _ => 0.0,
    }
}

/// Dibuja un bloque en `(x, y_top)` y devuelve la nueva `y` tras el bloque.
fn dibujar_bloque(
    cr: &cairo::Context,
    b: &Bloque,
    config: &ExportConfig,
    paleta: &Paleta,
    ancho_util: f64,
    x: f64,
    y_top: f64,
) -> f64 {
    let (tamano, _) = tamano_y_estilo(b.tipo, config);
    let alto_linea = tamano * INTERLINEA;

    // Fondo de la página (solo en oscuro; el claro y el mono son blancos).
    // Se pinta una vez por página, no aquí.

    match b.tipo {
        TipoBloque::Regla => {
            cr.set_source_rgb(paleta.regla.0, paleta.regla.1, paleta.regla.2);
            cr.set_line_width(1.0);
            cr.move_to(x, y_top + REGLA_ALTO / 2.0);
            cr.line_to(x + ancho_util, y_top + REGLA_ALTO / 2.0);
            let _ = cr.stroke();
        }
        TipoBloque::Imagen => {
            let (alt, _dest) = b
                .imagen
                .as_ref()
                .map(|(a, d)| (a.as_str(), d.as_str()))
                .unwrap_or(("", ""));
            let etiqueta = if alt.is_empty() {
                "[image]".to_string()
            } else {
                format!("[image: {alt}]")
            };
            cr.set_source_rgb(paleta.atenuado.0, paleta.atenuado.1, paleta.atenuado.2);
            cr.rectangle(x + 4.0, y_top + 4.0, ancho_util - 8.0, IMAGEN_ALTO - 8.0);
            let _ = cr.stroke();
            cr.set_source_rgb(paleta.texto.0, paleta.texto.1, paleta.texto.2);
            cr.set_font_size(config.tamano_cuerpo * 0.85);
            let ext = cr.text_extents(&etiqueta).unwrap();
            cr.move_to(
                x + 12.0,
                y_top + 4.0 + (IMAGEN_ALTO - 8.0) / 2.0 + ext.height() / 2.0,
            );
            let _ = cr.show_text(&etiqueta);
        }
        TipoBloque::Salto => {}
        TipoBloque::Codigo
        | TipoBloque::Tabla
        | TipoBloque::Cita
        | TipoBloque::Cabecera { .. }
        | TipoBloque::Parrafo => {
            let contenido = alto_contenido(b, config);
            // Color base del texto según el tipo de bloque.
            let color_texto = match b.tipo {
                TipoBloque::Cabecera { .. } => paleta.cabecera,
                TipoBloque::Cita => paleta.cita,
                _ => paleta.texto,
            };

            // Caja de fondo para código y tabla.
            if matches!(b.tipo, TipoBloque::Codigo | TipoBloque::Tabla) {
                let color = if b.tipo == TipoBloque::Codigo {
                    paleta.codigo_fondo
                } else {
                    paleta.tabla_fondo
                };
                cr.set_source_rgb(color.0, color.1, color.2);
                cr.rectangle(x, y_top, ancho_util, contenido);
                let _ = cr.fill();
            }
            // Barra de cita.
            if b.tipo == TipoBloque::Cita {
                cr.set_source_rgb(
                    paleta.cita_barra.0,
                    paleta.cita_barra.1,
                    paleta.cita_barra.2,
                );
                cr.rectangle(x, y_top, 2.0, contenido);
                let _ = cr.fill();
            }

            let mut cursor_y = y_top;
            if matches!(b.tipo, TipoBloque::Codigo | TipoBloque::Tabla) {
                cursor_y += PADDING_CAJA;
            }
            if let TipoBloque::Cabecera { nivel } = b.tipo {
                cursor_y += espaciado_cabecera(nivel, config) / 2.0;
            }

            let font_ext = {
                seleccionar_fuente(cr, config, Estilo::default(), tamano);
                cr.font_extents().unwrap()
            };
            let ascent = font_ext.ascent();

            for (idx_linea, linea_visual) in b.envuelto.iter().enumerate() {
                let mut x_actual = x + sangrado_x(b, ancho_util);
                if matches!(b.tipo, TipoBloque::Codigo | TipoBloque::Tabla) {
                    x_actual += PADDING_CAJA;
                }
                // Cabecera de tabla: la primera fila va en negrita.
                let es_cabecera_tabla = b.tipo == TipoBloque::Tabla && idx_linea == 0;
                for palabra in linea_visual {
                    let estilo = if es_cabecera_tabla {
                        Estilo {
                            negrita: true,
                            ..palabra.estilo
                        }
                    } else {
                        palabra.estilo
                    };
                    dibujar_palabra(
                        cr,
                        config,
                        paleta,
                        color_texto,
                        palabra,
                        estilo,
                        tamano,
                        x_actual,
                        cursor_y + ascent,
                    );
                    x_actual += palabra.ancho;
                    let espacio = cr.text_extents(" ").unwrap().x_advance();
                    x_actual += espacio;
                }
                cursor_y += alto_linea;
            }

            // Filete bajo H1/H2.
            if let TipoBloque::Cabecera { nivel } = b.tipo {
                if nivel <= 2 {
                    let y_regla = y_top + contenido - 4.0;
                    cr.set_source_rgb(paleta.regla.0, paleta.regla.1, paleta.regla.2);
                    cr.set_line_width(1.0);
                    cr.move_to(x, y_regla);
                    cr.line_to(x + ancho_util, y_regla);
                    let _ = cr.stroke();
                }
            }
        }
    }

    y_top + b.alto
}

/// Dibuja una palabra con su estilo (incluidos tachado y subrayado manuales).
#[allow(clippy::too_many_arguments)]
fn dibujar_palabra(
    cr: &cairo::Context,
    config: &ExportConfig,
    paleta: &Paleta,
    color_texto: (f64, f64, f64),
    palabra: &Palabra,
    estilo: Estilo,
    tamano: f64,
    x: f64,
    baseline: f64,
) {
    seleccionar_fuente(cr, config, estilo, tamano);

    let color = if estilo.enlace {
        paleta.enlace
    } else if estilo.codigo {
        paleta.codigo
    } else {
        color_texto
    };
    cr.set_source_rgb(color.0, color.1, color.2);

    cr.move_to(x, baseline);
    let _ = cr.show_text(&palabra.texto);

    let ancho = cr.text_extents(&palabra.texto).unwrap().x_advance();
    let ext = cr.font_extents().unwrap();

    // Tachado: raya a media altura de x-height.
    if estilo.tachado {
        cr.set_source_rgb(color.0, color.1, color.2);
        cr.set_line_width(1.0);
        let y = baseline - ext.ascent() * 0.33;
        cr.move_to(x, y);
        cr.line_to(x + ancho, y);
        let _ = cr.stroke();
    }
    // Subrayado para enlaces.
    if estilo.enlace {
        cr.set_source_rgb(color.0, color.1, color.2);
        cr.set_line_width(1.0);
        let y = baseline + ext.descent() * 0.3;
        cr.move_to(x, y);
        cr.line_to(x + ancho, y);
        let _ = cr.stroke();
    }
}

// ---------------------------------------------------------------------------
// Operación de impresión GTK
// ---------------------------------------------------------------------------

/// Construye una [`gtk4::PrintOperation`] lista para exportar `texto`.
///
/// `begin_print` mide los bloques con cairo y planifica las páginas;
/// `draw_page` pinta la página ya planificada. `sistema_oscuro` resuelve el
/// esquema `system`.
pub fn crear_operacion(
    config: &ExportConfig,
    texto: &str,
    titulo: &str,
    sistema_oscuro: bool,
) -> gtk4::PrintOperation {
    let op = gtk4::PrintOperation::new();
    op.set_job_name(titulo);

    let setup = gtk4::PageSetup::new();
    let papel = gtk4::PaperSize::new(Some(config.papel.paper_name()));
    setup.set_paper_size_and_default_margins(&papel);
    setup.set_orientation(match config.orientacion {
        ExportOrientation::Portrait => gtk4::PageOrientation::Portrait,
        ExportOrientation::Landscape => gtk4::PageOrientation::Landscape,
    });
    op.set_default_page_setup(Some(&setup));

    let analisis = analyze(texto);
    let bloques: Rc<RefCell<Vec<Bloque>>> =
        Rc::new(RefCell::new(construir_bloques(texto, &analisis)));
    let paginas: Rc<RefCell<Vec<Pagina>>> = Rc::new(RefCell::new(Vec::new()));
    let paleta = resolver_paleta(config.esquema, sistema_oscuro);
    let config = config.clone();

    {
        let bloques = bloques.clone();
        let paginas = paginas.clone();
        let config = config.clone();
        op.connect_begin_print(move |op, ctx| {
            let cr = ctx.cairo_context();
            let ancho = ctx.width();
            let alto = ctx.height();
            {
                let mut bloques = bloques.borrow_mut();
                for b in bloques.iter_mut() {
                    medir_bloque(b, &config, &cr, ancho);
                }
                *paginas.borrow_mut() = planificar_paginas(&bloques, alto);
            }
            op.set_n_pages(paginas.borrow().len() as i32);
        });
    }

    {
        let bloques = bloques.clone();
        let paginas = paginas.clone();
        let config = config.clone();
        op.connect_draw_page(move |_op, ctx, nr| {
            let cr = ctx.cairo_context();
            let ancho = ctx.width();
            let alto = ctx.height();

            // Fondo de la página.
            cr.set_source_rgb(paleta.fondo.0, paleta.fondo.1, paleta.fondo.2);
            cr.rectangle(0.0, 0.0, ancho, alto);
            let _ = cr.fill();

            let paginas = paginas.borrow();
            let bloques = bloques.borrow();
            let Some(pagina) = paginas.get(nr as usize) else {
                return;
            };
            let mut y = 0.0;
            for &idx in &pagina.bloques {
                if let Some(b) = bloques.get(idx) {
                    y = dibujar_bloque(&cr, b, &config, &paleta, ancho, 0.0, y);
                }
            }
        });
    }

    op
}

/// Abre el diálogo de impresión/exportación a PDF para el documento.
pub fn exportar(
    parent: &impl IsA<gtk4::Window>,
    config: &ExportConfig,
    texto: &str,
    titulo: &str,
    sistema_oscuro: bool,
) {
    let op = crear_operacion(config, texto, titulo, sistema_oscuro);
    match op.run(gtk4::PrintOperationAction::PrintDialog, Some(parent)) {
        Ok(gtk4::PrintOperationResult::Error) => {
            if let Some(e) = op.error() {
                glib::g_warning!("scribe", "no se pudo exportar a PDF: {e}");
            }
        }
        Ok(_) => {}
        Err(e) => glib::g_warning!("scribe", "no se pudo exportar a PDF: {e}"),
    }
}

/// Exporta directamente a `ruta` (sin diálogo). La usan los tests de
/// integración (`tests/export_pdf.rs`); en producción el diálogo interactivo
/// es [`exportar`].
#[allow(dead_code)]
pub fn exportar_a_fichero(
    config: &ExportConfig,
    texto: &str,
    titulo: &str,
    ruta: &Path,
    sistema_oscuro: bool,
) -> Result<(), glib::Error> {
    let op = crear_operacion(config, texto, titulo, sistema_oscuro);
    op.set_export_filename(ruta);
    // La acción Export es síncrona: al volver, el PDF está escrito (resultado
    // `Apply`). El fichero queda completo aunque `is_finished()` solo se ponga
    // a true al emitirse `done`.
    op.run(gtk4::PrintOperationAction::Export, None::<&gtk4::Window>)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn p(alto: f64) -> Bloque {
        Bloque::nuevo(TipoBloque::Parrafo, alto)
    }
    fn h(nivel: u8, alto: f64) -> Bloque {
        Bloque::nuevo(TipoBloque::Cabecera { nivel }, alto)
    }
    fn salto() -> Bloque {
        Bloque::nuevo(TipoBloque::Salto, 0.0)
    }
    fn codigo(alto: f64) -> Bloque {
        Bloque::nuevo(TipoBloque::Codigo, alto)
    }

    #[test]
    fn pdf_la_cabecera_baja_de_pagina_si_quedaria_huerfana() {
        let bloques = [p(35.0), h(1, 10.0), p(10.0)];
        let paginas = planificar_paginas(&bloques, 50.0);
        assert_eq!(paginas.len(), 2);
        assert_eq!(paginas[0].bloques, vec![0]);
        // La cabecera viaja con su contenido a la página siguiente.
        assert_eq!(paginas[1].bloques, vec![1, 2]);
    }

    #[test]
    fn pdf_newpage_fuerza_un_salto_de_pagina() {
        let bloques = [p(20.0), salto(), p(20.0)];
        let paginas = planificar_paginas(&bloques, 50.0);
        assert_eq!(paginas.len(), 2);
        assert_eq!(paginas[0].bloques, vec![0]);
        assert_eq!(paginas[1].bloques, vec![2]);
    }

    #[test]
    fn pdf_un_bloque_de_codigo_no_se_parte_si_cabe_en_una_pagina() {
        // El bloque de código (40) no cabe tras el párrafo (20) en 50, pero sí
        // entero en una página nueva; el párrafo final baja a la tercera.
        let bloques = [p(20.0), codigo(40.0), p(20.0)];
        let paginas = planificar_paginas(&bloques, 50.0);
        assert_eq!(paginas.len(), 3);
        assert_eq!(paginas[1].bloques, vec![1]);
    }

    #[test]
    fn pdf_el_salto_al_principio_no_crea_una_pagina_vacia() {
        let bloques = [salto(), p(10.0), p(10.0)];
        let paginas = planificar_paginas(&bloques, 50.0);
        assert_eq!(paginas.len(), 1);
        assert_eq!(paginas[0].bloques, vec![1, 2]);
    }

    #[test]
    fn pdf_un_documento_vacio_da_una_sola_pagina() {
        let paginas = planificar_paginas(&[], 50.0);
        assert_eq!(paginas.len(), 1);
        assert!(paginas[0].bloques.is_empty());
    }

    #[test]
    fn pdf_un_bloque_mas_alto_que_la_pagina_se_dibuja_solo_y_recortado() {
        let bloques = [p(10.0), codigo(120.0), p(10.0)];
        let paginas = planificar_paginas(&bloques, 50.0);
        // El código (120 > 50) ocupa su propia página; párrafo inicial y final
        // quedan aparte.
        assert_eq!(paginas.len(), 3);
        assert_eq!(paginas[1].bloques, vec![1]);
    }

    #[test]
    fn pdf_detecta_la_linea_newpage() {
        assert!(es_salto_pagina("\\newpage"));
        assert!(es_salto_pagina("  \\newpage  "));
        assert!(es_salto_pagina(
            "<div style=\"page-break-after: always\"></div>"
        ));
        assert!(!es_salto_pagina("newpage"));
        assert!(!es_salto_pagina("texto normal"));
    }

    #[test]
    fn pdf_segmenta_negrita_cursiva_codigo_y_enlace() {
        let frases = segmentar_inline("un **texto** con *cursiva* y `código` y [enlace](http://x)");
        let estilos: Vec<Estilo> = frases
            .iter()
            .filter(|f| f.estilo != Estilo::default())
            .map(|f| f.estilo)
            .collect();
        assert!(estilos.iter().any(|e| e.negrita));
        assert!(estilos.iter().any(|e| e.cursiva));
        assert!(estilos.iter().any(|e| e.codigo));
        assert!(estilos.iter().any(|e| e.enlace));
        // El texto del enlace no lleva la URL.
        assert!(frases.iter().all(|f| !f.texto.contains("http")));
    }

    #[test]
    fn pdf_separa_el_marcador_de_lista() {
        assert_eq!(separar_marcador_lista("- item"), Some(("- ", "item")));
        assert_eq!(separar_marcador_lista("12. item"), Some(("12. ", "item")));
        assert_eq!(
            separar_marcador_lista("- [x] tarea"),
            Some(("- [x] ", "tarea"))
        );
        assert_eq!(separar_marcador_lista("no es lista"), None);
    }
}
