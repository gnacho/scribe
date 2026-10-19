//! `MarkdownView`: un `GtkSourceView` que además **dibuja** lo que un
//! `GtkTextTag` no puede expresar.
//!
//! Un tag cambia cómo se ve un texto, pero no lo sustituye ni pinta encima. Por
//! eso las viñetas, las casillas de tarea, las reglas horizontales, la barra de
//! las citas y la caja de los bloques de código se ocultan en el buffer y se
//! pintan aquí, sobre el hueco que dejan.
//!
//! El gancho es `GtkTextView::snapshot_layer`, un vfunc pensado exactamente
//! para esto: se llama antes y después de que la vista pinte su propio texto, y
//! trabaja en **coordenadas de buffer**, así que el desplazamiento lo resuelve
//! GTK y aquí no hay que compensar nada.
//!
//! El widget no sabe nada de Scribe: recibe una lista de [`Ornament`] y una
//! paleta, y dibuja. Junto con [`crate::markdown_render`] forma una pieza
//! autocontenida, sin dependencias de la aplicación.
//!
//! **Aviso (mitigación GNOME/gtk#8346):** mientras
//! [`crate::settings::gtk_hides_invisible_safely`] devuelva `false`, el editor
//! no oculta texto; las marcas sustituidas por un adorno se **encogen**
//! (escala ≈ 0 y transparentes, tag `syn_shrink`) en vez de hacerse
//! invisibles, de modo que siguen participando en la maquetación y el camino
//! del aborto es inalcanzable. El hueco mínimo que dejan basta para pintar
//! aquí el adorno sustituto.
//!
//! **Rejilla de tablas:** una tabla se lee como una tabla. La geometría de sus
//! columnas sale de la fila delimitadora (la calcula
//! [`crate::markdown_render`]); aquí se dibujan los separadores verticales
//! continuos y el filete bajo la cabecera, y el editor recoloca cada celda con
//! los tags de relleno (`letter-spacing` sobre el `|` que precede a la celda).
//! La fila donde está el cursor se revela: el editor deja su fuente crudo y el
//! widget no dibuja la rejilla sobre ella, porque es la que se está editando.

use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use gtk4::{gdk, graphene, gsk};
use gtksourceview5 as sourceview;

use crate::markdown_render::{
    canvas_layout, Align, LinkRef, Ornament, CANVAS_CELL_PAD, CANVAS_ROW_PAD,
};

/// Colores de los adornos. Los fija la aplicación para que casen con su tema.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrnamentPalette {
    /// Viñetas y casillas marcadas.
    pub accent: gdk::RGBA,
    /// Reglas horizontales y borde de las casillas sin marcar.
    pub muted: gdk::RGBA,
    /// Fondo de la caja de los bloques de código.
    pub block: gdk::RGBA,
    /// Fondo de la caja de las tablas.
    pub table: gdk::RGBA,
    /// Barra vertical de las citas.
    pub quote: gdk::RGBA,
    /// Marca de verificación sobre una casilla marcada.
    pub on_accent: gdk::RGBA,
}

impl Default for OrnamentPalette {
    fn default() -> Self {
        let grey = gdk::RGBA::new(0.55, 0.55, 0.55, 1.0);
        Self {
            accent: grey,
            muted: grey,
            block: gdk::RGBA::new(0.5, 0.5, 0.5, 0.10),
            table: gdk::RGBA::new(0.5, 0.5, 0.5, 0.07),
            quote: grey,
            on_accent: gdk::RGBA::WHITE,
        }
    }
}

// --- Medidas de los adornos, en píxeles lógicos -----------------------------

/// Distancia del centro de la viñeta al borde izquierdo del texto.
const BULLET_OFFSET: f32 = 15.0;
const BULLET_RADIUS: f32 = 3.0;
/// Lado de la casilla de tarea.
const CHECKBOX_SIZE: f32 = 14.0;
/// Distancia del borde izquierdo de la casilla al borde del texto.
const CHECKBOX_OFFSET: f32 = 22.0;
/// Sangrado de la caja de código respecto al margen de la columna.
pub(crate) const BLOCK_PADDING: f32 = 8.0;
const BLOCK_RADIUS: f32 = 8.0;
const QUOTE_BAR_WIDTH: f32 = 3.0;
const QUOTE_BAR_OFFSET: f32 = 20.0;
const RULE_THICKNESS: f32 = 1.0;
/// Grosor de la rejilla vertical de una tabla, en píxeles.
const TABLE_GRID_THICKNESS: f32 = 1.0;
/// Alto del hueco que el tag `imagegap` reserva bajo la línea de una imagen.
const IMAGE_GAP_HEIGHT: f32 = crate::markdown_render::IMAGE_GAP_HEIGHT as f32;
/// Alto del hueco que el tag `youtubegap` reserva bajo la línea de una tarjeta.
const YOUTUBE_GAP_HEIGHT: f32 = crate::markdown_render::YOUTUBE_GAP_HEIGHT as f32;
/// Ancho máximo de una tarjeta de vídeo (16:9).
const YOUTUBE_MAX_WIDTH: f32 = 480.0;
/// Radio del botón de reproducción de una tarjeta.
const PLAY_RADIUS: f32 = 26.0;
/// Alto máximo de la imagen pintada en ese hueco (se escala manteniendo ratio).
const IMAGE_MAX_HEIGHT: f32 = 144.0;
/// Entradas máximas de la caché de texturas; al superarlo se vacía entera.
const MAX_TEXTURE_CACHE: usize = 64;
/// Entradas máximas de la caché de tags de relleno; al superarlo se vacía.
const MAX_PAD_TAGS: usize = 64;
/// Caracteres con los que se mide el avance de una celda de tabla: la media
/// de un tramo largo absorbe el redondeo a píxeles enteros de Pango.
const ADVANCE_SAMPLE: usize = 40;
/// Tamaño máximo de fichero de imagen que se intenta cargar (20 MB).
const MAX_IMAGE_BYTES: u64 = 20 * 1024 * 1024;

mod imp {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;
    use std::rc::Rc;

    #[derive(Default)]
    pub struct MarkdownView {
        pub ornaments: RefCell<Vec<Ornament>>,
        pub palette: Cell<OrnamentPalette>,
        /// CSS del tema (fondo/primer plano) aplicado a la vista.
        pub css_tema: std::cell::RefCell<Option<gtk4::CssProvider>>,
        /// Numero de lineas del buffer cuando se calcularon los adornos. Si el
        /// buffer ya no coincide, los adornos apuntan a lineas que se han
        /// movido y no se dibuja nada hasta que llegue la siguiente pasada.
        pub line_count: Cell<i32>,
        /// Texturas por ruta absoluta. Los fallos también se cachean (`None`)
        /// para no releer del disco en cada fotograma.
        pub textures: RefCell<HashMap<String, Option<gdk::Texture>>>,
        /// Tags de relleno de tabla (`letter-spacing`) ya creados, por valor.
        /// Los rellenos se repiten mucho entre tablas y filas: sin caché, cada
        /// pasada crearía un tag por celda.
        pub pad_tags: RefCell<HashMap<i32, glib::GString>>,
        /// Cache de tags de hueco (`pixels_below_lines`) para las filas
        /// colapsadas del lienzo de tabla: un tag por alto distinto.
        pub gap_tags: RefCell<HashMap<i32, glib::GString>>,
        /// Enlaces del documento, con su destino ya resuelto. Los usa el editor
        /// para abrir con Ctrl+clic y para el globo de ayuda del puntero.
        pub links: RefCell<Vec<LinkRef>>,
        /// URLs remotas cuya descarga está en marcha, para no pedir dos veces la
        /// misma imagen (varias líneas pueden usar la misma URL).
        pub remote_pending: RefCell<std::collections::HashSet<String>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MarkdownView {
        const NAME: &'static str = "ScribeMarkdownView";
        type Type = super::MarkdownView;
        type ParentType = sourceview::View;
    }

    impl ObjectImpl for MarkdownView {}
    impl WidgetImpl for MarkdownView {}
    impl sourceview::subclass::view::ViewImpl for MarkdownView {}

    impl TextViewImpl for MarkdownView {
        fn snapshot_layer(&self, layer: gtk4::TextViewLayer, snapshot: gtk4::Snapshot) {
            match layer {
                // Las cajas y barras van debajo del texto; los glifos, encima,
                // para que nunca queden tapados por un fondo de párrafo. Las
                // imágenes ocupan su propio hueco (tag `imagegap`), así que
                // bajo el texto no tapan nada.
                gtk4::TextViewLayer::BelowText => {
                    self.draw_blocks(&snapshot);
                    self.draw_images(&snapshot);
                    self.draw_youtube(&snapshot);
                    self.draw_table_canvas(&snapshot);
                }
                gtk4::TextViewLayer::AboveText => self.draw_glyphs(&snapshot),
                _ => {}
            }
            self.parent_snapshot_layer(layer, snapshot);
        }
    }

    impl MarkdownView {
        /// Rango de líneas lógicas visibles, para no calcular geometría de
        /// adornos que están fuera de la pantalla.
        fn visible_lines(&self) -> (i32, i32) {
            let view = self.obj();
            let rect = view.visible_rect();
            let (top, _) = view.line_at_y(rect.y());
            let (bottom, _) = view.line_at_y(rect.y() + rect.height());
            (top.line(), bottom.line())
        }

        /// `(y, alto)` de una línea lógica completa, incluidas sus continuaciones.
        fn line_extent(&self, line: i32) -> Option<(f32, f32)> {
            let view = self.obj();
            let iter = view.buffer().iter_at_line(line)?;
            let (y, height) = view.line_yrange(&iter);
            Some((y as f32, height as f32))
        }

        /// `(x, y, alto)` de la primera línea de pantalla: `x` es el margen
        /// izquierdo del párrafo, que depende del tag y no del widget.
        /// Línea lógica del cursor. En esa línea no se pintan los adornos
        /// que sustituyen al fuente (issue #41): lo que se edita es el
        /// texto crudo y las marcas ya se muestran solas.
        pub(super) fn cursor_line(&self) -> i32 {
            let view = self.obj();
            let buffer = view.buffer();
            buffer.iter_at_mark(&buffer.get_insert()).line()
        }

        pub(super) fn line_origin(&self, line: i32) -> Option<(f32, f32, f32)> {
            let view = self.obj();
            let iter = view.buffer().iter_at_line(line)?;
            let rect = view.iter_location(&iter);
            Some((rect.x() as f32, rect.y() as f32, rect.height() as f32))
        }

        /// Extremos horizontales de la columna de texto.
        fn column(&self) -> (f32, f32) {
            let view = self.obj();
            (
                view.left_margin() as f32,
                (view.width() - view.right_margin()) as f32,
            )
        }

        /// Avance horizontal de un carácter de tabla, en píxeles. La rejilla se
        /// calcula en celdas de carácter y GTK no expone el avance que da el
        /// tag `table`, así que se mide con el contexto de la propia vista
        /// (familia monoespaciada al tamaño por defecto del widget). Se mide un
        /// tramo largo para que el redondeo a píxeles de Pango no pese.
        pub(super) fn cell_advance(&self) -> f32 {
            let view = self.obj();
            let layout = view.create_pango_layout(None);
            let sample = "0".repeat(ADVANCE_SAMPLE);
            layout.set_markup(&format!("<span font_family=\"monospace\">{sample}</span>"));
            let (width, _) = layout.pixel_size();
            width as f32 / ADVANCE_SAMPLE as f32
        }

        /// Altura de línea de la monoespaciada, en píxeles. El lienzo de tabla
        /// la usa para el alto de sus filas pintadas, igual que el editor para
        /// los huecos de las filas colapsadas: misma medida, mismo alto.
        pub(super) fn cell_height(&self) -> f32 {
            let view = self.obj();
            let layout = view.create_pango_layout(None);
            layout.set_markup("<span font_family=\"monospace\">0</span>");
            let (_, height) = layout.pixel_size();
            height as f32
        }

        /// Nombre del tag de hueco (`pixels_below_lines`) para un alto dado,
        /// en píxeles. Un tag por valor distinto, cacheado: el editor lo aplica
        /// sobre cada fila fuente colapsada de un lienzo de tabla.
        pub(super) fn gap_tag(&self, px: i32) -> glib::GString {
            let mut cache = self.gap_tags.borrow_mut();
            if let Some(name) = cache.get(&px) {
                return name.clone();
            }
            if cache.len() >= MAX_PAD_TAGS {
                cache.clear();
            }
            let name = glib::GString::from(format!("tcanvasgap{px}"));
            let tag = gtk4::TextTag::builder()
                .name(name.as_str())
                .pixels_below_lines(px)
                .build();
            self.obj().buffer().tag_table().add(&tag);
            cache.insert(px, name.clone());
            name
        }

        /// Nombre del tag de relleno (`letter-spacing`) para un avance dado, en
        /// unidades Pango (1024 = 1 px). Un tag por valor distinto, cacheado:
        /// el editor lo aplica sobre el `|` que precede a cada celda.
        pub(super) fn pad_tag(&self, units: i32) -> glib::GString {
            let mut cache = self.pad_tags.borrow_mut();
            if let Some(name) = cache.get(&units) {
                return name.clone();
            }
            if cache.len() >= MAX_PAD_TAGS {
                cache.clear();
            }
            let name = glib::GString::from(format!("tablepad{units}"));
            let tag = gtk4::TextTag::builder()
                .name(name.as_str())
                .letter_spacing(units)
                .build();
            self.obj().buffer().tag_table().add(&tag);
            cache.insert(units, name.clone());
            name
        }

        /// `(x de la primera frontera, ancho, avance de celda)` de la rejilla de
        /// una tabla, en píxeles. La X sale de la fila delimitadora (mismo
        /// margen de párrafo que el resto de la tabla) y las fronteras se
        /// cuentan en celdas de carácter desde su inicio.
        fn grid_extent(&self, anchor: i32, bounds: &[usize]) -> Option<(f32, f32, f32)> {
            let (x, _, _) = self.line_origin(anchor)?;
            let (first, last) = (bounds.first()?, bounds.last()?);
            let advance = self.cell_advance();
            let x0 = x + *first as f32 * advance;
            Some((x0, (*last - *first) as f32 * advance, advance))
        }

        /// Rejilla de tabla que cabe entera en la columna de texto. Una tabla
        /// más ancha que la columna envuelve sus filas, y entonces las líneas
        /// de la rejilla cruzarían el texto envuelto: se comporta como si no
        /// hubiera rejilla (la caja y el filete abrazan el contenido).
        fn grid_fitting(
            &self,
            anchor: i32,
            bounds: &[usize],
            right: f32,
        ) -> Option<(f32, f32, f32)> {
            let (x0, width, advance) = self.grid_extent(anchor, bounds)?;
            (x0 + width <= right).then_some((x0, width, advance))
        }

        /// Segmentos verticales de una rejilla: el rango visible del bloque,
        /// partido por la línea del cursor (donde el fuente se ve crudo y la
        /// rejilla estorbaría).
        fn grid_segments(&self, first: i32, last: i32, top: f32, bottom: f32) -> Vec<(f32, f32)> {
            let cursor_line = {
                let view = self.obj();
                let buffer = view.buffer();
                buffer.iter_at_offset(buffer.cursor_position()).line()
            };
            let segments = if (first..=last).contains(&cursor_line) {
                match self.line_extent(cursor_line) {
                    Some((y, height)) => {
                        vec![(top, y.max(top)), ((y + height).min(bottom), bottom)]
                    }
                    None => vec![(top, bottom)],
                }
            } else {
                vec![(top, bottom)]
            };
            segments
                .into_iter()
                .filter(|(y0, y1)| y1 - y0 > 0.5)
                .collect()
        }

        /// `(x_inicio, x_fin)` del contenido real de una tabla: la X del
        /// primer carácter de la fila menos sangrada y la X tras el último
        /// carácter de la fila más ancha. Como los pipes están encogidos,
        /// mide solo el texto de las celdas. Las tablas enormes se miden
        /// solo en la ventana anclada a lo visible, por la misma razón que
        /// el rect vertical: no pedir geometría de miles de líneas fuera de
        /// pantalla en mitad del dibujado.
        ///
        /// Una fila que envuelve en varias líneas de visualización mide hasta
        /// `right`: el final lógico cae en la última línea (la más corta) y
        /// el texto envuelto llega hasta el borde de la columna.
        fn table_extent(
            &self,
            first: i32,
            last: i32,
            anchor_top: i32,
            anchor_bottom: i32,
            right: f32,
        ) -> Option<(f32, f32)> {
            let (from, to) = if last - first <= 200 {
                (first, last)
            } else {
                (anchor_top, anchor_bottom)
            };
            let view = self.obj();
            let buffer = view.buffer();
            let mut start_x = f32::MAX;
            let mut end_x = 0.0f32;
            let mut any = false;
            for line in from..=to {
                let Some(mut iter) = buffer.iter_at_line(line) else {
                    continue;
                };
                let start = view.iter_location(&iter);
                start_x = start_x.min(start.x() as f32);
                if !iter.ends_line() {
                    iter.forward_to_line_end();
                    let rect = view.iter_location(&iter);
                    let end = if (rect.y() as f32) > start.y() as f32 {
                        right
                    } else {
                        (rect.x() + rect.width()) as f32
                    };
                    end_x = end_x.max(end);
                }
                any = true;
            }
            any.then_some((start_x, end_x))
        }

        /// Caja horizontal de una tabla: su contenido más el margen de la
        /// caja, sin salirse nunca de la columna de texto. Si la tabla tiene
        /// rejilla, la caja abraza sus columnas (geometría de la fila
        /// delimitadora), no el relleno del fuente.
        fn table_box(
            &self,
            first: i32,
            last: i32,
            anchor_top: i32,
            anchor_bottom: i32,
        ) -> (f32, f32) {
            let (left, right) = self.column();
            let grid = self.ornaments.borrow().iter().find_map(|o| match o {
                Ornament::TableGrid {
                    first: f,
                    last: l,
                    anchor,
                    bounds,
                    ..
                } if *f as i32 == first && *l as i32 == last => {
                    self.grid_fitting(*anchor as i32, bounds, right)
                }
                _ => None,
            });
            if let Some((x0, width, _)) = grid {
                return (
                    (x0 - BLOCK_PADDING).max(left),
                    (x0 + width + BLOCK_PADDING).min(right),
                );
            }
            match self.table_extent(first, last, anchor_top, anchor_bottom, right) {
                Some((s, e)) => (
                    (s - BLOCK_PADDING).max(left),
                    (e + BLOCK_PADDING).min(right),
                ),
                None => (left + BLOCK_PADDING, right - BLOCK_PADDING),
            }
        }

        /// Los adornos van por numero de linea. Si el buffer ha cambiado desde
        /// que se calcularon, dibujarlos pondria cajas y vinetas en sitios
        /// equivocados; se salta el fotograma y ya llegara la decoracion nueva.
        pub(super) fn stale(&self) -> bool {
            self.line_count.get() != self.obj().buffer().end_iter().line() + 1
        }

        fn draw_blocks(&self, snapshot: &gtk4::Snapshot) {
            let ornaments = self.ornaments.borrow();
            if ornaments.is_empty() || self.stale() {
                return;
            }
            let palette = self.palette.get();
            let (first_visible, last_visible) = self.visible_lines();
            let (left, right) = self.column();

            // Las tablas con lienzo pintan su propio sombreado a ancho
            // completo: la caja de `Ornament::Table` se mediría sobre las
            // filas colapsadas y quedaría más estrecha que el lienzo.
            let con_lienzo = |first: i32| {
                ornaments.iter().any(
                    |o| matches!(o, Ornament::TableCanvas { first: f, .. } if *f as i32 == first),
                )
            };
            for ornament in ornaments.iter() {
                let (first, last) = match ornament {
                    Ornament::CodeBlock { first, last } | Ornament::Quote { first, last } => {
                        (*first as i32, *last as i32)
                    }
                    Ornament::Table { first, last } => {
                        if con_lienzo(*first as i32) {
                            continue;
                        }
                        (*first as i32, *last as i32)
                    }
                    _ => continue,
                };
                if last < first_visible || first > last_visible {
                    continue;
                }

                // Solo se pide geometria de lineas cercanas a lo visible. Un
                // bloque que abarca medio documento obligaria a GTK a validar
                // la maquetacion de miles de lineas en mitad del dibujado, y
                // eso deja la vista en un estado incoherente: gtksourceview
                // aborta despues con «byte index off the end of the line».
                //
                // El rect se LIMITA al rango visible (mas una linea de margen):
                // un fill uniforme no necesita cubrir el bloque entero, y los
                // rects enormes o con origen negativo rompen el clip redondeado
                // de GSK (la caja desaparecia en medio de bloques largos).
                let anchor_top = first.max(first_visible - 1).max(0);
                let anchor_bottom = last.min(last_visible + 1);
                let Some((anchor_y, _)) = self.line_extent(anchor_top) else {
                    continue;
                };
                let Some((bottom_y, bottom_h)) = self.line_extent(anchor_bottom) else {
                    continue;
                };
                let top = anchor_y;
                let bottom = bottom_y + bottom_h;
                // Esquinas redondeadas solo en los bordes reales del bloque; si
                // el bloque sigue fuera de pantalla, el borde visible va recto.
                let corners = |radius: f32| {
                    let corner = |visible_edge: bool| {
                        let r = if visible_edge { radius } else { 0.0 };
                        graphene::Size::new(r, r)
                    };
                    [
                        corner(first == anchor_top),   // sup-izq
                        corner(first == anchor_top),   // sup-dcha
                        corner(last == anchor_bottom), // inf-dcha
                        corner(last == anchor_bottom), // inf-izq
                    ]
                };

                match ornament {
                    Ornament::CodeBlock { .. } | Ornament::Table { .. } => {
                        let is_table = matches!(ornament, Ornament::Table { .. });
                        let fill = if is_table {
                            palette.table
                        } else {
                            palette.block
                        };
                        // La caja de una tabla abraza el ancho real de sus
                        // columnas (estilo GitHub): a ancho completo parecía
                        // que sobrara una columna vacía a la derecha. La de
                        // los bloques de código sigue a ancho completo.
                        let (box_left, box_right) = if is_table {
                            self.table_box(first, last, anchor_top, anchor_bottom)
                        } else {
                            (left + BLOCK_PADDING, right - BLOCK_PADDING)
                        };
                        let rect = graphene::Rect::new(
                            box_left,
                            top,
                            (box_right - box_left).max(0.0),
                            (bottom - top).max(0.0),
                        );
                        let [tl, tr, br, bl] = corners(BLOCK_RADIUS);
                        let rounded = gsk::RoundedRect::new(rect, tl, tr, br, bl);
                        snapshot.push_rounded_clip(&rounded);
                        snapshot.append_color(&fill, &rect);
                        snapshot.pop();
                    }
                    Ornament::Quote { .. } => {
                        // La barra se ancla al margen del párrafo citado, no al
                        // de la ventana, para que respete la sangría del tag.
                        let x = self
                            .line_origin(anchor_top)
                            .map(|(x, _, _)| x - QUOTE_BAR_OFFSET)
                            .unwrap_or(left + BLOCK_PADDING);
                        let rect =
                            graphene::Rect::new(x, top, QUOTE_BAR_WIDTH, (bottom - top).max(0.0));
                        let [tl, tr, br, bl] = corners(QUOTE_BAR_WIDTH / 2.0);
                        let rounded = gsk::RoundedRect::new(rect, tl, tr, br, bl);
                        snapshot.push_rounded_clip(&rounded);
                        snapshot.append_color(&palette.quote, &rect);
                        snapshot.pop();
                    }
                    _ => {}
                }
            }

            // Rejilla de tabla: una línea vertical continua en cada frontera
            // que marca la fila delimitadora. Los pipes del fuente van
            // encogidos y caen a distinta X según el relleno de cada fila, así
            // que no sirven de referencia: la geometría es la de la fila de
            // guiones, en celdas de carácter. Va debajo del texto (esta capa).
            for ornament in ornaments.iter() {
                let Ornament::TableGrid {
                    first,
                    last,
                    anchor,
                    bounds,
                    ..
                } = ornament
                else {
                    continue;
                };
                let (first, last, anchor) = (*first as i32, *last as i32, *anchor as i32);
                if last < first_visible - 1 || first > last_visible + 1 {
                    continue;
                }
                let Some((x0, _, advance)) = self.grid_fitting(anchor, bounds, right) else {
                    continue;
                };
                // El rect se limita al rango visible (mismo motivo que las
                // cajas): un bloque largo no debe obligar a maquetar miles de
                // líneas en mitad del dibujado.
                let anchor_top = first.max(first_visible - 1).max(0);
                let anchor_bottom = last.min(last_visible + 1);
                let Some((top, _)) = self.line_extent(anchor_top) else {
                    continue;
                };
                let Some((bottom_y, bottom_h)) = self.line_extent(anchor_bottom) else {
                    continue;
                };
                let segments = self.grid_segments(first, last, top, bottom_y + bottom_h);
                // Solo las fronteras interiores: los bordes exteriores los
                // dibuja la caja del bloque.
                let interior = &bounds[1..bounds.len().saturating_sub(1)];
                for &bound in interior {
                    let x = x0 + (bound - bounds[0]) as f32 * advance;
                    if x < left || x > right {
                        continue;
                    }
                    for (y0, y1) in &segments {
                        let bar = graphene::Rect::new(
                            x - TABLE_GRID_THICKNESS / 2.0,
                            *y0,
                            TABLE_GRID_THICKNESS,
                            (y1 - y0).max(0.0),
                        );
                        snapshot.append_color(&palette.muted, &bar);
                    }
                }
            }
        }

        fn draw_glyphs(&self, snapshot: &gtk4::Snapshot) {
            let ornaments = self.ornaments.borrow();
            if ornaments.is_empty() || self.stale() {
                return;
            }
            let palette = self.palette.get();
            let (first_visible, last_visible) = self.visible_lines();
            let (left, right) = self.column();
            let cursor_line = self.cursor_line();

            for ornament in ornaments.iter() {
                let line = match ornament {
                    Ornament::Bullet { line, .. }
                    | Ornament::Checkbox { line, .. }
                    | Ornament::TableRule { line }
                    | Ornament::HeadingRule { line }
                    | Ornament::Rule { line } => *line as i32,
                    _ => continue,
                };
                if line < first_visible || line > last_visible || line == cursor_line {
                    continue;
                }
                let Some((x, y, height)) = self.line_origin(line) else {
                    continue;
                };
                let middle = y + height / 2.0;

                match ornament {
                    Ornament::Bullet { depth, .. } => {
                        draw_bullet(snapshot, x - BULLET_OFFSET, middle, *depth, &palette)
                    }
                    Ornament::Checkbox { checked, .. } => draw_checkbox(
                        snapshot,
                        x - CHECKBOX_OFFSET,
                        middle - CHECKBOX_SIZE / 2.0,
                        *checked,
                        &palette,
                    ),
                    Ornament::Rule { .. } => {
                        let rect = graphene::Rect::new(
                            left,
                            (middle - RULE_THICKNESS / 2.0).round(),
                            (right - left).max(0.0),
                            RULE_THICKNESS,
                        );
                        snapshot.append_color(&palette.muted, &rect);
                    }
                    // Filete fino bajo un título H1/H2 (estilo GitHub), con
                    // el ancho del propio texto del título.
                    Ornament::HeadingRule { .. } => {
                        let view = self.obj();
                        let Some(mut end_iter) = view.buffer().iter_at_line(line) else {
                            continue;
                        };
                        if !end_iter.ends_line() {
                            end_iter.forward_to_line_end();
                        }
                        let end_rect = view.iter_location(&end_iter);
                        let y = (end_rect.y() + end_rect.height()) as f32 - RULE_THICKNESS;
                        let rect = graphene::Rect::new(
                            x,
                            y.round(),
                            (end_rect.x() as f32 - x).max(0.0),
                            RULE_THICKNESS,
                        );
                        snapshot.append_color(&palette.muted, &rect);
                    }
                    // La cabecera de la tabla: ocupa el hueco que deja la fila
                    // de guiones y abarca justo las columnas de la rejilla, no
                    // toda la columna de texto (a ancho completo parecía que
                    // hubiera una columna vacía extra a la derecha).
                    Ornament::TableRule { .. } => {
                        let grid = ornaments.iter().find_map(|o| match o {
                            Ornament::TableGrid {
                                first,
                                last,
                                anchor,
                                bounds,
                                ..
                            } if (*first as i32) <= line && line <= (*last as i32) => {
                                self.grid_fitting(*anchor as i32, bounds, right)
                            }
                            _ => None,
                        });
                        let (rx, rw) = match grid {
                            Some((x0, width, _)) => (x0, width),
                            None => {
                                let box_x = ornaments.iter().find_map(|o| match o {
                                    Ornament::Table { first, last }
                                        if (*first as i32) <= line && line <= (*last as i32) =>
                                    {
                                        let (f, l) = (*first as i32, *last as i32);
                                        let at = f.max(first_visible - 1).max(0);
                                        let ab = l.min(last_visible + 1);
                                        Some(self.table_box(f, l, at, ab))
                                    }
                                    _ => None,
                                });
                                match box_x {
                                    Some((bl, br)) => (bl, br - bl),
                                    None => (
                                        left + BLOCK_PADDING * 2.0,
                                        right - left - BLOCK_PADDING * 4.0,
                                    ),
                                }
                            }
                        };
                        let rect = graphene::Rect::new(
                            rx,
                            (middle - RULE_THICKNESS / 2.0).round(),
                            rw.max(0.0),
                            RULE_THICKNESS,
                        );
                        snapshot.append_color(&palette.muted, &rect);
                    }
                    _ => {}
                }
            }

            // Glifos posicionados por offset de carácter (no por línea). Hoy solo
            // `Break`, el sustituto del `<br>` dentro de una tabla.
            let view = self.obj();
            let buffer = view.buffer();
            for ornament in ornaments.iter() {
                let offset = match ornament {
                    Ornament::Break { offset } => *offset as i32,
                    _ => continue,
                };
                let iter = buffer.iter_at_offset(offset);
                let line = iter.line();
                if line < first_visible || line > last_visible {
                    continue;
                }
                let rect = view.iter_location(&iter);
                draw_break(
                    snapshot,
                    rect.x() as f32,
                    rect.y() as f32,
                    rect.height() as f32,
                    &palette,
                );
            }
        }

        /// Textura de una ruta, cacheada. Las rutas locales se leen del disco;
        /// las URLs, de la caché de descargas (`queue_remote`) y, si aún no
        /// están, se piden en segundo plano y se pinta el placeholder.
        fn texture_for(&self, dest: &str) -> Option<gdk::Texture> {
            if let Some(cached) = self.textures.borrow().get(dest) {
                return cached.clone();
            }
            let loaded = load_texture(dest);
            if loaded.is_none() && is_remote(dest) {
                self.obj().queue_remote(dest);
            }
            self.cache_texture(dest, loaded.clone());
            loaded
        }

        /// Guarda una textura en la caché de memoria, con el mismo desalojo
        /// (`None` para los fallos) que el resto.
        pub(super) fn cache_texture(&self, dest: &str, texture: Option<gdk::Texture>) {
            let mut cache = self.textures.borrow_mut();
            if cache.len() >= MAX_TEXTURE_CACHE {
                cache.clear();
            }
            cache.insert(dest.to_string(), texture);
        }

        /// Offset del buffer que hay bajo un punto del widget.
        pub(super) fn offset_at(&self, x: f64, y: f64) -> Option<usize> {
            let view = self.obj();
            let (bx, by) =
                view.window_to_buffer_coords(gtk4::TextWindowType::Widget, x as i32, y as i32);
            let (iter, _) = view.iter_at_position(bx, by)?;
            Some(iter.offset() as usize)
        }

        /// URL del vídeo cuya tarjeta contiene un punto, en coordenadas de
        /// buffer. La geometría es la misma que usa `draw_youtube`.
        pub(super) fn youtube_card_at(&self, bx: f64, by: f64) -> Option<String> {
            let ornaments = self.ornaments.borrow();
            let (left, right) = self.column();
            let max_w = (right - left - BLOCK_PADDING * 2.0).max(0.0);
            if max_w <= 0.0 {
                return None;
            }
            let punto = graphene::Point::new(bx as f32, by as f32);
            for ornament in ornaments.iter() {
                let Ornament::Youtube { line, url, .. } = ornament else {
                    continue;
                };
                let Some(top) =
                    self.gap_top(*line as i32, i32::MIN / 2, i32::MAX / 2, YOUTUBE_GAP_HEIGHT)
                else {
                    continue;
                };
                let (x, y, w, h) = card_rect(left, max_w, top);
                if graphene::Rect::new(x, y, w, h).contains_point(&punto) {
                    return Some(url.clone());
                }
            }
            None
        }

        /// Imágenes en bloque: se pintan en el hueco que el tag `imagegap`
        /// reserva bajo su línea, centradas y escaladas manteniendo ratio.
        /// Si el fichero no existe o no carga, un placeholder gris.
        fn draw_images(&self, snapshot: &gtk4::Snapshot) {
            let ornaments = self.ornaments.borrow();
            if ornaments.is_empty() || self.stale() {
                return;
            }
            let palette = self.palette.get();
            let (first_visible, last_visible) = self.visible_lines();
            let (left, right) = self.column();
            let max_w = (right - left - BLOCK_PADDING * 2.0).max(0.0);
            if max_w <= 0.0 {
                return;
            }
            let view = self.obj();

            for ornament in ornaments.iter() {
                let Ornament::Image { line, dest, alt } = ornament else {
                    continue;
                };
                let line = *line as i32;
                if line == self.cursor_line() {
                    continue;
                }
                let Some(top) = self.gap_top(line, first_visible, last_visible, IMAGE_GAP_HEIGHT)
                else {
                    continue;
                };

                match self.texture_for(dest) {
                    Some(texture) => {
                        let tw = texture.width() as f32;
                        let th = texture.height() as f32;
                        if tw <= 0.0 || th <= 0.0 {
                            continue;
                        }
                        // Escala para caber en (ancho visible) × 144px; las
                        // imágenes pequeñas no se agrandan.
                        let scale = (max_w / tw).min(IMAGE_MAX_HEIGHT / th).min(1.0);
                        let (w, h) = (tw * scale, th * scale);
                        let x = left + BLOCK_PADDING + ((max_w - w) / 2.0).max(0.0);
                        let y = top + ((IMAGE_GAP_HEIGHT - h) / 2.0).max(0.0);
                        snapshot.save();
                        snapshot.translate(&graphene::Point::new(x, y));
                        snapshot.scale(scale, scale);
                        snapshot.append_texture(&texture, &graphene::Rect::new(0.0, 0.0, tw, th));
                        snapshot.restore();
                        // Borde fino alrededor, ya en coordenadas finales.
                        let border = graphene::Rect::new(x, y, w, h);
                        let rounded = gsk::RoundedRect::from_rect(border, 2.0);
                        snapshot.append_border(
                            &rounded,
                            &[1.0, 1.0, 1.0, 1.0],
                            &[palette.muted; 4],
                        );
                    }
                    None => {
                        // Placeholder: texto pequeño y gris, sin borde.
                        let label = if alt.is_empty() {
                            dest.as_str()
                        } else {
                            alt.as_str()
                        };
                        let layout =
                            view.create_pango_layout(Some(&format!("[ imagen: {label} ]")));
                        if let Some(desc) = view.pango_context().font_description() {
                            let size = desc.size();
                            if size > 0 {
                                let mut small = desc;
                                small.set_size(size * 4 / 5);
                                layout.set_font_description(Some(&small));
                            }
                        }
                        snapshot.save();
                        snapshot.translate(&graphene::Point::new(left + BLOCK_PADDING, top + 6.0));
                        snapshot.append_layout(&layout, &palette.muted);
                        snapshot.restore();
                    }
                }
            }
        }

        /// Lienzos de tabla (tablas de prosa que no caben en la rejilla del
        /// fuente): cada fila fuente colapsa (tag `tcanvas_src` + hueco) y se
        /// pinta bajo su línea con el texto de cada celda envuelto en su
        /// columna, cabecera en negrita y filete, y alineaciones respetadas.
        /// La fila del cursor no se pinta: el editor revela ahí el fuente.
        fn draw_table_canvas(&self, snapshot: &gtk4::Snapshot) {
            let ornaments = self.ornaments.borrow();
            if ornaments.is_empty() || self.stale() {
                return;
            }
            let palette = self.palette.get();
            let (first_visible, last_visible) = self.visible_lines();
            let (left, right) = self.column();
            let max_w = (right - left - BLOCK_PADDING * 2.0).max(0.0);
            let advance = self.cell_advance();
            if advance <= 0.0 || max_w <= 0.0 {
                return;
            }
            let column_cells = max_w / advance;
            let line_height = self.cell_height();
            let view = self.obj();
            let cursor_line = view
                .buffer()
                .iter_at_mark(&view.buffer().get_insert())
                .line();

            for ornament in ornaments.iter() {
                let Ornament::TableCanvas { columns, rows, .. } = ornament else {
                    continue;
                };
                let Some(layout) = canvas_layout(columns, rows, column_cells) else {
                    continue;
                };
                eprintln!(
                    "CANVAS row_lines={:?} primeras={:?}",
                    layout.row_lines,
                    layout.cells.first().map(|c| c.first().map(|l| l.len()))
                );
                // X de cada frontera de columna, en píxeles.
                let mut xs = Vec::with_capacity(layout.col_widths.len() + 1);
                let mut x = left + BLOCK_PADDING;
                for w in &layout.col_widths {
                    xs.push(x);
                    x += w * advance;
                }
                xs.push(x);
                for (ri, row) in rows.iter().enumerate() {
                    if row.line as i32 == cursor_line {
                        continue;
                    }
                    // La fila pinta su caja de línea completa (el editor
                    // reservó el hueco dentro de ella): sombreado continuo sin
                    // franjas blancas entre filas.
                    if row.line as i32 + 1 < first_visible || row.line as i32 > last_visible + 1 {
                        continue;
                    }
                    let Some((top, alto)) = self.line_extent(row.line as i32) else {
                        continue;
                    };
                    let stroke = gsk::Stroke::new(1.0);
                    // Sombreado de la fila a todo el ancho del lienzo, con
                    // velo gris en las filas alternas (zebra).
                    let fondo = gsk::PathBuilder::new();
                    fondo.add_rect(&graphene::Rect::new(
                        xs[0],
                        top,
                        xs.last().unwrap() - xs[0],
                        alto,
                    ));
                    snapshot.append_fill(&fondo.to_path(), gsk::FillRule::Winding, &palette.table);
                    if ri % 2 == 1 {
                        let velo = gsk::PathBuilder::new();
                        velo.add_rect(&graphene::Rect::new(
                            xs[0],
                            top,
                            xs.last().unwrap() - xs[0],
                            alto,
                        ));
                        snapshot.append_fill(
                            &velo.to_path(),
                            gsk::FillRule::Winding,
                            &gdk::RGBA::new(0.5, 0.5, 0.5, 0.07),
                        );
                    }
                    if ri == 0 {
                        // Borde superior: sobre la cabecera.
                        let borde = gsk::PathBuilder::new();
                        borde.move_to(xs[0], top);
                        borde.line_to(*xs.last().unwrap(), top);
                        snapshot.append_stroke(&borde.to_path(), &stroke, &palette.muted);
                    }
                    let base = top + CANVAS_ROW_PAD;
                    // Celdas: línea a línea, con la alineación de su columna.
                    // La cabecera se centra (como en las tablas HTML); el resto
                    // usa la alineación de su columna. Los tramos en negrita
                    // (****) del fuente pintan con peso bold.
                    for (ci, cell_lines) in layout.cells[ri].iter().enumerate() {
                        let col_w = layout.col_widths[ci] * advance;
                        let disponible = col_w - 2.0 * CANVAS_CELL_PAD;
                        let centrar = row.header;
                        for (li, lineas) in cell_lines.iter().enumerate() {
                            let mut ancho_linea = 0.0f32;
                            let mut layouts: Vec<(gtk4::pango::Layout, f32, bool)> = Vec::new();
                            for tramo in lineas {
                                let pango = view.create_pango_layout(Some(&tramo.text));
                                if let Some(desc) = view.pango_context().font_description() {
                                    let mut mono = gtk4::pango::FontDescription::from_string(
                                        &desc.to_string(),
                                    );
                                    mono.set_family("monospace");
                                    if row.header || tramo.bold {
                                        mono.set_weight(gtk4::pango::Weight::Bold);
                                    }
                                    pango.set_font_description(Some(&mono));
                                }
                                let (lw, _) = pango.pixel_size();
                                layouts.push((pango, lw as f32, tramo.bold));
                                ancho_linea += lw as f32;
                            }
                            let dx = if centrar {
                                ((disponible - ancho_linea) / 2.0).max(0.0)
                            } else {
                                match columns.get(ci).map(|(a, _)| a).copied() {
                                    Some(Align::Right) => (disponible - ancho_linea).max(0.0),
                                    Some(Align::Center) => {
                                        ((disponible - ancho_linea) / 2.0).max(0.0)
                                    }
                                    _ => 0.0,
                                }
                            };
                            let mut x = xs[ci] + CANVAS_CELL_PAD + dx;
                            for (pango, lw, _bold) in layouts {
                                snapshot.save();
                                snapshot.translate(&graphene::Point::new(
                                    x,
                                    base + li as f32 * line_height,
                                ));
                                snapshot.append_layout(&pango, &view.color());
                                snapshot.restore();
                                x += lw;
                            }
                        }
                    }
                    // Filete bajo la cabecera y separadores de fila.
                    let rule = gsk::PathBuilder::new();
                    rule.move_to(xs[0], top + alto);
                    rule.line_to(*xs.last().unwrap(), top + alto);
                    snapshot.append_stroke(&rule.to_path(), &stroke, &palette.muted);
                    // Fronteras verticales: se alargan un píxel arriba y abajo
                    // para coserse con la fila vecina.
                    for fx in &xs {
                        let linea = gsk::PathBuilder::new();
                        linea.move_to(*fx, top - 1.0);
                        linea.line_to(*fx, top + alto + 1.0);
                        snapshot.append_stroke(&linea.to_path(), &stroke, &palette.muted);
                    }
                }
            }
        }

        /// Tarjetas de vídeo: se pintan en el hueco que reserva `youtubegap`,
        /// centradas y en 16:9. La miniatura llega por la misma caché que las
        /// imágenes (descarga en segundo plano); mientras no esté, la tarjeta
        /// sale con el fondo atenuado y el botón de reproducción.
        fn draw_youtube(&self, snapshot: &gtk4::Snapshot) {
            let ornaments = self.ornaments.borrow();
            if ornaments.is_empty() || self.stale() {
                return;
            }
            let palette = self.palette.get();
            let (first_visible, last_visible) = self.visible_lines();
            let (left, right) = self.column();
            let max_w = (right - left - BLOCK_PADDING * 2.0).max(0.0);
            if max_w <= 0.0 {
                return;
            }

            for ornament in ornaments.iter() {
                let Ornament::Youtube { line, id, .. } = ornament else {
                    continue;
                };
                if *line as i32 == self.cursor_line() {
                    continue;
                }
                let Some(top) = self.gap_top(
                    *line as i32,
                    first_visible,
                    last_visible,
                    YOUTUBE_GAP_HEIGHT,
                ) else {
                    continue;
                };
                let (x, y, w, h) = card_rect(left, max_w, top);
                let rounded = gsk::RoundedRect::from_rect(graphene::Rect::new(x, y, w, h), 6.0);
                snapshot.push_rounded_clip(&rounded);
                match self.texture_for(&crate::markdown_render::youtube_thumbnail(id)) {
                    Some(texture) => {
                        // La miniatura se escala para *cubrir* el hueco (sin
                        // deformar y recortando lo que sobra): las miniaturas
                        // de YouTube son 4:3 con bandas negras, y así quedan
                        // fuera.
                        let tw = texture.width() as f32;
                        let th = texture.height() as f32;
                        if tw > 0.0 && th > 0.0 {
                            let escala = (w / tw).max(h / th);
                            let (iw, ih) = (tw * escala, th * escala);
                            snapshot.save();
                            snapshot.translate(&graphene::Point::new(
                                x - (iw - w) / 2.0,
                                y - (ih - h) / 2.0,
                            ));
                            snapshot.scale(escala, escala);
                            snapshot
                                .append_texture(&texture, &graphene::Rect::new(0.0, 0.0, tw, th));
                            snapshot.restore();
                        }
                    }
                    None => snapshot.append_color(&palette.block, &graphene::Rect::new(x, y, w, h)),
                }
                snapshot.pop();
                snapshot.append_border(&rounded, &[1.0, 1.0, 1.0, 1.0], &[palette.muted; 4]);
                draw_play_badge(snapshot, x + w / 2.0, y + h / 2.0, &palette);
            }
        }

        /// Coordenada Y alta del hueco de un adorno de una línea, si esa línea
        /// está (o casi) en pantalla.
        fn gap_top(&self, line: i32, first: i32, last: i32, gap: f32) -> Option<f32> {
            if line < first - 1 || line > last + 1 {
                return None;
            }
            let (line_y, line_h) = self.line_extent(line)?;
            // El hueco queda al final de la línea. Si la altura reportada ya lo
            // incluye, el adorno va pegado al final; si no, justo debajo.
            Some(if line_h > gap + 8.0 {
                line_y + line_h - gap
            } else {
                line_y + line_h
            })
        }
    }

    /// Rectángulo `(x, y, ancho, alto)` de una tarjeta 16:9 dentro del hueco de
    /// `YOUTUBE_GAP_HEIGHT`, centrada en la columna y sin pasar de
    /// `YOUTUBE_MAX_WIDTH`.
    pub(super) fn card_rect(left: f32, max_w: f32, top: f32) -> (f32, f32, f32, f32) {
        let mut w = max_w.min(YOUTUBE_MAX_WIDTH);
        let mut h = w * 9.0 / 16.0;
        if h > YOUTUBE_GAP_HEIGHT - 12.0 {
            h = YOUTUBE_GAP_HEIGHT - 12.0;
            w = h * 16.0 / 9.0;
        }
        let x = left + BLOCK_PADDING + ((max_w - w) / 2.0).max(0.0);
        let y = top + ((YOUTUBE_GAP_HEIGHT - h) / 2.0).max(0.0);
        (x, y, w, h)
    }

    /// Botón de reproducción de una tarjeta: disco oscuro translúcido con un
    /// triángulo claro. Es lo único que se ve mientras la miniatura descarga.
    fn draw_play_badge(snapshot: &gtk4::Snapshot, cx: f32, cy: f32, palette: &OrnamentPalette) {
        let builder = gsk::PathBuilder::new();
        builder.add_circle(&graphene::Point::new(cx, cy), PLAY_RADIUS);
        let disco = gdk::RGBA::new(0.0, 0.0, 0.0, 0.45);
        snapshot.append_fill(&builder.to_path(), gsk::FillRule::Winding, &disco);

        let lado = PLAY_RADIUS * 0.72;
        let triangulo = gsk::PathBuilder::new();
        triangulo.move_to(cx - lado * 0.42, cy - lado * 0.72);
        triangulo.line_to(cx - lado * 0.42, cy + lado * 0.72);
        triangulo.line_to(cx + lado * 0.78, cy);
        triangulo.close();
        snapshot.append_fill(
            &triangulo.to_path(),
            gsk::FillRule::Winding,
            &palette.on_accent,
        );
    }

    /// ¿Es una URL http(s)? Las locales se leen del disco; las remotas, de la
    /// caché de descargas.
    pub(super) fn is_remote(dest: &str) -> bool {
        dest.starts_with("http://") || dest.starts_with("https://")
    }

    /// Directorio de la caché en disco de las imágenes descargadas.
    pub(super) fn remote_cache_dir() -> std::path::PathBuf {
        glib::user_cache_dir().join("scribe").join("images")
    }

    /// Fichero ya descargado de una URL, si está en la caché.
    fn remote_cache_file(url: &str) -> Option<std::path::PathBuf> {
        let path = remote_cache_dir().join(crate::markdown_render::remote_image_cache_name(url));
        path.is_file().then_some(path)
    }

    /// Descarga una imagen remota sin bloquear la UI. Primero pregunta el
    /// tamaño (para no traerse un fichero enorme) y luego el contenido; `listo`
    /// recibe la textura, ya guardada en la caché de disco, o `None` si algo
    /// falló. La red la pone GIO (el backend https de gvfs).
    pub(super) fn fetch_remote_image(url: &str, listo: impl Fn(Option<gdk::Texture>) + 'static) {
        let uri = url.to_string();
        let listo = Rc::new(listo);
        let tras_tamano = listo.clone();
        gio::File::for_uri(url).query_info_async(
            "standard::size",
            gio::FileQueryInfoFlags::NONE,
            glib::Priority::DEFAULT_IDLE,
            gio::Cancellable::NONE,
            move |resultado| {
                if let Ok(info) = resultado {
                    if info.size() > MAX_IMAGE_BYTES as i64 {
                        tras_tamano(None);
                        return;
                    }
                }
                let uri_descarga = uri.clone();
                let tras_carga = tras_tamano.clone();
                gio::File::for_uri(&uri).load_bytes_async(
                    gio::Cancellable::NONE,
                    move |resultado| {
                        let textura = match resultado {
                            Ok((bytes, _etag)) if bytes.len() as u64 <= MAX_IMAGE_BYTES => {
                                save_remote(&uri_descarga, &bytes)
                            }
                            _ => None,
                        };
                        tras_carga(textura);
                    },
                );
            },
        );
    }

    /// Carga una textura con límites de seguridad: ficheros locales (o de la
    /// caché de descargas) de hasta 20 MB. Aquí nunca se toca la red.
    pub(super) fn load_texture(dest: &str) -> Option<gdk::Texture> {
        let path = if is_remote(dest) {
            remote_cache_file(dest)?
        } else {
            std::path::PathBuf::from(dest)
        };
        let meta = std::fs::metadata(&path).ok()?;
        if !meta.is_file() || meta.len() > MAX_IMAGE_BYTES {
            return None;
        }
        gdk::Texture::from_filename(&path).ok()
    }

    /// Comprueba los bytes descargados y los guarda en la caché de disco. Se
    /// escribe a un temporal y se renombra: si dos vistas piden la misma URL a
    /// la vez, el fichero de la caché nunca queda a medias.
    pub(super) fn save_remote(url: &str, bytes: &glib::Bytes) -> Option<gdk::Texture> {
        let texture = gdk::Texture::from_bytes(bytes).ok()?;
        let dir = remote_cache_dir();
        if std::fs::create_dir_all(&dir).is_ok() {
            let path = dir.join(crate::markdown_render::remote_image_cache_name(url));
            let temporal = path.with_extension("part");
            if std::fs::write(&temporal, bytes.as_ref()).is_ok()
                && std::fs::rename(&temporal, &path).is_err()
            {
                let _ = std::fs::remove_file(&temporal);
            }
        }
        Some(texture)
    }

    /// Nivel 1 disco, nivel 2 anillo, nivel 3 en adelante cuadrado: la misma
    /// progresión que usan los navegadores para las listas anidadas.
    fn draw_bullet(
        snapshot: &gtk4::Snapshot,
        cx: f32,
        cy: f32,
        depth: usize,
        palette: &OrnamentPalette,
    ) {
        let builder = gsk::PathBuilder::new();
        match depth {
            1 => {
                builder.add_circle(&graphene::Point::new(cx, cy), BULLET_RADIUS);
                snapshot.append_fill(&builder.to_path(), gsk::FillRule::Winding, &palette.accent);
            }
            2 => {
                builder.add_circle(&graphene::Point::new(cx, cy), BULLET_RADIUS - 0.6);
                let stroke = gsk::Stroke::new(1.4);
                snapshot.append_stroke(&builder.to_path(), &stroke, &palette.accent);
            }
            _ => {
                let side = BULLET_RADIUS * 1.7;
                let rect = graphene::Rect::new(cx - side / 2.0, cy - side / 2.0, side, side);
                let rounded = gsk::RoundedRect::from_rect(rect, 1.0);
                snapshot.push_rounded_clip(&rounded);
                snapshot.append_color(&palette.accent, &rect);
                snapshot.pop();
            }
        }
    }

    fn draw_checkbox(
        snapshot: &gtk4::Snapshot,
        x: f32,
        y: f32,
        checked: bool,
        palette: &OrnamentPalette,
    ) {
        let rect = graphene::Rect::new(x, y, CHECKBOX_SIZE, CHECKBOX_SIZE);
        let rounded = gsk::RoundedRect::from_rect(rect, 4.0);

        if checked {
            snapshot.push_rounded_clip(&rounded);
            snapshot.append_color(&palette.accent, &rect);
            snapshot.pop();

            // La marca se traza a mano: tres puntos y un trazo redondeado.
            let builder = gsk::PathBuilder::new();
            builder.move_to(x + CHECKBOX_SIZE * 0.26, y + CHECKBOX_SIZE * 0.52);
            builder.line_to(x + CHECKBOX_SIZE * 0.44, y + CHECKBOX_SIZE * 0.70);
            builder.line_to(x + CHECKBOX_SIZE * 0.76, y + CHECKBOX_SIZE * 0.32);
            let stroke = gsk::Stroke::new(1.8);
            stroke.set_line_cap(gsk::LineCap::Round);
            stroke.set_line_join(gsk::LineJoin::Round);
            snapshot.append_stroke(&builder.to_path(), &stroke, &palette.on_accent);
        } else {
            snapshot.append_border(&rounded, &[1.5, 1.5, 1.5, 1.5], &[palette.muted; 4]);
        }
    }

    /// Glifo que sustituye al `<br>` dentro de una celda de tabla: una esquina
    /// en forma de «L» con una pequeña flecha hacia abajo, evocando el retorno
    /// de carro. Va en el color apagado para no competir con el contenido.
    fn draw_break(
        snapshot: &gtk4::Snapshot,
        x: f32,
        y: f32,
        height: f32,
        palette: &OrnamentPalette,
    ) {
        let s = height * 0.32;
        let top = y + height * 0.30;
        let bottom = top + s;
        let builder = gsk::PathBuilder::new();
        // Vertical bajando y horizontal a la izquierda: esquina de retorno.
        builder.move_to(x + s, top);
        builder.line_to(x + s, bottom);
        builder.line_to(x, bottom);
        // Pequeña flecha arriba en el extremo derecho, indicando dirección.
        builder.move_to(x + s, top);
        builder.line_to(x + s + s * 0.35, top + s * 0.35);
        builder.move_to(x + s, top);
        builder.line_to(x + s - s * 0.35, top + s * 0.35);
        let stroke = gsk::Stroke::new(1.1);
        stroke.set_line_cap(gsk::LineCap::Round);
        stroke.set_line_join(gsk::LineJoin::Round);
        snapshot.append_stroke(&builder.to_path(), &stroke, &palette.muted);
    }
}

glib::wrapper! {
    pub struct MarkdownView(ObjectSubclass<imp::MarkdownView>)
        @extends sourceview::View, gtk4::TextView, gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Scrollable;
}

impl MarkdownView {
    pub fn with_buffer(buffer: &impl IsA<gtk4::TextBuffer>) -> Self {
        glib::Object::builder().property("buffer", buffer).build()
    }

    /// Sustituye la lista de adornos y repinta. Barata: no calcula geometría,
    /// eso ocurre al dibujar y solo para lo que se ve.
    ///
    /// `links` son los enlaces del documento (para Ctrl+clic y el globo de
    /// ayuda) y `line_count` es el numero de lineas del buffer para el que se
    /// calcularon: sirve para descartar adornos que ya no cuadran.
    pub fn set_ornaments(&self, ornaments: Vec<Ornament>, links: Vec<LinkRef>, line_count: i32) {
        self.imp().ornaments.replace(ornaments);
        self.imp().links.replace(links);
        self.imp().line_count.set(line_count);
        self.queue_draw();
    }

    /// URL del enlace que hay en un punto del widget (coordenadas del widget,
    /// las que recibe un controlador de gestos), si el punto cae sobre un
    /// enlace que la app puede abrir.
    pub fn link_at_position(&self, x: f64, y: f64) -> Option<String> {
        let offset = self.imp().offset_at(x, y)?;
        let links = self.imp().links.borrow();
        let link = crate::markdown_render::link_at(&links, offset)?;
        crate::markdown_render::is_openable_link(&link.dest).then(|| link.dest.clone())
    }

    /// URL del vídeo cuya tarjeta contiene un punto del widget.
    pub fn card_at_position(&self, x: f64, y: f64) -> Option<String> {
        let (bx, by) =
            self.window_to_buffer_coords(gtk4::TextWindowType::Widget, x as i32, y as i32);
        self.imp().youtube_card_at(bx as f64, by as f64)
    }

    /// Línea cuya casilla de tarea hay bajo un punto del widget (las
    /// coordenadas de un gesto), con una zona de acierto algo generosa.
    /// `None` si el punto no cae sobre ninguna casilla.
    pub fn checkbox_at_position(&self, x: f64, y: f64) -> Option<i32> {
        let (bx, by) =
            self.window_to_buffer_coords(gtk4::TextWindowType::Widget, x as i32, y as i32);
        let (bx, by) = (bx as f32, by as f32);
        let imp = self.imp();
        if imp.stale() {
            return None;
        }
        // La caja de cada casilla se deduce de la geometría de su línea (la
        // misma que usa el pintado): acierta el punto que caiga dentro, con
        // un margen de golpe algo generoso.
        let margen = 6.0;
        let mut linea = None;
        for ornamento in imp.ornaments.borrow().iter() {
            let Ornament::Checkbox { line, .. } = ornamento else {
                continue;
            };
            let line = *line as i32;
            let Some((origin_x, origin_y, height)) = imp.line_origin(line) else {
                continue;
            };
            let casilla_x = origin_x - CHECKBOX_OFFSET;
            let casilla_y = origin_y + height / 2.0 - CHECKBOX_SIZE / 2.0;
            if bx >= casilla_x - margen
                && bx <= casilla_x + CHECKBOX_SIZE + margen
                && by >= casilla_y - margen
                && by <= casilla_y + CHECKBOX_SIZE + margen
            {
                linea = Some(line);
                break;
            }
        }
        linea
    }

    /// Alterna la casilla de tarea de una línea entre `[ ]` y `[x]` editando
    /// el fuente. La re-decoración refresca el glifo.
    pub fn toggle_checkbox(&self, line: i32) {
        let buffer = self.buffer();
        let Some(inicio) = buffer.iter_at_line(line) else {
            return;
        };
        let mut fin = inicio;
        if !fin.forward_to_line_end() {
            return;
        }
        let texto = inicio.slice(&fin);
        let Some(corchete) = texto.find('[') else {
            return;
        };
        if !texto[corchete + 1..].starts_with([' ', 'x', 'X']) {
            return;
        }
        // El corchete es un byte del texto de la línea; el iterador trabaja
        // con offsets de carácter.
        let pos = inicio.offset() + texto[..corchete].chars().count() as i32 + 1;
        let mut interior = buffer.iter_at_offset(pos);
        let nuevo = match interior.char() {
            'x' | 'X' => ' ',
            _ => 'x',
        };
        let mut fin_char = buffer.iter_at_offset(pos + 1);
        buffer.delete(&mut interior, &mut fin_char);
        buffer.insert(&mut interior, &nuevo.to_string());
    }

    /// Destino que hay que abrir al hacer clic en un punto: la tarjeta de vídeo
    /// que esté debajo (con clic normal o con Ctrl) o, solo con Ctrl, el enlace
    /// del texto. Es exactamente la decisión que aplica el gesto de clic del
    /// editor, aislada aquí para poder comprobarla sin sintetizar eventos.
    pub fn dest_under_pointer(&self, x: f64, y: f64, ctrl: bool) -> Option<String> {
        self.card_at_position(x, y)
            .or_else(|| ctrl.then(|| self.link_at_position(x, y)).flatten())
    }

    /// Descarga una imagen remota sin bloquear la UI: al llegar, guarda la
    /// textura en la caché (memoria y disco) y repinta. Los fallos también se
    /// recuerdan para no reintentar en cada fotograma.
    fn queue_remote(&self, url: &str) {
        if !self
            .imp()
            .remote_pending
            .borrow_mut()
            .insert(url.to_string())
        {
            return;
        }
        let debil = self.downgrade();
        let uri = url.to_string();
        imp::fetch_remote_image(url, move |textura| {
            if let Some(view) = debil.upgrade() {
                view.remote_done(&uri, textura);
            }
        });
    }

    /// Cierra una descarga: sale de la lista de pendientes, entra en la caché de
    /// texturas y se repinta.
    fn remote_done(&self, url: &str, texture: Option<gdk::Texture>) {
        self.imp().remote_pending.borrow_mut().remove(url);
        self.imp().cache_texture(url, texture);
        self.queue_draw();
    }

    /// Fondo y primer plano del widget por CSS mínimo (issue #25: los temas
    /// cambian el lienzo, no solo los adornos).
    #[allow(deprecated)] // style_context por widget: sin reemplazo directo
    pub fn set_colores_widget(&self, primer_plano: &str, fondo: &str) {
        let provider = gtk4::CssProvider::new();
        provider.load_from_string(&format!(
            "textview {{ background-color: {fondo}; color: {primer_plano}; }}"
        ));
        let ctx = self.style_context();
        let imp = self.imp();
        if let Some(previo) = imp.css_tema.take() {
            ctx.remove_provider(&previo);
        }
        ctx.add_provider(&provider, gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION);
        imp.css_tema.replace(Some(provider));
    }

    pub fn set_palette(&self, palette: OrnamentPalette) {
        if self.imp().palette.get() != palette {
            self.imp().palette.set(palette);
            self.queue_draw();
        }
    }

    /// Olvida la textura cacheada de una ruta para que la siguiente decoración
    /// la vuelva a cargar (p. ej. tras escribir una imagen recién pegada).
    pub fn invalidate_texture(&self, path: &str) {
        self.imp().textures.borrow_mut().remove(path);
    }

    /// Avance horizontal de una celda de tabla, en píxeles. Lo necesita el
    /// editor para convertir a `letter-spacing` los rellenos que el parser
    /// calcula en celdas de carácter.
    pub fn cell_advance(&self) -> f32 {
        self.imp().cell_advance()
    }

    /// Altura de línea de la monoespaciada, en píxeles.
    pub fn cell_height(&self) -> f32 {
        self.imp().cell_height()
    }

    /// Nombre del tag de hueco (`pixels_below_lines`) para `px` píxeles,
    /// creado y cacheado bajo demanda.
    pub fn gap_tag(&self, px: i32) -> glib::GString {
        self.imp().gap_tag(px)
    }

    /// ¿Cae esta línea dentro de una tabla? El editor lo consulta para
    /// re-decorar solo cuando el cursor entra, sale o cambia de fila dentro de
    /// una tabla: es lo único del marcado que se revela en la línea del cursor.
    pub fn line_in_table(&self, line: i32) -> bool {
        self.imp().ornaments.borrow().iter().any(|o| {
            matches!(o, Ornament::Table { first, last } if *first as i32 <= line && line <= *last as i32)
        })
    }

    /// Tag de relleno de tabla para un avance en unidades Pango; el editor lo
    /// aplica sobre el `|` que precede a cada celda.
    pub fn pad_tag(&self, units: i32) -> glib::GString {
        self.imp().pad_tag(units)
    }
}

impl Default for MarkdownView {
    fn default() -> Self {
        glib::Object::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn la_tarjeta_de_video_es_16_9_y_cabe_en_su_hueco() {
        let (x, y, w, h) = imp::card_rect(10.0, 900.0, 100.0);
        assert!((w / h - 16.0 / 9.0).abs() < 0.06, "{w}x{h} no es 16:9");
        assert!(h <= YOUTUBE_GAP_HEIGHT, "{h} no cabe en el hueco");
        assert!(x >= 10.0 + BLOCK_PADDING);
        assert!(y >= 100.0);
        // En una columna estrecha la tarjeta se ajusta al ancho disponible.
        let (esq, _, ancho, _) = imp::card_rect(0.0, 200.0, 0.0);
        assert!(ancho <= 200.0);
        assert_eq!(esq, BLOCK_PADDING);
    }

    #[test]
    fn las_imagenes_remotas_se_reconocen_por_el_esquema() {
        assert!(imp::is_remote("https://example.com/a.png"));
        assert!(imp::is_remote("http://example.com/a.png"));
        assert!(!imp::is_remote("/tmp/a.png"));
        assert!(!imp::is_remote("ftp://example.com/a.png"));
    }

    /// E2E de la descarga remota: pide una imagen real, espera a que llegue y
    /// comprueba que queda en la caché de disco y que se recarga de ahí (sin
    /// volver a pedirla). Se marca como ignorada porque necesita red; se
    /// ejecuta a mano con `--ignored`. No inicializa GTK: las texturas y GIO no
    /// necesitan display, así que puede correr junto a otros tests del proceso.
    #[test]
    #[ignore = "requiere red"]
    fn una_imagen_remota_se_descarga_y_queda_en_la_cache() {
        let url = "https://commonmark.org/help/images/favicon.png";
        let cache =
            imp::remote_cache_dir().join(crate::markdown_render::remote_image_cache_name(url));
        let _ = std::fs::remove_file(&cache);
        assert!(
            imp::load_texture(url).is_none(),
            "la caché debería estar vacía"
        );

        let resultado: std::rc::Rc<RefCell<Option<bool>>> = Default::default();
        let guardado = resultado.clone();
        imp::fetch_remote_image(url, move |textura| {
            *guardado.borrow_mut() = Some(textura.is_some());
        });

        let contexto = glib::MainContext::default();
        let limite = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while resultado.borrow().is_none() && std::time::Instant::now() < limite {
            while contexto.iteration(false) {}
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        assert_eq!(*resultado.borrow(), Some(true), "no llegó la textura");
        assert!(cache.is_file(), "la descarga no se guardó en la caché");
        let textura = imp::load_texture(url).expect("la caché debería cargarse del disco");
        assert!(textura.width() > 0 && textura.height() > 0);
    }
}
