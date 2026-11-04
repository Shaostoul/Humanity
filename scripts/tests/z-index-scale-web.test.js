// One layer order on the website (navigation rung 3, 2026-11-04; docs/design/navigation.md
// section 6): every z-index is on the theme's `--z-*` scale, the scale is the operator's order, and
// every popup's shade covers the whole window, both menus and the warning included. The operator,
// 2026-11-02: "I guess when a full screen popup happens. The click to exit background dark shading
// for the page under the popup is like, layer 1, then the static popup window is layer 2, then any
// interactive bit are layer 3. This way it hides/shades the top menu buttons too."
//
// Run: node --test scripts/tests/z-index-scale-web.test.js   (listed in `just rig-tests`)
// HOS_WEB_DIR points it at another copy of web/ (the red run below read git's copy of main).
//
// RED FIRST (2026-11-04), on the website as rung 4 left it (HOS_WEB_DIR at an export of 5662c257c):
// - "theme.css defines the layer scale": no --z-* token at all.
// - "every z-index on the website is on the scale": "123 z-indexes in 48 values are not on the
//   layer scale" (web/chat/chat-messages.js:649 10000, web/shared/shell.js:408 5500, ...).
// - "every popup's shade covers the whole window, over both menus": 17, each "(under the menu)
//   (stops at the warning)": the Tasks (three), Trade, Wallet, Dev, Market (four), Guilds and
//   Settings' vault (three) modals at 800 to 1100 under the menu's 5500, the chat's help and profile
//   modals at 100 and 150, and the chat's incoming call at 300.
// The control, "the scan still finds the shell's menu and the Tasks modal", passed throughout.

const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const { ORDER, scale, resolve } = require('./z-scale.js');

const ROOT = path.join(__dirname, '..', '..');
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, 'web');

/** z-indexes that stay numbers, each with its reason. Empty: hold it there. */
const ALLOWED = {};

const SKIP_DIRS = new Set(['dist', 'node_modules', 'vendor']);
function files(dir, out = []) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) { if (!SKIP_DIRS.has(e.name)) files(p, out); }
    else if (/\.(css|html|js)$/.test(e.name) && !/\.min\.js$/.test(e.name) && e.name !== 'i18n-en.js') out.push(p);
  }
  return out;
}
const rel = (f) => path.relative(WEB, f).replace(/\\/g, '/');

/** Every z-index written under web/: { where, value } (a CSS declaration or a script's zIndex). */
function zIndexes() {
  const out = [];
  for (const file of files(WEB)) {
    const lines = fs.readFileSync(file, 'utf8').replace(/\r\n/g, '\n').split('\n');
    lines.forEach((line, i) => {
      // CSS (a stylesheet, a <style>, a style="..." or cssText string): z-index: <value> up to ; or a quote.
      const css = /z-index\s*:\s*([^;'"`}\n]+)/g;
      let m;
      while ((m = css.exec(line))) out.push({ where: rel(file) + ':' + (i + 1), value: m[1].trim() });
      // A script: el.style.zIndex = '...' (or a number).
      const js = /\.zIndex\s*=\s*(['"`]?)([^'"`;\n]+)\1/g;
      while ((m = js.exec(line))) out.push({ where: rel(file) + ':' + (i + 1), value: m[2].trim() });
    });
  }
  return out;
}

test('theme.css defines the layer scale, in the operator\'s order', () => {
  const sc = scale(WEB);
  const missing = ORDER.filter(([t]) => !(t in sc)).map(([t]) => '--z-' + t);
  assert.deepStrictEqual(missing, [], 'theme.css defines the layer scale (node scripts/gen-theme-css.js writes it from theme.ron\'s z_* tokens)');
  // Nearest first: each token's z-index is larger than the next one's, with room between them for
  // a band's own raised parts (calc(var(--z-x) + N)).
  for (let i = 0; i + 1 < ORDER.length; i++) {
    const [near, a] = ORDER[i];
    const [far, b] = ORDER[i + 1];
    assert.ok(sc[near] > sc[far], `his ${a} (--z-${near}: ${sc[near]}) is nearer than his ${b} (--z-${far}: ${sc[far]})`);
  }
});

test('every z-index on the website is on the scale', () => {
  const sc = scale(WEB);
  const found = zIndexes();
  assert.ok(found.length >= 100, 'the scan found only ' + found.length + ' z-indexes: is it still reading web/?');
  // The band each token's raised parts must stay in: below the next nearer token.
  const next = {};
  for (let i = 1; i < ORDER.length; i++) next[ORDER[i][0]] = sc[ORDER[i - 1][0]];
  const wrong = [];
  for (const z of found) {
    if (ALLOWED[z.where]) continue;
    const r = resolve(z.value, sc);
    if (!r) { wrong.push(`${z.where} z-index: ${z.value} (not on the scale)`); continue; }
    if (r.literal) { wrong.push(`${z.where} z-index: ${z.value}`); continue; }
    if (r.offset > 0 && next[r.token] !== undefined && r.value >= next[r.token]) {
      wrong.push(`${z.where} z-index: ${z.value} (${r.value} leaves its band: --z-${r.token} + ${r.offset} reaches the next layer)`);
    }
    if (r.offset < 0 && r.token !== 'page') wrong.push(`${z.where} z-index: ${z.value} (only the page has anything behind it)`);
  }
  const values = new Set(wrong.map((w) => w.split('z-index: ')[1]));
  assert.deepStrictEqual(
    wrong,
    [],
    `${wrong.length} z-indexes in ${values.size} values are not on the layer scale: write var(--z-<layer>) from web/shared/theme.css ` +
      '(docs/design/navigation.md section 6; the desktop\'s src/gui/layers.rs), or calc(var(--z-<layer>) + N) for a part raised inside its band'
  );
});

// ---- Popups cover the whole window --------------------------------------------------------------

/** `prop: value` pairs of a declaration list. */
function decls(body) {
  const out = {};
  for (const part of body.replace(/\/\*[\s\S]*?\*\//g, '').split(';')) {
    const i = part.indexOf(':');
    if (i < 0) continue;
    const name = part.slice(0, i).trim().toLowerCase();
    if (!/^-?[a-z][a-z-]*$/.test(name)) continue;
    out[name] = part.slice(i + 1).replace(/!important/i, '').trim();
  }
  return out;
}
const isZero = (v) => v !== undefined && /^0(px|rem|em|%)?$/.test(v.trim());
function insetParts(v) {
  const p = v.trim().split(/\s+(?![^(]*\))/);
  return { top: p[0], bottom: p.length === 1 ? p[0] : p.length === 2 ? p[0] : p[2] };
}

/** Rule-like pieces (CSS rules, style="...", cssText strings) of `src`, each with its line. */
function pieces(src) {
  const out = [];
  const lineOf = (at) => src.slice(0, at).split('\n').length;
  let m;
  const re = /\{([^{}]*)\}/g;
  while ((m = re.exec(src))) {
    if (/[=]/.test(m[1]) || /\breturn\b|\bfunction\b/.test(m[1])) continue;
    const before = src.slice(Math.max(0, m.index - 300), m.index).replace(/\/\*[\s\S]*?\*\//g, '');
    out.push({ selector: before.split(/[{};\n'"`+]/).pop().trim(), body: m[1], line: lineOf(m.index) });
  }
  const re2 = /style\s*=\s*(\\?["'])([\s\S]*?)\1/g;
  while ((m = re2.exec(src))) out.push({ selector: 'style="..."', body: m[2], line: lineOf(m.index) });
  const re3 = /cssText\s*\+?=\s*(['"`])([\s\S]*?)\1/g;
  while ((m = re3.exec(src))) out.push({ selector: 'cssText', body: m[2], line: lineOf(m.index) });
  return out;
}

/** The z-index the shell gives the top menu (`.hub-nav`), read rather than copied. */
function menuZ(sc) {
  const shell = fs.readFileSync(path.join(WEB, 'shared', 'shell.js'), 'utf8').replace(/\r\n/g, '\n');
  const at = shell.indexOf('    .hub-nav {\n');
  assert.ok(at >= 0, 'shell.js styles .hub-nav, the top menu');
  const z = decls(shell.slice(at + 14, shell.indexOf('}', at)))['z-index'];
  const r = resolve(z, sc);
  assert.ok(r, 'the top menu has a z-index the scan can read: ' + z);
  return r.value;
}

/** Every popup's shade under web/: a fixed layer from the top of the window with a dimming fill. */
function shades() {
  const found = [];
  for (const file of files(WEB)) {
    const src = fs.readFileSync(file, 'utf8').replace(/\r\n/g, '\n');
    for (const p of pieces(src)) {
      const d = decls(p.body);
      if (d.position !== 'fixed') continue;
      const fill = d.background || d['background-color'] || '';
      if (!/rgba\(\s*0\s*,\s*0\s*,\s*0|var\(--bg-modal\)|var\(--set-cover-bg\)/.test(fill)) continue;
      const ins = d.inset !== undefined ? insetParts(d.inset) : {};
      const top = d.top !== undefined ? d.top : ins.top;
      const bottom = d.bottom !== undefined ? d.bottom : ins.bottom;
      if (!isZero(top)) continue; // a dim that starts under the menu (the phone drawers') is the page's
      found.push({ where: rel(file) + ':' + p.line, selector: p.selector, bottom, z: d['z-index'] });
    }
  }
  return found;
}

test('every popup\'s shade covers the whole window, over both menus and the warning', () => {
  const sc = scale(WEB);
  const menu = menuZ(sc);
  const found = shades();
  assert.ok(found.length >= 20, 'the scan found only ' + found.length + ' shades: is it still reading web/?');
  const wrong = [];
  for (const s of found) {
    const r = s.z === undefined ? null : resolve(s.z, sc);
    const over = r && r.value > menu;
    const whole = isZero(s.bottom);
    if (!over || !whole) wrong.push(`${s.where} ${s.selector} { bottom: ${s.bottom}; z-index: ${s.z} }${over ? '' : ' (under the menu)'}${whole ? '' : ' (stops at the warning)'}`);
  }
  assert.deepStrictEqual(wrong, [], 'a popup\'s shade dims everything farther away, the menus and the warning included: inset: 0 and z-index: var(--z-shade)');
});

test('the scan still finds the shell\'s menu and the Tasks modal', () => {
  const sc = scale(WEB);
  assert.ok(menuZ(sc) > 0, 'the menu has a z-index');
  assert.ok(shades().some((s) => s.where.startsWith('pages/tasks.html') && /modal-overlay/.test(s.selector)), 'the scan sees the Tasks page\'s .modal-overlay');
});
