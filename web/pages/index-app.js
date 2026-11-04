/* The front door (web/pages/index.html, docs/design/front-door.md, adopted by the operator on
   2026-11-02: "Let's do it."). A title screen with tools in it: the mission in a line, what
   HumanityOS is, one search, "New here? Start here", the buttons to the in-depth areas, one row of
   update cards from this server (a signed release's card dressed its own way), and Start here's
   one question. No chat on this page (the operator, 2026-11-02: "Let's exclude it from the front
   page. It's not worth the hassle."): the Chat button opens the chat.

   What it reads, all of it read-only:
     - /data/front_door/config.json, destinations.json and goals.json (rows, not code: a button or
       a goal is a line of data) and, for the search, search_words.json.
     - GET /api/front-door (src/relay/front_door.rs): the newest releases, each with `signed`. Once
       per visit, and again only when the tab comes back after config.refresh_after_ms, never on a
       timer. This server only: a visitor's address never goes to GitHub.
     - On the first key in the search box (never on load): /data/how_to/index.json,
       /data/how_to/planned.json and /data/library/index.json (titles and tags, about 90 KB with
       the words file; never the Library's 9.5 MB full-text index). "Search the whole Library"
       hands the words to /library?q=.
     - /data/onboarding/quests.json for Start here's steps, with the Onboarding page's own progress
       (localStorage humanity_quest_progress), so a tick here is a tick there.
   Kept on this device, in try/catch so a private window still works: the goal picked
   (hos_front_door_goal). Nothing is sent anywhere.

   Works in the browser (it starts itself) and under Node for scripts/tests/front-door-web.test.js
   (module.exports.createFrontDoor, with the document, the network and the words handed in). */
(function (root) {
  'use strict';

  var NODE = typeof module === 'object' && module && module.exports;
  var GOAL_KEY = 'hos_front_door_goal';
  var QUEST_KEY = 'humanity_quest_progress';
  var NAME_RE = /^[A-Za-z0-9_-]{1,24}$/;

  /** env = { document, fetch, storage, t, hint, icon, fd (web/shared/front-door.js), location,
      history, scrollTo(el), now(), onWidth(el, f) }. */
  function createFrontDoor(env) {
    var doc = env.document;
    var FD = env.fd;
    var T = env.t;
    var state = { cfg: null, updates: [], layout: null, idx: null, idxLoading: null, goals: [], chains: {}, loadedAt: 0 };

    function byId(id) { return doc.getElementById(id); }
    function el(tag, cls, words) {
      var e = doc.createElement(tag);
      if (cls) e.className = cls;
      if (words != null) e.textContent = words;
      return e;
    }
    function hasClass(e, c) { return (' ' + e.className + ' ').indexOf(' ' + c + ' ') >= 0; }
    function setClass(e, c, on) {
      var list = e.className.split(/\s+/).filter(function (x) { return x && x !== c; });
      if (on) list.push(c);
      e.className = list.join(' ');
    }
    function setVar(e, name, value) {
      if (e.style && typeof e.style.setProperty === 'function') e.style.setProperty(name, value);
      else e.style[name] = value;
    }
    function clear(e) { while (e.firstChild || (e.childNodes && e.childNodes.length)) e.removeChild(e.firstChild || e.childNodes[0]); }
    function icon(name, size) {
      var s = el('span', 'pg-ico');
      s.setAttribute('aria-hidden', 'true');
      var svg = env.icon ? env.icon(name, size) : '';
      if (svg && s.insertAdjacentHTML) s.insertAdjacentHTML('beforeend', svg);
      return s;
    }
    function getJson(url) {
      return env.fetch(url, { cache: 'no-cache' }).then(function (r) {
        if (!r.ok) throw new Error(url + ': ' + r.status);
        return r.json();
      });
    }
    function store(key, value) {
      try { if (value == null) env.storage.removeItem(key); else env.storage.setItem(key, value); } catch (e) { /* private window */ }
    }
    function stored(key) {
      try { return env.storage ? env.storage.getItem(key) : null; } catch (e) { return null; }
    }
    function query() {
      var s = (env.location && env.location.search) || '';
      var out = {};
      s.replace(/^\?/, '').split('&').forEach(function (kv) {
        if (!kv) return;
        var at = kv.indexOf('=');
        var k = decodeURIComponent(at < 0 ? kv : kv.slice(0, at));
        out[k] = at < 0 ? '' : decodeURIComponent(kv.slice(at + 1).replace(/\+/g, ' '));
      });
      return out;
    }

    // ── The welcome step (?welcome=1, from the chat) ──
    function wireWelcome() {
      var q = query();
      var sec = byId('fd-welcome');
      if (!sec || q.welcome !== '1') return;
      sec.hidden = false;
      var input = byId('fd-welcome-name');
      var join = byId('fd-welcome-join');
      var err = byId('fd-welcome-error');
      var saved = stored('humanity_name');
      if (saved) input.value = saved;
      if (input.focus) input.focus();
      function go() {
        var name = String(input.value || '').trim();
        if (!NAME_RE.test(name)) { err.textContent = T('front_door.welcome_error_name'); return; }
        err.textContent = '';
        join.disabled = true;
        join.textContent = T('front_door.welcome_joining');
        // Keep ONLY the chosen name and hand over. The chat makes the identity itself, which is what
        // makes it report `isNew`, which runs the wizard that SHOWS THE 24-WORD RECOVERY PHRASE.
        // This page once made the key here; the chat then found a key, reported `restored`, and a
        // person who signed up here never saw their phrase (and was told their identity was
        // restored seconds after making it). Clearing browser data would then have lost it for good.
        try {
          env.storage.setItem('humanity_name', name);
          var ret = q['return'] || '/chat';
          // Only a page of this site: the welcome step must never send someone elsewhere.
          if (!/^\/[A-Za-z0-9_\-\/]*$/.test(ret) || ret.indexOf('//') === 0) ret = '/chat';
          if (env.location.replace) env.location.replace(ret + '#joined');
        } catch (e) {
          err.textContent = T('front_door.welcome_error_storage');
          join.disabled = false;
          join.textContent = T('front_door.welcome_join');
        }
      }
      join.addEventListener('click', go);
      input.addEventListener('keydown', function (e) { if (e.key === 'Enter') go(); });
      if (env.scrollTo) env.scrollTo(sec);
    }

    // ── The title screen's buttons (destinations.json) ──
    function drawMenu(list) {
      var nav = byId('fd-menu');
      clear(nav);
      list.forEach(function (d) {
        var a = el('a', 'fd-tile');
        a.setAttribute('href', d.href);
        env.hint(a, d.hint);
        a.appendChild(icon(d.icon, 26));
        var words = el('span', '');
        words.appendChild(el('span', 'fd-tile-name', T(d.name)));
        words.appendChild(el('span', 'fd-tile-line', T(d.line)));
        a.appendChild(words);
        nav.appendChild(a);
      });
    }

    // ── The update cards ──
    function drawCards(force) {
      var row = byId('fd-cards');
      var width = env.widthOf ? env.widthOf(row) : row.clientWidth;
      var lay = FD.cardLayout(width || 0, state.cfg);
      var n = Math.min(lay.n, state.updates.length);
      var key = (lay.swipe ? 's' : 'g') + n;
      if (!force && state.layout === key) return;
      state.layout = key;
      setClass(row, 'fd-swipe', lay.swipe);
      setVar(row, '--fd-n', String(Math.max(n, 1)));
      if (lay.swipe) setVar(row, '--fd-swipe-w', lay.w + 'px');
      clear(row);
      var deps = { document: doc, t: T, hint: env.hint, icon: env.icon, now: env.now ? env.now() : Date.now() };
      state.updates.slice(0, n).forEach(function (u) { row.appendChild(FD.updateCard(u, deps)); });
    }

    function note(words) {
      var p = byId('fd-cards-note');
      p.textContent = words || '';
      p.hidden = !words;
    }

    function loadUpdates() {
      state.loadedAt = env.now ? env.now() : Date.now();
      return getJson(state.cfg.updates_url)
        .then(function (j) {
          // The server keeps only release cards; a card that is not one is dropped here too.
          state.updates = (j.updates || []).filter(function (u) { return u && typeof u.title === 'string'; }).slice(0, state.cfg.card_max);
          note(state.updates.length ? '' : T('front_door.updates_none'));
        })
        .catch(function () { state.updates = []; note(T('front_door.updates_offline')); })
        .then(function () { drawCards(true); });
    }

    // ── The search ──
    function loadIndex() {
      if (state.idxLoading) return state.idxLoading;
      state.idxLoading = Promise.all([
        getJson('/data/how_to/index.json'),
        getJson('/data/how_to/planned.json'),
        getJson('/data/library/index.json'),
        getJson('/data/front_door/search_words.json')
      ]).then(function (g) {
        state.idx = FD.buildIndex(g[0], g[1], g[2], g[3]);
        return state.idx;
      }, function (e) { state.idxLoading = null; throw e; });
      return state.idxLoading;
    }

    function resultLink(href, title, sub, cls, hintKey) {
      var li = el('li', '');
      var a = el('a', '');
      a.setAttribute('href', href);
      env.hint(a, hintKey);
      a.appendChild(el('span', cls || '', title));
      if (sub) a.appendChild(el('span', 'fd-r-sub', sub));
      li.appendChild(a);
      return li;
    }
    function group(box, headKey, items) {
      if (!items.length) return;
      box.appendChild(el('h3', '', T(headKey)));
      var ul = el('ul', '');
      items.forEach(function (li) { ul.appendChild(li); });
      box.appendChild(ul);
    }

    function search(q) {
      var box = byId('fd-results');
      clear(box);
      q = String(q || '').trim();
      if (!q) { box.hidden = true; return null; }
      box.hidden = false;
      if (!state.idx) { box.appendChild(el('span', 'fd-r-none', T('front_door.search_loading'))); return null; }
      var r = FD.search(state.idx, q);
      group(box, 'front_door.search_howtos', r.howtos.map(function (h) { return resultLink('/how-to#' + encodeURIComponent(h.id), h.title, h.summary, '', 'front_door.result_hint'); }));
      group(box, 'front_door.search_planned', r.planned.map(function (h) { return resultLink('/how-to', h.title, h.summary, 'fd-r-planned', 'front_door.planned_hint'); }));
      group(box, 'front_door.search_library', r.docs.map(function (d) { return resultLink('/library#' + d.slug, d.title, d.where, '', 'front_door.result_hint'); }));
      if (!r.howtos.length && !r.planned.length && !r.docs.length) box.appendChild(el('span', 'fd-r-none', T('front_door.search_none', { q: q })));
      var more = el('a', 'fd-r-more', T('front_door.search_more', { q: q }));
      more.setAttribute('href', '/library?q=' + encodeURIComponent(q));
      env.hint(more, 'front_door.search_more_hint');
      box.appendChild(more);
      return r;
    }

    /** Type `q` into the box as a person would: the index loads on the first key. */
    function typeQuery(q) {
      var input = byId('fd-q');
      input.value = q;
      if (state.idx) { search(q); return Promise.resolve(); }
      search(q);
      return loadIndex().then(function () { search(input.value); }, function () { /* offline: the box says so on the next key */ });
    }

    function wireSearch() {
      var input = byId('fd-q');
      var timer = null;
      input.addEventListener('input', function () {
        if (timer && env.clearTimeout) env.clearTimeout(timer);
        var q = input.value;
        if (!state.idx) { typeQuery(q); return; }
        if (env.setTimeout) timer = env.setTimeout(function () { search(q); }, 120);
        else search(q);
      });
      input.addEventListener('keydown', function (e) {
        if (e.key === 'Escape') { input.value = ''; search(''); }
      });
      // The suggestions float over the page (index.html's .fd-results), so a press outside the
      // search closes them, and going back into the box brings them back.
      if (doc.addEventListener) {
        doc.addEventListener('pointerdown', function (e) {
          var area = input.closest ? input.closest('.fd-search') : null;
          var box = byId('fd-results');
          if (box && area && e.target && !area.contains(e.target)) box.hidden = true;
        });
      }
      input.addEventListener('focus', function () { if (input.value && state.idx) search(input.value); });
    }

    // ── Start here: what brought you here? ──
    function progress() {
      try { return JSON.parse(stored(QUEST_KEY) || '{}') || {}; } catch (e) { return {}; }
    }

    function drawAnswer(goal) {
      var box = byId('fd-answer');
      clear(box);
      if (!goal) { box.hidden = true; return; }
      box.hidden = false;
      var left = el('div', '');
      left.appendChild(el('h3', '', T('front_door.goal_first')));
      if (goal.emergency) left.appendChild(el('p', 'fd-emergency', T('front_door.goal_emergency')));
      left.appendChild(el('p', '', T(goal.answer)));
      var open = el('a', 'fd-open', T(goal.first.label));
      open.setAttribute('href', FD.firstHref(goal.first));
      env.hint(open, 'front_door.goal_open_hint');
      left.appendChild(open);
      box.appendChild(left);

      var chain = goal.chain && state.chains[goal.chain];
      setClass(box, 'fd-one', !chain);
      if (!chain) return;
      var right = el('div', '');
      right.appendChild(el('h3', '', T('front_door.goal_then')));
      var p = progress();
      var st = FD.progressOf(chain, p);
      var line = el('p', '');
      line.appendChild(el('span', 'fd-chain-name', chain.title));
      line.appendChild(el('span', 'fd-chain-count', st.next < 0 ? T('quests.state_done') : T('quests.step_count', { n: st.next + 1, total: chain.steps.length })));
      right.appendChild(line);
      var meter = el('span', 'fd-meter');
      meter.setAttribute('aria-hidden', 'true');
      chain.steps.forEach(function (s) { meter.appendChild(el('i', p[chain.id + ':' + s.id] ? 'done' : '')); });
      right.appendChild(meter);
      if (st.next >= 0) right.appendChild(el('p', 'fd-chain-next', chain.steps[st.next].title));
      var steps = el('a', 'fd-chain-link', T('quests.see_chain'));
      steps.setAttribute('href', '/onboarding');
      env.hint(steps, 'front_door.goal_steps_hint');
      right.appendChild(steps);
      box.appendChild(right);
    }

    function pick(id, remember) {
      var goal = state.goals.filter(function (g) { return g.id === id; })[0] || null;
      byId('fd-goals').querySelectorAll('.fd-goal').forEach(function (b) {
        b.setAttribute('aria-pressed', String(!!goal && b.dataset.goal === goal.id));
      });
      if (remember) store(GOAL_KEY, goal ? goal.id : null);
      byId('fd-back').hidden = !goal;
      byId('fd-back-words').textContent = goal ? T('front_door.start_back', { goal: T(goal.name) }) : '';
      drawAnswer(goal);
      return goal;
    }

    function drawGoals(list) {
      state.goals = list;
      var grid = byId('fd-goals');
      clear(grid);
      list.forEach(function (g) {
        var b = el('button', 'fd-goal');
        b.setAttribute('type', 'button');
        b.dataset.goal = g.id;
        b.setAttribute('aria-pressed', 'false');
        env.hint(b, 'front_door.goal_hint');
        b.appendChild(icon(g.icon, 24));
        var words = el('span', '');
        words.appendChild(el('span', 'fd-tile-name', T(g.name)));
        words.appendChild(el('span', 'fd-tile-line', T(g.line)));
        b.appendChild(words);
        b.addEventListener('click', function () { pick(g.id, true); });
        grid.appendChild(b);
      });
      var saved = stored(GOAL_KEY);
      if (saved) pick(saved, false);
    }

    function wireStart() {
      byId('fd-change').addEventListener('click', function () { pick(null, true); });
      byId('fd-start').addEventListener('click', function (e) {
        if (e && e.preventDefault) e.preventDefault();
        if (env.scrollTo) env.scrollTo(byId('start'));
        var first = byId('fd-goals').querySelector('.fd-goal');
        if (first && first.focus) first.focus({ preventScroll: true });
        if (env.history && env.history.replaceState) env.history.replaceState(null, '', '#start');
      });
    }

    /** The tab came back: ask for the updates again, if it was away long enough. */
    function visible() {
      if (!state.cfg) return Promise.resolve(false);
      var now = env.now ? env.now() : Date.now();
      if (now - state.loadedAt < state.cfg.refresh_after_ms) return Promise.resolve(false);
      return loadUpdates().then(function () { return true; });
    }

    function start() {
      wireWelcome();
      return getJson('/data/front_door/config.json').then(function (cfg) {
        state.cfg = cfg;
        wireSearch();
        wireStart();
        return Promise.all([
          getJson('/data/front_door/destinations.json').then(function (d) { drawMenu(d.destinations || []); }),
          loadUpdates(),
          Promise.all([getJson('/data/front_door/goals.json'), getJson('/data/onboarding/quests.json').catch(function () { return { chains: [] }; })])
            .then(function (g) { (g[1].chains || []).forEach(function (c) { state.chains[c.id] = c; }); drawGoals(g[0].goals || []); })
        ]);
      }).then(function () {
        if (env.location && env.location.hash === '#start' && env.scrollTo) env.scrollTo(byId('start'));
        if (env.onWidth) env.onWidth(byId('fd-cards'), function () { drawCards(false); });
      });
    }

    return { start: start, state: state, search: search, typeQuery: typeQuery, pick: pick, drawCards: drawCards, visible: visible, hasClass: hasClass };
  }

  function bootInBrowser(win) {
    var doc = win.document;
    var i18n = win.i18n;
    var go = function () {
      var storage = null;
      try { storage = win.localStorage; } catch (e) { storage = null; }
      doc.querySelectorAll('.pg-ico[data-icon]').forEach(function (e) {
        if (win.hosIcon) e.insertAdjacentHTML('beforeend', win.hosIcon(e.dataset.icon, parseInt(e.dataset.size, 10) || 20));
      });
      var reduce = win.matchMedia && win.matchMedia('(prefers-reduced-motion: reduce)').matches;
      var page = createFrontDoor({
        document: doc, fetch: function (u, i) { return win.fetch(u, i); }, storage: storage,
        t: function (k, v) { return i18n ? i18n.t(k, v) : k; },
        hint: function (e, k) { if (i18n) i18n.hint(e, k); },
        icon: win.hosIcon ? function (n, s) { return win.hosIcon(n, s); } : null,
        fd: win.HosFrontDoor, location: win.location, history: win.history,
        now: function () { return Date.now(); },
        setTimeout: function (f, ms) { return win.setTimeout(f, ms); }, clearTimeout: function (t) { win.clearTimeout(t); },
        scrollTo: function (e) { if (e && e.scrollIntoView) e.scrollIntoView({ behavior: reduce ? 'auto' : 'smooth', block: 'start' }); },
        widthOf: function (e) { return e.clientWidth; },
        onWidth: function (e, f) {
          if (win.ResizeObserver) new win.ResizeObserver(f).observe(e);
          else win.addEventListener('resize', f);
        }
      });
      doc.addEventListener('visibilitychange', function () { if (doc.visibilityState === 'visible') page.visible(); });
      page.start().catch(function (e) { if (win.console) win.console.error('front door:', e); });
    };
    if (doc.readyState === 'loading') doc.addEventListener('DOMContentLoaded', go);
    else go();
  }

  if (NODE) module.exports = { createFrontDoor: createFrontDoor };
  else bootInBrowser(root);
})(typeof window !== 'undefined' ? window : globalThis);
