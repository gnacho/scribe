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

use crate::markdown_render::Ornament;

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
const BLOCK_PADDING: f32 = 8.0;
const BLOCK_RADIUS: f32 = 8.0;
const QUOTE_BAR_WIDTH: f32 = 3.0;
const QUOTE_BAR_OFFSET: f32 = 20.0;
const RULE_THICKNESS: f32 = 1.0;
/// Grosor de la rejilla vertical de una tabla, en píxeles.
const TABLE_GRID_THICKNESS: f32 = 1.0;
/// Alto del hueco que el tag `imagegap` reserva bajo la línea de una imagen.
const IMAGE_GAP_HEIGHT: f32 = crate::markdown_render::IMAGE_GAP_HEIGHT as f32;
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

    #[derive(Default)]
    pub struct MarkdownView {
        pub ornaments: RefCell<Vec<Ornament>>,
        pub palette: Cell<OrnamentPalette>,
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
        fn line_origin(&self, line: i32) -> Option<(f32, f32, f32)> {
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
        fn table_extent(
            &self,
            first: i32,
            last: i32,
            anchor_top: i32,
            anchor_bottom: i32,
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
                start_x = start_x.min(view.iter_location(&iter).x() as f32);
                if !iter.ends_line() {
                    iter.forward_to_line_end();
                }
                end_x = end_x.max(view.iter_location(&iter).x() as f32);
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
                    self.grid_extent(*anchor as i32, bounds)
                }
                _ => None,
            });
            if let Some((x0, width, _)) = grid {
                return (
                    (x0 - BLOCK_PADDING).max(left),
                    (x0 + width + BLOCK_PADDING).min(right),
                );
            }
            match self.table_extent(first, last, anchor_top, anchor_bottom) {
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
        fn stale(&self) -> bool {
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

            for ornament in ornaments.iter() {
                let (first, last) = match ornament {
                    Ornament::CodeBlock { first, last }
                    | Ornament::Table { first, last }
                    | Ornament::Quote { first, last } => (*first as i32, *last as i32),
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
                let Some((x0, _, advance)) = self.grid_extent(anchor, bounds) else {
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

            for ornament in ornaments.iter() {
                let line = match ornament {
                    Ornament::Bullet { line, .. }
                    | Ornament::Checkbox { line, .. }
                    | Ornament::TableRule { line }
                    | Ornament::HeadingRule { line }
                    | Ornament::Rule { line } => *line as i32,
                    _ => continue,
                };
                if line < first_visible || line > last_visible {
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
                                self.grid_extent(*anchor as i32, bounds)
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

        /// Textura de una ruta, cacheada. Las URLs no se cargan (sin red en
        /// el sandbox): devuelven `None` y se pinta el placeholder.
        fn texture_for(&self, dest: &str) -> Option<gdk::Texture> {
            if let Some(cached) = self.textures.borrow().get(dest) {
                return cached.clone();
            }
            let loaded = load_texture(dest);
            let mut cache = self.textures.borrow_mut();
            if cache.len() >= MAX_TEXTURE_CACHE {
                cache.clear();
            }
            cache.insert(dest.to_string(), loaded.clone());
            loaded
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
                if line < first_visible - 1 || line > last_visible + 1 {
                    continue;
                }
                let Some((line_y, line_h)) = self.line_extent(line) else {
                    continue;
                };
                // El hueco de `imagegap` queda al final de la línea. Si la
                // altura reportada lo incluye, la imagen va pegada al final;
                // si no, justo debajo del texto.
                let top = if line_h > IMAGE_GAP_HEIGHT + 8.0 {
                    line_y + line_h - IMAGE_GAP_HEIGHT
                } else {
                    line_y + line_h
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
    }

    /// Carga una textura local con límites de seguridad: sin red (las URLs se
    /// rechazan), solo ficheros regulares de hasta 20 MB.
    fn load_texture(dest: &str) -> Option<gdk::Texture> {
        if dest.starts_with("http://") || dest.starts_with("https://") {
            return None;
        }
        let path = std::path::Path::new(dest);
        let meta = std::fs::metadata(path).ok()?;
        if !meta.is_file() || meta.len() > MAX_IMAGE_BYTES {
            return None;
        }
        gdk::Texture::from_filename(path).ok()
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
    /// `line_count` es el numero de lineas del buffer para el que se
    /// calcularon: sirve para descartar adornos que ya no cuadran.
    pub fn set_ornaments(&self, ornaments: Vec<Ornament>, line_count: i32) {
        self.imp().ornaments.replace(ornaments);
        self.imp().line_count.set(line_count);
        self.queue_draw();
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
