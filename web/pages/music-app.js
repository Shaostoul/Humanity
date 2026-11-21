// The Music page's web mirror (docs/design/music-page.md section 19), version 2: the desktop page's
// tabs (Listen, Explore, Written music, Your music; the app's src/gui/pages/music/), Explore's
// cards for each genre and mood with the same plain sentences, the Written music list with the same
// search, sort, filters and details (written.rs `catalogue`), and the favourites kept as share codes
// in this browser (localStorage `hos_music_favourites`). The cards and the list read
// /data/music/catalogue.json, written from the app's own RON data by scripts/gen-music-catalogue.js.
// The website cannot play the music yet (the engine runs in the desktop app; Listen says so).
//
// The share code reader follows crates/humanity-music/src/favourite.rs exactly: "HOSM1-", then
// Crockford base32 of the seed as a variable-length number, the tuning, the genre, the mood and the
// name, and a 16-bit checksum.
//
// In node (scripts/tests/music-web.test.js) the file exports its functions instead of starting.
(function () {
  'use strict';
  const KEY = 'hos_music_favourites';
  // The written pieces whose details were opened here, newest first (the app's pieces heard, rung 2).
  const OPENED = 'hos_music_opened';
  const OPENED_MOST = 50;
  const PREFIX = 'HOSM1-';
  const B32 = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';
  const TABS = ['listen', 'explore', 'written', 'yours'];

  function b32decode(text) {
    const out = [];
    let acc = 0, bits = 0;
    for (let c of text.toUpperCase()) {
      if (c === '-' || /\s/.test(c)) continue;
      if (c === 'I' || c === 'L') c = '1';
      if (c === 'O') c = '0';
      const v = B32.indexOf(c);
      if (v < 0) return null;
      acc = ((acc << 5) | v) & 0xffff;
      bits += 5;
      if (bits >= 8) { bits -= 8; out.push((acc >> bits) & 0xff); }
    }
    return out;
  }

  function check16(bytes) {
    let h = 0x811c9dc5;
    for (const b of bytes) { h ^= b; h = Math.imul(h, 0x01000193) >>> 0; }
    return ((h ^ (h >>> 16)) & 0xffff) >>> 0;
  }

  // Read a code to {name, genre, mood, seed} (seed as a string: it may pass 2^53), or null.
  function read(code) {
    code = (code || '').trim();
    if (code.slice(0, PREFIX.length).toUpperCase() !== PREFIX) return null;
    const bytes = b32decode(code.slice(PREFIX.length));
    if (!bytes || bytes.length < 12) return null;
    const data = bytes.slice(0, -2);
    const sum = bytes[bytes.length - 2] | (bytes[bytes.length - 1] << 8);
    if (check16(data) !== sum) return null;
    let i = 0;
    const u8 = () => { if (i >= data.length) throw new Error('short'); return data[i++]; };
    const str = () => { const n = u8(); if (i + n > data.length) throw new Error('short'); const s = new TextDecoder().decode(new Uint8Array(data.slice(i, i + n))); i += n; return s; };
    try {
      let seed = 0n, shift = 0n;
      for (;;) { const b = u8(); seed |= BigInt(b & 0x7f) << shift; shift += 7n; if (!(b & 0x80)) break; }
      i += 1 + 2 + 1 + 1 + 1; // transpose, tempo, swing, muted, soloed
      const levels = u8(); i += levels * 2;
      const leads = u8();
      for (let k = 0; k < leads; k++) { u8(); str(); }
      const genre = str(), mood = str(), name = str();
      return { name, genre, mood, seed: seed.toString() };
    } catch (e) { return null; }
  }

  // ── Words, as the app picks them: a key's words, else a community file's plain words ──────────
  const tr = (key, vars) => (typeof i18n !== 'undefined' ? i18n.t(key, vars) : key);
  function words(key, plain, fallback) {
    if (key) { const w = tr(key); if (w && w !== key) return w; }
    return plain || fallback || '';
  }
  const readable = (id) => String(id || '').replace(/_/g, ' ');
  const scoreWords = (s) => [words(s.words, s.name, readable(s.id)), words(s.composer_words, s.composer, '')];
  const eraWords = (s) => words(s.era_words, s.era, '');
  const instrumentWords = (keys) => keys.map((k) => words(k, '', readable(k.split('.').pop()))).filter((w, i, a) => a.indexOf(w) === i).join(', ');
  const lengthWords = (sec) => { const s = Math.round(Math.max(0, sec)); return Math.floor(s / 60) + ':' + String(s % 60).padStart(2, '0'); };
  function tempoWords(r) {
    if (!r) return '';
    const [lo, hi] = r;
    return Math.abs(hi - lo) < 1 ? tr('music_page.card_tempo_one', { bpm: lo.toFixed(0) }) : tr('music_page.card_tempo', { lo: lo.toFixed(0), hi: hi.toFixed(0) });
  }
  const cmp = (a, b) => (a < b ? -1 : a > b ? 1 : 0);

  // The written pieces a query keeps, in its order (the app's written.rs `catalogue`): every word
  // typed is found among the title, composer, era, year, edition, source, typesetter, licence and
  // instruments; the composer and era filters are words as shown; a composer sorts by surname; eras
  // sort in the order they came (each era's earliest year), then by year; ties by title.
  function catalogue(scores, q) {
    q = q || {};
    const typed = String(q.text || '').split(/\s+/).filter(Boolean).map((w) => w.toLowerCase());
    const keep = [];
    for (const s of scores) {
      const [title, composer] = scoreWords(s);
      const era = eraWords(s);
      if ((q.composer && composer !== q.composer) || (q.era && era !== q.era)) continue;
      if (typed.length) {
        const hay = [title, composer, era, String(s.year || 0), s.edition, s.source, s.typesetter, s.licence, instrumentWords(s.parts || [])].map((x) => String(x || '').toLowerCase()).join(' ');
        if (!typed.every((w) => hay.includes(w))) continue;
      }
      const surname = (composer.split(/\s+/).filter(Boolean).pop() || '').toLowerCase();
      keep.push({ s, title: title.toLowerCase(), composer: surname + ' ' + composer.toLowerCase(), era });
    }
    const eraFrom = (era) => Math.min(...keep.filter((k) => k.era === era).map((k) => k.s.year || Infinity));
    keep.sort((a, b) => {
      let by = 0;
      if (q.sort === 'composer') by = cmp(a.composer, b.composer);
      else if (q.sort === 'era') by = cmp(eraFrom(a.era), eraFrom(b.era)) || cmp(a.era, b.era) || (a.s.year || 0) - (b.s.year || 0);
      else if (q.sort === 'year') by = (a.s.year || 0) - (b.s.year || 0);
      else if (q.sort === 'length') by = (a.s.length_s || 0) - (b.s.length_s || 0);
      return by || cmp(a.title, b.title);
    });
    return keep.map((k) => k.s);
  }

  // Each genre's card, as the app draws it (explore.rs `genre_cards`); a genre without its own tempo
  // plays the calm mood's.
  function genreCards(cat) {
    const calm = (cat.moods || [])[0];
    return (cat.genres || []).map((g) => ({
      id: g.id,
      name: words(g.words, g.name, g.id),
      plain: words(g.plain_words, g.plain, ''),
      tempo: tempoWords(g.tempo_bpm || (calm && calm.tempo_bpm)),
      instruments: tr('music_page.card_instruments', { list: instrumentWords(g.instruments || []) }),
    }));
  }

  // Each mood's card: its name, where it plays, its tempo, and whether it is heard as danger.
  function moodCards(cat) {
    return (cat.moods || []).map((m) => ({ id: m.id, name: words(m.words, '', readable(m.id)), where: words(m.where_words, m.where_plain, ''), tempo: tempoWords(m.tempo_bpm), danger: !!m.danger }));
  }

  // ── The page ─────────────────────────────────────────────────────────────────────────────────────
  function start(doc, fetchFn, storage) {
    const $ = (id) => doc.getElementById(id);
    const el = (tag, cls, text) => { const e = doc.createElement(tag); if (cls) e.className = cls; if (text !== undefined) e.textContent = text; return e; };
    const tip = (e, key) => { e.setAttribute('data-i18n-tip', key); e.setAttribute('title', tr(key)); return e; };

    // The tabs.
    function show(tab) {
      for (const t of TABS) {
        const b = $('mp-tab-' + t), p = $('mp-panel-' + t);
        if (b) b.setAttribute('aria-selected', t === tab ? 'true' : 'false');
        if (p) p.hidden = t !== tab;
      }
    }
    for (const t of TABS) { const b = $('mp-tab-' + t); if (b) b.addEventListener('click', () => show(t)); }
    show('listen');

    // The keyboard shortcuts (the app's, section 16): 1 to 4 the tabs, / the search. Never while a box
    // has the keyboard, and never with a modifier held.
    if (doc.addEventListener) {
      doc.addEventListener('keydown', (ev) => {
        const t = ev.target || {};
        const tag = String(t.tagName || '').toUpperCase();
        if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || t.isContentEditable) return;
        if (ev.ctrlKey || ev.metaKey || ev.altKey) return;
        const n = ['1', '2', '3', '4'].indexOf(ev.key);
        if (n >= 0) { show(TABS[n]); if (ev.preventDefault) ev.preventDefault(); return; }
        if (ev.key === '/') {
          show('written');
          const s = $('mp-w-search');
          if (s && s.focus) s.focus();
          if (ev.preventDefault) ev.preventDefault();
        }
      });
    }

    // The pieces opened here (kept in this browser, with Clear).
    const loadOpened = () => { try { const v = JSON.parse(storage.getItem(OPENED) || '[]'); return Array.isArray(v) ? v.filter((h) => h && typeof h.id === 'string' && typeof h.at === 'number').slice(0, OPENED_MOST) : []; } catch (e) { return []; } };
    const saveOpened = (list) => { try { storage.setItem(OPENED, JSON.stringify(list)); } catch (e) { /* private window: kept for this visit only */ } };
    let opened = loadOpened();
    function agoWords(secs) {
      const mins = Math.floor(secs / 60);
      if (mins < 1) return tr('music_page.heard_now');
      if (mins < 60) return tr('music_page.heard_minutes', { n: mins });
      if (mins < 60 * 48) return tr('music_page.heard_hours', { n: Math.floor(mins / 60) });
      return tr('music_page.heard_days', { n: Math.floor(mins / (60 * 24)) });
    }
    function renderOpened() {
      const ul = $('mp-heard');
      if (!ul) return;
      ul.textContent = '';
      if (!opened.length) { ul.appendChild(el('li', 'muted', tr('music_page.heard_none'))); return; }
      const now = Date.now() / 1000;
      const scores = (state.cat && state.cat.scores) || [];
      for (const h of opened) {
        const s = scores.find((x) => x.id === h.id);
        const title = s ? scoreWords(s)[0] : readable(h.id);
        const li = el('li');
        const b = tip(el('button', 'title', title), 'music_page.web_heard_open_hint');
        b.type = 'button';
        b.addEventListener('click', () => { state.selected = h.id; show('written'); renderWritten(); });
        li.appendChild(b);
        li.appendChild(el('span', 'line', agoWords(now - h.at)));
        ul.appendChild(li);
      }
    }
    function noteOpened(id) {
      opened = [{ id, at: Math.floor(Date.now() / 1000) }, ...opened.filter((h) => h.id !== id)].slice(0, OPENED_MOST);
      saveOpened(opened);
      renderOpened();
    }
    const clearBtn = $('mp-heard-clear');
    if (clearBtn) clearBtn.addEventListener('click', () => { opened = []; saveOpened(opened); renderOpened(); });

    // The favourites.
    const load = () => { try { const v = JSON.parse(storage.getItem(KEY) || '[]'); return Array.isArray(v) ? v.filter((c) => typeof c === 'string') : []; } catch (e) { return []; } };
    const save = (codes) => { try { storage.setItem(KEY, JSON.stringify(codes)); } catch (e) { /* private window: kept for this visit only */ } };
    let codes = load();
    const list = $('mp-list'), msg = $('mp-msg'), input = $('mp-code');
    const genreWords = (id) => words('music.genre.' + id, '', readable(id));
    const moodWords = (id) => words('music.mood.' + id, '', readable(id));
    function renderFavourites() {
      list.textContent = '';
      if (!codes.length) { list.appendChild(el('li', 'muted', tr('music_page.web_none'))); return; }
      codes.forEach((code, idx) => {
        const f = read(code);
        if (!f) return;
        const li = el('li');
        li.appendChild(el('div', 'name', f.name || tr('music_page.default_name', { seed: f.seed })));
        li.appendChild(el('div', 'line', tr('music_page.fav_line', { genre: genreWords(f.genre), mood: moodWords(f.mood), seed: f.seed })));
        const row = el('div', 'row');
        const copy = tip(el('button', '', tr('music_page.web_copy')), 'music_page.copy_code_hint');
        copy.type = 'button';
        copy.addEventListener('click', () => {
          const clip = typeof navigator !== 'undefined' && navigator.clipboard;
          (clip ? clip.writeText(code) : Promise.reject()).then(() => { msg.textContent = tr('music_page.code_copied'); }).catch(() => { input.value = code; if (input.select) input.select(); });
        });
        const remove = tip(el('button', '', tr('music_page.web_remove')), 'music_page.remove_hint');
        remove.type = 'button';
        remove.addEventListener('click', () => { codes.splice(idx, 1); save(codes); renderFavourites(); });
        row.appendChild(copy);
        row.appendChild(remove);
        li.appendChild(row);
        list.appendChild(li);
      });
    }
    $('mp-add').addEventListener('click', () => {
      const code = input.value.trim();
      const f = read(code);
      if (!f) { msg.textContent = tr('music_page.web_bad_code'); return; }
      if (!codes.includes(code)) { codes.push(code); save(codes); }
      input.value = '';
      msg.textContent = tr('music_page.paste_ok', { name: f.name || f.seed });
      renderFavourites();
    });
    renderFavourites();

    // Explore and Written music, from the catalogue.
    const state = { cat: null, q: { text: '', sort: 'title', composer: '', era: '' }, selected: '' };
    function card(title, lines) {
      const c = el('div', 'mp-card');
      c.appendChild(el('div', 'name', title));
      for (const [cls, text] of lines) if (text) c.appendChild(el('div', cls, text));
      return c;
    }
    function renderExplore() {
      const g = $('mp-genres'), m = $('mp-moods');
      g.textContent = '';
      m.textContent = '';
      for (const c of genreCards(state.cat)) g.appendChild(card(c.name, [['plain', c.plain], ['line', c.tempo], ['line', c.instruments]]));
      for (const c of moodCards(state.cat)) m.appendChild(card(c.name, [['plain', c.where], ['line', c.tempo], ['danger', c.danger ? tr('music_page.mood_danger') : '']]));
    }
    function fillSelect(sel, all, values, chosen) {
      sel.textContent = '';
      const first = el('option', '', all);
      first.value = '';
      sel.appendChild(first);
      for (const v of values) { const o = el('option', '', v); o.value = v; sel.appendChild(o); }
      sel.value = chosen;
    }
    function renderWritten() {
      const scores = state.cat.scores || [];
      const rows = catalogue(scores, state.q);
      $('mp-w-count').textContent = tr('music_page.written_count', { n: rows.length, total: scores.length });
      const ul = $('mp-w-list');
      ul.textContent = '';
      if (!rows.length) ul.appendChild(el('li', 'muted', tr('music_page.written_no_match')));
      for (const s of rows) {
        const [title, composer] = scoreWords(s);
        const li = el('li', s.id === state.selected ? 'chosen' : '');
        const b = tip(el('button', 'title', title), 'music_page.written_row_hint');
        b.type = 'button';
        b.addEventListener('click', () => { state.selected = s.id; noteOpened(s.id); renderWritten(); });
        li.appendChild(b);
        li.appendChild(el('span', 'line', [composer, eraWords(s), s.length_s ? lengthWords(s.length_s) : ''].filter(Boolean).join(', ')));
        ul.appendChild(li);
      }
      const chosen = scores.find((s) => s.id === state.selected) || rows[0];
      const d = $('mp-w-detail');
      d.textContent = '';
      if (!chosen) { d.appendChild(el('p', 'muted', tr('music_page.detail_none'))); return; }
      const [title, composer] = scoreWords(chosen);
      d.appendChild(el('h3', '', title));
      const dl = el('dl');
      const row = (key, value) => { if (!value) return; dl.appendChild(el('dt', '', tr(key))); dl.appendChild(el('dd', '', value)); };
      row('music_page.detail_composer', chosen.composer_years ? tr('music_page.detail_composer_value', { composer, years: chosen.composer_years }) : composer);
      row('music_page.detail_era', eraWords(chosen));
      row('music_page.detail_year', chosen.year ? String(chosen.year) : '');
      row('music_page.detail_key', chosen.key_root ? tr('music_page.detail_key_value', { root: chosen.key_root, mode: words('music.mode.' + chosen.key_mode, '', chosen.key_mode) }) : '');
      row('music_page.detail_length', chosen.length_s ? lengthWords(chosen.length_s) : '');
      row('music_page.detail_instruments', instrumentWords(chosen.parts || []));
      row('music_page.detail_edition', chosen.edition);
      row('music_page.detail_source', chosen.source);
      row('music_page.detail_typesetter', chosen.typesetter);
      row('music_page.detail_licence', chosen.licence);
      row('music_page.detail_tempo', words(chosen.tempo_words, '', ''));
      row('music_page.detail_finding', chosen.tempo_finding);
      d.appendChild(dl);
      const credit = words(chosen.credit_words, '', '');
      if (credit) d.appendChild(el('p', 'muted', credit));
      if (/^https:\/\//.test(chosen.source_url || '')) {
        const a = tip(el('a', '', tr('music_page.detail_open_source')), 'music_page.detail_open_source_hint');
        a.setAttribute('href', chosen.source_url);
        a.setAttribute('rel', 'noopener');
        a.setAttribute('target', '_blank');
        d.appendChild(a);
      }
      // Its MEI file, exactly as written (the app's Save as MEI); MIDI is saved from the app.
      if (/^[a-z0-9_]+\.mei$/.test(chosen.file || '')) {
        const m = tip(el('a', 'download', tr('music_page.web_download_mei')), 'music_page.web_download_mei_hint');
        m.setAttribute('href', '/data/music/scores/' + chosen.file);
        m.setAttribute('download', chosen.file);
        d.appendChild(m);
        d.appendChild(el('p', 'muted', tr('music_page.web_midi_note')));
      }
    }
    function catalogueReady(cat) {
      state.cat = cat;
      const scores = cat.scores || [];
      const uniq = (f) => scores.map(f).filter(Boolean).filter((v, i, a) => a.indexOf(v) === i).sort();
      fillSelect($('mp-w-composer'), tr('music_page.written_all_composers'), uniq((s) => scoreWords(s)[1]), '');
      fillSelect($('mp-w-era'), tr('music_page.written_all_eras'), uniq(eraWords), '');
      renderExplore();
      renderWritten();
      renderOpened();
    }
    const onInput = (id, field) => { const e = $(id); const go = () => { state.q[field] = e.value; if (state.cat) renderWritten(); }; e.addEventListener('input', go); e.addEventListener('change', go); };
    onInput('mp-w-search', 'text');
    onInput('mp-w-sort', 'sort');
    onInput('mp-w-composer', 'composer');
    onInput('mp-w-era', 'era');
    return fetchFn('/data/music/catalogue.json')
      .then((r) => (r.ok ? r.json() : Promise.reject(new Error(String(r.status)))))
      .then(catalogueReady)
      .catch(() => { $('mp-genres').textContent = tr('music_page.web_catalogue_failed'); })
      .then(() => ({ show, state, renderWritten, opened: () => opened }));
  }

  const api = { read, words, scoreWords, eraWords, lengthWords, catalogue, genreCards, moodCards, start, TABS };
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
  else {
    // Reading localStorage itself throws where site data is blocked: the page then keeps favourites for the visit only.
    let storage;
    try { storage = window.localStorage; } catch (e) { storage = { getItem: () => null, setItem: () => {} }; }
    start(document, (u) => fetch(u), storage);
  }
})();
