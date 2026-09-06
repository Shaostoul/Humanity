/* Library page: the web mirror of the native Library (src/gui/pages/library.rs).
   Reads the SAME data/library/index.json manifest and the SAME markdown files
   the native app loads from disk, so the two clients cannot drift. The external
   tools/services that used to share this page moved to /tools in v0.1063:
   Library is what you READ, Tools is what you GO USE. */
(function() {
  'use strict';

  var MANIFEST_URL = '/data/library/index.json';
  var DOC_BASE = '/data/library/';
  var GLOSSARY_URL = '/data/glossary.json';

  var manifest = null;
  var glossary = null;
  var current = null;       // {ci, di} of the open doc, or 'dictionary'
  var docCache = {};        // file -> markdown text
  var dictQuery = '';

  function esc(s) {
    if (!s) return '';
    return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;')
      .replace(/>/g, '&gt;').replace(/"/g, '&quot;');
  }

  function md(text) {
    if (window.hosMarkdown && window.hosMarkdown.render) return window.hosMarkdown.render(text);
    return '<pre>' + esc(text) + '</pre>';
  }

  function contentEl() { return document.getElementById('lib-content'); }

  /* Deep links: /library#the-five-adversaries opens that doc directly.
     Slugs match the /accord convention (filename minus .md, _ -> -), so the
     two pages share one link grammar. Native needs no equivalent: it has no
     URLs, and its rail already navigates directly. */
  function slugOf(file) {
    return String(file).replace(/\.md$/i, '').replace(/_/g, '-').toLowerCase();
  }
  function findBySlug(slug) {
    if (!manifest || !slug) return null;
    var cats = manifest.categories || [];
    for (var ci = 0; ci < cats.length; ci++) {
      var docs = cats[ci].docs || [];
      for (var di = 0; di < docs.length; di++) {
        if (slugOf(docs[di].file) === slug) return { ci: ci, di: di };
      }
    }
    return null;
  }
  function openHashDoc() {
    var hit = findBySlug(location.hash.replace(/^#/, ''));
    if (hit) openDoc(hit.ci, hit.di);
    return !!hit;
  }

  /* ── Left rail: nested category tree, mirroring the native collapsing headers ── */
  function renderRail() {
    var rail = document.getElementById('lib-rail');
    if (!rail) return;

    if (!manifest) {
      rail.innerHTML = '<div class="lib-empty">Loading...</div>';
      return;
    }
    var cats = manifest.categories || [];
    if (!cats.length) {
      rail.innerHTML = '<div class="lib-empty">No documents found. Run scripts/build-library.js.</div>';
      return;
    }

    var html = '';
    cats.forEach(function(cat, ci) {
      var docs = cat.docs || [];
      if (!docs.length) return;
      html += '<div class="lib-cat">' +
        '<div class="lib-cat-head" role="button" tabindex="0" data-cat="' + ci + '">' +
          '<span class="lib-cat-arrow" id="lib-arrow-' + ci + '">&#9660;</span>' +
          '<span>' + esc(cat.name) + '</span>' +
        '</div>' +
        '<div class="lib-cat-docs" id="lib-docs-' + ci + '">' +
          docs.map(function(d, di) {
            var active = current && current.ci === ci && current.di === di;
            return '<button class="lib-doc' + (active ? ' active' : '') +
              '" data-ci="' + ci + '" data-di="' + di + '">' + esc(d.title) + '</button>';
          }).join('') +
        '</div>' +
      '</div>';
    });

    html += '<div class="lib-special">' +
      '<button class="lib-doc' + (current === 'dictionary' ? ' active' : '') +
      '" data-dict="1" style="padding-left:0;font-weight:600;">Dictionary</button>' +
    '</div>';

    rail.innerHTML = html;

    rail.querySelectorAll('[data-cat]').forEach(function(h) {
      var toggle = function() {
        var ci = h.getAttribute('data-cat');
        var box = document.getElementById('lib-docs-' + ci);
        var arrow = document.getElementById('lib-arrow-' + ci);
        if (!box) return;
        var hidden = box.style.display === 'none';
        box.style.display = hidden ? 'block' : 'none';
        if (arrow) arrow.classList.toggle('collapsed', !hidden);
      };
      h.addEventListener('click', toggle);
      h.addEventListener('keydown', function(ev) {
        if (ev.key === 'Enter' || ev.key === ' ') { ev.preventDefault(); toggle(); }
      });
    });
    rail.querySelectorAll('[data-ci]').forEach(function(b) {
      b.addEventListener('click', function() {
        openDoc(+b.getAttribute('data-ci'), +b.getAttribute('data-di'));
      });
    });
    var dictBtn = rail.querySelector('[data-dict]');
    if (dictBtn) dictBtn.addEventListener('click', openDictionary);
  }

  /* ── Reader ── */
  function openDoc(ci, di) {
    var cat = (manifest.categories || [])[ci];
    var doc = cat && (cat.docs || [])[di];
    if (!doc) return;
    current = { ci: ci, di: di };
    // Keep the address bar shareable without growing history on every click.
    if (history.replaceState) history.replaceState(null, '', '#' + slugOf(doc.file));
    renderRail();

    var el = contentEl();
    if (docCache[doc.file]) {
      el.innerHTML = '<div class="md-viewer">' + md(docCache[doc.file]) + '</div>';
      el.scrollTop = 0;
      return;
    }
    el.innerHTML = '<div class="lib-empty">Loading ' + esc(doc.title) + '...</div>';
    fetch(DOC_BASE + encodeURIComponent(doc.file), { cache: 'no-cache' })
      .then(function(r) {
        if (!r.ok) throw new Error('HTTP ' + r.status);
        return r.text();
      })
      .then(function(text) {
        docCache[doc.file] = text;
        // Guard against a slow fetch landing after the reader moved on.
        if (current && current.ci === ci && current.di === di) {
          el.innerHTML = '<div class="md-viewer">' + md(text) + '</div>';
          el.scrollTop = 0;
        }
      })
      .catch(function(err) {
        console.error('library: could not load ' + doc.file, err);
        el.innerHTML = '<div class="lib-empty">Could not load "' + esc(doc.title) +
          '". Reload the page, or report it on the Bugs page.</div>';
      });
  }

  /* ── Dictionary: every glossary term, searchable ── */
  function openDictionary() {
    current = 'dictionary';
    renderRail();
    renderDictionary();
    if (!glossary) {
      fetch(GLOSSARY_URL, { cache: 'no-cache' })
        .then(function(r) { return r.ok ? r.json() : Promise.reject(new Error('HTTP ' + r.status)); })
        .then(function(j) { glossary = j; if (current === 'dictionary') renderDictionary(); })
        .catch(function(err) {
          console.error('library: could not load glossary', err);
          glossary = { terms: {}, categories: {} };
          if (current === 'dictionary') renderDictionary();
        });
    }
  }

  function renderDictionary() {
    var el = contentEl();
    if (!glossary) {
      el.innerHTML = '<div class="lib-empty">Loading dictionary...</div>';
      return;
    }
    var terms = glossary.terms || {};
    var cats = glossary.categories || {};
    var q = dictQuery.toLowerCase().trim();
    var keys = Object.keys(terms).filter(function(k) {
      if (!q) return true;
      var t = terms[k];
      return (t.term || k).toLowerCase().indexOf(q) >= 0
        || (t.definition || '').toLowerCase().indexOf(q) >= 0;
    }).sort(function(a, b) {
      return (terms[a].term || a).localeCompare(terms[b].term || b);
    });

    var html = '<input type="text" id="dict-search" placeholder="Search every term..." value="' +
      esc(dictQuery) + '">';
    if (!keys.length) {
      html += '<div class="lib-empty">No matches. Missing a word we should define? ' +
        'Tell us in chat, the dictionary grows from exactly that.</div>';
    } else {
      html += keys.map(function(k) {
        var t = terms[k];
        var catName = cats[t.category] || t.category || '';
        return '<div class="dict-term">' +
          '<div class="dict-word">' + esc(t.term || k) +
            (catName ? ' <span class="dict-def">(' + esc(catName) + ')</span>' : '') +
          '</div>' +
          '<div class="dict-def">' + esc(t.definition || '') + '</div>' +
        '</div>';
      }).join('');
    }
    el.innerHTML = html;

    var search = document.getElementById('dict-search');
    if (search) {
      search.addEventListener('input', function() {
        dictQuery = search.value;
        var pos = search.selectionStart;
        renderDictionary();
        var again = document.getElementById('dict-search');
        if (again) { again.focus(); again.setSelectionRange(pos, pos); }
      });
    }
  }

  document.addEventListener('DOMContentLoaded', function() {
    fetch(MANIFEST_URL, { cache: 'no-cache' })
      .then(function(r) {
        if (!r.ok) throw new Error('HTTP ' + r.status);
        return r.json();
      })
      .then(function(j) {
        manifest = j;
        renderRail();
        // A #slug in the URL wins; otherwise open the first document,
        // matching the native page's default.
        if (!openHashDoc()) {
          var cats = manifest.categories || [];
          for (var ci = 0; ci < cats.length; ci++) {
            if ((cats[ci].docs || []).length) { openDoc(ci, 0); break; }
          }
        }
        window.addEventListener('hashchange', openHashDoc);
      })
      .catch(function(err) {
        console.error('library: could not load ' + MANIFEST_URL, err);
        var rail = document.getElementById('lib-rail');
        if (rail) rail.innerHTML = '<div class="lib-empty">Could not load the library.</div>';
      });
  });
})();
