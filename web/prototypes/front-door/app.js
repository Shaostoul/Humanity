/* The front door PROTOTYPE (docs/design/front-door.md). A decision aid for the operator, not the
   build: it shows the title screen (the mission in a line, what HumanityOS is, one search, "New
   here? Start here", the buttons to the in-depth areas), one row of update cards from
   #announcements (a signed release's card dressed its own way), and Start here's one question.
   No chat slice: the operator, 2026-11-02, "That's fair. We can just exclude the chat on front
   page." (the design, section 4).

   What it reads, all of it read-only:
     - ./config.json, ./destinations.json, ./goals.json, ./search-words.json (this folder; the
       real build would move them to data/front_door/).
     - GET /api/messages?channel=announcements: the relay's existing message history, which
       answers anyone today (docs/design/front-door.md, section 5). If it cannot be reached, the
       nine real posts saved in ./sample-updates.json are shown and the page says they are samples.
     - On the first key in the search box (never on load): /data/how_to/index.json,
       /data/how_to/planned.json, /data/library/index.json, the same files the How-to page and
       the Library read.
     - ./signed-releases.json: which releases carry the signed release manifest, read from the
       releases' files on GitHub on 2026-11-02. It stands in for the `signed` field the real
       endpoint would give each update (the design, section 3.1); the visitor's browser never asks
       GitHub itself.
     - /data/onboarding/quests.json for Start here's steps, with the Onboarding page's own
       progress (localStorage humanity_quest_progress), so a tick here is a tick there.

   Kept on this device, in try/catch so a private window still works: the goal picked
   (hos_front_door_goal). Nothing is sent anywhere.

   For the screenshots: ?theme=light shows the light theme without changing the person's
   setting; ?col=wide tries the 1560 px column; ?measure=1 shows the measurement readout (window,
   column, cards that fit) and measures the last 100 headlines at each card width; ?min=<px>
   tries another narrowest card width. */
(function () {
  'use strict';

  var BASE = '/prototypes/front-door/';
  var P = new URLSearchParams(location.search);
  var MEASURE = P.get('measure') === '1';
  var root = document.documentElement;

  // The light theme for a screenshot, the way settings.js shows it for a person who chose light
  // with the default accent: shell.js's light palette, and no inline accent over it.
  // settings.js applies the stored choice again once the page has loaded, so the theme function
  // itself answers light for this visit (and nothing is written to the person's settings).
  if (P.get('theme') === 'light' && typeof window.hosApplyTheme === 'function') {
    var applyTheme = window.hosApplyTheme;
    window.hosApplyTheme = function () { return applyTheme('light'); };
    var light = function () {
      window.hosApplyTheme();
      ['--accent', '--accent-hover', '--accent-dim'].forEach(function (k) { root.style.removeProperty(k); });
    };
    light();
    document.addEventListener('DOMContentLoaded', function () { setTimeout(light, 0); });
    window.addEventListener('load', function () { setTimeout(light, 0); });
  }
  if (P.get('col') === 'wide') root.setAttribute('data-fd-col', 'wide');

  var T = function (key, vars) { return window.i18n ? window.i18n.t(key, vars) : key; };

  function span(cls, words) {
    var s = document.createElement('span');
    if (cls) s.className = cls;
    if (words != null) s.textContent = words;
    return s;
  }
  function icon(name, size) {
    var s = span('pg-ico');
    s.setAttribute('aria-hidden', 'true');
    if (window.hosIcon) s.innerHTML = window.hosIcon(name, size || 24);
    return s;
  }
  function getJson(url) {
    return fetch(url, { cache: 'no-cache' }).then(function (r) {
      if (!r.ok) throw new Error(url + ': ' + r.status);
      return r.json();
    });
  }
  function store(key, value) {
    try { if (value == null) localStorage.removeItem(key); else localStorage.setItem(key, value); } catch (e) { /* private window */ }
  }
  function stored(key) {
    try { return localStorage.getItem(key); } catch (e) { return null; }
  }

  // The static icons in the markup (the search box's magnifier).
  Array.prototype.forEach.call(document.querySelectorAll('.pg-ico[data-icon]'), function (el) {
    if (window.hosIcon) el.innerHTML = window.hosIcon(el.dataset.icon, parseInt(el.dataset.size, 10) || 20);
  });

  var cfg = null;

  // ── The title screen's buttons (destinations.json) ──
  function drawMenu(list) {
    var nav = document.getElementById('fd-menu');
    list.forEach(function (d) {
      var a = document.createElement('a');
      a.className = 'fd-tile';
      a.href = d.href;
      i18n.hint(a, d.hint);
      a.appendChild(icon(d.icon, 26));
      var words = span('');
      words.appendChild(span('fd-tile-name', T(d.name)));
      words.appendChild(span('fd-tile-line', T(d.line)));
      a.appendChild(words);
      nav.appendChild(a);
    });
  }

  // ── The update cards ──
  // A post in #announcements reads "<emoji> **vX.Y.Z**: <headline> · [Release notes](<url>)"
  // (docs/PRIORITIES.md, one post per release since 2026-10-09). A post that does not parse is
  // shown as its plain words, with no version and no link.
  var ANNOUNCE = /\*\*v([0-9][0-9A-Za-z.+-]*)\*\*\s*:\s*([\s\S]*?)\s*(?:·\s*)?\[[^\]]*\]\(<?([^)>\s]+)>?\)\s*$/;
  function parseUpdate(m) {
    var hit = ANNOUNCE.exec(String(m.content || ''));
    var plain = String(m.content || '').replace(/\*\*/g, '').replace(/\[([^\]]*)\]\([^)]*\)/g, '$1').trim();
    var url = hit && hit[3];
    // Only a link to this project's own release page: an announcement is posted with the bot
    // secret, and the front door must not become a way to put any link before every visitor.
    if (url && url.indexOf(cfg.release_notes_prefix) !== 0) url = null;
    return { version: hit ? hit[1] : null, title: hit ? hit[2] : plain, url: url, at: m.timestamp };
  }

  var updates = [];
  var shownCount = -1;
  // Version -> when it was signed, from ./signed-releases.json (see the header comment).
  var signed = {};
  function cardFor(u) {
    var c = document.createElement('article');
    var isSigned = !!(u.version && signed[u.version]);
    c.className = 'fd-card' + (isSigned ? ' fd-signed' : '');
    c.setAttribute('role', 'listitem');
    var head = span('fd-card-head');
    head.appendChild(span('fd-card-ver', u.version ? 'v' + u.version : ''));
    var d = new Date(u.at);
    var sameYear = d.getFullYear() === new Date().getFullYear();
    head.appendChild(span('fd-card-date', d.toLocaleDateString(undefined, sameYear ? { month: 'short', day: 'numeric' } : { year: 'numeric', month: 'short', day: 'numeric' })));
    c.appendChild(head);
    var title = document.createElement('p');
    title.className = 'fd-card-title';
    title.textContent = u.title;
    title.title = u.title;
    c.appendChild(title);
    var foot = span('fd-card-foot');
    if (u.url) {
      var link = document.createElement('a');
      link.className = 'fd-card-link';
      link.href = u.url;
      link.rel = 'noopener';
      link.textContent = T('front_door.update_notes');
      i18n.hint(link, 'front_door.update_notes_hint');
      foot.appendChild(link);
    }
    if (isSigned) {
      // The word as well as the colour; its hint says what signed means for a person, and the
      // keyboard reaches it so the hint shows on focus too.
      var mark = span('fd-signed-mark');
      mark.tabIndex = 0;
      mark.appendChild(icon('shield', 13));
      mark.appendChild(span('', T('front_door.signed')));
      mark.setAttribute('aria-label', T('front_door.signed') + '. ' + T('front_door.signed_hint'));
      i18n.hint(mark, 'front_door.signed_hint');
      foot.appendChild(mark);
    }
    if (foot.firstChild) c.appendChild(foot);
    return c;
  }

  /** How many cards the row shows: as many as fit at card_min_px, at most card_max; below two,
      a swipe row of card_max (docs/design/front-door.md, section 3, the rule). */
  function cardLayout(width) {
    var min = parseInt(P.get('min'), 10) || cfg.card_min_px;
    var gap = cfg.card_gap_px;
    var fit = Math.floor((width + gap) / (min + gap));
    if (fit >= 2) return { swipe: false, n: Math.min(fit, cfg.card_max), min: min };
    return { swipe: true, n: cfg.card_max, min: min, w: Math.min(cfg.swipe_card_px, Math.round(width * 0.82)) };
  }

  function drawCards(force) {
    var row = document.getElementById('fd-cards');
    var width = row.clientWidth;
    var lay = cardLayout(width);
    var n = Math.min(lay.n, updates.length);
    var key = (lay.swipe ? 's' : 'g') + n;
    if (!force && key === shownCount) { measureLine(lay, width); return; }
    shownCount = key;
    row.classList.toggle('fd-swipe', lay.swipe);
    row.style.setProperty('--fd-n', String(Math.max(n, 1)));
    if (lay.swipe) row.style.setProperty('--fd-swipe-w', lay.w + 'px');
    while (row.firstChild) row.removeChild(row.firstChild);
    updates.slice(0, n).forEach(function (u) { row.appendChild(cardFor(u)); });
    measureLine(lay, width);
  }

  function loadUpdates() {
    var want = MEASURE ? 100 : cfg.card_max;
    return getJson(cfg.updates_url + want)
      .then(function (j) { return { list: j.messages || [], sample: false }; })
      .catch(function () { return getJson(BASE + 'sample-updates.json').then(function (j) { return { list: j.messages || [], sample: true }; }); })
      .then(function (got) {
        var all = got.list.map(parseUpdate).sort(function (a, b) { return b.at - a.at; });
        if (!all.length && !got.sample) {
          return getJson(BASE + 'sample-updates.json').then(function (j) {
            return { all: (j.messages || []).map(parseUpdate).sort(function (a, b) { return b.at - a.at; }), sample: true };
          });
        }
        return { all: all, sample: got.sample };
      })
      .then(function (r) {
        updates = r.all.slice(0, cfg.card_max);
        var note = document.getElementById('fd-cards-note');
        if (r.sample) { note.textContent = T('front_door.updates_sample'); note.hidden = false; }
        drawCards(true);
        if (window.ResizeObserver) new ResizeObserver(function () { drawCards(false); }).observe(document.getElementById('fd-cards'));
        if (MEASURE) window.addEventListener('resize', function () { drawCards(false); });
        if (MEASURE) sweep(r.all);
      });
  }

  // ── The search: the How-tos (their problems' signs included) and the Library's titles ──
  var idx = null;
  var idxLoading = null;
  function slugOf(file) { return String(file).replace(/\.md$/i, '').replace(/_/g, '-').toLowerCase(); }
  function loadIndex() {
    if (idxLoading) return idxLoading;
    idxLoading = Promise.all([
      getJson('/data/how_to/index.json'),
      getJson('/data/how_to/planned.json'),
      getJson('/data/library/index.json'),
      getJson(BASE + 'search-words.json')
    ]).then(function (g) {
      var docs = [];
      (function walk(cats, path) {
        (cats || []).forEach(function (c) {
          var here = path.concat([c.name || c.title || '']);
          (c.docs || []).forEach(function (d) { docs.push({ title: d.title, slug: slugOf(d.file), where: here.join(' / '), hay: (d.title + ' ' + (d.tags || []).join(' ') + ' ' + here.join(' ')).toLowerCase() }); });
          walk(c.children, here);
        });
      })(g[2].categories, []);
      idx = {
        howtos: (g[0].how_tos || []).map(function (h) { return { id: h.id, title: h.title, summary: h.summary, hay: (h.search + ' ' + h.title).toLowerCase() }; }),
        planned: (g[1].planned || []).map(function (h) { return { id: h.id, title: h.title, summary: h.summary, hay: (h.title + ' ' + h.summary).toLowerCase() }; }),
        docs: docs,
        same: g[3].same || [],
        ignore: g[3].ignore || []
      };
      return idx;
    });
    return idxLoading;
  }

  /** The query as groups of words, any one of which may match: "throwing up" becomes the vomit
      row, "diarrhea" the diarrhoea row; small words are dropped. Every group must match. */
  function groupsOf(q) {
    var text = ' ' + q.toLowerCase().replace(/[^\p{L}\p{N}'\s-]/gu, ' ').replace(/\s+/g, ' ').trim() + ' ';
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
  function hits(list, groups) {
    if (!groups.length) return [];
    return list.filter(function (e) {
      return groups.every(function (g) { return g.some(function (alt) { return e.hay.indexOf(alt) >= 0; }); });
    }).map(function (e) {
      var t = e.title.toLowerCase();
      var score = groups.filter(function (g) { return g.some(function (alt) { return t.indexOf(alt) >= 0; }); }).length;
      return { e: e, score: score };
    }).sort(function (a, b) { return b.score - a.score; }).map(function (x) { return x.e; });
  }

  function resultLink(href, title, sub, cls, hintKey) {
    var li = document.createElement('li');
    var a = document.createElement('a');
    a.href = href;
    i18n.hint(a, hintKey);
    a.appendChild(span(cls || '', title));
    if (sub) a.appendChild(span('fd-r-sub', sub));
    li.appendChild(a);
    return li;
  }
  function group(box, headKey, items) {
    if (!items.length) return;
    var h = document.createElement('h3');
    h.textContent = T(headKey);
    box.appendChild(h);
    var ul = document.createElement('ul');
    items.forEach(function (li) { ul.appendChild(li); });
    box.appendChild(ul);
  }

  function search(q) {
    var box = document.getElementById('fd-results');
    while (box.firstChild) box.removeChild(box.firstChild);
    q = q.trim();
    if (!q) { box.hidden = true; return; }
    box.hidden = false;
    if (!idx) { box.appendChild(span('fd-r-none', T('front_door.search_loading'))); return; }
    var g = groupsOf(q);
    var how = hits(idx.howtos, g).slice(0, 4);
    var planned = hits(idx.planned, g).slice(0, 2);
    var docs = hits(idx.docs, g).slice(0, 5);
    group(box, 'front_door.search_howtos', how.map(function (h) { return resultLink('/how-to#' + encodeURIComponent(h.id), h.title, h.summary, '', 'front_door.result_hint'); }));
    group(box, 'front_door.search_planned', planned.map(function (h) { return resultLink('/how-to', h.title, h.summary, 'fd-r-planned', 'front_door.planned_hint'); }));
    group(box, 'front_door.search_library', docs.map(function (d) { return resultLink('/library#' + d.slug, d.title, d.where, '', 'front_door.result_hint'); }));
    if (!how.length && !planned.length && !docs.length) box.appendChild(span('fd-r-none', T('front_door.search_none', { q: q })));
    var more = document.createElement('a');
    more.className = 'fd-r-more';
    more.href = '/library?q=' + encodeURIComponent(q);
    more.textContent = T('front_door.search_more', { q: q });
    i18n.hint(more, 'front_door.search_more_hint');
    box.appendChild(more);
  }

  function wireSearch() {
    var input = document.getElementById('fd-q');
    var timer = null;
    input.addEventListener('input', function () {
      clearTimeout(timer);
      var q = input.value;
      if (!idx) loadIndex().then(function () { search(input.value); }).catch(function () { idxLoading = null; });
      timer = setTimeout(function () { search(q); }, 120);
    });
    input.addEventListener('keydown', function (e) {
      if (e.key === 'Escape') { input.value = ''; search(''); }
    });
    // A query in the address (?q=), so a screenshot can show the search answering.
    var start = P.get('q');
    if (start) { input.value = start; loadIndex().then(function () { search(start); }); }
  }

  // ── Start here: what brought you here? ──
  var GOAL_KEY = 'hos_front_door_goal';
  var QUEST_KEY = 'humanity_quest_progress';
  var goals = [];
  var chains = {};
  function firstHref(f) {
    if (f.kind === 'how_to') return '/how-to#' + encodeURIComponent(f.id);
    if (f.kind === 'library') return '/library#' + f.id;
    return f.id;
  }
  function progressOf(chain) {
    var p = {};
    try { p = JSON.parse(stored(QUEST_KEY) || '{}') || {}; } catch (e) { p = {}; }
    var done = 0, next = -1;
    chain.steps.forEach(function (s, i) { if (p[chain.id + ':' + s.id]) done++; else if (next < 0) next = i; });
    return { done: done, next: next, p: p };
  }

  function drawAnswer(goal) {
    var box = document.getElementById('fd-answer');
    while (box.firstChild) box.removeChild(box.firstChild);
    if (!goal) { box.hidden = true; return; }
    box.hidden = false;
    var left = document.createElement('div');
    var h = document.createElement('h3');
    h.textContent = T('front_door.goal_first');
    left.appendChild(h);
    var p = document.createElement('p');
    p.textContent = T(goal.answer);
    left.appendChild(p);
    if (goal.emergency) {
      var em = document.createElement('p');
      em.className = 'fd-emergency';
      em.textContent = T('front_door.goal_emergency');
      left.appendChild(em);
    }
    var open = document.createElement('a');
    open.className = 'fd-open';
    open.href = firstHref(goal.first);
    open.textContent = T(goal.first.label);
    i18n.hint(open, 'front_door.goal_open_hint');
    left.appendChild(open);
    box.appendChild(left);

    var chain = goal.chain && chains[goal.chain];
    box.classList.toggle('fd-one', !chain);
    if (chain) {
      var right = document.createElement('div');
      var h2 = document.createElement('h3');
      h2.textContent = T('front_door.goal_then');
      right.appendChild(h2);
      var st = progressOf(chain);
      var line = document.createElement('p');
      line.appendChild(span('fd-chain-name', chain.title));
      line.appendChild(span('fd-chain-count', st.next < 0 ? T('quests.state_done') : T('quests.step_count', { n: st.next + 1, total: chain.steps.length })));
      right.appendChild(line);
      var meter = span('fd-meter');
      meter.setAttribute('aria-hidden', 'true');
      chain.steps.forEach(function (s) {
        var seg = document.createElement('i');
        if (st.p[chain.id + ':' + s.id]) seg.className = 'done';
        meter.appendChild(seg);
      });
      right.appendChild(meter);
      if (st.next >= 0) {
        var nx = document.createElement('p');
        nx.className = 'fd-chain-next';
        nx.textContent = chain.steps[st.next].title;
        right.appendChild(nx);
      }
      var steps = document.createElement('a');
      steps.className = 'fd-chain-link';
      steps.href = '/onboarding';
      steps.textContent = T('quests.see_chain');
      i18n.hint(steps, 'front_door.goal_steps_hint');
      right.appendChild(steps);
      box.appendChild(right);
    }
  }

  function pick(id, remember) {
    var goal = goals.filter(function (g) { return g.id === id; })[0] || null;
    Array.prototype.forEach.call(document.querySelectorAll('.fd-goal'), function (b) {
      b.setAttribute('aria-pressed', String(!!goal && b.dataset.goal === goal.id));
    });
    if (remember) store(GOAL_KEY, goal ? goal.id : null);
    var back = document.getElementById('fd-back');
    back.hidden = !goal;
    if (goal) document.getElementById('fd-back-words').textContent = T('front_door.start_back', { goal: T(goal.name) });
    drawAnswer(goal);
  }

  function drawGoals(list) {
    goals = list;
    var grid = document.getElementById('fd-goals');
    list.forEach(function (g) {
      var b = document.createElement('button');
      b.type = 'button';
      b.className = 'fd-goal';
      b.dataset.goal = g.id;
      b.setAttribute('aria-pressed', 'false');
      i18n.hint(b, 'front_door.goal_hint');
      b.appendChild(icon(g.icon, 24));
      var words = span('');
      words.appendChild(span('fd-tile-name', T(g.name)));
      words.appendChild(span('fd-tile-line', T(g.line)));
      b.appendChild(words);
      b.addEventListener('click', function () { pick(g.id, true); });
      grid.appendChild(b);
    });
    document.getElementById('fd-change').addEventListener('click', function () { pick(null, true); });
    var saved = P.get('goal') || stored(GOAL_KEY);
    if (saved) pick(saved, false);
  }

  document.getElementById('fd-start').addEventListener('click', function (e) {
    e.preventDefault();
    var reduce = window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    document.getElementById('start').scrollIntoView({ behavior: reduce ? 'auto' : 'smooth', block: 'start' });
    var first = document.querySelector('.fd-goal');
    if (first) first.focus({ preventScroll: true });
    if (history.replaceState) history.replaceState(null, '', '#start');
  });

  // ── Measurement (?measure=1), the evidence for section 3 of the design ──
  function measureLine(lay, width) {
    if (!MEASURE) return;
    var box = document.getElementById('fd-measure');
    box.hidden = false;
    var first = document.querySelector('#fd-cards .fd-card');
    var p = box.querySelector('[data-line]') || box.appendChild(document.createElement('p'));
    p.setAttribute('data-line', '1');
    p.textContent = T('front_door.measure_line', {
      w: window.innerWidth, h: window.innerHeight, col: width,
      n: lay.swipe ? 'swipe' : Math.min(lay.n, updates.length), min: lay.min,
      card: first ? Math.round(first.getBoundingClientRect().width) : 0
    });
    box.setAttribute('data-fit', JSON.stringify({ w: window.innerWidth, h: window.innerHeight, col: width, swipe: lay.swipe, n: lay.n, min: lay.min }));
  }

  /** Each headline at each card width, in the card's own font: how many lines it takes. */
  function sweep(all) {
    var probe = document.createElement('p');
    probe.className = 'fd-card-title';
    probe.style.cssText = 'position:absolute;left:-10000px;top:0;visibility:hidden;display:block;-webkit-line-clamp:unset;min-height:0;overflow:visible;';
    document.body.appendChild(probe);
    var lh = parseFloat(getComputedStyle(probe).lineHeight);
    var rows = [];
    for (var w = 160; w <= 360; w += 20) {
      probe.style.width = (w - 30) + 'px';   // the card's padding (14 + 14) and border (1 + 1)
      var lines = all.map(function (u) { probe.textContent = u.title; return Math.round(probe.getBoundingClientRect().height / lh); }).sort(function (a, b) { return a - b; });
      rows.push({ card: w, max: lines[lines.length - 1], p90: lines[Math.floor(lines.length * 0.9)], over4: lines.filter(function (n) { return n > 4; }).length, over5: lines.filter(function (n) { return n > 5; }).length });
    }
    probe.remove();
    var box = document.getElementById('fd-measure');
    var p = document.createElement('p');
    p.textContent = T('front_door.measure_sweep', { count: all.length, table: rows.map(function (r) { return r.card + ': ' + r.max + '/' + r.p90; }).join(', ') });
    box.appendChild(p);
    box.setAttribute('data-sweep', JSON.stringify(rows));
  }

  // ── Start ──
  getJson(BASE + 'config.json').then(function (c) {
    cfg = c;
    wireSearch();
    return Promise.all([
      getJson(BASE + 'destinations.json').then(function (d) { drawMenu(d.destinations || []); }),
      // Which releases are signed, then the cards (a card is dressed once, when it is drawn).
      getJson(BASE + 'signed-releases.json')
        .then(function (j) { (j.signed || []).forEach(function (s) { signed[s.version] = s.signed_at; }); })
        .catch(function () { /* none known: every card is drawn plain */ })
        .then(loadUpdates),
      Promise.all([getJson(BASE + 'goals.json'), getJson('/data/onboarding/quests.json').catch(function () { return { chains: [] }; })])
        .then(function (g) { (g[1].chains || []).forEach(function (c2) { chains[c2.id] = c2; }); drawGoals(g[0].goals || []); })
    ]);
  }).then(function () {
    if (window.i18n) window.i18n.apply(document.querySelector('.fd'));
    if (location.hash === '#start') document.getElementById('start').scrollIntoView();
    document.documentElement.setAttribute('data-fd-ready', '1');
  }).catch(function (e) { console.error('front door prototype:', e); });
})();
