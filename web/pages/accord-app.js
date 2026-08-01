/**
 * HumanityOS Humanity Accord document browser.
 *
 * Two-pane: the left nav lists the Accord documents grouped by category; clicking
 * one renders it via the shared markdown renderer (window.hosMarkdown, see
 * web/shared/markdown.js).
 *
 * SOURCE (changed 2026-07-30): this reads `data/library/index.json`, the SAME
 * manifest the native Library and web /library read, filtered to the categories
 * flagged `accord: true` by scripts/build-library.js. It used to fetch a separate
 * relay endpoint (GET /api/docs/accord), which meant the constitution had two
 * independent document pipelines that could drift, and made this page fail
 * whenever the relay was down even though the documents are static files.
 *
 * /accord stays a public permalink for the constitution (the focused
 * accord-flagged view, 20 documents as of v0.1090.2), while /library is the
 * full document tree (62 docs after the mission-layer expansion). One data
 * source, two presentations.
 *
 * Slugs are derived from the manifest filename (humanity_accord.md ->
 * humanity-accord), matching the slugs the old relay endpoint served, so
 * existing deep links like /accord#humanity-accord keep working. Those links
 * live in governance.html, mission.html and onboarding.html.
 */
(function () {
  'use strict';

  const MANIFEST_URL = '/data/library/index.json';
  const DOC_BASE = '/data/library/';

  const $ = (id) => document.getElementById(id);
  let docsList = [];      // [{title, file, slug, category}]
  let activeSlug = null;
  const bodyCache = {};   // file -> markdown text

  function escapeHtml(str) {
    const div = document.createElement('div');
    div.textContent = str == null ? '' : str;
    return div.innerHTML;
  }

  /** humanity_accord.md -> humanity-accord (matches the retired relay slugs). */
  function slugFor(file) {
    return String(file).replace(/\.md$/i, '').replace(/_/g, '-').toLowerCase();
  }

  async function loadNav() {
    const nav = $('accord-nav');
    try {
      const res = await fetch(MANIFEST_URL, { cache: 'no-cache' });
      if (!res.ok) throw new Error('HTTP ' + res.status);
      const manifest = await res.json();

      docsList = [];
      for (const cat of (manifest.categories || [])) {
        if (!cat.accord) continue; // the Accord subset only
        for (const d of (cat.docs || [])) {
          docsList.push({ title: d.title, file: d.file, slug: slugFor(d.file), category: cat.name });
        }
      }

      if (!docsList.length) {
        nav.innerHTML = '<div class="accord-error">No Accord documents found. '
          + 'Run scripts/build-library.js.</div>';
        return;
      }
      renderNav();

      // Deep-link: /accord#slug opens that doc directly.
      const initial = (window.location.hash || '').replace(/^#/, '');
      const target = docsList.find((d) => d.slug === initial) || docsList[0];
      if (target) openDoc(target.slug);
    } catch (e) {
      console.error('accord: could not load ' + MANIFEST_URL, e);
      nav.innerHTML = '<div class="accord-error">Could not load the Accord index.</div>';
    }
  }

  function renderNav() {
    const nav = $('accord-nav');
    const categories = [];
    const byCat = new Map();
    for (const doc of docsList) {
      if (!byCat.has(doc.category)) { categories.push(doc.category); byCat.set(doc.category, []); }
      byCat.get(doc.category).push(doc);
    }
    let html = '';
    for (const cat of categories) {
      html += '<div class="accord-category">' + escapeHtml(cat) + '</div>';
      for (const doc of byCat.get(cat)) {
        const active = doc.slug === activeSlug ? ' active' : '';
        html += '<button class="accord-doc-link' + active + '" data-slug="' + escapeHtml(doc.slug) + '">'
          + escapeHtml(doc.title) + '</button>';
      }
    }
    nav.innerHTML = html;
    nav.querySelectorAll('.accord-doc-link').forEach((btn) => {
      btn.addEventListener('click', () => openDoc(btn.getAttribute('data-slug')));
    });
  }

  async function openDoc(slug) {
    const doc = docsList.find((d) => d.slug === slug);
    if (!doc) return;
    activeSlug = slug;
    renderNav();
    window.location.hash = slug;

    const content = $('accord-content');
    const paint = (md) => {
      content.innerHTML = '<div class="md-viewer"><h1>' + escapeHtml(doc.title) + '</h1>'
        + window.hosMarkdown.render(md) + '</div>';
      content.scrollTop = 0;
    };

    if (bodyCache[doc.file]) { paint(bodyCache[doc.file]); return; }
    content.innerHTML = '<div class="loading">Loading...</div>';
    try {
      const res = await fetch(DOC_BASE + encodeURIComponent(doc.file), { cache: 'no-cache' });
      if (!res.ok) throw new Error('HTTP ' + res.status);
      const md = await res.text();
      bodyCache[doc.file] = md;
      // Guard against a slow fetch landing after the reader moved on.
      if (activeSlug === slug) paint(md);
    } catch (e) {
      console.error('accord: could not load ' + doc.file, e);
      content.innerHTML = '<div class="accord-error">Could not load "'
        + escapeHtml(doc.title) + '".</div>';
    }
  }

  window.addEventListener('DOMContentLoaded', loadNav);
})();
