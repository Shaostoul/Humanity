/* How-to page (web): the web mirror of the desktop app's How-to page (src/gui/pages/how_to.rs and
   how_to/, docs/design/how-to.md increment 1). The same data, read the same way: the category tree
   (/data/how_to/categories.json), the planned list (/data/how_to/planned.json), the index the
   build writes (/data/how_to/index.json, each how-to's line, hash and search words, and the names
   of what the how-tos name), and each how-to's own file on open. The page's words are the
   desktop's, from /data/how_to/words.json (W with a key). The rules (Short and Full, the visible steps,
   the current step, a run's ticks) are /shared/how-to-logic.js, the twin of the desktop's.

   Three panes, as the desktop's: the rail (search, which reads the problems' signs too; Doing
   now; the category tree with counts), the list, and the how-to in the order that is its safety
   design: what it is and how dangerous, Short or Full, Stop and call someone if, What you need,
   Start, the steps, It worked if, If something goes wrong, Learn more, Your history, Sources.

   Runs stay in this browser: IndexedDB `humanity-how-to`, store `runs`, one record per run keyed
   by its id and holding the desktop's run shape, step state keyed by step id. Nothing is sent
   anywhere. Short or Full is localStorage `hos_how_to_view`, the split `hos_how_to_layout`, the
   fold `hos_how_to_how_open`; every storage call is in try/catch, so a private window still works
   for this visit. Deep links: /how-to#<id> opens a how-to, /how-to#<id>/<step id> its step. */
(function (root) {
  'use strict';
  var NODE = typeof module === 'object' && module && module.exports;

  /**
   * The page. `env`: document, root (the element it is built in), announce (the live region),
   * fetch, storage (localStorage, or null), runs ({ load() -> Promise<runs>, save(run) -> Promise }),
   * now() (ms), location ({ hash }), history (or null), setInterval, logic (how-to-logic.js),
   * panes (files-panes.js, or null: the panes then stack as plain blocks), how (how-it-works.js, or
   * null), win (the window, for the panes' height; or null). Returns { start, state, render, open }.
   */
  function createHowToPage(env) {
  var L = env.logic;
  var P = env.panes || null;
  var HOW = env.how || null;
  var doc = env.document;
  var rootEl = env.root;
  var announceEl = env.announce || null;

  var WORDS = {};
  function W(k) { return Object.prototype.hasOwnProperty.call(WORDS, k) ? WORDS[k] : k; }
  function WF(k, fill) { var s = W(k); Object.keys(fill).forEach(function (n) { s = s.split('{' + n + '}').join(String(fill[n])); }); return s; }

  /** The words of the hazard scale and the difficulty, by value (whole keys, so the words test can see them). */
  var HAZARD_WORDS = { none: 'hazard_none', caution: 'hazard_caution', serious: 'hazard_serious', lethal: 'hazard_lethal' };
  var DIFFICULTY_WORDS = ['difficulty_1', 'difficulty_2', 'difficulty_3', 'difficulty_4', 'difficulty_5'];
  var VIEW_KEY = 'hos_how_to_view';
  var LAYOUT_KEY = 'hos_how_to_layout';
  var HOW_KEY = 'hos_how_to_how_open';
  /** The desktop's SHARES, side by side and stacked. */
  var SHARES = [0.22, 0.30, 0.48];
  var storage = env.storage || null;
  function getKept(k) { try { return storage ? storage.getItem(k) : null; } catch (e) { return null; } }
  function keep(k, v) { try { if (storage) storage.setItem(k, v); } catch (e) { /* this visit only */ } }

  /** The pages a go step opens on the website, by the desktop's page id. A page the website has no
      twin of is done in the desktop app, and the step says so. */
  var WEB_PAGES = { settings: '/settings', library: '/library', apps: '/apps', tasks: '/tasks', maps: '/maps', market: '/market', files: '/files', how_to: '/how-to', crafting: '/crafting', inventory: '/inventory', chat: '/chat' };

  var st = {
    categories: [], planned: [], index: [], names: {}, locale: null, bodies: {}, runs: [],
    branch: null, open: null, search: '', sort: 'suggested', easy: false, quick: false,
    expanded: {}, view: getKept(VIEW_KEY) === 'short' ? 'short' : 'full', typing: {}, error: '', loaded: false,
    layout: readLayout(), scrollTo: null
  };

  function readLayout() {
    var v = null;
    try { v = JSON.parse(getKept(LAYOUT_KEY) || 'null'); } catch (e) { v = null; }
    return {
      across: P ? P.checked(v && v.across, SHARES) : SHARES.slice(),
      down: P ? P.checked(v && v.down, SHARES) : SHARES.slice()
    };
  }

  /* ── The runs: kept by env.runs (IndexedDB in the browser, see bootInBrowser) ── */
  function saveRun(run) {
    var i = st.runs.findIndex(function (r) { return r.run === run.run; });
    if (i >= 0) st.runs[i] = run; else st.runs.unshift(run);
    Promise.resolve(env.runs.save(JSON.parse(JSON.stringify(run)))).then(function () {
      if (st.error) { st.error = ''; render(); }
    }).catch(function (e) { st.error = WF('save_failed', { why: String(e && e.message || e) }); render(); });
  }
  function openRun(id) { return st.runs.filter(function (r) { return r.how_to === id && !r.finished; })[0] || null; }

  /* ── Helpers ── */
  function h(tag, props, kids) {
    var el = doc.createElement(tag);
    var p = props || {};
    Object.keys(p).forEach(function (k) {
      var v = p[k];
      if (v === undefined || v === null) return;
      if (k === 'text') el.textContent = String(v);
      else if (k === 'on') Object.keys(v).forEach(function (ev) { el.addEventListener(ev, v[ev]); });
      else if (k === 'dataset') Object.keys(v).forEach(function (d) { el.dataset[d] = String(v[d]); });
      else if (k === 'attrs') Object.keys(v).forEach(function (a) { el.setAttribute(a, String(v[a])); });
      else el[k] = v;
    });
    [].concat(kids == null ? [] : kids).forEach(function (c) {
      if (c == null || c === false) return;
      el.appendChild(typeof c === 'string' ? doc.createTextNode(c) : c);
    });
    return el;
  }
  function minutes(n) { return n < 60 ? WF('minutes', { n: n }) : WF('hours', { h: Math.floor(n / 60), m: n % 60 }); }
  function timeWords(t) { var a = minutes(t.active_min); return t.waiting_min ? a + ' ' + WF('plus_waiting', { time: minutes(t.waiting_min) }) : a; }
  function clock(ms) {
    var s = Math.ceil(ms / 1000), hh = Math.floor(s / 3600), mm = Math.floor(s / 60) % 60, ss = s % 60;
    function p2(n) { return (n < 10 ? '0' : '') + n; }
    return hh > 0 ? hh + ':' + p2(mm) + ':' + p2(ss) : mm + ':' + p2(ss);
  }
  function hazardBadge(hz) {
    if (!hz || hz === 'none') return null;
    var svg = doc.createElementNS('http://www.w3.org/2000/svg', 'svg');
    svg.setAttribute('viewBox', '0 0 24 24'); svg.setAttribute('aria-hidden', 'true');
    var tri = doc.createElementNS('http://www.w3.org/2000/svg', 'path');
    tri.setAttribute('d', 'M12 2 L23 21 L1 21 Z');
    tri.setAttribute('fill', hz === 'lethal' ? 'currentColor' : 'none');
    tri.setAttribute('stroke', 'currentColor'); tri.setAttribute('stroke-width', '2'); tri.setAttribute('stroke-linejoin', 'round');
    var mark = doc.createElementNS('http://www.w3.org/2000/svg', 'path');
    mark.setAttribute('d', 'M12 9 V14 M12 17 V17.5'); mark.setAttribute('stroke', hz === 'lethal' ? 'var(--bg-card)' : 'currentColor');
    mark.setAttribute('stroke-width', '2.4'); mark.setAttribute('stroke-linecap', 'round');
    svg.appendChild(tri); svg.appendChild(mark);
    return h('span', { className: 'howto-hazard howto-hazard-' + hz }, [svg, W(HAZARD_WORDS[hz])]);
  }
  function difficultyWord(d) { return W(DIFFICULTY_WORDS[Math.min(5, Math.max(1, d)) - 1]); }
  function reviewWords(entry) {
    var r = entry.review || {};
    if (entry.current === false) return W('review_changed');
    if (r.status === 'verified') return WF('review_verified', { date: r.checked || '' });
    if (r.status === 'sourced') return W('review_sourced');
    return W('review_stub');
  }
  function nameOf(kind, id) { return (st.names[kind] && st.names[kind][id]) || (kind === 'items' ? L.itemWords(id) : id); }
  function findCat(id, nodes) {
    nodes = nodes || st.categories;
    for (var i = 0; i < nodes.length; i++) {
      if (nodes[i].id === id) return nodes[i];
      var f = findCat(id, nodes[i].children || []); if (f) return f;
    }
    return null;
  }
  function branchIds(id) {
    var out = {}; var n = findCat(id);
    (function add(x) { if (!x) return; out[x.id] = true; (x.children || []).forEach(add); })(n);
    return out;
  }
  function pathTo(id) {
    var path = [];
    (function walk(nodes) {
      for (var i = 0; i < nodes.length; i++) {
        path.push(nodes[i].id);
        if (nodes[i].id === id || walk(nodes[i].children || [])) return true;
        path.pop();
      }
      return false;
    })(st.categories);
    return path;
  }
  function counts(id) {
    var ids = branchIds(id);
    return [st.index.filter(function (e) { return ids[e.category]; }).length, st.planned.filter(function (p) { return ids[p.category]; }).length];
  }
  function announce(text) { if (announceEl) announceEl.textContent = text; }

  /* ── Opening ── */
  function open(id, step) {
    st.open = id;
    var e = st.index.filter(function (x) { return x.id === id; })[0];
    var p = st.planned.filter(function (x) { return x.id === id; })[0];
    var cat = (e && e.category) || (p && p.category);
    if (cat) pathTo(cat).forEach(function (c) { st.expanded[c] = true; });
    st.scrollTo = step || null;
    var want = '#' + id + (step ? '/' + step : '');
    if (env.location.hash !== want) { try { if (env.history) env.history.replaceState(null, '', want); else env.location.hash = want; } catch (err) { /* only the address */ } }
    if (e && !st.bodies[id]) {
      getJson('/data/how_to/' + e.file).then(function (body) {
        st.bodies[id] = body; render();
      }).catch(function (err) { st.error = String(err); render(); });
    }
    render();
  }

  /* ── The rail ── */
  function drawRail() {
    var box = h('div', { className: 'howto-rail' });
    var search = h('input', { type: 'search', className: 'howto-search', value: st.search, placeholder: W('search_hint'), attrs: { 'aria-label': W('search_hint') } });
    search.addEventListener('input', function () { st.search = search.value; if (st.search.trim()) st.branch = null; render(); var s = rootEl.querySelector('.howto-search'); if (s) { s.focus(); if (s.setSelectionRange) s.setSelectionRange(s.value.length, s.value.length); } });
    box.appendChild(search);

    var now = env.now();
    var doing = st.runs.filter(function (r) { return !r.finished; }).map(function (r) {
      var e = st.index.filter(function (x) { return x.id === r.how_to; })[0];
      if (!e) return null;
      var body = st.bodies[r.how_to];
      var timer = null;
      Object.keys(r.waits || {}).forEach(function (s) { if (!L.isDone(r, s)) { var left = L.waitLeftMs(r, s, now); if (left !== null) timer = timer === null ? left : Math.min(timer, left); } });
      var line;
      if (timer === 0) line = W('wait_up');
      else if (timer !== null) line = WF('wait_left', { time: clock(timer) });
      else if (body) {
        var at = L.position(body, r);
        var cur = L.currentStep(body, r);
        line = !at ? WF('all_done', { total: L.visibleSteps(body, r).length }) : WF('step_of', { n: at[0], total: at[1] }) + (cur ? ': ' + cur.text : '');
      } else line = '';
      return h('button', { type: 'button', className: 'howto-doing', on: { click: function () { open(e.id); } } }, [e.title, line ? h('small', { text: line }) : null]);
    }).filter(Boolean);
    if (doing.length) { box.appendChild(h('div', { className: 'howto-label', text: W('doing_now') })); doing.forEach(function (d) { box.appendChild(d); }); }

    box.appendChild(h('div', { className: 'howto-label', text: W('categories') }));
    var allOn = !st.branch && !st.search.trim();
    box.appendChild(row(0, null, W('all'), WF('counts', { written: st.index.length, planned: st.planned.length }), allOn, function () { st.branch = null; st.search = ''; render(); }));
    (function nodes(list, depth) {
      list.forEach(function (c) {
        var cn = counts(c.id), hasKids = (c.children || []).length > 0, openNode = !!st.expanded[c.id];
        var label = cn[1] > 0 ? cn[0] + ' / ' + (cn[0] + cn[1]) : String(cn[0]);
        var on = st.branch === c.id && !st.search.trim();
        box.appendChild(row(depth, hasKids ? openNode : null, c.title, label, on, function (ev) {
          if (hasKids && ev && ev.target && ev.target.classList && ev.target.classList.contains('howto-fold')) { st.expanded[c.id] = !openNode; render(); return; }
          st.branch = c.id; st.search = ''; st.expanded[c.id] = true; render();
        }));
        if (openNode) nodes(c.children || [], depth + 1);
      });
    })(st.categories, 0);
    return box;
  }
  function row(depth, openNode, title, count, on, click) {
    var fold = openNode === null ? '' : (openNode ? '▾' : '▸');
    return h('button', { type: 'button', className: 'howto-row', attrs: { 'aria-current': on ? 'true' : 'false', 'aria-expanded': openNode === null ? undefined : String(openNode) }, style: 'padding-left:calc(var(--space-sm) + ' + depth + ' * var(--space-md))', on: { click: click } }, [
      h('span', { className: 'howto-fold', text: fold, attrs: { 'aria-hidden': 'true' } }),
      h('span', { className: 'howto-title', text: title }),
      h('span', { className: 'howto-count', text: count })
    ]);
  }

  /* ── The list ── */
  function entries() {
    var q = st.search.trim();
    var ids = st.branch ? branchIds(st.branch) : null;
    var order = function (cat) { return pathTo(cat).map(function (c) { var n = findCat(c); return n ? n.order || 0 : 0; }); };
    var cmpOrder = function (a, b) { var x = order(a), y = order(b); for (var i = 0; i < Math.max(x.length, y.length); i++) { if ((x[i] || 0) !== (y[i] || 0)) return (x[i] || 0) - (y[i] || 0); } return 0; };
    var words = q.toLowerCase().split(/\s+/).filter(Boolean);
    var written = st.index.filter(function (e) {
      if (ids && !ids[e.category]) return false;
      if (words.length && !words.every(function (w) { return (e.search + ' ' + e.title.toLowerCase()).indexOf(w) >= 0; })) return false;
      if (st.easy && e.difficulty > 1) return false;
      if (st.quick && e.time.active_min + e.time.waiting_min >= 30) return false;
      return true;
    });
    if (st.sort === 'az') written.sort(function (a, b) { return a.title.toLowerCase().localeCompare(b.title.toLowerCase()); });
    else if (st.sort === 'quickest') written.sort(function (a, b) { return (a.time.active_min + a.time.waiting_min) - (b.time.active_min + b.time.waiting_min) || a.title.localeCompare(b.title); });
    else written.sort(function (a, b) { return cmpOrder(a.category, b.category) || a.title.localeCompare(b.title); });
    var out = written.map(function (e) { return { written: e }; });
    if (!st.easy && !st.quick) {
      st.planned.filter(function (p) {
        if (ids && !ids[p.category]) return false;
        var hay = (p.title + ' ' + p.summary).toLowerCase();
        return words.every(function (w) { return hay.indexOf(w) >= 0; });
      }).sort(function (a, b) { return cmpOrder(a.category, b.category) || a.title.localeCompare(b.title); }).forEach(function (p) { out.push({ planned: p }); });
    }
    return out;
  }
  function drawList() {
    var box = h('div', { className: 'howto-list' });
    var q = st.search.trim(), cat = st.branch ? findCat(st.branch) : null;
    box.appendChild(h('h2', { text: q ? WF('results_for', { q: q }) : cat ? cat.title : W('all'), style: 'font-size:var(--text-lg);margin:0' }));
    if (!q && cat) box.appendChild(h('p', { className: 'howto-muted', text: cat.line }));
    var tabs = h('div', { className: 'howto-tabs', attrs: { role: 'group' } });
    [['suggested', 'sort_suggested'], ['az', 'sort_az'], ['quickest', 'sort_quickest']].forEach(function (s) {
      tabs.appendChild(h('button', { type: 'button', className: 'howto-tab', text: W(s[1]), attrs: { 'aria-pressed': String(st.sort === s[0]) }, on: { click: function () { st.sort = s[0]; render(); } } }));
    });
    tabs.appendChild(h('button', { type: 'button', className: 'howto-tab', text: W('chip_easy'), attrs: { 'aria-pressed': String(st.easy) }, on: { click: function () { st.easy = !st.easy; render(); } } }));
    tabs.appendChild(h('button', { type: 'button', className: 'howto-tab', text: W('chip_quick'), attrs: { 'aria-pressed': String(st.quick) }, on: { click: function () { st.quick = !st.quick; render(); } } }));
    box.appendChild(tabs);
    var list = entries();
    if (!list.length) { box.appendChild(h('p', { className: 'howto-muted', text: W('list_empty') })); return box; }
    list.forEach(function (x) {
      if (x.written) {
        var e = x.written;
        box.appendChild(h('button', { type: 'button', className: 'howto-card', attrs: { 'aria-current': String(st.open === e.id) }, on: { click: function () { open(e.id); } } }, [
          h('strong', { text: e.title }),
          h('span', { className: 'howto-summary', text: e.summary }),
          h('span', { className: 'howto-meta' }, [timeWords(e.time) + ' · ' + difficultyWord(e.difficulty), hazardBadge(e.hazard), reviewWords(e)])
        ]));
      } else {
        var p = x.planned;
        box.appendChild(h('button', { type: 'button', className: 'howto-card howto-planned', attrs: { 'aria-current': String(st.open === p.id) }, on: { click: function () { open(p.id); } } }, [h('strong', { text: p.title }), h('em', { text: W('not_written') })]));
      }
    });
    return box;
  }

  /* ── The how-to ── */
  function safetyNote(safety) {
    var box = h('div', { className: 'howto-safety howto-hazard-' + safety.level }, [hazardBadge(safety.level)]);
    box.appendChild(h('p', {}, [h('strong', { text: safety.short || W(HAZARD_WORDS[safety.level]) })]));
    if (st.view === 'full') (safety.full || []).forEach(function (t) { box.appendChild(h('p', { className: 'howto-second', text: t })); });
    return box;
  }
  function ensureRun(body, entry) {
    var r = openRun(body.id);
    if (!r) { r = L.startRun(body, entry.hash, env.now()); }
    return JSON.parse(JSON.stringify(r));
  }
  function lastValue(id, step) {
    for (var i = 0; i < st.runs.length; i++) {
      var r = st.runs[i];
      if (r.how_to === id && r.finished && r.steps && r.steps[step] && r.steps[step].value) return r.steps[step].value;
    }
    return null;
  }
  function drawSteps(body, entry, run) {
    var ol = h('ol', { className: 'howto-steps' });
    var visible = L.visibleSteps(body, run);
    var current = run ? L.currentStep(body, run) : null;
    var desktopOnly = (body.apps || []).length && (body.apps || []).indexOf('website') < 0;
    visible.forEach(function (s, i) {
      if (st.view === 'short' && !L.inShort(s)) return;
      var done = L.isDone(run, s.id), isCur = current && current.id === s.id;
      var li = h('li', { className: 'howto-step' + (done ? ' howto-step-done' : '') + (isCur ? ' howto-step-current' : ''), id: 'step-' + s.id });
      var line = h('div', { className: 'howto-step-line' }, [h('span', { className: 'howto-marker', text: isCur ? '→' : '', attrs: { 'aria-hidden': 'true' } })]);
      var tickable = ['check', 'choose', 'measure', 'action'].indexOf(s.kind) < 0;
      var cbId = 'tick-' + s.id;
      if (tickable) {
        var cb = h('input', { type: 'checkbox', id: cbId, checked: done });
        cb.addEventListener('change', function () { var r = ensureRun(body, entry); L.setDone(r, s.id, cb.checked, env.now()); saveRun(r); render(); });
        line.appendChild(cb);
      }
      line.appendChild(h('span', { className: 'howto-num', text: (i + 1) + '.' }));
      line.appendChild(tickable ? h('label', { className: 'howto-text', attrs: { for: cbId }, text: s.text }) : h('span', { className: 'howto-text', text: s.text }));
      if (isCur) line.appendChild(h('span', { className: 'sr-only', style: 'position:absolute;left:-10000px', text: WF('step_of', { n: i + 1, total: visible.length }) }));
      li.appendChild(line);
      var bodyEl = h('div', { className: 'howto-step-body' });
      var cond = s['if'] || {};
      if (cond.place) bodyEl.appendChild(h('p', { className: 'howto-muted', text: WF('only_in', { place: nameOf('places', cond.place) }) }));
      var ctl = h('div', { className: 'howto-controls' });
      if (s.kind === 'wait' && !done) {
        var left = run ? L.waitLeftMs(run, s.id, env.now()) : null;
        if (left === null) ctl.appendChild(h('button', { type: 'button', text: W('wait_start'), on: { click: function () { var r = ensureRun(body, entry); L.startWait(r, s.id, (s.wait && s.wait.min_s) || 0, env.now()); saveRun(r); render(); } } }));
        else if (left === 0) ctl.appendChild(h('strong', { text: W('wait_up') }));
        else ctl.appendChild(h('span', { className: 'howto-timer', dataset: { timer: s.id }, text: WF('wait_left', { time: clock(left) }) }));
      } else if (s.kind === 'check') {
        var ans = run && run.steps && run.steps[s.id] ? run.steps[s.id].answer : null;
        [['yes', true], ['no', false]].forEach(function (a) {
          ctl.appendChild(h('button', { type: 'button', text: W(a[0]), attrs: { 'aria-pressed': String(ans === a[0]) }, on: { click: function () { var r = ensureRun(body, entry); L.answer(body, r, s.id, a[1], env.now()); saveRun(r); render(); } } }));
        });
        if (ans === 'no') ctl.appendChild(h('span', { className: 'howto-muted', text: W('answered_no') }));
      } else if (s.kind === 'measure' && s.record) {
        var saved = run && run.steps && run.steps[s.id] ? run.steps[s.id].value : '';
        if (!(s.id in st.typing)) st.typing[s.id] = saved || '';
        var input = h('input', { type: 'text', value: st.typing[s.id], attrs: { 'aria-label': s.record.label } });
        input.addEventListener('input', function () { st.typing[s.id] = input.value; });
        ctl.appendChild(h('span', { className: 'howto-second', text: s.record.label }));
        ctl.appendChild(input);
        if (s.record.unit) ctl.appendChild(h('span', { className: 'howto-muted', text: s.record.unit }));
        ctl.appendChild(h('button', { type: 'button', text: W('record_save'), on: { click: function () { var r = ensureRun(body, entry); L.record(r, s.id, st.typing[s.id], env.now()); saveRun(r); render(); } } }));
        if (s.record.remember) { var lv = lastValue(body.id, s.id); if (lv) bodyEl.appendChild(h('p', { className: 'howto-muted', text: WF('last_time', { value: lv }) })); }
      } else if (s.kind === 'choose') {
        var chosen = run && run.choices ? run.choices[s.id] : null;
        (s.options || []).forEach(function (o) {
          ctl.appendChild(h('button', { type: 'button', text: o.label, attrs: { 'aria-pressed': String(chosen === o.id) }, on: { click: function () { var r = ensureRun(body, entry); L.choose(r, s.id, o.id, env.now()); saveRun(r); render(); } } }));
        });
      } else if (s.kind === 'call' && s.how_to) {
        ctl.appendChild(h('button', { type: 'button', text: W('call_open'), on: { click: function () { open(s.how_to); } } }));
      } else if (s.kind === 'go' && s.page) {
        var href = WEB_PAGES[s.page];
        if (href && !desktopOnly) ctl.appendChild(h('a', { href: href, text: W('open') }));
        else ctl.appendChild(h('span', { className: 'howto-muted', text: W('desktop_only') }));
      } else if (s.kind === 'action') {
        ctl.appendChild(h('span', { className: 'howto-muted', text: W('action_later') }));
      } else if (s.kind === 'command' && s.command) {
        ctl.appendChild(h('code', { className: 'howto-command', text: s.command }));
        ctl.appendChild(h('button', { type: 'button', text: W('copy'), on: { click: function () { try { env.copy(s.command); announce(W('copied')); } catch (e) { /* the command is on the page */ } } } }));
      }
      if (ctl.childNodes.length) bodyEl.appendChild(ctl);
      if (s.safety) bodyEl.appendChild(safetyNote(s.safety));
      if (st.view === 'full') (s.why || []).concat(s.detail || []).forEach(function (t) { bodyEl.appendChild(h('p', { className: 'howto-second', text: t })); });
      li.appendChild(bodyEl);
      ol.appendChild(li);
    });
    return ol;
  }

  function drawView() {
    var box = h('div', { className: 'howto-view' });
    var id = st.open;
    if (!id) { box.appendChild(h('p', { className: 'howto-muted', text: W('choose_one') })); return box; }
    var planned = st.planned.filter(function (p) { return p.id === id; })[0];
    if (planned) {
      box.appendChild(h('h2', { text: planned.title, style: 'color:var(--text-muted)' }));
      box.appendChild(h('p', { className: 'howto-muted', text: W('not_written') }));
      box.appendChild(h('p', { className: 'howto-second', text: W('planned_line') }));
      box.appendChild(h('p', { text: planned.summary }));
      return box;
    }
    var entry = st.index.filter(function (e) { return e.id === id; })[0];
    var body = st.bodies[id];
    if (!entry) { box.appendChild(h('p', { className: 'howto-muted', text: W('choose_one') })); return box; }
    if (!body) { box.appendChild(h('p', { className: 'howto-muted', text: W('loading') })); return box; }

    box.appendChild(h('h2', { text: body.title }));
    box.appendChild(h('p', { className: 'howto-second', text: body.summary }));
    box.appendChild(h('p', { className: 'howto-muted', text: W('for') + ': ' + body['for'] }));
    box.appendChild(h('div', { className: 'howto-meta' }, [timeWords(body.time) + ' · ' + difficultyWord(body.difficulty), hazardBadge(body.hazard)]));
    var who = body.written_by === 'person' ? W('written_person') : W('written_ai');
    var meta2 = [reviewWords(entry), '· ' + who];
    if ((body.realities || []).indexOf('sim') >= 0) meta2.push('· ' + W('also_in_game'));
    if ((body.apps || []).length && body.apps.indexOf('website') < 0) meta2.push('· ' + W('desktop_only'));
    box.appendChild(h('div', { className: 'howto-meta' }, meta2));

    var tabs = h('div', { className: 'howto-tabs', attrs: { role: 'group', title: W('view_tip') } });
    [['short', 'view_short'], ['full', 'view_full']].forEach(function (v) {
      tabs.appendChild(h('button', { type: 'button', className: 'howto-tab', text: W(v[1]), title: W('view_tip'), attrs: { 'aria-pressed': String(st.view === v[0]) }, on: { click: function () { st.view = v[0]; keep(VIEW_KEY, v[0]); render(); } } }));
    });
    box.appendChild(tabs);

    if ((body.stop_and_call || []).length) {
      var stop = h('section', { className: 'howto-stop', attrs: { 'aria-label': W('stop_title') } }, [h('h3', {}, [hazardBadge('lethal'), W('stop_title')])]);
      var dl = h('dl');
      body.stop_and_call.forEach(function (c) {
        dl.appendChild(h('dt', { text: c.when }));
        var dd = h('dd', {}, [h('div', { text: c['do'] })]);
        if (c.who) {
          if (c.who.indexOf('role:') === 0) {
            var role = c.who.slice(5);
            dd.appendChild(h('div', { className: 'howto-second', text: nameOf('roles', role) }));
            var a = st.locale && st.locale.authorities && st.locale.authorities[role];
            if (a) {
              dd.appendChild(h('div', { className: 'howto-second', text: WF('where_you_are', { place: st.locale.name, who: a.name }) }));
              if (a.note) dd.appendChild(h('div', { className: 'howto-muted', text: a.note }));
              if (a.url) dd.appendChild(h('a', { href: a.url, text: a.url, rel: 'noopener', target: '_blank' }));
            }
          } else dd.appendChild(h('div', { className: 'howto-second', text: c.who }));
        }
        dl.appendChild(dd);
      });
      stop.appendChild(dl);
      box.appendChild(stop);
    }

    if ((body.tools || []).length || (body.materials || []).length) {
      box.appendChild(h('h3', { text: W('need_title') }));
      [['tools', body.tools || []], ['materials', body.materials || []]].forEach(function (pair) {
        if (!pair[1].length) return;
        box.appendChild(h('p', { className: 'howto-second', text: W(pair[0]) }));
        var ul = h('ul', { className: 'howto-need' });
        pair[1].forEach(function (n) {
          var name = n.item ? nameOf('items', n.item) : (n.proposed_item || '');
          var q = n.qty, amount = '';
          if (q) amount = q.value != null ? String(q.value) : (q.min != null && q.max != null ? WF('qty_range', { a: q.min, b: q.max }) : String(q.min != null ? q.min : q.max != null ? q.max : ''));
          var unit = q && q.unit && q.unit !== 'each' ? ' ' + q.unit : '';
          var line = (amount ? amount + unit + ' ' : '') + name + (n.optional ? ' (' + W('only_sometimes') + ')' : '');
          ul.appendChild(h('li', {}, [line, n.note ? h('div', { className: 'howto-muted', text: n.note }) : null]));
        });
        box.appendChild(ul);
      });
    }

    var run = openRun(id);
    if (!run) {
      var ever = st.runs.some(function (r) { return r.how_to === id; });
      box.appendChild(h('p', {}, [h('button', { type: 'button', className: 'howto-start', title: W('start_tip'), text: ever ? W('start_again') : W('start'), on: { click: function () { saveRun(L.startRun(body, entry.hash, env.now())); render(); } } })]));
    } else {
      var at = L.position(body, run);
      box.appendChild(h('p', { className: 'howto-progress', text: !at ? WF('all_done', { total: L.visibleSteps(body, run).length }) : WF('step_of', { n: at[0], total: at[1] }) }));
      if (run.hash && run.hash !== entry.hash) box.appendChild(h('p', { className: 'howto-muted', text: W('changed_since_start') }));
    }

    box.appendChild(h('h3', { text: W('steps_title') }));
    box.appendChild(drawSteps(body, entry, run));
    if (run) L.removedSteps(body, run).forEach(function (sid) { box.appendChild(h('p', { className: 'howto-muted', text: WF('removed_step', { id: sid }) })); });

    if ((body.worked_if || []).length) {
      box.appendChild(h('h3', { text: W('worked_if') }));
      var wl = h('ul', { className: 'howto-need' });
      body.worked_if.forEach(function (t) { wl.appendChild(h('li', { text: t })); });
      box.appendChild(wl);
    }
    if (run) {
      var ends = h('div', { className: 'howto-ends' });
      [['worked', 'worked', 'howto-worked'], ['did_not_work', 'did_not_work', ''], ['stopped', 'stop_run', '']].forEach(function (o) {
        ends.appendChild(h('button', { type: 'button', className: o[2], text: W(o[1]), on: { click: function () { var r = JSON.parse(JSON.stringify(run)); L.finish(r, o[0], env.now()); saveRun(r); render(); } } }));
      });
      box.appendChild(ends);
    }

    if ((body.problems || []).length) {
      box.appendChild(h('h3', { text: W('problems_title') }));
      var pointed = {};
      if (run) Object.keys(run.steps || {}).forEach(function (k) { var g = run.steps[k].went_to; if (g) pointed[g] = true; });
      body.problems.forEach(function (p) {
        var pb = h('div', { className: 'howto-problem' + (pointed[p.id] ? ' howto-problem-lit' : ''), id: 'problem-' + p.id }, [h('strong', { text: p.sign })]);
        if (p.why) pb.appendChild(h('p', { className: 'howto-second', text: W('problem_why') + ' ' + p.why }));
        pb.appendChild(h('p', { text: W('problem_fix') + ' ' + p.fix }));
        if (p.stop) pb.appendChild(h('p', { style: 'color:var(--danger)', text: W('get_help') }));
        box.appendChild(pb);
      });
    }

    if ((body.reading || []).length || (body.topics || []).length || (body.skills || []).length) {
      box.appendChild(h('h3', { text: W('learn_more') }));
      (body.reading || []).forEach(function (slug) { box.appendChild(h('p', {}, [nameOf('reading', slug) + ' ', h('a', { href: '/library#' + slug, text: W('read_library') })])); });
      (body.topics || []).forEach(function (t) { box.appendChild(h('p', { className: 'howto-second', text: WF('curriculum_topic', { title: nameOf('topics', t) }) })); });
      (body.skills || []).forEach(function (s) { box.appendChild(h('p', { className: 'howto-second', text: WF('skill', { name: nameOf('skills', s) }) })); });
    }

    box.appendChild(h('h3', { text: W('history_title') }));
    var past = st.runs.filter(function (r) { return r.how_to === id; });
    if (!past.length) box.appendChild(h('p', { className: 'howto-muted', text: W('history_none') }));
    past.forEach(function (r) {
      var day = (r.started || '').slice(0, 10);
      var line = !r.outcome ? WF('history_open', { date: day }) : WF('ended', { date: day, outcome: W(r.outcome === 'worked' ? 'outcome_worked' : r.outcome === 'did_not_work' ? 'outcome_did_not_work' : 'outcome_stopped') });
      var values = (body.steps || []).filter(function (s) { return r.steps && r.steps[s.id] && r.steps[s.id].value; }).map(function (s) { return (s.record ? s.record.label : s.id) + ': ' + r.steps[s.id].value; });
      box.appendChild(h('p', { className: 'howto-second', text: values.length ? line + ' · ' + values.join(' · ') : line }));
    });

    box.appendChild(h('h3', { text: W('sources_title') }));
    (body.sources || []).forEach(function (s) {
      var src = h('div', { className: 'howto-source' }, [h('div', { text: s.title })]);
      var dated = s.dated === 'undated' ? W('undated') : s.dated;
      src.appendChild(h('div', { className: 'howto-muted', text: (s.source ? s.source + ' · ' : '') + WF('source_dates', { dated: dated, read: s.read }) }));
      if (s.where) src.appendChild(h('a', { href: s.where, text: s.where, rel: 'noopener', target: '_blank' }));
      if (s.in_project) src.appendChild(h('div', { className: 'howto-muted', text: s.in_project }));
      if (s.quote) src.appendChild(h('div', {}, [h('q', { text: s.quote })]));
      if (st.view === 'full' && s.note) src.appendChild(h('div', { className: 'howto-muted', text: s.note }));
      box.appendChild(src);
    });
    box.appendChild(h('p', { className: 'howto-muted', text: reviewWords(entry) }));
    return box;
  }

  /* ── The page ── */
  var how = null, panes = null;
  function render() {
    var scroll = [].map.call(rootEl.querySelectorAll('.files-pane'), function (p) { return p.scrollTop; });
    rootEl.replaceChildren();
    var head = h('div', { className: 'howto-head' }, [h('h1', { text: W('title') })]);
    if (HOW && !how) {
      how = HOW.createFold({ document: doc, storage: storage, key: HOW_KEY, id: 'howto-how', fill: function (f) {
        ['how_lists', 'how_views', 'how_runs', 'how_review', 'how_ai'].forEach(function (k) { f.line(W(k)); });
      } });
    }
    if (how) head.appendChild(how.toggle);
    rootEl.appendChild(head);
    rootEl.appendChild(h('p', { className: 'howto-line', text: W('line') }));
    if (how) rootEl.appendChild(how.panel);
    if (st.error) rootEl.appendChild(h('p', { className: 'howto-error', text: st.error }));
    if (!st.loaded) { rootEl.appendChild(h('p', { className: 'howto-muted', text: W('loading') })); return; }
    var parts = [drawRail(), drawList(), drawView()];
    if (P) {
      panes = P.createPanes({
        h: h, id: 'howto-panes', count: 3, label: W('title'), win: env.win || null,
        viewport: function () { return env.win ? { width: rootEl.clientWidth || env.win.innerWidth, height: env.win.innerHeight } : { width: 1600, height: 1000 }; },
        shares: function (way) { return st.layout[way]; },
        defaults: function () { return SHARES; },
        setShares: function (way, shares, settled) { st.layout[way] = shares; if (settled) keep(LAYOUT_KEY, JSON.stringify(st.layout)); }
      });
      parts.forEach(function (p, i) {
        if (i > 0) panes.el.appendChild(panes.handle(i - 1));
        panes.el.appendChild(h('div', { className: 'files-pane' }, [p]));
      });
      rootEl.appendChild(panes.el);
      panes.layout();
      [].forEach.call(panes.el.querySelectorAll('.files-pane'), function (p, i) { if (scroll[i]) p.scrollTop = scroll[i]; });
    } else parts.forEach(function (p) { rootEl.appendChild(p); });
    if (st.scrollTo && st.bodies[st.open]) {
      var target = rootEl.querySelector('#step-' + st.scrollTo);
      st.scrollTo = null;
      if (target && target.scrollIntoView) target.scrollIntoView({ block: 'center' });
    }
  }

  /** Once a second while a timer runs on screen: its countdown, and a notice the moment it ends.
      The page calls it on env.setInterval; the tests call it by hand. */
  var announced = {};
  function tick() {
    if (!st.loaded) return;
    var now = env.now(), changed = false;
    st.runs.filter(function (r) { return !r.finished; }).forEach(function (r) {
      Object.keys(r.waits || {}).forEach(function (s) {
        var left = L.waitLeftMs(r, s, now);
        if (left === 0 && !L.isDone(r, s) && !announced[r.run + '/' + s]) {
          announced[r.run + '/' + s] = true;
          var e = st.index.filter(function (x) { return x.id === r.how_to; })[0];
          announce(WF('wait_ready', { title: e ? e.title : r.how_to }));
          changed = true;
        } else if (left !== null && left > 0) changed = true;
      });
    });
    if (!changed) return;
    // Only the countdowns' text, so a field being typed in keeps its focus.
    var timers = rootEl.querySelectorAll('[data-timer]');
    var due = [].some.call(timers, function (el) { var r = openRun(st.open); return !r || L.waitLeftMs(r, el.dataset.timer, now) === 0; });
    var typing = doc.activeElement && doc.activeElement.tagName === 'INPUT';
    if (due || !timers.length) { if (!typing) render(); return; }
    [].forEach.call(timers, function (el) { var r = openRun(st.open); if (r) el.textContent = WF('wait_left', { time: clock(L.waitLeftMs(r, el.dataset.timer, now) || 0) }); });
  }

  /** Open what the address names: #<id> or #<id>/<step id>. */
  function follow() {
    var m = String(env.location.hash || '').slice(1).split('/');
    if (m[0]) open(decodeURIComponent(m[0]), m[1] ? decodeURIComponent(m[1]) : null);
    else render();
  }

  function getJson(url) { return env.fetch(url, { cache: 'no-cache' }).then(function (r) { if (!r.ok) throw new Error(url + ': ' + r.status); return r.json(); }); }

  /** Read everything and draw the page. Resolves once the page is drawn with its data. */
  function start() {
    render();
    return Promise.all([
      getJson('/data/how_to/words.json'),
      getJson('/data/how_to/categories.json'),
      getJson('/data/how_to/planned.json'),
      getJson('/data/how_to/index.json'),
      Promise.resolve(env.runs.load()).catch(function () { return []; })
    ]).then(function (got) {
      WORDS = got[0].words || {};
      st.categories = (got[1].categories || []).slice().sort(function (a, b) { return (a.order || 0) - (b.order || 0); });
      (function sortKids(nodes) { nodes.forEach(function (n) { n.children = (n.children || []).slice().sort(function (a, b) { return (a.order || 0) - (b.order || 0); }); sortKids(n.children); }); })(st.categories);
      st.planned = got[2].planned || [];
      st.index = got[3].how_tos || [];
      st.names = got[3].names || {};
      st.locale = got[3].locale || null;
      st.runs = (got[4] || []).slice().sort(function (a, b) { return a.started < b.started ? 1 : a.started > b.started ? -1 : 0; });
      st.loaded = true;
      // The open runs' how-tos, so Doing now can say each one's next step, and the one the address names.
      var want = String(env.location.hash || '').slice(1).split('/')[0];
      var ids = st.runs.filter(function (r) { return !r.finished; }).map(function (r) { return r.how_to; }).concat(want ? [decodeURIComponent(want)] : []);
      return Promise.all(ids.map(function (id) {
        var e = st.index.filter(function (x) { return x.id === id; })[0];
        return e && !st.bodies[id] ? getJson('/data/how_to/' + e.file).then(function (b) { st.bodies[id] = b; }).catch(function () {}) : null;
      }));
    }).then(function () {
      follow();
      if (env.setInterval) env.setInterval(tick, 1000);
    }).catch(function (e) { st.error = String(e); st.loaded = false; render(); });
  }

  /** Open a how-to and wait for its file (the tests; a click opens it and draws when it arrives). */
  function openAndLoad(id, step) {
    open(id, step);
    var e = st.index.filter(function (x) { return x.id === id; })[0];
    if (!e || st.bodies[id]) return Promise.resolve();
    return getJson('/data/how_to/' + e.file).then(function (b) { st.bodies[id] = b; render(); });
  }

  return { start: start, state: st, render: render, open: open, openAndLoad: openAndLoad, tick: tick, follow: follow, relayout: function () { if (panes) panes.layout(); } };
  }

  // ── In the browser ──

  /** The runs in IndexedDB `humanity-how-to`, store `runs`, keyed by the run's id. */
  function idbRuns(win) {
    var dbPromise = null;
    function db() {
      if (dbPromise) return dbPromise;
      dbPromise = new Promise(function (resolve) {
        try {
          var req = win.indexedDB.open('humanity-how-to', 1);
          req.onupgradeneeded = function () { req.result.createObjectStore('runs', { keyPath: 'run' }); };
          req.onsuccess = function () { resolve(req.result); };
          req.onerror = function () { resolve(null); };
        } catch (e) { resolve(null); }
      });
      return dbPromise;
    }
    return {
      load: function () {
        return db().then(function (d) {
          if (!d) return [];
          return new Promise(function (resolve) {
            try {
              var req = d.transaction('runs', 'readonly').objectStore('runs').getAll();
              req.onsuccess = function () { resolve(req.result || []); };
              req.onerror = function () { resolve([]); };
            } catch (e) { resolve([]); }
          });
        });
      },
      save: function (run) {
        return db().then(function (d) {
          if (!d) throw new Error('this browser keeps no data for this site');
          return new Promise(function (resolve, reject) {
            try {
              var tx = d.transaction('runs', 'readwrite');
              tx.objectStore('runs').put(run);
              tx.oncomplete = function () { resolve(); };
              tx.onerror = function () { reject(tx.error); };
            } catch (e) { reject(e); }
          });
        });
      }
    };
  }

  function bootInBrowser(win) {
    var doc = win.document;
    var go = function () {
      var host = doc.getElementById('howto-root');
      if (!host) return;
      var storage = null;
      try { storage = win.localStorage; } catch (e) { storage = null; }
      var page = createHowToPage({
        document: doc, root: host, announce: doc.getElementById('howto-announce'),
        fetch: function (u, i) { return win.fetch(u, i); }, storage: storage, runs: idbRuns(win),
        now: function () { return Date.now(); }, location: win.location, history: win.history,
        setInterval: function (f, ms) { return win.setInterval(f, ms); }, logic: win.HowToLogic,
        panes: win.FilesPanes, how: win.HosHowItWorks, win: win,
        copy: function (text) { return win.navigator.clipboard.writeText(text); }
      });
      win.addEventListener('resize', page.relayout);
      win.addEventListener('hashchange', page.follow);
      page.start();
    };
    if (doc.readyState === 'loading') doc.addEventListener('DOMContentLoaded', go);
    else go();
  }

  var api = { createHowToPage: createHowToPage };
  if (NODE) module.exports = api;
  else bootInBrowser(root);
})(typeof window !== 'undefined' ? window : globalThis);
