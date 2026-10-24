/* How-to rules, shared by the website's How-to page (web/pages/how-to-app.js) and the build
   (scripts/build-how-to.js). The desktop app's twins: src/how_to/view.rs and runs.rs (the page's
   rules, the same names), check.rs (the checker), canonical.rs (the hash). docs/design/how-to.md.

   THE CHECKER HERE AND THE ONE IN RUST GIVE THE SAME CODES. tests/fixtures/how_to/broken.json is a
   list of edits to a valid base how-to, one per code; both checkers must refuse each with its code
   (`node scripts/build-how-to.js --check`, and `just test-lib how_to`), and the build compares this
   file's CODES with check.rs's, so a gate cannot exist on one side only. The shape check reads
   schemas/how_to.toml itself (passed in as cx.schema), so the schema file is the shape.

   Works as a browser script (window.HowToLogic) and as a Node module (module.exports). No
   dependencies; the hash takes a BLAKE3 function (web/shared/blake3.js) as an argument. */
(function (root) {
  'use strict';

  var HAZARDS = ['none', 'caution', 'serious', 'lethal'];
  function rank(h) { return HAZARDS.indexOf(h); }
  function isGrave(level) { return level === 'serious' || level === 'lethal'; }

  /* ── The page's rules (view.rs) ── */

  /** A step shows in Short unless it says short: false, and a serious or lethal one always does. */
  function inShort(step) {
    var grave = step.safety && isGrave(step.safety.level);
    return step.short !== false || !!grave;
  }

  /** The steps a run shows: an `if` on a choose step shows once that option is picked; an `if`
      on `place` always shows, labelled, until the person's place is a saved setting. */
  function visibleSteps(howTo, run) {
    return (howTo.steps || []).filter(function (s) {
      var cond = s['if'] || {};
      return Object.keys(cond).every(function (k) {
        if (k === 'place') return true;
        return !!(run && run.choices && run.choices[k] === cond[k]);
      });
    });
  }

  function shown(howTo, run, view) {
    return visibleSteps(howTo, run).filter(function (s) { return view === 'full' || inShort(s); });
  }

  function isDone(run, id) { return !!(run && run.steps && run.steps[id] && run.steps[id].done); }

  function currentStep(howTo, run) {
    var v = visibleSteps(howTo, run);
    for (var i = 0; i < v.length; i++) if (!isDone(run, v[i].id)) return v[i];
    return null;
  }

  /** Where a run is, for "Step 3 of 9": the current step's place among the visible steps and how
      many there are, or null when every one is done (view.rs position). */
  function position(howTo, run) {
    var v = visibleSteps(howTo, run);
    for (var i = 0; i < v.length; i++) if (!isDone(run, v[i].id)) return [i + 1, v.length];
    return null;
  }

  function progress(howTo, run) {
    var v = visibleSteps(howTo, run);
    return [v.filter(function (s) { return isDone(run, s.id); }).length, v.length];
  }

  function removedSteps(howTo, run) {
    var ids = {};
    (howTo.steps || []).forEach(function (s) { ids[s.id] = true; });
    return Object.keys((run && run.steps) || {}).filter(function (id) { return !ids[id]; }).sort();
  }

  function itemWords(id) {
    var base = id.slice(-2) === '_0' ? id.slice(0, -2) : id;
    return base.replace(/_/g, ' ').toLowerCase();
  }

  /** Everything search reads, lower case: the title, summary, who it is for, the steps, the
      problems' signs and fixes (people arrive with a symptom), and the tools and materials. */
  function searchText(howTo) {
    var parts = [howTo.title, howTo.summary, howTo['for']];
    (howTo.steps || []).forEach(function (s) { parts.push(s.text); });
    (howTo.problems || []).forEach(function (p) { parts.push(p.sign); parts.push(p.fix); });
    var out = parts.join(' ').toLowerCase();
    (howTo.tools || []).concat(howTo.materials || []).forEach(function (n) {
      var id = n.item || n.proposed_item;
      if (id) out += ' ' + itemWords(id);
    });
    return out;
  }

  function matches(howTo, q) {
    q = (q || '').trim().toLowerCase();
    if (!q) return true;
    var hay = searchText(howTo);
    return q.split(/\s+/).every(function (w) { return hay.indexOf(w) >= 0; });
  }

  /* ── Runs (runs.rs) ── */

  function pad(n, w) { n = String(n); while (n.length < w) n = '0' + n; return n; }
  function iso(ms) {
    var d = new Date(Math.floor(ms / 1000) * 1000);
    return d.getUTCFullYear() + '-' + pad(d.getUTCMonth() + 1, 2) + '-' + pad(d.getUTCDate(), 2) + 'T' +
      pad(d.getUTCHours(), 2) + ':' + pad(d.getUTCMinutes(), 2) + ':' + pad(d.getUTCSeconds(), 2) + 'Z';
  }
  function msFromIso(s) {
    if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/.test(s || '')) return null;
    var t = Date.parse(s);
    return isNaN(t) ? null : t;
  }

  function startRun(howTo, hash, nowMs) {
    var stamp = iso(nowMs).replace(/[-:]/g, '').replace('T', '-').replace('Z', '');
    var tail = pad(Math.floor(Math.random() * 65536).toString(16), 4);
    return { run: 'r-' + stamp + '-' + tail, how_to: howTo.id, hash: hash || '', reality: 'real', started: iso(nowMs),
      finished: null, outcome: null, choices: {}, steps: {}, waits: {}, notes: '', photos: [], task: null, place: null };
  }
  function stepOf(run, id) { run.steps[id] = run.steps[id] || {}; return run.steps[id]; }
  function setDone(run, id, done, nowMs) { stepOf(run, id).done = done ? iso(nowMs) : null; }
  /** Yes ticks a check. No ticks it too and records where it went; when that is a step, the run
      goes back there: that step and every one after it up to the check are done again. */
  function answer(howTo, run, id, yes, nowMs) {
    var steps = howTo.steps || [];
    var me = steps.filter(function (s) { return s.id === id; })[0] || {};
    var target = me.no || null;
    var toStep = target && steps.some(function (s) { return s.id === target; });
    if (!yes && toStep) {
      var from = -1, to = -1;
      steps.forEach(function (s, i) { if (s.id === target) from = i; if (s.id === id) to = i; });
      var lo = Math.min(from, to), hi = Math.max(from, to);
      for (var i = lo; i <= hi; i++) {
        var st = run.steps[steps[i].id];
        if (st) { st.done = null; if (steps[i].kind === 'check') st.answer = null; }
      }
      stepOf(run, id).went_to = target;
      return;
    }
    var s = stepOf(run, id);
    s.answer = yes ? 'yes' : 'no';
    s.went_to = yes ? null : target;
    s.done = iso(nowMs);
  }
  function choose(run, id, option, nowMs) { run.choices[id] = option; setDone(run, id, true, nowMs); }
  function record(run, id, value, nowMs) {
    var v = (value || '').trim(); var s = stepOf(run, id);
    s.value = v || null; s.done = v ? iso(nowMs) : null;
  }
  function startWait(run, id, seconds, nowMs) { run.waits[id] = { ends: iso(nowMs + seconds * 1000) }; }
  function waitLeftMs(run, id, nowMs) {
    var w = run.waits && run.waits[id]; if (!w) return null;
    var ends = msFromIso(w.ends); if (ends === null) return null;
    return Math.max(0, ends - nowMs);
  }
  function finish(run, outcome, nowMs) { run.outcome = outcome; run.finished = iso(nowMs); run.waits = {}; }

  /* ── The hash (canonical.rs): BLAKE3 of the canonical CBOR without `review` ── */

  function concat(chunks) {
    var len = 0; chunks.forEach(function (c) { len += c.length; });
    var out = new Uint8Array(len), off = 0;
    chunks.forEach(function (c) { out.set(c, off); off += c.length; });
    return out;
  }
  function head(major, n) {
    var mt = major << 5;
    if (n < 24) return new Uint8Array([mt | n]);
    if (n < 0x100) return new Uint8Array([mt | 24, n]);
    if (n < 0x10000) return new Uint8Array([mt | 25, (n >> 8) & 0xff, n & 0xff]);
    if (n < 0x100000000) return new Uint8Array([mt | 26, (n >>> 24) & 0xff, (n >>> 16) & 0xff, (n >>> 8) & 0xff, n & 0xff]);
    var b = new Uint8Array(9); b[0] = mt | 27; var big = BigInt(n);
    for (var i = 8; i >= 1; i--) { b[i] = Number(big & 0xffn); big >>= 8n; }
    return b;
  }
  function utf8(s) {
    if (typeof TextEncoder !== 'undefined') return new TextEncoder().encode(s);
    return Uint8Array.from(Buffer.from(s, 'utf8'));
  }
  /** Canonical CBOR of a JSON value: shortest heads, definite lengths, map keys shorter first then
      bytewise; no floats (whole numbers of zero or more only). */
  function cbor(v) {
    if (v === null) return new Uint8Array([0xf6]);
    if (v === true) return new Uint8Array([0xf5]);
    if (v === false) return new Uint8Array([0xf4]);
    if (typeof v === 'number') {
      if (!Number.isInteger(v) || v < 0) throw new Error(v + ' is not a whole number of zero or more');
      return head(0, v);
    }
    if (typeof v === 'string') { var b = utf8(v); return concat([head(3, b.length), b]); }
    if (Array.isArray(v)) return concat([head(4, v.length)].concat(v.map(cbor)));
    var pairs = Object.keys(v).map(function (k) { return [cbor(k), cbor(v[k])]; });
    pairs.sort(function (a, b) {
      var ka = a[0], kb = b[0];
      if (ka.length !== kb.length) return ka.length - kb.length;
      for (var i = 0; i < ka.length; i++) if (ka[i] !== kb[i]) return ka[i] - kb[i];
      return 0;
    });
    var parts = [head(5, pairs.length)];
    pairs.forEach(function (p) { parts.push(p[0], p[1]); });
    return concat(parts);
  }
  function canonicalBytes(json) {
    var copy = {}; Object.keys(json).forEach(function (k) { if (k !== 'review') copy[k] = json[k]; });
    return cbor(copy);
  }
  function hash(json, blake3) { return blake3.toHex(blake3.hash(canonicalBytes(json))); }

  /* ── The checker (check.rs) ── */

  var CODES = [
    ['too_big', 'refuse'], ['json', 'refuse'], ['whole_numbers', 'refuse'], ['shape', 'refuse'],
    ['file_name', 'refuse'], ['folder', 'refuse'], ['category_unknown', 'refuse'], ['duplicate_id', 'refuse'],
    ['step_id_duplicate', 'refuse'], ['kind_fields', 'refuse'], ['if_unknown', 'refuse'], ['no_target', 'refuse'],
    ['hazard_below_step', 'refuse'], ['safety_without_short', 'refuse'], ['short_hides_safety', 'refuse'],
    ['uncited_number', 'refuse'], ['cite_unknown', 'refuse'], ['source_unknown', 'refuse'],
    ['in_project_missing', 'refuse'], ['role_unknown', 'refuse'], ['item_unknown', 'refuse'],
    ['uses_unlisted', 'refuse'], ['recipe_unknown', 'refuse'], ['skill_unknown', 'refuse'],
    ['topic_unknown', 'refuse'], ['reading_unknown', 'refuse'], ['tag_unknown', 'refuse'],
    ['needs_unknown', 'refuse'], ['call_unknown', 'refuse'], ['page_unknown', 'refuse'],
    ['action_unknown', 'refuse'], ['image_without_alt', 'refuse'], ['review_status', 'refuse'],
    ['verified_without_hash', 'refuse'], ['lethal_without_person', 'refuse'], ['review_hash_stale', 'refuse'],
    ['big', 'warn'], ['serious_without_stop', 'warn'], ['long_quote', 'warn']
  ];
  var SEVERITY = {}; CODES.forEach(function (c) { SEVERITY[c[0]] = c[1]; });
  var MAX_BYTES = 64 * 1024, WARN_BYTES = 16 * 1024, LONG_QUOTE = 300;

  function isId(s) { return typeof s === 'string' && /^[a-z][a-z0-9_]*$/.test(s); }
  function isDate(s) {
    var m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(s || '');
    if (!m) return false;
    var mo = +m[2], d = +m[3];
    return mo >= 1 && mo <= 12 && d >= 1 && d <= 31;
  }
  function isSourceDate(s) {
    if (s === 'undated' || isDate(s)) return true;
    if (/^\d{4}$/.test(s || '')) return true;
    var m = /^(\d{4})-(\d{2})$/.exec(s || '');
    return !!m && +m[2] >= 1 && +m[2] <= 12;
  }
  function hasDigit(s) { return /[0-9]/.test(s || ''); }
  function byteLength(text) { return utf8(text).length; }

  /** The [fields."..."] tables of schemas/how_to.toml (the shape), as path -> spec. Only the keys
      the shape uses are read: type, required, values, min, max, min_items, nullable. */
  function parseSchema(toml, table) {
    table = table || 'fields';
    var out = {}, cur = null, inMulti = false;
    toml.split(/\r?\n/).forEach(function (raw) {
      var line = raw.trim();
      // A """ string: an odd number of """ on a line opens or closes one; what is inside is prose.
      var triples = line.split('"""').length - 1;
      if (inMulti) { if (triples % 2 === 1) inMulti = false; return; }
      if (triples % 2 === 1) { inMulti = true; return; }
      if (!line || line[0] === '#') return;
      var h = /^\[([a-z_]+)\.(?:"([^"]+)"|([A-Za-z0-9_.\[\]]+))\]$/.exec(line);
      if (h) { cur = h[1] === table ? (out[h[2] || h[3]] = {}) : null; return; }
      if (line[0] === '[') { cur = null; return; }
      if (!cur) return;
      var kv = /^([a-z_]+)\s*=\s*(.*)$/.exec(line);
      if (!kv) return;
      var key = kv[1], val = kv[2].trim();
      if (['type', 'required', 'values', 'min', 'max', 'min_items', 'nullable'].indexOf(key) < 0) return;
      if (val.indexOf('"""') === 0) return;
      if (val === 'true' || val === 'false') cur[key] = val === 'true';
      else if (/^-?\d+$/.test(val)) cur[key] = +val;
      else if (val[0] === '[') cur[key] = JSON.parse(val);
      else if (val[0] === '"') cur[key] = JSON.parse(val);
    });
    return out;
  }

  /** Shape against the schema: missing, unknown, of the wrong type, outside an enum or a range. */
  function shapeErrors(doc, schema) {
    var errs = [];
    function children(path) {
      var pre = path ? path + '.' : '';
      return Object.keys(schema).filter(function (p) {
        return p.indexOf(pre) === 0 && p.slice(pre.length).indexOf('.') < 0 && p !== path;
      });
    }
    function checkObject(obj, path) {
      if (obj === null || typeof obj !== 'object' || Array.isArray(obj)) { errs.push((path || 'the how-to') + ' is not an object'); return; }
      var kids = children(path), names = {};
      kids.forEach(function (p) { names[p.slice(path ? path.length + 1 : 0)] = p; });
      Object.keys(obj).forEach(function (k) { if (!names[k]) errs.push('unknown field `' + (path ? path + '.' : '') + k + '`'); });
      Object.keys(names).forEach(function (k) {
        var spec = schema[names[k]];
        if (!(k in obj)) { if (spec.required) errs.push('missing field `' + names[k] + '`'); return; }
        checkValue(obj[k], names[k], spec);
      });
    }
    function checkValue(v, path, spec) {
      var t = spec.type;
      if (v === null) { if (!spec.nullable) errs.push('`' + path + '` is null'); return; }
      var arr = /^array<(.+)>$/.exec(t);
      if (arr) {
        if (!Array.isArray(v)) { errs.push('`' + path + '` is not a list'); return; }
        if (spec.min_items && v.length < spec.min_items) errs.push('`' + path + '` needs at least ' + spec.min_items);
        v.forEach(function (x) {
          if (arr[1] === 'object') checkObject(x, path + '[]');
          else checkValue(x, path + '[]', { type: arr[1], values: spec.values });
        });
        return;
      }
      if (t === 'object') return checkObject(v, path);
      if (t === 'map<string>') {
        if (typeof v !== 'object' || Array.isArray(v)) errs.push('`' + path + '` is not a map');
        else Object.keys(v).forEach(function (k) { if (typeof v[k] !== 'string') errs.push('`' + path + '.' + k + '` is not text'); });
        return;
      }
      if (t === 'id') { if (!isId(v)) errs.push('`' + path + '` `' + v + '` is not snake_case'); return; }
      if (t === 'string') { if (typeof v !== 'string' || !v.trim()) errs.push('`' + path + '` is not a line of text'); else if (path === 'sources[].dated' && !isSourceDate(v)) errs.push('`' + path + '` `' + v + '` is not a date or the word undated'); return; }
      if (t === 'text') { if (typeof v !== 'string') errs.push('`' + path + '` is not text'); return; }
      if (t === 'date') { if (!isDate(v)) errs.push('`' + path + '` `' + v + '` is not YYYY-MM-DD'); return; }
      if (t === 'bool') { if (typeof v !== 'boolean') errs.push('`' + path + '` is not true or false'); return; }
      if (t === 'int') {
        if (typeof v !== 'number') { errs.push('`' + path + '` is not a number'); return; }
        if (spec.min !== undefined && v < spec.min) errs.push('`' + path + '` is below ' + spec.min);
        if (spec.max !== undefined && v > spec.max) errs.push('`' + path + '` is above ' + spec.max);
        return;
      }
      if (t === 'enum') { if ((spec.values || []).indexOf(v) < 0) errs.push('`' + path + '` `' + v + '` is not one of ' + (spec.values || []).join(', ')); return; }
      errs.push('`' + path + '`: the schema type ' + t + ' is not one the checker knows');
    }
    checkObject(doc, '');
    // What the schema's types cannot say, as the Rust checker says it.
    (doc.tools || []).concat(doc.materials || []).forEach(function (n) {
      if (n && !n.item && !n.proposed_item) errs.push('a tool or material names neither item nor proposed_item');
    });
    var keys = {};
    (doc.sources || []).forEach(function (s) { if (s && keys[s.key]) errs.push('two sources share the key `' + s.key + '`'); if (s) keys[s.key] = true; });
    return errs;
  }

  function badNumbers(v, at, out) {
    if (typeof v === 'number') { if (!Number.isInteger(v) || v < 0) out.push(at + ' = ' + v); }
    else if (Array.isArray(v)) v.forEach(function (x, i) { badNumbers(x, at + '/' + i, out); });
    else if (v && typeof v === 'object') Object.keys(v).forEach(function (k) { badNumbers(v[k], at + '/' + k, out); });
    return out;
  }

  /** The fields a step's kind needs, and the ones no other kind may carry. */
  function kindFieldProblems(s) {
    var out = [];
    function has(present, name, kinds) { if (present && kinds.indexOf(s.kind) < 0) out.push('`' + name + '` belongs to a ' + kinds.join(' or ') + ' step'); }
    has(s.no != null, 'no', ['check']);
    has(s.wait != null, 'wait', ['wait']);
    has(s.record != null, 'record', ['measure']);
    has((s.options || []).length > 0, 'options', ['choose']);
    has(s.how_to != null, 'how_to', ['call']);
    has(s.page != null, 'page', ['go']);
    has(s.section != null, 'section', ['go']);
    has(s.action != null, 'action', ['action']);
    has(s.command != null, 'command', ['command']);
    has(s.shell != null, 'shell', ['command']);
    function need(ok, what) { if (!ok) out.push('a ' + s.kind + ' step needs ' + what); }
    if (s.kind === 'wait') need(!!s.wait && s.wait.min_s > 0 && (s.wait.max_s == null || s.wait.max_s >= s.wait.min_s), '`wait` with min_s above 0 and max_s, if given, not below it');
    if (s.kind === 'measure') need(!!s.record, '`record`');
    if (s.kind === 'choose') {
      var ids = {}; (s.options || []).forEach(function (o) { ids[o.id] = true; });
      need((s.options || []).length >= 2 && Object.keys(ids).length === (s.options || []).length, 'two or more `options` with different ids');
    }
    if (s.kind === 'call') need(!!s.how_to, '`how_to`');
    if (s.kind === 'go') need(!!s.page, '`page`');
    if (s.kind === 'action') need(!!s.action, '`action`');
    if (s.kind === 'command') need(!!(s.command && s.command.trim()), '`command`');
    return out;
  }

  /**
   * Check one how-to file. `file` is its path under data/how_to/ ("water/boil_water_safe.json"),
   * `text` its contents. cx: { schema, categories (id -> top), written (id -> file), planned (set
   * object), items, recipes, skills, topics, reading, tags (set objects or null), sources (id ->
   * isRole, or null), places, pages (page -> [sections], or null), actions, repoHas (path -> bool,
   * or null), blake3 }. Returns { findings: [{code, message}], howTo }.
   */
  function check(file, text, cx) {
    var out = [];
    function push(code, message) { out.push({ code: code, message: message }); }
    var bytes = byteLength(text);
    if (bytes > MAX_BYTES) { push('too_big', bytes + ' bytes, past the ' + MAX_BYTES + ' a how-to may be'); return { findings: out, howTo: null }; }
    if (bytes > WARN_BYTES) push('big', bytes + ' bytes: this is turning into a Library guide');
    var doc;
    try { doc = JSON.parse(text); } catch (e) { push('json', 'not JSON: ' + e.message); return { findings: out, howTo: null }; }
    var bad = badNumbers(doc, '', []);
    if (bad.length) { push('whole_numbers', 'every number is a whole number of zero or more: ' + bad.join(', ')); return { findings: out, howTo: null }; }
    var shape = shapeErrors(doc, cx.schema);
    if (shape.length) { shape.forEach(function (m) { push('shape', m); }); return { findings: out, howTo: null }; }
    var h = doc;

    // Where it sits.
    var stem = file.split('/').pop();
    if (stem !== h.id + '.json') push('file_name', 'the file is `' + stem + '` but the id is `' + h.id + '`');
    var top = cx.categories[h.category];
    if (!top) push('category_unknown', 'category `' + h.category + '` is not in data/how_to/categories.json');
    else if (file.indexOf('/') >= 0 && file.split('/')[0] !== top) push('folder', 'the file belongs in data/how_to/' + top + '/');
    if (cx.written[h.id] && cx.written[h.id] !== file) push('duplicate_id', '`' + h.id + '` is also the id of ' + cx.written[h.id]);
    if (cx.planned[h.id]) push('duplicate_id', '`' + h.id + '` is still in data/how_to/planned.json');

    // Steps.
    var seen = {}, stepIds = {}, problemIds = {};
    (h.steps || []).forEach(function (s) { if (seen[s.id]) push('step_id_duplicate', 'two steps are called `' + s.id + '`'); seen[s.id] = true; stepIds[s.id] = true; });
    (h.problems || []).forEach(function (p) { problemIds[p.id] = true; });
    var chosen = {};
    var listed = {};
    (h.tools || []).concat(h.materials || []).forEach(function (n) { if (n.item) listed[n.item] = true; });
    (h.steps || []).forEach(function (s) {
      kindFieldProblems(s).forEach(function (m) { push('kind_fields', 'step `' + s.id + '` (' + s.kind + '): ' + m); });
      var cond = s['if'] || {};
      Object.keys(cond).forEach(function (k) {
        if (k === 'place') { if (cx.places && !cx.places[cond[k]]) push('if_unknown', 'step `' + s.id + '`: place `' + cond[k] + '` is not a jurisdiction'); return; }
        if (!chosen[k]) push('if_unknown', 'step `' + s.id + '` depends on `' + k + '`, which is not a choose step before it');
        else if (chosen[k].indexOf(cond[k]) < 0) push('if_unknown', 'step `' + s.id + '`: `' + cond[k] + '` is not an option of `' + k + '`');
      });
      if (s.kind === 'choose') chosen[s.id] = (s.options || []).map(function (o) { return o.id; });
      if (s.no != null && !problemIds[s.no] && !stepIds[s.no]) push('no_target', 'step `' + s.id + '` sends No to `' + s.no + '`');
      if (s.who != null && (h.roles || []).indexOf(s.who) < 0) push('role_unknown', 'step `' + s.id + '` is done by `' + s.who + '`, which is not in roles');
      if (s.image && !(s.image.alt && s.image.alt.trim())) push('image_without_alt', 'step `' + s.id + '`: the picture has no alt');
      if (s.kind === 'call' && s.how_to && !cx.written[s.how_to]) push('call_unknown', 'step `' + s.id + '` calls `' + s.how_to + '`, which is not written');
      if (s.kind === 'go' && cx.pages && s.page) {
        if (!cx.pages[s.page]) push('page_unknown', 'step `' + s.id + '` opens page `' + s.page + '`');
        else if (s.section && cx.pages[s.page].indexOf(s.section) < 0) push('page_unknown', 'step `' + s.id + '` opens `' + s.page + '` at `' + s.section + '`');
      }
      if (s.action != null && !(cx.actions || {})[s.action]) push('action_unknown', 'step `' + s.id + '` runs the admin action `' + s.action + '` (ids arrive in increment 6)');
      (s.uses || []).forEach(function (u) { if (!listed[u]) push('uses_unlisted', 'step `' + s.id + '` uses `' + u + '`, which is not in the tools or materials'); });
    });

    // Safety.
    var worst = -1;
    (h.steps || []).forEach(function (s) { if (s.safety) worst = Math.max(worst, rank(s.safety.level)); });
    if (worst > rank(h.hazard)) push('hazard_below_step', 'the hazard is `' + h.hazard + '` but a step is `' + HAZARDS[worst] + '`');
    (h.steps || []).forEach(function (s) {
      if (s.safety && isGrave(s.safety.level)) {
        if (!(s.safety.short && s.safety.short.trim())) push('safety_without_short', 'step `' + s.id + '`: a ' + s.safety.level + ' note needs a short line');
        if (s.short === false) push('short_hides_safety', 'step `' + s.id + '` carries a ' + s.safety.level + ' note, so Short may not leave it out');
      }
    });
    if (rank(h.hazard) >= rank('serious')) {
      (h.steps || []).forEach(function (s) {
        var words = [s.text].concat(s.why || [], s.detail || []);
        if (s.safety) words = words.concat([s.safety.short || ''], s.safety.full || []);
        (s.options || []).forEach(function (o) { words.push(o.label); });
        if (s.record) words.push(s.record.label);
        var states = words.some(hasDigit) || s.wait != null || !!(s.record && s.record.range);
        if (states && !(s.cites || []).length) push('uncited_number', 'step `' + s.id + '` states a number in a ' + h.hazard + ' how-to and cites no source');
      });
      (h.problems || []).forEach(function (p) {
        if ([p.sign, p.why, p.fix].some(hasDigit) && !(p.cites || []).length) push('uncited_number', 'problem `' + p.id + '` states a number and cites no source');
      });
      (h.stop_and_call || []).forEach(function (c, i) {
        if ((hasDigit(c.when) || hasDigit(c['do'])) && !(c.cites || []).length) push('uncited_number', 'stop and call entry ' + (i + 1) + ' states a number and cites no source');
      });
      if (!(h.stop_and_call || []).length) push('serious_without_stop', 'a ' + h.hazard + ' how-to with no stop and call entry');
    }

    // Sources.
    var keys = {};
    (h.sources || []).forEach(function (s) { keys[s.key] = true; });
    var cited = [];
    (h.steps || []).forEach(function (s) { cited.push(['step `' + s.id + '`', s.cites || []]); });
    (h.problems || []).forEach(function (p) { cited.push(['problem `' + p.id + '`', p.cites || []]); });
    (h.stop_and_call || []).forEach(function (c, i) { cited.push(['stop and call entry ' + (i + 1), c.cites || []]); });
    cited.forEach(function (c) { c[1].forEach(function (k) { if (!keys[k]) push('cite_unknown', c[0] + ' cites `' + k + '`, which is not a key in sources'); }); });
    (h.sources || []).forEach(function (s) {
      if (!s.source && !s.in_project) push('source_unknown', 'source `' + s.key + '` names neither a registry source nor an in_project path');
      else if (s.source && cx.sources) {
        if (!(s.source in cx.sources)) push('source_unknown', 'source `' + s.key + '` names `' + s.source + '`, which is not in the registry');
        else if (cx.sources[s.source]) push('source_unknown', 'source `' + s.key + '` names `' + s.source + '`, which is a role');
      }
      if (s.in_project && cx.repoHas && (s.in_project.indexOf('..') >= 0 || !cx.repoHas(s.in_project))) push('in_project_missing', 'source `' + s.key + '` names ' + s.in_project + ', which is not in this repository');
      if (s.quote && Array.from(s.quote).length > LONG_QUOTE) push('long_quote', 'source `' + s.key + '` quotes more than ' + LONG_QUOTE + ' characters');
    });
    (h.stop_and_call || []).forEach(function (c, i) {
      var role = typeof c.who === 'string' && c.who.indexOf('role:') === 0 ? c.who.slice(5) : null;
      if (role && cx.sources && cx.sources[role] !== true) push('role_unknown', 'stop and call entry ' + (i + 1) + ' names role `' + role + '`');
    });

    // References.
    function unknown(set, ids) { return set ? ids.filter(function (i) { return !set[i]; }) : []; }
    var items = (h.tools || []).concat(h.materials || []).map(function (n) { return n.item; }).filter(Boolean);
    if (h.help) items = items.concat(h.help.acts_on || []);
    if (h.makes) {
      items = items.concat((h.makes.inputs || []).concat(h.makes.outputs || []).map(function (c) { return c.item; }));
      if (h.makes.station) items.push(h.makes.station);
    }
    unknown(cx.items, items).forEach(function (i) { push('item_unknown', '`' + i + '` is not an item in data/items.csv'); });
    if (h.makes) unknown(cx.recipes, [h.makes.recipe]).forEach(function (r) { push('recipe_unknown', 'makes names recipe `' + r + '`'); });
    var skills = (h.skills || []).concat(h.makes && h.makes.skill ? [h.makes.skill] : []);
    unknown(cx.skills, skills).forEach(function (s) { push('skill_unknown', 'skill `' + s + '`'); });
    unknown(cx.topics, h.topics || []).forEach(function (t) { push('topic_unknown', 'topic `' + t + '`'); });
    unknown(cx.reading, h.reading || []).forEach(function (r) { push('reading_unknown', '`' + r + '` is not a Library document'); });
    unknown(cx.tags, h.tags || []).forEach(function (t) { push('tag_unknown', 'tag `' + t + '`'); });
    (h.needs_first || []).forEach(function (n) { if (!cx.written[n] && !cx.planned[n]) push('needs_unknown', 'needs_first names `' + n + '`'); });

    // Review.
    var r = h.review;
    if (r.status === 'absent') push('review_status', 'a written how-to cannot be `absent`');
    if ((r.status === 'sourced' || r.status === 'verified') && !(h.sources || []).length) push('review_status', 'marked `' + r.status + '` with no sources');
    if (r.status === 'verified') {
      if (!r.hash) push('verified_without_hash', 'verified with no hash');
      if (h.hazard === 'lethal' && !r.person) push('lethal_without_person', 'a lethal how-to is not verified until a person has read it and dated it');
    }
    if (r.hash) {
      var now = hash(doc, cx.blake3);
      if (now !== r.hash) push('review_hash_stale', 'the words changed since the review: it checked ' + r.hash + ', they hash to ' + now);
    }
    return { findings: out, howTo: doc };
  }

  function refusals(findings) { return findings.filter(function (f) { return SEVERITY[f.code] === 'refuse'; }); }

  var api = {
    HAZARDS: HAZARDS, rank: rank, isGrave: isGrave,
    inShort: inShort, visibleSteps: visibleSteps, shown: shown, isDone: isDone, currentStep: currentStep,
    progress: progress, position: position, removedSteps: removedSteps, matches: matches, searchText: searchText, itemWords: itemWords,
    iso: iso, msFromIso: msFromIso, startRun: startRun, setDone: setDone, answer: answer, choose: choose,
    record: record, startWait: startWait, waitLeftMs: waitLeftMs, finish: finish,
    cbor: cbor, canonicalBytes: canonicalBytes, hash: hash,
    CODES: CODES, SEVERITY: SEVERITY, parseSchema: parseSchema, shapeErrors: shapeErrors, check: check, refusals: refusals,
    isDate: isDate, isId: isId
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else root.HowToLogic = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
