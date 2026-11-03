/* The front door's rules and its update card (docs/design/front-door.md, 2026-11-02), shared by the
   front page (web/pages/index-app.js) and the Devlog (web/pages/devlog-app.js), and run by their
   tests under Node (scripts/tests/front-door-web.test.js).

   - cardLayout: how many update cards a row shows at a width (design section 3, the rule): as many
     as fit at card_min_px, at most card_max; below two side by side, a swipe row of card_max.
   - The search over the How-tos and the Library's titles, in plain words (section 7.4):
     data/front_door/search_words.json says which words mean the same and which small words to drop.
   - Start here's helpers (section 7.3): where a goal's first thing opens, how far its quest chain is.
   - updateCard: one release's card, a signed release dressed its own way (section 3.1): the theme's
     verified colour on its top band, a faint wash of it, and a "Signed" mark with a shield. Its
     styles are web/shared/update-card.css; theme tokens only.

   The cards' data comes from this server alone (/api/front-door, /api/releases, src/relay/front_door.rs),
   so a visitor's address never goes to GitHub. The server already keeps only a link to the
   release's own page; isReleasePage checks it again here, so a card can never carry another address
   even from a server that does not filter.

   Works as a browser script (window.HosFrontDoor) and as a Node module (module.exports). No
   dependencies: the page hands in its words (t), hints, icons and document. */
(function (root) {
  'use strict';

  var RELEASE_PAGE_PREFIX = 'https://github.com/Shaostoul/Humanity/releases/tag/';

  /** Whether `url` is exactly `version`'s own release page. */
  function isReleasePage(url, version) {
    return typeof url === 'string' && /^[0-9]+(\.[0-9]+)+$/.test(String(version || '')) && url === RELEASE_PAGE_PREFIX + 'v' + version;
  }

  /** How many cards the row shows in a row `width` CSS pixels wide, by data/front_door/config.json. */
  function cardLayout(width, cfg) {
    var min = cfg.card_min_px;
    var gap = cfg.card_gap_px;
    var fit = Math.floor((width + gap) / (min + gap));
    if (fit >= 2) return { swipe: false, n: Math.min(fit, cfg.card_max) };
    return { swipe: true, n: cfg.card_max, w: Math.min(cfg.swipe_card_px, Math.round(width * 0.82)) };
  }

  /* ── The search ── */

  function slugOf(file) { return String(file).replace(/\.md$/i, '').replace(/_/g, '-').toLowerCase(); }

  /** The search's index from the four files it reads on the first key: the How-to index, the planned
      list, the Library's manifest (titles and tags, never its full-text index) and the plain words. */
  function buildIndex(howIndex, planned, library, words) {
    var docs = [];
    (function walk(cats, trail) {
      (cats || []).forEach(function (c) {
        var here = trail.concat([c.name || c.title || '']);
        (c.docs || []).forEach(function (d) {
          docs.push({ title: d.title, slug: slugOf(d.file), where: here.join(' / '), hay: (d.title + ' ' + (d.tags || []).join(' ') + ' ' + here.join(' ')).toLowerCase() });
        });
        walk(c.children, here);
      });
    })(library && library.categories, []);
    return {
      howtos: ((howIndex && howIndex.how_tos) || []).map(function (h) { return { id: h.id, title: h.title, summary: h.summary, hay: ((h.search || '') + ' ' + h.title).toLowerCase() }; }),
      planned: ((planned && planned.planned) || []).map(function (h) { return { id: h.id, title: h.title, summary: h.summary, hay: (h.title + ' ' + (h.summary || '')).toLowerCase() }; }),
      docs: docs,
      same: (words && words.same) || [],
      ignore: (words && words.ignore) || []
    };
  }

  /** The query as groups of words, any one of which may match: "throwing up" becomes the vomit row,
      "diarrhea" the diarrhoea row; the small words are dropped. Every group must match. */
  function groupsOf(q, idx) {
    var text = ' ' + String(q).toLowerCase().replace(/[^\p{L}\p{N}'\s-]/gu, ' ').replace(/\s+/g, ' ').trim() + ' ';
    var groups = [];
    var phrases = [];
    idx.same.forEach(function (row) { row.forEach(function (p) { if (p.indexOf(' ') >= 0) phrases.push({ p: p, row: row }); }); });
    phrases.sort(function (a, b) { return b.p.length - a.p.length; });
    phrases.forEach(function (x) {
      if (text.indexOf(' ' + x.p + ' ') >= 0) { groups.push(x.row); text = text.replace(' ' + x.p + ' ', ' '); }
    });
    text.trim().split(' ').forEach(function (w) {
      if (w.length < 2 || idx.ignore.indexOf(w) >= 0) return;
      var alts = [w];
      idx.same.forEach(function (row) { if (row.indexOf(w) >= 0) alts = alts.concat(row); });
      groups.push(alts);
    });
    return groups;
  }

  /** The entries of `list` every group matches, the ones matching most groups in their title first. */
  function hits(list, groups) {
    if (!groups.length) return [];
    return list.filter(function (e) {
      return groups.every(function (g) { return g.some(function (alt) { return e.hay.indexOf(alt) >= 0; }); });
    }).map(function (e, i) {
      var t = e.title.toLowerCase();
      var score = groups.filter(function (g) { return g.some(function (alt) { return t.indexOf(alt) >= 0; }); }).length;
      return { e: e, score: score, i: i };
    }).sort(function (a, b) { return b.score - a.score || a.i - b.i; }).map(function (x) { return x.e; });
  }

  /** Everything a query finds: up to 4 How-tos, 2 planned ones, 5 Library documents. */
  function search(idx, q) {
    var g = groupsOf(q, idx);
    return { howtos: hits(idx.howtos, g).slice(0, 4), planned: hits(idx.planned, g).slice(0, 2), docs: hits(idx.docs, g).slice(0, 5) };
  }

  /* ── Start here ── */

  /** Where a goal's first thing opens. */
  function firstHref(first) {
    if (first.kind === 'how_to') return '/how-to#' + encodeURIComponent(first.id);
    if (first.kind === 'library') return '/library#' + first.id;
    return first.id;
  }

  /** How far a quest chain is, from the Onboarding page's own progress (humanity_quest_progress):
      steps done, the index of the next step (-1 when all are done). */
  function progressOf(chain, progress) {
    var p = progress || {};
    var done = 0;
    var next = -1;
    chain.steps.forEach(function (s, i) { if (p[chain.id + ':' + s.id]) done++; else if (next < 0) next = i; });
    return { done: done, next: next };
  }

  /* ── The update card ── */

  /** "Nov 2", or "Nov 2, 2025" when it is not this year. */
  function dateText(at, now) {
    var d = new Date(at);
    var sameYear = d.getFullYear() === new Date(now == null ? Date.now() : now).getFullYear();
    return d.toLocaleDateString(undefined, sameYear ? { month: 'short', day: 'numeric' } : { year: 'numeric', month: 'short', day: 'numeric' });
  }

  /** One release's card. `u` is an entry of the server's answer ({ version, title, url, at, signed });
      `deps` = { document, t(key), hint(el, key), icon(name, size) -> svg markup or '', now }. Built
      with textContent only, so nothing the server says can become markup. */
  function updateCard(u, deps) {
    var doc = deps.document;
    var T = deps.t;
    function el(tag, cls, words) {
      var e = doc.createElement(tag);
      if (cls) e.className = cls;
      if (words != null) e.textContent = words;
      return e;
    }
    var signed = u.signed === true;
    var card = el('article', 'update-card' + (signed ? ' update-card-signed' : ''));
    card.setAttribute('role', 'listitem');
    var head = el('div', 'update-card-head');
    head.appendChild(el('span', 'update-card-ver', u.version ? 'v' + u.version : ''));
    head.appendChild(el('span', 'update-card-date', u.at ? dateText(u.at, deps.now) : ''));
    card.appendChild(head);
    var title = el('p', 'update-card-title', u.title || '');
    title.setAttribute('title', u.title || '');
    card.appendChild(title);
    var foot = el('div', 'update-card-foot');
    if (isReleasePage(u.url, u.version)) {
      var link = el('a', 'update-card-link', T('front_door.update_notes'));
      link.setAttribute('href', u.url);
      link.setAttribute('rel', 'noopener');
      deps.hint(link, 'front_door.update_notes_hint');
      foot.appendChild(link);
    }
    if (signed) {
      // The word as well as the colour; the keyboard reaches it, so its hint shows on focus too.
      var mark = el('span', 'update-card-signed-mark');
      mark.setAttribute('tabindex', '0');
      var ico = el('span', 'pg-ico');
      ico.setAttribute('aria-hidden', 'true');
      var svg = deps.icon ? deps.icon('shield', 13) : '';
      if (svg && ico.insertAdjacentHTML) ico.insertAdjacentHTML('beforeend', svg);
      mark.appendChild(ico);
      mark.appendChild(el('span', '', T('front_door.signed')));
      mark.setAttribute('aria-label', T('front_door.signed') + '. ' + T('front_door.signed_hint'));
      deps.hint(mark, 'front_door.signed_hint');
      foot.appendChild(mark);
    }
    if (foot.childNodes.length) card.appendChild(foot);
    return card;
  }

  var api = {
    RELEASE_PAGE_PREFIX: RELEASE_PAGE_PREFIX, isReleasePage: isReleasePage, cardLayout: cardLayout,
    slugOf: slugOf, buildIndex: buildIndex, groupsOf: groupsOf, hits: hits, search: search,
    firstHref: firstHref, progressOf: progressOf, dateText: dateText, updateCard: updateCard
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else root.HosFrontDoor = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
