/**
 * HumanityOS Humanity Accord document browser.
 * Two-pane: left nav lists the curated Accord docs (grouped by category,
 * fetched from GET /api/docs/accord); clicking one fetches its content from
 * GET /api/docs/accord/{slug} and renders it via the shared markdown
 * renderer (window.hosMarkdown, see web/shared/markdown.js).
 */
(function () {
  'use strict';

  const $ = (id) => document.getElementById(id);
  let docsList = [];
  let activeSlug = null;

  function escapeHtml(str) {
    const div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
  }

  async function loadNav() {
    const nav = $('accord-nav');
    try {
      const res = await fetch('/api/docs/accord');
      const data = await res.json();
      if (!data.ok || !Array.isArray(data.docs)) {
        nav.innerHTML = '<div class="accord-error">Could not load the Accord index.</div>';
        return;
      }
      docsList = data.docs;
      renderNav();

      // Deep-link: /accord#slug opens that doc directly (index.html, governance.html,
      // and onboarding.html link here as /accord#humanity-accord).
      const initial = (window.location.hash || '').replace(/^#/, '');
      const target = docsList.find((d) => d.slug === initial) || docsList[0];
      if (target) openDoc(target.slug);
    } catch (e) {
      nav.innerHTML = '<div class="accord-error">Could not reach the server.</div>';
    }
  }

  function renderNav() {
    const nav = $('accord-nav');
    const categories = [];
    const bySlug = new Map();
    for (const doc of docsList) {
      if (!bySlug.has(doc.category)) { categories.push(doc.category); bySlug.set(doc.category, []); }
      bySlug.get(doc.category).push(doc);
    }
    let html = '';
    for (const cat of categories) {
      html += '<div class="accord-category">' + escapeHtml(cat) + '</div>';
      for (const doc of bySlug.get(cat)) {
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
    activeSlug = slug;
    renderNav();
    window.location.hash = slug;
    const content = $('accord-content');
    content.innerHTML = '<div class="loading">Loading...</div>';
    try {
      const res = await fetch('/api/docs/accord/' + encodeURIComponent(slug));
      const data = await res.json();
      if (!data.ok) {
        content.innerHTML = '<div class="accord-error">Document not found.</div>';
        return;
      }
      const html = window.hosMarkdown.render(data.content);
      content.innerHTML = '<div class="md-viewer"><h1>' + escapeHtml(data.title) + '</h1>' + html + '</div>';
    } catch (e) {
      content.innerHTML = '<div class="accord-error">Could not reach the server.</div>';
    }
  }

  window.addEventListener('DOMContentLoaded', loadNav);
})();
