/* Scribe landing i18n: ES and EN dictionaries, 1:1 parity.
   Change a text = change both entries at once.
   Table cells: leading "✓", "✗" or "◐" becomes the symbol; "n/a" means no verified data. */

const I18N = {
  en: {
    meta: {
      title: "Scribe: a native Markdown editor for GNOME and Linux",
      description: "Scribe renders Markdown as you type, in one plain-text buffer. Native GTK4 and libadwaita, written in Rust, a 2.6 MB binary, no Electron. Free and open source under AGPL-3.0."
    },
    aria: {
      theme: "Dark theme",
      lang: "Language",
      prev: "Previous screenshot",
      next: "Next screenshot"
    },
    nav: {
      look: "Look inside",
      compare: "Compare",
      install: "Install",
      about: "About"
    },
    hero: {
      h1: "Markdown, without the markup",
      sub: "Native GNOME editor. Type, and it already looks like the page.",
      cta: "Download for Linux",
      flathub: "Flathub, soon"
    },
    cards: {
      inline: {
        title: "One buffer, already rendered",
        text: "The source and the page are the same text. Headings, bold, tables and quotes render in place, over the plain Markdown buffer, with nothing to keep in sync."
      },
      tiny: {
        text: "GTK4, libadwaita and GtkSourceView, written in Rust. No Electron, no WebKit, no hidden browser inside."
      },
      pdf: {
        title: "Real tables in the PDF",
        text: "Export to PDF keeps your tables as tables, with column widths and alignments, plus page breaks with ",
        text2: ". ",
        link: "See how"
      }
    },
    shots: {
      title: "Look inside",
      views: {
        editor: "The editor: inline rendering over the plain buffer",
        split: "Split view: buffer and rendered preview side by side",
        outline: "Outline sidebar over the document structure",
        focus: "Focus mode: one paragraph, nothing else",
        typewriter: "Typewriter scrolling keeps the line at center",
        find: "Search and replace with regex support",
        prefs: "Preferences: font, editor and export pages",
        shortcuts: "Every shortcut, in the app's own cheat sheet"
      }
    },
    compare: {
      title: "Markdown on Linux, with no fine print",
      lead: "Last fact-checked on October 25, 2026, against each project's official website, pricing page and releases. If a piece of data is not in the official source, it is flagged here as unverified instead of being used for comparison.",
      intro: "Every project sums up its approach in one phrase: \"Sharpen your thinking\" (Obsidian, obsidian.md), \"Seamless Live Preview\" (Typora, typora.io), \"Edit Markdown in style\" (Apostrophe, apps.gnome.org/Apostrophe) and \"Next generation markdown editor\" (Mark Text, github.com/marktext/marktext). This table puts the facts behind each phrase: what each editor does, what it does not, and what cannot be verified.",
      legend: "yes, out of the box. yes with caveats (paid, extra dependencies, or limited). no. n/a: no verified official data. No claim is made either way.",
      exportNote: "About tables in PDF: no other project states in its official documentation how tables are rendered on export, so those cells stay unverified. What is verifiable is on the Scribe side: it exports PDF with real tables, in the fix published on October 25, 2026 (issue 138 of its repository).",
      sizeNote: "Sizes measured in October 2026 on each project's official packages. Apostrophe's comes from the Arch Linux package data. Scribe's is re-measured on every publication.",
      not: {
        title: "What Scribe does not do",
        items: [
          "Cloud sync. Your files live on your disk, in plain Markdown.",
          "A plugin system. No store, no extension API.",
          "Math formulas (LaTeX). If you need MathJax or KaTeX, this is not your tool.",
          "Windows, macOS or mobile. Linux only.",
          "Flathub, for now. It is on its way: today installation goes through GitHub Releases, or the community AUR."
        ],
        close: "If you need any of the above, the table above says where to look: Obsidian for plugins and sync, Typora for Windows and macOS, Mark Text for free cross-platform, and Apostrophe for the classic editor-plus-preview flow."
      },
      sources: "Sources: obsidian.md (homepage, license, pricing, sync, downloads and changelog), typora.io (homepage, releases and acknowledgements), apps.gnome.org/Apostrophe, archlinux.org/packages (Apostrophe's size) and github.com/marktext/marktext (releases). All checked on October 25, 2026.",
      blocks: [
        {
          title: "Editing",
          rows: [
            ["Rendering as you type", "◐ Live Preview, hybrid", "✓ Inline WYSIWYG", "✗ Editor and preview in separate panes", "✓ Real-time WYSIWYG", "✓ Inline rendering on the buffer: you type, and the text is already rendered"],
            ["Math formulas", "✓ MathJax 4", "✓ MathJax", "◐ Optional, via texlive", "✓ KaTeX", "✗"]
          ]
        },
        {
          title: "Export",
          rows: [
            ["Export to PDF", "✓ Desktop and mobile", "✓ With bookmarks", "✓ Via pandoc", "✓ Yes", "✓ Yes"],
            ["Tables in the PDF", "n/a", "n/a", "n/a", "n/a", "✓ Real tables"],
            ["Page breaks", "n/a", "n/a", "n/a", "✓ Added in 2026", "✓ With \\newpage"]
          ]
        },
        {
          title: "Ecosystem",
          rows: [
            ["License and price", "Proprietary, free, no account (commercial use included). Optional Commercial plan: $50/user/year", "Proprietary, $14.99 one-time, 3 devices (15-day trial)", "GPL-3.0, free", "MIT, free", "AGPL-3.0, free"],
            ["Cloud sync", "◐ Paid add-on ($4-8/month)", "✗", "✗", "✗", "✗"],
            ["Plugins and themes", "✓ Thousands of plugins and themes", "◐ CSS themes, no plugins", "◐ Themes, no plugins", "◐ Themes, no plugins", "✗ No plugins. Light and dark schemes"],
            ["Status", "Very active: v1.14.5 (October 22, 2026)", "Active: v1.14.10", "Active: v3.5 (October 8, 2026)", "Revived in 2026: v0.23.0 (October 21, 2026), after 4 years without stable releases", "Active: v0.3.20, before 1.0"]
          ]
        },
        {
          title: "Platform",
          rows: [
            ["Technology", "Electron 43", "Electron", "GTK4, libadwaita, GtkSourceView 5, Python", "Electron", "GTK4, libadwaita, GtkSourceView 5, Rust"],
            ["Systems", "Windows, macOS, Linux and mobile", "Windows, macOS and Linux", "Linux only", "Windows, macOS and Linux", "Linux only"],
            ["Package size", "Around 139 MB (AppImage)", "Around 113 MB (deb)", "2.2 MB installed", "Between 106 and 157 MB depending on format", "2.6 MB binary"]
          ]
        }
      ]
    },
    install: {
      title: "Easy, quick installation",
      lead: "Native packages and an AppImage, published on GitHub Releases. No sign-ups, no accounts, no extra repositories to configure.",
      releases: {
        title: "GitHub Releases: deb, rpm and AppImage",
        text: "deb packages for Debian and Ubuntu, rpm for Fedora and openSUSE, and an AppImage that works everywhere else. Exact file names change with each version: copy them from the releases page (github.com/gnacho/scribe/releases)."
      },
      flathub: {
        text: "The Flathub publication is on its way. Once it is live, it will show up here."
      },
      aur: {
        title: "AUR (Arch and derivatives)",
        text: "Two packages maintained by somepaulo, from the community: scribe-bin installs the prebuilt binary from each release, and scribe-git builds from source. They are not official project packages: the credit goes to their maintainer."
      },
      copy: "Copy to clipboard",
      copied: "Copied to clipboard",
      copyFail: "Copy failed, select the text manually",
      soon: "Soon",
      tabsLabel: "Package formats",
      panels: {
        deb: {
          hint: "Debian, Ubuntu and derivatives. Download the file and install it with apt.",
          note: "The package declares its dependencies, so apt resolves them. File name: scribe_<version>-1_amd64.deb."
        },
        rpm: {
          hint: "Fedora, openSUSE and derivatives. Download the file and install it with dnf.",
          note: "Same idea on the rpm side: your package manager resolves GTK4 and libadwaita. File name: scribe-<version>-1.x86_64.rpm."
        },
        appimage: {
          hint: "Any distribution. Download it, make it executable and run it.",
          note: "No install step and no root. File name: Scribe-<version>-x86_64.AppImage, with a capital S in the asset name."
        },
        flathub: {
          hint: "The universal, sandboxed route. Not published yet.",
          note: "It is being prepared. When it is live, it will appear here."
        },
        aur: {
          hint: "Arch and derivatives, from the AUR with scribe-bin.",
          note: "scribe-bin installs the prebuilt binary. scribe-git, which builds from source, is also available. Community packages, not official ones."
        }
      },
      reqs1: {
        title: "What you need",
        text: "A Linux distribution with GTK4 and libadwaita. The deb and rpm packages declare their dependencies and your package manager resolves them. The AppImage runs directly, with no install step."
      },
      reqs2: {
        title: "About the version",
        text: "The current one is v0.3.20, still before 1.0. Scribe is free and open source under AGPL-3.0."
      }
    },
    about: {
      title: "About",
      story: "Scribe started from a specific frustration: I was looking for a Markdown editor that renders the text as I type, with no preview panel and no separate windows, and almost everything that did it came from Electron, well over a hundred megabytes. On Linux I had something better at hand: GTK4, libadwaita and GtkSourceView, the same libraries as the GNOME desktop. So I wrote one in Rust. The result weighs 2.6 MB as a binary and does exactly what I was looking for: you type, and the text is already rendered.",
      project: "It is a personal project, maintained in my spare time, published under AGPL-3.0 and free, always. The current version is 0.3.20: there is still a way to go to 1.0, and things may change between versions.",
      expect: "With contributions or support it could perhaps grow faster, but I cannot promise anything. What is here today: every release published on GitHub, the code open in the repository, and the issues in plain sight.",
      star: "Star on GitHub"
    },
    cta: {
      title: "Try it. It weighs 2.6 MB.",
      sub: "Download the latest release and open a Markdown file. The text renders as you type, and there is nothing else to set up.",
      action: "Download Scribe"
    },
    footer: {
      license: "Free software under AGPL-3.0",
      built: "This page runs no trackers and loads no analytics."
    },
    tableHeaders: ["Feature", "Obsidian", "Typora", "Apostrophe", "Mark Text", "Scribe"]
  },

  es: {
    meta: {
      title: "Scribe: un editor Markdown nativo para GNOME y Linux",
      description: "Scribe renderiza Markdown mientras escribes, en un único buffer de texto plano. GTK4 y libadwaita nativos, escrito en Rust, un binario de 2,6 MB, sin Electron. Libre y de código abierto bajo AGPL-3.0."
    },
    aria: {
      theme: "Tema oscuro",
      lang: "Idioma",
      prev: "Captura anterior",
      next: "Captura siguiente"
    },
    nav: {
      look: "Míralo por dentro",
      compare: "Comparativa",
      install: "Instalación",
      about: "Acerca de"
    },
    hero: {
      h1: "Markdown, sin el marcado",
      sub: "Editor nativo de GNOME. Escribes y el texto ya se ve como página.",
      cta: "Descargar para Linux",
      flathub: "Flathub, en camino"
    },
    cards: {
      inline: {
        title: "Un solo buffer, ya renderizado",
        text: "El fuente y la página son el mismo texto. Títulos, negritas, tablas y citas se renderizan en su sitio, sobre el buffer Markdown plano, sin nada que sincronizar."
      },
      tiny: {
        text: "GTK4, libadwaita y GtkSourceView, escrito en Rust. Sin Electron, sin WebKit, sin navegador escondido dentro."
      },
      pdf: {
        title: "Tablas reales en el PDF",
        text: "La exportación a PDF mantiene tus tablas como tablas, con anchos y alineaciones de columna, y saltos de página con ",
        text2: ". ",
        link: "Ver cómo"
      }
    },
    shots: {
      title: "Míralo por dentro",
      views: {
        editor: "El editor: render inline sobre el buffer plano",
        split: "Vista dividida: buffer y previsualización lado a lado",
        outline: "Panel de esquema sobre la estructura del documento",
        focus: "Modo enfoque: un párrafo, nada más",
        typewriter: "El scroll de máquina de escribir mantiene la línea al centro",
        find: "Búsqueda y reemplazo con soporte de expresiones regulares",
        prefs: "Preferencias: páginas de tipografía, editor y exportación",
        shortcuts: "Todos los atajos, en la chuleta de la propia app"
      }
    },
    compare: {
      title: "Markdown en Linux, sin letra pequeña",
      lead: "Última contrastación: 25 de octubre de 2026, contra las webs oficiales, las páginas de precios y las releases de cada proyecto. Si un dato no está en la fuente oficial, aquí se marca como sin verificar en lugar de usarse para comparar.",
      intro: "Cada proyecto resume su enfoque en una frase: \"Sharpen your thinking\" (Obsidian, obsidian.md), \"Seamless Live Preview\" (Typora, typora.io), \"Edit Markdown in style\" (Apostrophe, apps.gnome.org/Apostrophe) y \"Next generation markdown editor\" (Mark Text, github.com/marktext/marktext). Esta tabla pone los hechos detrás de cada frase: qué hace cada editor, qué no hace y qué no se puede verificar.",
      legend: "sí, de serie. sí con matices (de pago, con dependencias o limitado). no. n/a: sin dato oficial verificado, no se afirma que lo haga ni que no lo haga.",
      exportNote: "Sobre las tablas en PDF: ningún otro proyecto declara en su documentación oficial cómo renderiza las tablas al exportar, así que esas casillas quedan sin verificar. Lo que sí es verificable es de Scribe: exporta PDF con tablas reales, arreglo publicado el 25 de octubre de 2026 (issue 138 de su repositorio).",
      sizeNote: "Pesos medidos en octubre de 2026 sobre los paquetes oficiales de cada proyecto. El de Apostrophe sale de los datos del paquete de Arch Linux. El de Scribe se re-mide en cada publicación.",
      not: {
        title: "Lo que Scribe no hace",
        items: [
          "Sincronización en la nube. Tus archivos viven en tu disco, en Markdown plano.",
          "Sistema de plugins. No hay tienda ni API de extensiones.",
          "Fórmulas matemáticas (LaTeX). Si necesitas MathJax o KaTeX, esta no es tu herramienta.",
          "Windows, macOS o móvil. Solo Linux.",
          "Flathub, de momento. Está en camino: hoy la instalación pasa por GitHub Releases, o por el AUR de la comunidad."
        ],
        close: "Si necesitas alguna de esas cosas, la tabla de arriba dice a quién mirar: Obsidian para plugins y sincronización, Typora para Windows y macOS, Mark Text para multiplataforma libre y Apostrophe para el flujo clásico de editor y vista previa por separado."
      },
      sources: "Fuentes: obsidian.md (portada, licencia, precios, sync, descargas y changelog), typora.io (portada, releases y acknowledgements), apps.gnome.org/Apostrophe, archlinux.org/packages (peso de Apostrophe) y github.com/marktext/marktext (releases). Todo contrastado el 25 de octubre de 2026.",
      blocks: [
        {
          title: "Edición",
          rows: [
            ["Render mientras escribes", "◐ Live Preview, híbrido", "✓ WYSIWYG inline", "✗ Editor y vista previa por separado", "✓ WYSIWYG en tiempo real", "✓ Render inline sobre el buffer: escribes y ya está renderizado"],
            ["Fórmulas matemáticas", "✓ MathJax 4", "✓ MathJax", "◐ Opcional, con texlive", "✓ KaTeX", "✗"]
          ]
        },
        {
          title: "Exportación",
          rows: [
            ["Exportar a PDF", "✓ Escritorio y móvil", "✓ Con marcadores", "✓ Vía pandoc", "✓ Sí", "✓ Sí"],
            ["Tablas en el PDF", "n/a", "n/a", "n/a", "n/a", "✓ Tablas reales"],
            ["Saltos de página", "n/a", "n/a", "n/a", "✓ Añadido en 2026", "✓ Con \\newpage"]
          ]
        },
        {
          title: "Ecosistema",
          rows: [
            ["Licencia y precio", "Propietaria, gratis y sin cuenta (uso comercial incluido). Commercial opcional: 50 $/usuario/año", "Propietaria, 14,99 $ pago único, 3 dispositivos (prueba de 15 días)", "GPL-3.0, gratis", "MIT, gratis", "AGPL-3.0, gratis"],
            ["Sincronización en la nube", "◐ Add-on de pago (4-8 $/mes)", "✗", "✗", "✗", "✗"],
            ["Plugins y temas", "✓ Miles de plugins y temas", "◐ Temas CSS, sin plugins", "◐ Temas, sin plugins", "◐ Temas, sin plugins", "✗ Sin plugins. Esquemas claro y oscuro"],
            ["Estado", "Muy activo: v1.14.5 (22 de octubre de 2026)", "Activo: v1.14.10", "Activo: v3.5 (8 de octubre de 2026)", "Reactivado en 2026: v0.23.0 (21 de octubre de 2026), tras 4 años sin versiones estables", "Activo: v0.3.20, antes de la 1.0"]
          ]
        },
        {
          title: "Plataforma",
          rows: [
            ["Tecnología", "Electron 43", "Electron", "GTK4, libadwaita, GtkSourceView 5, Python", "Electron", "GTK4, libadwaita, GtkSourceView 5, Rust"],
            ["Sistemas", "Windows, macOS, Linux y móvil", "Windows, macOS y Linux", "Solo Linux", "Windows, macOS y Linux", "Solo Linux"],
            ["Peso del paquete", "En torno a 139 MB (AppImage)", "En torno a 113 MB (deb)", "2,2 MB instalado", "Entre 106 y 157 MB según formato", "2,6 MB binario"]
          ]
        }
      ]
    },
    install: {
      title: "Instalación fácil y rápida",
      lead: "Paquetes nativos y un AppImage, publicados en GitHub Releases. Sin registros, sin cuentas y sin configurar repositorios.",
      releases: {
        title: "GitHub Releases: deb, rpm y AppImage",
        text: "Paquetes deb para Debian y Ubuntu, rpm para Fedora y openSUSE, y un AppImage que funciona en el resto. Los nombres exactos de archivo cambian con cada versión: cópialos de la página de releases (github.com/gnacho/scribe/releases)."
      },
      flathub: {
        text: "La publicación en Flathub está en camino. Cuando esté lista, aparecerá aquí."
      },
      aur: {
        title: "AUR (Arch y derivadas)",
        text: "Dos paquetes mantenidos por somepaulo, de la comunidad: scribe-bin instala el binario de cada release y scribe-git compila desde el código fuente. No son paquetes oficiales del proyecto: el agradecimiento es para su mantenedor."
      },
      copy: "Copiar al portapapeles",
      copied: "Copiado al portapapeles",
      copyFail: "No se pudo copiar, selecciona el texto a mano",
      soon: "Pronto",
      tabsLabel: "Formatos de paquete",
      panels: {
        deb: {
          hint: "Debian, Ubuntu y derivadas. Descarga el archivo e instálalo con apt.",
          note: "El paquete declara sus dependencias y apt las resuelve. Nombre del archivo: scribe_<version>-1_amd64.deb."
        },
        rpm: {
          hint: "Fedora, openSUSE y derivadas. Descarga el archivo e instálalo con dnf.",
          note: "La misma idea en rpm: tu gestor de paquetes resuelve GTK4 y libadwaita. Nombre del archivo: scribe-<version>-1.x86_64.rpm."
        },
        appimage: {
          hint: "Cualquier distribución. Descárgalo, dale permiso de ejecución y arranca.",
          note: "Sin instalación y sin root. Nombre del archivo: Scribe-<version>-x86_64.AppImage, con S mayúscula en el asset."
        },
        flathub: {
          hint: "La vía universal y en sandbox. Aún no está publicada.",
          note: "Se está preparando. Cuando esté lista aparecerá aquí."
        },
        aur: {
          hint: "Arch y derivadas, desde el AUR con scribe-bin.",
          note: "scribe-bin instala el binario ya compilado. scribe-git, que compila desde el código, también está disponible. Paquetes de la comunidad, no oficiales."
        }
      },
      reqs1: {
        title: "Lo que necesitas",
        text: "Una distribución Linux con GTK4 y libadwaita. Los paquetes deb y rpm declaran sus dependencias y las resuelve tu gestor de paquetes. El AppImage arranca directamente, sin paso de instalación."
      },
      reqs2: {
        title: "Sobre la versión",
        text: "La actual es la v0.3.20, todavía antes de la 1.0. Scribe es gratuito y de código abierto bajo AGPL-3.0."
      }
    },
    about: {
      title: "Acerca de",
      story: "Scribe nació de una frustración concreta: buscaba un editor Markdown que renderizara el texto mientras escribo, sin panel de vista previa ni ventanas aparte, y casi todo lo que lo hacía venía de Electron, por encima de los cien megabytes. En Linux tenía a mano algo mejor: GTK4, libadwaita y GtkSourceView, las mismas bibliotecas del escritorio GNOME. Así que lo escribí en Rust. El resultado pesa 2,6 MB como binario y hace justo lo que buscaba: escribes y el texto ya está renderizado.",
      project: "Es un proyecto personal, mantenido en mi tiempo libre, publicado bajo AGPL-3.0 y gratuito siempre. La versión actual es la 0.3.20: falta para la 1.0 y puede haber cambios entre versiones.",
      expect: "Con colaboraciones o apoyo quizás podría crecer más rápido, pero no puedo prometer nada. Lo que hay hoy: cada release publicada en GitHub, el código abierto en el repositorio y las incidencias a la vista de cualquiera.",
      star: "Da una estrella en GitHub"
    },
    cta: {
      title: "Pruébalo. Pesa 2,6 MB.",
      sub: "Descarga la última versión y abre un fichero Markdown. El texto se renderiza mientras escribes y no hay nada más que configurar.",
      action: "Descargar Scribe"
    },
    footer: {
      license: "Software libre bajo AGPL-3.0",
      built: "Esta página no carga rastreadores ni analíticas."
    },
    tableHeaders: ["Funcionalidad", "Obsidian", "Typora", "Apostrophe", "Mark Text", "Scribe"]
  }
};
