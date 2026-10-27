import { currentLanguage, setLanguage, t, translatePage } from './i18n.js?v=20261027a';

const root = document.documentElement;
const themeButton = document.querySelector('.theme-toggle');
const savedTheme = localStorage.getItem('scribe-theme');
const preferredDark = window.matchMedia('(prefers-color-scheme: dark)').matches;

function setTheme(theme) {
  root.dataset.theme = theme;
  const dark = theme === 'dark';
  themeButton.setAttribute('aria-label', t(dark ? 'Activar tema claro' : 'Activar tema oscuro'));
  themeButton.setAttribute('title', t('Cambiar tema'));
  localStorage.setItem('scribe-theme', theme);
}

setTheme(savedTheme || (preferredDark ? 'dark' : 'light'));
themeButton.addEventListener('click', () => setTheme(root.dataset.theme === 'dark' ? 'light' : 'dark'));

const menuButton = document.querySelector('.menu-toggle');
const nav = document.querySelector('.main-nav');
menuButton.addEventListener('click', () => {
  const open = menuButton.getAttribute('aria-expanded') !== 'true';
  menuButton.setAttribute('aria-expanded', String(open));
  menuButton.setAttribute('aria-label', t(open ? 'Cerrar menú' : 'Abrir menú'));
  nav.classList.toggle('open', open);
});
nav.querySelectorAll('a').forEach(link => link.addEventListener('click', () => {
  nav.classList.remove('open');
  menuButton.setAttribute('aria-expanded', 'false');
  menuButton.setAttribute('aria-label', t('Abrir menú'));
}));

const featureContent = {
  focus: {
    image: '/assets/modo-foco.png',
    alt: 'Modo foco de Scribe con el párrafo actual resaltado y el resto atenuado',
    kicker: 'Modo foco + máquina de escribir',
    title: 'La interfaz desaparece.<br>El texto se queda.',
    copy: 'Atenúa todo salvo el párrafo actual y mantén el cursor centrado verticalmente. Ajusta la fuente, el interlineado y el ancho de columna hasta encontrar tu ritmo.',
    list: ['Modo foco con Ctrl+Shift+F', 'Cursor centrado con Ctrl+Shift+T', 'Cinco temas y corrector ortográfico'],
    en: {
      image: '/assets/modo-foco-en.png',
      alt: 'Scribe focus mode with the current paragraph highlighted and the rest dimmed',
      kicker: 'Focus mode + typewriter mode',
      title: 'The interface disappears.<br>Your words remain.',
      copy: 'Dim everything except the current paragraph and keep the cursor vertically centered. Adjust the font, line spacing and column width until you find your rhythm.',
      list: ['Focus mode with Ctrl+Shift+F', 'Centered cursor with Ctrl+Shift+T', 'Five themes and spell checking']
    }
  },
  markdown: {
    image: '/assets/markdown-en-vivo.png',
    alt: 'Documento Markdown renderizado en Scribe con tablas, listas, citas e imagen',
    kicker: 'Render sobre el propio texto',
    title: 'Markdown que cobra vida.<br>Sin salir del documento.',
    copy: 'Cabeceras, tablas, tareas, código, citas e imágenes se ven como documento mientras escribes. El archivo sigue siendo Markdown plano, sin HTML ni un navegador oculto.',
    list: ['Casillas de tarea que puedes marcar', 'Imágenes pegadas o arrastradas', 'Listas que continúan y se renumeran'],
    en: {
      image: '/assets/markdown-en-vivo-en.png',
      alt: 'Markdown document rendered in Scribe with tables, lists, quotes and an image',
      kicker: 'Rendering over the text itself',
      title: 'Markdown comes alive.<br>Without leaving the document.',
      copy: 'Headings, tables, tasks, code, quotes and images look like a document as you type. The file remains plain Markdown, with no HTML or hidden browser.',
      list: ['Clickable task checkboxes', 'Pasted or dropped images', 'Lists that continue and renumber automatically']
    }
  },
  find: {
    image: '/assets/buscar-reemplazar.png',
    alt: 'Búsqueda integrada en Scribe con coincidencias resaltadas',
    kicker: 'Buscar + reemplazar',
    title: 'Encuentra cualquier cosa.<br>Sin romper tu ritmo.',
    copy: 'Busca dentro del documento activo, salta entre coincidencias y despliega el reemplazo solo cuando lo necesites. Todo ocurre dentro del editor, sin diálogos que te saquen del texto.',
    list: ['Contador y navegación entre resultados', 'Mayúsculas y expresiones regulares', 'Buscar con Ctrl+F y reemplazar con Ctrl+H'],
    en: {
      image: '/assets/buscar-reemplazar-en.png',
      alt: 'Built-in search in Scribe with highlighted matches',
      kicker: 'Find + replace',
      title: 'Find anything.<br>Without breaking your flow.',
      copy: 'Search within the active document, jump between matches and expand replace only when you need it. Everything happens inside the editor, with no dialogs pulling you away from the text.',
      list: ['Match counter and navigation', 'Case sensitivity and regular expressions', 'Find with Ctrl+F and replace with Ctrl+H']
    }
  },
  split: {
    image: '/assets/vista-dividida.png',
    alt: 'Scribe con el editor y la vista dividida a la derecha',
    kicker: 'Vista dividida opcional',
    title: 'Una segunda mirada.<br>Solo cuando la pides.',
    copy: 'La esencia de Scribe es una sola superficie, pero también puedes abrir una vista completa a la derecha para revisar el resultado mientras escribes. Usa el mismo motor de renderizado.',
    list: ['Se abre con Ctrl+Shift+P', 'El mismo motor que el editor', 'Barra lateral con recientes e índice'],
    en: {
      image: '/assets/vista-dividida-en.png',
      alt: 'Scribe with the editor and a split view on the right',
      kicker: 'Optional split view',
      title: 'A second look.<br>Only when you ask.',
      copy: 'Scribe is built around a single surface, but you can also open a full view on the right to review the result as you write. It uses the same rendering engine.',
      list: ['Open with Ctrl+Shift+P', 'The same engine as the editor', 'Sidebar with recents and outline']
    }
  }
};

const featureImage = document.querySelector('#feature-image');
const featureTabs = document.querySelectorAll('.feature-tabs button');
function selectFeature(tab) {
  const baseFeature = featureContent[tab.dataset.feature];
  const feature = currentLanguage === 'en' ? baseFeature.en : baseFeature;
  featureTabs.forEach(item => item.setAttribute('aria-selected', String(item === tab)));
  featureImage.classList.add('changing');
  const preload = new Image();
  preload.src = feature.image;
  const update = () => {
    featureImage.src = feature.image;
    featureImage.alt = feature.alt;
    document.querySelector('#feature-kicker').textContent = feature.kicker;
    document.querySelector('#feature-title').innerHTML = feature.title;
    document.querySelector('#feature-copy').textContent = feature.copy;
    document.querySelector('#feature-list').innerHTML = feature.list.map(item => `<li>${item}</li>`).join('');
    requestAnimationFrame(() => featureImage.classList.remove('changing'));
  };
  if (preload.complete) update(); else preload.addEventListener('load', update, { once: true });
}
featureTabs.forEach((tab, index) => {
  tab.addEventListener('click', () => selectFeature(tab));
  tab.addEventListener('keydown', event => {
    if (!['ArrowLeft', 'ArrowRight'].includes(event.key)) return;
    event.preventDefault();
    const direction = event.key === 'ArrowRight' ? 1 : -1;
    const next = featureTabs[(index + direction + featureTabs.length) % featureTabs.length];
    next.focus();
    selectFeature(next);
  });
});

const formats = {
  flathub: {
    description: 'Para cualquier distribución, con actualizaciones automáticas junto al sistema.',
    descriptionEn: 'For any distribution, updating together with the rest of your system.',
    code: 'flatpak install flathub io.github.gnacho.Scribe\nflatpak run io.github.gnacho.Scribe',
    link: 'https://flathub.org/apps/io.github.gnacho.Scribe',
    linkText: 'Ver en Flathub',
    linkTextEn: 'View on Flathub'
  },
  deb: {
    description: 'Para Debian, Ubuntu y derivados. Descarga e instala con apt.',
    descriptionEn: 'For Debian, Ubuntu and derivatives. Download and install with apt.',
    code: 'wget https://github.com/gnacho/scribe/releases/download/v<version>/scribe_<version>-1_amd64.deb\nsudo apt install ./scribe_<version>-1_amd64.deb',
    link: 'https://github.com/gnacho/scribe/releases/latest',
    linkText: 'Descargar desde GitHub',
    linkTextEn: 'Download from GitHub'
  },
  rpm: {
    description: 'Para Fedora y derivados. Descarga e instala con dnf.',
    descriptionEn: 'For Fedora and derivatives. Download and install with dnf.',
    code: 'wget https://github.com/gnacho/scribe/releases/download/v<version>/scribe-<version>-1.x86_64.rpm\nsudo dnf install ./scribe-<version>-1.x86_64.rpm',
    link: 'https://github.com/gnacho/scribe/releases/latest',
    linkText: 'Descargar desde GitHub',
    linkTextEn: 'Download from GitHub'
  },
  appimage: {
    description: 'Un único archivo, sin instalación. Descarga, da permisos y abre.',
    descriptionEn: 'A single file, no installation. Download, make it executable and open.',
    code: 'wget https://github.com/gnacho/scribe/releases/download/v<version>/Scribe-<version>-x86_64.AppImage\nchmod +x Scribe-<version>-x86_64.AppImage\n./Scribe-<version>-x86_64.AppImage',
    link: 'https://github.com/gnacho/scribe/releases/latest',
    linkText: 'Descargar desde GitHub',
    linkTextEn: 'Download from GitHub'
  },
  aur: {
    description: 'Para Arch Linux, disponible desde AUR con tu helper habitual.',
    descriptionEn: 'For Arch Linux, available from AUR with your usual helper.',
    code: 'yay -S scribe-bin',
    link: 'https://aur.archlinux.org/packages/scribe-bin',
    linkText: 'Ver en AUR',
    linkTextEn: 'View on AUR'
  }
};

const description = document.querySelector('#install-description');
const code = document.querySelector('#install-code');
const installDownload = document.querySelector('#install-download');
const tabs = document.querySelectorAll('.install-tabs button');
function updateInstall(tab) {
  tabs.forEach(item => item.setAttribute('aria-selected', String(item === tab)));
  const format = formats[tab.dataset.format];
  description.textContent = currentLanguage === 'en' ? format.descriptionEn : format.description;
  code.textContent = format.code;
  installDownload.href = format.link;
  installDownload.innerHTML = `${currentLanguage === 'en' ? format.linkTextEn : format.linkText} <span>↗</span>`;
}
tabs.forEach(tab => tab.addEventListener('click', () => updateInstall(tab)));

const toast = document.querySelector('.toast');
let toastTimer;
document.querySelector('.copy-button').addEventListener('click', async () => {
  try {
    await navigator.clipboard.writeText(code.textContent);
    toast.textContent = currentLanguage === 'en' ? 'Commands copied' : 'Comandos copiados';
  } catch {
    const selection = window.getSelection();
    const range = document.createRange();
    range.selectNodeContents(code);
    selection.removeAllRanges();
    selection.addRange(range);
    toast.textContent = currentLanguage === 'en' ? 'Commands selected; press Ctrl+C' : 'Comandos seleccionados; pulsa Ctrl+C';
  }
  toast.classList.add('visible');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => toast.classList.remove('visible'), 2600);
});

const demoContent = document.querySelector('#demo-content');
const DEMO_PAUSE = 2000;
let demoRun = 0;
let demoHasStarted = false;
let demoStartTimer;
let demoLoopTimer;
let demoCompleted = false;
let demoHeroVisible = false;

function getDemoSteps() {
  if (currentLanguage === 'en') return [
    { raw: '# Field journal', html: '<h2 class="demo-rendered">Field journal</h2>' },
    { raw: 'The fog has not lifted yet. I write slowly, **as the text finds its shape**.', html: '<p class="doc-lead demo-rendered">The fog has not lifted yet. I write slowly, <strong>as the text finds its shape</strong>.</p>' },
    { raw: '## The trail', html: '<h3 class="demo-rendered">The trail</h3>' },
    { raw: 'Two things worth remembering:', html: '<p>Two things worth remembering:</p>', plain: true },
    { raw: '- The trail is marked again.', html: '<ul class="demo-rendered"><li>The trail is marked again.</li></ul>' },
    { raw: '- Bring gloves next time.', html: '<li class="demo-rendered">Bring gloves next time.</li>', append: 'ul' },
    { raw: '> Clarity is not adding more.', html: '<blockquote class="demo-rendered">Clarity is not adding more.</blockquote>' },
    { raw: '> It is removing everything in the way.', html: '<br><span class="demo-rendered">It is removing everything in the way.</span>', append: 'blockquote' },
    { raw: '*And keep writing.*', html: '<div class="typing-line demo-rendered"><em>And keep writing.</em><span class="caret" aria-hidden="true"></span></div>' }
  ];
  return [
    { raw: '# Cuaderno de campo', html: '<h2 class="demo-rendered">Cuaderno de campo</h2>' },
    { raw: 'La niebla todavía no se ha levantado. Escribo despacio, **mientras el texto encuentra su forma**.', html: '<p class="doc-lead demo-rendered">La niebla todavía no se ha levantado. Escribo despacio, <strong>mientras el texto encuentra su forma</strong>.</p>' },
    { raw: '## El sendero', html: '<h3 class="demo-rendered">El sendero</h3>' },
    { raw: 'Dos cosas que merece la pena recordar:', html: '<p>Dos cosas que merece la pena recordar:</p>', plain: true },
    { raw: '- El camino vuelve a estar señalizado.', html: '<ul class="demo-rendered"><li>El camino vuelve a estar señalizado.</li></ul>' },
    { raw: '- Traer guantes la próxima vez.', html: '<li class="demo-rendered">Traer guantes la próxima vez.</li>', append: 'ul' },
    { raw: '> La claridad no es añadir más.', html: '<blockquote class="demo-rendered">La claridad no es añadir más.</blockquote>' },
    { raw: '> Es quitar todo lo que estorba.', html: '<br><span class="demo-rendered">Es quitar todo lo que estorba.</span>', append: 'blockquote' },
    { raw: '*Y seguir escribiendo.*', html: '<div class="typing-line demo-rendered"><em>Y seguir escribiendo.</em><span class="caret" aria-hidden="true"></span></div>' }
  ];
}

function appendRenderedStep(step) {
  if (step.append) {
    const target = demoContent.querySelector(`${step.append}:last-of-type`);
    target.insertAdjacentHTML('beforeend', step.html);
  } else {
    demoContent.insertAdjacentHTML('beforeend', step.html);
  }
}

function renderStaticDemo() {
  demoContent.innerHTML = '';
  getDemoSteps().forEach(appendRenderedStep);
}

function demoWait(ms) {
  return new Promise(resolve => setTimeout(resolve, ms));
}

function scheduleDemoRestart(run) {
  clearTimeout(demoLoopTimer);
  if (run !== demoRun || !demoHeroVisible || document.visibilityState !== 'visible') return;
  demoLoopTimer = setTimeout(() => {
    if (run !== demoRun || !demoHeroVisible || document.visibilityState !== 'visible') return;
    typeDemo();
  }, DEMO_PAUSE);
}

async function typeDemo() {
  demoHasStarted = true;
  demoCompleted = false;
  clearTimeout(demoStartTimer);
  clearTimeout(demoLoopTimer);
  const run = ++demoRun;
  if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
    renderStaticDemo();
    return;
  }
  demoContent.innerHTML = '';
  for (const step of getDemoSteps()) {
    if (run !== demoRun) return;
    const rawLine = document.createElement('div');
    rawLine.className = 'demo-raw';
    if (step.plain) rawLine.classList.add('demo-plain');
    const rawText = document.createElement('span');
    const caret = document.createElement('span');
    caret.className = 'demo-caret';
    caret.setAttribute('aria-hidden', 'true');
    rawLine.append(rawText, caret);
    demoContent.appendChild(rawLine);
    for (let index = 1; index <= step.raw.length; index++) {
      if (run !== demoRun) return;
      rawText.textContent = step.raw.slice(0, index);
      await demoWait(24);
    }
    await demoWait(180);
    if (run !== demoRun) return;
    if (step.plain) {
      caret.remove();
      rawLine.classList.remove('demo-raw', 'demo-plain');
      rawLine.classList.add('demo-plain-done');
      await demoWait(260);
      continue;
    }
    rawLine.remove();
    appendRenderedStep(step);
    await demoWait(260);
  }
  if (run !== demoRun) return;
  demoCompleted = true;
  scheduleDemoRestart(run);
}
document.querySelector('.replay-button').addEventListener('click', typeDemo);
document.querySelector('.demo-open').addEventListener('click', () => {
  document.querySelector('.document').scrollIntoView({ behavior: 'smooth', block: 'center' });
  typeDemo();
});

function refreshLocalizedContent() {
  selectFeature(document.querySelector('.feature-tabs button[aria-selected="true"]'));
  updateInstall(document.querySelector('.install-tabs button[aria-selected="true"]'));
  renderStaticDemo();
}

document.querySelector('.language-toggle').addEventListener('click', () => {
  setLanguage(currentLanguage === 'es' ? 'en' : 'es');
  refreshLocalizedContent();
  if (!window.matchMedia('(prefers-reduced-motion: reduce)').matches) typeDemo();
});

setLanguage(currentLanguage);
refreshLocalizedContent();

if (!window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
  const maybeStartDemo = () => {
    if (demoHasStarted || !demoHeroVisible || document.visibilityState !== 'visible') return;
    clearTimeout(demoStartTimer);
    demoStartTimer = setTimeout(() => {
      if (!demoHasStarted && demoHeroVisible && document.visibilityState === 'visible') typeDemo();
    }, 1400);
  };
  const heroObserver = new IntersectionObserver(entries => {
    demoHeroVisible = entries[0].isIntersecting;
    if (!demoHeroVisible) return;
    if (demoCompleted) scheduleDemoRestart(demoRun);
    else maybeStartDemo();
  }, { threshold: 0.35 });
  heroObserver.observe(document.querySelector('.hero-product'));
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState !== 'visible') return;
    if (demoCompleted) scheduleDemoRestart(demoRun);
    else maybeStartDemo();
  });
}
