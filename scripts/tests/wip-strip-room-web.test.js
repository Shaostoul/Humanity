// Nothing on the website hides under the unfinished-software warning (2026-10-30, the seams
// review of the warning strip, docs/design/app-bottom-strip.md).
//
// The strip (web/shared/shell.js, .hos-wip-strip) is fixed to the bottom of every page, and it
// sets --hos-wip-h to its measured height. Every OTHER element fixed or sticky at the bottom of
// the window has to stand on it, `bottom: var(--hos-wip-h, 0px)` (or a calc() holding it), or it
// is covered: Settings' Save bar, the chat's phone drawers and the Tasks drawer were, half under
// the strip or hidden when it wrapped. The one other answer allowed is a layer that covers the
// whole window ABOVE the strip (a dialog's backdrop, a full-screen viewer: inset or top and
// bottom 0, z-index over the strip's), which hides the warning for as long as it is open, the
// way the menus and help do.
//
// Run: node --test scripts/tests/wip-strip-room-web.test.js   (listed in `just rig-tests`)
// HOS_WEB_DIR points it at another copy of web/.
//
// HOW IT FINDS THEM, in every .css, .html and .js file under web/ (generated and vendored files
// aside): CSS rules (in a stylesheet, a <style>, or a stylesheet a script writes), style="..."
// attributes, and `.style.cssText = ...` strings. A rule counts when it says position: fixed or
// sticky, or sets bottom or inset for a selector a rule in the same file makes fixed or sticky.
// It is anchored at the bottom when its bottom (from `bottom`, or the bottom part of `inset`) is
// set and not auto, or when it is fixed and as tall as the window (`height: 100vh`, the chat's
// thread panel), so its own bottom is the window's.
//
// Red first, 2026-10-30, on the website as it stood after the strip merged: 33 rules, Settings'
// #settings-action-bar, the chat's #sidebar and #right-sidebar and the Tasks .detail-overlay among
// them, every dialog backdrop under the strip's z-index, and the toasts; then, once the scan also
// read window-tall fixed panels, the chat's #thread-panel. The "the scan still finds the footer"
// control passed throughout (the footer already stood on the strip).

const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');

const ROOT = path.join(__dirname, '..', '..');
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, 'web');

/** The strip's own z-index (shell.js), read rather than copied, so the rule follows it. */
const STRIP_Z = (() => {
  const shell = fs.readFileSync(path.join(WEB, 'shared', 'shell.js'), 'utf8');
  const at = shell.indexOf('.hos-wip-strip {');
  const m = /z-index:\s*(\d+)/.exec(shell.slice(at, shell.indexOf('}', at)));
  assert.ok(at >= 0 && m, 'shell.js styles .hos-wip-strip with a z-index');
  return Number(m[1]);
})();

/** Rules that are not page furniture standing at the bottom, each with its reason. */
const EXEMPT = {
  '.hos-wip-strip': 'the strip itself',
  '#hos-space-bg': 'the starfield painted behind every page (z-index -1)',
  '.files-edit-foot': 'sticky to the foot of the Files page\'s own scrolling pane, not the window; the pane is page content, which the strip\'s spacer clears',
};

const SKIP_DIRS = new Set(['dist', 'node_modules', 'vendor']);
function files(dir, out = []) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) { if (!SKIP_DIRS.has(e.name)) files(p, out); }
    else if (/\.(css|html|js)$/.test(e.name) && !/\.min\.js$/.test(e.name) && e.name !== 'i18n-en.js') out.push(p);
  }
  return out;
}

/** `prop: value` pairs of a declaration list, lower-cased names, comments and !important dropped. */
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

/** The bottom part of an `inset` shorthand. */
function insetBottom(v) {
  const parts = v.trim().split(/\s+(?![^(]*\))/);
  return parts.length === 1 ? parts[0] : parts.length === 2 ? parts[0] : parts[2];
}
function insetTop(v) {
  return v.trim().split(/\s+(?![^(]*\))/)[0];
}

const isZero = (v) => v !== undefined && /^0(px|rem|em|%)?$/.test(v.trim());

/** Every rule-like piece of `src`: {selector, body, line}. */
function pieces(src) {
  const out = [];
  const starts = [0];
  for (let i = src.indexOf('\n'); i >= 0; i = src.indexOf('\n', i + 1)) starts.push(i + 1);
  const lineOf = (at) => {
    let lo = 0, hi = starts.length - 1;
    while (lo < hi) { const mid = (lo + hi + 1) >> 1; if (starts[mid] <= at) lo = mid; else hi = mid - 1; }
    return lo + 1;
  };
  // CSS rules: a brace pair with no brace inside, its selector the text back to the previous
  // brace, semicolon, quote or line. Bodies that read as script (an `=`, a statement) are not CSS.
  const re = /\{([^{}]*)\}/g;
  let m;
  while ((m = re.exec(src))) {
    const body = m[1];
    if (/[=]/.test(body) || /\breturn\b|\bfunction\b/.test(body)) continue;
    const before = src.slice(Math.max(0, m.index - 300), m.index).replace(/\/\*[\s\S]*?\*\//g, '');
    const sel = before.split(/[{};\n'"`+]/).pop().trim();
    out.push({ selector: sel, body, line: lineOf(m.index) });
  }
  // style="..." attributes, in markup and in markup a script writes.
  const re2 = /style\s*=\s*(\\?["'])([\s\S]*?)\1/g;
  while ((m = re2.exec(src))) out.push({ selector: 'style="..."', body: m[2], line: lineOf(m.index) });
  // el.style.cssText = '...' (or "..." or `...`).
  const re3 = /cssText\s*\+?=\s*(['"`])([\s\S]*?)\1/g;
  while ((m = re3.exec(src))) out.push({ selector: 'cssText', body: m[2], line: lineOf(m.index) });
  return out;
}

/** Every bottom-anchored fixed or sticky rule under web/, with what it says (scanned once). */
let scanned = null;
function scan() {
  if (scanned) return scanned;
  const found = [];
  for (const file of files(WEB)) {
    const src = fs.readFileSync(file, 'utf8').replace(/\r\n/g, '\n');
    const all = pieces(src);
    const fixedSelectors = new Set(all.filter((p) => /^(fixed|sticky)$/.test(decls(p.body).position || '')).map((p) => p.selector));
    for (const p of all) {
      const d = decls(p.body);
      const fixed = /^(fixed|sticky)$/.test(d.position || '');
      if (!fixed && !(fixedSelectors.has(p.selector) && (d.bottom || d.inset) && !d.position)) continue;
      let bottom = d.bottom !== undefined ? d.bottom : d.inset !== undefined ? insetBottom(d.inset) : undefined;
      const top = d.top !== undefined ? d.top : d.inset !== undefined ? insetTop(d.inset) : undefined;
      // A fixed panel as tall as the window reaches its bottom too (the chat's thread panel).
      const fullHeight = fixed && bottom === undefined && d.height !== undefined && /\b100[dsl]?vh\b/.test(d.height);
      if (fullHeight) bottom = 'auto; height: ' + d.height;
      if (bottom === undefined || bottom === 'auto') continue;
      found.push({
        where: path.relative(WEB, file).replace(/\\/g, '/') + ':' + p.line,
        selector: p.selector,
        bottom,
        z: d['z-index'] !== undefined && /^-?\d+$/.test(d['z-index']) ? Number(d['z-index']) : 0,
        covers: isZero(top) && (fullHeight ? /^100[dsl]?vh$/.test(d.height.trim()) : isZero(bottom)),
      });
    }
  }
  scanned = found;
  return found;
}

test('every element fixed at the bottom of the window stands on the warning, or covers it on purpose', () => {
  const found = scan();
  assert.ok(found.length >= 20, 'the scan found only ' + found.length + ' bottom-anchored rules: is it still reading web/?');
  const wrong = found.filter((r) => {
    if (EXEMPT[r.selector]) return false;
    if (r.bottom.includes('--hos-wip-h')) return false;
    if (r.covers && r.z > STRIP_Z) return false; // a layer over the whole window, the strip included
    return true;
  });
  assert.deepStrictEqual(
    wrong.map((r) => `${r.where} ${r.selector} { bottom: ${r.bottom}; z-index: ${r.z} }`),
    [],
    'these sit under the unfinished-software warning: give each `bottom: var(--hos-wip-h, 0px)` (or calc(<its offset> + var(--hos-wip-h, 0px))), or make a full-window layer stand above the strip (z-index over ' + STRIP_Z + ')'
  );
});

// A page box sized to the window (`height: calc(100vh - <the menu>)`, the full-height app pages)
// must leave the strip its room too: the strip's spacer goes after it in the page, so a box as tall
// as the window less the menu makes the whole window scroll by the strip's height, a second
// scrollbar over the page's own. Red first, 2026-10-31: Accord, Calendar, Inventory (2), Maps,
// Notes (2), Profile and the Files item window's panes; Settings had been fixed in the seams pass.
test('every page box as tall as the window leaves the warning its room', () => {
  const wrong = [];
  let seen = 0;
  for (const file of files(WEB)) {
    const src = fs.readFileSync(file, 'utf8').replace(/\r\n/g, '\n');
    for (const p of pieces(src)) {
      const d = decls(p.body);
      if (/^(fixed|sticky|absolute)$/.test(d.position || '')) continue; // the scan above, or inside its own box
      const h = d.height;
      if (h === undefined || !/\b100[dsl]?vh\s*-/.test(h)) continue;
      seen++;
      if (!h.includes('--hos-wip-h')) wrong.push(`${path.relative(WEB, file).replace(/\\/g, '/')}:${p.line} ${p.selector} { height: ${h} }`);
    }
  }
  assert.ok(seen >= 5, 'the scan found only ' + seen + ' window-tall page boxes: is it still reading web/?');
  assert.deepStrictEqual(wrong, [], 'these make the page scroll by the warning\'s height: subtract var(--hos-wip-h, 0px) in the calc()');
});

test('the scan still finds the footer, which stands on the strip', () => {
  // The control: a rule known to be bottom-anchored is found and passes for the right reason.
  const footer = scan().filter((r) => r.selector === '.site-footer');
  assert.ok(footer.length >= 1, 'the scan no longer sees shell.js\'s .site-footer');
  assert.ok(footer.every((r) => r.bottom.includes('--hos-wip-h')), JSON.stringify(footer));
  // And the exempt rules are still there to be exempt (a stale entry would hide nothing, but lie).
  for (const sel of Object.keys(EXEMPT)) assert.ok(scan().some((r) => r.selector === sel), sel + ' is no longer on the website: drop it from EXEMPT');
});
