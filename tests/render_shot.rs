//! Captura visual del editor: renderiza un documento a PNGs paginados para
//! verificar la vista enriquecida (tablas, cajas de código, filetes de
//! título, citas, listas, imágenes) sin abrir ventanas interactivas.
//!
//! Ejecutar: xvfb-run -a cargo test --test render_shot -- --ignored --nocapture
//! Entrada:  MD_PATH (por defecto, documento de muestra integrado)
//! Salida:   /mnt/agents/work/shots/pagina-NN.png

// Los módulos de la app se incluyen vía #[path] y el harness solo ejercita
// parte de su API: el resto aparece como código muerto aunque la app sí lo
// use (mismo criterio que tests/crash_stress.rs).
#![allow(dead_code)]

#[path = "../src/editor.rs"]
mod editor;
#[path = "../src/markdown_render.rs"]
mod markdown_render;
#[path = "../src/markdown_view.rs"]
mod markdown_view;
#[path = "../src/settings.rs"]
mod settings;

use editor::Editor;
use gtk4::prelude::*;
use std::time::{Duration, Instant};

const DOC: &str = "\
# Título principal

| Command    | Description                      | Precio |
| ---------- | -------------------------------- | -----: |
| git status | List all **new** or modified     |    $12 |
| git diff   | Show file `differences`          |  $1600 |
";

/// PNG de 64x48 generado con un codificador real (bicolor rojo/azul), para
/// probar el pintado de imágenes locales.
const TEST_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 64, 0, 0, 0, 48, 8, 2,
    0, 0, 0, 46, 41, 235, 72, 0, 0, 0, 91, 73, 68, 65, 84, 120, 156, 237, 209, 65, 13, 192, 48, 16,
    3, 193, 166, 184, 14, 73, 17, 7, 86, 65, 228, 49, 138, 180, 131, 192, 43, 175, 249, 246, 115,
    179, 87, 15, 56, 85, 128, 86, 128, 86, 128, 86, 128, 86, 128, 86, 128, 86, 128, 86, 128, 86,
    128, 86, 128, 86, 128, 182, 246, 140, 222, 112, 228, 250, 7, 10, 208, 10, 208, 10, 208, 10,
    208, 10, 208, 10, 208, 10, 208, 10, 208, 10, 208, 10, 208, 10, 208, 126, 242, 14, 2, 253, 217,
    136, 163, 109, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

fn pump(ctx: &gtk4::glib::MainContext, ms: u64) {
    let t = Instant::now();
    while t.elapsed() < Duration::from_millis(ms) {
        while ctx.iteration(false) {}
        std::thread::sleep(Duration::from_millis(4));
    }
}

/// Renderiza el contenido visible de la ventana a un PNG usando el renderer
/// de la superficie ya realizada.
fn shot(win: &gtk4::Window, path: &std::path::Path) {
    let w = win.width() as f64;
    let h = win.height() as f64;
    let paintable = gtk4::WidgetPaintable::new(Some(win));
    let snapshot = gtk4::Snapshot::new();
    paintable.snapshot(&snapshot, w, h);
    let node = snapshot.to_node().expect("snapshot sin nodo raíz");
    let renderer = win
        .native()
        .expect("sin native")
        .renderer()
        .expect("sin renderer");
    let texture = renderer.render_texture(&node, None);
    texture.save_to_png(path).expect("guardar png");
}

#[test]
#[ignore = "requiere display: xvfb-run -a cargo test --test render_shot -- --ignored --nocapture"]
fn captura_vista_enriquecida() {
    gtk4::init().expect("GTK necesita un display; ejecuta bajo xvfb-run");
    libadwaita::init().expect("libadwaita init");

    let out = std::env::var("SCRIBE_SHOTS_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("scribe-shots"));
    std::fs::create_dir_all(&out).expect("crear dir de salida");
    std::fs::write(out.join("test.png"), TEST_PNG).expect("escribir imagen de prueba");

    // Documento: el indicado por MD_PATH o la muestra integrada.
    let doc = std::env::var("MD_PATH")
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_else(|| DOC.to_string());

    let editor = Editor::new();
    editor.set_base_dir(Some(out.to_path_buf()));
    let win = gtk4::Window::builder()
        .default_width(760)
        .default_height(1000)
        .build();
    win.set_child(Some(&editor.widget));
    win.present();

    let ctx = gtk4::glib::MainContext::default();
    editor.set_text(&doc);
    // Drena el debounce de decoración y el layout. Con imágenes remotas hay que
    // dar más margen (SCRIBE_WAIT_MS) para que lleguen las descargas; la segunda
    // pasada las lee ya de la caché de disco.
    let espera = std::env::var("SCRIBE_WAIT_MS")
        .ok()
        .and_then(|ms| ms.parse().ok())
        .unwrap_or(800);
    pump(&ctx, espera);

    let total = editor.text().lines().count().max(1);
    // Filas de texto por página (~1000 px de ventana): paginar con solape de
    // una línea para no cortar cajas ni tablas a la mitad sin contexto.
    let per_page = 38usize;
    let mut first = 0usize;
    let mut page = 0usize;
    while first < total {
        editor.scroll_to_line(first as i32);
        pump(&ctx, 350);
        shot(&win, &out.join(format!("pagina-{page:02}.png")));
        page += 1;
        first += per_page;
    }
    eprintln!("{page} paginas escritas en {}", out.display());
}

/// Lienzo de tabla (issue #54): las filas fuente de una tabla de prosa se
/// colapsan y reciben un tag de hueco; la fila del cursor queda cruda.
#[test]
#[ignore = "requiere display"]
fn lienzo_colapsa_filas_y_revela_la_del_cursor() {
    gtk4::init().expect("GTK necesita un display");
    libadwaita::init().expect("libadwaita init");
    let ctx = gtk4::glib::MainContext::default();
    let doc = "| Una cabecera bastante larga que no cabe | B |\n|---|---|\n| una celda de prosa bastante larga que no cabe en su columna corta para nada | otra |\n";
    let editor = Editor::new();
    let win = gtk4::Window::builder()
        .default_width(700)
        .default_height(400)
        .build();
    win.set_child(Some(&editor.widget));
    win.present();
    editor.set_text(doc);
    pump(&ctx, 1200);

    let buffer = editor.view.buffer();
    let colapsado = buffer
        .tag_table()
        .lookup("tcanvas_src")
        .expect("tag de colapso creado");
    // Fila de contenido (línea 2): colapsada y con hueco.
    let iter = buffer.iter_at_line(2).expect("línea 2");
    assert!(
        iter.has_tag(&colapsado),
        "la fila de contenido lleva el colapso"
    );
    let con_hueco = std::cell::Cell::new(false);
    buffer.tag_table().foreach(|t| {
        if t.name()
            .map(|n| n.starts_with("tcanvasgap"))
            .unwrap_or(false)
        {
            con_hueco.set(true);
        }
    });
    assert!(con_hueco.get(), "hay un tag de hueco de lienzo");

    // Con el cursor en esa fila se revela cruda: sin colapso tras decorar.
    editor.place_cursor_at(3, 1);
    pump(&ctx, 600);
    let iter = buffer.iter_at_line(2).expect("línea 2");
    assert!(
        !iter.has_tag(&colapsado),
        "la fila del cursor se revela sin colapso"
    );
}

/// Corrector ortográfico (issue #51): encendido marca la palabra mal escrita
/// con el tag de libspelling; apagado la deja limpia.
#[test]
#[ignore = "requiere display y diccionario del sistema (es_ES)"]
fn ortografia_subraya_y_se_apaga() {
    gtk4::init().expect("GTK necesita un display");
    libadwaita::init().expect("libadwaita init");
    libspelling::init();
    let ctx = gtk4::glib::MainContext::default();
    let editor = Editor::new();
    // SIN ventana (bisect): la vista vive sin present.
    editor.set_text("holaa mundo\n");
    // libspelling no marca la palabra que solapa con el cursor: el cursor va
    // al final del documento (como dejara el usuario al terminar de escribir).
    editor.place_cursor_at(1, 1);
    // libspelling no marca el rango que solapa con el cursor (no subraya lo
    // que estas tecleando): el cursor va al final del documento.
    // SIN cursor movido (variante de depuracion)
    let tabla = editor.view.buffer().tag_table();
    tabla.connect_tag_added(|_t, tag| {
        eprintln!(
            "TAG-ADDED: name={:?} anon={}",
            tag.name(),
            tag.name().is_none()
        );
    });
    tabla.connect_tag_removed(|_t, tag| {
        eprintln!(
            "TAG-REMOVED: name={:?} anon={}",
            tag.name(),
            tag.name().is_none()
        );
    });
    editor.set_spellcheck(true);
    pump(&ctx, 2500);

    let buffer = editor.view.buffer();
    let tag = buffer
        .tag_table()
        .lookup("ortografia")
        .expect("el tag de ortografia existe en la tabla");
    let marcada = |buffer: &gtk4::TextBuffer, tag: &gtk4::TextTag| {
        let ultimo = buffer.end_iter().offset();
        (0..ultimo).any(|off| buffer.iter_at_offset(off).has_tag(tag))
    };
    pump(&ctx, 4000);
    assert!(
        marcada(&buffer, &tag),
        "la palabra mal escrita queda marcada por el corrector"
    );
    assert_eq!(
        tag.underline(),
        gtk4::pango::Underline::Single,
        "con el corrector encendido, la marca subraya"
    );

    // Al apagarlo la marca permanece aplicada (apagarla del todo rompe
    // libspelling 0.4) pero deja de subrayar.
    editor.set_spellcheck(false);
    assert_eq!(
        tag.underline(),
        gtk4::pango::Underline::None,
        "sin subrayado"
    );
    editor.set_spellcheck(true);
    assert_eq!(
        tag.underline(),
        gtk4::pango::Underline::Single,
        "vuelve a subrayar"
    );
}

/// Prueba directa de libspelling (sin el Editor): buffer + checker + adapter.
#[test]
#[ignore = "requiere display"]
fn prueba_directa_libspelling() {
    gtk4::init().expect("GTK");
    libspelling::init();
    let buffer = gtksourceview5::Buffer::new(None::<&gtk4::TextTagTable>);
    let checker = libspelling::Checker::default();
    let adapter = libspelling::TextBufferAdapter::new(&buffer, &checker);
    buffer.set_text("holaa mundo\n");

    // CON ventana y vista (variante de bisect): igual que el flujo del Editor.
    let vista = gtksourceview5::View::with_buffer(&buffer);
    let win = gtk4::Window::builder()
        .default_width(700)
        .default_height(400)
        .build();
    win.set_child(Some(&vista));
    win.present();

    // CON libadwaita (como el test de ortografia).
    let _ = libadwaita::init();

    // CON menu extra (como el flujo del Editor).
    let vista = gtksourceview5::View::with_buffer(&buffer);
    vista.set_extra_menu(Some(&adapter.menu_model()));

    // CON cursor movido (como el flujo del Editor).
    let mut fin = buffer.start_iter();
    fin.forward_to_end();
    buffer.place_cursor(&fin);

    adapter.set_enabled(true);

    let ctx = gtk4::glib::MainContext::default();
    let inicio = std::time::Instant::now();
    let mut marcada = false;
    while inicio.elapsed().as_millis() < 3000 && !marcada {
        while ctx.iteration(false) {}
        std::thread::sleep(std::time::Duration::from_millis(20));
        if let Some(tag) = adapter.tag() {
            let ultimo = buffer.end_iter().offset();
            marcada = (0..ultimo).any(|off| buffer.iter_at_offset(off).has_tag(&tag));
        }
    }
    eprintln!(
        "DEBUG directa: marcada={marcada} idioma={:?}",
        adapter.language()
    );
    assert!(marcada, "libspelling no marca en la prueba directa");
}

/// Bisect del marcado: cada paso anade una diferencia respecto a la prueba
/// directa que SI funciona.
#[test]
#[ignore = "requiere display"]
fn bisect_libspelling() {
    gtk4::init().expect("GTK");
    libspelling::init();
    let ctx = gtk4::glib::MainContext::default();
    let marca = |buffer: &gtksourceview5::Buffer, adapter: &libspelling::TextBufferAdapter| {
        let inicio = std::time::Instant::now();
        let mut marcada = false;
        while inicio.elapsed().as_millis() < 2000 && !marcada {
            while ctx.iteration(false) {}
            std::thread::sleep(std::time::Duration::from_millis(20));
            if let Some(tag) = adapter.tag() {
                let ultimo = buffer.end_iter().offset();
                marcada = (0..ultimo).any(|off| buffer.iter_at_offset(off).has_tag(&tag));
            }
        }
        marcada
    };

    // B: buffer CON tabla de tags.
    let tags = gtk4::TextTagTable::new();
    let buffer = gtksourceview5::Buffer::new(Some(&tags));
    let checker = libspelling::Checker::default();
    let adapter = libspelling::TextBufferAdapter::new(&buffer, &checker);
    buffer.set_text("holaa mundo\n");
    // B4: menu extra ANTES de encender.
    let vista = gtksourceview5::View::with_buffer(&buffer);
    vista.set_extra_menu(Some(&adapter.menu_model()));
    adapter.set_enabled(true);
    let ok = marca(&buffer, &adapter);
    eprintln!("B (tabla de tags + menu): {ok}");

    // C: apagado al crear y encendido despues.
    let tags = gtk4::TextTagTable::new();
    let buffer = gtksourceview5::Buffer::new(Some(&tags));
    let checker = libspelling::Checker::default();
    let adapter = libspelling::TextBufferAdapter::new(&buffer, &checker);
    adapter.set_enabled(false);
    buffer.set_text("holaa mundo\n");
    adapter.set_enabled(true);
    let ok = marca(&buffer, &adapter);
    eprintln!("C (apagado y encendido): {ok}");

    // D: como C pero moviendo el cursor al final.
    let tags = gtk4::TextTagTable::new();
    let buffer = gtksourceview5::Buffer::new(Some(&tags));
    let checker = libspelling::Checker::default();
    let adapter = libspelling::TextBufferAdapter::new(&buffer, &checker);
    adapter.set_enabled(false);
    buffer.set_text("holaa mundo\n");
    let mut fin = buffer.start_iter();
    fin.forward_to_end();
    buffer.place_cursor(&fin);
    adapter.set_enabled(true);
    let ok = marca(&buffer, &adapter);
    eprintln!("D (cursor al final): {ok}");

    // B2: buffer CON la tabla de tags del Editor (build_tags).
    let editor2 = Editor::new();
    let tabla = editor2.view.buffer().tag_table();
    let buffer = gtksourceview5::Buffer::new(Some(&tabla));
    let checker = libspelling::Checker::default();
    let adapter = libspelling::TextBufferAdapter::new(&buffer, &checker);
    buffer.set_text("holaa mundo\n");
    adapter.set_enabled(true);
    let ok = {
        let inicio = std::time::Instant::now();
        let mut m = false;
        while inicio.elapsed().as_millis() < 2000 && !m {
            while ctx.iteration(false) {}
            std::thread::sleep(std::time::Duration::from_millis(20));
            if let Some(tag) = adapter.tag() {
                let ultimo = buffer.end_iter().offset();
                m = (0..ultimo).any(|off| buffer.iter_at_offset(off).has_tag(&tag));
            }
        }
        m
    };
    eprintln!("B2 (tabla del editor): {ok}");

    // B3: buffer del editor EN VIVO (con decorate corriendo al editar).
    let buffer = editor2
        .view
        .buffer()
        .downcast::<gtksourceview5::Buffer>()
        .expect("buffer del editor");
    let checker = libspelling::Checker::default();
    let adapter = libspelling::TextBufferAdapter::new(&buffer, &checker);
    buffer.set_text("holaa mundo\n");

    // B3b: CON ventana presentada (bisect).
    let win3 = gtk4::Window::builder()
        .default_width(700)
        .default_height(400)
        .build();
    win3.set_child(Some(&editor2.widget));
    win3.present();
    pump(&ctx, 400);
    let ok3 = {
        let inicio = std::time::Instant::now();
        let mut m = false;
        while inicio.elapsed().as_millis() < 2000 && !m {
            while ctx.iteration(false) {}
            std::thread::sleep(std::time::Duration::from_millis(20));
            if let Some(tag) = adapter.tag() {
                let ultimo = buffer.end_iter().offset();
                m = (0..ultimo).any(|off| buffer.iter_at_offset(off).has_tag(&tag));
            }
        }
        m
    };
    eprintln!("B3b (con ventana): {ok3}");
    let ok = {
        let inicio = std::time::Instant::now();
        let mut m = false;
        while inicio.elapsed().as_millis() < 2000 && !m {
            while ctx.iteration(false) {}
            std::thread::sleep(std::time::Duration::from_millis(20));
            if let Some(tag) = adapter.tag() {
                let ultimo = buffer.end_iter().offset();
                m = (0..ultimo).any(|off| buffer.iter_at_offset(off).has_tag(&tag));
            }
        }
        m
    };
    eprintln!("B3 (buffer del editor en vivo): {ok}");

    // E: como D mas el menu extra.
    let tags = gtk4::TextTagTable::new();
    let buffer = gtksourceview5::Buffer::new(Some(&tags));
    let checker = libspelling::Checker::default();
    let adapter = libspelling::TextBufferAdapter::new(&buffer, &checker);
    adapter.set_enabled(false);
    buffer.set_text("holaa mundo\n");
    let mut fin = buffer.start_iter();
    fin.forward_to_end();
    buffer.place_cursor(&fin);
    let menu = adapter.menu_model();
    adapter.set_enabled(true);
    let ok = marca(&buffer, &adapter);
    eprintln!("E (con menu): {ok} menu={}", menu.n_items());
}
