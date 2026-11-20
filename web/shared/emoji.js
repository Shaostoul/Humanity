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

  const api = { tokens, keyOf, filesFrom, pieces, BASE };

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

    api.parse = parse;
    api.ready = ready;
    window.hosEmoji = api;
  }
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
})();
