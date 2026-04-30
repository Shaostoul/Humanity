/* Tools catalog page — loads catalog.json and renders browsable tool listings */
(function() {
  'use strict';

  let catalog = null;

  async function loadCatalog() {
    try {
      const resp = await fetch('/api/files/read?path=tools/catalog.json');
      if (resp.ok) {
        const data = await resp.json();
        catalog = JSON.parse(data.content);
      }
    } catch (_) {}

    // Fallback: try direct path
    if (!catalog) {
      try {
        const resp = await fetch('/data/tools/catalog.json');
        if (resp.ok) catalog = await resp.json();
      } catch (_) {}
    }

    // Hardcoded fallback if both fail
    if (!catalog) {
      catalog = { categories: [] };
    }

    renderCatalog('');
  }

  function renderCatalog(filter) {
    const container = document.getElementById('tools-list');
    if (!container || !catalog) return;

    const q = filter.toLowerCase().trim();
    let html = '';

    for (const cat of catalog.categories) {
      // Filter: check if category name, any tool name, or any extension matches
      const matchingTools = cat.tools.filter(t => {
        if (!q) return true;
        return t.name.toLowerCase().includes(q) ||
               t.description.toLowerCase().includes(q) ||
               t.platforms.some(p => p.includes(q)) ||
               cat.name.toLowerCase().includes(q) ||
               cat.extensions.some(e => e.includes(q));
      });

      if (matchingTools.length === 0) continue;

      const extsHtml = cat.extensions.map(e =>
        '<span class="cat-ext">' + e + '</span>'
      ).join('');

      const toolsHtml = matchingTools.map(t => {
        const platforms = t.platforms.map(p =>
          '<span class="tool-badge badge-' + p + '">' + capitalize(p) + '</span>'
        ).join('');

        return '<div class="tool-card">' +
          '<div class="tool-name"><a href="' + esc(t.url) + '" target="_blank" rel="noopener">' + esc(t.name) + '</a></div>' +
          '<div class="tool-desc">' + esc(t.description) + '</div>' +
          '<div class="tool-meta">' +
            platforms +
            '<span class="tool-license">' + esc(t.license) + '</span>' +
            '<span class="tool-size">' + esc(t.size) + '</span>' +
          '</div>' +
        '</div>';
      }).join('');

      html += '<div class="cat-section" data-cat="' + cat.id + '">' +
        '<div class="cat-header" onclick="toggleCat(\'' + cat.id + '\')">' +
          '<span class="cat-arrow" id="arrow-' + cat.id + '">&#9660;</span>' +
          '<h2>' + esc(cat.name) + '</h2>' +
          '<span class="cat-count">' + matchingTools.length + ' tools</span>' +
        '</div>' +
        '<div class="cat-exts">' + extsHtml + '</div>' +
        '<div class="tools-grid" id="grid-' + cat.id + '">' + toolsHtml + '</div>' +
      '</div>';
    }

    if (!html) {
      html = '<div class="no-results">No tools match your search.</div>';
    }

    container.innerHTML = html;
  }

  // Toggle category collapse
  window.toggleCat = function(catId) {
    const grid = document.getElementById('grid-' + catId);
    const arrow = document.getElementById('arrow-' + catId);
    if (!grid) return;
    const hidden = grid.style.display === 'none';
    grid.style.display = hidden ? 'grid' : 'none';
    if (arrow) arrow.classList.toggle('collapsed', !hidden);
  };

  function esc(s) {
    if (!s) return '';
    return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
  }

  function capitalize(s) {
    return s.charAt(0).toUpperCase() + s.slice(1);
  }

  // Init
  document.addEventListener('DOMContentLoaded', function() {
    loadCatalog();

    const search = document.getElementById('tools-search');
    if (search) {
      let debounce = null;
      search.addEventListener('input', function() {
        clearTimeout(debounce);
        debounce = setTimeout(function() {
          renderCatalog(search.value);
        }, 200);
      });
    }
  });
})();
