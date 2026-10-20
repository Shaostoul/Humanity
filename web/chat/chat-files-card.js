// ── chat-files-card.js ────────────────────────────────────────────────────
// A Media item posted in chat, shown as a card (2026-10-17). The operator: "can
// we make it when someone clicks on the file in chat it brings up the full popup
// window for that item instead of just having truncated version. I uploaded a
// set of files but, they're all part of the same thing. I'd rather people know
// exactly what they're downloading instead of guessing ... a card in the chat
// for the media thing(s). Like a simplified version of the full popup card. Card
// could start as a collapsed line ... switchable between expanded and collapsed."
//
// The desktop app is the original (src/gui/pages/files/chat_card.rs); this file
// copies its behaviour and words:
// - A message holding the address of an item on THIS site's Files page
//   (`https://<this origin>/files/<number>`, or `/files/<number>`; Add to Files's
//   "Also post in chat" posts exactly that, web/pages/files-add.js postText) shows,
//   under its words, a card for the item, and the address is taken out of the
//   words. Another website's /files/ pages stay plain links; punctuation after an
//   address is not part of it; each item once.
// - Collapsed (how it starts): a triangle to open it out ("Show more" / "Show
//   less"), the item's own file's type badge, its title (a click opens the item),
//   "2 files, 2 MB", and Open.
// - Expanded: also its cover picture (else its own picture when it is one) under
//   the Files page's rules for pictures (the desktop's tiles.rs show_of): the
//   protected setup leaves it out with its line; a labelled item says
//   "Labelled: Spoiler." with Show the picture; a picture from someone who is not
//   a friend says "Pictures from Ann show only when you choose." with Show the
//   picture; one too large to show here is left out. A click on the picture opens
//   the item. Then "by Ann in Fire flow arts", its description, and every file it
//   holds with its type badge and "name, size" ("name, cover picture, size").
// - Open, the title and the picture bring up the item's FULL window over the
//   chat: the Files page's own item page in a frame (/files/<id>?window=item,
//   /shared/upload-window.js), the same page with its Download for each file,
//   never a copy of it. The chat signs for the page in that frame (2026-10-18):
//   with a passphrase-wrapped identity the page has no key of its own, and a
//   members-only item read "not there". It lends that page its upload token too
//   (the same day), for the item's files: the cover, Play here and each Download.
// - The thread panel draws its messages' cards by the same rules (2026-10-18,
//   chat-messages.js putThreadMessage); the website's own place, the desktop has
//   no thread panel. Cards no longer in the document are let go (prune).
// - The item is asked for once per page (the same request the Files page opens an
//   item by its number with: POST /api/files/list signed over "media_list\n<ts>"
//   when the person has an identity here, so members see members-only items; else
//   GET /api/files?id=<n>&limit=1). Whether a card is open is remembered per
//   message and item for the page's life.
// - Everything the server sends goes in with textContent. A picture loads only
//   from this server's own addresses; a members-only one through the members'
//   route with this session's upload token in a header, shown through an object
//   URL (as the Files page does), so the token is never in an address.
//
// A stricter rule than the desktop's, on purpose: with the protected setup on,
// the picture is left out when EITHER the item's uploader or the message's
// sender is not a friend (the desktop asks about the uploader only), so a
// non-friend's message never shows a picture through a card.
//
// A classic script in the browser, loaded before app.js (which calls
// filesCardsPlan and filesCardsPut while it draws a message), reading the chat's
// own globals only when a card is drawn; a CommonJS module under Node, where
// scripts/tests/files-card-web.test.js runs it against a stand-in document.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  // What kind of file each one is, and its name as shown (/shared/file-risk.js): under Node the
  // module; in the browser the page's HosFileRisk (index.html loads it first), looked up when used.
  const FR_MODULE = typeof module === 'object' && module && module.exports ? require('../shared/file-risk.js') : null;
  const MIB = 1024 * 1024;
  // The kinds of item (the Files page's own file): the word a title without a name falls back to.
  const KINDS_URL = '/data/files/kinds.json';
  // How near its end a scrolled list still counts as at its bottom: a browser's scroll position can
  // stop a fraction of a pixel short of it (a zoomed page), never as far as a line of text.
  const BOTTOM_SLACK_PX = 4;
  /** Is the scrolled element `list` at its bottom (within BOTTOM_SLACK_PX)? */
  function atBottom(list) {
    const h = Number(list && list.scrollHeight) || 0;
    const top = Number(list && list.scrollTop) || 0;
    const seen = Number(list && list.clientHeight) || 0;
    return h - top - seen <= BOTTOM_SLACK_PX;
  }
  // The largest picture shown in a card (the desktop's image cache, MAX_IMAGE_BYTES).
  const MAX_PICTURE_BYTES = 16 * MIB;
  // What ends a sentence after an address: not part of it (the desktop's `links`).
  const LINK_TAIL = '.,;:!?)]}\'"';

  // The card's words (the desktop's chat_card.rs, item_view.rs NOT_THERE, tiles.rs LABEL_HIDDEN_LINE).
  const WORDS = Object.freeze({
    open: 'Open',
    openTip: 'See everything about it, with a Download for each file.',
    moreTip: 'Show more',
    lessTip: 'Show less',
    loading: 'Loading the item from the Files page...',
    loadingPicture: 'Loading the picture...',
    notThere: 'That item is not on the Files page, or you may not see it.',
    failed: 'This item from the Files page could not be loaded: ',
    showPicture: 'Show the picture',
    labelHidden: 'Hidden: the protected setup leaves out items with a content label.',
    // The Files page's own words for a members-only picture (web/pages/files-app.js FILES_WORDS).
    waitingToken: 'This members-only file shows once this page is signed in to the server.',
    fileFailed: 'This file could not be loaded: ',
  });
  const labelled = (names) => 'Labelled: ' + names.join(', ') + '.';
  const picturesFrom = (name) => 'Pictures from ' + name + ' show only when you choose.';

  // ── What a message links to ──

  /** One word of a message as an address and what ends the sentence after it: [link, tail]. */
  function splitTail(token) {
    let link = token;
    while (link && LINK_TAIL.includes(link[link.length - 1])) link = link.slice(0, -1);
    return [link, token.slice(link.length)];
  }

  /** The item number in `link` when it is shaped like a Media item's address (scheme in any case), else null. */
  function addressId(link) {
    let path = link;
    const scheme = /^https?:\/\//i.exec(link);
    if (scheme) {
      const rest = link.slice(scheme[0].length);
      const slash = rest.indexOf('/');
      if (slash < 0) return null;
      path = rest.slice(slash);
    }
    const m = /^\/files\/(\d{1,15})\/*$/.exec(path);
    return m ? Number(m[1]) : null;
  }

  /** Every token of `text` shaped like a Media item's address: [{raw, id}] (the desktop's `links`). */
  function links(text) {
    const out = [];
    for (const token of String(text == null ? '' : text).split(/\s+/)) {
      if (!token) continue;
      const link = splitTail(token)[0];
      const id = addressId(link);
      if (id != null) out.push({ raw: link, id });
    }
    return out;
  }

  /**
   * Is `raw` (an address in a message) on `origin`, this site? A bare path is. The scheme and the
   * host are read as a browser reads them, in any case ("Https://" is this site's too).
   */
  function onThisSite(raw, origin) {
    if (raw.startsWith('/')) return true;
    try { return new URL(raw).origin === new URL(String(origin)).origin; } catch (e) { return false; }
  }

  /** The Media items a message links to on this site (`origin`), in order, once each. */
  function itemLinks(text, origin) {
    const ids = [];
    for (const l of links(text)) if (onThisSite(l.raw, origin) && !ids.includes(l.id)) ids.push(l.id);
    return ids;
  }

  /**
   * `text` without the addresses of items `ids` on this site (`origin`; their cards show them). ONLY
   * AN EXACT ADDRESS IS TAKEN OUT (2026-10-17, review): a word of the message whose address (its
   * trailing punctuation off) IS this site's address of one of those items, `http(s)://<this
   * site>/files/<n>` or `/files/<n>`, and nothing inside another word, which once glued a
   * stranger's `htt/files/5ps://evil.example/login` into a live https:// link; another website's
   * /files/ page never, whatever its number. The punctuation after a taken address stays, as a word
   * of its own. A line with nothing taken out is kept exactly as it was; one with something taken
   * out is its own leading space and the words left, one space apart, and goes when none are left.
   * The desktop's twin is src/gui/pages/files/chat_card.rs `without_links`.
   */
  function withoutLinks(text, ids, origin) {
    const s = String(text == null ? '' : text);
    if (!ids || !ids.length) return s;
    const out = [];
    // Line by line at each "\n", as the desktop splits them (a line's own "\r" is its own).
    for (const line of s.split('\n')) {
      const words = [];
      let taken = false;
      for (const token of line.split(/\s+/)) {
        if (!token) continue;
        const [link, tail] = splitTail(token);
        const id = addressId(link);
        if (id != null && ids.includes(id) && onThisSite(link, origin)) {
          taken = true;
          if (tail) words.push(tail);
        } else {
          words.push(token);
        }
      }
      if (!taken) out.push(line);
      else if (words.length) out.push(/^\s*/.exec(line)[0] + words.join(' '));
    }
    return out.join('\n');
  }

  // ── What a card says ──

  function oneDecimal(x) {
    return x.toFixed(1).replace(/\.0$/, '');
  }
  /** Bytes as people read them, word for word the Files page's (files-app.js sizeWords). */
  function sizeWords(bytes) {
    const b = Math.max(0, Number(bytes) || 0);
    if (b >= 1024 * MIB) return oneDecimal(b / (1024 * MIB)) + ' GB';
    if (b >= MIB) return (b / MIB >= 100 ? String(Math.round(b / MIB)) : oneDecimal(b / MIB)) + ' MB';
    if (b >= 1024) return Math.round(b / 1024) + ' KB';
    return b + (b === 1 ? ' byte' : ' bytes');
  }
  const num = (x) => (Number.isFinite(Number(x)) ? Number(x) : 0);
  const trimmed = (x) => (typeof x === 'string' ? x.trim() : '');

  /** "2 files, 2 MB" ("1 file, 4 KB"): how many files, at least one, and how much in all. */
  function countLine(item) {
    const n = Math.max(1, Math.floor(num(item && item.file_count)));
    const total = num(item && item.total_bytes) > 0 ? num(item.total_bytes) : num(item && item.size_bytes);
    return n + (n === 1 ? ' file, ' : ' files, ') + sizeWords(total);
  }
  /** The stored name in an /uploads/ address, or ''. */
  function uploadName(url) {
    const s = String(url || '');
    return /^\/uploads\/[^/]+$/.test(s) ? s.slice('/uploads/'.length) : '';
  }
  /**
   * The kinds file (/data/files/kinds.json, which the Files page and the desktop read) as the card
   * needs it: [{id, one}], `one` the name of one item of the kind (else its label). Entries without
   * an id and a label are left out, as the Files page's parseKinds leaves them.
   */
  function parseKinds(data) {
    const list = data && Array.isArray(data.kinds) ? data.kinds : [];
    return list
      .filter((k) => k && typeof k.id === 'string' && k.id.trim() && typeof k.label === 'string' && k.label.trim())
      .map((k) => ({ id: k.id, one: typeof k.one === 'string' && k.one ? k.one : k.label }));
  }
  /** The word for one item of `kind` (the desktop's kinds::one_name): the kinds file's, else the kind as the server named it. */
  function kindWord(kind, kinds) {
    const k = (Array.isArray(kinds) ? kinds : []).find((x) => x.id === kind);
    return k ? k.one : String(kind || '');
  }
  /** Does the item's name come down to its kind's word (no title, no name of its file, no stored name)? */
  function namedByKind(item) {
    return !trimmed(item && item.title) && !trimmed(item && item.original_name) && !uploadName(item && item.url);
  }
  /**
   * What the item is called: its title, else its file's own name, else its stored name, else the
   * word for one item of its kind (`kinds`, the kinds file; the desktop's display_title and the
   * Files page's itemName say "3D model", never "model").
   */
  function displayTitle(item, kinds) {
    return trimmed(item && item.title) || trimmed(item && item.original_name) || uploadName(item && item.url)
      || kindWord(item && item.kind, kinds) || 'This item';
  }
  /** Who put it there, by name, trimmed ("someone" when the server knows no name; the desktop's by_name). */
  function byName(item) {
    return trimmed(item && item.uploader_name) || 'someone';
  }
  /** "by Ann in Fire flow arts" (the Files page's by-line, files-item.js byLine; the desktop's by_line). */
  function byLine(item) {
    const by = 'by ' + byName(item);
    const cat = item && item.category && typeof item.category.name === 'string' ? item.category.name.trim() : '';
    return cat ? by + ' in ' + cat : by;
  }
  /**
   * A file's name as it is shown (2026-10-17): /shared/file-risk.js plainName, so a direction mark
   * cannot make "photo<RLO>gnp.exe" read as "photoexe.png". Should that file not have loaded, every
   * format and control character goes (stricter, never looser).
   */
  function plainName(name) {
    const FR = FR_MODULE || root.HosFileRisk || null;
    if (FR && typeof FR.plainName === 'function') return FR.plainName(name);
    return String(name == null ? '' : name).replace(/[\p{Cc}\p{Cf}]/gu, '').trim();
  }
  /** The item's files, its own first (files-item.js filesOf). */
  function filesOf(item) {
    if (item && Array.isArray(item.files) && item.files.length) return item.files;
    return [{ file: 0, filename: item ? item.filename || '' : '', original_name: item ? item.original_name || '' : '', kind: item ? item.kind : '', size_bytes: item ? item.size_bytes : 0, url: item ? item.url : '' }];
  }
  /** The name its own file is downloaded under (files-app.js downloadName). */
  function ownName(item, kinds) {
    return trimmed(item && item.original_name) || (item && typeof item.filename === 'string' && item.filename.trim()) || uploadName(item && item.url) || displayTitle(item, kinds);
  }
  /** The name file `f` is downloaded under (files-item.js fileName). */
  function fileName(item, f, kinds) {
    if (!f || !f.file) return ownName(item, kinds);
    return trimmed(f.original_name) || trimmed(f.filename) || displayTitle(item, kinds) + '-' + item.id + '-' + f.file;
  }
  /** The name file `f` is shown by: plain (plainName), else the item's own name (files-item.js shownName). */
  function shownName(item, f, kinds) {
    return plainName(fileName(item, f, kinds)) || plainName(displayTitle(item, kinds)) || 'This item';
  }
  /** Every line of the expanded card's file list: [{name, line}], its own first, then the others, then the cover. */
  function fileLines(item, kinds) {
    const rows = filesOf(item).map((f) => [f, false]).concat(item && item.cover ? [[item.cover, true]] : []);
    return rows.map(([f, cover]) => {
      const name = shownName(item, f, kinds);
      return { name, line: name + (cover ? ', cover picture, ' : ', ') + sizeWords(f.size_bytes) };
    });
  }

  /**
   * The path of a file on this site's own addresses, or null: /uploads/<name> or the members'
   * route /api/files/file/<id> (with `?file=<n>` for another of its files), as the Files page's
   * ownFilePath. Nothing from anywhere else is ever loaded.
   */
  function ownPath(url, origin) {
    if (typeof url !== 'string' || !url) return null;
    let u;
    try { u = new URL(url, origin); } catch (e) { return null; }
    try { if (u.origin !== new URL(origin).origin) return null; } catch (e) { return null; }
    if (/^\/uploads\/[^/]+$/.test(u.pathname)) return u.pathname;
    const n = u.searchParams.get('file');
    if (/^\/api\/files\/file\/\d+$/.test(u.pathname)) return u.pathname + (n && /^\d{1,6}$/.test(n) && n !== '0' ? '?file=' + n : '');
    return null;
  }

  /**
   * What an open card shows for `item`'s picture (the desktop's tiles.rs show_of, in its order).
   * `f`: {me, from (the message's sender), hides(key) (the protected setup leaves out pictures
   * from them), on (the setup is on), line (its line for a hidden picture), friend(key),
   * allowed (Show the picture was pressed for it), labels (the server's [{id, label}]), origin}.
   * Answers {show: 'none'} | {show: 'hidden', line} | {show: 'ask', line} | {show: 'load', path, members}.
   */
  function pictureOf(item, f) {
    const cover = item && item.cover && typeof item.cover.url === 'string' && item.cover.url ? item.cover : null;
    const pic = cover ? { url: cover.url, size: num(cover.size_bytes) } : item && item.kind === 'picture' ? { url: item.url, size: num(item.size_bytes) } : null;
    if (!pic) return { show: 'none' };
    const key = String((item && item.uploader_key) || '').toLowerCase();
    const own = !!key && key === String(f.me || '').toLowerCase();
    if (f.hides(key) || (f.from && f.hides(f.from))) return { show: 'hidden', line: f.line || '' };
    const labels = Array.isArray(item.labels) ? item.labels : [];
    if (labels.length && f.on && !own) return { show: 'hidden', line: WORDS.labelHidden };
    if (labels.length && !f.allowed) {
      const known = Array.isArray(f.labels) ? f.labels : [];
      return { show: 'ask', line: labelled(labels.map((id) => { const l = known.find((x) => x && x.id === id); return (l && l.label) || id; })) };
    }
    if (!own && !f.allowed && !f.friend(key)) return { show: 'ask', line: picturesFrom(byName(item)) };
    if (pic.size > MAX_PICTURE_BYTES) return { show: 'none' };
    const path = ownPath(pic.url, f.origin);
    if (!path) return { show: 'none' };
    return { show: 'load', path, members: path.startsWith('/api/files/file/') };
  }

  /** The card's key for item `id` in the message sent by `from` at `ts` (the desktop's key_of). */
  function keyOf(from, ts, id) {
    return String(from || '') + ':' + String(ts == null ? '' : ts) + ':' + id;
  }

  /** What a refusal says (files-app.js refusalOf): the relay's words, or its status. */
  async function refusalWords(res) {
    let text = '';
    try { text = String(await res.text()).trim(); } catch (e) { text = ''; }
    let json = null;
    if (text.startsWith('{')) { try { json = JSON.parse(text); } catch (e) { json = null; } }
    if (json && typeof json.error === 'string' && json.error) return json.error;
    if (!text || text.startsWith('<') || json) return 'The server answered ' + res.status + '.';
    return text;
  }

  // ── The cards ──

  /**
   * The cards of one page. `env`:
   *   document, fetch(url, init), origin (this site's) or origin(),
   *   myKey() (the person's Dilithium key hex, '' without an identity here),
   *   sign(purpose) -> Promise<{key, timestamp, sig}|null> (a signed read, "purpose\ntimestamp"),
   *   token() (this session's upload token, '' until signed in), URL (createObjectURL),
   *   badge(slot, name) (draws the type badge of a file named `name` into `slot`: app.js
   *   fileBadgeFill, HosFileRisk.badgeHtml),
   *   hidesPicturesFrom(key), protectedOn(), hiddenLine() (the protected setup's rule, and its line),
   *   isFriend(key) (pictures from them show without a click),
   *   openItem(id, item, title) (brings up the item's full window, named `title`; see itemWindowOpener),
   *   keepBottom(change) (runs `change`, which makes cards taller, keeping the message list at
   *   its bottom when it was there: chat-files-card.js keepAtBottom; optional).
   * Returns {attach(ids, msg) -> element, cache, open, allowed, render(id), prune(), kinds()}.
   */
  function createFilesCards(env) {
    const doc = env.document;
    const origin = () => (typeof env.origin === 'function' ? env.origin() : env.origin) || '';
    const cache = new Map();   // item id -> {state: 'loading'|'ready'|'missing'|'failed', item, labels, why}
    const open = new Set();    // the keys of the cards opened out (keyOf), for the page's life
    const allowed = new Set(); // the items whose picture Show the picture let through
    const mounted = new Map(); // item id -> Set of cards drawn for it ({el, id, key, from})
    const pictures = new Map(); // a members-only picture's path -> {state: 'loading'|'ready'|'error', url, why}
    let kinds = null;           // the kinds file's [{id, one}] once it is in ([] when it could not be read)
    let kindsAsked = null;      // its request, made once for the page and only when a card needs it
    let pruneAsked = false;     // a prune is queued for the end of this turn
    const keep = (change) => (typeof env.keepBottom === 'function' ? env.keepBottom(change) : change());

    /**
     * Let go of every card no longer in the document (2026-10-18): its message left the page
     * (another conversation opened, the thread panel drawn again). render() prunes only the cards
     * of an item it draws again, which never happens to an item already in the cache, so a long
     * session kept every card ever drawn. Run once at the end of the turn cards were put in, when
     * the ones just made are in the document: their callers put them there at once.
     */
    function prune() {
      for (const [id, cards] of Array.from(mounted)) {
        for (const card of Array.from(cards)) if (card.el.isConnected === false) cards.delete(card);
        if (!cards.size) mounted.delete(id);
      }
    }
    function pruneSoon() {
      if (pruneAsked) return;
      pruneAsked = true;
      Promise.resolve().then(() => { pruneAsked = false; prune(); });
    }

    function el(tag, cls, text) {
      const e = doc.createElement(tag);
      if (cls) e.className = cls;
      if (text != null) e.textContent = text;
      return e;
    }
    function button(cls, text, title, onClick) {
      const b = el('button', cls, text);
      b.type = 'button';
      if (title) b.title = title;
      b.addEventListener('click', (e) => { if (e && typeof e.stopPropagation === 'function') e.stopPropagation(); onClick(); });
      return b;
    }
    function badge(name) {
      const slot = el('span', 'file-badge-slot files-card-badge');
      if (typeof env.badge === 'function') env.badge(slot, name);
      return slot;
    }

    /** The kinds file, asked for once (an item with no name is called by its kind's word). */
    function loadKinds() {
      if (!kindsAsked) {
        kindsAsked = Promise.resolve()
          .then(() => env.fetch(KINDS_URL))
          .then((res) => (res && res.ok ? res.json() : null))
          .then((data) => { kinds = parseKinds(data); }, () => { kinds = []; })
          .then(() => { if (kinds === null) kinds = []; });
      }
      return kindsAsked;
    }

    /** Ask the server for item `id`, once (signed when the person has an identity here). */
    async function fetchItem(id) {
      if (cache.has(id)) return;
      cache.set(id, { state: 'loading' });
      let entry;
      try {
        let res = null;
        const me = typeof env.myKey === 'function' ? env.myKey() : '';
        if (me && typeof env.sign === 'function') {
          const auth = await env.sign('media_list');
          if (auth) {
            res = await env.fetch('/api/files/list', {
              method: 'POST', headers: { 'Content-Type': 'application/json' },
              body: JSON.stringify(Object.assign({ id: Number(id), limit: 1 }, auth)),
            });
          }
        }
        if (!res) res = await env.fetch('/api/files?id=' + encodeURIComponent(String(id)) + '&limit=1');
        if (!res || !res.ok) {
          entry = { state: 'failed', why: res ? await refusalWords(res) : 'The server could not be reached.' };
        } else {
          const a = await res.json();
          const item = a && Array.isArray(a.items) ? a.items.find((i) => i && String(i.id) === String(id)) : null;
          entry = item ? { state: 'ready', item, labels: Array.isArray(a.labels) ? a.labels : [] } : { state: 'missing' };
          // Called by its kind: the card is drawn once the kinds file says that kind's word.
          if (item && namedByKind(item)) await loadKinds();
        }
      } catch (e) {
        entry = { state: 'failed', why: e && e.message ? e.message : String(e) };
      }
      cache.set(id, entry);
      render(id);
    }

    /**
     * Draw again every card of item `id` still on the page (its item came, or its picture). They
     * grow: a list that was at its bottom stays there.
     */
    function render(id) {
      const cards = mounted.get(id);
      if (!cards) return;
      keep(() => {
        for (const card of Array.from(cards)) {
          // A card whose message has left the page (another conversation opened) is forgotten.
          if (card.el.isConnected === true) card.shown = true;
          else if (card.el.isConnected === false && card.shown) { cards.delete(card); continue; }
          draw(card);
        }
      });
    }

    /** A members-only picture: fetched with the upload token in a header, shown through an object URL. */
    function memberPicture(path, id) {
      let p = pictures.get(path);
      if (p) return p;
      const t = typeof env.token === 'function' ? env.token() : '';
      if (!t) return { state: 'waiting' };
      p = { state: 'loading', url: '', why: '' };
      pictures.set(path, p);
      Promise.resolve()
        .then(() => env.fetch(path, { headers: { 'X-Upload-Token': t } }))
        .then(async (res) => {
          if (!res || !res.ok) throw new Error(res ? await refusalWords(res) : 'The server could not be reached.');
          const blob = await res.blob();
          const U = env.URL || (typeof URL !== 'undefined' ? URL : null);
          p.url = U.createObjectURL(blob);
          p.state = 'ready';
        })
        .catch((e) => { p.state = 'error'; p.why = e && e.message ? e.message : String(e); })
        .then(() => render(id));
      return p;
    }

    /** The picture part of an open card, or null. */
    function picturePart(card, entry) {
      const item = entry.item;
      const facts = {
        me: typeof env.myKey === 'function' ? env.myKey() : '',
        from: card.from,
        hides: (key) => (typeof env.hidesPicturesFrom === 'function' ? !!env.hidesPicturesFrom(key) : false),
        on: typeof env.protectedOn === 'function' && !!env.protectedOn(),
        line: typeof env.hiddenLine === 'function' ? env.hiddenLine() : '',
        friend: (key) => typeof env.isFriend === 'function' && !!env.isFriend(key),
        allowed: allowed.has(item.id),
        labels: entry.labels,
        origin: origin(),
      };
      const d = pictureOf(item, facts);
      if (d.show === 'none') return null;
      if (d.show === 'hidden') return d.line ? el('p', 'files-card-note files-card-hidden', d.line) : null;
      if (d.show === 'ask') {
        const row = el('div', 'files-card-ask');
        row.appendChild(el('span', 'files-card-note', d.line));
        row.appendChild(button('files-card-btn files-card-show', WORDS.showPicture, '', () => { allowed.add(item.id); render(item.id); }));
        return row;
      }
      let src = d.path;
      if (d.members) {
        const p = memberPicture(d.path, item.id);
        if (p.state === 'waiting') return el('p', 'files-card-note', WORDS.waitingToken);
        if (p.state === 'error') return el('p', 'files-card-note files-card-failed', WORDS.fileFailed + p.why);
        if (p.state !== 'ready') return el('p', 'files-card-note', WORDS.loadingPicture);
        src = p.url;
      }
      const pic = button('files-card-picture', null, WORDS.openTip, () => openItem(item));
      const img = el('img', 'files-card-img');
      img.src = src;
      img.alt = displayTitle(item, kinds);
      pic.appendChild(img);
      return pic;
    }

    function openItem(item) {
      if (typeof env.openItem === 'function') env.openItem(item.id, item, displayTitle(item, kinds));
    }

    /** Draw one card from what is known of its item. */
    function draw(card) {
      const entry = cache.get(card.id) || { state: 'loading' };
      const box = card.el;
      box.replaceChildren();
      box.dataset.mediaItem = String(card.id);
      if (entry.state !== 'ready') {
        box.className = 'files-card files-card-wait';
        const words = entry.state === 'missing' ? WORDS.notThere : entry.state === 'failed' ? WORDS.failed + entry.why : WORDS.loading;
        box.appendChild(el('div', 'files-card-note' + (entry.state === 'failed' ? ' files-card-failed' : ''), words));
        return;
      }
      const item = entry.item;
      const isOpen = open.has(card.key);
      box.className = 'files-card' + (isOpen ? ' open' : '');
      // The line it starts as: the triangle, its type, its title, how many files and how much, Open.
      const line = el('div', 'files-card-line');
      const toggle = button('files-card-toggle', null, isOpen ? WORDS.lessTip : WORDS.moreTip, () => {
        if (open.has(card.key)) open.delete(card.key); else open.add(card.key);
        draw(card);
        // The card was drawn afresh: the keyboard stays on its triangle.
        const again = typeof card.el.querySelector === 'function' ? card.el.querySelector('.files-card-toggle') : null;
        if (again && typeof again.focus === 'function') again.focus();
      });
      if (typeof toggle.setAttribute === 'function') {
        toggle.setAttribute('aria-expanded', isOpen ? 'true' : 'false');
        toggle.setAttribute('aria-label', isOpen ? WORDS.lessTip : WORDS.moreTip);
      }
      toggle.appendChild(el('span', 'files-card-triangle'));
      line.appendChild(toggle);
      // Its own file's type, judged by the name it is shown by (plainName).
      line.appendChild(badge(shownName(item, null, kinds)));
      line.appendChild(button('files-card-title', displayTitle(item, kinds), WORDS.openTip, () => openItem(item)));
      line.appendChild(el('span', 'files-card-count', countLine(item)));
      line.appendChild(button('files-card-btn files-card-open', WORDS.open, WORDS.openTip, () => openItem(item)));
      box.appendChild(line);
      if (!isOpen) return;
      // Opened out: its picture, who put it there, its description, and every file it holds.
      const more = el('div', 'files-card-more');
      const pic = picturePart(card, entry);
      if (pic) more.appendChild(pic);
      more.appendChild(el('div', 'files-card-by', byLine(item)));
      const desc = trimmed(item.description);
      if (desc) more.appendChild(el('div', 'files-card-desc', desc));
      const list = el('ul', 'files-card-files');
      for (const f of fileLines(item, kinds)) {
        const row = el('li', 'files-card-file');
        row.appendChild(badge(f.name));
        row.appendChild(el('span', 'files-card-file-name', f.line));
        list.appendChild(row);
      }
      more.appendChild(list);
      box.appendChild(more);
    }

    /**
     * The cards for items `ids` in a message (`msg`: {from, timestamp}), in one element to put under
     * its words. Each item not known yet is asked for (once for the page).
     */
    function attach(ids, msg) {
      const m = msg || {};
      const wrap = el('div', 'files-cards');
      for (const id of ids || []) {
        const card = { id, key: keyOf(m.from, m.timestamp, id), from: m.from || '', el: el('div', 'files-card'), shown: false };
        if (!mounted.has(id)) mounted.set(id, new Set());
        mounted.get(id).add(card);
        wrap.appendChild(card.el);
        draw(card);
        if (!cache.has(id)) fetchItem(id);
      }
      pruneSoon();
      return wrap;
    }

    return { attach, render, cache, open, allowed, mounted, pictures, fetchItem, prune, kinds: () => kinds };
  }

  /**
   * Brings up item windows over the page (/shared/upload-window.js, `UW`): the Files page's own item
   * page in a frame, /files/<id>?window=item. One window shown at a time; one with an Edit's file
   * still uploading is kept (hidden, it carries on) and another item gets a window of its own.
   * Opening an item whose window is kept that way brings THAT window back (never a second frame on
   * the same item), and upload-window.js tells the page in it to open its item again (its own
   * Close had closed it), with its Edit and the upload's progress when one is still going.
   * Each window signs for the page in it with the chat's own identity (`sign`, `myKey`; 2026-10-18,
   * upload-window.js): the item page has no key of its own when the identity is passphrase-wrapped.
   * And lends it the chat's upload token (`token`; 2026-10-18), which a members-only item's files
   * are fetched with: that page has no sign-in of its own then either.
   * `opts`: window, document, onStatus(text), sign(purpose), myKey(), token(). Returns open(id, item, title).
   */
  function itemWindowOpener(UW, opts) {
    const o = opts || {};
    const hosts = []; // every window made, kept for its listeners: one is reused before another is made
    return function openItemWindow(id, item, title) {
      const page = UW.itemPage(id);
      const label = title || (item && displayTitle(item)) || UW.WORDS.itemTitle;
      // The window already on this item (shown, or hidden while its upload goes), else one with
      // nothing going in it, else a new one.
      let host = hosts.find((h) => h.page() === page && h.element()) || hosts.find((h) => !h.isBusy()) || null;
      if (!host) {
        host = UW.createHost({ window: o.window, document: o.document, page, label, onStatus: o.onStatus, sign: o.sign, myKey: o.myKey, token: o.token });
        hosts.push(host);
      }
      // One shown at a time: any other is closed (hidden and kept while something goes in it).
      for (const other of hosts) if (other !== host && other.isOpen()) other.close();
      host.showPage(page, label);
      return host;
    };
  }

  const api = {
    WORDS, MAX_PICTURE_BYTES, links, itemLinks, withoutLinks, sizeWords, countLine, displayTitle, byName, byLine,
    filesOf, ownName, fileName, shownName, fileLines, ownPath, pictureOf, keyOf, createFilesCards, itemWindowOpener,
    parseKinds, kindWord, plainName, KINDS_URL, BOTTOM_SLACK_PX, atBottom,
  };
  if (typeof module === 'object' && module && module.exports) { module.exports = api; return; }
  root.HosFilesCard = api;

  // ── In the chat (the browser) ──
  // The chat's own globals (app.js, chat-dms.js, chat-protected.js, crypto.js), read only when a card
  // is drawn, so this file can load before them.
  let lastSigned = 0;
  const chatKey = () => (typeof myKey === 'string' ? myKey : '');
  /**
   * A signed read or write, as the Files page signs one ("media_list\n<timestamp>"), with the
   * chat's own identity, held unlocked in this page; never two with the same timestamp (the relay
   * refuses a repeat). The cards' item requests, and the item windows' pages (upload-window.js,
   * which signs only the Files page's purposes for them).
   */
  async function chatSign(purpose) {
    const key = chatKey();
    if (typeof pqSignChatMessage !== 'function' || !key) return null;
    const timestamp = Math.max(Date.now(), lastSigned + 1);
    lastSigned = timestamp;
    const sig = await pqSignChatMessage(purpose, timestamp);
    // Signed by the identity unlocked when it was asked: answered only if it is still the one.
    return sig && chatKey() === key ? { key, timestamp, sig } : null;
  }
  /**
   * The chat's upload token for this server (app.js updatePeerList keeps it, from this connection's
   * own sign-in to the site the chat is on), lent to the item windows' pages (2026-10-18,
   * upload-window.js): only while that connection is open and signed in, since a token ends with its
   * socket and the one kept from before a reconnect is no longer known to the server. '' otherwise.
   */
  function chatToken() {
    const t = typeof myUploadToken === 'string' ? myUploadToken : '';
    const signedIn = typeof identityConfirmed !== 'undefined' && identityConfirmed === true;
    const open = typeof ws !== 'undefined' && !!ws && ws.readyState === 1;
    return t && signedIn && open ? t : '';
  }
  const openItemWindow = root.HosUploadWindow
    ? itemWindowOpener(root.HosUploadWindow, {
      window: root, document: root.document,
      onStatus: (text) => { if (typeof addNotice === 'function') addNotice(text, 'green', 10); },
      sign: chatSign, myKey: chatKey, token: chatToken,
    })
    : (id) => { try { root.open('/files/' + id, '_blank', 'noopener'); } catch (e) { /* no window */ } };
  const cards = createFilesCards({
    document: root.document,
    fetch: (u, i) => root.fetch(u, i),
    origin: () => (root.location && root.location.origin) || '',
    myKey: chatKey,
    sign: chatSign,
    token: () => (typeof myUploadToken === 'string' ? myUploadToken : ''),
    URL: root.URL,
    badge: (slot, name) => { if (typeof fileBadgeFill === 'function') fileBadgeFill(slot, name, null); },
    // The protected setup (chat-protected.js): could it not be read, a picture is left out.
    hidesPicturesFrom: (key) => (typeof protectedHidesPicturesFrom === 'function' ? protectedHidesPicturesFrom(key) : typeof protectedIsOn === 'function' && protectedIsOn()),
    protectedOn: () => (typeof protectedIsOn === 'function' ? protectedIsOn() : false),
    hiddenLine: () => (typeof protectedPreset !== 'undefined' && protectedPreset ? protectedPreset.picture_hidden_line || '' : ''),
    // A friend for pictures: mine, or a mutual follow holding a pass from me (chat-dms.js).
    isFriend: (key) => (typeof privateFileFromFriend === 'function' ? privateFileFromFriend(key) : false),
    openItem: (id, item, title) => openItemWindow(id, item, title),
    keepBottom: (change) => keepAtBottom(change),
  });
  root.hosFilesCards = cards;

  // The scrolled lists cards are drawn in: the messages, and the thread panel's (2026-10-18).
  const CARD_LISTS = ['messages', 'thread-panel-messages'];
  /**
   * Run `change` (a card going in, or filling in once its item comes), keeping each list cards are
   * drawn in at its bottom when it was there (2026-10-17, review): a card is put in after
   * appendMessage has scrolled, and grows again when its item arrives, which left a new message's
   * card below the fold. Someone who has scrolled up is left where they are.
   */
  function keepAtBottom(change) {
    const d = root.document;
    const lists = d && typeof d.getElementById === 'function' ? CARD_LISTS.map((id) => d.getElementById(id)).filter(Boolean) : [];
    const was = lists.filter((list) => atBottom(list));
    const out = change();
    for (const list of was) list.scrollTop = list.scrollHeight;
    return out;
  }

  // A picture or file link in the words, which the protected setup takes out of a non-friend's
  // message (app.js formatBody, its last step): the desktop then draws no card either (chat.rs `withheld`).
  const PICTURE_LINK = /(?:https?:\/\/[^\s<]+|\/uploads\/[^\s<]+)\.(?:png|jpe?g|gif|webp)/i;
  function withheld(text, fromKey) {
    if (!fromKey || typeof protectedHidesPicturesFrom !== 'function' || !protectedHidesPicturesFrom(fromKey)) return false;
    const files = typeof FILE_LINK_RE !== 'undefined' ? String(text).match(FILE_LINK_RE) : null;
    return !!files || PICTURE_LINK.test(String(text));
  }

  /**
   * For a message's text from `fromKey`: the words to draw ({words}) and the Media items its cards
   * show ({ids}); the words are the text itself when there are none.
   */
  root.filesCardsPlan = function filesCardsPlan(text, fromKey) {
    const here = (root.location && root.location.origin) || '';
    const ids = withheld(text, fromKey) ? [] : itemLinks(text, here);
    return { words: ids.length ? withoutLinks(text, ids, here) : String(text == null ? '' : text), ids };
  };

  // The cards each message row holds, so a row drawn again (an edit) puts its new ones in their place.
  const boxes = new WeakMap();
  /**
   * Put the cards for `ids` under the words of message row `row` (by `fromKey` at `ts`), in place of
   * any it had: a row of the message list (its words `.body`) or of the thread panel (its words
   * `.thread-msg-body`, chat-messages.js putThreadMessage, 2026-10-18).
   */
  root.filesCardsPut = function filesCardsPut(row, ids, fromKey, ts) {
    if (!row) return null;
    const old = boxes.get(row);
    if (old && typeof old.remove === 'function') old.remove();
    boxes.delete(row);
    if (!ids || !ids.length) return null;
    // A list at its bottom (appendMessage has just scrolled it there) stays there with the card in.
    return keepAtBottom(() => {
      const box = cards.attach(ids, { from: fromKey, timestamp: ts });
      const body = typeof row.querySelector === 'function' ? row.querySelector('.body') || row.querySelector('.thread-msg-body') : null;
      if (body && typeof body.after === 'function') body.after(box);
      else row.appendChild(box);
      boxes.set(row, box);
      return box;
    });
  };
})(typeof window !== 'undefined' ? window : globalThis);
