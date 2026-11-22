// A MEDIA PLAYER'S CONTROLS AS DRAWN ICONS, the website's twin of src/gui/widgets/transport_icons.rs
// (docs/design/music-page.md section 28.5; the operator, 2026-11-21: "Let's add icons for the buttons
// too. Like play, fastword, etc."). Each icon is inline SVG we draw ourselves from the same shapes as
// the app's (a 24-unit square; the app's fractions times 24), in the text colour (currentColor): no icon
// font, no picture file, no licence, nothing downloaded. The button that carries one names it with its
// aria-label (words by key), so a screen reader hears "Next".
'use strict';

(function (root) {
  const P = (x, y) => (x * 24).toFixed(2) + ',' + (y * 24).toFixed(2);
  const tri = (l, t, r, b, dir) => dir > 0
    ? '<polygon points="' + P(l, t) + ' ' + P(r, (t + b) / 2) + ' ' + P(l, b) + '"/>'
    : '<polygon points="' + P(r, t) + ' ' + P(l, (t + b) / 2) + ' ' + P(r, b) + '"/>';
  const rect = (l, t, r, b) => '<rect x="' + (l * 24).toFixed(2) + '" y="' + (t * 24).toFixed(2) + '" width="' + ((r - l) * 24).toFixed(2) + '" height="' + ((b - t) * 24).toFixed(2) + '" rx="0.5"/>';
  const line = (pts) => '<polyline fill="none" stroke="currentColor" stroke-width="2.6" stroke-linejoin="round" points="' + pts.map((p) => P(p[0], p[1])).join(' ') + '"/>';
  const head = (x, y, dir, size) => {
    const bx = x - dir * size, s = size * 0.6;
    return '<polygon points="' + P(x, y) + ' ' + P(bx, y - s) + ' ' + P(bx, y + s) + '"/>';
  };

  // The icons, by the names the app's TransportIcon has.
  const SHAPES = {
    play: () => tri(0.22, 0.12, 0.86, 0.88, 1),
    pause: () => rect(0.2, 0.14, 0.4, 0.86) + rect(0.6, 0.14, 0.8, 0.86),
    previous: () => rect(0.14, 0.18, 0.26, 0.82) + tri(0.28, 0.18, 0.86, 0.82, -1),
    next: () => tri(0.14, 0.18, 0.72, 0.82, 1) + rect(0.74, 0.18, 0.86, 0.82),
    shuffle: () => line([[0.08, 0.28], [0.3, 0.28], [0.62, 0.72], [0.78, 0.72]]) + line([[0.08, 0.72], [0.3, 0.72], [0.62, 0.28], [0.78, 0.28]]) + head(0.94, 0.28, 1, 0.2) + head(0.94, 0.72, 1, 0.2),
    repeat: () => line([[0.16, 0.6], [0.16, 0.3], [0.66, 0.3]]) + line([[0.84, 0.4], [0.84, 0.7], [0.34, 0.7]]) + head(0.92, 0.3, 1, 0.26) + head(0.08, 0.7, -1, 0.26),
    up_next: () => tri(0.1, 0.18, 0.28, 0.38, 1) + line([[0.36, 0.28], [0.9, 0.28]]) + line([[0.12, 0.52], [0.9, 0.52]]) + line([[0.12, 0.76], [0.9, 0.76]]),
    heart: () => {
      const pts = [];
      for (let i = 0; i < 28; i++) {
        const t = (i / 28) * Math.PI * 2;
        const x = 16 * Math.pow(Math.sin(t), 3);
        const y = 13 * Math.cos(t) - 5 * Math.cos(2 * t) - 2 * Math.cos(3 * t) - Math.cos(4 * t);
        pts.push([0.5 + x / 34, 0.54 - y / 34]);
      }
      return '<polygon fill="none" stroke="currentColor" stroke-width="2.2" points="' + pts.map((p) => P(p[0], p[1])).join(' ') + '"/>';
    },
    speaker: () => rect(0.1, 0.38, 0.26, 0.62) + '<polygon points="' + P(0.24, 0.38) + ' ' + P(0.5, 0.16) + ' ' + P(0.5, 0.84) + ' ' + P(0.24, 0.62) + '"/>' +
      '<path fill="none" stroke="currentColor" stroke-width="2.6" d="M ' + P(0.62, 0.38) + ' A 3.84 3.84 0 0 1 ' + P(0.62, 0.62) + ' M ' + P(0.66, 0.27) + ' A 7.2 7.2 0 0 1 ' + P(0.66, 0.73) + '"/>',
  };

  /** The inline SVG of icon `name` (a key of SHAPES), `size` pixels square, drawn in currentColor. */
  function svg(name, size) {
    const draw = SHAPES[name];
    if (!draw) return '';
    const px = size || 24;
    return '<svg viewBox="0 0 24 24" width="' + px + '" height="' + px + '" fill="currentColor" aria-hidden="true" focusable="false">' + draw() + '</svg>';
  }

  const api = { svg, NAMES: Object.keys(SHAPES) };
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
  else root.TransportIcons = api;
})(typeof window !== 'undefined' ? window : this);
