//! Ventana de preferencias (AdwPreferencesWindow), organizada por páginas
//! según las HIG de GNOME: apariencia, editor y plantillas.

use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use std::rc::Rc;

use crate::settings::{gtk_hides_invisible_safely, AppSettings, FontFamily, MarkupVisibility};
use crate::templates;

fn spin(title: &str, subtitle: &str, adj: &gtk4::Adjustment, digits: u32) -> adw::SpinRow {
    adw::SpinRow::builder()
        .title(title)
        .subtitle(subtitle)
        .adjustment(adj)
        .digits(digits)
        .build()
}

fn combo(title: &str, subtitle: &str, options: &[&str], selected: u32) -> adw::ComboRow {
    adw::ComboRow::builder()
        .title(title)
        .subtitle(subtitle)
        .model(&gtk4::StringList::new(options))
        .selected(selected)
        .build()
}

/// `apply` se invoca tras cada cambio para que la ventana relea los ajustes.
pub fn present(parent: &adw::ApplicationWindow, settings: &Rc<AppSettings>, apply: Rc<dyn Fn()>) {
    let window = adw::PreferencesWindow::builder()
        .transient_for(parent)
        .modal(true)
        .search_enabled(true)
        .build();

    window.add(&appearance_page(settings, &apply));
    window.add(&editor_page(settings, &apply));
    window.add(&templates_page(parent, settings, &apply));
    window.present();
}

fn appearance_page(settings: &Rc<AppSettings>, apply: &Rc<dyn Fn()>) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .title("Apariencia")
        .icon_name("preferences-desktop-appearance-symbolic")
        .build();

    let theme_group = adw::PreferencesGroup::builder().title("Tema").build();
    let theme = combo(
        "Esquema de color",
        "Sigue al sistema salvo que fuerces uno",
        &["Sistema", "Claro", "Oscuro"],
        settings.color_scheme_index(),
    );
    theme_group.add(&theme);
    page.add(&theme_group);

    let text_group = adw::PreferencesGroup::builder()
        .title("Texto")
        .description("El código y las tablas usan siempre monoespaciada")
        .build();

    let family = combo(
        "Familia tipográfica",
        "Cuerpo del documento",
        &["Sans (Cantarell)", "Serif", "Monoespaciada"],
        settings.font_family().index(),
    );
    let size = spin(
        "Tamaño",
        "Píxeles",
        &gtk4::Adjustment::new(settings.font_size() as f64, 9.0, 40.0, 1.0, 2.0, 0.0),
        0,
    );
    let spacing = spin(
        "Interlineado",
        "Multiplicador sobre el tamaño de fuente",
        &gtk4::Adjustment::new(settings.line_spacing(), 1.0, 3.0, 0.1, 0.1, 0.0),
        1,
    );
    let column = spin(
        "Ancho de la columna",
        "Ancho máximo del texto, en píxeles",
        &gtk4::Adjustment::new(
            settings.column_width() as f64,
            480.0,
            1400.0,
            20.0,
            50.0,
            0.0,
        ),
        0,
    );
    for row in [&size, &spacing, &column] {
        text_group.add(row);
    }
    text_group.add(&family);
    page.add(&text_group);

    {
        let settings = settings.clone();
        let apply = apply.clone();
        theme.connect_selected_notify(move |row| {
            settings.set_color_scheme_index(row.selected());
            apply();
        });
    }
    {
        let settings = settings.clone();
        let apply = apply.clone();
        family.connect_selected_notify(move |row| {
            settings.set_font_family(FontFamily::from_index(row.selected()));
            apply();
        });
    }
    {
        let settings = settings.clone();
        let apply = apply.clone();
        size.connect_value_notify(move |row| {
            settings.set_font_size(row.value() as i32);
            apply();
        });
    }
    {
        let settings = settings.clone();
        let apply = apply.clone();
        spacing.connect_value_notify(move |row| {
            settings.set_line_spacing(row.value());
            apply();
        });
    }
    {
        let settings = settings.clone();
        let apply = apply.clone();
        column.connect_value_notify(move |row| {
            settings.set_column_width(row.value() as i32);
            apply();
        });
    }

    page
}

fn editor_page(settings: &Rc<AppSettings>, apply: &Rc<dyn Fn()>) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .title("Editor")
        .icon_name("document-edit-symbolic")
        .build();

    let markup_group = adw::PreferencesGroup::builder()
        .title("Marcado")
        .description("Qué hacer con los asteriscos, almohadillas y URLs mientras escribes")
        .build();
    // Mientras `gtk_hides_invisible_safely()` sea falso (mitigación de
    // GNOME/gtk#8346), «Ocultar» y «Al enfocar» se comportan como «Atenuar»:
    // la UI debe decirlo. Al reactivarse el ocultado con un GTK sano, quitar
    // este aviso y volver a las cadenas y el subtítulo originales.
    let (subtitle, options): (&str, &[&str]) = if gtk_hides_invisible_safely() {
        (
            "«Al enfocar» las revela solo en la línea del cursor",
            &["Ocultar siempre", "Mostrar al enfocar", "Atenuar siempre"],
        )
    } else {
        (
            "Temporalmente las tres atenúan: ocultar está desactivado por un bug de GTK",
            &[
                "Ocultar siempre (temporalmente atenúa)",
                "Mostrar al enfocar (temporalmente atenúa)",
                "Atenuar siempre",
            ],
        )
    };
    let markup = combo(
        "Marcas de Markdown",
        subtitle,
        options,
        settings.markup_visibility().index(),
    );
    markup_group.add(&markup);
    page.add(&markup_group);

    let writing_group = adw::PreferencesGroup::builder().title("Escritura").build();
    let lists = adw::SwitchRow::builder()
        .title("Continuar listas")
        .subtitle("Intro repite el guion o el número; en un elemento vacío, cierra la lista")
        .active(settings.continue_lists())
        .build();
    let focus = adw::SwitchRow::builder()
        .title("Modo foco")
        .subtitle("Atenúa todo salvo el párrafo actual")
        .active(settings.focus_mode())
        .build();
    let typewriter = adw::SwitchRow::builder()
        .title("Máquina de escribir")
        .subtitle("Mantiene la línea del cursor centrada verticalmente")
        .active(settings.typewriter_mode())
        .build();
    let tabs = spin(
        "Ancho de tabulación",
        "Espacios por nivel de sangría",
        &gtk4::Adjustment::new(settings.tab_width() as f64, 2.0, 8.0, 1.0, 1.0, 0.0),
        0,
    );
    for row in [&lists, &focus, &typewriter] {
        writing_group.add(row);
    }
    writing_group.add(&tabs);
    page.add(&writing_group);

    let save_group = adw::PreferencesGroup::builder().title("Guardado").build();
    let autosave = adw::SwitchRow::builder()
        .title("Guardado automático")
        .subtitle("Solo si el documento ya tiene un fichero asociado")
        .active(settings.autosave())
        .build();
    let interval = spin(
        "Intervalo",
        "Segundos entre guardados",
        &gtk4::Adjustment::new(
            settings.autosave_interval() as f64,
            5.0,
            600.0,
            5.0,
            15.0,
            0.0,
        ),
        0,
    );
    interval.set_sensitive(settings.autosave());
    save_group.add(&autosave);
    save_group.add(&interval);
    page.add(&save_group);

    {
        let settings = settings.clone();
        let apply = apply.clone();
        markup.connect_selected_notify(move |row| {
            settings.set_markup_visibility(MarkupVisibility::from_index(row.selected()));
            apply();
        });
    }
    for (row, setter) in [
        (
            &lists,
            Box::new(|s: &AppSettings, v: bool| s.set_continue_lists(v))
                as Box<dyn Fn(&AppSettings, bool)>,
        ),
        (
            &focus,
            Box::new(|s: &AppSettings, v: bool| s.set_focus_mode(v)),
        ),
        (
            &typewriter,
            Box::new(|s: &AppSettings, v: bool| s.set_typewriter_mode(v)),
        ),
    ] {
        let settings = settings.clone();
        let apply = apply.clone();
        row.connect_active_notify(move |row| {
            setter(&settings, row.is_active());
            apply();
        });
    }
    {
        let settings = settings.clone();
        let apply = apply.clone();
        tabs.connect_value_notify(move |row| {
            settings.set_tab_width(row.value() as i32);
            apply();
        });
    }
    {
        let settings = settings.clone();
        let apply = apply.clone();
        let interval_row = interval.clone();
        autosave.connect_active_notify(move |row| {
            settings.set_autosave(row.is_active());
            interval_row.set_sensitive(row.is_active());
            apply();
        });
    }
    {
        let settings = settings.clone();
        let apply = apply.clone();
        interval.connect_value_notify(move |row| {
            settings.set_autosave_interval(row.value() as i32);
            apply();
        });
    }

    page
}

fn templates_page(
    parent: &adw::ApplicationWindow,
    settings: &Rc<AppSettings>,
    apply: &Rc<dyn Fn()>,
) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .title("Plantillas")
        .icon_name("document-new-symbolic")
        .build();

    let list = templates::list();
    let mut names: Vec<&str> = vec!["Documento en blanco"];
    names.extend(list.iter().map(|t| t.name.as_str()));
    let current = settings.default_template();
    let selected = list
        .iter()
        .position(|t| t.name == current)
        .map(|i| i as u32 + 1)
        .unwrap_or(0);

    let group = adw::PreferencesGroup::builder()
        .title("Documentos nuevos")
        .description(
            "Las plantillas son ficheros .md en tu carpeta de datos. \
             Admiten {{title}}, {{date}}, {{time}}, {{datetime}} y {{year}}.",
        )
        .build();

    let default_row = combo(
        "Plantilla por defecto",
        "Se usa al crear un documento nuevo",
        &names,
        selected,
    );
    group.add(&default_row);

    let open_button = gtk4::Button::builder()
        .icon_name("folder-open-symbolic")
        .valign(gtk4::Align::Center)
        .css_classes(vec!["flat".to_string()])
        .tooltip_text("Abrir la carpeta")
        .build();
    let folder_row = adw::ActionRow::builder()
        .title("Carpeta de plantillas")
        .subtitle(templates::dir().to_string_lossy().as_ref())
        .activatable_widget(&open_button)
        .build();
    folder_row.add_suffix(&open_button);
    group.add(&folder_row);
    page.add(&group);

    let recents_group = adw::PreferencesGroup::builder().title("Historial").build();
    let clear_button = gtk4::Button::builder()
        .label("Vaciar")
        .valign(gtk4::Align::Center)
        .css_classes(vec!["destructive-action".to_string()])
        .build();
    let clear_row = adw::ActionRow::builder()
        .title("Documentos recientes")
        .subtitle("Borra la lista que aparece en la barra lateral")
        .activatable_widget(&clear_button)
        .build();
    clear_row.add_suffix(&clear_button);
    recents_group.add(&clear_row);
    page.add(&recents_group);

    {
        let settings = settings.clone();
        let apply = apply.clone();
        default_row.connect_selected_notify(move |row| {
            let index = row.selected() as usize;
            let name = if index == 0 {
                String::new()
            } else {
                templates::list()
                    .get(index - 1)
                    .map(|t| t.name.clone())
                    .unwrap_or_default()
            };
            settings.set_default_template(&name);
            apply();
        });
    }
    {
        let parent = parent.clone();
        open_button.connect_clicked(move |_| templates::open_dir(&parent));
    }
    {
        let settings = settings.clone();
        let apply = apply.clone();
        clear_button.connect_clicked(move |button| {
            settings.clear_recent_files();
            button.set_sensitive(false);
            apply();
        });
    }

    page
}
