/* Resources page, context-aware curated links (Real = real-world help, Sim = in-game guides) */
(function() {
  'use strict';

  // ── Data (Infinite-of-X: every entry lives in data/resources.json, not in code) ──
  var DATA_URL = '/data/resources.json';

  var resourceData = null;   // parsed data/resources.json; null until the fetch lands
  var loadFailed = false;    // true if the fetch or the JSON parse failed

  var activeCategory = null; // null = show all

  function getContext() {
    return window.hos_context || 'real';
  }

  // The block for the active context ({ title, subtitle, categories }), or null
  // while the data file is still loading (or failed to load).
  function getData() {
    if (!resourceData || !resourceData.contexts) return null;
    var ctx = getContext();
    return resourceData.contexts[ctx] || resourceData.contexts.real || null;
  }

  function updateHeader() {
    var ctx = getContext();
    var title = document.getElementById('res-title');
    var subtitle = document.getElementById('res-subtitle');
    if (!title || !subtitle) return;
    var data = getData();
    // Headings ship with the data, so adding a context needs no code change.
    // The literals below are only the pre-load fallback.
    title.textContent = (data && data.title) ||
      (ctx === 'sim' ? 'Game Guides' : 'Resources');
    subtitle.textContent = (data && data.subtitle) ||
      (ctx === 'sim'
        ? 'In-game wiki, tutorials, and reference guides for HumanityOS simulation.'
        : 'Curated links to real-world help: education, health, legal, housing, and more.');
  }

  function renderFilterBar() {
    var bar = document.getElementById('filter-bar');
    if (!bar) return;
    var data = getData();
    if (!data) { bar.innerHTML = ''; return; } // still loading, or the load failed

    var html = '<button class="filter-btn' + (activeCategory === null ? ' active' : '') + '" aria-pressed="' + (activeCategory === null) + '" onclick="window._resFilter(null)">All</button>';
    for (var i = 0; i < data.categories.length; i++) {
      var cat = data.categories[i];
      var isActive = activeCategory === cat.name;
      html += '<button class="filter-btn' + (isActive ? ' active' : '') + '" aria-pressed="' + isActive + '" onclick="window._resFilter(\'' + cat.name.replace(/'/g, "\\'") + '\')">' + cat.name + '</button>';
    }
    bar.innerHTML = html;
  }

  function render(filter) {
    var container = document.getElementById('res-list');
    if (!container) return;

    var data = getData();
    if (!data) {
      // Honest states: never pretend the list is empty when it simply is not here yet.
      container.innerHTML = loadFailed
        ? '<div class="no-results">Could not load the resource list from ' + esc(DATA_URL) + '. Check your connection, then reload the page.</div>'
        : '<div class="no-results">Loading resources...</div>';
      return;
    }

    var q = (filter || '').toLowerCase().trim();
    var html = '';
    var totalShown = 0;

    for (var i = 0; i < data.categories.length; i++) {
      var cat = data.categories[i];

      // Category filter
      if (activeCategory && cat.name !== activeCategory) continue;

      // Search filter (url is optional: a guide that is not written yet has none)
      var matching = (cat.resources || []).filter(function(r) {
        if (!q) return true;
        return (r.name || '').toLowerCase().indexOf(q) !== -1 ||
               (r.desc || '').toLowerCase().indexOf(q) !== -1 ||
               cat.name.toLowerCase().indexOf(q) !== -1 ||
               (r.url || '').toLowerCase().indexOf(q) !== -1;
      });

      if (matching.length === 0) continue;
      totalShown += matching.length;

      var iconHtml = window.hosIcon ? hosIcon(cat.icon || 'globe', 18) : '';

      html += '<div class="cat-section">';
      html += '<div class="cat-header" role="button" tabindex="0" aria-expanded="true" onclick="window._resToggleCat(this)" onkeydown="if(event.key===\'Enter\'||event.key===\' \'){event.preventDefault();window._resToggleCat(this);}">';
      html += '<span class="cat-arrow">&#9660;</span> ';
      html += '<span>' + iconHtml + '</span>';
      html += '<h2>' + esc(cat.name) + '</h2>';
      html += '<span class="cat-count">(' + matching.length + ')</span>';
      html += '</div>';

      html += '<div class="res-grid">';
      for (var j = 0; j < matching.length; j++) {
        var r = matching[j];
        var url = r.url || '';
        var isExternal = url.indexOf('http') === 0;
        var targetAttr = isExternal ? ' target="_blank" rel="noopener noreferrer"' : '';
        html += '<div class="res-card">';
        if (url) {
          html += '<div class="res-name"><a href="' + esc(url) + '"' + targetAttr + '>' + esc(r.name) + '</a></div>';
        } else {
          // No target exists yet, so render the title as plain text.
          // A link that goes nowhere is worse than no link.
          html += '<div class="res-name">' + esc(r.name) + '</div>';
        }
        html += '<div class="res-desc">' + esc(r.desc || '') + '</div>';
        html += '<div class="res-tags">';
        if (isExternal) {
          html += '<span class="res-tag">' + esc(hostOf(url)) + '</span>';
        } else if (url) {
          html += '<span class="res-tag">In app</span>';
        } else {
          html += '<span class="res-tag res-tag-planned">Not written yet</span>';
        }
        html += '</div>';
        html += '</div>';
      }
      html += '</div></div>';
    }

    if (totalShown === 0) {
      html = '<div class="no-results">No resources found matching your search.</div>';
    }

    container.innerHTML = html;
  }

  // Hostname badge for an external link; falls back to the raw URL if unparseable.
  function hostOf(url) {
    try {
      return new URL(url).hostname.replace('www.', '');
    } catch (e) {
      return url;
    }
  }

  function esc(s) {
    var d = document.createElement('div');
    d.textContent = s;
    return d.innerHTML;
  }

  // ── Public filter handler ──
  window._resFilter = function(cat) {
    activeCategory = cat;
    renderFilterBar();
    var el = document.getElementById('res-search');
    render(el ? el.value : '');
  };

  // ── Public category collapse/expand (keyboard reachable, reports state) ──
  window._resToggleCat = function(header) {
    var grid = header.nextElementSibling;
    if (!grid) return;
    var wasCollapsed = grid.style.display === 'none';
    grid.style.display = wasCollapsed ? 'grid' : 'none';
    header.setAttribute('aria-expanded', wasCollapsed ? 'true' : 'false');
    var arrow = header.querySelector('.cat-arrow');
    if (arrow) arrow.classList.toggle('collapsed', !wasCollapsed);
  };

  // ── Search input ──
  var searchEl = document.getElementById('res-search');
  if (searchEl) {
    searchEl.addEventListener('input', function() {
      render(this.value);
    });
  }

  // ── Context change listener ──
  window.addEventListener('hos-context-change', function() {
    activeCategory = null;
    updateHeader();
    renderFilterBar();
    render(searchEl ? searchEl.value : '');
  });

  // ── Load data/resources.json, then render ──
  function rerender() {
    updateHeader();
    renderFilterBar();
    render(searchEl ? searchEl.value : '');
  }

  updateHeader();
  render(''); // shows "Loading resources..." until the fetch lands

  fetch(DATA_URL, { cache: 'no-cache' })
    .then(function(resp) {
      if (!resp.ok) throw new Error('HTTP ' + resp.status);
      return resp.json();
    })
    .then(function(json) {
      resourceData = json;
      rerender();
    })
    .catch(function(err) {
      loadFailed = true;
      console.error('[resources] could not load ' + DATA_URL, err);
      rerender();
    });

})();
