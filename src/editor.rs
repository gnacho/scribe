use gdk_pixbuf::prelude::PixbufLoaderExt;
use gettextrs::gettext;
use gtk4::pango;
use gtk4::prelude::*;
use gtksourceview5::prelude::*;
use libadwaita as adw;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use crate::markdown_render::{
    analyze, canvas_layout, ornaments_for, pad_units, LinkRef, Ornament, Span, SpanKind,
    UnclosedFence, CANVAS_ROW_PAD, MAX_LIVE_BYTES,
};
use crate::markdown_view::{MarkdownView, OrnamentPalette, BLOCK_PADDING};
use crate::settings::{gtk_hides_invisible_safely, FontFamily, MarkupVisibility};
use std::path::{Path, PathBuf};

type ChangedCallback = Rc<RefCell<Option<Box<dyn Fn(&str)>>>>;
/// Aviso transitorio (toast) que la ventana muestra; lo conecta `window` al
/// crear el editor, igual que `connect_changed`.
type ToastCallback = Rc<RefCell<Option<Box<dyn Fn(&str)>>>>;

const MIN_MARGIN: i32 = 24;
/// Lado máximo (en píxeles) de una imagen pegada o soltada; si lo supera se
/// reescala manteniendo la proporción antes de guardarla.
const MAX_LADO_IMAGEN: u32 = 2560;

thread_local! {
    /// Provider CSS único de la aplicación (D9): antes cada Editor instalaba
    /// el suyo en el display y nunca lo retiraba, así que con pestañas se
    /// acumulaban tantos providers como documentos. Ahora lo crea main una
    /// sola vez y `set_font` lo recarga (el selector `textview.scribe-editor`
    /// aplica a todos los editores, que comparten la misma fuente).
    static APP_CSS: RefCell<Option<gtk4::CssProvider>> = const { RefCell::new(None) };
}

/// Instala el provider CSS de la aplicación. Lo llama main en el arranque;
/// sin él (tests headless que crean un Editor suelto) `set_font` simplemente
/// no inyecta CSS.
pub fn install_app_css(provider: gtk4::CssProvider) {
    APP_CSS.with(|slot| *slot.borrow_mut() = Some(provider));
}

/// Márgenes extra (izquierdo, derecho) de cada tag de bloque respecto al margen
/// de la columna. El derecho importa en los bloques con caja dibujada: deja el
/// texto por dentro del recuadro en vez de pegado al borde.
const BLOCK_INDENTS: [(&str, i32, i32); 6] = [
    ("quote", 26, 0),
    ("codeblock", 24, 24),
    ("table", 24, 24),
    ("li1", 26, 0),
    ("li2", 52, 0),
    ("li3", 78, 0),
];

#[derive(Clone, Copy)]
struct Decoration {
    markup: MarkupVisibility,
    focus_mode: bool,
    spellcheck: bool,
}

impl Default for Decoration {
    fn default() -> Self {
        Self {
            markup: MarkupVisibility::Focus,
            focus_mode: false,
            spellcheck: true,
        }
    }
}

/// Corrector ortográfico sobre enchant: el diccionario del locale del
/// sistema (hunspell es_ES, en_US, ...). Con `dict` a `None` (sin
/// diccionario instalado) no marca nada y todo valida.
struct Ortografia {
    dict: Option<enchant::Dict>,
}

impl Ortografia {
    /// Resuelve el diccionario con el locale del sistema (LC_MESSAGES o
    /// LANG, sin codificacion ni modificador); si enchant no lo tiene,
    /// prueba ingles y se queda sin.
    fn con_locale_del_sistema() -> Self {
        let mut broker = enchant::Broker::new();
        let mut dict = None;
        let senales = [
            std::env::var("LC_MESSAGES").ok(),
            std::env::var("LANG").ok(),
        ];
        for valor in senales.iter().flatten() {
            let base = valor
                .split('.')
                .next()
                .and_then(|sin_codigo| sin_codigo.split('@').next())
                .unwrap_or_default();
            if base.chars().count() < 2 {
                continue;
            }
            if let Ok(encontrado) = broker.request_dict(base) {
                dict = Some(encontrado);
                break;
            }
        }
        if dict.is_none() {
            dict = broker.request_dict("en_US").ok();
        }
        Self { dict }
    }

    /// Valida una palabra: `true` si el diccionario la conoce (o si no hay
    /// diccionario, en cuyo caso no se marca nada).
    fn valida(&self, palabra: &str) -> bool {
        match &self.dict {
            Some(dict) => dict.check(palabra).unwrap_or(true),
            None => true,
        }
    }
}

/// Contexto de una pasada de decoración: todo lo que `decorate` necesita
/// sobre el documento activo. Clonable: los cierres de señales capturan una
/// copia y cada re-decoración la consume.
#[derive(Clone)]
struct DecoracionCtx {
    view: glib::WeakRef<MarkdownView>,
    buffer: glib::WeakRef<gtksourceview5::Buffer>,
    decoration: Rc<Cell<Decoration>>,
    generation: Rc<Cell<u64>>,
    base_dir: Rc<RefCell<Option<PathBuf>>>,
    unclosed_fences: Rc<RefCell<Vec<UnclosedFence>>>,
    ortografia: Rc<Ortografia>,
    /// Arranque sin salto: el primer ancho valido arma `por_revelar` y la
    /// siguiente pasada de decoracion revela la vista ya compuesta.
    revelada: Rc<Cell<bool>>,
    por_revelar: Rc<Cell<bool>>,
}

pub struct Editor {
    pub widget: gtk4::ScrolledWindow,
    pub view: MarkdownView,
    buffer: gtksourceview5::Buffer,
    tags: gtk4::TextTagTable,
    on_changed: ChangedCallback,
    last_line: Rc<Cell<i32>>,
    decoration: Rc<Cell<Decoration>>,
    column_width: Rc<Cell<i32>>,
    continue_lists: Rc<Cell<bool>>,
    typewriter: Rc<Cell<bool>>,
    /// Directorio del documento abierto, contra el que se resuelven las
    /// rutas relativas de las imágenes. `None` = no resolver (placeholder).
    base_dir: Rc<RefCell<Option<PathBuf>>>,
    /// Vallas de código abiertas detectadas en el último `decorate`. La barra
    /// de estado de la ventana las consulta para avisar al usuario.
    unclosed_fences: Rc<RefCell<Vec<UnclosedFence>>>,
    /// Aviso transitorio (toast) que la ventana conecta al crear el editor;
    /// se usa para los avisos de pegado/soltado de imágenes.
    on_toast: ToastCallback,
    /// Ajustes de búsqueda del documento (patrón, vuelta al final).
    search_settings: gtksourceview5::SearchSettings,
    /// Contexto de búsqueda del documento, vivo toda la vida del editor: el
    /// índice se mantiene caliente aunque la barra de búsqueda esté cerrada.
    search_context: gtksourceview5::SearchContext,
    /// Contexto de las pasadas de decoración: lo capturan los cierres de
    /// señales por copia y lleva dentro el corrector ortográfico.
    decoracion_ctx: DecoracionCtx,
}

fn build_tags() -> gtk4::TextTagTable {
    let table = gtk4::TextTagTable::new();
    let add = |t: gtk4::TextTag| {
        table.add(&t);
    };

    // El orden fija la prioridad: lo añadido después gana.
    // Bloques → cabeceras → en línea → marcas → atenuado de foco.

    for (name, _, _) in BLOCK_INDENTS {
        let builder = gtk4::TextTag::builder().name(name);
        let tag = match name {
            "quote" => builder.style(pango::Style::Italic).build(),
            // `table` vuelve a monoespaciada: la rejilla visual se apoya en el
            // padding del fuente (véase `format_tables`) y eso exige ancho
            // fijo. El inline (negrita, cursiva, color, código) son otros
            // atributos y siguen aplicando encima sin problema.
            "table" => builder.family("monospace").build(),
            "codeblock" => builder.family("monospace").scale(0.9).build(),
            // Sin sangría francesa: la viñeta se dibuja en el canalón, así que
            // todas las líneas del elemento arrancan en el mismo sitio.
            _ => builder.build(),
        };
        add(tag);
    }

    // La fila de guiones va oculta: se encoge para que el hueco donde se pinta
    // la línea de cabecera no ocupe un renglón entero.
    add(gtk4::TextTag::builder()
        .name("tablerule")
        .scale(0.4)
        .pixels_above_lines(0)
        .pixels_below_lines(0)
        .build());
    // Texto de la fila de cabecera de una tabla: negrita, sin fondo (la caja
    // del bloque ya lo da).
    add(gtk4::TextTag::builder()
        .name("tablehead")
        .weight(700)
        .build());
    add(gtk4::TextTag::builder()
        .name("fence")
        .family("monospace")
        .scale(0.72)
        .build());
    add(gtk4::TextTag::builder()
        .name("html")
        .family("monospace")
        .scale(0.85)
        .build());
    // Hueco bajo la línea de una imagen en bloque: ahí se pinta la textura
    // (o el placeholder) desde MarkdownView.
    add(gtk4::TextTag::builder()
        .name("imagegap")
        .pixels_below_lines(crate::markdown_render::IMAGE_GAP_HEIGHT)
        .build());
    // Y el de una tarjeta de vídeo, más alto porque va en 16:9.
    add(gtk4::TextTag::builder()
        .name("youtubegap")
        .pixels_below_lines(crate::markdown_render::YOUTUBE_GAP_HEIGHT)
        .build());

    // Nota: GtkTextTag:letter-spacing no admite valores negativos, así que el
    // ajuste óptico de las cabeceras se hace solo con escala y peso.
    for (name, scale, weight, above, below) in [
        ("h1", 1.9_f64, 800, 30, 12),
        ("h2", 1.5, 750, 26, 10),
        ("h3", 1.24, 700, 20, 8),
        ("h4", 1.1, 700, 16, 6),
        ("h5", 1.0, 700, 14, 5),
        ("h6", 1.0, 700, 14, 5),
    ] {
        add(gtk4::TextTag::builder()
            .name(name)
            .scale(scale)
            .weight(weight)
            .pixels_above_lines(above)
            .pixels_below_lines(below)
            .build());
    }

    add(gtk4::TextTag::builder().name("bold").weight(700).build());
    add(gtk4::TextTag::builder()
        .name("italic")
        .style(pango::Style::Italic)
        .build());
    add(gtk4::TextTag::builder()
        .name("strike")
        .strikethrough(true)
        .build());
    add(gtk4::TextTag::builder()
        .name("code")
        .family("monospace")
        .scale(0.9)
        .build());
    // Código en línea dentro de una tabla: monoespaciado y a escala 1.0, para
    // que todos los tramos de la fila avancen lo mismo. La rejilla de la
    // tabla se calcula en celdas de carácter y una escala distinta la tuerce.
    add(gtk4::TextTag::builder()
        .name("tablecode")
        .family("monospace")
        .build());
    add(gtk4::TextTag::builder()
        .name("link")
        .underline(pango::Underline::Single)
        .build());
    add(gtk4::TextTag::builder()
        .name("footnote")
        .scale(0.8)
        .rise(4000)
        .build());
    add(gtk4::TextTag::builder()
        .name("footnotedef")
        .scale(0.88)
        .build());
    add(gtk4::TextTag::builder()
        .name("listmarker")
        .weight(700)
        .build());

    // Registrado pero NUNCA aplicado mientras
    // `settings::gtk_hides_invisible_safely()` sea falso: GTK aborta en
    // `gtk_text_iter_set_visible_line_index` cuando el buffer contiene texto
    // invisible (GNOME/gtk#8346; el fix, MR !10228, sigue sin publicarse).
    // Se conserva para reactivar el modo «ocultar» con un GTK sano.
    add(gtk4::TextTag::builder()
        .name("syn_hidden")
        .invisible(true)
        .build());
    add(gtk4::TextTag::builder().name("syn_shown").build());

    // El modo foco atenúa todo salvo el párrafo actual. Va DESPUÉS de todos
    // los tags de estilo para pisar su color, pero ANTES de `syn_shrink`: si
    // no, su foreground opaco haría visibles las marcas encogidas (motas de
    // ~1px) en los párrafos atenuados.
    add(gtk4::TextTag::builder().name("unfocused").build());

    // Encoger en vez de ocultar (mitigación de GNOME/gtk#8346): la marca
    // sustituida sigue en la maquetación con escala mínima y totalmente
    // transparente, así que el texto nunca queda excluido del layout y el
    // camino del aborto es inalcanzable por construcción. El color se fija
    // aquí y no en el tema: debe ser transparente con cualquier paleta.
    add(gtk4::TextTag::builder()
        .name("syn_shrink")
        .scale(0.05)
        .foreground_rgba(&rgba(0x000000, 0.0))
        .build());
    // Filas fuente de un lienzo de tabla pintado: colapsan (igual que las
    // marcas encogidas) y la vista pinta la fila real bajo su hueco.
    // Corrector ortografico (issue #51): subrayado de las palabras que el
    // diccionario no valida, en el rojo de error de GNOME.
    add(gtk4::TextTag::builder()
        .name("ortografia")
        .underline(pango::Underline::Single)
        .underline_rgba(&rgba(0xe01b24, 1.0))
        .build());
    add(gtk4::TextTag::builder()
        .name("tcanvas_src")
        .scale(0.0)
        .foreground_rgba(&rgba(0x000000, 0.0))
        .build());

    table
}

/// GtkSourceView pinta el fondo desde su *style scheme*, no desde el tema GTK.
fn apply_scheme(buffer: &gtksourceview5::Buffer, dark: bool) {
    let manager = gtksourceview5::StyleSchemeManager::default();
    let candidates: [&str; 2] = if dark {
        ["Adwaita-dark", "classic-dark"]
    } else {
        ["Adwaita", "classic"]
    };
    for id in candidates {
        if let Some(scheme) = manager.scheme(id) {
            buffer.set_style_scheme(Some(&scheme));
            return;
        }
    }
}

fn rgba(hex: u32, alpha: f32) -> gtk4::gdk::RGBA {
    gtk4::gdk::RGBA::new(
        ((hex >> 16) & 0xff) as f32 / 255.0,
        ((hex >> 8) & 0xff) as f32 / 255.0,
        (hex & 0xff) as f32 / 255.0,
        alpha,
    )
}

fn apply_theme(tags: &gtk4::TextTagTable, view: &MarkdownView, dark: bool) {
    let set = |name: &str, fg: Option<&str>, para_bg: Option<&str>| {
        if let Some(tag) = tags.lookup(name) {
            tag.set_foreground(fg);
            tag.set_paragraph_background(para_bg);
        }
    };

    // Las citas y los bloques de código ya no llevan fondo de párrafo: su caja
    // y su barra se dibujan, que permite esquinas redondeadas y márgenes.
    if dark {
        set("code", Some("#f0a868"), None);
        set("tablecode", Some("#f0a868"), None);
        set("codeblock", Some("#e4e4e4"), None);
        set("table", Some("#e0e0e0"), None);
        set("tablehead", Some("#e0e0e0"), None);
        set("fence", Some("#8a8a8a"), None);
        set("quote", Some("#c2c2c2"), None);
        set("link", Some("#82b8f0"), None);
        set("footnote", Some("#82b8f0"), None);
        set("listmarker", Some("#82b8f0"), None);
        set("html", Some("#8f8f8f"), None);
        set("footnotedef", Some("#a0a0a0"), None);
        set("syn_shown", Some("#787878"), None);
        set("unfocused", Some("#5c5c5c"), None);
        view.set_palette(OrnamentPalette {
            accent: rgba(0x82b8f0, 1.0),
            muted: rgba(0x6f6f6f, 1.0),
            block: rgba(0xffffff, 0.07),
            table: rgba(0xffffff, 0.05),
            quote: rgba(0x6f6f6f, 1.0),
            on_accent: rgba(0x1b1b1b, 1.0),
        });
    } else {
        set("code", Some("#a34a00"), None);
        set("tablecode", Some("#a34a00"), None);
        set("codeblock", Some("#1f1f1f"), None);
        set("table", Some("#1f1f1f"), None);
        set("tablehead", Some("#1f1f1f"), None);
        set("fence", Some("#8b8a88"), None);
        set("quote", Some("#54535a"), None);
        set("link", Some("#1a6ed8"), None);
        set("footnote", Some("#1a6ed8"), None);
        set("listmarker", Some("#1a6ed8"), None);
        set("html", Some("#8b8a88"), None);
        set("footnotedef", Some("#77767b"), None);
        set("syn_shown", Some("#b5b4b1"), None);
        set("unfocused", Some("#bdbcba"), None);
        view.set_palette(OrnamentPalette {
            accent: rgba(0x1a6ed8, 1.0),
            muted: rgba(0xc0bfbc, 1.0),
            block: rgba(0x000000, 0.05),
            table: rgba(0x000000, 0.035),
            quote: rgba(0xc0bfbc, 1.0),
            on_accent: rgba(0xffffff, 1.0),
        });
    }
}

/// Aplica el ancho de columna **fuera** del ciclo de validación del
/// `GtkTextView`.
///
/// El aviso `page-size` que dispara el recolocado se emite desde dentro de
/// `gtk_text_view_validate_onscreen` (`set_hadjustment_values`, llamado por
/// `update_adjustments`). Cambiar ahí los márgenes invalida el layout, de modo
/// que el `g_assert (priv->onscreen_validated)` que va justo después encuentra
/// el flag en FALSE y aborta la aplicación (GTK >= 4.22.5, `gtktextview.c:4924`).
/// Por eso el cambio se programa en un idle: cuando corre, la validación ya
/// terminó y su invalidación solo pide otra, que es el camino previsto por GTK.
fn apply_column_margins_deferred(
    view: &MarkdownView,
    tags: &gtk4::TextTagTable,
    ctx: &DecoracionCtx,
    margin: i32,
) {
    if view.left_margin() == margin {
        return;
    }
    let view = view.clone();
    let tags = tags.clone();
    let ctx = ctx.clone();
    glib::idle_add_local_once(move || {
        if view.left_margin() == margin {
            return;
        }
        view.set_left_margin(margin);
        view.set_right_margin(margin);
        set_column_margins(&tags, margin);
        // Arranque sin salto: el margen recien aplicado es el que usa la
        // decoracion de revelado; armar aqui (despues de aplicar) y pedir
        // una pasada garantiza que lo primero visible ya sale compuesto.
        if !ctx.revelada.get() {
            // Armado del revelado de arranque: la proxima pasada ya corre
            // con este margen aplicado y sale compuesta.
            ctx.por_revelar.set(true);
        }
        schedule_decoration(&ctx, Duration::ZERO);
    });
}

fn set_column_margins(tags: &gtk4::TextTagTable, base: i32) {
    for (name, left, right) in BLOCK_INDENTS {
        if let Some(tag) = tags.lookup(name) {
            tag.set_left_margin(base + left);
            tag.set_right_margin(base + right);
        }
    }
    if let Some(tag) = tags.lookup("fence") {
        tag.set_left_margin(base + 24);
        tag.set_right_margin(base + 24);
    }
}

/// Límites del párrafo (bloque entre líneas en blanco) que contiene `line`.
fn paragraph_bounds(buffer: &gtksourceview5::Buffer, line: i32) -> (i32, i32) {
    let is_blank = |n: i32| -> bool {
        match buffer.iter_at_line(n) {
            Some(start) => {
                let mut end = start;
                if !end.ends_line() {
                    end.forward_to_line_end();
                }
                buffer.text(&start, &end, true).trim().is_empty()
            }
            None => true,
        }
    };
    let mut first = line;
    while first > 0 && !is_blank(first - 1) {
        first -= 1;
    }
    let last_line = buffer.end_iter().line();
    let mut last = line;
    while last < last_line && !is_blank(last + 1) {
        last += 1;
    }
    (first, last)
}

/// Toda la decoracion pasa por aqui.
///
/// Nunca se aplican tags desde dentro de una senal del buffer: GTK no espera
/// que la invisibilidad de su texto cambie mientras esta procesando una
/// edicion, y hacerlo descuadra la maquetacion. El sintoma es un aborto en
/// `gtk_text_iter_set_visible_line_index` («byte index off the end of the
/// line») y, antes de llegar ahi, tags de bloque que se extienden mas alla de
/// su rango. Se difiere siempre al bucle principal.
fn schedule_decoration(ctx: &DecoracionCtx, delay: Duration) {
    let config = ctx.decoration.get();
    let current = ctx.generation.get().wrapping_add(1);
    ctx.generation.set(current);

    let ctx = ctx.clone();
    glib::timeout_add_local_once(delay, move || {
        // Si ha llegado otra peticion mientras esperabamos, esta sobra.
        if ctx.generation.get() != current {
            return;
        }
        let Some(view) = ctx.view.upgrade() else {
            return;
        };
        let Some(buffer) = ctx.buffer.upgrade() else {
            return;
        };
        decorate(&view, &buffer, &ctx, config);
    });
}

/// Resuelve el destino de una imagen contra el directorio del documento.
/// Abre una URL en la aplicación predeterminada del escritorio (el navegador,
/// el cliente de correo...). El `UriLauncher` de GTK respeta el portal de
/// escritorio; si no hay nadie que atienda el esquema, se avisa por la salida
/// de error y no se rompe nada.
fn abrir_en_navegador(widget: &impl IsA<gtk4::Widget>, url: &str) {
    let ventana = widget
        .root()
        .and_then(|root| root.downcast::<gtk4::Window>().ok());
    let uri = url.to_string();
    gtk4::UriLauncher::new(url).launch(
        ventana.as_ref(),
        gio::Cancellable::NONE,
        move |resultado| {
            if let Err(error) = resultado {
                eprintln!("no se pudo abrir {uri}: {error}");
            }
        },
    );
}

/// Las URLs y las rutas absolutas se devuelven tal cual; sin directorio base,
/// la relativa se devuelve sin resolver (el widget pintará el placeholder).
/// `~` no se expande.
fn resolve_image_dest(dest: &str, base_dir: &Option<PathBuf>) -> String {
    let is_url = dest.starts_with("http://") || dest.starts_with("https://");
    if is_url || std::path::Path::new(dest).is_absolute() {
        return dest.to_string();
    }
    match base_dir {
        Some(dir) => dir.join(dest).to_string_lossy().into_owned(),
        None => dest.to_string(),
    }
}

/// Cap de palabras por pasada de ortografia: un documento gigante no debe
/// pagar miles de busquedas de hunspell en cada tecla (la pasada siguiente
/// cubrira lo que falte si el usuario sigue escribiendo cerca del final).
const ORTOGRAFIA_MAX_PALABRAS: usize = 4000;

/// Es esta pieza parte de una palabra? Letras y apostrofo interior.
fn es_de_palabra(ch: char, previa: Option<char>) -> bool {
    if ch.is_alphabetic() {
        return true;
    }
    // Apostrofo interior (castellano: "del'", frances/ingles: "don't").
    ch == '\'' && previa.is_some_and(|p| p.is_alphabetic())
}

/// Pase de ortografia (issue #51): recorre el texto, marca con el tag
/// «ortografia» las palabras que el diccionario no valida y, si el
/// corrector esta apagado, limpia cualquier marca previa.
fn revisa_ortografia(
    buffer: &gtksourceview5::Buffer,
    ortografia: &Ortografia,
    text: &str,
    byte_to_char: &[i32],
    enabled: bool,
) {
    let start = buffer.start_iter();
    let end = buffer.end_iter();
    let subrayado = match buffer.tag_table().lookup("ortografia") {
        Some(tag) => tag,
        None => return,
    };
    buffer.remove_tag(&subrayado, &start, &end);
    if !enabled {
        return;
    }

    let mut word_start: Option<usize> = None;
    let mut previa: Option<char> = None;
    let mut revisadas = 0usize;

    let mut i = 0usize;
    while i <= text.len() {
        let ch = text[i..].chars().next();
        match ch {
            Some(c) if es_de_palabra(c, previa) => {
                if word_start.is_none() {
                    word_start = Some(i);
                }
                previa = Some(c);
                i += c.len_utf8();
            }
            _ => {
                if let Some(desde) = word_start.take() {
                    let palabra = &text[desde..i];
                    revisadas += 1;
                    if revisadas <= ORTOGRAFIA_MAX_PALABRAS
                        && palabra.chars().count() >= 2
                        && !ortografia.valida(palabra)
                    {
                        let (Some(&from), Some(&to)) =
                            (byte_to_char.get(desde), byte_to_char.get(i.min(text.len())))
                        else {
                            continue;
                        };
                        if from < to {
                            let ini = buffer.iter_at_offset(from);
                            let fin = buffer.iter_at_offset(to);
                            buffer.apply_tag_by_name("ortografia", &ini, &fin);
                        }
                    }
                }
                previa = None;
                if let Some(c) = ch {
                    i += c.len_utf8();
                } else {
                    break;
                }
            }
        }
    }
}

fn decorate(
    view: &MarkdownView,
    buffer: &gtksourceview5::Buffer,
    ctx: &DecoracionCtx,
    config: Decoration,
) {
    let base_dir = &ctx.base_dir;
    let unclosed_fences = &ctx.unclosed_fences;
    let ortografia = &ctx.ortografia;
    let start = buffer.start_iter();
    let end = buffer.end_iter();
    // Se retiran SOLO los tags con nombre (los nuestros): el buffer puede
    // llevar tags de otras librerias creados sin nombre (el corrector
    // ortografico marca con uno anonimo) que remove_all_tags borraria sin
    // que nadie los vuelva a aplicar.
    let mut con_nombre: Vec<gtk4::TextTag> = Vec::new();
    buffer.tag_table().foreach(|t| {
        if t.name().is_some() {
            con_nombre.push(t.clone());
        }
    });
    for tag in &con_nombre {
        buffer.remove_tag(tag, &start, &end);
    }

    let line_count = end.line() + 1;
    let text = buffer.text(&start, &end, true).to_string();
    if text.is_empty() || text.len() > MAX_LIVE_BYTES {
        unclosed_fences.borrow_mut().clear();
        view.set_ornaments(Vec::new(), Vec::new(), line_count);
        if !ctx.revelada.get() && !ctx.por_revelar.get() {
            ctx.revelada.set(true);
            view.set_opacity(1.0);
        }
        return;
    }

    // GtkTextBuffer indexa por caracteres; pulldown-cmark, por bytes.
    let mut byte_to_char = vec![0i32; text.len() + 1];
    let mut char_index = 0i32;
    let mut byte_index = 0usize;
    for ch in text.chars() {
        for slot in byte_to_char.iter_mut().skip(byte_index).take(ch.len_utf8()) {
            *slot = char_index;
        }
        byte_index += ch.len_utf8();
        char_index += 1;
    }

    // Corrector ortografico (issue #51): subraya las palabras que el checker
    // del diccionario no valida. Va aqui, donde byte_to_char ya traduce a
    // offsets del widget. La pasada anterior se borro al inicio.
    revisa_ortografia(
        buffer,
        ortografia,
        text.as_str(),
        &byte_to_char,
        config.spellcheck,
    );
    byte_to_char[text.len()] = char_index;

    let analysis = analyze(&text);
    *unclosed_fences.borrow_mut() = analysis.unclosed_fences.clone();
    let cursor_line = buffer.iter_at_offset(buffer.cursor_position()).line();
    // Árbol de decisión de visibilidad. La preferencia persistida
    // (`config.markup`) decide el look; la puerta
    // `gtk_hides_invisible_safely()` (GNOME/gtk#8346) solo decide si las
    // marcas sustituidas se encogen (`syn_shrink`, hoy siempre) o se ocultan
    // de verdad (`syn_hidden`, futuro GTK sano):
    //
    // - «Atenuar» (Dim): marcas y sustituidas → `syn_shown`; solo se pintan
    //   los adornos ambientales y aditivos, porque un sustituto taparía su
    //   marca visible.
    // - «Ocultar»/«Al enfocar» sin puerta: marcas → `syn_shown`; sustituidas
    //   → `syn_shrink` (encogidas, no ocultas: véase build_tags); se pintan
    //   todos los adornos. «Al enfocar» equivale a «Ocultar»: ya no hay
    //   revelado por línea (véase preferences.rs).
    // - «Ocultar»/«Al enfocar» con puerta: comportamiento clásico —
    //   `syn_hidden` y todos los adornos.
    let gate = gtk_hides_invisible_safely();
    let dim = config.markup == MarkupVisibility::Dim;

    // En la línea del cursor el fuente se ve crudo: los adornos que sustituyen
    // a una marca (los pipes de una tabla, su fila de guiones) quedan detrás de
    // lo que el usuario está editando. Fuera de las tablas no se revela nada,
    // que es la política que ya tenía la vista.
    let mut starts = vec![0usize];
    for (i, b) in text.bytes().enumerate() {
        if b == b'\n' {
            starts.push(i + 1);
        }
    }
    let linea_desde = starts
        .get(cursor_line as usize)
        .copied()
        .unwrap_or(text.len());
    let linea_hasta = starts
        .get(cursor_line as usize + 1)
        .copied()
        .unwrap_or(text.len());
    let (cursor_table, cursor_from, cursor_to) = {
        let line = cursor_line as usize;
        let in_table = analysis.ornaments.iter().any(
            |o| matches!(o, Ornament::Table { first, last } if *first <= line && line <= *last),
        );
        if in_table {
            let from = starts.get(line).copied().unwrap_or(text.len());
            let to = starts.get(line + 1).copied().unwrap_or(text.len());
            (true, from, to)
        } else {
            (false, 0, 0)
        }
    };
    let revealed = |span: &Span| -> bool {
        !dim && cursor_table && span.start >= cursor_from && span.end <= cursor_to
    };

    // Revelado de arranque: cuando el ancho real ya aplico sus margenes y
    // esta pasada termina, todo lo visible sale compuesto de una pieza.
    if !ctx.revelada.get() && ctx.por_revelar.get() {
        ctx.revelada.set(true);
        view.set_opacity(1.0);
    }

    for span in &analysis.spans {
        eprintln!(
            "LOOP DBG {:?} tag={} rango=({},{})",
            span.kind, span.tag, span.start, span.end
        );
        // Los rangos del parser deberían estar siempre dentro del texto, pero
        // un cambio en pulldown-cmark o en analyze no debe convertirse en un
        // panic en producción.
        debug_assert!(span.end <= text.len(), "span fuera de rango: {span:?}");
        let (from, to) = match (byte_to_char.get(span.start), byte_to_char.get(span.end)) {
            (Some(&from), Some(&to)) => (from, to),
            _ => continue,
        };
        if from >= to {
            continue;
        }
        if span.tag == "tablerule" && revealed(span) {
            // La fila de guiones se está editando: sin encoger y sin filete,
            // para que se vea el fuente.
            continue;
        }
        let start_iter = buffer.iter_at_offset(from);
        let end_iter = buffer.iter_at_offset(to);
        let name = match span.kind {
            SpanKind::Style => span.tag,
            SpanKind::Marker => {
                // Las marcas se muestran en la línea del cursor (para poder
                // editarlas) y se encogen en el resto: la escala minima es el
                // ocultado seguro sin depender de la puerta GTK.
                if revealed(span) {
                    "syn_shown"
                } else {
                    let en_cursor = span.start >= linea_desde && span.end <= linea_hasta;
                    match config.markup {
                        MarkupVisibility::Dim => {
                            if en_cursor {
                                "syn_shown"
                            } else {
                                "syn_shrink"
                            }
                        }
                        MarkupVisibility::Focus => {
                            if en_cursor {
                                "syn_shown"
                            } else {
                                "syn_shrink"
                            }
                        }
                        MarkupVisibility::Hidden => {
                            if gate {
                                "syn_hidden"
                            } else {
                                "syn_shrink"
                            }
                        }
                    }
                }
            }
            SpanKind::Replaced => {
                // Las marcas sustituidas por un adorno (el `- ` de las listas,
                // el `# ` de los titulos) se muestran en la línea del cursor y
                // se encogen en el resto. Sin la puerta GTK, encoger es el
                // ocultado seguro.
                let en_cursor = span.start >= linea_desde && span.end <= linea_hasta;
                if revealed(span) {
                    "syn_shown"
                } else {
                    match config.markup {
                        MarkupVisibility::Dim => {
                            if en_cursor {
                                "syn_shown"
                            } else {
                                "syn_shrink"
                            }
                        }
                        MarkupVisibility::Focus => {
                            if en_cursor {
                                "syn_shown"
                            } else {
                                "syn_shrink"
                            }
                        }
                        MarkupVisibility::Hidden => {
                            if gate {
                                "syn_hidden"
                            } else {
                                "syn_shrink"
                            }
                        }
                    }
                }
            }
        };
        buffer.apply_tag_by_name(name, &start_iter, &end_iter);
    }

    // Relleno de las celdas de tabla: `letter-spacing` sobre el `|` que
    // precede a cada celda, para que su contenido caiga en la columna que marca
    // la fila de guiones y no en la X que le deja el relleno del fuente. El
    // avance de celda se mide del widget (Pango); GTK no expone el del tag.
    let advance = view.cell_advance();
    if advance > 0.0 {
        for ornament in &analysis.ornaments {
            let Ornament::TableGrid { pads, .. } = ornament else {
                continue;
            };
            for pad in pads {
                // El ancla que abre su línea (el `|` inicial de la celda 0)
                // recibe la mitad del desplazamiento: `pad_units` lo compensa.
                let opens_line =
                    pad.offset == 0 || text.as_bytes().get(pad.offset - 1) == Some(&b'\n');
                let units = pad_units(pad.cells, advance, opens_line);
                if units <= 0 {
                    continue;
                }
                let (Some(&from), Some(&to)) = (
                    byte_to_char.get(pad.offset),
                    byte_to_char.get(pad.offset + 1),
                ) else {
                    continue;
                };
                if from >= to {
                    continue;
                }
                let start_iter = buffer.iter_at_offset(from);
                if start_iter.line() == cursor_line {
                    continue;
                }
                let end_iter = buffer.iter_at_offset(to);
                let name = view.pad_tag(units);
                buffer.apply_tag_by_name(&name, &start_iter, &end_iter);
            }
        }
    }

    // Los adornos se filtran por familia según la preferencia (véase
    // `ornaments_for`): en «Atenuar» solo ambientales y aditivos; en los
    // modos WYSIWYG todos, con las marcas sustituidas encogidas u ocultas.
    let mut ornaments = ornaments_for(&analysis.ornaments, config.markup);
    // Lienzos de tabla (tablas de prosa que no caben en la rejilla): cada
    // fila fuente colapsa y bajo ella se reserva, con `pixels_below_lines`, el
    // alto de la fila que pinta la vista. La fila del cursor se queda cruda,
    // como el revelado de la rejilla. En el modo «Atenuar» el adorno se filtra
    // antes de llegar aquí y el fuente se ve entero. El ancho de columna usa
    // la MISMA fórmula que la vista para que el hueco y la pintura casen.
    {
        let advance = view.cell_advance();
        if advance > 0.0 {
            let line_height = view.cell_height();
            let column_cells = ((view.width() - view.left_margin() - view.right_margin()) as f32
                - 2.0 * BLOCK_PADDING)
                .max(0.0)
                / advance;
            if column_cells >= 16.0 {
                // Con la vista aún sin asignar (decorado inicial) el ancho es
                // irreal y el envoltorio daría unas alturas absurdas: se
                // espera al notify de ancho, que pide una re-decoración.
                let mut line_starts = vec![0usize];
                for (i, b) in text.bytes().enumerate() {
                    if b == b'\n' {
                        line_starts.push(i + 1);
                    }
                }
                for ornament in &ornaments {
                    let Ornament::TableCanvas {
                        columns,
                        rows,
                        anchor,
                        ..
                    } = ornament
                    else {
                        continue;
                    };
                    let Some(layout) = canvas_layout(columns, rows, column_cells) else {
                        continue;
                    };
                    let colapsa = |linea: usize| {
                        if linea as i32 == cursor_line {
                            return;
                        }
                        let desde = line_starts.get(linea).copied().unwrap_or(text.len());
                        let hasta = line_starts.get(linea + 1).copied().unwrap_or(text.len());
                        let (Some(&from), Some(&to)) =
                            (byte_to_char.get(desde), byte_to_char.get(hasta))
                        else {
                            return;
                        };
                        if from >= to {
                            return;
                        }
                        let start_iter = buffer.iter_at_offset(from);
                        let end_iter = buffer.iter_at_offset(to);
                        buffer.apply_tag_by_name("tcanvas_src", &start_iter, &end_iter);
                    };
                    // La fila delimitadora colapsa sin hueco: su geometría la
                    // pinta el lienzo.
                    colapsa(*anchor);
                    for (ri, row) in rows.iter().enumerate() {
                        colapsa(row.line);
                        if row.line as i32 == cursor_line {
                            continue;
                        }
                        let desde = line_starts.get(row.line).copied().unwrap_or(text.len());
                        let hasta = line_starts.get(row.line + 1).copied().unwrap_or(text.len());
                        let (Some(&from), Some(&to)) =
                            (byte_to_char.get(desde), byte_to_char.get(hasta))
                        else {
                            continue;
                        };
                        if from >= to {
                            continue;
                        }
                        let gap_px = (layout.row_lines[ri] as f32 * line_height
                            + 2.0 * CANVAS_ROW_PAD)
                            .ceil() as i32;
                        let name = view.gap_tag(gap_px);
                        let start_iter = buffer.iter_at_offset(from);
                        let end_iter = buffer.iter_at_offset(to);
                        buffer.apply_tag_by_name(&name, &start_iter, &end_iter);
                    }
                }
            }
        }
    }

    // Los adornos van por número de línea salvo `Break`, que guarda un byte
    // offset: el widget indexa por caracteres, así que lo convertimos.
    for o in &mut ornaments {
        let cap = byte_to_char.len().saturating_sub(1);
        match o {
            Ornament::Break { offset } => {
                let at = (*offset).min(cap);
                *offset = byte_to_char[at] as usize;
            }
            // Las rutas relativas se resuelven contra el directorio del
            // documento: el painter solo trabaja con rutas absolutas (o con
            // URLs, que no carga: pinta el placeholder).
            Ornament::Image { dest, .. } => {
                *dest = resolve_image_dest(dest, &base_dir.borrow());
            }
            _ => {}
        }
    }
    // Los enlaces vienen en bytes (pulldown-cmark) y el widget indexa por
    // caracteres: se traducen con la misma tabla que los tramos, o los enlaces
    // de un texto con acentos apuntarían a otro sitio.
    let links: Vec<LinkRef> = analysis
        .links
        .iter()
        .filter_map(|link| {
            let (from, to) = (
                byte_to_char.get(link.start).copied()?,
                byte_to_char.get(link.end).copied()?,
            );
            (from < to).then(|| LinkRef {
                start: from as usize,
                end: to as usize,
                dest: link.dest.clone(),
            })
        })
        .collect();
    view.set_ornaments(ornaments, links, line_count);

    if config.focus_mode {
        apply_focus_shade(buffer, cursor_line);
    }
}

/// Atenúa todo salvo el párrafo del cursor (modo foco). Es barato — solo
/// mueve un tag, sin re-analizar el documento — y por eso puede llamarse en
/// cada cambio de línea del cursor.
fn apply_focus_shade(buffer: &gtksourceview5::Buffer, cursor_line: i32) {
    buffer.remove_tag_by_name("unfocused", &buffer.start_iter(), &buffer.end_iter());
    let (first, last) = paragraph_bounds(buffer, cursor_line);
    if let Some(para_start) = buffer.iter_at_line(first) {
        buffer.apply_tag_by_name("unfocused", &buffer.start_iter(), &para_start);
    }
    let after = buffer
        .iter_at_line(last + 1)
        .unwrap_or_else(|| buffer.end_iter());
    buffer.apply_tag_by_name("unfocused", &after, &buffer.end_iter());
}

/// Marcador que continúa una lista, o `None` si la línea no es un elemento.
/// El `bool` indica que el elemento está vacío, en cuyo caso hay que cerrarlo.
fn list_continuation(line: &str) -> Option<(String, bool)> {
    let indent: String = line
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect();
    let rest = &line[indent.len()..];
    let first = rest.chars().next()?;

    let (marker, body) = if matches!(first, '-' | '*' | '+') {
        let after = &rest[1..];
        let gap = after.len() - after.trim_start_matches(' ').len();
        if gap == 0 {
            return None;
        }
        (format!("{first}{}", " ".repeat(gap)), &after[gap..])
    } else if first.is_ascii_digit() {
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        let after = &rest[digits.len()..];
        let delim = after.chars().next()?;
        if delim != '.' && delim != ')' {
            return None;
        }
        let after = &after[1..];
        let gap = after.len() - after.trim_start_matches(' ').len();
        if gap == 0 {
            return None;
        }
        let next = digits.parse::<u64>().unwrap_or(1) + 1;
        (format!("{next}{delim}{}", " ".repeat(gap)), &after[gap..])
    } else {
        return None;
    };

    // Casillas de tarea: la siguiente empieza sin marcar.
    let (marker, body) = if let Some(stripped) = body
        .strip_prefix("[ ] ")
        .or_else(|| body.strip_prefix("[x] "))
        .or_else(|| body.strip_prefix("[X] "))
    {
        (format!("{marker}[ ] "), stripped)
    } else {
        (marker, body)
    };

    Some((format!("{indent}{marker}"), body.trim().is_empty()))
}

// --- pegado y soltado de imágenes -------------------------------------------
//
// Las imágenes se guardan en `images/`, junto al documento, y se inserta un
// enlace relativo `![](images/…)`. Las funciones de nombre, ruta y escala son
// puras para poder probarlas en unit tests.

/// Ruta disponible para una imagen `base.png` dentro de `dir`, añadiendo
/// sufijo `-2`, `-3`… si ya existe (evita sobrescribir una imagen anterior).
fn nombre_disponible(dir: &Path, base: &str) -> PathBuf {
    let mut candidato = dir.join(format!("{base}.png"));
    let mut n = 2;
    while candidato.exists() {
        candidato = dir.join(format!("{base}-{n}.png"));
        n += 1;
    }
    candidato
}

/// Dimensiones de una imagen reescalada para que su lado mayor no supere
/// `max`, manteniendo la proporción. Si ya cabe (o alguna dimensión es cero),
/// se devuelve tal cual.
fn dimensiones_objeto(ancho: u32, alto: u32, max: u32) -> (u32, u32) {
    let lado_largo = ancho.max(alto);
    if lado_largo <= max || ancho == 0 || alto == 0 {
        return (ancho, alto);
    }
    let factor = max as f64 / lado_largo as f64;
    let nuevo_ancho = ((ancho as f64) * factor).round() as u32;
    let nuevo_alto = ((alto as f64) * factor).round() as u32;
    (nuevo_ancho.max(1), nuevo_alto.max(1))
}

/// Ruta relativa de `hacia_imagen` desde el directorio `desde_doc`. Se usa
/// para escribir el enlace: misma carpeta -> `images/x.png`; si el documento
/// vive en una subcarpeta -> `../images/x.png`.
fn ruta_relativa(desde_doc: &Path, hacia_imagen: &Path) -> String {
    let desde: Vec<_> = desde_doc.components().collect();
    let hacia: Vec<_> = hacia_imagen.components().collect();
    let mut comun = 0;
    while comun < desde.len() && comun < hacia.len() && desde[comun] == hacia[comun] {
        comun += 1;
    }
    let mut partes: Vec<String> = Vec::new();
    for _ in comun..desde.len() {
        partes.push("..".to_string());
    }
    for componente in &hacia[comun..] {
        if let std::path::Component::Normal(nombre) = componente {
            partes.push(nombre.to_string_lossy().into_owned());
        }
    }
    if partes.is_empty() {
        ".".to_string()
    } else {
        partes.join("/")
    }
}

/// Decodifica `bytes` con gdk-pixbuf, reescala si el lado mayor supera
/// `MAX_LADO_IMAGEN` y devuelve los bytes PNG resultantes.
fn bytes_a_png(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let cargador = gdk_pixbuf::PixbufLoader::new();
    cargador
        .write(bytes)
        .map_err(|e| format!("no se pudo leer la imagen: {e}"))?;
    cargador
        .close()
        .map_err(|e| format!("no se pudo cerrar la imagen: {e}"))?;
    let pixbuf = cargador
        .pixbuf()
        .ok_or_else(|| "no se pudo decodificar la imagen".to_string())?;
    let (ancho, alto) = (pixbuf.width() as u32, pixbuf.height() as u32);
    let (dest_ancho, dest_alto) = dimensiones_objeto(ancho, alto, MAX_LADO_IMAGEN);
    let pixbuf = if (dest_ancho, dest_alto) != (ancho, alto) {
        pixbuf
            .scale_simple(
                dest_ancho as i32,
                dest_alto as i32,
                gdk_pixbuf::InterpType::Bilinear,
            )
            .ok_or_else(|| "no se pudo reescalar la imagen".to_string())?
    } else {
        pixbuf
    };
    pixbuf
        .save_to_bufferv("png", &[])
        .map_err(|e| format!("no se pudo guardar la imagen: {e}"))
}

/// Tallo `imagen-AAAAMMDD-HHMMSS` con la fecha local actual (sin chrono).
fn tallo_temporal() -> String {
    let ahora = glib::DateTime::now_local().unwrap_or_else(|_| {
        glib::DateTime::now_utc().expect("el reloj del sistema no está disponible")
    });
    format!(
        "imagen-{:04}{:02}{:02}-{:02}{:02}{:02}",
        ahora.year(),
        ahora.month(),
        ahora.day_of_month(),
        ahora.hour(),
        ahora.minute(),
        ahora.second()
    )
}

/// Decide si un fichero es una imagen por su extensión (formato raster que
/// gdk-pixbuf sabe abrir).
fn es_imagen(path: &Path) -> bool {
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => matches!(
            ext.to_ascii_lowercase().as_str(),
            "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "tiff" | "tif" | "ico"
        ),
        None => false,
    }
}

/// true si el portapapeles ofrece una imagen (mime raster o textura Gdk).
fn portapapeles_tiene_imagen(formatos: &gdk4::ContentFormats) -> bool {
    formatos.contains_type(gdk4::Texture::static_type())
        || [
            "image/png",
            "image/jpeg",
            "image/gif",
            "image/webp",
            "image/bmp",
        ]
        .iter()
        .any(|mime| formatos.contain_mime_type(mime))
}

fn emit_toast(on_toast: &ToastCallback, mensaje: &str) {
    if let Some(callback) = on_toast.borrow().as_ref() {
        callback(mensaje);
    }
}

/// Inserta `![](relativa)` en el cursor (o en `objetivo` si se da), con el
/// cursor colocado sobre el texto alternativo, entre `![` y `]`. Es una sola
/// acción de usuario: se deshace de golpe con Ctrl+Z.
fn insertar_enlace_imagen(
    buffer: &gtksourceview5::Buffer,
    view: &MarkdownView,
    relativa: &str,
    objetivo: Option<gtk4::TextIter>,
) {
    if let Some(iter) = objetivo.as_ref() {
        buffer.place_cursor(iter);
    }
    let (mut inicio, mut fin) = match buffer.selection_bounds() {
        Some(limites) => limites,
        None => {
            let iter = buffer.iter_at_offset(buffer.cursor_position());
            (iter, iter)
        }
    };
    let offset = inicio.offset();
    let enlace = format!("![]({relativa})");
    buffer.begin_user_action();
    buffer.delete(&mut inicio, &mut fin);
    let mut at = buffer.iter_at_offset(offset);
    buffer.insert(&mut at, &enlace);
    buffer.end_user_action();
    let cursor = buffer.iter_at_offset(offset + 2);
    buffer.place_cursor(&cursor);
    view.grab_focus();
}

/// Guarda `bytes` (reescalados a PNG) en `images/` junto al documento y enlaza
/// el fichero en el cursor (o en `objetivo` si se da). `tallo` es el nombre
/// base sin extensión. Devuelve `true` si se insertó.
#[allow(clippy::too_many_arguments)]
fn guardar_e_insertar_imagen(
    buffer: &gtksourceview5::Buffer,
    view: &MarkdownView,
    base_dir: &Rc<RefCell<Option<PathBuf>>>,
    on_toast: &ToastCallback,
    bytes: &[u8],
    tallo: &str,
    aviso_sin_fichero: &str,
    objetivo: Option<gtk4::TextIter>,
) -> bool {
    let Some(dir) = base_dir.borrow().clone() else {
        emit_toast(on_toast, aviso_sin_fichero);
        return false;
    };
    let Ok(png) = bytes_a_png(bytes) else {
        emit_toast(on_toast, &gettext("Could not save the image"));
        return false;
    };
    let carpeta = dir.join("images");
    if std::fs::create_dir_all(&carpeta).is_err() {
        emit_toast(on_toast, &gettext("Could not save the image"));
        return false;
    }
    let destino = nombre_disponible(&carpeta, tallo);
    if std::fs::write(&destino, &png).is_err() {
        emit_toast(on_toast, &gettext("Could not save the image"));
        return false;
    }
    // La ruta recién escrita puede estar cacheada como fallo; se olvida para
    // que la siguiente decoración la cargue y la imagen aparezca al momento.
    view.invalidate_texture(&destino.to_string_lossy());
    let relativa = ruta_relativa(&dir, &destino);
    insertar_enlace_imagen(buffer, view, &relativa, objetivo);
    true
}

/// Lee el portapapeles como textura y, si trae una imagen, la guarda e
/// inserta. Se llama cuando Ctrl+V encuentra una imagen en el portapapeles.
fn pegar_desde_portapapeles(
    buffer: &gtksourceview5::Buffer,
    view: &MarkdownView,
    base_dir: &Rc<RefCell<Option<PathBuf>>>,
    on_toast: &ToastCallback,
) {
    let Some(display) = gdk4::Display::default() else {
        return;
    };
    let buffer = buffer.clone();
    let view = view.clone();
    let base_dir = base_dir.clone();
    let on_toast = on_toast.clone();
    display
        .clipboard()
        .read_texture_async(None::<&gio::Cancellable>, move |resultado| {
            let Ok(Some(textura)) = resultado else {
                return;
            };
            let bytes = textura.save_to_png_bytes();
            guardar_e_insertar_imagen(
                &buffer,
                &view,
                &base_dir,
                &on_toast,
                bytes.as_ref(),
                &tallo_temporal(),
                &gettext("Save the document before pasting images"),
                None,
            );
        });
}

/// Maneja un arrastre sobre el editor: textura o lista de ficheros. Devuelve
/// `true` si aceptó el soltado.
fn soltar_sobre_editor(
    view: &MarkdownView,
    buffer: &gtksourceview5::Buffer,
    base_dir: &Rc<RefCell<Option<PathBuf>>>,
    on_toast: &ToastCallback,
    value: &glib::Value,
    x: f64,
    y: f64,
) -> bool {
    let Some(dir) = base_dir.borrow().clone() else {
        emit_toast(
            on_toast,
            &gettext("Save the document before dropping images"),
        );
        return false;
    };
    let objetivo = view.iter_at_location(x as i32, y as i32);

    // Textura soltada (p. ej. arrastrada desde el navegador): se trata como
    // un pegado, con tallo temporal.
    if let Ok(textura) = value.get::<gdk4::Texture>() {
        let bytes = textura.save_to_png_bytes();
        return guardar_e_insertar_imagen(
            buffer,
            view,
            base_dir,
            on_toast,
            bytes.as_ref(),
            &tallo_temporal(),
            &gettext("Save the document before dropping images"),
            objetivo,
        );
    }

    // Ficheros soltados desde el gestor de archivos.
    if let Ok(ficheros) = value.get::<gdk4::FileList>() {
        let mut insertado = false;
        for fichero in ficheros.files() {
            let Some(ruta) = fichero.path() else {
                continue;
            };
            if !es_imagen(&ruta) {
                continue;
            }
            // Ya dentro de la carpeta del documento: se enlaza tal cual, sin
            // copiar ni reescalar (se respeta el fichero del usuario).
            if ruta.starts_with(&dir) {
                let relativa = ruta_relativa(&dir, &ruta);
                insertar_enlace_imagen(buffer, view, &relativa, objetivo);
                insertado = true;
                continue;
            }
            // Desde fuera: se copia a images/ con la misma regla de reescalado
            // que el pegado, conservando el nombre original.
            let Ok(bytes) = std::fs::read(&ruta) else {
                continue;
            };
            let tallo = ruta
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "imagen".to_string());
            if guardar_e_insertar_imagen(
                buffer,
                view,
                base_dir,
                on_toast,
                &bytes,
                &tallo,
                &gettext("Save the document before dropping images"),
                objetivo,
            ) {
                insertado = true;
            }
        }
        return insertado;
    }

    false
}

/// Salta a la coincidencia siguiente (o anterior) de un contexto: selecciona
/// el rango y lo lleva a la vista. `forward`/`backward` solo devuelven el
/// rango; seleccionarlo es responsabilidad nuestra (si no, la coincidencia no
/// se ve ni cuenta el contador). Función libre para poder reutilizarla desde
/// un idle tras un reemplazo.
fn search_step(
    buffer: &gtksourceview5::Buffer,
    context: &gtksourceview5::SearchContext,
    view: &MarkdownView,
    forward: bool,
) -> bool {
    if context.occurrences_count() <= 0 {
        return false;
    }
    // Con una coincidencia ya seleccionada se sigue desde su borde: si se
    // partiera del cursor, que está dentro de la coincidencia, se volvería a
    // encontrar la misma una y otra vez.
    let anchor = match buffer.selection_bounds() {
        Some((start, end)) => {
            if forward {
                end
            } else {
                start
            }
        }
        None => {
            let mut iter = buffer.iter_at_mark(&buffer.get_insert());
            if forward && !iter.is_end() {
                iter.forward_char();
            }
            iter
        }
    };
    let found = if forward {
        context.forward(&anchor)
    } else {
        context.backward(&anchor)
    };
    let Some((start, end, _wrapped)) = found else {
        return false;
    };
    buffer.select_range(&start, &end);
    let mut start = start;
    view.scroll_to_iter(&mut start, 0.15, true, 0.0, 0.5);
    true
}

impl Editor {
    pub fn new() -> Self {
        let tags = build_tags();
        let buffer = gtksourceview5::Buffer::new(Some(&tags));
        buffer.set_highlight_syntax(false);
        buffer.set_highlight_matching_brackets(false);

        let view = MarkdownView::with_buffer(&buffer);
        view.set_wrap_mode(gtk4::WrapMode::Word);
        view.set_show_line_numbers(false);
        view.set_show_right_margin(false);
        view.set_highlight_current_line(false);
        view.set_indent_width(4);
        view.set_tab_width(4);
        view.set_insert_spaces_instead_of_tabs(true);
        view.set_smart_backspace(true);
        view.set_top_margin(52);
        view.set_bottom_margin(280);
        view.set_left_margin(MIN_MARGIN);
        view.set_right_margin(MIN_MARGIN);
        view.add_css_class("scribe-editor");

        let widget = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vexpand(true)
            .hexpand(true)
            .child(&view)
            .build();

        // El CSS lo instala main a nivel de aplicación (véase APP_CSS).

        let dark = adw::StyleManager::default().is_dark();
        apply_scheme(&buffer, dark);
        apply_theme(&tags, &view, dark);
        set_column_margins(&tags, MIN_MARGIN);

        // Búsqueda: contexto propio del documento, con vuelta al final al
        // llegar al extremo. El resaltado se enciende solo mientras la barra
        // de búsqueda está abierta (lo gobierna la ventana).
        let search_settings = gtksourceview5::SearchSettings::new();
        search_settings.set_wrap_around(true);
        search_settings.set_at_word_boundaries(false);
        let search_context = gtksourceview5::SearchContext::new(&buffer, Some(&search_settings));
        search_context.set_highlight(false);

        // Corrector ortografico (issue #51): el diccionario lo resuelve
        // enchant con el locale del sistema; sin diccionario, la revision
        // no marca nada (y la preferencia sigue funcionando).
        let ortografia = Rc::new(Ortografia::con_locale_del_sistema());

        // Anti-salto de arranque: la vista arranca invisible y se revela al
        // terminar la primera decoracion que corre con la geometria real ya
        // aplicada (el primer ancho valido llega con el mapeo de la ventana);
        // si nunca llega, una red de seguridad la revela a los dos segundos.
        view.set_opacity(0.0);
        let revelada = Rc::new(Cell::new(false));
        let por_revelar = Rc::new(Cell::new(false));
        {
            let view_reveal = view.clone();
            let revelada_seguridad = revelada.clone();
            glib::timeout_add_local_once(Duration::from_millis(2000), move || {
                if !revelada_seguridad.get() {
                    revelada_seguridad.set(true);
                    view_reveal.set_opacity(1.0);
                }
            });
        }

        let on_changed = Rc::new(RefCell::new(None));
        let last_line = Rc::new(Cell::new(-1));
        let generation = Rc::new(Cell::new(0));
        let decoration = Rc::new(Cell::new(Decoration::default()));
        let column_width = Rc::new(Cell::new(720));
        let continue_lists = Rc::new(Cell::new(true));
        let typewriter = Rc::new(Cell::new(false));
        let base_dir = Rc::new(RefCell::new(None));
        let unclosed_fences = Rc::new(RefCell::new(Vec::new()));
        let on_toast = Rc::new(RefCell::new(None));
        let decoracion_ctx = DecoracionCtx {
            view: view.downgrade(),
            buffer: buffer.downgrade(),
            decoration: decoration.clone(),
            generation: generation.clone(),
            base_dir: base_dir.clone(),
            unclosed_fences: unclosed_fences.clone(),
            ortografia,
            revelada,
            por_revelar: por_revelar.clone(),
        };

        let editor = Self {
            widget,
            view,
            buffer,
            tags,
            on_changed,
            last_line,
            decoration,
            column_width,
            continue_lists,
            typewriter,
            base_dir,
            unclosed_fences,
            on_toast,
            search_settings,
            search_context,
            decoracion_ctx,
        };
        editor.connect_signals();
        editor
    }

    fn connect_signals(&self) {
        if let Some(hadj) = self.view.hadjustment() {
            let view = self.view.clone();
            let tags = self.tags.clone();
            let column_width = self.column_width.clone();
            let ctx = self.decoracion_ctx.clone();
            hadj.connect_page_size_notify(move |adj| {
                let width = adj.page_size() as i32;
                if width <= 0 {
                    return;
                }
                // Margen lateral acotado: nunca más de 1/8 de la ventana por
                // lado (issue #63). En pantallas anchas la columna crece con
                // la ventana en vez de dejar grandes huecos; en estrechas
                // manda el ancho configurado (minimo 24 px).
                let margin = ((width - column_width.get()) / 2)
                    .max(MIN_MARGIN)
                    .min((width / 8).max(MIN_MARGIN));
                apply_column_margins_deferred(&view, &tags, &ctx, margin);
                // El layout del lienzo de tabla depende del ancho: al
                // cambiarlo hay que re-decorar para recalcular sus huecos.
                schedule_decoration(&ctx, Duration::ZERO);
            });
        }

        // El StyleManager vive toda la aplicación: capturar los widgets en
        // fuerte impediría liberar este editor al cerrar su pestaña.
        // R6 (aceptado): se registra un handler por editor y no se puede
        // desconectar; como todas las capturas son débiles, los handlers de
        // editores cerrados quedan inertes (un no-op por cambio de tema),
        // no hay fuga de widgets.
        let ctx = self.decoracion_ctx.clone();
        let tags = self.tags.downgrade();
        let buffer = self.buffer.downgrade();
        let view = self.view.downgrade();
        adw::StyleManager::default().connect_dark_notify(move |sm| {
            let Some(tags) = tags.upgrade() else {
                return;
            };
            let Some(buffer) = buffer.upgrade() else {
                return;
            };
            let Some(view) = view.upgrade() else {
                return;
            };
            apply_scheme(&buffer, sm.is_dark());
            apply_theme(&tags, &view, sm.is_dark());
            schedule_decoration(&ctx, Duration::ZERO);
        });

        // Las señales del buffer viven tanto como el buffer, y la vista posee
        // el buffer: capturar la vista en débil rompe el ciclo.
        let on_changed = self.on_changed.clone();
        let last_line = self.last_line.clone();
        let ctx = self.decoracion_ctx.clone();
        self.buffer.connect_changed(move |buf| {
            let text = buf.text(&buf.start_iter(), &buf.end_iter(), true);
            if let Some(cb) = on_changed.borrow().as_ref() {
                cb(&text);
            }
            last_line.set(buf.iter_at_offset(buf.cursor_position()).line());
            schedule_decoration(&ctx, Duration::from_millis(45));
        });

        let last_line = self.last_line.clone();
        let decoration = self.decoration.clone();
        let ctx = self.decoracion_ctx.clone();
        let typewriter = self.typewriter.clone();
        let view = self.view.downgrade();
        self.buffer.connect_cursor_position_notify(move |buf| {
            if typewriter.get() {
                if let Some(view) = view.upgrade() {
                    let mark = buf.get_insert();
                    glib::idle_add_local_once(move || {
                        view.scroll_to_mark(&mark, 0.0, true, 0.0, 0.5);
                    });
                }
            }
            let line = buf.iter_at_offset(buf.cursor_position()).line();
            if last_line.get() == line {
                return;
            }
            last_line.set(line);
            // El decorado depende del cursor: las marcas de sintaxis solo se
            // muestran en su línea, el atenuado del modo foco lo sigue y el
            // lienzo de tablas revela su fila. La generación invalida las
            // pasadas que se solapen.
            schedule_decoration(&ctx, Duration::ZERO);
            if decoration.get().focus_mode {
                apply_focus_shade(buf, line);
            }
        });

        // Continuar listas al pulsar Intro. Fase de captura para adelantarnos
        // al manejador propio del GtkTextView.
        let controller = gtk4::EventControllerKey::new();
        controller.set_propagation_phase(gtk4::PropagationPhase::Capture);
        let buffer = self.buffer.clone();
        let continue_lists = self.continue_lists.clone();
        controller.connect_key_pressed(move |_, key, _, state| {
            let plain = !state.intersects(
                gdk4::ModifierType::CONTROL_MASK
                    | gdk4::ModifierType::SHIFT_MASK
                    | gdk4::ModifierType::ALT_MASK,
            );
            if !continue_lists.get()
                || !plain
                || !matches!(key, gdk4::Key::Return | gdk4::Key::KP_Enter)
            {
                return glib::Propagation::Proceed;
            }

            let cursor = buffer.iter_at_offset(buffer.cursor_position());
            if !cursor.ends_line() || buffer.selection_bounds().is_some() {
                return glib::Propagation::Proceed;
            }
            let Some(line_start) = buffer.iter_at_line(cursor.line()) else {
                return glib::Propagation::Proceed;
            };
            let line = buffer.text(&line_start, &cursor, true).to_string();
            let Some((marker, empty)) = list_continuation(&line) else {
                return glib::Propagation::Proceed;
            };

            buffer.begin_user_action();
            if empty {
                let mut from = line_start;
                let mut to = cursor;
                buffer.delete(&mut from, &mut to);
            } else {
                let mut at = buffer.iter_at_offset(buffer.cursor_position());
                buffer.insert(&mut at, &format!("\n{marker}"));
            }
            buffer.end_user_action();
            glib::Propagation::Stop
        });
        self.view.add_controller(controller);

        // Pegar imágenes (Ctrl+V): en fase de captura para adelantarnos al
        // pegado de texto del GtkTextView. Si el portapapeles trae una imagen,
        // se guarda e inserta; si no, se deja pasar el evento y el widget pega
        // el texto como siempre (no se rompe el pegado de texto).
        let paste_buffer = self.buffer.clone();
        let paste_view = self.view.clone();
        let paste_base_dir = self.base_dir.clone();
        let paste_on_toast = self.on_toast.clone();
        let paste_controller = gtk4::EventControllerKey::new();
        paste_controller.set_propagation_phase(gtk4::PropagationPhase::Capture);
        paste_controller.connect_key_pressed(move |_, key, _, state| {
            let es_pegar = key == gdk4::Key::v
                && state.intersects(gdk4::ModifierType::CONTROL_MASK)
                && !state.intersects(gdk4::ModifierType::SHIFT_MASK | gdk4::ModifierType::ALT_MASK);
            if !es_pegar {
                return glib::Propagation::Proceed;
            }
            let Some(display) = gdk4::Display::default() else {
                return glib::Propagation::Proceed;
            };
            if !portapapeles_tiene_imagen(&display.clipboard().formats()) {
                return glib::Propagation::Proceed;
            }
            pegar_desde_portapapeles(&paste_buffer, &paste_view, &paste_base_dir, &paste_on_toast);
            glib::Propagation::Stop
        });
        self.view.add_controller(paste_controller);

        // Soltar imágenes (arrastrar y soltar): ficheros (GdkFileList) o una
        // textura directa (p. ej. desde el navegador).
        let drop_buffer = self.buffer.clone();
        let drop_view = self.view.clone();
        let drop_base_dir = self.base_dir.clone();
        let drop_on_toast = self.on_toast.clone();
        let drop_target =
            gtk4::DropTarget::new(gdk4::FileList::static_type(), gdk4::DragAction::COPY);
        drop_target.set_types(&[gdk4::FileList::static_type(), gdk4::Texture::static_type()]);
        drop_target.connect_drop(move |_target, value, x, y| {
            soltar_sobre_editor(
                &drop_view,
                &drop_buffer,
                &drop_base_dir,
                &drop_on_toast,
                value,
                x,
                y,
            )
        });
        self.view.add_controller(drop_target);

        // Ctrl+clic sobre un enlace lo abre en el navegador; el clic normal
        // sigue siendo del texto (colocar el cursor, seleccionar). Va en fase
        // de captura para ver el evento antes que los controladores propios del
        // GtkTextView, y se rechaza explícitamente (`Denied`) cuando no hay nada
        // que abrir, para no quitarle el clic.
        let click = gtk4::GestureClick::new();
        click.set_button(gdk4::BUTTON_PRIMARY);
        click.set_propagation_phase(gtk4::PropagationPhase::Capture);
        let click_view = self.view.clone();
        click.connect_pressed(move |gesture, _, x, y| {
            // El clic sobre la casilla de una tarea la alterna, sin mover el
            // cursor (el gesto se queda el evento).
            if let Some(line) = click_view.checkbox_at_position(x, y) {
                gesture.set_state(gtk4::EventSequenceState::Claimed);
                click_view.toggle_checkbox(line);
                return;
            }
            // El clic sobre una tarjeta de vídeo la abre; con Ctrl, además,
            // cualquier enlace del texto.
            let ctrl = gesture
                .current_event_state()
                .contains(gdk4::ModifierType::CONTROL_MASK);
            match click_view.dest_under_pointer(x, y, ctrl) {
                Some(url) => {
                    gesture.set_state(gtk4::EventSequenceState::Claimed);
                    abrir_en_navegador(&click_view, &url);
                }
                None => {
                    gesture.set_state(gtk4::EventSequenceState::Denied);
                }
            }
        });
        self.view.add_controller(click);

        // El globo de ayuda con el destino solo sale con Ctrl pulsado, como en
        // el editor de texto de GNOME: sin Ctrl el puntero está escribiendo, no
        // navegando. El estado de Ctrl se sigue con un controlador de teclado, y
        // el globo se recalcula tanto al mover el puntero como al pulsar o
        // soltar Ctrl, para que no haya que menear el ratón para verlo.
        let ctrl = Rc::new(Cell::new(false));
        let ultimo: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        let posicion: Rc<Cell<Option<(f64, f64)>>> = Rc::new(Cell::new(None));
        let aplicar_globo = {
            let view = self.view.clone();
            let ctrl = ctrl.clone();
            let ultimo = ultimo.clone();
            move |x: f64, y: f64| {
                let destino = ctrl.get().then(|| view.link_at_position(x, y)).flatten();
                let texto =
                    destino.map(|url| gettext("Ctrl+click to open {url}").replace("{url}", &url));
                let mut ultimo = ultimo.borrow_mut();
                if *ultimo != texto {
                    let mostrar = texto.is_some();
                    view.set_tooltip_text(texto.as_deref());
                    *ultimo = texto;
                    if mostrar {
                        view.trigger_tooltip_query();
                    }
                }
            }
        };

        let teclas_ctrl = gtk4::EventControllerKey::new();
        teclas_ctrl.set_propagation_phase(gtk4::PropagationPhase::Capture);
        let al_pulsar = aplicar_globo.clone();
        let posicion_pulsar = posicion.clone();
        let ctrl_pulsado = ctrl.clone();
        teclas_ctrl.connect_key_pressed(move |_, key, _, _| {
            if matches!(key, gdk4::Key::Control_L | gdk4::Key::Control_R) {
                ctrl_pulsado.set(true);
                if let Some((x, y)) = posicion_pulsar.get() {
                    al_pulsar(x, y);
                }
            }
            glib::Propagation::Proceed
        });
        let al_soltar = aplicar_globo.clone();
        let posicion_soltar = posicion.clone();
        teclas_ctrl.connect_key_released(move |_, key, _, _| {
            if matches!(key, gdk4::Key::Control_L | gdk4::Key::Control_R) {
                ctrl.set(false);
                if let Some((x, y)) = posicion_soltar.get() {
                    al_soltar(x, y);
                }
            }
        });
        self.view.add_controller(teclas_ctrl);

        let motion = gtk4::EventControllerMotion::new();
        motion.connect_motion(move |_, x, y| {
            posicion.set(Some((x, y)));
            aplicar_globo(x, y);
        });
        self.view.add_controller(motion);
    }

    // --- contenido -------------------------------------------------------

    pub fn set_text(&self, text: &str) {
        self.buffer.set_text(text);
        self.buffer.set_modified(false);
        // Detecta las vallas abiertas ya en el arranque, para que el aviso de
        // la barra de estado aparezca al abrir el documento (decorate es
        // diferido y no puede esperarse para el primer pintado).
        *self.unclosed_fences.borrow_mut() = analyze(text).unclosed_fences;
        let start = self.buffer.start_iter();
        self.buffer.place_cursor(&start);
        self.last_line.set(0);
        self.refresh();
    }

    pub fn text(&self) -> String {
        self.buffer
            .text(&self.buffer.start_iter(), &self.buffer.end_iter(), true)
            .to_string()
    }

    // --- búsqueda ---------------------------------------------------------

    /// Contexto de búsqueda del documento. La ventana se engancha a
    /// `occurrences-count-notify` para llevar el contador «N de M».
    pub fn search_context(&self) -> &gtksourceview5::SearchContext {
        &self.search_context
    }

    /// Fija el patrón de búsqueda. Devuelve el total de coincidencias, que
    /// puede llegar más tarde por señal si el documento es grande.
    pub fn set_search_text(&self, text: &str) -> i32 {
        if self.search_settings.search_text().as_deref() == Some(text) {
            return self.search_context.occurrences_count();
        }
        self.search_settings.set_search_text(Some(text));
        self.search_context.occurrences_count()
    }

    /// Re-fija el patrón (quítalo y ponlo) para forzar que GtkSourceView
    /// recompile y recompute tras un `replace()`, que deja el contexto en un
    /// estado del que `forward` no vuelve por sí solo.
    pub fn refresh_search(&self) {
        let patron = self.search_settings.search_text();
        self.search_settings.set_search_text(None);
        self.search_settings.set_search_text(patron.as_deref());
    }

    /// Enciende o apaga el resaltado de todas las coincidencias.
    pub fn set_search_highlight(&self, on: bool) {
        self.search_context.set_highlight(on);
    }

    /// TEMPORAL (depuracion del corrector): fija el texto sin refrescar la
    /// decoracion.
    #[allow(dead_code)]
    pub fn texto_sin_decorar(&self, text: &str) {
        self.buffer.set_text(text);
    }

    /// Corrector ortográfico: enciende o apaga el subrayado de las palabras
    /// que el diccionario no valida (issue #51). El motor es parte del pase
    /// de decoración: apagarlo simplemente salta la revisión.
    pub fn set_spellcheck(&self, enabled: bool) {
        let mut config = self.decoration.get();
        config.spellcheck = enabled;
        self.decoration.set(config);
        if let Some(tag) = self.buffer.tag_table().lookup("ortografia") {
            tag.set_underline(if enabled {
                pango::Underline::Single
            } else {
                pango::Underline::None
            });
        }
        self.refresh();
    }

    /// Sensibilidad a mayúsculas/minúsculas del patrón (issue #50). Cada
    /// documento conserva su propio ajuste.
    pub fn set_search_case_sensitive(&self, on: bool) {
        self.search_settings.set_case_sensitive(on);
    }

    pub fn search_case_sensitive(&self) -> bool {
        self.search_settings.is_case_sensitive()
    }

    /// Trata el patrón como expresión regular (issue #50).
    pub fn set_search_regex(&self, on: bool) {
        self.search_settings.set_regex_enabled(on);
    }

    pub fn search_regex(&self) -> bool {
        self.search_settings.is_regex_enabled()
    }

    /// El patrón actual no compila como expresión regular. GtkSourceView no
    /// expone el error de compilación, así que se valida aparte para pintar
    /// la entrada; glib y gtksourceview usan sintaxis PCRE compatibles.
    pub fn search_pattern_has_error(&self) -> bool {
        if !self.search_settings.is_regex_enabled() {
            return false;
        }
        match self.search_settings.search_text() {
            Some(patron) => {
                !patron.is_empty()
                    && glib::Regex::new(
                        &patron,
                        glib::RegexCompileFlags::DEFAULT,
                        glib::RegexMatchFlags::DEFAULT,
                    )
                    .is_err()
            }
            None => false,
        }
    }

    /// Reemplaza la coincidencia seleccionada, si lo seleccionado es una
    /// coincidencia completa, y salta a la siguiente. Devuelve si hubo
    /// reemplazo.
    pub fn replace_current(&self, replacement: &str) -> bool {
        let Some((mut start, mut end)) = self.buffer.selection_bounds() else {
            return false;
        };
        // occurrence_position es 1-based y -1 cuando el rango no es una
        // coincidencia: así no se pisa texto arbitrario seleccionado a mano.
        if self.search_context.occurrence_position(&start, &end) < 1 {
            return false;
        }
        if self
            .search_context
            .replace(&mut start, &mut end, replacement)
            .is_err()
        {
            return false;
        }
        // El replace deja el contexto en un estado del que `forward` no
        // vuelve por sí solo (ver refresh_search): se re-fija el patrón y el
        // salto a la siguiente se difiere al idle, tras la recomputación.
        self.refresh_search();
        let buffer = self.buffer.clone();
        let context = self.search_context.clone();
        let view = self.view.clone();
        glib::idle_add_local_once(move || {
            search_step(&buffer, &context, &view, true);
        });
        true
    }

    /// Reemplaza todas las coincidencias y avisa en `hecho` con cuántas
    /// fueron. El trabajo se difiere a un idle: tras un cambio del buffer
    /// (o un `replace()`) hay que dejar que GtkSourceView recompute; el patrón
    /// se re-fija para forzar la recuperación de un estado usable, y en el
    /// idle `replace_all` de C ya no tropieza con el assert de la crate.
    pub fn replace_all_matches<F: FnOnce(usize) + 'static>(&self, replacement: &str, hecho: F) {
        self.refresh_search();
        let context = self.search_context.clone();
        let replacement = replacement.to_string();
        glib::idle_add_local_once(move || {
            let n = context.occurrences_count().max(0) as usize;
            let hechas = if n > 0 && context.replace_all(&replacement).is_ok() {
                n
            } else {
                0
            };
            hecho(hechas);
        });
    }

    /// Total de coincidencias y número de la que tiene el cursor (0 si el
    /// cursor no está sobre ninguna).
    pub fn search_state(&self) -> (i32, i32) {
        let total = self.search_context.occurrences_count().max(0);
        let position = self
            .buffer
            .selection_bounds()
            .map(|(start, end)| self.search_context.occurrence_position(&start, &end))
            .unwrap_or(0)
            .max(0);
        (position, total)
    }

    /// Siguiente coincidencia, dando la vuelta al llegar al final.
    pub fn search_next(&self) -> bool {
        self.search_step(true)
    }

    /// Coincidencia anterior, dando la vuelta al llegar al principio.
    pub fn search_previous(&self) -> bool {
        self.search_step(false)
    }

    fn search_step(&self, forward: bool) -> bool {
        search_step(&self.buffer, &self.search_context, &self.view, forward)
    }

    pub fn refresh(&self) {
        schedule_decoration(&self.decoracion_ctx, Duration::ZERO);
    }

    /// Directorio del documento, contra el que se resuelven las rutas
    /// relativas de las imágenes. `None` = documento sin fichero: no se
    /// resuelven y se pinta el placeholder. Programa una redecoración.
    pub fn set_base_dir(&self, dir: Option<PathBuf>) {
        *self.base_dir.borrow_mut() = dir;
        self.refresh();
    }

    pub fn connect_changed<F: Fn(&str) + 'static>(&self, callback: F) {
        *self.on_changed.borrow_mut() = Some(Box::new(callback));
    }

    /// Conecta el aviso transitorio (toast). Lo llama la ventana al crear el
    /// editor; se usa para avisar cuando falta un fichero al pegar imágenes.
    pub fn connect_toast<F: Fn(&str) + 'static>(&self, callback: F) {
        *self.on_toast.borrow_mut() = Some(Box::new(callback));
    }

    pub fn cursor_position(&self) -> (i32, i32) {
        let iter = self.buffer.iter_at_offset(self.buffer.cursor_position());
        (iter.line() + 1, iter.line_offset() + 1)
    }

    pub fn scroll_to_line(&self, line: i32) {
        if let Some(mut iter) = self.buffer.iter_at_line(line) {
            self.buffer.place_cursor(&iter);
            self.view.scroll_to_iter(&mut iter, 0.0, true, 0.0, 0.3);
            self.view.grab_focus();
        }
    }

    /// Coloca el cursor en línea/columna (1-based) y lo lleva a la vista.
    /// Si la posición excede el documento, se clampa al final de la línea o
    /// del buffer. La usa el arranque con `fichero.md:42:7`.
    pub fn place_cursor_at(&self, line: u32, column: u32) {
        let line_index = line.saturating_sub(1) as i32;
        let target = column.saturating_sub(1) as i32;
        let mut iter = match self.buffer.iter_at_line(line_index) {
            Some(it) => it,
            None => self.buffer.end_iter(),
        };
        let mut line_end = iter;
        if !line_end.ends_line() {
            line_end.forward_to_line_end();
        }
        // Avanza dentro de la línea sin pasarse del final de la misma.
        let disponibles = line_end.line_offset() - iter.line_offset();
        if target < disponibles {
            iter.forward_chars(target);
        } else {
            iter = line_end;
        }
        self.buffer.place_cursor(&iter);
        self.view.scroll_to_iter(&mut iter, 0.0, true, 0.0, 0.3);
        self.view.grab_focus();
    }

    /// Envuelve la selección con un marcador (`**`, `*`, `` ` ``…).
    pub fn wrap_selection(&self, marker: &str) {
        let (mut start, mut end) = match self.buffer.selection_bounds() {
            Some(bounds) => bounds,
            None => {
                let iter = self.buffer.iter_at_offset(self.buffer.cursor_position());
                (iter, iter)
            }
        };
        let selected = self.buffer.text(&start, &end, true).to_string();
        let offset = start.offset();
        self.buffer.begin_user_action();
        self.buffer.delete(&mut start, &mut end);
        let mut at = self.buffer.iter_at_offset(offset);
        self.buffer
            .insert(&mut at, &format!("{marker}{selected}{marker}"));
        self.buffer.end_user_action();
        let cursor = self
            .buffer
            .iter_at_offset(offset + marker.chars().count() as i32);
        self.buffer.place_cursor(&cursor);
        self.view.grab_focus();
    }

    // --- apariencia y comportamiento -------------------------------------

    pub fn set_font(&self, family: FontFamily, size: i32, line_spacing: f64) {
        let size = size.clamp(9, 40);
        APP_CSS.with(|slot| {
            if let Some(css) = slot.borrow().as_ref() {
                css.load_from_string(&format!(
                    "textview.scribe-editor {{ font-family: {}; font-size: {size}px; }}",
                    family.css_stack()
                ));
            }
        });
        let extra = ((size as f64) * (line_spacing - 1.0)).round().max(0.0) as i32;
        self.view.set_pixels_above_lines(extra / 2);
        self.view.set_pixels_below_lines(extra / 2);
        self.view.set_pixels_inside_wrap(extra / 3);
    }

    pub fn set_column_width(&self, width: i32) {
        self.column_width.set(width.clamp(480, 1400));
        if let Some(hadj) = self.view.hadjustment() {
            let available = hadj.page_size() as i32;
            let margin = ((available - self.column_width.get()) / 2).max(MIN_MARGIN);
            apply_column_margins_deferred(&self.view, &self.tags, &self.decoracion_ctx, margin);
        }
    }

    pub fn set_markup_visibility(&self, visibility: MarkupVisibility) {
        let mut config = self.decoration.get();
        config.markup = visibility;
        self.decoration.set(config);
        self.refresh();
    }

    pub fn set_focus_mode(&self, enabled: bool) {
        let mut config = self.decoration.get();
        config.focus_mode = enabled;
        self.decoration.set(config);
        self.refresh();
    }

    pub fn set_typewriter_mode(&self, enabled: bool) {
        self.typewriter.set(enabled);
        if enabled {
            self.view
                .scroll_to_mark(&self.buffer.get_insert(), 0.0, true, 0.0, 0.5);
        }
    }

    pub fn set_continue_lists(&self, enabled: bool) {
        self.continue_lists.set(enabled);
    }

    /// Realinea todas las tablas del documento en el fuente. Es una sola
    /// accion de usuario, asi que se deshace de golpe con Ctrl+Z.
    pub fn format_tables(&self) -> bool {
        let text = self.text();
        let Some(formatted) = crate::markdown_render::format_tables(&text) else {
            return false;
        };
        let offset = self.buffer.cursor_position();
        self.buffer.begin_user_action();
        let (mut start, mut end) = (self.buffer.start_iter(), self.buffer.end_iter());
        self.buffer.delete(&mut start, &mut end);
        let mut at = self.buffer.start_iter();
        self.buffer.insert(&mut at, &formatted);
        self.buffer.end_user_action();
        let restored = self
            .buffer
            .iter_at_offset(offset.min(self.buffer.end_iter().offset()));
        self.buffer.place_cursor(&restored);
        true
    }

    pub fn set_tab_width(&self, width: i32) {
        let width = width.clamp(2, 8) as u32;
        self.view.set_tab_width(width);
        self.view.set_indent_width(width as i32);
    }

    /// Cierra las vallas de código abiertas donde la heurística lo sugiera
    /// (véase `markdown_render::close_unclosed_fences`). Es una sola acción de
    /// usuario, así que se deshace de golpe con Ctrl+Z, como `format_tables`.
    pub fn close_unclosed_fences(&self) -> bool {
        let text = self.text();
        let Some(fixed) = crate::markdown_render::close_unclosed_fences(&text) else {
            return false;
        };
        let offset = self.buffer.cursor_position();
        self.buffer.begin_user_action();
        let (mut start, mut end) = (self.buffer.start_iter(), self.buffer.end_iter());
        self.buffer.delete(&mut start, &mut end);
        let mut at = self.buffer.start_iter();
        self.buffer.insert(&mut at, &fixed);
        self.buffer.end_user_action();
        let restored = self
            .buffer
            .iter_at_offset(offset.min(self.buffer.end_iter().offset()));
        self.buffer.place_cursor(&restored);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// El nombre de imagen evita colisiones añadiendo `-2`, `-3`… si el
    /// fichero ya existe.
    #[test]
    fn nombre_disponible_imagen_evita_colisiones() {
        let dir = std::env::temp_dir().join(format!("scribe-nombre-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let base = "imagen-20261007-203045";
        let primero = nombre_disponible(&dir, base);
        assert_eq!(
            primero.file_name().and_then(|n| n.to_str()),
            Some("imagen-20261007-203045.png")
        );
        std::fs::write(&primero, b"x").unwrap();

        let segundo = nombre_disponible(&dir, base);
        assert_eq!(
            segundo.file_name().and_then(|n| n.to_str()),
            Some("imagen-20261007-203045-2.png")
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// El reescalado a 2560 mantiene la proporción y no agranda lo pequeño.
    #[test]
    fn dimensiones_objeto_escala_a_2560() {
        assert_eq!(dimensiones_objeto(5120, 2560, 2560), (2560, 1280));
        assert_eq!(dimensiones_objeto(2000, 4000, 2560), (1280, 2560));
        assert_eq!(dimensiones_objeto(2560, 2560, 2560), (2560, 2560));
        assert_eq!(dimensiones_objeto(800, 600, 2560), (800, 600));
    }

    /// La imagen en la misma carpeta se enlaza como `images/x.png`.
    #[test]
    fn ruta_relativa_imagen_misma_carpeta() {
        let desde = Path::new("/tmp/doc");
        let hacia = Path::new("/tmp/doc/images/x.png");
        assert_eq!(ruta_relativa(desde, hacia), "images/x.png");
    }

    /// Desde una subcarpeta del documento, la imagen sube con `../`.
    #[test]
    fn ruta_relativa_imagen_desde_subcarpeta() {
        let desde = Path::new("/tmp/docs/sub");
        let hacia = Path::new("/tmp/docs/images/x.png");
        assert_eq!(ruta_relativa(desde, hacia), "../images/x.png");
    }
}
