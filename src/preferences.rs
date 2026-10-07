//! Ventana de preferencias (AdwPreferencesWindow), organizada por páginas
//! según las HIG de GNOME: apariencia, editor y exportación.

use gettextrs::gettext;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use std::rc::Rc;

use crate::settings::{
    gtk_hides_invisible_safely, AppSettings, ExportColorScheme, ExportMonoFont, ExportOrientation,
    ExportPageSize, FontFamily, MarkupVisibility,
};

fn spin(title: &str, subtitle: &str, adj: &gtk4::Adjustment, digits: u32) -> adw::SpinRow {
    adw::SpinRow::builder()
        .title(title)
        .subtitle(subtitle)
        .adjustment(adj)
        .digits(digits)
        .build()
}

fn combo_i18n(title: &str, subtitle: &str, options: &[String], selected: u32) -> adw::ComboRow {
    let refs: Vec<&str> = options.iter().map(String::as_str).collect();
    adw::ComboRow::builder()
        .title(title)
        .subtitle(subtitle)
        .model(&gtk4::StringList::new(&refs))
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
    window.add(&export_page(settings, &apply));
    window.present();
}

fn appearance_page(settings: &Rc<AppSettings>, apply: &Rc<dyn Fn()>) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .title(gettext("Appearance"))
        .icon_name("preferences-desktop-appearance-symbolic")
        .build();

    let theme_group = adw::PreferencesGroup::builder()
        .title(gettext("Theme"))
        .build();
    let theme = combo_i18n(
        &gettext("Color scheme"),
        &gettext("Follows the system unless you force one"),
        &[gettext("System"), gettext("Light"), gettext("Dark")],
        settings.color_scheme_index(),
    );
    theme_group.add(&theme);
    page.add(&theme_group);

    let text_group = adw::PreferencesGroup::builder()
        .title(gettext("Text"))
        .description(gettext("Code and tables always use a monospaced font"))
        .build();

    let family = combo_i18n(
        &gettext("Font family"),
        &gettext("Document body"),
        &[
            gettext("Sans (Cantarell)"),
            gettext("Serif"),
            gettext("Monospace"),
        ],
        settings.font_family().index(),
    );
    let size = spin(
        &gettext("Size"),
        &gettext("Pixels"),
        &gtk4::Adjustment::new(settings.font_size() as f64, 9.0, 40.0, 1.0, 2.0, 0.0),
        0,
    );
    let spacing = spin(
        &gettext("Line spacing"),
        &gettext("Multiplier on the font size"),
        &gtk4::Adjustment::new(settings.line_spacing(), 1.0, 3.0, 0.1, 0.1, 0.0),
        1,
    );
    let column = spin(
        &gettext("Column width"),
        &gettext("Maximum text width, in pixels"),
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
        .title(gettext("Editor"))
        .icon_name("document-edit-symbolic")
        .build();

    let markup_group = adw::PreferencesGroup::builder()
        .title(gettext("Markup"))
        .description(gettext(
            "What to do with asterisks, hash marks and URLs while you write",
        ))
        .build();
    // Mientras `gtk_hides_invisible_safely()` sea falso (mitigación de
    // GNOME/gtk#8346), «Ocultar» y «Al enfocar» no ocultan de verdad: las
    // marcas sustituidas se encogen (tag `syn_shrink`) y las demás se
    // atenúan, y «Al enfocar» equivale a «Ocultar» porque ya no hay revelado
    // por línea. Con un GTK sano, sin sufijo y ocultado real.
    let markup = if gtk_hides_invisible_safely() {
        combo_i18n(
            &gettext("Markdown marks"),
            &gettext("“On focus” reveals them only on the cursor line"),
            &[
                gettext("Always hide"),
                gettext("Show on focus"),
                gettext("Always dim"),
            ],
            settings.markup_visibility().index(),
        )
    } else {
        combo_i18n(
            &gettext("Markdown marks"),
            &gettext("Hiding text still crashes this GTK: the marks shrink instead"),
            &[
                gettext("Always hide (shrinks the marks; GTK cannot hide them yet)"),
                gettext("Show on focus (shrinks the marks; GTK cannot hide them yet)"),
                gettext("Always dim"),
            ],
            settings.markup_visibility().index(),
        )
    };
    markup_group.add(&markup);
    page.add(&markup_group);

    let writing_group = adw::PreferencesGroup::builder()
        .title(gettext("Writing"))
        .build();
    let lists = adw::SwitchRow::builder()
        .title(gettext("Continue lists"))
        .subtitle(gettext(
            "Enter repeats the dash or number; on an empty item, it closes the list",
        ))
        .active(settings.continue_lists())
        .build();
    let focus = adw::SwitchRow::builder()
        .title(gettext("Focus mode"))
        .subtitle(gettext("Dims everything except the current paragraph"))
        .active(settings.focus_mode())
        .build();
    let typewriter = adw::SwitchRow::builder()
        .title(gettext("Typewriter mode"))
        .subtitle(gettext("Keeps the cursor line vertically centered"))
        .active(settings.typewriter_mode())
        .build();
    let tabs = spin(
        &gettext("Tab width"),
        &gettext("Spaces per indent level"),
        &gtk4::Adjustment::new(settings.tab_width() as f64, 2.0, 8.0, 1.0, 1.0, 0.0),
        0,
    );
    for row in [&lists, &focus, &typewriter] {
        writing_group.add(row);
    }
    writing_group.add(&tabs);
    page.add(&writing_group);

    let save_group = adw::PreferencesGroup::builder()
        .title(gettext("Saving"))
        .build();
    let autosave = adw::SwitchRow::builder()
        .title(gettext("Autosave"))
        .subtitle(gettext("Only when the document already has a file"))
        .active(settings.autosave())
        .build();
    let interval = spin(
        &gettext("Interval"),
        &gettext("Seconds between saves"),
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

    let history_group = adw::PreferencesGroup::builder()
        .title(gettext("History"))
        .build();
    let clear_button = gtk4::Button::builder()
        .label(gettext("Clear"))
        .valign(gtk4::Align::Center)
        .css_classes(vec!["destructive-action".to_string()])
        .build();
    let clear_row = adw::ActionRow::builder()
        .title(gettext("Recent documents"))
        .subtitle(gettext("Clears the list shown in the sidebar"))
        .activatable_widget(&clear_button)
        .build();
    clear_row.add_suffix(&clear_button);
    history_group.add(&clear_row);
    page.add(&history_group);

    {
        let settings = settings.clone();
        let apply = apply.clone();
        clear_button.connect_clicked(move |button| {
            settings.clear_recent_files();
            button.set_sensitive(false);
            apply();
        });
    }

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

fn export_page(settings: &Rc<AppSettings>, apply: &Rc<dyn Fn()>) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .title(gettext("Export"))
        .icon_name("document-print-symbolic")
        .build();

    let group = adw::PreferencesGroup::builder()
        .title(gettext("PDF"))
        .description(gettext(
            "These settings apply to PDF export only, not to the editor view",
        ))
        .build();

    let scheme = combo_i18n(
        &gettext("Color scheme"),
        &gettext("Monochrome prints everything in black on white"),
        &[
            gettext("System"),
            gettext("Light"),
            gettext("Dark"),
            gettext("Monochrome"),
        ],
        settings.export_color_scheme().index(),
    );
    let family = combo_i18n(
        &gettext("Font family"),
        &gettext("Document body"),
        &[
            gettext("Sans (Cantarell)"),
            gettext("Serif"),
            gettext("Monospace"),
        ],
        settings.export_font_family().index(),
    );
    let size = spin(
        &gettext("Font size"),
        &gettext("Points"),
        &gtk4::Adjustment::new(settings.export_font_size() as f64, 8.0, 24.0, 1.0, 2.0, 0.0),
        0,
    );
    let mono = combo_i18n(
        &gettext("Monospace font"),
        &gettext("Used for code blocks and tables"),
        &[
            "Adwaita Mono".to_string(),
            "Source Code Pro".to_string(),
            "Fira Mono".to_string(),
        ],
        settings.export_mono_font().index(),
    );
    let page_size = combo_i18n(
        &gettext("Page size"),
        &gettext("Paper"),
        &["A4".to_string(), "Letter".to_string()],
        settings.export_page_size().index(),
    );
    let orientation = combo_i18n(
        &gettext("Orientation"),
        &gettext("Paper orientation"),
        &[gettext("Portrait"), gettext("Landscape")],
        settings.export_orientation().index(),
    );

    for row in [&scheme, &family, &mono, &page_size, &orientation] {
        group.add(row);
    }
    group.add(&size);
    page.add(&group);

    {
        let settings = settings.clone();
        let apply = apply.clone();
        scheme.connect_selected_notify(move |row| {
            settings.set_export_color_scheme(ExportColorScheme::from_index(row.selected()));
            apply();
        });
    }
    {
        let settings = settings.clone();
        let apply = apply.clone();
        family.connect_selected_notify(move |row| {
            settings.set_export_font_family(FontFamily::from_index(row.selected()));
            apply();
        });
    }
    {
        let settings = settings.clone();
        let apply = apply.clone();
        size.connect_value_notify(move |row| {
            settings.set_export_font_size(row.value() as i32);
            apply();
        });
    }
    {
        let settings = settings.clone();
        let apply = apply.clone();
        mono.connect_selected_notify(move |row| {
            settings.set_export_mono_font(ExportMonoFont::from_index(row.selected()));
            apply();
        });
    }
    {
        let settings = settings.clone();
        let apply = apply.clone();
        page_size.connect_selected_notify(move |row| {
            settings.set_export_page_size(ExportPageSize::from_index(row.selected()));
            apply();
        });
    }
    {
        let settings = settings.clone();
        let apply = apply.clone();
        orientation.connect_selected_notify(move |row| {
            settings.set_export_orientation(ExportOrientation::from_index(row.selected()));
            apply();
        });
    }

    page
}
