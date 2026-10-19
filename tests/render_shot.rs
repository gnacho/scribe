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

#[path = "../src/tema.rs"]
mod tema;

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
/// con el tag propio; apagado deja de subrayar.
#[test]
#[ignore = "requiere display y diccionario del sistema (es_ES)"]
fn ortografia_subraya_y_se_apaga() {
    gtk4::init().expect("GTK necesita un display");
    libadwaita::init().expect("libadwaita init");
    let ctx = gtk4::glib::MainContext::default();
    let editor = Editor::new();
    // SIN ventana (bisect): la vista vive sin present.
    editor.set_text("holaa mundo\n");
    // El subrayado en vivo es incómodo sobre la palabra que se teclea: el
    // cursor va al final del documento (como lo dejara el usuario).
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

/// Margen lateral acotado (issue #63): en una ventana ancha el margen no pasa
/// de 1/8 de la ventana; en una estrecha manda el minimo de 24 px.
#[test]
#[ignore = "requiere display"]
fn margen_acotado_en_ventana_ancha() {
    gtk4::init().expect("GTK necesita un display");
    libadwaita::init().expect("libadwaita init");
    let ctx = gtk4::glib::MainContext::default();
    let editor = Editor::new();
    let win = gtk4::Window::builder()
        .default_width(1500)
        .default_height(500)
        .build();
    win.set_child(Some(&editor.widget));
    win.present();
    editor.set_text("pros\n");
    pump(&ctx, 1500);

    // 1500/8 = 187: el margen se queda ahi en vez de (1500-720)/2 = 390.
    assert_eq!(
        editor.view.left_margin(),
        187,
        "el margen lateral se acota a 1/8 de la ventana"
    );

    // Ventana estrecha: manda el minimo de 24 px.
    let estrecha = gtk4::Window::builder()
        .default_width(760)
        .default_height(400)
        .build();
    let editor2 = Editor::new();
    estrecha.set_child(Some(&editor2.widget));
    estrecha.present();
    editor2.set_text("pros\n");
    pump(&ctx, 1500);
    assert_eq!(
        editor2.view.left_margin(),
        24,
        "en ventana estrecha manda el minimo"
    );
}

/// Casillas de tarea clicables: el punto de la casilla se reconoce y el
/// toggle edita el fuente entre [ ] y [x] (issue de listas de tareas).
#[test]
#[ignore = "requiere display"]
fn casillas_de_tarea_clicables() {
    gtk4::init().expect("GTK necesita un display");
    libadwaita::init().expect("libadwaita init");
    let ctx = gtk4::glib::MainContext::default();
    let editor = Editor::new();
    let win = gtk4::Window::builder()
        .default_width(760)
        .default_height(400)
        .build();
    win.set_child(Some(&editor.widget));
    win.present();
    editor.set_text("- [ ] tarea pendiente\n- [x] tarea hecha\n");
    pump(&ctx, 1200);

    let view = &editor.view;
    let buffer = view.buffer();
    // Casilla de la linea 0: centro de la casilla en coordenadas de buffer.
    let iter = buffer.iter_at_line(0).expect("linea 0");
    let loc = view.iter_location(&iter);
    let centro_buffer = (
        (loc.x() as f32 - 15.0) as i32,
        (loc.y() as f32 + loc.height() as f32 / 2.0) as i32,
    );
    let (wx, wy) = view.buffer_to_window_coords(
        gtk4::TextWindowType::Widget,
        centro_buffer.0,
        centro_buffer.1,
    );
    assert_eq!(
        view.checkbox_at_position(wx as f64, wy as f64),
        Some(0),
        "el centro de la casilla de la linea 0 se reconoce"
    );
    assert_eq!(
        view.checkbox_at_position(wx as f64 + 300.0, wy as f64),
        None,
        "un punto alejado de la casilla no se reconoce"
    );

    view.toggle_checkbox(0);
    pump(&ctx, 400);
    let texto = buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), true)
        .to_string();
    assert!(
        texto.starts_with("- [x] tarea pendiente"),
        "el toggle marca la tarea: {texto:?}"
    );

    view.toggle_checkbox(0);
    pump(&ctx, 400);
    let texto = buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), true)
        .to_string();
    assert!(
        texto.starts_with("- [ ] tarea pendiente"),
        "el toggle desmarca de nuevo: {texto:?}"
    );
    assert!(
        texto.contains("- [x] tarea hecha"),
        "la otra casilla no se toca"
    );
}

/// Modo foco: las marcas de sintaxis solo se muestran en la línea del cursor;
/// el resto quedan encogidas (syn_shrink; sin invisible por gtk#8346). Al
/// mover el cursor se conmutan.
#[test]
#[ignore = "requiere display"]
fn marcas_solo_en_la_linea_del_cursor() {
    gtk4::init().expect("GTK necesita un display");
    libadwaita::init().expect("libadwaita init");
    let ctx = gtk4::glib::MainContext::default();
    let editor = Editor::new();
    let win = gtk4::Window::builder()
        .default_width(760)
        .default_height(400)
        .build();
    win.set_child(Some(&editor.widget));
    win.present();
    let doc = "# Titulo\n\n- **negrita** en lista\n";
    editor.set_text(doc);
    // Cursor en el titulo (linea 1 en base 1): su marca visible, la de la
    // lista encogida.
    editor.place_cursor_at(1, 2);
    pump(&ctx, 1200);
    let buffer = editor.view.buffer();
    let nombre_en = |offset: i32| -> String {
        buffer
            .iter_at_offset(offset)
            .tags()
            .iter()
            .filter_map(|t| t.name().map(|n| n.as_str().to_string()))
            .find(|n| n.starts_with("syn_"))
            .unwrap_or_else(|| "NINGUNO".to_string())
    };
    assert_eq!(
        nombre_en(1),
        "syn_shown",
        "la marca del titulo del cursor se muestra"
    );
    assert_eq!(
        nombre_en(11),
        "syn_shrink",
        "el guion de la lista (otra linea) queda encogido"
    );
    assert_eq!(
        nombre_en(13),
        "syn_shrink",
        "los asteriscos de la lista quedan encogidos"
    );

    // Cursor en la lista (linea 3 en base 1): se conmutan.
    editor.place_cursor_at(3, 5);
    pump(&ctx, 1200);
    assert_eq!(
        nombre_en(1),
        "syn_shrink",
        "al salir del titulo, su marca queda encogida"
    );
    assert_eq!(
        nombre_en(11),
        "syn_shown",
        "la marca de la linea del cursor se muestra"
    );
    assert_eq!(
        nombre_en(13),
        "syn_shown",
        "los asteriscos de la linea del cursor se muestran"
    );
}

/// Arranque sin salto de ancho (petición): la vista arranca oculta y se
/// revela cuando el primer ancho real ya aplico sus margenes.
#[test]
#[ignore = "requiere display"]
fn arranque_sin_salto_de_ancho() {
    gtk4::init().expect("GTK necesita un display");
    libadwaita::init().expect("libadwaita init");
    let ctx = gtk4::glib::MainContext::default();
    let editor = Editor::new();
    let win = gtk4::Window::builder()
        .default_width(1500)
        .default_height(500)
        .build();
    win.set_child(Some(&editor.widget));
    win.present();
    editor.set_text("# Doc\n\npros de relleno para que haya algo que decorar\n");

    // La vista se revela en cuanto la primera decoracion corre con la
    // geometria definitiva (antes de que el usuario vea nada a medio hacer).
    pump(&ctx, 700);
    assert!(
        (editor.view.opacity() - 1.0).abs() < 0.01,
        "la vista se revela (opacity={})",
        editor.view.opacity()
    );
    assert_eq!(
        editor.view.left_margin(),
        187,
        "el margen acotado ya esta aplicado al revelar"
    );
}

/// Corrector (issue #62): las acciones spellcheck.* anaden al diccionario,
/// ignoran la sesion y reemplazan la palabra bajo el cursor.
#[test]
#[ignore = "requiere display y diccionario del sistema (es_ES)"]
fn menu_ortografia_sugerencias_y_diccionario() {
    gtk4::init().expect("GTK necesita un display");
    libadwaita::init().expect("libadwaita init");
    let ctx = gtk4::glib::MainContext::default();
    let editor = Editor::new();
    let win = gtk4::Window::builder()
        .default_width(760)
        .default_height(400)
        .build();
    win.set_child(Some(&editor.widget));
    win.present();
    editor.set_text("el qwerz raro\n\notra linea con xyzzy suelta\n");
    pump(&ctx, 1200);

    let buffer = editor.view.buffer();
    let subrayado = buffer
        .tag_table()
        .lookup("ortografia")
        .expect("tag de ortografia");
    let marcada = |offset: i32| buffer.iter_at_offset(offset).has_tag(&subrayado);

    // "qwerz" ocupa los offsets 3..8 de la linea 0.
    assert!(marcada(3), "qwerz arranca marcada");

    // Anadir al diccionario personal: la palabra deja de marcarse.
    editor.place_cursor_at(1, 5);
    pump(&ctx, 100);
    editor
        .view
        .activate_action("spellcheck.anadir", Some(&"qwerz".to_variant()))
        .expect("la accion anadir existe y se activa");
    pump(&ctx, 700);
    assert!(!marcada(3), "tras anadir al diccionario, qwerz no se marca");

    // Reemplazar la palabra bajo el cursor: "xyzzy" -> "quiso".
    editor.place_cursor_at(3, 17);
    pump(&ctx, 100);
    editor
        .view
        .activate_action("spellcheck.reemplazar", Some(&"quiso".to_variant()))
        .expect("la accion reemplazar existe y se activa");
    pump(&ctx, 700);
    let texto = buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), true)
        .to_string();
    assert!(
        texto.contains("quiso"),
        "el reemplazo inserta la sugerencia: {texto:?}"
    );
    assert!(
        !texto.contains("xyzzy"),
        "el reemplazo elimina la palabra: {texto:?}"
    );
}

/// Temas semánticos (issue #25): con el tema sepia elegido en la preferencia
/// `theme`, los tags y la paleta toman sus colores.
#[test]
#[ignore = "requiere display"]
fn tema_sepia_aplica_sus_colores() {
    gtk4::init().expect("GTK necesita un display");
    libadwaita::init().expect("libadwaita init");
    let ctx = gtk4::glib::MainContext::default();
    // El tema se lee de gsettings en la creacion del editor: keyfile previo.
    let xdg = std::path::PathBuf::from("/tmp/opencode/xdg-tema-test");
    std::fs::create_dir_all(xdg.join("glib-2.0/settings")).ok();
    std::fs::write(
        xdg.join("glib-2.0/settings/keyfile"),
        "[app/scribe/Scribe]\ntheme=\"sepia\"\n",
    )
    .ok();
    // Sin poder re-dirigir el backend del proceso, se fija la propiedad
    // directamente sobre el esquema instalado y se restaura al terminar.
    let ajustes = gio::Settings::new("app.scribe.Scribe");
    let previo = ajustes.string("theme").to_string();
    ajustes.set_string("theme", "sepia").unwrap();
    let editor = Editor::new();
    ajustes.set_string("theme", &previo).unwrap();
    let win = gtk4::Window::builder()
        .default_width(760)
        .default_height(400)
        .build();
    win.set_child(Some(&editor.widget));
    win.present();
    editor.set_text("# Titulo\n\n[enlace](https://example.com)\n");
    pump(&ctx, 1200);

    let buffer = editor.view.buffer();
    let color_de = |nombre: &str| -> (u8, u8, u8) {
        let c = buffer
            .tag_table()
            .lookup(nombre)
            .and_then(|t| t.foreground_rgba())
            .expect("color del tag");
        (
            (c.red() * 255.0) as u8,
            (c.green() * 255.0) as u8,
            (c.blue() * 255.0) as u8,
        )
    };
    assert_eq!(
        color_de("link"),
        (138, 90, 43),
        "el enlace toma el acento sepia"
    );
    assert_eq!(
        color_de("h1"),
        (58, 47, 33),
        "el encabezado toma el color sepia"
    );
}
