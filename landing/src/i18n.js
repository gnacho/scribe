const urlLanguage = new URLSearchParams(location.search).get('lang');
export let currentLanguage = urlLanguage === 'en' || urlLanguage === 'es'
  ? urlLanguage
  : localStorage.getItem('scribe-language') === 'en' ? 'en' : 'es';

export const en = {
  'Scribe — Escribe. Ya está maquetado.': 'Scribe — Write. It’s already typeset.',
  'Descargar': 'Download',
  'Descargar para Linux': 'Download for Linux',
  'Cómo funciona': 'How it works',
  'Funciones': 'Features',
  'Instalar': 'Install',
  'Historia': 'Story',
  'Editor Markdown nativo para Linux': 'Native Markdown editor for Linux',
  'Escribe.': 'Write.',
  'Ya está': 'It’s already',
  'maquetado.': 'typeset.',
  'Sin panel de vista previa. Sin cambiar de modo. Scribe convierte el Markdown en una página mientras escribes, justo donde estás escribiendo.': 'No preview pane. No switching modes. Scribe turns Markdown into a page as you type, right where you are typing.',
  'Ver cómo funciona': 'See how it works',
  'Gratis, siempre': 'Free, forever',
  'Abrir': 'Open',
  'Cuaderno de campo': 'Field journal',
  'Borrador': 'Draft',
  '27 OCTUBRE 2026 \u00a0 / \u00a0 NOTAS': '27 OCTOBER 2026 \u00a0 / \u00a0 NOTES',
  'La niebla todavía no se ha levantado. Escribo despacio,': 'The fog has not lifted yet. I write slowly,',
  'mientras el texto encuentra su forma': 'as the text finds its shape',
  'El sendero': 'The trail',
  'Dos cosas que merece la pena recordar:': 'Two things worth remembering:',
  'El camino vuelve a estar señalizado.': 'The trail is marked again.',
  'Traer guantes la próxima vez.': 'Bring gloves next time.',
  'La claridad no es añadir más.': 'Clarity is not adding more.',
  'Es quitar todo lo que estorba.': 'It is removing everything in the way.',
  '142 palabras': '142 words',
  'Nativo': 'Native',
  'El código y la página son el mismo texto.': 'The source and the page are the same text.',
  '↻ \u00a0Repetir escritura': '↻ \u00a0Replay typing',
  'AGPL-3': 'AGPL-3',
  'Software libre': 'Free software',
  'Telemetría': 'Telemetry',
  'Gratis, ahora y siempre': 'Free, now and forever',
  'Hecho con Rust, GTK4 y libadwaita.': 'Built with Rust, GTK4 and libadwaita.',
  'Como debe sentirse una app de Linux.': 'How a Linux app should feel.',
  'La idea': 'The idea',
  'Un solo texto.': 'One text.',
  'Ninguna fricción.': 'No friction.',
  'La mayoría de editores te hacen elegir entre mirar el código o mirar el resultado. Scribe no.': 'Most editors make you choose between looking at the source or the result. Scribe does not.',
  'Los títulos, la negrita, las citas y las tablas se renderizan sobre el propio buffer de Markdown. No hay dos paneles que sincronizar ni una vista previa que perseguir con los ojos.': 'Headings, bold text, quotes and tables render over the Markdown buffer itself. There are no two panes to sync and no preview to chase with your eyes.',
  'Escribes en Markdown. Lees una página. Todo ocurre en el mismo lugar.': 'You write in Markdown. You read a page. It all happens in the same place.',
  'Menos interfaz': 'Less interface',
  'Funcionalidades': 'Features',
  'Mucho más que': 'Much more than',
  'Markdown bonito.': 'pretty Markdown.',
  'Scribe está lleno de pequeñas decisiones que hacen más cómodo escribir a diario. Explóralas con capturas reales de la aplicación.': 'Scribe is full of small decisions that make everyday writing more comfortable. Explore them through real application screenshots.',
  'Concentración': 'Focus',
  'Markdown en vivo': 'Live Markdown',
  'Buscar y reemplazar': 'Find and replace',
  'Vista dividida': 'Split view',
  'Captura real de Scribe': 'Real Scribe screenshot',
  'TABLAS': 'TABLES',
  'Tablas de verdad': 'Real tables',
  'Columnas alineadas, celdas que ajustan su texto y una fila editable que vuelve al Markdown justo cuando la necesitas.': 'Aligned columns, cells that wrap their text and an editable row that returns to Markdown right when you need it.',
  'IMÁGENES': 'IMAGES',
  'Arrastra y sigue': 'Drop and keep writing',
  'Pega o suelta una imagen: Scribe la guarda junto al documento, crea el enlace relativo y la muestra en su sitio.': 'Paste or drop an image: Scribe saves it next to the document, creates the relative link and displays it in place.',
  'DOCUMENTOS': 'DOCUMENTS',
  'Tu trabajo, protegido': 'Your work, protected',
  'Autoguardado configurable, escritura atómica y recuperación de borradores tras un cierre inesperado.': 'Configurable autosave, atomic writes and draft recovery after an unexpected shutdown.',
  'ORGANIZACIÓN': 'ORGANIZATION',
  'Todo a mano': 'Everything within reach',
  'Pestañas multidocumento, archivos recientes e índice navegable en la barra lateral. Sin cambiar de aplicación.': 'Multi-document tabs, recent files and a navigable outline in the sidebar. Without switching applications.',
  'DETALLES': 'DETAILS',
  'Pequeñas ayudas': 'Little helpers',
  'Tareas marcables, listas que continúan solas, enlaces con Ctrl+clic, tarjetas de YouTube y atajos de formato.': 'Clickable tasks, automatic list continuation, Ctrl+click links, YouTube cards and formatting shortcuts.',
  'EXPORTACIÓN': 'EXPORT',
  'Un PDF a tu manera': 'A PDF your way',
  'A4 o Letter, vertical u horizontal, fuentes y colores independientes. Incluso un modo monocromo para imprimir.': 'A4 or Letter, portrait or landscape, independent fonts and colors. Even a monochrome mode for printing.',
  'Pequeño por fuera. Serio por dentro.': 'Small outside. Serious inside.',
  'Todo lo necesario.': 'Everything you need.',
  'Nada que sobre.': 'Nothing you don’t.',
  'Nativo de verdad': 'Truly native',
  'Scribe está escrito en Rust y usa GTK4, libadwaita y GtkSourceView. Sin Electron, sin WebKit, sin un navegador disfrazado de editor.': 'Scribe is written in Rust and uses GTK4, libadwaita and GtkSourceView. No Electron, no WebKit, no browser disguised as an editor.',
  'PDF que respeta tu trabajo': 'A PDF that respects your work',
  'Las tablas siguen siendo tablas, con sus anchos y alineaciones. Añade saltos de página con': 'Tables remain tables, with their widths and alignments. Add page breaks with',
  'y exporta un documento listo para compartir.': 'and export a document ready to share.',
  'Tus archivos siguen siendo tuyos': 'Your files remain yours',
  'Markdown plano, en tu disco. Sin cuenta, sin nube obligatoria, sin un formato propietario del que depender mañana.': 'Plain Markdown, on your disk. No account, no mandatory cloud, no proprietary format to depend on tomorrow.',
  'Y lo que no hace.': 'And what it does not do.',
  'No sincroniza con la nube. No tiene plugins. Scribe prefiere hacer una cosa bien antes que prometer hacerlo todo.': 'No cloud sync. No plugins. Scribe would rather do one thing well than promise to do everything.',
  'Instalación': 'Installation',
  'De cero a escribir': 'From zero to writing',
  'en un minuto.': 'in one minute.',
  'Elige tu formato. Sin configuración inicial y sin cuenta.': 'Choose your format. No initial setup and no account.',
  'Descargar última versión': 'Download latest release',
  'Por qué existe': 'Why it exists',
  'Buscaba un editor que renderizara el texto mientras escribía. En Linux tenía todas las piezas para hacerlo mejor. Así que lo escribí.': 'I wanted an editor that rendered text as I typed. On Linux I had all the pieces to make it better. So I wrote it.',
  'Scribe es un proyecto personal de Nacho, mantenido en su tiempo libre. El código, las versiones y los problemas pendientes están a la vista. Es software libre hoy y seguirá siéndolo mañana.': 'Scribe is Nacho’s personal project, maintained in his spare time. The code, releases and open issues are all in plain sight. It is free software today and will remain so tomorrow.',
  'v0.3.20 · camino a 1.0': 'v0.3.20 · road to 1.0',
  'Ver issues abiertos ↗': 'View open issues ↗',
  'Código abierto, de verdad': 'Open source, for real',
  'Prueba Scribe.': 'Try Scribe.',
  '100% software libre.': '100% free software.',
  'AGPL-3. Gratis, ahora y siempre.': 'AGPL-3. Free, now and forever.',
  'Sin telemetría, sin cuentas y sin nube obligatoria.': 'No telemetry, no accounts and no mandatory cloud.',
  'Explorar el código': 'Explore the code',
  'Explora el código ↗': 'Explore the code ↗',
  'Danos una estrella': 'Give us a star',
  '¿Te gusta Scribe? Dale una estrella en GitHub ★': 'Like Scribe? Give it a star on GitHub ★',
  'Markdown nativo para GNOME y Linux.': 'Native Markdown for GNOME and Linux.',
  'Analítica sin cookies, sin anuncios ni perfiles.': 'Cookieless analytics. No ads, no profiles.',
  'Copiar': 'Copy',
  'MENOS EDITOR.': 'LESS EDITOR.',
  'MÁS TEXTO.': 'MORE TEXT.',
  '# Una idea clara': '# A clear idea',
  '**Sin distracciones**': '**No distractions**',
  '> Menos interfaz': '> Less interface',
  'Una idea clara': 'A clear idea',
  'Sin distracciones': 'No distractions',
  'Comandos copiados': 'Commands copied'
};

const originalText = new WeakMap();
const originalAttributes = new WeakMap();

export function t(spanish) {
  return currentLanguage === 'en' ? (en[spanish] || spanish) : spanish;
}

function translatedValue(original) {
  const leading = original.match(/^\s*/)[0];
  const trailing = original.match(/\s*$/)[0];
  const core = original.trim();
  return leading + t(core) + trailing;
}

export function translatePage() {
  document.documentElement.lang = currentLanguage;
  document.title = t('Scribe — Escribe. Ya está maquetado.');
  const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, {
    acceptNode(node) {
      if (!node.nodeValue.trim()) return NodeFilter.FILTER_REJECT;
      if (node.parentElement.closest('script, style, pre, [data-i18n-skip], #typed-text')) return NodeFilter.FILTER_REJECT;
      return NodeFilter.FILTER_ACCEPT;
    }
  });
  const nodes = [];
  while (walker.nextNode()) nodes.push(walker.currentNode);
  nodes.forEach(node => {
    if (!originalText.has(node)) originalText.set(node, node.nodeValue);
    node.nodeValue = translatedValue(originalText.get(node));
  });

  document.querySelectorAll('[aria-label], [title], [alt]').forEach(element => {
    if (element.closest('[data-i18n-skip]')) return;
    if (!originalAttributes.has(element)) originalAttributes.set(element, {});
    const originals = originalAttributes.get(element);
    ['aria-label', 'title', 'alt'].forEach(attribute => {
      if (!element.hasAttribute(attribute)) return;
      if (!(attribute in originals)) originals[attribute] = element.getAttribute(attribute);
      element.setAttribute(attribute, t(originals[attribute]));
    });
  });

  const languageButton = document.querySelector('.language-toggle');
  languageButton.textContent = currentLanguage === 'es' ? 'EN' : 'ES';
  languageButton.setAttribute('aria-label', currentLanguage === 'es' ? 'Switch to English' : 'Cambiar a español');
  languageButton.setAttribute('title', currentLanguage === 'es' ? 'Switch to English' : 'Cambiar a español');
  const description = document.querySelector('meta[name="description"]');
  description.content = currentLanguage === 'es'
    ? 'Scribe es un editor Markdown nativo para Linux. Software libre AGPL-3, sin telemetría, cuentas ni nube obligatoria.'
    : 'Scribe is a native Markdown editor for Linux. AGPL-3 free software, with no telemetry, accounts or mandatory cloud.';
}

export function setLanguage(language) {
  currentLanguage = language;
  localStorage.setItem('scribe-language', language);
  translatePage();
}

export const attributeEnglish = {
  'Abrir menú': 'Open menu',
  'Cerrar menú': 'Close menu',
  'Activar tema oscuro': 'Switch to dark theme',
  'Activar tema claro': 'Switch to light theme',
  'Cambiar tema': 'Change theme',
  'Abrir documento de demostración': 'Open demo document',
  'Reproducir demostración de escritura': 'Replay typing demo',
  'Copiar comandos de instalación': 'Copy installation commands',
  'Demostración visual del editor Scribe': 'Visual demo of the Scribe editor',
  'Datos principales': 'Key facts',
  'Ejemplos de formato Markdown': 'Markdown formatting examples',
  'Explorar funcionalidades': 'Explore features',
  'Formatos de instalación': 'Installation formats',
  'Navegación principal': 'Main navigation',
  'Scribe, inicio': 'Scribe, home'
};
Object.assign(en, attributeEnglish);
