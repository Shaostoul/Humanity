// The website's layer scale and how to read a z-index written on it (navigation rung 3,
// 2026-11-04; docs/design/navigation.md section 6). The scale is theme.css's `--z-*` custom
// properties, which scripts/gen-theme-css.js writes from data/gui/theme.ron's `z_*` tokens, the
// same table the desktop's src/gui/layers.rs names (`Layer::web_token`). A larger z-index is
// nearer the viewer on the website; his numbers count the other way (lower is nearer).
//
// A z-index on the scale is `var(--z-<name>)`, or `calc(var(--z-<name>) + N)` for a part raised
// inside its own band (a nested popup over the one it came from, the page's own parts over their
// neighbours), or `calc(var(--z-page) - N)` for what is painted behind the page.
//
// Shared by z-index-scale-web.test.js, wip-strip-room-web.test.js and wip-strip-web.test.js.

const fs = require('node:fs');
const path = require('node:path');

/** His order, nearest the viewer first: [token, his number]. */
const ORDER = [
  ['tooltip', 1],
  ['urgent', 2],
  ['popup-controls', 3],
  ['popup', 4],
  ['shade', 5],
  ['menu-1-drop', 9],
  ['menu-1', 10],
  ['menu-2-drop', 19],
  ['menu-2', 20],
  ['warning', 21],
  ['footer', 30],
  ['page-float', 40],
  ['page', 41],
];

/** The `--z-*` values theme.css defines (its first, dark-theme :root block), by name. */
function scale(webDir) {
  const css = fs.readFileSync(path.join(webDir, 'shared', 'theme.css'), 'utf8');
  const out = {};
  const re = /--z-([a-z0-9-]+):\s*(-?\d+);/g;
  let m;
  while ((m = re.exec(css))) if (!(m[1] in out)) out[m[1]] = Number(m[2]);
  return out;
}

/**
 * The number a z-index value comes to, or null when it is neither a number nor on the scale.
 * `{ value, token, offset, literal }`: `token` is the scale's name (null for a bare number).
 */
function resolve(value, sc) {
  const v = String(value).trim().replace(/\s*!important$/i, '');
  if (/^-?\d+$/.test(v)) return { value: Number(v), token: null, offset: 0, literal: true };
  let m = /^var\(--z-([a-z0-9-]+)\)$/.exec(v);
  if (m) return m[1] in sc ? { value: sc[m[1]], token: m[1], offset: 0, literal: false } : null;
  m = /^calc\(\s*var\(--z-([a-z0-9-]+)\)\s*([+-])\s*(\d+)\s*\)$/.exec(v);
  if (m && m[1] in sc) {
    const off = (m[2] === '-' ? -1 : 1) * Number(m[3]);
    return { value: sc[m[1]] + off, token: m[1], offset: off, literal: false };
  }
  return null;
}

module.exports = { ORDER, scale, resolve };
