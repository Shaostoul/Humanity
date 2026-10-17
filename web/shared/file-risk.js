/* ── file-risk.js ────────────────────────────────────────────────────────────
 * What a file is, said before someone opens one they were sent (2026-10-17,
 * from a suggestion in the project's Discord: "something that says 'hey, this
 * is an executable/script/zip/whatever, make sure you scan anything before
 * opening it'"). The desktop app's twin is src/net/file_risk.rs; both read
 * one list, /data/safety/file_risks.json, and both answer the shared cases in
 * scripts/tests/fixtures/file-risk-cases.json the same way.
 *
 * Not an antivirus: it reads the file's name and, when the page has them, its
 * first bytes, and says what kind of thing the file is and what to be careful
 * of. A name that says a picture, a video, a song, a document or a 3D model
 * over the bytes of a program or an archive is called out as disguised. The
 * receiving page is where this belongs: a private file is encrypted before it
 * is uploaded, so the server never sees its real name or contents.
 *
 * HosFileRisk.load() fetches the list (once); HosFileRisk.assess(name, head)
 * answers {kind, disguised, words} or null (`head`: the first bytes as a
 * Uint8Array, or null when not known). Until the list is in, assess answers null.
 *
 * Every file also gets its TYPE shown, always (the operator, 2026-10-17: for
 * "people inexperienced with technology and possibly illiterate people"): a
 * badge with a drawing, a word and a colour. HosFileRisk.fileType(name, head)
 * answers {id, label, icon, level, ending, disguised}, and HosFileRisk.badgeHtml
 * (name, head) the badge itself (class `file-badge level-<level>`, its drawing
 * an inline SVG built from the list's `icons`), the desktop app's
 * gui/widgets/file_badge.rs. Before the list is in, the badge is a plain grey
 * "File" with the file's ending, and `fillBadges(root)` redraws every
 * `[data-file-badge]` once it is.
 */
(function (root) {
  'use strict';

  const LIST_URL = '/data/safety/file_risks.json';
  let list = null;
  let loading = null;

  /** Fetch the list once (`fetchFn` for tests); resolves to it, or null if it could not be read. */
  function load(fetchFn) {
    if (list) return Promise.resolve(list);
    const f = fetchFn || (typeof root.fetch === 'function' ? root.fetch.bind(root) : null);
    if (!f) return Promise.resolve(null);
    if (!loading) {
      loading = Promise.resolve(f(LIST_URL))
        .then((r) => (r && r.ok !== false ? r.json() : null))
        .then((d) => { if (d && Array.isArray(d.kinds)) list = d; loading = null; return list; })
        .catch(() => { loading = null; return null; });
    }
    return loading;
  }

  /** Use `d` as the list (the tests, and a page that already has it). */
  function use(d) { list = d; }

  /** Whether `head` starts with the bytes the hex text `magic` spells. */
  function startsWithHex(head, magic) {
    if (!head || !magic || magic.length % 2) return false;
    const n = magic.length / 2;
    if (head.length < n) return false;
    for (let i = 0; i < n; i++) {
      if (head[i] !== parseInt(magic.slice(i * 2, i * 2 + 2), 16)) return false;
    }
    return n > 0;
  }

  /** The longest of `endings` that `name` ends with (after a dot), or ''. */
  function ending(name, endings) {
    let best = '';
    for (const e of endings || []) if (name.endsWith('.' + e) && e.length > best.length) best = e;
    return best;
  }

  /** The file's own ending as shown in the words: ".exe", or "no ending". */
  function shownEnding(name) {
    const i = name.lastIndexOf('.');
    if (i <= 0 || i === name.length - 1) return 'no ending';
    const ext = name.slice(i + 1);
    return /[\/\\ ]/.test(ext) ? 'no ending' : '.' + ext;
  }

  /**
   * A word of caution about a file named `name` whose first bytes are `head`, or null. The name
   * decides first; when it says nothing risky, the bytes are checked (as src/net/file_risk.rs
   * `assess` does, in the same order).
   */
  function assess(name, head, d) {
    const data = d || list;
    if (!data) return null;
    const lower = String(name || '').trim().toLowerCase();
    let byName = null;
    for (const k of data.kinds) {
      const e = ending(lower, k.extensions);
      if (e && (!byName || e.length > byName.ext.length)) byName = { kind: k, ext: e };
    }
    if (byName) return { kind: byName.kind.id, disguised: false, words: byName.kind.words.split('{ext}').join('.' + byName.ext) };
    if (!head || !head.length) return null;
    const actual = data.kinds.find((k) => (k.magic || []).some((m) => startsWithHex(head, m)));
    if (!actual) return null;
    let claimed = null;
    for (const n of data.named_as || []) {
      const e = ending(lower, n.extensions);
      if (e && (!claimed || e.length > claimed.ext.length)) claimed = { named: n, ext: e };
    }
    if (claimed) {
      // A harmless kind whose bytes are allowed for it (a text file starting "#!") gets no word.
      if (!claimed.named.disguised_if.includes(actual.id)) return null;
      return {
        kind: actual.id,
        disguised: true,
        words: data.disguised.split('{claimed}').join(claimed.named.name).split('{ext}').join('.' + claimed.ext).split('{actual}').join(actual.name),
      };
    }
    return { kind: actual.id, disguised: false, words: actual.words.split('{ext}').join(shownEnding(lower)) };
  }

  /**
   * The type of a file named `name` whose first bytes are `head`, for its badge (always an
   * answer): by its name (a kind that needs care first, the longest ending), then, when the bytes
   * are known, what they say; a harmless name over a program's, a script's or an archive's bytes is
   * that kind, disguised and `danger`. Anything else is the list's `unknown`. As
   * src/net/file_risk.rs `file_type` does, in the same order.
   */
  function fileType(name, head, d) {
    const data = d || list;
    const lower = String(name || '').trim().toLowerCase();
    const shown = shownEnding(lower);
    const end = shown.startsWith('.') ? shown : '';
    if (!data) return { id: 'unknown', label: 'File', icon: 'unknown', level: 'unknown', ending: end, disguised: false };
    const ofKind = (k, disguised) => ({ id: k.id, label: k.label, icon: k.icon, level: disguised ? 'danger' : k.level, ending: end, disguised });
    let byName = null;
    for (const k of data.kinds) {
      const e = ending(lower, k.extensions);
      if (e && (!byName || e.length > byName.ext.length)) byName = { kind: k, ext: e };
    }
    if (byName) return ofKind(byName.kind, false);
    const h = head && head.length ? head : null;
    const actual = h ? data.kinds.find((k) => (k.magic || []).some((m) => startsWithHex(h, m))) : null;
    let claimed = null;
    for (const n of data.named_as || []) {
      const e = ending(lower, n.extensions);
      if (e && (!claimed || e.length > claimed.ext.length)) claimed = { named: n, ext: e };
    }
    if (claimed && actual && claimed.named.disguised_if.includes(actual.id)) return ofKind(actual, true);
    if (claimed) return { id: claimed.named.id, label: claimed.named.label, icon: claimed.named.icon, level: claimed.named.level, ending: end, disguised: false };
    if (actual) return ofKind(actual, false);
    const u = data.unknown || {};
    return { id: u.id || 'unknown', label: u.label || 'File', icon: u.icon || 'unknown', level: u.level || 'unknown', ending: end, disguised: false };
  }

  const escText = (t) => String(t == null ? '' : t).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));

  /** The drawing named `icon` as SVG markup (`size` pixels), from the list's `icons`. */
  function iconSvg(icon, size, d) {
    const data = d || list;
    const icons = (data && data.icons) || {};
    const shapes = icons[icon] || icons[(data && data.unknown && data.unknown.icon) || 'unknown'] || [];
    const n = (v) => Number(v) || 0;
    const parts = shapes.map((sh) => {
      const fill = sh.fill ? 'currentColor' : 'none';
      if (sh.rect) return '<rect x="' + n(sh.rect[0]) + '" y="' + n(sh.rect[1]) + '" width="' + n(sh.rect[2]) + '" height="' + n(sh.rect[3]) + '" fill="' + fill + '"/>';
      if (sh.line) return '<line x1="' + n(sh.line[0]) + '" y1="' + n(sh.line[1]) + '" x2="' + n(sh.line[2]) + '" y2="' + n(sh.line[3]) + '"/>';
      if (sh.circle) return '<circle cx="' + n(sh.circle[0]) + '" cy="' + n(sh.circle[1]) + '" r="' + n(sh.circle[2]) + '" fill="' + fill + '"/>';
      if (sh.poly) {
        const pts = [];
        for (let i = 0; i + 1 < sh.poly.length; i += 2) pts.push(n(sh.poly[i]) + ',' + n(sh.poly[i + 1]));
        return sh.closed ? '<polygon points="' + pts.join(' ') + '" fill="' + fill + '"/>' : '<polyline points="' + pts.join(' ') + '" fill="none"/>';
      }
      return '';
    }).join('');
    const px = Number(size) || 16;
    return '<svg class="file-badge-icon" viewBox="0 0 48 48" width="' + px + '" height="' + px + '" aria-hidden="true" stroke="currentColor" stroke-width="4" stroke-linecap="round" stroke-linejoin="round">' + parts + '</svg>';
  }

  /**
   * The badge for a file named `name` (first bytes `head`, or null): its drawing, its type's word
   * and its ending, coloured by its level. Carries `data-file-badge` (the name) and, when the
   * bytes are known, `data-file-head` (them, in hex), so `fillBadges` redraws it from both: a
   * disguised file stays red however often it is redrawn. Every text in it is escaped.
   */
  function badgeHtml(name, head, d) {
    const t = fileType(name, head, d);
    const hex = head && head.length ? Array.from(head.slice(0, 16), (b) => (b & 255).toString(16).padStart(2, '0')).join('') : '';
    const end = t.ending ? '<span class="file-badge-ending">' + escText(t.ending) + '</span>' : '';
    const title = t.disguised ? t.label + ', pretending to be something else' : t.label;
    return '<span class="file-badge level-' + escText(t.level) + (t.disguised ? ' disguised' : '') + '" data-file-badge="' + escText(name) + '"' + (hex ? ' data-file-head="' + hex + '"' : '') + ' data-file-type="' + escText(t.id) + '" title="' + escText(title) + '">'
      + iconSvg(t.icon, 16, d) + '<span class="file-badge-label">' + escText(t.label) + '</span>' + end + '</span>';
  }

  /** Redraw every `[data-file-badge]` under `root` from the list (a badge drawn before it was in). */
  function fillBadges(root) {
    if (!list || !root || typeof root.querySelectorAll !== 'function') return;
    for (const el of Array.from(root.querySelectorAll('[data-file-badge]'))) {
      const head = el.dataset.fileHead ? Uint8Array.from((el.dataset.fileHead.match(/../g) || []).map((b) => parseInt(b, 16))) : null;
      el.outerHTML = badgeHtml(el.dataset.fileBadge, head);
    }
  }

  const api = { load, use, assess, fileType, iconSvg, badgeHtml, fillBadges, ready: () => !!list, LIST_URL };
  root.HosFileRisk = api;
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  // In a page, fetch the list as soon as the script loads, so it is in before any file is shown.
  else if (typeof root.document !== 'undefined') load();
})(typeof window !== 'undefined' ? window : globalThis);
