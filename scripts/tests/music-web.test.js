// The website's Music page mirrors the desktop page's version 2 (docs/design/music-page.md section
// 19; src/gui/pages/music/): the same tabs by the same words keys, Explore's genre and mood cards with
// the same plain sentences, and the Written music list with the same search, sort, filters and
// details, read from data/music/catalogue.json, which scripts/gen-music-catalogue.js writes from the
// app's own RON data (a stale copy fails here).
//
// Run: node --test scripts/tests/music-web.test.js
//
// web/pages/music-app.js runs against a small stand-in document holding only the elements the page
// has (each id is checked against web/pages/music.html, so the stand-in cannot hold one the page
// lacks), a stand-in fetch that answers the catalogue, and i18n.t over data/i18n/en.json.
// MUSIC_APP=<path> points the test at another copy of the script (used to see it red).
//
// Seen RED 2026-11-20 against a copy whose `catalogue` returned the pieces as listed (no search,
// filter or sort) and whose genre cards took each genre's "about" in place of its plain sentence:
// three of five failed, "search beethoven" (all four pieces in the index's order), "the Mothership
// calm card shows its plain sentence", and "the mothership_calm card's sentence is on the page".
'use strict';

const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');

const ROOT = path.join(__dirname, '..', '..');
const EN = JSON.parse(fs.readFileSync(path.join(ROOT, 'data', 'i18n', 'en.json'), 'utf8'));
const word = (key) => key.split('.').reduce((v, k) => (v && typeof v === 'object' ? v[k] : undefined), EN);
global.i18n = {
  t(key, vars) {
    const w = word(key);
    if (typeof w !== 'string') return key;
    return w.replace(/\{\{(\w+)\}\}/g, (m, n) => (vars && vars[n] !== undefined ? String(vars[n]) : m));
  },
};
const app = require(process.env.MUSIC_APP || path.join(ROOT, 'web', 'pages', 'music-app.js'));
const gen = require(path.join(ROOT, 'scripts', 'gen-music-catalogue.js'));
const CAT = JSON.parse(fs.readFileSync(path.join(ROOT, 'data', 'music', 'catalogue.json'), 'utf8'));
const HTML = fs.readFileSync(path.join(ROOT, 'web', 'pages', 'music.html'), 'utf8').replace(/\r\n/g, '\n');
const MOD_RS = fs.readFileSync(path.join(ROOT, 'src', 'gui', 'pages', 'music', 'mod.rs'), 'utf8');

// ── A document that holds only what was built in it ─────────────────────────────────────────────
class El {
  constructor(tag, id) {
    this.tagName = tag.toUpperCase();
    this.id = id || '';
    this.children = [];
    this.attrs = {};
    this.listeners = {};
    this.className = '';
    this.hidden = false;
    this.value = '';
    this._text = '';
  }
  get textContent() { return this._text + this.children.map((c) => c.textContent).join(''); }
  set textContent(v) { this._text = String(v); this.children = []; }
  get innerHTML() { throw new Error('the page writes text, never HTML'); }
  set innerHTML(v) { throw new Error('the page writes text, never HTML'); }
  appendChild(c) { this.children.push(c); return c; }
  setAttribute(k, v) { this.attrs[k] = String(v); }
  getAttribute(k) { return this.attrs[k] ?? null; }
  addEventListener(type, f) { (this.listeners[type] = this.listeners[type] || []).push(f); }
  fire(type) { (this.listeners[type] || []).forEach((f) => f({ target: this })); }
  /** Every element below this one with the class `cls`. */
  all(cls) { const out = []; const walk = (e) => { for (const c of e.children) { if (c.className.split(' ').includes(cls)) out.push(c); walk(c); } }; walk(this); return out; }
  find(tag) { const out = []; const walk = (e) => { for (const c of e.children) { if (c.tagName === tag.toUpperCase()) out.push(c); walk(c); } }; walk(this); return out; }
}

const IDS = ['mp-list', 'mp-msg', 'mp-code', 'mp-add', 'mp-genres', 'mp-moods', 'mp-styles', 'mp-heard', 'mp-heard-clear', 'mp-w-search', 'mp-w-sort', 'mp-w-composer', 'mp-w-era', 'mp-w-count', 'mp-w-list', 'mp-w-detail', ...app.TABS.flatMap((t) => ['mp-tab-' + t, 'mp-panel-' + t])];

/** The page over a stand-in document. `store`: this browser's storage, shared between visits. */
function page(store = new Map()) {
  const els = {};
  for (const id of IDS) {
    assert.ok(HTML.includes(`id="${id}"`), `music.html has #${id}`);
    els[id] = new El(id.startsWith('mp-tab') ? 'button' : id === 'mp-w-search' ? 'input' : 'div', id);
  }
  els['mp-w-sort'].value = 'title';
  let focused = null;
  els['mp-w-search'].focus = () => { focused = 'mp-w-search'; };
  const keys = [];
  const doc = { getElementById: (id) => els[id] || null, createElement: (tag) => new El(tag), addEventListener: (type, f) => { if (type === 'keydown') keys.push(f); } };
  // A key pressed with `target` holding the keyboard (an element, or the page itself).
  const press = (key, target = { tagName: 'BODY' }, mods = {}) => keys.forEach((f) => f({ key, target, preventDefault() {}, ...mods }));
  const storage = { getItem: (k) => store.get(k) ?? null, setItem: (k, v) => store.set(k, v) };
  const fetchFn = (url) => { assert.strictEqual(url, '/data/music/catalogue.json'); return Promise.resolve({ ok: true, json: () => Promise.resolve(CAT) }); };
  return app.start(doc, fetchFn, storage).then((handle) => ({ els, handle, press, store, focused: () => focused }));
}

const ids = (list) => list.map((s) => s.id);
const q = (text, sort, composer, era) => ({ text, sort, composer: composer || '', era: era || '' });

test('the catalogue is the app data, current', () => {
  assert.strictEqual(fs.readFileSync(gen.OUT, 'utf8').replace(/\r\n/g, '\n'), gen.text(), 'data/music/catalogue.json is current: run node scripts/gen-music-catalogue.js');
  const files = (d) => fs.readdirSync(path.join(ROOT, 'data', 'music', d)).filter((f) => f.endsWith('.ron')).length;
  assert.strictEqual(CAT.genres.length, files('genres'), 'one entry per genre file');
  assert.strictEqual(CAT.moods.length, files('moods'), 'one entry per mood file');
  assert.strictEqual(CAT.genres[0].id, 'mothership_calm', 'the default genre first, as the app lists them');
});

test('the written list searches, filters and sorts as the app does', () => {
  const s = CAT.scores;
  assert.deepStrictEqual(ids(app.catalogue(s, q('beethoven', 'title'))), ['fur_elise', 'ode_to_joy', 'beethoven_symphony5_1'], 'search beethoven');
  assert.deepStrictEqual(ids(app.catalogue(s, q('Samelis', 'title'))), ['fur_elise', 'beethoven_symphony5_1'], 'search a typesetter');
  assert.deepStrictEqual(ids(app.catalogue(s, q('beethoven symphony', 'title'))), ['beethoven_symphony5_1'], 'every word typed');
  assert.deepStrictEqual(ids(app.catalogue(s, q('', 'title', word('music.score.composer_grieg')))), ['mountain_king'], 'the composer filter');
  assert.deepStrictEqual(ids(app.catalogue(s, q('', 'title', '', word('music.era.romantic')))), ['mountain_king'], 'the era filter');
  assert.strictEqual(ids(app.catalogue(s, q('', 'composer'))).pop(), 'mountain_king', 'by composer: Beethoven, then Grieg');
  assert.deepStrictEqual(ids(app.catalogue(s, q('', 'length'))), ['ode_to_joy', 'mountain_king', 'fur_elise', 'beethoven_symphony5_1'], 'by length, shortest first');
  const romance = { id: 'schumann_romance', name: 'Romance in A minor', composer: 'Clara Schumann', era: 'Romantic', year: 1853, length_s: 200, parts: ['instrument_packs.part.piano'] };
  assert.deepStrictEqual(ids(app.catalogue([...s, romance], q('', 'year', '', 'Romantic'))), ['schumann_romance', 'mountain_king'], 'a new entry is listed and sorted by year (1853 before 1875)');
});

test('a card for every genre and mood, with its plain sentence', () => {
  const genres = app.genreCards(CAT);
  assert.strictEqual(genres.length, CAT.genres.length);
  for (const g of CAT.genres) {
    const c = genres.find((x) => x.id === g.id);
    assert.strictEqual(c.plain, word(g.plain_words), `the ${c.name} card shows its plain sentence`);
    assert.ok(c.tempo && c.instruments.length > 0, `the ${c.name} card shows its tempo and instruments`);
  }
  for (const m of app.moodCards(CAT)) assert.strictEqual(m.where, word(CAT.moods.find((x) => x.id === m.id).where_words), `the ${m.name} card says where it plays`);
  assert.ok(app.moodCards(CAT).some((m) => m.danger), 'the danger moods say so');
  const community = { genres: [{ id: 'chiptune', name: 'Chiptune', plain: 'Bleeps and bloops from old game consoles.', tempo_bpm: [140, 160], instruments: [] }], moods: CAT.moods };
  assert.strictEqual(app.genreCards(community)[0].plain, 'Bleeps and bloops from old game consoles.', 'a community genre gives plain words');
});

test('the page: tabs, cards, the list and the details', async () => {
  const { els } = await page();
  const visible = () => app.TABS.filter((t) => !els['mp-panel-' + t].hidden);
  assert.deepStrictEqual(visible(), ['listen'], 'Listen first, as in the app');
  els['mp-tab-explore'].fire('click');
  assert.deepStrictEqual(visible(), ['explore']);
  assert.strictEqual(els['mp-tab-explore'].getAttribute('aria-selected'), 'true');
  const cards = els['mp-genres'].all('mp-card');
  assert.strictEqual(cards.length, CAT.genres.length, 'a card per genre');
  for (const g of CAT.genres) assert.ok(els['mp-genres'].textContent.includes(word(g.plain_words)), `the ${g.id} card's sentence is on the page`);
  assert.strictEqual(els['mp-moods'].all('mp-card').length, CAT.moods.length, 'a card per mood');
  // Written music.
  els['mp-tab-written'].fire('click');
  const rows = () => els['mp-w-list'].find('button').map((b) => b.textContent);
  const title = (id) => word(CAT.scores.find((s) => s.id === id).words);
  assert.deepStrictEqual(rows(), ['fur_elise', 'mountain_king', 'ode_to_joy', 'beethoven_symphony5_1'].map(title), 'by title');
  els['mp-w-search'].value = 'beethoven';
  els['mp-w-search'].fire('input');
  assert.deepStrictEqual(rows(), ['fur_elise', 'ode_to_joy', 'beethoven_symphony5_1'].map(title), 'the search narrows the list');
  assert.strictEqual(els['mp-w-count'].textContent, i18n.t('music_page.written_count', { n: 3, total: CAT.scores.length }));
  els['mp-w-search'].value = '';
  els['mp-w-search'].fire('input');
  els['mp-w-sort'].value = 'length';
  els['mp-w-sort'].fire('change');
  assert.deepStrictEqual(rows(), ['ode_to_joy', 'mountain_king', 'fur_elise', 'beethoven_symphony5_1'].map(title), 'sorted by length');
  // The details of a chosen piece: where it came from.
  els['mp-w-list'].find('button').find((b) => b.textContent === title('fur_elise')).fire('click');
  const detail = els['mp-w-detail'].textContent;
  const fur = CAT.scores.find((s) => s.id === 'fur_elise');
  for (const d of [fur.edition, fur.source, fur.typesetter, fur.licence, word(fur.tempo_words), word(fur.credit_words)]) assert.ok(detail.includes(d), `the details show ${JSON.stringify(d)}`);
  const link = els['mp-w-detail'].find('a')[0];
  assert.strictEqual(link.getAttribute('href'), fur.source_url, 'a link to the source page');
  assert.strictEqual(link.getAttribute('rel'), 'noopener');
});

test('the tabs are the app tabs, by the same words keys', () => {
  const native = [...MOD_RS.matchAll(/Tab::\w+ => \("(music_page\.tab_\w+)", "(music_page\.tab_\w+_hint)"\)/g)].map((m) => [m[1], m[2]]);
  assert.strictEqual(native.length, app.TABS.length, 'the app has as many tabs as the website');
  for (const [key, hint] of native) {
    assert.ok(HTML.includes(`data-i18n="${key}" data-i18n-tip="${hint}"`), `the website's tab ${key} with its hint`);
    assert.ok(typeof word(key) === 'string' && typeof word(hint) === 'string', `${key} has words`);
  }
});

// RUNG 2 (docs/design/music-page.md section 19: the web half of each native rung that needs no engine).

// Seen RED 2026-11-20 against a copy of music-app.js whose key handler skipped its text-box check:
// "1 while typing in the search box leaves the tab".
test('the shortcuts: 1 to 4 the tabs, / the search, never while typing', async () => {
  const { els, press, focused } = await page();
  const visible = () => app.TABS.filter((t) => !els['mp-panel-' + t].hidden);
  press('2');
  assert.deepStrictEqual(visible(), ['explore'], '2 opens Explore');
  press('/');
  assert.deepStrictEqual(visible(), ['written'], '/ opens Written music');
  assert.strictEqual(focused(), 'mp-w-search', 'and gives the search box the keyboard');
  press('1', els['mp-w-search']);
  assert.deepStrictEqual(visible(), ['written'], '1 while typing in the search box leaves the tab');
  press('1', { tagName: 'BODY' }, { ctrlKey: true });
  assert.deepStrictEqual(visible(), ['written'], 'Ctrl+1 stays the browser\'s');
  press('4');
  assert.deepStrictEqual(visible(), ['yours'], '4 opens Your music');
  for (const k of ['music_page.key_tabs', 'music_page.key_search']) assert.ok(HTML.includes(`data-i18n="${k}"`), `the help lists ${k}`);
});

// Seen RED 2026-11-20 against a copy that kept the opened pieces in a variable only: "a second visit
// lists the pieces opened (left: [], right: [fur_elise, ode_to_joy])".
test('the pieces opened are kept in this browser, newest first, and Clear empties them', async () => {
  const store = new Map();
  const first = await page(store);
  const title = (id) => word(CAT.scores.find((s) => s.id === id).words);
  const open = (els, id) => els['mp-w-list'].find('button').find((b) => b.textContent === title(id)).fire('click');
  open(first.els, 'ode_to_joy');
  open(first.els, 'fur_elise');
  const second = await page(store);
  const listed = () => second.els['mp-heard'].find('button').map((b) => b.textContent);
  assert.deepStrictEqual(second.handle.opened().map((h) => h.id), ['fur_elise', 'ode_to_joy'], 'a second visit lists the pieces opened');
  assert.deepStrictEqual(listed(), [title('fur_elise'), title('ode_to_joy')], 'by their titles, newest first');
  second.els['mp-heard-clear'].fire('click');
  assert.deepStrictEqual(second.handle.opened(), [], 'Clear empties the list');
  assert.strictEqual(JSON.parse(store.get('hos_music_opened')).length, 0, 'and what the browser keeps');
});

test('a written piece offers its MEI file, exactly as the app keeps it', async () => {
  const { els } = await page();
  const fur = CAT.scores.find((s) => s.id === 'fur_elise');
  assert.strictEqual(fur.file, 'fur_elise.mei', 'the catalogue names each piece\'s file');
  els['mp-w-list'].find('button').find((b) => b.textContent === word(fur.words)).fire('click');
  const link = els['mp-w-detail'].find('a').find((a) => a.getAttribute('download'));
  assert.ok(link, 'a download link');
  assert.strictEqual(link.getAttribute('href'), '/data/music/scores/fur_elise.mei');
  assert.ok(fs.existsSync(path.join(ROOT, 'data', 'music', 'scores', fur.file)), 'the file it points at ships');
  assert.ok(els['mp-w-detail'].textContent.includes(i18n.t('music_page.web_midi_note')), 'and says where MIDI files are saved, and why');
});

// Rung 3a (music-page.md section 15a): the sound styles, a card each on Explore as the app draws them,
// from data/music/styles through the catalogue; and the Listen tab naming the tuning the app switches.
// Seen RED 2026-11-21 with MUSIC_APP at the rung 2 script (git's copy before this rung): "app.styleCards
// is not a function", and with the catalogue not regenerated: "one entry per style file".
test('a card for every sound style, As written first, each with its plain sentence', async () => {
  const files = fs.readdirSync(path.join(ROOT, 'data', 'music', 'styles')).filter((f) => f.endsWith('.ron'));
  assert.strictEqual((CAT.styles || []).length, files.length, 'one entry per style file');
  const cards = app.styleCards(CAT);
  assert.strictEqual(cards[0].name, word('music_page.style_as_written'), 'As written first, as the app draws it');
  for (const s of CAT.styles) {
    const c = cards.find((x) => x.id === s.id);
    assert.strictEqual(c.name, word(s.words), `the ${s.id} card is named by its words`);
    assert.strictEqual(c.plain, word(s.plain_words), `the ${c.name} card shows its plain sentence`);
  }
  const { els } = await page();
  assert.strictEqual(els['mp-styles'].all('mp-card').length, CAT.styles.length + 1, 'a card per style, and As written');
  assert.ok(els['mp-styles'].textContent.includes(word('music.style.retro_fm_plain')), 'the retro FM sentence is on the page');
  assert.ok(HTML.includes('data-i18n="music_page.web_tune"') && word('music_page.web_tune').includes('432'), 'Listen names the tuning, A = 432 by default');
  const community = { styles: [{ id: 'chiptune', name: 'Chiptune', plain: 'Square waves and a noise channel.' }] };
  assert.strictEqual(app.styleCards(community)[1].plain, 'Square waves and a noise channel.', 'a community style gives plain words');
});
