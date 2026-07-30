/* Tools page: the catalog of EXTERNAL things, mirroring the native Tools page
   (src/gui/pages/tools.rs). Both read the SAME data/external/catalog.json, which
   holds two kinds:
     - "software": free programs you download and run yourself
     - "service" : real-world help websites and organizations
   Add entries to the data file, never here (Infinite-of-X). */
(function() {
  'use strict';

  var DATA_URL = '/data/external/catalog.json';

  var catalog = null;
  var loadFailed = false;
  var activeKind = null;      // null = every kind
  var activeCategory = null;  // null = every category
  var query = '';

  function esc(s) {
    if (!s) return '';
    return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;')
      .replace(/>/g, '&gt;').replace(/"/g, '&quot;');
  }

  function capitalize(s) {
    return s.charAt(0).toUpperCase() + s.slice(1);
  }

  /* Human label for a kind id. The ids come from the data file; these are the
     words we show for the two we ship. */
  function kindLabel(id) {
    var k = (catalog && catalog.kinds || []).find(function(x) { return x.id === id; });
    return k ? k.label : capitalize(id);
  }

  function matches(entry, cat) {
    if (!query) return true;
    var q = query;
    return entry.name.toLowerCase().indexOf(q) >= 0
      || (entry.description || '').toLowerCase().indexOf(q) >= 0
      || cat.name.toLowerCase().indexOf(q) >= 0
      || (entry.platforms || []).some(function(p) { return p.indexOf(q) >= 0; })
      || (cat.extensions || []).some(function(e) { return e.indexOf(q) >= 0; });
  }

  /* Kind + category filter chips. Categories are scoped to the active kind so
     the row never offers a filter that cannot match. */
  function renderFilters() {
    var bar = document.getElementById('tools-filters');
    if (!bar || !catalog) return;

    var kinds = [];
    (catalog.categories || []).forEach(function(c) {
      if (c.kind && kinds.indexOf(c.kind) < 0) kinds.push(c.kind);
    });

    var html = '<button class="filter-btn' + (activeKind === null ? ' active' : '') +
      '" data-kind="">Everything</button>';
    kinds.forEach(function(k) {
      html += '<button class="filter-btn' + (activeKind === k ? ' active' : '') +
        '" data-kind="' + esc(k) + '">' + esc(kindLabel(k)) + '</button>';
    });

    var cats = (catalog.categories || []).filter(function(c) {
      return activeKind === null || c.kind === activeKind;
    });
    if (cats.length) {
      html += '<span class="filter-sep" aria-hidden="true"></span>';
      html += '<button class="filter-btn' + (activeCategory === null ? ' active' : '') +
        '" data-cat="">All categories</button>';
      cats.forEach(function(c) {
        html += '<button class="filter-btn' + (activeCategory === c.id ? ' active' : '') +
          '" data-cat="' + esc(c.id) + '">' + esc(c.name) + '</button>';
      });
    }
    bar.innerHTML = html;

    bar.querySelectorAll('[data-kind]').forEach(function(btn) {
      btn.addEventListener('click', function() {
        var k = btn.getAttribute('data-kind');
        activeKind = k === '' ? null : k;
        activeCategory = null; // categories are kind-scoped
        renderFilters();
        render();
      });
    });
    bar.querySelectorAll('[data-cat]').forEach(function(btn) {
      btn.addEventListener('click', function() {
        var c = btn.getAttribute('data-cat');
        activeCategory = c === '' ? null : c;
        renderFilters();
        render();
      });
    });
  }

  function entryCard(entry, cat) {
    var badges = (entry.platforms || []).map(function(p) {
      return '<span class="tool-badge badge-' + esc(p) + '">' + esc(capitalize(p)) + '</span>';
    }).join('');
    var meta = badges;
    if (entry.license) meta += '<span class="tool-license">' + esc(entry.license) + '</span>';
    if (entry.size) meta += '<span class="tool-size">' + esc(entry.size) + '</span>';
    // A help service has no license/platform/size, so its meta row is omitted
    // rather than rendered empty.
    var action = cat.kind === 'service' ? 'Open website' : 'Download';
    return '<div class="tool-card">' +
      '<div class="tool-name"><a href="' + esc(entry.url) + '" target="_blank" rel="noopener">' +
        esc(entry.name) + '</a></div>' +
      '<div class="tool-desc">' + esc(entry.description) + '</div>' +
      (meta ? '<div class="tool-meta">' + meta + '</div>' : '') +
      '<div class="tool-action"><a class="tool-go" href="' + esc(entry.url) +
        '" target="_blank" rel="noopener">' + action + '</a></div>' +
    '</div>';
  }

  function render() {
    var container = document.getElementById('tools-list');
    if (!container) return;

    if (loadFailed) {
      container.innerHTML = '<div class="no-results">Could not load the catalog. ' +
        'Reload the page, or report it on the Bugs page.</div>';
      return;
    }
    if (!catalog) {
      container.innerHTML = '<div class="no-results">Loading...</div>';
      return;
    }

    var html = '';
    var total = 0;
    (catalog.categories || []).forEach(function(cat) {
      if (activeKind !== null && cat.kind !== activeKind) return;
      if (activeCategory !== null && cat.id !== activeCategory) return;

      var hits = (cat.entries || []).filter(function(e) { return matches(e, cat); });
      if (!hits.length) return;
      total += hits.length;

      var exts = (cat.extensions || []).map(function(e) {
        return '<span class="cat-ext">' + esc(e) + '</span>';
      }).join('');

      html += '<div class="cat-section" data-cat="' + esc(cat.id) + '">' +
        '<div class="cat-header" role="button" tabindex="0" data-toggle="' + esc(cat.id) + '">' +
          '<span class="cat-arrow" id="arrow-' + esc(cat.id) + '">&#9660;</span>' +
          '<h2>' + esc(cat.name) + '</h2>' +
          '<span class="cat-count">' + hits.length + ' ' +
            (hits.length === 1 ? 'entry' : 'entries') + '</span>' +
          '<span class="cat-kind">' + esc(kindLabel(cat.kind)) + '</span>' +
        '</div>' +
        (exts ? '<div class="cat-exts">' + exts + '</div>' : '') +
        '<div class="tools-grid" id="grid-' + esc(cat.id) + '">' +
          hits.map(function(e) { return entryCard(e, cat); }).join('') +
        '</div>' +
      '</div>';
    });

    container.innerHTML = html || '<div class="no-results">Nothing matches your search.</div>';

    var countEl = document.getElementById('tools-count');
    if (countEl) countEl.textContent = total ? (total + (total === 1 ? ' result' : ' results')) : '';

    container.querySelectorAll('[data-toggle]').forEach(function(h) {
      var toggle = function() {
        var id = h.getAttribute('data-toggle');
        var grid = document.getElementById('grid-' + id);
        var arrow = document.getElementById('arrow-' + id);
        if (!grid) return;
        var hidden = grid.style.display === 'none';
        grid.style.display = hidden ? 'grid' : 'none';
        if (arrow) arrow.classList.toggle('collapsed', !hidden);
      };
      h.addEventListener('click', toggle);
      h.addEventListener('keydown', function(ev) {
        if (ev.key === 'Enter' || ev.key === ' ') { ev.preventDefault(); toggle(); }
      });
    });
  }

  document.addEventListener('DOMContentLoaded', function() {
    render(); // "Loading..." until the fetch lands

    fetch(DATA_URL, { cache: 'no-cache' })
      .then(function(resp) {
        if (!resp.ok) throw new Error('HTTP ' + resp.status);
        return resp.json();
      })
      .then(function(json) {
        catalog = json;
        renderFilters();
        render();
      })
      .catch(function(err) {
        console.error('tools: could not load ' + DATA_URL, err);
        loadFailed = true;
        render();
      });

    var search = document.getElementById('tools-search');
    if (search) {
      var debounce = null;
      search.addEventListener('input', function() {
        clearTimeout(debounce);
        debounce = setTimeout(function() {
          query = search.value.toLowerCase().trim();
          render();
        }, 150);
      });
    }
  });
})();
