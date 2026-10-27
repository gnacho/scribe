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
//! - Las listas conservan el marcador del fuente (`-`, `1.`, `- [ ]`). Las
//!   tablas se dibujan como tabla real (columnas con su ancho repartido por
//!   contenido, cabecera en negrita y rejilla ligera); una celda cuyo texto
//!   no cabe envuelve dentro de su columna.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use gtk4::cairo;
use gtk4::prelude::*;

use crate::markdown_render::{analyze, Analysis, Ornament, SpanKind};
use crate::settings::{
    AppSettings, ExportColorScheme, ExportMonoFont, ExportOrientation, ExportPageSize,
    ExportTemplate, FontFamily,
};

// ---------------------------------------------------------------------------
// Constantes de maquetación
// ---------------------------------------------------------------------------

/// Altura de línea relativa al tamaño del bloque (cuerpo = 1.4). Es el valor
/// de las plantillas Custom, Article y Letter.
const INTERLINEA: f64 = 1.4;
/// Separación vertical entre bloques consecutivos (puntos).
const ESPACIO_BLOQUE: f64 = 10.0;
/// Padding interior de las cajas (bloques de código y tablas).
const PADDING_CAJA: f64 = 6.0;
/// Padding horizontal del texto dentro de una celda (a cada lado).
const CELDA_PAD: f64 = 6.0;
/// Padding vertical del texto dentro de una fila (arriba y abajo).
const FILA_PAD_V: f64 = 3.0;
/// Alto de un bloque de regla horizontal.
const REGLA_ALTO: f64 = 14.0;
/// Alto del placeholder de una imagen.
const IMAGEN_ALTO: f64 = 42.0;
/// Sangrado horizontal por nivel de lista (puntos).
const INDENT_LISTA: f64 = 18.0;
/// Sangrado de la barra de cita (puntos).
const INDENT_CITA: f64 = 14.0;
/// Escala de las cabeceras por nivel (índice 0 sin uso): misma proporción que
/// la vista del editor.
const ESCALA_CABECERA: [f64; 5] = [1.0, 1.9, 1.5, 1.25, 1.1];
/// Espaciado extra de una cabecera, como fracción del cuerpo (índice 0 sin uso).
const ESPACIADO_CABECERA: [f64; 5] = [0.0, 1.0, 0.8, 0.6, 0.4];
/// Alto de la banda de cabecera/pie, dentro del área de contenido (puntos).
const BANDA_ADORNO: f64 = 18.0;
/// Cuerpo de los adornos de cabecera y pie.
const TAMANO_ADORNO: f64 = 8.0;

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
    pub template: ExportTemplate,
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
            template: s.export_template(),
        }
    }

    /// Aplica la tipografía y el esquema que impone la plantilla, dejando
    /// intactos los ajustes granulares cuando la plantilla no los fija.
    fn con_plantilla(&self, plantilla: &Plantilla) -> Self {
        let mut config = self.clone();
        if let Some(familia) = plantilla.familia {
            config.familia_cuerpo = familia;
        }
        if let Some(tamano) = plantilla.tamano {
            config.tamano_cuerpo = tamano;
        }
        if let Some(esquema) = plantilla.esquema {
            config.esquema = esquema;
        }
        config
    }
}

// ---------------------------------------------------------------------------
// Plantillas de página
// ---------------------------------------------------------------------------

/// Márgenes de página en puntos.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Margenes {
    superior: f64,
    inferior: f64,
    izquierda: f64,
    derecha: f64,
}

impl Margenes {
    /// Márgenes iguales en los cuatro lados.
    const fn simetricos(margen: f64) -> Self {
        Self {
            superior: margen,
            inferior: margen,
            izquierda: margen,
            derecha: margen,
        }
    }
}

/// Contenido de la banda de cabecera o pie de página.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Adorno {
    Nada,
    /// Título del documento (en la cabecera).
    Titulo,
    /// Número de página «n / total» (en el pie).
    NumeroPagina,
}

/// Valores de maquetación resueltos de una plantilla. Los `None` significan
/// "lo que traiga el ajuste granular" o "el margen por defecto del papel".
#[derive(Debug, Clone, Copy)]
struct Plantilla {
    margenes: Option<Margenes>,
    interlinea: f64,
    espacio_bloque: f64,
    /// Escala de cada nivel de cabecera (índices 1..=4).
    escala: [f64; 5],
    /// Espaciado extra de cabecera como fracción del cuerpo (1..=4).
    espaciado: [f64; 5],
    cabecera: Adorno,
    pie: Adorno,
    bloque_titulo: bool,
    familia: Option<FontFamily>,
    tamano: Option<f64>,
    esquema: Option<ExportColorScheme>,
}

impl Plantilla {
    /// Resuelve la plantilla elegida. Custom no fija nada: reproduce la salida
    /// de siempre con los ajustes granulares del usuario.
    fn desde(config: &ExportConfig) -> Self {
        match config.template {
            ExportTemplate::Custom => Self::custom(),
            ExportTemplate::Article => Self::article(),
            ExportTemplate::Manuscript => Self::manuscript(),
            ExportTemplate::Letter => Self::letter(),
            ExportTemplate::Compact => Self::compact(),
        }
    }

    fn custom() -> Self {
        Self {
            margenes: None,
            interlinea: INTERLINEA,
            espacio_bloque: ESPACIO_BLOQUE,
            escala: ESCALA_CABECERA,
            espaciado: ESPACIADO_CABECERA,
            cabecera: Adorno::Nada,
            pie: Adorno::Nada,
            bloque_titulo: false,
            familia: None,
            tamano: None,
            esquema: None,
        }
    }

    /// Artículo: los valores de siempre, sin adornos ni portada.
    fn article() -> Self {
        Self {
            familia: Some(FontFamily::Sans),
            tamano: Some(11.0),
            esquema: Some(ExportColorScheme::Light),
            ..Self::custom()
        }
    }

    /// Manuscrito: serif, doble espacio, márgenes de una pulgada, portada con
    /// el título, cabecera con el título y número de página al pie.
    fn manuscript() -> Self {
        Self {
            margenes: Some(Margenes::simetricos(72.0)),
            interlinea: 2.0,
            espacio_bloque: 12.0,
            escala: [1.0, 1.5, 1.3, 1.15, 1.05],
            espaciado: [0.0, 1.2, 1.0, 0.8, 0.6],
            cabecera: Adorno::Titulo,
            pie: Adorno::NumeroPagina,
            bloque_titulo: true,
            familia: Some(FontFamily::Serif),
            tamano: Some(12.0),
            esquema: Some(ExportColorScheme::Light),
        }
    }

    /// Carta: como el artículo pero con márgenes más ajustados y sin portada.
    fn letter() -> Self {
        Self {
            margenes: Some(Margenes::simetricos(54.0)),
            ..Self::article()
        }
    }

    /// Compacta: para notas técnicas, cuerpo menor y espaciado apretado.
    fn compact() -> Self {
        Self {
            margenes: Some(Margenes::simetricos(36.0)),
            interlinea: 1.25,
            espacio_bloque: 7.0,
            escala: [1.0, 1.5, 1.3, 1.15, 1.05],
            espaciado: [0.0, 0.8, 0.65, 0.5, 0.35],
            cabecera: Adorno::Nada,
            pie: Adorno::NumeroPagina,
            bloque_titulo: false,
            familia: Some(FontFamily::Sans),
            tamano: Some(10.0),
            esquema: Some(ExportColorScheme::Light),
        }
    }

    /// Escala de una cabecera por nivel.
    fn escala_cabecera(&self, nivel: u8) -> f64 {
        self.escala.get(nivel as usize).copied().unwrap_or(1.0)
    }

    /// Espaciado extra de una cabecera (puntos).
    fn espaciado_cabecera(&self, nivel: u8, tamano_cuerpo: f64) -> f64 {
        tamano_cuerpo * self.espaciado.get(nivel as usize).copied().unwrap_or(0.4)
    }

    /// Banda reservada arriba para la cabecera.
    fn banda_superior(&self) -> f64 {
        if self.cabecera == Adorno::Nada {
            0.0
        } else {
            BANDA_ADORNO
        }
    }

    /// Banda reservada abajo para el pie.
    fn banda_inferior(&self) -> f64 {
        if self.pie == Adorno::Nada {
            0.0
        } else {
            BANDA_ADORNO
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
    tabla_cabecera: (f64, f64, f64),
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
            tabla_cabecera: (0.90, 0.90, 0.90),
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
            tabla_cabecera: hex(0x323232),
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
            tabla_cabecera: hex(0xefefef),
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
    /// Tabla: filas x columnas limpias (fila 0 = cabecera, `\|` resuelto).
    pub celdas: Vec<Vec<String>>,
    /// Tabla: alineación por columna (de la fila delimitadora).
    pub alineacion: Vec<AlineacionCelda>,
    /// Tabla: [fila][columna][línea visual] ya envuelta y medida.
    pub tabla_envuelta: Vec<Vec<Vec<Vec<Palabra>>>>,
    /// Tabla: ancho de contenido por columna (sin el padding de celda).
    pub anchos_columna: Vec<f64>,
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
                .collect();
            // La fila delimitadora (la línea de guiones bajo la cabecera) da
            // las alineaciones y no es una fila de datos.
            let mut alineacion: Vec<AlineacionCelda> = Vec::new();
            let mut filas: Vec<Vec<String>> = Vec::new();
            for linea in &lineas_bloque {
                if es_fila_delimitadora(linea) {
                    if let Some(al) = parsear_alineaciones(linea) {
                        alineacion = al;
                    }
                    continue;
                }
                filas.push(partir_fila(linea));
            }
            let columnas = filas.first().map(Vec::len).unwrap_or(0);
            for fila in &mut filas {
                fila.resize(columnas, String::new());
                fila.truncate(columnas);
            }
            alineacion.resize(columnas, AlineacionCelda::default());
            alineacion.truncate(columnas);
            bloques.push(Bloque {
                tipo: TipoBloque::Tabla,
                celdas: filas,
                alineacion,
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

/// Alineación de una columna de tabla (fila 0 delimitadora).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlineacionCelda {
    #[default]
    Izquierda,
    Centro,
    Derecha,
}

/// Parte una fila de tabla en celdas: corta por `|` no escapado, resuelve
/// `\|` a `|` literal y recorta espacios. Las celdas vacías de los extremos
/// (fruto de los `|` de apertura y cierre del fuente) se descartan.
fn partir_fila(linea: &str) -> Vec<String> {
    let mut celdas: Vec<String> = vec![String::new()];
    let mut escapado = false;
    for ch in linea.chars() {
        if escapado {
            if ch == '|' {
                celdas.last_mut().unwrap().push('|');
            } else {
                celdas.last_mut().unwrap().push('\\');
                celdas.last_mut().unwrap().push(ch);
            }
            escapado = false;
        } else if ch == '\\' {
            escapado = true;
        } else if ch == '|' {
            celdas.push(String::new());
        } else {
            celdas.last_mut().unwrap().push(ch);
        }
    }
    if escapado {
        celdas.last_mut().unwrap().push('\\');
    }
    for celda in &mut celdas {
        *celda = celda.trim().to_string();
    }
    if celdas.first().map(|c| c.is_empty()).unwrap_or(false) {
        celdas.remove(0);
    }
    if celdas.last().map(|c| c.is_empty()).unwrap_or(false) {
        celdas.pop();
    }
    celdas
}

/// Parsea la fila delimitadora de una tabla (`| --- | :-: | --: |`) y devuelve
/// la alineación de cada columna. `None` si la línea no es delimitadora.
fn parsear_alineaciones(linea: &str) -> Option<Vec<AlineacionCelda>> {
    if !es_fila_delimitadora(linea) {
        return None;
    }
    Some(
        partir_fila(linea)
            .into_iter()
            .map(
                |celda| match (celda.starts_with(':'), celda.ends_with(':')) {
                    (true, true) => AlineacionCelda::Centro,
                    (false, true) => AlineacionCelda::Derecha,
                    _ => AlineacionCelda::Izquierda,
                },
            )
            .collect(),
    )
}

/// Reparte el ancho disponible entre columnas dadas sus anchuras naturales
/// (palabra más ancha) y sus mínimos. Si todo cabe, el sobrante se reparte
/// proporcional al ancho natural; si ni los mínimos caben, todo se escala al
/// disponible; si no, se encoge cada columna según su margen (natural menos
/// mínimo) sin bajar del mínimo.
fn calcular_anchos(naturales: &[f64], minimos: &[f64], disponible: f64) -> Vec<f64> {
    let suma: f64 = naturales.iter().sum();
    let suma_min: f64 = minimos.iter().sum();
    if disponible > 0.0 && suma <= disponible {
        let sobrante = disponible - suma;
        return naturales
            .iter()
            .map(|&n| {
                if suma > 0.0 {
                    n + sobrante * (n / suma)
                } else {
                    sobrante / naturales.len().max(1) as f64
                }
            })
            .collect();
    }
    if suma_min >= disponible || suma <= 0.0 {
        let factor = if suma_min > 0.0 {
            disponible / suma_min
        } else {
            1.0
        };
        return minimos.iter().map(|&m| m * factor).collect();
    }
    let deficit = suma - disponible;
    let margen: f64 = naturales
        .iter()
        .zip(minimos)
        .map(|(&n, &m)| (n - m).max(0.0))
        .sum();
    naturales
        .iter()
        .zip(minimos)
        .map(|(&n, &m)| {
            let margen_col = (n - m).max(0.0);
            let recorte = if margen > 0.0 {
                deficit * (margen_col / margen)
            } else {
                0.0
            };
            (n - recorte).max(m)
        })
        .collect()
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

/// Familia tipográfica del cuerpo.
fn familia_cuerpo(ff: FontFamily) -> &'static str {
    match ff {
        FontFamily::Sans => "Cantarell",
        FontFamily::Serif => "Source Serif 4",
        FontFamily::Mono => "Adwaita Mono",
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
fn tamano_y_estilo(
    tipo: TipoBloque,
    config: &ExportConfig,
    plantilla: &Plantilla,
) -> (f64, Estilo) {
    match tipo {
        TipoBloque::Cabecera { nivel } => (
            config.tamano_cuerpo * plantilla.escala_cabecera(nivel),
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

/// Alto de una fila de tabla: máximo de líneas visuales entre sus celdas.
fn alto_fila_tabla(fila: &[Vec<Vec<Palabra>>], alto_linea: f64) -> f64 {
    let lineas = fila.iter().map(Vec::len).max().unwrap_or(0).max(1);
    lineas as f64 * alto_linea + 2.0 * FILA_PAD_V
}

/// Alto del contenido de un bloque (sin la separación inferior).
fn alto_contenido(b: &Bloque, config: &ExportConfig, plantilla: &Plantilla) -> f64 {
    match b.tipo {
        TipoBloque::Regla => REGLA_ALTO,
        TipoBloque::Imagen => IMAGEN_ALTO,
        TipoBloque::Salto => 0.0,
        TipoBloque::Tabla => {
            let tamano = config.tamano_cuerpo * 0.9;
            let alto_linea = tamano * plantilla.interlinea;
            let mut alto = 2.0 * PADDING_CAJA;
            for fila in &b.tabla_envuelta {
                alto += alto_fila_tabla(fila, alto_linea);
            }
            alto
        }
        _ => {
            let (tamano, _) = tamano_y_estilo(b.tipo, config, plantilla);
            let alto_linea = tamano * plantilla.interlinea;
            let mut alto = b.envuelto.len() as f64 * alto_linea;
            if let TipoBloque::Cabecera { nivel } = b.tipo {
                alto += plantilla.espaciado_cabecera(nivel, config.tamano_cuerpo);
            }
            if b.tipo == TipoBloque::Codigo {
                alto += PADDING_CAJA * 2.0;
            }
            alto
        }
    }
}

/// Mide un bloque (rellena `envuelto` y `alto`) con el contexto de cairo dado.
fn medir_bloque(
    b: &mut Bloque,
    config: &ExportConfig,
    plantilla: &Plantilla,
    cr: &cairo::Context,
    ancho_util: f64,
) {
    let (tamano, estilo_base) = tamano_y_estilo(b.tipo, config, plantilla);
    let ancho_texto = ancho_util - sangrado_x(b, ancho_util);

    let mut envuelto: Vec<Vec<Palabra>> = Vec::new();
    match b.tipo {
        TipoBloque::Cabecera { .. } | TipoBloque::Parrafo => {
            envuelto = envolver_frases(&b.frases, tamano, config, cr, ancho_texto);
        }
        TipoBloque::Codigo | TipoBloque::Cita => {
            for line in &b.lineas {
                let frases = vec![Frase {
                    texto: line.clone(),
                    estilo: estilo_base,
                }];
                envuelto.extend(envolver_frases(&frases, tamano, config, cr, ancho_texto));
            }
        }
        TipoBloque::Tabla => medir_tabla(b, config, cr, ancho_util),
        _ => {}
    }
    if b.tipo != TipoBloque::Tabla {
        b.envuelto = envuelto;
    }

    let contenido = alto_contenido(b, config, plantilla);
    b.alto = contenido
        + if b.tipo == TipoBloque::Salto {
            0.0
        } else {
            plantilla.espacio_bloque
        };
}

/// Mide una tabla: reparte anchos de columna por contenido y envuelve cada
/// celda dentro de su columna.
fn medir_tabla(b: &mut Bloque, config: &ExportConfig, cr: &cairo::Context, ancho_util: f64) {
    let columnas = b.celdas.first().map(Vec::len).unwrap_or(0);
    if columnas == 0 {
        b.tabla_envuelta = Vec::new();
        b.anchos_columna = Vec::new();
        return;
    }
    let tamano = config.tamano_cuerpo * 0.9;
    let disponible = (ancho_util - 2.0 * CELDA_PAD * columnas as f64).max(1.0);

    // Anchura natural de cada columna: su palabra más ancha.
    let mut frases_celdas: Vec<Vec<Vec<Frase>>> = Vec::new();
    let mut naturales = vec![0.0f64; columnas];
    for fila in &b.celdas {
        let mut fila_frases: Vec<Vec<Frase>> = Vec::new();
        for (j, celda) in fila.iter().enumerate().take(columnas) {
            let frases = segmentar_inline(celda);
            for f in &frases {
                for word in f.texto.split_whitespace() {
                    let ancho = medir_ancho(cr, config, f.estilo, tamano, word);
                    naturales[j] = naturales[j].max(ancho);
                }
            }
            fila_frases.push(frases);
        }
        frases_celdas.push(fila_frases);
    }

    let anchos = calcular_anchos(&naturales, &naturales, disponible);
    b.anchos_columna = anchos.clone();

    // Envuelve cada celda dentro de su columna.
    let mut tabla: Vec<Vec<Vec<Vec<Palabra>>>> = Vec::new();
    for fila_frases in &frases_celdas {
        let mut fila_env: Vec<Vec<Vec<Palabra>>> = Vec::new();
        for (j, frases) in fila_frases.iter().enumerate() {
            let ancho_col = anchos.get(j).copied().unwrap_or(1.0);
            fila_env.push(envolver_frases(frases, tamano, config, cr, ancho_col));
        }
        tabla.push(fila_env);
    }
    b.tabla_envuelta = tabla;
}

/// Sangrado horizontal izquierdo de un bloque (puntos).
fn sangrado_x(b: &Bloque, _ancho_util: f64) -> f64 {
    match b.tipo {
        TipoBloque::Parrafo => INDENT_LISTA * b.nivel_lista as f64,
        TipoBloque::Cita => INDENT_CITA,
        _ => 0.0,
    }
}

/// Parámetros del dibujado que se mantienen entre bloques de una misma página.
#[derive(Clone, Copy)]
struct Lienzo<'a> {
    config: &'a ExportConfig,
    plantilla: &'a Plantilla,
    paleta: &'a Paleta,
    ancho: f64,
}

/// Dibuja un bloque en `(x, y_top)` y devuelve la nueva `y` tras el bloque.
fn dibujar_bloque(cr: &cairo::Context, b: &Bloque, lienzo: Lienzo, x: f64, y_top: f64) -> f64 {
    let Lienzo {
        config,
        plantilla,
        paleta,
        ancho: ancho_util,
    } = lienzo;
    let (tamano, _) = tamano_y_estilo(b.tipo, config, plantilla);
    let alto_linea = tamano * plantilla.interlinea;

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
        TipoBloque::Tabla => {
            dibujar_tabla(cr, b, lienzo, x, y_top);
        }
        TipoBloque::Codigo
        | TipoBloque::Cita
        | TipoBloque::Cabecera { .. }
        | TipoBloque::Parrafo => {
            let contenido = alto_contenido(b, config, plantilla);
            // Color base del texto según el tipo de bloque.
            let color_texto = match b.tipo {
                TipoBloque::Cabecera { .. } => paleta.cabecera,
                TipoBloque::Cita => paleta.cita,
                _ => paleta.texto,
            };

            // Caja de fondo para código.
            if b.tipo == TipoBloque::Codigo {
                cr.set_source_rgb(
                    paleta.codigo_fondo.0,
                    paleta.codigo_fondo.1,
                    paleta.codigo_fondo.2,
                );
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
            if b.tipo == TipoBloque::Codigo {
                cursor_y += PADDING_CAJA;
            }
            if let TipoBloque::Cabecera { nivel } = b.tipo {
                cursor_y += plantilla.espaciado_cabecera(nivel, config.tamano_cuerpo) / 2.0;
            }

            let font_ext = {
                seleccionar_fuente(cr, config, Estilo::default(), tamano);
                cr.font_extents().unwrap()
            };
            let ascent = font_ext.ascent();

            for linea_visual in b.envuelto.iter() {
                let mut x_actual = x + sangrado_x(b, ancho_util);
                if b.tipo == TipoBloque::Codigo {
                    x_actual += PADDING_CAJA;
                }
                for palabra in linea_visual {
                    dibujar_palabra(
                        cr,
                        config,
                        paleta,
                        color_texto,
                        palabra,
                        palabra.estilo,
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

/// Dibuja una tabla con columnas reales: caja de fondo, franja de cabecera en
/// negrita con filete fuerte, rejilla ligera entre filas y columnas, y texto
/// de celda alineado y envuelto dentro de su columna.
fn dibujar_tabla(cr: &cairo::Context, b: &Bloque, lienzo: Lienzo, x: f64, y_top: f64) {
    let Lienzo {
        config,
        plantilla,
        paleta,
        ancho: ancho_util,
    } = lienzo;
    let tamano = config.tamano_cuerpo * 0.9;
    let alto_linea = tamano * plantilla.interlinea;
    let contenido = alto_contenido(b, config, plantilla);

    // Caja de fondo completa.
    cr.set_source_rgb(
        paleta.tabla_fondo.0,
        paleta.tabla_fondo.1,
        paleta.tabla_fondo.2,
    );
    cr.rectangle(x, y_top, ancho_util, contenido);
    let _ = cr.fill();

    let font_ext = {
        seleccionar_fuente(cr, config, Estilo::default(), tamano);
        cr.font_extents().unwrap()
    };
    let ascent = font_ext.ascent();
    let espacio = {
        seleccionar_fuente(cr, config, Estilo::default(), tamano);
        cr.text_extents(" ").unwrap().x_advance()
    };

    // Franja de cabecera (fila 0).
    let alto_cabecera = b
        .tabla_envuelta
        .first()
        .map(|fila| alto_fila_tabla(fila, alto_linea))
        .unwrap_or(0.0);
    if alto_cabecera > 0.0 {
        cr.set_source_rgb(
            paleta.tabla_cabecera.0,
            paleta.tabla_cabecera.1,
            paleta.tabla_cabecera.2,
        );
        cr.rectangle(x, y_top, ancho_util, alto_cabecera);
        let _ = cr.fill();
    }

    let mut y_fila = y_top + PADDING_CAJA;
    for (i, fila) in b.tabla_envuelta.iter().enumerate() {
        let mut x_celda = x + CELDA_PAD;
        for (j, celda) in fila.iter().enumerate() {
            let ancho_col = b.anchos_columna.get(j).copied().unwrap_or(0.0);
            for (k, linea_visual) in celda.iter().enumerate() {
                let ancho_linea: f64 = linea_visual.iter().map(|p| p.ancho).sum::<f64>()
                    + espacio * linea_visual.len().saturating_sub(1) as f64;
                let mut x_texto = match b.alineacion.get(j) {
                    Some(AlineacionCelda::Centro) => x_celda + (ancho_col - ancho_linea) / 2.0,
                    Some(AlineacionCelda::Derecha) => x_celda + (ancho_col - ancho_linea).max(0.0),
                    _ => x_celda,
                };
                let baseline = y_fila + FILA_PAD_V + k as f64 * alto_linea + ascent;
                for palabra in linea_visual {
                    // La fila de cabecera va en negrita.
                    let estilo = if i == 0 {
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
                        paleta.texto,
                        palabra,
                        estilo,
                        tamano,
                        x_texto,
                        baseline,
                    );
                    x_texto += palabra.ancho + espacio;
                }
            }
            x_celda += ancho_col + 2.0 * CELDA_PAD;
        }
        y_fila += alto_fila_tabla(fila, alto_linea);

        // Filete fuerte bajo la cabecera; línea clara entre el resto de filas.
        if i == 0 {
            cr.set_source_rgb(paleta.regla.0, paleta.regla.1, paleta.regla.2);
            cr.set_line_width(1.2);
        } else {
            cr.set_source_rgb(paleta.atenuado.0, paleta.atenuado.1, paleta.atenuado.2);
            cr.set_line_width(0.7);
        }
        cr.move_to(x, y_fila);
        cr.line_to(x + ancho_util, y_fila);
        let _ = cr.stroke();
    }

    // Separadores verticales entre columnas, dentro del área de contenido.
    if b.anchos_columna.len() < 2 {
        return;
    }
    cr.set_source_rgb(paleta.atenuado.0, paleta.atenuado.1, paleta.atenuado.2);
    cr.set_line_width(0.7);
    let mut acumulado = 0.0;
    for (j, &ancho_col) in b.anchos_columna.iter().enumerate() {
        acumulado += ancho_col + 2.0 * CELDA_PAD;
        if j + 1 < b.anchos_columna.len() {
            let x_v = x + acumulado - CELDA_PAD;
            cr.move_to(x_v, y_top + PADDING_CAJA);
            cr.line_to(x_v, y_top + contenido - PADDING_CAJA);
        }
    }
    let _ = cr.stroke();
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
/// Bloque de título que encabeza la primera página de las plantillas con
/// portada. Se mide y pagina como una cabecera de nivel 1.
fn bloque_titulo(titulo: &str) -> Bloque {
    Bloque {
        tipo: TipoBloque::Cabecera { nivel: 1 },
        frases: vec![Frase {
            texto: titulo.to_string(),
            estilo: Estilo::default(),
        }],
        ..Default::default()
    }
}

/// Texto de una banda de cabecera o pie para la página `pagina` (0-based).
fn texto_adorno(adorno: Adorno, titulo: &str, pagina: usize, total: usize) -> Option<String> {
    match adorno {
        Adorno::Nada => None,
        Adorno::Titulo => Some(titulo.to_string()),
        Adorno::NumeroPagina => Some(format!("{} / {}", pagina + 1, total)),
    }
}

/// Dibuja una línea de adorno centrada en el ancho útil.
fn dibujar_adorno(
    cr: &cairo::Context,
    config: &ExportConfig,
    paleta: &Paleta,
    texto: &str,
    ancho: f64,
    baseline: f64,
) {
    cr.select_font_face(
        familia_cuerpo(config.familia_cuerpo),
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
    );
    cr.set_font_size(TAMANO_ADORNO);
    cr.set_source_rgb(paleta.atenuado.0, paleta.atenuado.1, paleta.atenuado.2);
    let recortado = recortar_a_ancho(cr, texto, ancho);
    if let Ok(ext) = cr.text_extents(&recortado) {
        cr.move_to(((ancho - ext.x_advance()) / 2.0).max(0.0), baseline);
        let _ = cr.show_text(&recortado);
    }
}

/// Recorta el texto con puntos suspensivos hasta que quepa en `ancho`.
fn recortar_a_ancho(cr: &cairo::Context, texto: &str, ancho: f64) -> String {
    let cabe = |t: &str| {
        cr.text_extents(t)
            .map(|e| e.x_advance() <= ancho)
            .unwrap_or(true)
    };
    if cabe(texto) {
        return texto.to_string();
    }
    let mut recortado: String = texto.to_string();
    while !recortado.is_empty() {
        recortado.pop();
        let prueba = format!("{}…", recortado.trim_end());
        if cabe(&prueba) {
            return prueba;
        }
    }
    String::new()
}

/// Prepara la operación de impresión. `draw_page` pinta la página ya
/// planificada. `sistema_oscuro` resuelve el esquema `system`.
pub fn crear_operacion(
    config: &ExportConfig,
    texto: &str,
    titulo: &str,
    sistema_oscuro: bool,
) -> gtk4::PrintOperation {
    let plantilla = Plantilla::desde(config);
    let config = config.con_plantilla(&plantilla);

    let op = gtk4::PrintOperation::new();
    op.set_job_name(titulo);

    let setup = gtk4::PageSetup::new();
    let papel = gtk4::PaperSize::new(Some(config.papel.paper_name()));
    setup.set_paper_size_and_default_margins(&papel);
    if let Some(m) = plantilla.margenes {
        setup.set_top_margin(m.superior, gtk4::Unit::Points);
        setup.set_bottom_margin(m.inferior, gtk4::Unit::Points);
        setup.set_left_margin(m.izquierda, gtk4::Unit::Points);
        setup.set_right_margin(m.derecha, gtk4::Unit::Points);
    }
    setup.set_orientation(match config.orientacion {
        ExportOrientation::Portrait => gtk4::PageOrientation::Portrait,
        ExportOrientation::Landscape => gtk4::PageOrientation::Landscape,
    });
    op.set_default_page_setup(Some(&setup));

    let analisis = analyze(texto);
    let mut iniciales = construir_bloques(texto, &analisis);
    if plantilla.bloque_titulo && !titulo.trim().is_empty() {
        iniciales.insert(0, bloque_titulo(titulo));
    }
    let bloques: Rc<RefCell<Vec<Bloque>>> = Rc::new(RefCell::new(iniciales));
    let paginas: Rc<RefCell<Vec<Pagina>>> = Rc::new(RefCell::new(Vec::new()));
    let paleta = resolver_paleta(config.esquema, sistema_oscuro);
    let banda_sup = plantilla.banda_superior();
    let banda_inf = plantilla.banda_inferior();
    let titulo_adorno = titulo.to_string();

    {
        let bloques = bloques.clone();
        let paginas = paginas.clone();
        let config = config.clone();
        op.connect_begin_print(move |op, ctx| {
            let cr = ctx.cairo_context();
            let ancho = ctx.width();
            let alto_util = (ctx.height() - banda_sup - banda_inf).max(1.0);
            {
                let mut bloques = bloques.borrow_mut();
                for b in bloques.iter_mut() {
                    medir_bloque(b, &config, &plantilla, &cr, ancho);
                }
                *paginas.borrow_mut() = planificar_paginas(&bloques, alto_util);
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
            let alto_pagina = ctx.height();

            // Fondo de la página.
            cr.set_source_rgb(paleta.fondo.0, paleta.fondo.1, paleta.fondo.2);
            cr.rectangle(0.0, 0.0, ancho, alto_pagina);
            let _ = cr.fill();

            let paginas = paginas.borrow();
            let bloques = bloques.borrow();
            let Some(pagina) = paginas.get(nr as usize) else {
                return;
            };
            let total = paginas.len();
            let mut y = banda_sup;
            let lienzo = Lienzo {
                config: &config,
                plantilla: &plantilla,
                paleta: &paleta,
                ancho,
            };
            for &idx in &pagina.bloques {
                if let Some(b) = bloques.get(idx) {
                    y = dibujar_bloque(&cr, b, lienzo, 0.0, y);
                }
            }

            // Cabecera: en la primera página se omite si la portada ya lleva
            // el título.
            if !(nr == 0 && plantilla.bloque_titulo) {
                if let Some(t) =
                    texto_adorno(plantilla.cabecera, &titulo_adorno, nr as usize, total)
                {
                    dibujar_adorno(
                        &cr,
                        &config,
                        &paleta,
                        &t,
                        ancho,
                        (banda_sup + TAMANO_ADORNO) / 2.0,
                    );
                }
            }
            if let Some(t) = texto_adorno(plantilla.pie, &titulo_adorno, nr as usize, total) {
                dibujar_adorno(
                    &cr,
                    &config,
                    &paleta,
                    &t,
                    ancho,
                    alto_pagina - banda_inf / 2.0 + TAMANO_ADORNO / 2.0,
                );
            }
        });
    }

    op
}

/// Lanza la operación con la acción pedida y avisa si falla.
fn ejecutar(
    parent: &impl IsA<gtk4::Window>,
    op: &gtk4::PrintOperation,
    accion: gtk4::PrintOperationAction,
    que: &str,
) {
    match op.run(accion, Some(parent)) {
        Ok(gtk4::PrintOperationResult::Error) => {
            if let Some(e) = op.error() {
                glib::g_warning!("scribe", "no se pudo {que}: {e}");
            }
        }
        Ok(_) => {}
        Err(e) => glib::g_warning!("scribe", "no se pudo {que}: {e}"),
    }
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
    ejecutar(
        parent,
        &op,
        gtk4::PrintOperationAction::PrintDialog,
        "exportar a PDF",
    );
}

/// Abre la previsualización de impresión con la misma maqueta que el PDF.
pub fn previsualizar(
    parent: &impl IsA<gtk4::Window>,
    config: &ExportConfig,
    texto: &str,
    titulo: &str,
    sistema_oscuro: bool,
) {
    let op = crear_operacion(config, texto, titulo, sistema_oscuro);
    ejecutar(
        parent,
        &op,
        gtk4::PrintOperationAction::Preview,
        "previsualizar la impresión",
    );
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

    #[test]
    fn pdf_partir_fila_basica() {
        assert_eq!(
            partir_fila("| a | b |"),
            vec!["a".to_string(), "b".to_string()]
        );
    }

    #[test]
    fn pdf_partir_fila_sin_pipes_exteriores() {
        assert_eq!(partir_fila("a | b"), vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn pdf_partir_fila_pipe_escapado() {
        assert_eq!(
            partir_fila("| a \\| b | c |"),
            vec!["a | b".to_string(), "c".to_string()]
        );
    }

    #[test]
    fn pdf_partir_fila_recorta_espacios() {
        assert_eq!(
            partir_fila("|   x   |  y  |"),
            vec!["x".to_string(), "y".to_string()]
        );
    }

    #[test]
    fn pdf_partir_fila_celda_vacia() {
        assert_eq!(partir_fila("| |"), vec![String::new()]);
    }

    #[test]
    fn pdf_parsear_alineaciones_de_delimitadora() {
        let al = parsear_alineaciones("| --- | :-: | --: | :- |").unwrap();
        assert_eq!(
            al,
            vec![
                AlineacionCelda::Izquierda,
                AlineacionCelda::Centro,
                AlineacionCelda::Derecha,
                AlineacionCelda::Izquierda,
            ]
        );
    }

    #[test]
    fn pdf_parsear_alineaciones_rechaza_fila_normal() {
        assert!(parsear_alineaciones("| a | b |").is_none());
        assert!(parsear_alineaciones("texto").is_none());
    }

    #[test]
    fn pdf_calcular_anchos_reparte_sobrante_proporcional() {
        let anchos = calcular_anchos(&[10.0, 20.0], &[10.0, 20.0], 60.0);
        assert!((anchos[0] - 20.0).abs() < 1e-6);
        assert!((anchos[1] - 40.0).abs() < 1e-6);
    }

    #[test]
    fn pdf_calcular_anchos_escala_si_no_caben_los_minimos() {
        let anchos = calcular_anchos(&[100.0, 100.0], &[100.0, 100.0], 50.0);
        assert!((anchos[0] - 25.0).abs() < 1e-6);
        assert!((anchos[1] - 25.0).abs() < 1e-6);
    }

    #[test]
    fn pdf_calcular_anchos_encoge_por_margen_respetando_minimos() {
        // Naturales 60/40, mínimos 10/40 y disponible 60: el déficit (40) se
        // reparte por margen (solo la columna 0 tiene margen 50), así que la
        // columna 0 queda en 20 y la 1 no baja de su mínimo.
        let anchos = calcular_anchos(&[60.0, 40.0], &[10.0, 40.0], 60.0);
        assert!((anchos[0] - 20.0).abs() < 1e-6);
        assert!((anchos[1] - 40.0).abs() < 1e-6);
    }

    #[test]
    fn pdf_construir_bloques_estructura_de_tabla() {
        let doc = "| Nombre | Edad |\n| --- | --: |\n| Ana | 34 |\n| Berto | 41 |\n";
        let analisis = analyze(doc);
        let bloques = construir_bloques(doc, &analisis);
        let tabla = bloques
            .iter()
            .find(|b| b.tipo == TipoBloque::Tabla)
            .expect("el documento debe producir un bloque de tabla");
        assert_eq!(
            tabla.celdas,
            vec![
                vec!["Nombre".to_string(), "Edad".to_string()],
                vec!["Ana".to_string(), "34".to_string()],
                vec!["Berto".to_string(), "41".to_string()],
            ]
        );
        assert_eq!(
            tabla.alineacion,
            vec![AlineacionCelda::Izquierda, AlineacionCelda::Derecha]
        );
    }

    fn config_de(template: ExportTemplate) -> ExportConfig {
        ExportConfig {
            esquema: ExportColorScheme::Light,
            familia_cuerpo: FontFamily::Sans,
            tamano_cuerpo: 11.0,
            fuente_mono: ExportMonoFont::AdwaitaMono,
            papel: ExportPageSize::A4,
            orientacion: ExportOrientation::Portrait,
            template,
        }
    }

    #[test]
    fn pdf_custom_no_impone_maquetacion_y_conserva_los_ajustes() {
        let config = config_de(ExportTemplate::Custom);
        let plantilla = Plantilla::desde(&config);
        assert!(plantilla.margenes.is_none());
        assert_eq!(plantilla.interlinea, INTERLINEA);
        assert_eq!(plantilla.espacio_bloque, ESPACIO_BLOQUE);
        assert_eq!(plantilla.cabecera, Adorno::Nada);
        assert_eq!(plantilla.pie, Adorno::Nada);
        assert!(!plantilla.bloque_titulo);
        let efectiva = config.con_plantilla(&plantilla);
        assert_eq!(efectiva.tamano_cuerpo, 11.0);
        assert_eq!(efectiva.familia_cuerpo, FontFamily::Sans);
    }

    #[test]
    fn pdf_article_reproduce_los_valores_de_siempre() {
        let config = config_de(ExportTemplate::Article);
        let plantilla = Plantilla::desde(&config);
        assert!(plantilla.margenes.is_none());
        assert_eq!(plantilla.interlinea, INTERLINEA);
        assert_eq!(plantilla.espacio_bloque, ESPACIO_BLOQUE);
        for (nivel, esperado) in [(1, 1.9), (2, 1.5), (3, 1.25), (4, 1.1)] {
            assert_eq!(plantilla.escala_cabecera(nivel), esperado);
        }
        assert_eq!(plantilla.banda_superior(), 0.0);
        assert_eq!(plantilla.banda_inferior(), 0.0);
        assert!(!plantilla.bloque_titulo);
    }

    #[test]
    fn pdf_manuscript_fija_margenes_interlineado_y_adornos() {
        let config = config_de(ExportTemplate::Manuscript);
        let plantilla = Plantilla::desde(&config);
        let margenes = plantilla.margenes.expect("manuscrito fija márgenes");
        assert_eq!(margenes, Margenes::simetricos(72.0));
        assert_eq!(plantilla.interlinea, 2.0);
        assert_eq!(plantilla.cabecera, Adorno::Titulo);
        assert_eq!(plantilla.pie, Adorno::NumeroPagina);
        assert!(plantilla.bloque_titulo);
        assert_eq!(plantilla.banda_superior(), BANDA_ADORNO);
        assert_eq!(plantilla.banda_inferior(), BANDA_ADORNO);
        let efectiva = config.con_plantilla(&plantilla);
        assert_eq!(efectiva.familia_cuerpo, FontFamily::Serif);
        assert_eq!(efectiva.tamano_cuerpo, 12.0);
    }

    #[test]
    fn pdf_compact_aprieta_el_espaciado_y_no_lleva_portada() {
        let config = config_de(ExportTemplate::Compact);
        let plantilla = Plantilla::desde(&config);
        assert_eq!(plantilla.margenes, Some(Margenes::simetricos(36.0)));
        assert_eq!(plantilla.interlinea, 1.25);
        assert_eq!(plantilla.espacio_bloque, 7.0);
        assert!(!plantilla.bloque_titulo);
        assert_eq!(plantilla.pie, Adorno::NumeroPagina);
    }

    #[test]
    fn pdf_letter_usa_margenes_ajustados_y_no_lleva_portada() {
        let config = config_de(ExportTemplate::Letter);
        let plantilla = Plantilla::desde(&config);
        assert_eq!(plantilla.margenes, Some(Margenes::simetricos(54.0)));
        assert!(!plantilla.bloque_titulo);
        assert_eq!(plantilla.cabecera, Adorno::Nada);
    }

    #[test]
    fn pdf_el_bloque_de_titulo_es_una_cabecera_de_nivel_uno() {
        let bloque = bloque_titulo("Mi documento");
        assert_eq!(bloque.tipo, TipoBloque::Cabecera { nivel: 1 });
        assert_eq!(bloque.frases.len(), 1);
        assert_eq!(bloque.frases[0].texto, "Mi documento");
    }

    #[test]
    fn pdf_el_pie_numera_con_el_total_de_paginas() {
        assert_eq!(
            texto_adorno(Adorno::NumeroPagina, "Título", 0, 3),
            Some("1 / 3".to_string())
        );
        assert_eq!(
            texto_adorno(Adorno::Titulo, "Título", 4, 9),
            Some("Título".to_string())
        );
        assert_eq!(texto_adorno(Adorno::Nada, "Título", 0, 1), None);
    }

    #[test]
    fn pdf_la_escala_de_cabecera_desconocida_no_crece() {
        let plantilla = Plantilla::desde(&config_de(ExportTemplate::Article));
        assert_eq!(plantilla.escala_cabecera(0), 1.0);
        assert_eq!(plantilla.escala_cabecera(9), 1.0);
    }
}
