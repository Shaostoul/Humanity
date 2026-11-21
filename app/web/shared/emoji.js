/**
 * Emoji pictures from our own copies (2026-11-20). Every emoji in a piece of text becomes an <img>
 * of its picture under /assets/emoji/ on THIS server, so no visitor's browser asks another site for
 * an emoji (until today the chat used Twemoji, whose pictures came from cdn.jsdelivr.net, one of
 * them pinned to @latest: docs/PRIORITIES.md 6b). The pictures are OpenMoji's, chosen by the
 * operator (2026-11-20: "of that list the OpenMoji sounds better"), CC BY-SA 4.0 and credited in
 * LICENSES.md and on the Credits page (data/credits.ron "openmoji"). Volunteers' own drawings will
 * replace them one at a time: assets/emoji/sets.json lists the sets in the order they are looked in.
 *
 * The rules, in one place:
 *   - What counts as an emoji: an emoji drawn as an emoji by default (😀), or any pictograph with
 *     U+FE0F after it (❤️, ©️); a skin tone after a person or hand (👍🏽); sequences joined by U+200D
 *     (👨‍👩‍👧, ❤️‍🔥, 🏳️‍🌈); a pair of regional letters (a flag, 🇺🇸); a keycap (#️⃣); a black flag
 *     with tag letters (a subdivision flag, England's). A pictograph that is text by default (©, ☺)
 *     stays text without U+FE0F, except the red heart ❤, which Twemoji also drew bare; anything with
 *     U+FE0E (draw as text) after it stays text. The same rule Twemoji followed, so nothing that was
 *     text becomes a picture and nothing that was a picture becomes text.
 *   - Which picture: pictures are named by code points in upper-case hex joined by "-" (OpenMoji's
 *     naming). Each is looked up with every U+FE0F taken out of both sides, because OpenMoji names a
 *     single emoji without it (2764.svg) and keeps it inside sequences (2764-FE0F-200D-1F525.svg),
 *     while people type either form (scripts/lib/emoji-pack.js keyOfName is the same rule).
 *   - When no set has the whole sequence: a joined sequence shows its parts (as a browser would),
 *     a pair of letters that is no flag shows the two letters, a skin tone on something that has
 *     none shows both, a subdivision flag nobody drew shows the black flag, and anything with no
 *     picture at all stays text. Every picture's alt is the exact text it stands for.
 *   - Where: every text node under the element given, but not inside script, style, textarea,
 *     select, iframe, noscript or an SVG drawing; a picture already made is left alone, so parsing
 *     twice changes nothing.
 *
 * The list of pictures is assets/emoji/index.json (generated: node scripts/gen-emoji-index.js),
 * fetched once. An element given before it arrives is drawn when it arrives. If it cannot be
 * fetched, emoji stay as the system draws them and the console says why.
 *
 * Loaded by /chat (web/chat/index.html). window.hosEmoji = { parse, ready, tokens, keyOf };
 * module.exports for node (scripts/tests/emoji-web.test.js). The desktop app's plan for the same
 * pictures: docs/design/emoji.md.
 */
(function () {
  'use strict';

  const BASE = '/assets/emoji/';
  const ZWJ = '‍';
  const VS16 = '️';
  const BLACK_FLAG = '\u{1F3F4}';

  // One emoji character with what may follow it. A person or hand with a skin tone; an emoji drawn
  // as one by default, unless U+FE0E asks for text; any pictograph followed by U+FE0F; and a
  // text-by-default pictograph that starts a joined sequence typed without U+FE0F (❤ U+200D 🔥,
  // which Unicode calls minimally qualified and still shows as the heart on fire).
  // One text-by-default exception, the red heart U+2764: Twemoji drew it bare (its only one, checked
  // 2026-11-20 against every text-by-default pictograph), and the chat stores the heart reaction bare
  // (data/reactions.json, the relay's canonical form), so without it the first reaction is a glyph.
  const FIRST = '(?:\\p{Emoji_Modifier_Base}\\uFE0F?\\p{Emoji_Modifier}' +
    '|\\p{Emoji_Presentation}(?!\\uFE0E)\\uFE0F?' +
    '|\\u2764(?!\\uFE0E)\\uFE0F?' +
    '|\\p{Extended_Pictographic}\\uFE0F' +
    '|\\p{Extended_Pictographic}(?=\\u200D[\\p{Extended_Pictographic}\\p{Emoji_Presentation}]))';
  // After a joiner, a text-by-default pictograph needs no U+FE0F (the joiner already says emoji).
  const NEXT = '(?:\\p{Emoji_Modifier_Base}\\uFE0F?\\p{Emoji_Modifier}' +
    '|\\p{Extended_Pictographic}\\uFE0F?' +
    '|\\p{Emoji_Presentation}\\uFE0F?)';
  // The order matters: at each place the first alternative that matches wins, so the flag pair, the
  // keycap and the tag sequence are tried before a lone character could take their first letter.
  const EMOJI = new RegExp([
    '\\p{Regional_Indicator}{2}',
    '[#*0-9]\\uFE0F?\\u20E3',
    '\\u{1F3F4}[\\u{E0020}-\\u{E007E}]+\\u{E007F}',
    FIRST + '(?:\\u200D' + NEXT + ')*',
  ].join('|'), 'gu');

  /** Elements whose text is never turned into pictures (Twemoji's list). */
  const SKIP = /^(?:iframe|noframes|noscript|script|select|style|textarea)$/;

  function hexOf(ch) {
    return ch.codePointAt(0).toString(16).toUpperCase().padStart(4, '0');
  }

  /** The key a piece of text is looked up by: its code points in upper-case hex, joined by "-",
   *  with every U+FE0F left out. */
  function keyOf(text) {
    return Array.from(text).filter((c) => c !== VS16).map(hexOf).join('-');
  }

  /** The picture of each key, from the index (assets/emoji/index.json): the first set that has a
   *  key draws it. */
  function filesFrom(index) {
    const files = new Map();
    for (const set of (index && index.sets) || []) {
      for (const name of String(set.names || '').split(' ')) {
        if (!name) continue;
        const key = name.split('-').filter((cp) => cp !== 'FE0F').join('-');
        if (!files.has(key)) files.set(key, BASE + set.dir + '/' + name + '.svg');
      }
    }
    return files;
  }

  /** One matched emoji as pictures and text: [{ alt, url }] when a set has it whole, otherwise its
   *  parts (the fallbacks in the comment at the top). */
  function pieces(seq, files) {
    const url = files.get(keyOf(seq));
    if (url) return [{ alt: seq, url }];
    const chars = Array.from(seq).filter((c) => c !== VS16);
    if (chars.length <= 1) return [{ text: seq }];
    if (seq.includes(ZWJ)) {
      const out = [];
      seq.split(ZWJ).forEach((part, i) => {
        if (i) out.push({ text: ZWJ });
        if (part) out.push(...pieces(part, files));
      });
      return out;
    }
    if (chars[0] === BLACK_FLAG && chars.length > 2) {
      const flag = files.get(hexOf(BLACK_FLAG));
      return flag ? [{ alt: seq, url: flag }] : [{ text: seq }];
    }
    return Array.from(seq).map((c) => {
      const u = c === VS16 ? null : files.get(hexOf(c));
      return u ? { alt: c, url: u } : { text: c };
    });
  }

  /** A whole string as pictures and text, in order, neighbouring text joined: [{ text } | { alt, url }]. */
  function tokens(text, files) {
    const out = [];
    const push = (p) => {
      const last = out[out.length - 1];
      if (p.text !== undefined && last && last.text !== undefined) last.text += p.text;
      else out.push(p);
    };
    let at = 0;
    EMOJI.lastIndex = 0;
    let m;
    while ((m = EMOJI.exec(text))) {
      if (m.index > at) push({ text: text.slice(at, m.index) });
      for (const p of pieces(m[0], files)) push(p);
      at = m.index + m[0].length;
    }
    if (at < text.length) push({ text: text.slice(at) });
    return out;
  }

  // ── The outline on a dark theme (2026-11-20, docs/design/emoji.md "The outline") ──────────────
  // OpenMoji draws every picture with black lines, which vanish on a dark ground. The operator
  // asked (2026-11-20) for "a faint light halo behind each emoji on dark themes, as a theme
  // setting so it can be adjusted or turned off". Its two tokens come from data/gui/theme.ron
  // through theme.css: --emoji-outline (the colour) and --emoji-outline-width (px; 0 is off, which
  // is what the light theme sets). The desktop draws the same outline into each picture it
  // rasterises (src/gui/widgets/emoji/outline.rs): the picture's silhouette grown by the width, in
  // the colour, beneath the picture. Here the same three steps are an SVG filter (feMorphology
  // growing the picture's alpha, feFlood and feComposite colouring it, feMerge laying the picture
  // over it), which web/chat/base.css applies to img.emoji while <html> has data-emoji-outline.

  /** A theme colour as written in theme.css (#rgb, #rrggbb, #rrggbbaa, rgb(), rgba()), as
   *  { r, g, b, a } with r, g, b 0 to 255 and a 0 to 1; null when it is not one of those. */
  function parseColour(text) {
    const s = String(text || '').trim().toLowerCase();
    let m = /^#([0-9a-f]{3,8})$/.exec(s);
    if (m) {
      let h = m[1];
      if (h.length === 3 || h.length === 4) h = h.split('').map((c) => c + c).join('');
      if (h.length !== 6 && h.length !== 8) return null;
      const v = (i) => parseInt(h.slice(i, i + 2), 16);
      return { r: v(0), g: v(2), b: v(4), a: h.length === 8 ? v(6) / 255 : 1 };
    }
    m = /^rgba?\(\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)\s*(?:,\s*([\d.]+)\s*)?\)$/.exec(s);
    if (m) return { r: +m[1], g: +m[2], b: +m[3], a: m[4] === undefined ? 1 : +m[4] };
    if (s === 'transparent') return { r: 0, g: 0, b: 0, a: 0 };
    return null;
  }

  /** The outline the two tokens ask for: { on, radius (px), colour ('rgb(r, g, b)'), opacity }.
   *  Off when the width is 0 or not a number, or the colour is unreadable or fully transparent. */
  function outlineFrom(widthText, colourText) {
    const radius = parseFloat(String(widthText || '').trim());
    const c = parseColour(colourText);
    if (!(radius > 0) || !c || !(c.a > 0)) return { on: false, radius: 0, colour: '', opacity: 0 };
    return { on: true, radius, colour: `rgb(${Math.round(c.r)}, ${Math.round(c.g)}, ${Math.round(c.b)})`, opacity: c.a };
  }

  const OUTLINE_ID = 'hos-emoji-outline';
  const SVG_NS = 'http://www.w3.org/2000/svg';

  const api = { tokens, keyOf, filesFrom, pieces, parseColour, outlineFrom, OUTLINE_ID, BASE };

  if (typeof window !== 'undefined' && typeof document !== 'undefined') {
    let files = null;
    const pending = [];

    const image = (p) => {
      const img = document.createElement('img');
      img.className = 'emoji';
      img.setAttribute('alt', p.alt);
      img.setAttribute('draggable', 'false');
      img.setAttribute('loading', 'lazy');
      img.setAttribute('decoding', 'async');
      // A picture that fails to load (the server unreachable) goes back to the text it stood for.
      img.onerror = function () {
        if (this.parentNode) this.parentNode.replaceChild(document.createTextNode(this.getAttribute('alt')), this);
      };
      img.setAttribute('src', p.url);
      return img;
    };

    const textNodes = (node, out) => {
      for (const child of Array.from(node.childNodes || [])) {
        if (child.nodeType === 3) out.push(child);
        else if (child.nodeType === 1 && !('ownerSVGElement' in child) && !SKIP.test(String(child.nodeName).toLowerCase())) textNodes(child, out);
      }
      return out;
    };

    const replaceText = (t) => {
      const value = t.nodeValue || '';
      if (!t.parentNode || !value) return;
      const parts = tokens(value, files);
      if (!parts.some((p) => p.url)) return;
      const frag = document.createDocumentFragment();
      for (const p of parts) frag.appendChild(p.url ? image(p) : document.createTextNode(p.text));
      t.parentNode.replaceChild(frag, t);
    };

    /** Turn every emoji under `node` (an element, or one text node) into its picture. Returns `node`. */
    const parse = (node) => {
      if (!node) return node;
      if (!files) {
        if (!pending.includes(node)) pending.push(node);
        return node;
      }
      if (node.nodeType === 3) replaceText(node);
      else textNodes(node, []).forEach(replaceText);
      return node;
    };

    const ready = (typeof fetch === 'function'
      ? fetch(BASE + 'index.json').then((r) => {
        if (!r.ok) throw new Error('HTTP ' + r.status);
        return r.json();
      })
      : Promise.reject(new Error('this browser has no fetch')))
      .then((index) => {
        files = filesFrom(index);
        pending.splice(0).forEach(parse);
        return true;
      })
      .catch((err) => {
        pending.length = 0;
        console.warn('[emoji] the emoji pictures are not available (' + (err && err.message) + '); emoji show as the system draws them');
        return false;
      });

    // The outline's filter (see the outline's note above `api`), made the first time it is on and
    // updated in place whenever the theme's tokens change.
    let outlineParts = null;
    const outlineFilter = () => {
      if (outlineParts) return outlineParts;
      const host = document.body || document.documentElement;
      if (!host || typeof document.createElementNS !== 'function') return null;
      const make = (tag, attrs) => {
        const e = document.createElementNS(SVG_NS, tag);
        for (const k of Object.keys(attrs)) e.setAttribute(k, attrs[k]);
        return e;
      };
      // Hidden by base.css (svg.hos-emoji-filters); sRGB so the halo blends as the desktop's does.
      const svg = make('svg', { class: 'hos-emoji-filters', width: '0', height: '0', 'aria-hidden': 'true', focusable: 'false' });
      const filter = make('filter', { id: OUTLINE_ID, x: '-25%', y: '-25%', width: '150%', height: '150%', 'color-interpolation-filters': 'sRGB' });
      const grow = make('feMorphology', { in: 'SourceAlpha', operator: 'dilate', radius: '1', result: 'grown' });
      const flood = make('feFlood', { 'flood-color': 'rgb(0, 0, 0)', 'flood-opacity': '0', result: 'ink' });
      const halo = make('feComposite', { in: 'ink', in2: 'grown', operator: 'in', result: 'halo' });
      const merge = make('feMerge', {});
      merge.appendChild(make('feMergeNode', { in: 'halo' }));
      merge.appendChild(make('feMergeNode', { in: 'SourceGraphic' }));
      for (const p of [grow, flood, halo, merge]) filter.appendChild(p);
      svg.appendChild(filter);
      host.appendChild(svg);
      outlineParts = { svg, filter, grow, flood };
      return outlineParts;
    };

    /** Read the theme's two outline tokens and apply them; returns what was applied. */
    const applyOutline = () => {
      const root = document.documentElement;
      if (!root || typeof getComputedStyle !== 'function') return outlineFrom('', '');
      const style = getComputedStyle(root);
      const o = outlineFrom(style.getPropertyValue('--emoji-outline-width'), style.getPropertyValue('--emoji-outline'));
      const parts = o.on ? outlineFilter() : null;
      if (parts) {
        parts.grow.setAttribute('radius', String(o.radius));
        parts.flood.setAttribute('flood-color', o.colour);
        parts.flood.setAttribute('flood-opacity', String(o.opacity));
        root.setAttribute('data-emoji-outline', '');
      } else {
        root.removeAttribute('data-emoji-outline');
      }
      return o;
    };

    // Once the page's body exists, and again whenever <html>'s theme changes (shell.js sets
    // data-theme for the light theme; a class or an inline style can carry a preset).
    const watchOutline = () => {
      applyOutline();
      if (typeof MutationObserver === 'function' && document.documentElement) {
        new MutationObserver(applyOutline).observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme', 'class', 'style'] });
      }
    };
    if (document.readyState === 'loading' && typeof document.addEventListener === 'function') document.addEventListener('DOMContentLoaded', watchOutline);
    else watchOutline();

    api.parse = parse;
    api.ready = ready;
    api.applyOutline = applyOutline;
    window.hosEmoji = api;
  }
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
})();
