/* Scribe landing: i18n engine, theme, language, comparison tables, slider, copy buttons. */

(function () {
  "use strict";

  var LANG_KEY = "scribe-lang";
  var THEME_KEY = "scribe-theme";
  var reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");

  var state = {
    lang: localStorage.getItem(LANG_KEY) || guessLang(),
    theme: localStorage.getItem(THEME_KEY) || guessTheme()
  };

  function guessLang() {
    var param = new URLSearchParams(window.location.search).get("lang");
    if (param === "es" || param === "en") { return param; }
    return (navigator.language || "en").toLowerCase().startsWith("es") ? "es" : "en";
  }

  function guessTheme() {
    var param = new URLSearchParams(window.location.search).get("theme");
    if (param === "dark" || param === "light") { return param; }
    return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }

  function t(path) {
    var parts = path.split(".");
    var node = I18N[state.lang];
    for (var i = 0; i < parts.length; i++) {
      if (node == null) { return path; }
      node = node[parts[i]];
    }
    return node == null ? path : node;
  }

  /* ---------- i18n apply ---------- */

  function applyI18n() {
    document.documentElement.lang = state.lang;
    document.title = t("meta.title");
    var desc = document.querySelector('meta[name="description"]');
    if (desc) { desc.setAttribute("content", t("meta.description")); }

    document.querySelectorAll("[data-i18n]").forEach(function (el) {
      el.textContent = t(el.getAttribute("data-i18n"));
    });
    document.querySelectorAll("[data-i18n-aria]").forEach(function (el) {
      el.setAttribute("aria-label", t(el.getAttribute("data-i18n-aria")));
    });

    var langBtn = document.getElementById("lang-btn");
    if (langBtn) { langBtn.textContent = state.lang === "en" ? "ES" : "EN"; }

    renderCompareTables();
    buildSlider();
    document.dispatchEvent(new Event("scribe:lang"));
  }

  /* ---------- comparison tables ---------- */

  function cellHtml(raw, isScribe) {
    var text = String(raw);
    var sym = null;
    var m = text.match(/^([✓✗◐])\s*/);
    if (m) {
      sym = m[1];
      text = text.slice(m[0].length);
    }
    var out = "";
    if (sym) {
      out += '<span class="sym">' + sym + "</span>";
      if (text.length) { out += esc(text); }
    } else {
      out += esc(text);
    }
    var cls = isScribe ? ' class="scribe-col"' : "";
    return "<td" + cls + ">" + out + "</td>";
  }

  function renderCompareTables() {
    var container = document.getElementById("compare-tables");
    if (!container) { return; }
    var dict = I18N[state.lang].compare;
    var headers = I18N[state.lang].tableHeaders;
    var html = "";
    dict.blocks.forEach(function (block) {
      html += '<div class="tbl-block"><h3>' + esc(block.title) + '</h3><div class="table-wrap"><table><caption class="sr-only">' + esc(block.title) + '</caption><thead><tr>';
      headers.forEach(function (h) { html += '<th scope="col">' + esc(h) + "</th>"; });
      html += "</tr></thead><tbody>";
      block.rows.forEach(function (row) {
        html += "<tr>";
        row.forEach(function (cell, c) {
          html += cellHtml(cell, c === 5);
        });
        html += "</tr>";
      });
      html += "</tbody></table></div></div>";
    });
    html += '<p class="note"><strong>✓</strong> ' + esc(dict.legend) + "</p>";
    container.innerHTML = html;
  }

  function esc(s) {
    return String(s).replace(/[&<>"']/g, function (ch) {
      return { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[ch];
    });
  }

  /* ---------- screenshots slider ---------- */

  var SHOT_VIEWS = ["editor", "split", "outline", "focus", "typewriter", "find", "prefs", "shortcuts"];
  var shotIndex = 0;
  var availableShots = [];

  function shotFile(view, lang, theme) {
    return "assets/shots/shot-" + view + "-" + lang + "-" + theme + ".webp?v=3";
  }

  function probeShots() {
    availableShots = [];
    var pending = SHOT_VIEWS.length;
    SHOT_VIEWS.forEach(function (view) {
      var img = new Image();
      img.onload = function () {
        availableShots.push(view);
        pending--;
        if (pending === 0) { finishProbe(); }
      };
      img.onerror = function () {
        pending--;
        if (pending === 0) { finishProbe(); }
      };
      img.src = shotFile(view, state.lang, state.theme);
    });
  }

  function finishProbe() {
    var section = document.querySelector(".shots");
    if (!section) { return; }
    if (availableShots.length === 0) {
      section.hidden = true;
      return;
    }
    section.hidden = false;
    var order = SHOT_VIEWS.filter(function (v) { return availableShots.indexOf(v) !== -1; });
    var first = availableShots.length === 0;
    availableShots = order;
    if (first) { shotIndex = 0; }
    buildThumbs();
    showShot(shotIndex);
  }

  function buildSlider() { probeShots(); }

  function buildThumbs() {
    var wrap = document.getElementById("shot-thumbs");
    if (!wrap) { return; }
    wrap.innerHTML = "";
    availableShots.forEach(function (view, i) {
      var btn = document.createElement("button");
      btn.type = "button";
      btn.setAttribute("aria-label", t("shots.views." + view));
      btn.setAttribute("aria-pressed", i === shotIndex ? "true" : "false");
      var img = document.createElement("img");
      img.src = shotFile(view, state.lang, state.theme);
      img.alt = "";
      btn.appendChild(img);
      btn.addEventListener("click", function () { showShot(i); });
      wrap.appendChild(btn);
    });
  }

  function showShot(i) {
    if (availableShots.length === 0) { return; }
    shotIndex = (i + availableShots.length) % availableShots.length;
    var view = availableShots[shotIndex];
    var img = document.getElementById("shot-img");
    var caption = document.getElementById("shot-caption");
    var swap = function () {
      if (img) {
        img.src = shotFile(view, state.lang, state.theme);
        img.alt = t("shots.views." + view);
      }
      if (caption) { caption.textContent = t("shots.views." + view); }
      var thumbs = document.querySelectorAll("#shot-thumbs button");
      thumbs.forEach(function (b, j) {
        b.setAttribute("aria-pressed", j === shotIndex ? "true" : "false");
      });
    };
    if (img && !reducedMotion.matches) {
      img.classList.add("is-fading");
      setTimeout(function () {
        swap();
        img.classList.remove("is-fading");
      }, 180);
    } else {
      swap();
    }
  }

  /* ---------- copy buttons ---------- */

  function wireCopyButtons() {
    document.querySelectorAll(".copy-btn").forEach(function (btn) {
      btn.addEventListener("click", function () {
        var pre = document.getElementById(btn.getAttribute("data-copy"));
        if (!pre) { return; }
        var text = pre.textContent;
        var done = function () {
          var original = t("install.copy");
          btn.textContent = t("install.copied");
          setTimeout(function () { btn.textContent = original; }, 2000);
        };
        var fail = function () {
          var original = t("install.copy");
          btn.textContent = t("install.copyFail");
          setTimeout(function () { btn.textContent = original; }, 2400);
        };
        if (navigator.clipboard && window.isSecureContext) {
          navigator.clipboard.writeText(text).then(done, function () { fallbackCopy(text, done, fail); });
        } else {
          fallbackCopy(text, done, fail);
        }
      });
    });
  }

  function fallbackCopy(text, done, fail) {
    var ta = document.createElement("textarea");
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    try { document.execCommand("copy"); done(); } catch (e) { if (fail) { fail(); } }
    document.body.removeChild(ta);
  }

  /* ---------- theme and language ---------- */

  function applyTheme() {
    document.documentElement.setAttribute("data-theme", state.theme);
    var sw = document.getElementById("theme-switch");
    if (sw) { sw.setAttribute("aria-checked", state.theme === "dark" ? "true" : "false"); }
    var meta = document.querySelector('meta[name="theme-color"]:not([media])');
    if (meta) { meta.setAttribute("content", state.theme === "dark" ? "#1d1d20" : "#fafafb"); }
  }

  function wireControls() {
    var sw = document.getElementById("theme-switch");
    if (sw) {
      sw.addEventListener("click", function () {
        state.theme = state.theme === "dark" ? "light" : "dark";
        localStorage.setItem(THEME_KEY, state.theme);
        var repaint = function () {
          applyTheme();
          buildSlider();
        };
        if (document.startViewTransition && !reducedMotion.matches) {
          document.startViewTransition(repaint);
        } else {
          repaint();
        }
      });
    }
    var langBtn = document.getElementById("lang-btn");
    if (langBtn) {
      langBtn.addEventListener("click", function () {
        state.lang = state.lang === "en" ? "es" : "en";
        localStorage.setItem(LANG_KEY, state.lang);
        applyI18n();
      });
    }
    var prev = document.getElementById("shot-prev");
    var next = document.getElementById("shot-next");
    if (prev) { prev.addEventListener("click", function () { showShot(shotIndex - 1); }); }
    if (next) { next.addEventListener("click", function () { showShot(shotIndex + 1); }); }
  }

  /* ---------- video ---------- */

  function wireVideo() {
    var video = document.getElementById("demo-video");
    if (!video) { return; }
    if (reducedMotion.matches) {
      video.pause();
      video.removeAttribute("autoplay");
      video.controls = true;
    }
    reducedMotion.addEventListener("change", function (ev) {
      if (ev.matches) { video.pause(); video.controls = true; } else { video.controls = false; video.play().catch(function () {}); }
    });
  }

  /* ---------- scroll reveals ---------- */

  function wireReveals() {
    var targets = document.querySelectorAll(".reveal");
    if (!targets.length) { return; }
    if (reducedMotion.matches || !("IntersectionObserver" in window)) {
      targets.forEach(function (el) { el.classList.add("is-visible"); });
      return;
    }
    var observer = new IntersectionObserver(function (entries) {
      entries.forEach(function (entry) {
        if (entry.isIntersecting) {
          entry.target.classList.add("is-visible");
          observer.unobserve(entry.target);
        }
      });
    }, { threshold: 0.12 });
    targets.forEach(function (el) { observer.observe(el); });
  }

  /* ---------- install tabs ---------- */

  function wireInstallTabs() {
    var tabs = Array.prototype.slice.call(document.querySelectorAll(".tabs [role=tab]"));
    var indicator = document.querySelector(".tabs-indicator");
    if (!tabs.length) { return; }

    function moveIndicator(tab) {
      if (!indicator || !tab) { return; }
      indicator.style.width = tab.offsetWidth + "px";
      indicator.style.transform = "translateX(" + tab.offsetLeft + "px)";
    }

    function activate(tab, focus) {
      tabs.forEach(function (other) {
        var on = other === tab;
        other.setAttribute("aria-selected", on ? "true" : "false");
        other.setAttribute("tabindex", on ? "0" : "-1");
        var panel = document.getElementById(other.getAttribute("aria-controls"));
        if (panel) { panel.hidden = !on; }
      });
      moveIndicator(tab);
      if (focus) { tab.focus(); }
    }

    tabs.forEach(function (tab, i) {
      tab.addEventListener("click", function () { activate(tab, false); });
      tab.addEventListener("keydown", function (ev) {
        var next = null;
        if (ev.key === "ArrowRight") { next = tabs[(i + 1) % tabs.length]; }
        else if (ev.key === "ArrowLeft") { next = tabs[(i - 1 + tabs.length) % tabs.length]; }
        else if (ev.key === "Home") { next = tabs[0]; }
        else if (ev.key === "End") { next = tabs[tabs.length - 1]; }
        if (next) { ev.preventDefault(); activate(next, true); }
      });
    });

    var initial = tabs.filter(function (tab) { return tab.getAttribute("aria-selected") === "true"; })[0] || tabs[0];
    activate(initial, false);
    window.addEventListener("resize", function () {
      var current = tabs.filter(function (tab) { return tab.getAttribute("aria-selected") === "true"; })[0];
      moveIndicator(current);
    });
    document.addEventListener("scribe:lang", function () {
      var current = tabs.filter(function (tab) { return tab.getAttribute("aria-selected") === "true"; })[0];
      moveIndicator(current);
    });
  }

  /* ---------- scroll effects ---------- */

  function wireScrollFx() {
    var progress = document.createElement("div");
    progress.className = "read-progress";
    progress.setAttribute("aria-hidden", "true");
    document.body.appendChild(progress);

    var nav = document.querySelector(".nav");
    var heroVideo = document.querySelector(".hero-video");
    var specs = Array.prototype.slice.call(document.querySelectorAll(".spec"));
    var ticking = false;

    function onScroll() {
      if (ticking) { return; }
      ticking = true;
      window.requestAnimationFrame(function () {
        var doc = document.documentElement;
        var max = doc.scrollHeight - window.innerHeight;
        var y = window.scrollY || doc.scrollTop;
        var ratio = max > 0 ? Math.min(1, y / max) : 0;
        progress.style.transform = "scaleX(" + ratio + ")";
        if (nav) { nav.classList.toggle("is-scrolled", y > 8); }
        if (heroVideo && !reducedMotion.matches) {
          heroVideo.style.transform = "translateY(" + Math.min(28, y * 0.06) + "px)";
        }
        ticking = false;
      });
    }
    window.addEventListener("scroll", onScroll, { passive: true });
    onScroll();

    if (!reducedMotion.matches) {
      specs.forEach(function (card) {
        card.addEventListener("mousemove", function (ev) {
          var r = card.getBoundingClientRect();
          var dx = (ev.clientX - r.left) / r.width - 0.5;
          var dy = (ev.clientY - r.top) / r.height - 0.5;
          card.style.transform = "perspective(700px) rotateX(" + (-dy * 5).toFixed(2) + "deg) rotateY(" + (dx * 5).toFixed(2) + "deg) translateY(-2px)";
        });
        card.addEventListener("mouseleave", function () { card.style.transform = ""; });
      });
    }

    reducedMotion.addEventListener("change", function () {
      if (reducedMotion.matches && heroVideo) { heroVideo.style.transform = ""; }
    });
  }

  /* ---------- init ---------- */

  applyTheme();
  applyI18n();
  wireControls();
  wireCopyButtons();
  wireVideo();
  wireReveals();
  wireInstallTabs();
  wireScrollFx();
})();
