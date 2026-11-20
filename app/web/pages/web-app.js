// Moved out of web/pages/web.html (2026-11-20) so the site's Content-Security-Policy can refuse
// inline script (docs/PRIORITIES.md item 6b). It runs at the same place in the page as before.
/**
 * The websites database, fetched at load. ONE list for both clients: the
 * desktop Browser page reads this same file (src/gui/mod.rs load_web_sites),
 * and tests/page_parity_lint.rs fails if either side stops reading it.
 * Before 2026-09-16 this page carried its own DEFAULT_SITES array and the
 * two lists had drifted apart.
 */
var SITES_URL = '/data/web/sites.json';
/** categories[] from the file, in display order: { id, name, color } */
var SITE_CATEGORIES = [];
/** sites[] from the file, normalized to the card shape used below */
var SITE_RECORDS = [];

var STORAGE_KEY = 'hos_bookmarks_v1';
var activeCategory = '';

/** Deterministic color from a string, generates a hue for the favicon circle */
function stringColor(str) {
  var hash = 0;
  for (var i = 0; i < str.length; i++) hash = str.charCodeAt(i) + ((hash << 5) - hash);
  var hue = Math.abs(hash) % 360;
  return 'hsl(' + hue + ', 55%, 40%)';
}

/** Extract first letter(s) for favicon placeholder */
function iconLetter(name) {
  var parts = name.trim().split(/\s+/);
  if (parts.length >= 2) return (parts[0][0] + parts[1][0]).toUpperCase();
  return name.trim().charAt(0).toUpperCase();
}

/** Human wording for a record's embed review state (shown on hover) */
function embedLabel(embed) {
  if (!embed) return '';
  if (embed.status === 'allowed') return 'Embedding: allowed (' + (embed.basis || '') + ')';
  if (embed.status === 'forbidden') return 'Embedding: forbidden (' + (embed.basis || '') + ')';
  if (embed.status === 'unknown') return 'Embedding: terms could not be found';
  return 'Embedding: not yet reviewed';
}

/** Load the database and render. A missing file leaves only custom bookmarks. */
function loadSites() {
  return fetch(SITES_URL, { cache: 'no-cache' })
    .then(function(r) { if (!r.ok) throw new Error('HTTP ' + r.status); return r.json(); })
    .then(function(db) {
      SITE_CATEGORIES = (db.categories || []).map(function(c) {
        return { id: c.id, name: c.name, color: c.color || 'accent' };
      });
      SITE_RECORDS = (db.sites || []).map(function(s) {
        return {
          name: s.name,
          desc: s.description || '',
          url: s.url,
          category: s.category,
          icon: s.icon || '',
          embed: s.embed || null,
          affiliate: s.affiliate || null
        };
      });
    })
    .catch(function(e) {
      console.warn('web: could not load ' + SITES_URL + ': ' + e.message);
    })
    .then(function() {
      fillCategorySelect();
      renderFilters();
      renderSites();
    });
}

/** The add-bookmark modal's category list comes from the same file. */
function fillCategorySelect() {
  var sel = document.getElementById('bm-cat');
  var html = '';
  SITE_CATEGORIES.forEach(function(c) {
    html += '<option value="' + esc(c.id) + '">' + esc(c.name) + '</option>';
  });
  html += '<option value="custom" selected>Custom</option>';
  sel.innerHTML = html;
}

/** Load user bookmarks from localStorage */
function loadUserBookmarks() {
  try {
    var data = localStorage.getItem(STORAGE_KEY);
    return data ? JSON.parse(data) : [];
  } catch (e) { return []; }
}

/** Save user bookmarks to localStorage */
function saveUserBookmarks(bookmarks) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(bookmarks));
}

/** Merge the database + user bookmarks into one list */
function getAllSites() {
  var user = loadUserBookmarks();
  // Tag user bookmarks so we can show delete button
  user.forEach(function(b) { b._custom = true; });
  return SITE_RECORDS.concat(user);
}

/** Display name of a category id (custom bookmarks store the id too) */
function categoryName(id) {
  for (var i = 0; i < SITE_CATEGORIES.length; i++) {
    if (SITE_CATEGORIES[i].id === id) return SITE_CATEGORIES[i].name;
  }
  return id === 'custom' ? 'Custom' : id;
}

/** Collect category ids: the file's order first, then any custom ones */
function getCategories(sites) {
  var cats = [];
  SITE_CATEGORIES.forEach(function(c) {
    sites.forEach(function(s) {
      if (s.category === c.id && cats.indexOf(c.id) === -1) cats.push(c.id);
    });
  });
  sites.forEach(function(s) {
    if (cats.indexOf(s.category) === -1) cats.push(s.category);
  });
  return cats;
}

/** Build category filter pills */
function renderFilters() {
  var sites = getAllSites();
  var cats = getCategories(sites);
  var el = document.getElementById('cat-filters');
  var html = '<span role="button" tabindex="0" class="cat-pill' + (activeCategory === '' ? ' active' : '') + '" ' + hosOn.attr('click', 'setCategory(\'\')') + '>All</span>';
  cats.forEach(function(c) {
    html += '<span role="button" tabindex="0" class="cat-pill' + (activeCategory === c ? ' active' : '') + '" ' + hosOn.attr('click', 'setCategory(' + hosArg(c) + ')') + '>' + esc(categoryName(c)) + '</span>';
  });
  el.innerHTML = html;
}

function setCategory(cat) {
  activeCategory = cat;
  renderFilters();
  renderSites();
}

/** Open a site in a new window. (The desktop app reads it in-app instead.) */
function openSite(url) {
  if (!url) return;
  window.open(url, '_blank', 'noopener');
}

/** Escape HTML to prevent XSS in user-supplied bookmark data */
function esc(str) {
  var d = document.createElement('div');
  d.textContent = str || '';
  return d.innerHTML.replace(/"/g, '&quot;').replace(/'/g, '&#39;');
}

/** Render site cards, filtered by active category and search text */
function renderSites() {
  var search = (document.getElementById('site-search').value || '').toLowerCase();
  var allSites = getAllSites();
  var filtered = allSites.filter(function(s) {
    if (activeCategory && s.category !== activeCategory) return false;
    if (search && s.name.toLowerCase().indexOf(search) === -1 &&
        (s.desc || '').toLowerCase().indexOf(search) === -1 &&
        (s.url || '').toLowerCase().indexOf(search) === -1 &&
        categoryName(s.category).toLowerCase().indexOf(search) === -1) return false;
    return true;
  });

  // Group by category
  var groups = {};
  var cats = getCategories(filtered);
  cats.forEach(function(c) { groups[c] = []; });
  filtered.forEach(function(s) {
    if (!groups[s.category]) { groups[s.category] = []; cats.push(s.category); }
    groups[s.category].push(s);
  });

  var html = '';
  cats.forEach(function(cat) {
    if (!groups[cat] || groups[cat].length === 0) return;
    html += '<div class="cat-section"><h2>' + esc(categoryName(cat)) + '</h2><div class="site-grid">';
    groups[cat].forEach(function(s) {
      var domain = s.url ? s.url.replace(/^https?:\/\//, '').replace(/\/.*$/, '') : '-';
      var letter = iconLetter(s.name);
      var color = stringColor(domain || s.name);
      var hasUrl = !!s.url;
      var tag = hasUrl ? 'a' : 'div';
      var review = s._custom ? 'Your bookmark' : embedLabel(s.embed);
      var hrefAttr = (hasUrl ? ' href="' + esc(s.url) + '" target="_blank" rel="noopener"' : '') +
        ' title="' + esc(s.name + '\n' + (s.url || '') + '\n' + review) + '"';
      var clickHandler = hasUrl ? ' ' + hosOn.attr('click', 'event.preventDefault();openSite(' + hosArg(s.url) + ')') + '' : '';

      html += '<' + tag + ' class="site-card"' + hrefAttr + clickHandler + '>' +
        '<div class="site-icon" style="background:' + color + '">' + letter + '</div>' +
        '<div class="site-info">' +
          '<div class="site-name">' + esc(s.name) + '</div>' +
          '<div class="site-desc">' + esc(s.desc) + '</div>' +
          '<div class="site-url">' + esc(domain) + '</div>' +
        '</div>';

      // Delete button for custom bookmarks
      if (s._custom) {
        html += '<button class="bookmark-delete" ' + hosOn.attr('click', 'event.preventDefault();event.stopPropagation();deleteBookmark(' + hosArg(s.url) + ',' + hosArg(s.name) + ')') + ' title="Remove bookmark" data-tip="Remove" data-tip-desc="Delete this custom bookmark.">✕</button>';
      }

      html += '</' + tag + '>';
    });
    html += '</div></div>';
  });

  if (!html) {
    html = '<div class="empty-state">No sites match your search.</div>';
  }

  document.getElementById('site-list').innerHTML = html;
}

function filterSites() { renderSites(); }

/** Delete a user bookmark by URL + name match */
function deleteBookmark(url, name) {
  var bookmarks = loadUserBookmarks();
  bookmarks = bookmarks.filter(function(b) {
    return !(b.url === url && b.name === name);
  });
  saveUserBookmarks(bookmarks);
  renderFilters();
  renderSites();
}

/** Open the add-bookmark modal */
function openAddModal() {
  document.getElementById('add-modal').classList.add('open');
  document.getElementById('bm-name').value = '';
  document.getElementById('bm-url').value = '';
  document.getElementById('bm-desc').value = '';
  document.getElementById('bm-cat').value = 'Custom';
  setTimeout(function() { document.getElementById('bm-name').focus(); }, 50);
}

/** Close the add-bookmark modal */
function closeAddModal() {
  document.getElementById('add-modal').classList.remove('open');
}

/** Save a new bookmark from the modal form */
function saveBookmark() {
  var name = document.getElementById('bm-name').value.trim();
  var url  = document.getElementById('bm-url').value.trim();
  var desc = document.getElementById('bm-desc').value.trim();
  var cat  = document.getElementById('bm-cat').value || 'custom';

  if (!name) { document.getElementById('bm-name').focus(); return; }
  if (!url)  { document.getElementById('bm-url').focus(); return; }

  // Auto-prepend https:// if no protocol
  if (url && !/^https?:\/\//i.test(url)) url = 'https://' + url;

  var bookmarks = loadUserBookmarks();
  bookmarks.push({ name: name, url: url, desc: desc || '', category: cat });
  saveUserBookmarks(bookmarks);

  closeAddModal();
  renderFilters();
  renderSites();
}

// ── Init ──
document.addEventListener('DOMContentLoaded', function() {
  loadSites();
});

// FAB click
document.getElementById('fab-add').addEventListener('click', openAddModal);

// Close modal on backdrop click
document.getElementById('add-modal').addEventListener('click', function(e) {
  if (e.target === this) closeAddModal();
});

// Close modal on Escape
document.addEventListener('keydown', function(e) {
  if (e.key === 'Escape') closeAddModal();
  // Enter in modal saves
  if (e.key === 'Enter' && document.getElementById('add-modal').classList.contains('open')) {
    e.preventDefault();
    saveBookmark();
  }
});
