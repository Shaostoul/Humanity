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

  const api = { load, use, assess, ready: () => !!list, LIST_URL };
  root.HosFileRisk = api;
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  // In a page, fetch the list as soon as the script loads, so it is in before any file is shown.
  else if (typeof root.document !== 'undefined') load();
})(typeof window !== 'undefined' ? window : globalThis);
