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
  var SEARCH_URL = '/data/library/search-index.json';
  var SYLLABUS_URL = '/data/curriculum/syllabus.json';

  var manifest = null;
  var glossary = null;
  var current = null;       // {ci, di} of the open doc, or 'dictionary'
  var docCache = {};        // file -> markdown text
  var dictQuery = '';
  var tagFilter = null;     // active tag id, or null for everything
  var searchIndex = null;   // lazy: fetched on the first keystroke, never on load
  var searchLoading = false;
  var searchQuery = '';
  var syllabus = null;      // fetched in the background at load; 10 KB gzipped
  var curFilter = '';       // '' | written | absent | lethal | locale

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
  /* A fragment is either `<doc>` or `<doc>/<heading>`. The second form is what
     lets one document link into the middle of another instead of dropping the
     reader at the top of a long file to hunt. Native parses the same grammar in
     src/gui/pages/library.rs. */
  function splitFragment(frag) {
    var at = String(frag || '').indexOf('/');
    if (at < 0) return { slug: frag, anchor: null };
    return { slug: frag.slice(0, at), anchor: frag.slice(at + 1) };
  }
  function openHashDoc() {
    // Two reserved fragments that are VIEWS rather than documents, matching
    // the native resolver, so a link can point at the map or at the words
    // without either having to become a fake .md file.
    var frag = location.hash.replace(/^#/, '');
    if (frag === 'curriculum') { openCurriculum(); return true; }
    if (frag === 'dictionary') { openDictionary(); return true; }
    var parts = splitFragment(location.hash.replace(/^#/, ''));
    var hit = findBySlug(parts.slug);
    if (hit) openDoc(hit.ci, hit.di, true, parts.anchor);
    return !!hit;
  }

  /* ── Tags ──
     Tags cross-cut the categories: a document sits in exactly one category but
     carries as many tags as apply, which is the only way to ask for "everything
     safety-critical" across shelves. Vocabulary comes from data/library/tags.json
     via the manifest, so web and native filter on identical ids. */

  function docHasTag(doc, tag) {
    if (!tag) return true;
    var tags = doc.tags || [];
    for (var i = 0; i < tags.length; i++) if (tags[i] === tag) return true;
    return false;
  }

  /** Human label for a tag id, falling back to the id so an unregistered tag is
      visible rather than silently dropped. */
  function tagLabel(id) {
    var groups = (manifest && manifest.tags) || [];
    for (var g = 0; g < groups.length; g++) {
      var ts = groups[g].tags || [];
      for (var i = 0; i < ts.length; i++) if (ts[i].id === id) return ts[i].label || id;
    }
    return id;
  }

  function setTagFilter(tag) {
    tagFilter = (tagFilter === tag) ? null : tag;   // clicking the active chip clears it
    renderTagBar();
    renderRail();
  }

  function renderTagBar() {
    var bar = document.getElementById('lib-tags');
    if (!bar) return;
    var groups = (manifest && manifest.tags) || [];
    if (!groups.length) { bar.hidden = true; return; }
    bar.hidden = false;

    var html = groups.map(function(g) {
      var chips = (g.tags || []).map(function(t) {
        var on = tagFilter === t.id;
        return '<button class="lib-tag' + (on ? ' active' : '') + '" data-tag="' +
          esc(t.id) + '" aria-pressed="' + (on ? 'true' : 'false') + '">' +
          esc(t.label || t.id) + '</button>';
      }).join('');
      if (!chips) return '';
      return '<div class="lib-tag-group"><span class="lib-tag-label">' +
        esc(g.label || '') + '</span>' + chips + '</div>';
    }).join('');

    if (tagFilter) {
      html += '<div class="lib-tag-group"><button class="lib-tag" data-clear="1">' +
        'Clear filter</button></div>';
    }
    bar.innerHTML = html;

    bar.querySelectorAll('[data-tag]').forEach(function(b) {
      b.addEventListener('click', function() { setTagFilter(b.getAttribute('data-tag')); });
    });
    var clear = bar.querySelector('[data-clear]');
    if (clear) clear.addEventListener('click', function() { tagFilter = null; renderTagBar(); renderRail(); });
  }

  /* ── Document search ──
     The index carries full text so a result can show a real snippet instead of
     just a title, which costs about 800 KB. So it is LAZY: nothing is fetched
     until the first keystroke, and nobody who only browses ever pays for it. */

  function loadSearchIndex(then) {
    if (searchIndex || searchLoading) { if (searchIndex) then(); return; }
    searchLoading = true;
    renderResults();   // show the loading note
    fetch(SEARCH_URL, { cache: 'no-cache' })
      .then(function(r) { if (!r.ok) throw new Error('HTTP ' + r.status); return r.json(); })
      .then(function(j) { searchIndex = j.docs || []; searchLoading = false; then(); })
      .catch(function(err) {
        console.error('library: search index failed to load', err);
        searchIndex = []; searchLoading = false; then();
      });
  }

  /** Score a document against the query terms, and find where the best hit is.
      Title and heading matches outrank body matches, because someone searching
      "botulism" wants the section about botulism, not the doc that mentions it
      once in passing. */
  function searchDocs(q) {
    var terms = q.toLowerCase().split(/\s+/).filter(function(t) { return t.length > 1; });
    if (!terms.length) return [];
    var out = [];
    (searchIndex || []).forEach(function(d) {
      var hay = d.text.toLowerCase();
      var title = (d.title || '').toLowerCase();
      var score = 0, missing = false;
      terms.forEach(function(t) {
        var inTitle = title.indexOf(t) >= 0;
        var inHead = d.headings.some(function(h) { return h.toLowerCase().indexOf(t) >= 0; });
        var inBody = hay.indexOf(t) >= 0;
        var inTag = (d.tags || []).some(function(x) { return x.indexOf(t) >= 0; });
        if (!inTitle && !inHead && !inBody && !inTag) { missing = true; return; }
        if (inTitle) score += 10;
        if (inHead) score += 5;
        if (inTag) score += 3;
        if (inBody) score += 1;
      });
      if (missing || !score) return;   // every term must appear somewhere

      // Which heading is the hit under? Walk headings and pick the last one
      // that precedes the first body match.
      var first = hay.indexOf(terms[0]);
      var where = '';
      if (first >= 0) {
        var best = -1;
        d.headings.forEach(function(h) {
          var at = hay.indexOf(h.toLowerCase());
          if (at >= 0 && at <= first && at > best) { best = at; where = h; }
        });
      }
      out.push({ doc: d, score: score, snippet: snippetFor(d.text, terms[0]), where: where });
    });
    return out.sort(function(a, b) { return b.score - a.score; }).slice(0, 25);
  }

  function snippetFor(text, term) {
    var at = text.toLowerCase().indexOf(term);
    if (at < 0) return text.slice(0, 160) + '...';
    var start = Math.max(0, at - 70);
    var raw = (start > 0 ? '...' : '') + text.slice(start, at + 110) + '...';
    // Escape first, then re-introduce the one tag we want, so a document
    // containing markup cannot inject anything.
    var safe = esc(raw);
    // Escape every non-alphanumeric so a term like "C++" or "3.5" cannot become
    // a regex. Broad on purpose: over-escaping is harmless, under-escaping throws.
    var esc2 = term.replace(/[^a-z0-9 ]/gi, function(c) { return '\\' + c; });
    var re = new RegExp('(' + esc2 + ')', 'ig');
    return safe.replace(re, '<mark>$1</mark>');
  }

  function renderResults() {
    var box = document.getElementById('lib-results');
    var tree = document.getElementById('lib-rail');
    if (!box) return;
    if (!searchQuery) {
      box.hidden = true; box.innerHTML = '';
      if (tree) tree.hidden = false;
      return;
    }
    box.hidden = false;
    if (tree) tree.hidden = true;   // results replace the tree while searching

    if (searchLoading) {
      box.innerHTML = '<div class="lib-search-note">Loading the index...</div>';
      return;
    }
    var hits = searchDocs(searchQuery);
    if (!hits.length) {
      box.innerHTML = '<div class="lib-search-note">Nothing matches "' + esc(searchQuery) + '".</div>';
      return;
    }
    box.innerHTML = '<div class="lib-search-note">' + hits.length +
      (hits.length === 25 ? '+' : '') + ' result' + (hits.length === 1 ? '' : 's') + '</div>' +
      hits.map(function(h) {
        // The index already knew which section the match was in and could only
        // open the document at the top, which on the Constitution is not an
        // answer. Carry the heading's anchor through the click.
        // A match under the document's own title is a match at the top, so
        // carry no anchor rather than a redundant one that would put
        // #storing-water-safely/storing-water-safely in the address bar.
        var anchor = (h.where && h.where !== h.doc.title
                      && window.hosMarkdown && window.hosMarkdown.headingSlug)
          ? window.hosMarkdown.headingSlug(h.where) : '';
        return '<button class="lib-result" data-slug="' + esc(h.doc.slug) + '"' +
            (anchor ? ' data-anchor="' + esc(anchor) + '"' : '') + '>' +
          '<div class="lib-result-title">' + esc(h.doc.title) +
            (h.where && h.where !== h.doc.title
              ? ' <span class="lib-result-where">&rsaquo; ' + esc(h.where) + '</span>' : '') +
          '</div>' +
          '<div class="lib-result-snip">' + h.snippet + '</div>' +
        '</button>';
      }).join('');

    box.querySelectorAll('[data-slug]').forEach(function(b) {
      b.addEventListener('click', function() {
        var hit = findBySlug(b.getAttribute('data-slug'));
        if (hit) openDoc(hit.ci, hit.di, true, b.getAttribute('data-anchor') || null);
      });
    });
  }

  /* ── Keyboard ──
     A library you can only use with a mouse is a library you use slowly. The
     shortcuts are the ones every search surface has, so nobody has to learn
     them: "/" to search, Escape to get out, arrows through the results, Enter
     to open. Native carries the same two in src/gui/pages/library.rs. */

  var resultCursor = -1;   // which result is highlighted, -1 for none

  function moveResultCursor(delta) {
    var box = document.getElementById('lib-results');
    if (!box || box.hidden) return false;
    var items = box.querySelectorAll('[data-slug]');
    if (!items.length) return false;
    resultCursor += delta;
    if (resultCursor < 0) resultCursor = items.length - 1;
    if (resultCursor >= items.length) resultCursor = 0;
    items.forEach(function(el, i) { el.classList.toggle('lib-result-on', i === resultCursor); });
    items[resultCursor].scrollIntoView({ block: 'nearest' });
    return true;
  }

  function openHighlightedResult() {
    var box = document.getElementById('lib-results');
    if (!box || box.hidden) return false;
    var items = box.querySelectorAll('[data-slug]');
    // Enter with nothing highlighted opens the top hit, which is what a reader
    // who typed a query and hit Enter almost always meant.
    var pick = items[resultCursor >= 0 ? resultCursor : 0];
    if (!pick) return false;
    pick.click();
    return true;
  }

  function wireSearch() {
    var input = document.getElementById('lib-search');
    if (!input) return;
    input.addEventListener('input', function() {
      searchQuery = input.value.trim();
      resultCursor = -1;
      if (!searchQuery) { renderResults(); return; }
      loadSearchIndex(renderResults);
      if (searchIndex) renderResults();
    });
    input.addEventListener('keydown', function(ev) {
      if (ev.key === 'Escape') {
        input.value = ''; searchQuery = ''; resultCursor = -1; renderResults(); input.blur();
      } else if (ev.key === 'ArrowDown') {
        if (moveResultCursor(1)) ev.preventDefault();
      } else if (ev.key === 'ArrowUp') {
        if (moveResultCursor(-1)) ev.preventDefault();
      } else if (ev.key === 'Enter') {
        if (openHighlightedResult()) ev.preventDefault();
      }
    });

    document.addEventListener('keydown', function(ev) {
      // Never steal a key from somebody typing, and never from a shortcut that
      // already means something to the browser.
      var t = ev.target;
      var typing = t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable);
      if (typing || ev.ctrlKey || ev.metaKey || ev.altKey) return;
      if (ev.key === '/') { ev.preventDefault(); input.focus(); input.select(); }
    });
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

    // Three tiers: section > category > document. Group the categories by the
    // section the manifest assigned, in the manifest's declared order, so web
    // and native present the same shape.
    var order = (manifest.sections || []).slice();
    cats.forEach(function(c) {
      if (c.section && order.indexOf(c.section) < 0) order.push(c.section);
    });
    if (!order.length) order = [null];   // older manifest: one unnamed group

    var html = '';
    var shown = 0;
    order.forEach(function(sectionName) {
      var inSection = cats
        .map(function(cat, ci) { return { cat: cat, ci: ci }; })
        .filter(function(x) {
          return sectionName === null ? true : x.cat.section === sectionName;
        });
      if (!inSection.length) return;

      // Only draw the section header once we know it has a visible document
      // under it, which an active tag filter can easily make false.
      var sectionHtml = '';
      var sectionShown = 0;
      inSection.forEach(function(x) {
        var r = renderCategory(x.cat, x.ci);
        sectionHtml += r.html;
        sectionShown += r.count;
      });
      if (!sectionShown) return;
      shown += sectionShown;
      if (sectionName) {
        html += '<div class="lib-section">' + esc(sectionName) + '</div>';
      }
      html += sectionHtml;
    });

    // Returns its own markup and count rather than mutating the outer state,
    // so the section pass above can decide whether a section has anything
    // visible in it BEFORE drawing that section's header.
    function renderCategory(cat, ci) {
      var docs = cat.docs || [];
      // Keep each doc's real index so openDoc(ci, di) still addresses the
      // unfiltered manifest; filtering must not renumber anything.
      var visible = docs.map(function(d, di) { return { d: d, di: di }; })
                        .filter(function(x) { return docHasTag(x.d, tagFilter); });
      if (!visible.length) return { html: '', count: 0 };
      return {
        count: visible.length,
        html: '<div class="lib-cat">' +
          '<div class="lib-cat-head" role="button" tabindex="0" data-cat="' + ci + '">' +
            '<span class="lib-cat-arrow" id="lib-arrow-' + ci + '">&#9660;</span>' +
            '<span>' + esc(cat.name) + '</span>' +
          '</div>' +
          '<div class="lib-cat-docs" id="lib-docs-' + ci + '">' +
            visible.map(function(x) {
              var active = current && current.ci === ci && current.di === x.di;
              return '<button class="lib-doc' + (active ? ' active' : '') +
                '" data-ci="' + ci + '" data-di="' + x.di + '">' + esc(x.d.title) + '</button>';
            }).join('') +
          '</div>' +
        '</div>',
      };
    }
    if (!shown && tagFilter) {
      html += '<div class="lib-empty">No documents carry that tag yet.</div>';
    }

    html += '<div class="lib-special">' +
      '<button class="lib-doc' + (current === 'dictionary' ? ' active' : '') +
      '" data-dict="1" style="padding-left:0;font-weight:600;">Dictionary</button>' +
      '<button class="lib-doc' + (current === 'curriculum' ? ' active' : '') +
      '" data-curriculum="1" style="padding-left:0;font-weight:600;">What there is to learn</button>' +
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
        openDoc(+b.getAttribute('data-ci'), +b.getAttribute('data-di'), true);
      });
    });
    var dictBtn = rail.querySelector('[data-dict]');
    if (dictBtn) dictBtn.addEventListener('click', openDictionary);
    var curBtn = rail.querySelector('[data-curriculum]');
    if (curBtn) curBtn.addEventListener('click', openCurriculum);
  }

  /**
   * Put the reader in front of the document they asked for.
   *
   * The page is a rail plus a content pane. On a WIDE screen those sit side by
   * side and the content is already at the top, so nothing needs to move. On a
   * NARROW screen the flex direction is column, so the rail stacks ABOVE the
   * content, and on this site that rail is every category and every document
   * title: about 2565px of it. Opening a document left the window at scrollY 0,
   * so the reader landed on the table of contents and had to scroll nearly three
   * screens to reach the thing they clicked.
   *
   * That hit deep links hardest, because /library#your-first-tomato is a link
   * someone follows from outside expecting to LAND on the article, but it hit an
   * ordinary tap in the rail exactly the same way.
   *
   * Detects the stacked case from geometry rather than a width breakpoint, so it
   * stays correct if the CSS breakpoint moves.
   */
  function revealReader() {
    var rail = document.getElementById('lib-rail');
    var el = contentEl();
    if (!rail || !el) return;
    var rr = rail.getBoundingClientRect();
    var cr = el.getBoundingClientRect();
    // Side by side: the content starts at or above the rail's bottom edge.
    if (cr.top < rr.bottom - 1) return;
    // Stacked. Only move if the content is not already the thing on screen.
    if (cr.top > 8) {
      el.scrollIntoView({ block: 'start' });
    }
  }

  /* ── Reader ── */
  /** The open document's own tags, clickable so a reader who likes this doc can
      find its siblings in one tap. Mirrors the native doc pane. */
  function docTagsHtml(doc) {
    var tags = doc.tags || [];
    if (!tags.length || !((manifest && manifest.tags) || []).length) return '';
    return '<div class="lib-doc-tags">' + tags.map(function(id) {
      var on = tagFilter === id;
      return '<button class="lib-tag' + (on ? ' active' : '') + '" data-doctag="' +
        esc(id) + '" aria-pressed="' + (on ? 'true' : 'false') + '">' +
        esc(tagLabel(id)) + '</button>';
    }).join('') + '</div>';
  }

  function bindDocTags(el) {
    el.querySelectorAll('[data-doctag]').forEach(function(b) {
      b.addEventListener('click', function() { setTagFilter(b.getAttribute('data-doctag')); });
    });
  }

  /**
   * @param reveal true when the reader ASKED for this document (a deep link, or
   *   a tap in the rail). A bare /library opens the first document by default,
   *   matching the native page, and that is a browsing visitor who wants the
   *   index: scrolling them into "Credits and Thanks" would be worse than the
   *   bug this fixes.
   */
  /* ── Contents ──
     A document the length of the Constitution or SELF-HOSTING had exactly one
     way in: scroll. The outline is a <details> so it costs a reader who does not
     want it one line, and the browser remembers nothing, which is deliberate:
     it reopens closed on each document rather than carrying a decision made
     about a different file. Below three headings there is no outline to draw. */
  function tocHtml(text) {
    var hs = (window.hosMarkdown && window.hosMarkdown.headings)
      ? window.hosMarkdown.headings(text) : [];
    if (hs.length < 3) return '';
    return '<details class="lib-toc"><summary>Contents (' + hs.length + ' sections)</summary>' +
      hs.map(function(h) {
        return '<a class="lib-toc-h' + h.level + '" href="#" data-anchor="' + esc(h.slug) + '">' +
          esc(h.text) + '</a>';
      }).join('') + '</details>';
  }

  /** Put a rendered document on screen, wire its outline, and jump to `anchor`
      if one was asked for. Shared by the cached and the fetched path so the two
      cannot drift. */
  /** What this document is FOR, from the syllabus: which real-life topic it
      teaches. A guide on a shelf does not say which question it answers; the
      curriculum does, and this is where the two meet. Silent until the
      syllabus has been fetched, which happens when the Curriculum view is
      opened, so a reader who never looks at the map pays nothing. */
  function teachesHtml(doc) {
    if (!syllabus || !doc) return '';
    var slug = slugOf(doc.file);
    var hits = (syllabus.topics || []).filter(function(t) {
      return (t.reading || []).indexOf(slug) >= 0;
    });
    if (!hits.length) return '';
    return '<div class="cur-teaches"><span class="cur-teaches-label">Teaches</span>' +
      hits.map(function(t) {
        var subj = (syllabus.subjects || []).filter(function(s) { return s.id === t.subject; })[0];
        return esc(t.title) + ' (' + esc(subj ? subj.title : t.subject) + ')' +
          (t.hazard === 'lethal'
            ? '<span class="cur-lethal">&#9888; can kill you if taught wrong</span>' : '');
      }).join(', ') + '</div>';
  }

  function paintDoc(el, doc, text, anchor) {
    el.innerHTML = docTagsHtml(doc) + teachesHtml(doc) + tocHtml(text) +
      '<div class="md-viewer">' + md(text) + '</div>';
    bindDocTags(el);
    el.querySelectorAll('.lib-toc [data-anchor]').forEach(function(a) {
      a.addEventListener('click', function(ev) {
        ev.preventDefault();
        scrollToAnchor(el, a.getAttribute('data-anchor'));
      });
    });
    el.scrollTop = 0;
    if (anchor) scrollToAnchor(el, anchor);
  }

  /* Scroll inside the reader pane, not the window. The pane is the scrolling
     element, so scrollIntoView on the window would move the page and leave the
     heading where it was. */
  function scrollToAnchor(el, anchor) {
    if (!anchor) return;
    var target = el.querySelector('[id="' + String(anchor).replace(/"/g, '') + '"]');
    if (!target) return;
    el.scrollTop = target.offsetTop - el.offsetTop - 8;
    target.classList.add('lib-jumped');
    setTimeout(function() { target.classList.remove('lib-jumped'); }, 1200);
  }

  function openDoc(ci, di, reveal, anchor) {
    var cat = (manifest.categories || [])[ci];
    var doc = cat && (cat.docs || [])[di];
    if (!doc) return;
    var wasFirst = current === null;
    current = { ci: ci, di: di };
    // PUSH rather than replace, so the browser Back button steps back through
    // the documents you actually read. replaceState kept the address bar
    // shareable but left one history entry for the whole Library, which meant
    // Back threw you out of it entirely: following a cross-reference was a
    // one-way trip. The very first document is a replace, so arriving at
    // /library does not need two Backs to leave.
    var hash = '#' + slugOf(doc.file) + (anchor ? '/' + anchor : '');
    if (history.pushState && !wasFirst && location.hash !== hash) {
      history.pushState(null, '', hash);
    } else if (history.replaceState) {
      history.replaceState(null, '', hash);
    }
    renderRail();

    var el = contentEl();
    if (docCache[doc.file]) {
      paintDoc(el, doc, docCache[doc.file], anchor);
      if (reveal) revealReader();
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
          paintDoc(el, doc, text, anchor);
          if (reveal) revealReader();
        }
      })
      .catch(function(err) {
        console.error('library: could not load ' + doc.file, err);
        el.innerHTML = '<div class="lib-empty">Could not load "' + esc(doc.title) +
          '". Reload the page, or report it on the Bugs page.</div>';
      });
  }

  /* ── Curriculum: the Library's map of ITSELF ──
     data/curriculum/syllabus.json names every subject a person needs and how
     far each topic has got, which is what makes "how complete is this"
     answerable. Showing the gaps is the point: a library that only displays
     what it HAS cannot tell you what it is missing. Mirrors the native view in
     src/gui/pages/library.rs. Lazy, like the search index: nobody who only
     reads a document pays for it. */

  /** Fetch the syllabus once, in the background, and repaint whatever is open
      when it lands.

      NOT lazy, unlike the search index. 60 KB raw and 10 KB over the wire, and
      it is what puts the "Teaches" line on every document. Making it lazy would
      mean the connection between a guide and the real-life topic it answers only
      appeared for readers who had already found the Curriculum view, which is
      backwards: the whole point is that a reader who has NOT found it sees why
      the document exists. The search index is a hundred times bigger and stays
      lazy for exactly that difference in cost. */
  function loadSyllabus() {
    if (syllabus || loadSyllabus.started) return;
    loadSyllabus.started = true;
    fetch(SYLLABUS_URL, { cache: 'no-cache' })
      .then(function(r) { if (!r.ok) throw new Error('HTTP ' + r.status); return r.json(); })
      .then(function(j) { syllabus = j; repaintForSyllabus(); })
      .catch(function(err) {
        console.error('library: syllabus failed to load', err);
        syllabus = { subjects: [], topics: [] };
        repaintForSyllabus();
      });
  }

  function repaintForSyllabus() {
    if (current === 'curriculum') { renderCurriculum(); return; }
    // A document is open and may now have a Teaches line it did not have a
    // moment ago. Repaint it rather than leaving the page half-informed.
    if (current && typeof current === 'object') {
      var cat = (manifest.categories || [])[current.ci];
      var doc = cat && (cat.docs || [])[current.di];
      if (doc && docCache[doc.file]) paintDoc(contentEl(), doc, docCache[doc.file], null);
    }
  }

  function openCurriculum() {
    current = 'curriculum';
    if (history.replaceState) history.replaceState(null, '', '#curriculum');
    renderRail();
    renderCurriculum();
    revealReader();
    loadSyllabus();
  }

  function topicKept(t) {
    if (curFilter === 'written') return t.status !== 'absent';
    if (curFilter === 'absent') return t.status === 'absent';
    if (curFilter === 'lethal') return t.hazard === 'lethal';
    if (curFilter === 'locale') return !!t.locale_dependent;
    return true;
  }

  function renderCurriculum() {
    var el = contentEl();
    if (!el) return;
    if (!syllabus) {
      el.innerHTML = '<div class="lib-empty">Loading what there is to learn...</div>';
      return;
    }
    var topics = syllabus.topics || [];
    var subjects = (syllabus.subjects || []).slice().sort(function(a, b) {
      return (a.order || 0) - (b.order || 0);
    });
    var written = topics.filter(function(t) { return t.status !== 'absent'; }).length;
    var graded = topics.filter(function(t) { return t.status === 'verified'; }).length;

    var chips = [
      ['Everything', ''], ['Written', 'written'], ['Not yet written', 'absent'],
      ['Can kill you if taught wrong', 'lethal'], ['Depends where you are', 'locale'],
    ].map(function(c) {
      return '<button class="lib-tag' + (curFilter === c[1] ? ' active' : '') +
        '" data-curfilter="' + esc(c[1]) + '">' + esc(c[0]) + '</button>';
    }).join('');

    var html = '<h2 class="cur-title">What there is to learn</h2>' +
      '<p class="cur-lede">' + topics.length + ' topics across ' + subjects.length +
      ' subjects. ' + written + ' have something written, ' + graded +
      ' have been checked by a second pass. The rest are named so the gap is' +
      ' countable rather than invisible.</p>' +
      '<div class="lib-tag-group">' + chips + '</div>';

    subjects.forEach(function(subj) {
      var ts = topics.filter(function(t) { return t.subject === subj.id && topicKept(t); });
      if (!ts.length) return;
      var done = ts.filter(function(t) { return t.status !== 'absent'; }).length;
      html += '<details class="cur-subject"' + (done > 0 ? ' open' : '') + '>' +
        '<summary>' + esc(subj.title) + ' <span class="cur-count">' + done + '/' + ts.length +
        '</span></summary>';
      ts.forEach(function(t) {
        var mark = t.status === 'verified' ? '<span class="cur-ok">&#10003;</span>'
          : t.status === 'absent' ? '<span class="cur-gap"></span>'
          : '<span class="cur-part">&middot;</span>';
        var badges = '';
        if (t.hazard === 'lethal') badges += '<span class="cur-lethal">&#9888; can kill you if taught wrong</span>';
        else if (t.hazard === 'serious') badges += '<span class="cur-serious">&#9888; serious</span>';
        if (t.locale_dependent) badges += '<span class="cur-locale">depends where you are</span>';
        var reading = (t.reading || []).map(function(slug) {
          return '<a class="cur-read" href="#' + esc(slug) + '">Read: ' + esc(slug) + '</a>';
        }).join('');
        html += '<div class="cur-topic' + (t.status === 'absent' ? ' cur-topic-gap' : '') + '">' +
          mark + '<div><div class="cur-topic-head">' + esc(t.title) + badges + '</div>' +
          '<div class="cur-summary">' + esc(t.summary || '') + '</div>' + reading + '</div></div>';
      });
      html += '</details>';
    });

    el.innerHTML = html;
    el.scrollTop = 0;
    el.querySelectorAll('[data-curfilter]').forEach(function(b) {
      b.addEventListener('click', function() {
        var v = b.getAttribute('data-curfilter');
        curFilter = (curFilter === v) ? '' : v;
        renderCurriculum();
      });
    });
    el.querySelectorAll('.cur-read').forEach(function(a) {
      a.addEventListener('click', function(ev) {
        ev.preventDefault();
        var hit = findBySlug(a.getAttribute('href').replace(/^#/, ''));
        if (hit) openDoc(hit.ci, hit.di, true);
      });
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
        renderTagBar();
        loadSyllabus();
        wireSearch();
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
