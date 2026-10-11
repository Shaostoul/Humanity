// ── media-app.js ──────────────────────────────────────────────────────────
// The Media page on the website (2026-10-11; docs/design/media-showcase.md,
// "Increment 2, exact", then round 2 against the relay's follow-ups: "Increment
// 2 follow-ups, exact", its As built note, and "Resumable uploads, exact"). The
// desktop app's Media page is the original (src/gui/pages/media.rs); this page
// mirrors its layout and words.
//
// What it shows: a strip of featured items (every featured item, asked for with
// `featured=1`), filters (category, kind, person, My picks, a search over
// titles), a grid of tiles newest first with More, a viewer, and the person's
// own uploads waiting to be published. What it does: Pick and Unpick, Remove
// (with the words naming what happens to the file), Feature and Unfeature for
// admins, Keep on the server and Stop keeping for admins and moderators, Report,
// Add to Media (the file picker, a short form, then a resumable upload with a
// progress bar, Pause and Resume), Put on Media for a Shared Files row (public
// only), and Suggest a category. The relay half is src/relay/media.rs and
// src/relay/resumable.rs.
//
// RULES THIS FILE KEEPS
// - Every element is built with createElement and textContent: nothing the
//   server sends is ever read as HTML.
// - A picture or video loads only from this server's own addresses: its
//   /uploads/ folder and the members' file route /api/media/file/{id}
//   (`ownFilePath`). Anything else is not loaded and says so.
// - A members-only PICTURE is fetched from /api/media/file/{id} with the
//   session's upload token in the X-Upload-Token HEADER (the relay's W3b) and
//   shown through an address of the page's own (URL.createObjectURL), so the
//   token is never in an <img src>, a Download link, a browser's downloads
//   list or a server's log. Each fetch can be stopped (an AbortController):
//   it is stopped, and its object URL revoked, once its item is no longer
//   shown (`sweepBlobs`); a list loaded again keeps the pictures it still
//   shows (round 3, review 5).
// - A members-only VIDEO, and every members-only Download, go through a
//   ticket (the relay's R3-8): POST /api/media/ticket with the token in the
//   header answers an address that opens that one file for ten minutes, so the
//   browser streams the video (range requests) and saves a download itself,
//   never holding the whole file in the page, and no token is in any address.
//   A video that outlives its ticket gets a new one where it stopped; a new
//   one refused (a ban, the item gone, too many open) puts the relay's words
//   where the video was (round 4, W4-2), and opening it again asks afresh.
// - The viewer is drawn again only when what it shows changes, so a playing
//   video keeps playing through a sign-in, a reconnect or a list reload.
// - A labelled item (spoiler, flashing lights, graphic) shows its label
//   instead of its picture until clicked, on its tile and in the viewer
//   (/media?id=<n> included): its file is not even requested before then.
// - Uploads in progress are a list of their own (each with its bar, Pause,
//   Resume and Cancel), each published by its own form, so opening another
//   form never leaves one behind (round 3, review 2). Uploads the server still
//   holds part of (`uploads_in_progress`, R3-7) are listed with Cancel. A
//   cancel is asked again while the server is busy, and the server deletes a
//   file the finish had stored with it, so nothing is removed by hand; the
//   page says what the server answered (round 4, W4-3).
// - Every write is signed (Dilithium over "purpose\ntimestamp", getPqSignedAuth
//   in /shared/pq-relay-auth.js, which never signs two requests with the same
//   timestamp) with a purpose that names its target: media_publish:<file name>,
//   media_remove:<item id or file name>, media_pick:<item id>,
//   media_feature:<item id>, media_keep:<item id>:<on|off>. A write on an item
//   also names the item's file (`filename`, the relay's W5), so a tile left on
//   a page that is out of date never acts on another item: the relay answers
//   409 and the page loads the list again.
// - The kinds are data: /data/media/kinds.json, the same file the desktop app
//   reads (tests/page_parity_lint.rs holds both to it). The labels come from
//   the server's answers.
//
// A classic script in the browser, a CommonJS module under Node, where
// scripts/tests/media-web.test.js and media-web-round2.test.js run the page
// against a stand-in document that has only what this file built in it.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  const MIB = 1024 * 1024;
  const PAGE_SIZE = 48;
  // The featured strip asks for every featured item, up to this many.
  const FEATURED_LIMIT = 100;
  // The kinds of item (data, read by both apps: the desktop's src/gui/pages/media/kinds.rs).
  const KINDS_URL = '/data/media/kinds.json';
  // Members-only pictures fetched at once, so a page of tiles does not ask for all at the same time.
  const BLOB_FETCHES_AT_ONCE = 4;
  // A ticket is not used for a new request this close to its end (the relay's are ten minutes).
  const TICKET_MARGIN_MS = 30000;
  // A playing video whose ticket fails gets a new one, but not for a ticket younger than this
  // (a fresh ticket failing is the file's own problem, not its age).
  const TICKET_RENEW_AFTER_MS = 60000;
  // How long a report waits for the server's answer before saying it did not come.
  const ANSWER_WAIT_MS = 15000;

  // ── Words (the design's own, shared with the desktop page) ──
  const MEDIA_WORDS = Object.freeze({
    lede: 'Pictures, videos, 3D models and files people on this server chose to share.',
    membersLine: "Sign in to this server to see its members' media.",
    membersOnlyServer: "This server's Media page is for its members.",
    alreadyPublic: 'This file is already public where it was shared.',
    serverMembersOnly: "This server's Media page is for its members, so this goes on it as members only.",
    removeKeeps: 'Remove from Media. The file stays where it came from.',
    removeDeletes: 'Remove from Media and delete the file.',
    removeEither: 'Remove from Media. A file shared in a chat or in Shared Files stays there; a file uploaded on this page is deleted.',
    removedKept: 'Removed from Media. The file stays where it came from.',
    removedDeleted: 'Removed from Media, and the file was deleted.',
    removed: 'Removed from Media.',
    removeWaiting: 'Remove this upload and delete the file.',
    removedWaiting: 'Removed, and the file was deleted.',
    empty: 'Nothing is on the Media page yet.',
    emptyFiltered: 'Nothing here matches these filters.',
    notThisServer: 'Not shown: this file is not on this server.',
    waitingToken: 'This members-only file shows once this page is signed in to the server.',
    loadingFile: 'Loading...',
    fileFailed: 'This file could not be loaded: ',
    noIdentity: 'Not signed in on this page. Open Chat in this browser first (it keeps your identity here), then press Try again. A passphrase-protected identity cannot be read by this page yet.',
    signingIn: 'Signing in to this server...',
    cannotSign: 'This page could not sign the request with your identity. Open Chat in this browser first, then try again.',
    notSignedInUpload: 'This page is not signed in to this server yet, so nothing was uploaded. Try again in a moment.',
    cannotUpload: 'Your role on this server cannot upload files, so Add to Media is not offered. An admin can change that in Server Settings > Roles.',
    published: 'It is on the Media page now.',
    suggestSent: 'Suggestion sent. You are told here, and in Chat, when an admin decides.',
    clickToShow: 'Click to show',
    loading: 'Loading...',
    keep: 'Keep on the server',
    stopKeeping: 'Stop keeping',
    keptBadge: 'Kept by the server',
    keptNow: "Kept on the server: it no longer counts against its uploader's share.",
    keptStopped: "No longer kept by the server: it counts against its uploader's share again.",
    reloaded: 'That was not the Media item you were looking at: the page was out of date, so it has been loaded again. Nothing was changed.',
    noFileName: "The server's answer does not name this item's file, so this page will not act on it. Load the page again later.",
    notFound: 'That Media item is not there, or this page cannot show it to you.',
    waitingTitle: 'Your uploads waiting to be published',
    waitingHint: 'Uploaded, but not on the Media page yet. Publish one, or remove it: an upload nobody publishes is deleted a day after it was uploaded.',
    uploadStarting: 'Starting the upload...',
    uploadPausing: 'Pausing once this piece has arrived...',
    uploadPaused: 'Paused. Resume carries on from here.',
    uploadWaitingConnection: 'Waiting for this page to be signed in to the server again...',
    uploadResumed: 'This file was part way up already, so it carries on from where it stopped.',
    uploadFinishing: 'All sent. The server is storing it...',
    uploadPublishing: 'Uploaded. Putting it on the Media page...',
    uploadWaits: ' The file is uploaded and waits a day for you to try again; after that it is deleted.',
    uploadCancelling: 'Cancelling...',
    uploadAllowanceThen: 'It carries on by itself then.',
    uploadServerErrorTail: ' The server gave this answer ten times running, so the upload waits for you: Retry carries on from where it stopped.',
    alreadyUploading: ' is uploading already: its progress is below.',
    unfinishedTitle: 'Unfinished uploads on the server',
    unfinishedHint: 'These stopped part way. Add to Media with the same file carries one on from where it stopped; Cancel deletes what arrived. Each is deleted a day after its last piece arrived.',
    unfinishedCancel: 'Cancel this upload and delete what arrived.',
    unfinishedCancelled: 'Cancelled, and what arrived was deleted.',
    notSignedInCancel: 'This page is not signed in to this server yet, so nothing was cancelled. Try again in a moment.',
    cancelledStored: 'Cancelled. The file had already been stored, so it was deleted too.',
    cancelledPublished: 'Cancelled, but the file was already on the Media page, so it stays there. Remove on its tile takes it off.',
    cancelUnconfirmed: 'The cancel could not reach the server, so what arrived may still be there. It is listed on this page with Cancel or Remove once the server answers again, and the server deletes it by itself within a day.',
    cancelNotFinished: 'The server could not finish the cancel, so what arrived may still be there. It is listed on this page with Cancel or Remove, to try again. The server said: ',
    cancellingUnfinished: 'Cancelling...',
    ticketOdd: 'The server did not give this page an address for the file.',
    reportLeadTail: ' is not told who reported them.',
    reportWhat: 'The report names this item. The server keeps a note of it (its title, kind, labels and who can see it), never the file.',
    reportNotSignedIn: 'This page is not signed in to this server yet, so the report was not sent. Try again in a moment.',
    reportNoAnswer: 'The server did not answer, so it is not known whether the report arrived. Try again in a moment.',
    reportReasonsFailed: 'The list of reasons could not be loaded from this server, so a report cannot be sent yet. Close this and try again in a moment.',
  });

  function num(x) {
    const n = Number(x);
    return Number.isFinite(n) && n > 0 ? n : 0;
  }

  // ── The kinds (data/media/kinds.json) ──

  /** The kinds file read into [{id, label, one, mark}]; an empty list when it is not one. */
  function parseKinds(data) {
    const list = data && Array.isArray(data.kinds) ? data.kinds : [];
    return list
      .filter((k) => k && typeof k.id === 'string' && k.id.trim() && typeof k.label === 'string' && k.label.trim())
      .map((k) => ({
        id: k.id,
        label: k.label,
        one: typeof k.one === 'string' && k.one ? k.one : k.label,
        mark: typeof k.mark === 'string' && k.mark ? k.mark : 'page',
      }));
  }

  /** A kind in words, from the kinds file: as one, as many, and its mark. A kind it does not list reads as the server named it. */
  function kindWords(kind, kinds) {
    const k = (Array.isArray(kinds) ? kinds : []).find((x) => x.id === kind);
    if (k) return { one: k.one, many: k.label, mark: k.mark };
    return { one: String(kind || ''), many: String(kind || ''), mark: 'page' };
  }

  function oneDecimal(x) {
    return x.toFixed(1).replace(/\.0$/, '');
  }

  /** Bytes as people read them: "412 MB", "12.4 GB", "50 GB", "900 KB". */
  function sizeWords(bytes) {
    const b = Math.max(0, Number(bytes) || 0);
    if (b >= 1024 * MIB) return oneDecimal(b / (1024 * MIB)) + ' GB';
    if (b >= MIB) return (b / MIB >= 100 ? String(Math.round(b / MIB)) : oneDecimal(b / MIB)) + ' MB';
    if (b >= 1024) return Math.round(b / 1024) + ' KB';
    return b + (b === 1 ? ' byte' : ' bytes');
  }

  /** "You have 412 MB of your 512 MB left". */
  function shareLeftWords(usedBytes, shareBytes) {
    const left = Math.max(0, shareBytes - usedBytes);
    return 'You have ' + sizeWords(left) + ' of your ' + sizeWords(shareBytes) + ' left';
  }

  /** The stop when a file is bigger than what is left of the person's share, else null. */
  function shareStop(file, usedBytes, shareBytes) {
    if (!file || !Number.isFinite(shareBytes) || shareBytes <= 0 || !Number.isFinite(usedBytes)) return null;
    const left = Math.max(0, shareBytes - usedBytes);
    if (file.size <= left) return null;
    return '"' + file.name + '" is ' + sizeWords(file.size) + ', more than the ' + sizeWords(left) +
      ' you have left of your ' + sizeWords(shareBytes) + ' share, so it was not uploaded. Remove one of your items, or ask an admin to raise each person\'s share.';
  }

  /** The stop when a file is over the role's largest Media file on this server (0: none), else null. */
  function limitStop(file, limitBytes) {
    if (!file || !Number.isFinite(limitBytes) || limitBytes <= 0 || file.size <= limitBytes) return null;
    return '"' + file.name + '" is ' + sizeWords(file.size) + ', over your role\'s ' + sizeWords(limitBytes) +
      ' limit for one Media file on this server, so it was not uploaded. An admin can raise it in Server Settings > Roles.';
  }

  /** The stop when a file is bigger than what is left of the whole Media space, else null. */
  function budgetStop(file, leftBytes) {
    if (!file || !Number.isFinite(leftBytes) || leftBytes < 0 || file.size <= leftBytes) return null;
    return '"' + file.name + '" is ' + sizeWords(file.size) + ', more than the ' + sizeWords(leftBytes) +
      ' left in this server\'s Media space, so it was not uploaded. An admin can make room, or raise the Media budget.';
  }

  /** The admins' meter: "12.4 GB of 50 GB used". */
  function meterWords(usedBytes, budgetBytes) {
    return sizeWords(usedBytes) + ' of ' + sizeWords(budgetBytes) + ' used';
  }

  /** The meter's own line for what the server keeps (inside "used", in nobody's share). */
  function keptMeterWords(keptBytes) {
    return 'Kept by the server: ' + sizeWords(keptBytes) + ' of that, counted in the budget and in nobody\'s share';
  }

  /**
   * The confirmation for Remove, naming what happens to the file: each item
   * says whether Remove gives its file back to chat or Shared Files
   * (`keeps_file`, the relay's A1) or deletes it.
   */
  function removeWords(item) {
    if (item && item.keeps_file === true) return MEDIA_WORDS.removeKeeps;
    if (item && item.keeps_file === false) return MEDIA_WORDS.removeDeletes;
    return MEDIA_WORDS.removeEither;
  }

  /** What the relay's answer to a removal says happened. */
  function removedWords(answer) {
    if (answer && answer.file_kept === true) return MEDIA_WORDS.removedKept;
    if (answer && answer.file_removed === true) return MEDIA_WORDS.removedDeleted;
    return MEDIA_WORDS.removed;
  }

  /** When an upload allowance resets, in this device's own time, or ''. */
  function resetsWords(ms) {
    const n = Number(ms);
    if (!Number.isFinite(n) || n <= 0) return '';
    let when = '';
    try { when = new Date(n).toLocaleString(undefined, { weekday: 'short', hour: 'numeric', minute: '2-digit' }); } catch (e) { when = ''; }
    if (!when) { try { when = new Date(n).toISOString().slice(0, 16).replace('T', ' ') + ' UTC'; } catch (e) { when = ''; } }
    return when ? 'That is ' + when + ' here.' : '';
  }

  // ── Addresses ──

  /**
   * The path of a file on this server's own addresses, or null: /uploads/<name>
   * or /api/media/file/<id>, on `origin`. Nothing from anywhere else is ever
   * loaded on this page.
   */
  function ownFilePath(url, origin) {
    if (typeof url !== 'string' || !url) return null;
    let u;
    try { u = new URL(url, origin); } catch (e) { return null; }
    if (u.origin !== new URL(origin).origin) return null;
    if (/^\/uploads\/[^/]+$/.test(u.pathname)) return u.pathname;
    if (/^\/api\/media\/file\/\d+$/.test(u.pathname)) return u.pathname;
    return null;
  }

  /**
   * Where the page gets an item's file: `{public: path}` (its /uploads/ address,
   * used as it is), `{members: path}` (the members' file route, fetched with the
   * token in a header and shown through an object URL), or `{why}` when it is not
   * on this server.
   */
  function fileRoute(item, origin) {
    const path = ownFilePath(item && item.url, origin);
    if (!path) return { why: MEDIA_WORDS.notThisServer };
    if (path.startsWith('/api/media/file/')) return { members: path };
    return { public: path };
  }

  /** The stored file name from an upload's address ("/uploads/x.png" is "x.png"). */
  function storedName(url) {
    const s = String(url || '');
    return s.startsWith('/uploads/') ? s.slice('/uploads/'.length) : s;
  }

  /**
   * The item's stored file name, which every write on an item names (the
   * relay's W5): the item's `filename` when the server's answer carries one,
   * else the name in its /uploads/ address. '' when neither says it (a
   * members-only item's address is its number, not its file).
   */
  function itemFileName(item) {
    const bare = (s) => (/^[^/\\]+$/.test(s) && s !== '.' && s !== '..' ? s : '');
    if (item && typeof item.filename === 'string' && item.filename) return bare(storedName(item.filename.trim()));
    const url = String((item && item.url) || '');
    return url.startsWith('/uploads/') ? bare(storedName(url)) : '';
  }

  /** What an item is called: its title, else its file's name, else its kind. */
  function itemName(item, kinds) {
    if (item && typeof item.title === 'string' && item.title.trim()) return item.title.trim();
    if (item && typeof item.original_name === 'string' && item.original_name.trim()) return item.original_name.trim();
    const url = String((item && item.url) || '');
    if (url.startsWith('/uploads/')) return storedName(url);
    return kindWords(item && item.kind, kinds).one || 'This item';
  }

  /** The name a downloaded file is saved under: the uploader's own name for it, else its stored name. */
  function downloadName(item, kinds) {
    if (item && typeof item.original_name === 'string' && item.original_name.trim()) return item.original_name.trim();
    return itemFileName(item) || itemName(item, kinds);
  }

  /** An item's labels as the server describes them, in its order; an unknown id as itself. */
  function labelsOf(item, labels) {
    const ids = Array.isArray(item && item.labels) ? item.labels : [];
    const known = Array.isArray(labels) ? labels : [];
    return ids.map((id) => known.find((l) => l && l.id === id) || { id, label: id, desc: '' });
  }

  function isAdminRole(role) {
    return role === 'admin' || role === 'owner';
  }

  /** Admins and moderators: the people who may keep an item on the server (media.rs keep_core). */
  function isStaffRole(role) {
    return isAdminRole(role) || role === 'mod' || role === 'moderator';
  }

  // ── Requests ──

  /** The list's filters as the relay takes them, without the empty ones. */
  function listParams(filters, before, limit) {
    const f = filters || {};
    const p = {};
    if (f.category) p.category = Number(f.category);
    if (f.kind) p.kind = f.kind;
    if (f.by) p.by = f.by;
    if (f.q && f.q.trim()) p.q = f.q.trim();
    if (f.featured) p.featured = 1;
    if (f.id) p.id = Number(f.id);
    if (before != null) p.before = Number(before);
    p.limit = limit || PAGE_SIZE;
    return p;
  }

  function query(params) {
    const q = new URLSearchParams();
    for (const k of Object.keys(params)) q.set(k, String(params[k]));
    return q.toString();
  }

  /** The unsigned public list. */
  function publicListUrl(params) {
    return '/api/media?' + query(params);
  }

  const purposes = Object.freeze({
    list: () => 'media_list',
    mine: () => 'media_mine',
    publish: (filename) => 'media_publish:' + filename,
    remove: (target) => 'media_remove:' + target,
    pick: (id) => 'media_pick:' + id,
    feature: (id) => 'media_feature:' + id,
    keep: (id, on) => 'media_keep:' + id + ':' + (on ? 'on' : 'off'),
    suggest: () => 'media_suggest',
    suggestions: () => 'media_suggestions',
    decide: (id, accept) => 'media_suggestions:' + id + ':' + (accept ? 'accept' : 'decline'),
    categories: (hashHex) => 'media_categories:' + hashHex,
    usage: () => 'media_usage',
  });

  function jsonPost(body) {
    return { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: typeof body === 'string' ? body : JSON.stringify(body) };
  }

  /**
   * What a refusal says: `{words, json}`. The relay answers most refusals in
   * plain words and some (a resumable upload's) as JSON with `error`; something
   * in front of it (a proxy's HTML page) gets plain words naming the status.
   */
  async function refusalOf(res) {
    let text = '';
    try { text = String(await res.text()).trim(); } catch (e) { text = ''; }
    let json = null;
    if (text.startsWith('{')) { try { json = JSON.parse(text); } catch (e) { json = null; } }
    if (json && typeof json.error === 'string' && json.error) return { words: json.error, json };
    if (!text || text.startsWith('<') || json) return { words: 'The server answered ' + res.status + '.', json };
    return { words: text, json: null };
  }

  /** The relay's words for a refusal. */
  async function refusalWords(res) {
    return (await refusalOf(res)).words;
  }

  /** A JSON answer, or throws Error(the relay's words) with `status` on it. */
  async function answerOf(res) {
    if (!res || !res.ok) {
      const err = new Error(res ? await refusalWords(res) : 'The server could not be reached.');
      err.status = res ? res.status : 0;
      throw err;
    }
    return res.json();
  }

  // The resumable upload and the uploads remembered in this browser: /pages/media-upload.js.
  const U = typeof module === 'object' && module && module.exports ? require('./media-upload.js') : root.MediaUpload;
  const { RESUME_KEY, rememberUpload, findUpload, forgetUpload, createUploader, fingerprint, cancelSession } = U || {};

  // ── DOM ──

  function makeH(doc) {
    return function h(tag, props, kids) {
      const el = doc.createElement(tag);
      const p = props || {};
      for (const k of Object.keys(p)) {
        const v = p[k];
        if (v === undefined || v === null) continue;
        if (k === 'text') el.textContent = String(v);
        else if (k === 'on') { for (const ev of Object.keys(v)) el.addEventListener(ev, v[ev]); }
        else if (k === 'dataset') { for (const d of Object.keys(v)) el.dataset[d] = String(v[d]); }
        else if (k === 'attrs') { for (const a of Object.keys(v)) el.setAttribute(a, String(v[a])); }
        else el[k] = v;
      }
      for (const c of [].concat(kids == null ? [] : kids)) {
        if (c == null || c === false) continue;
        el.appendChild(typeof c === 'string' ? doc.createTextNode(c) : c);
      }
      return el;
    };
  }

  /**
   * The page. `env`:
   *   document, root (the element the page is built in), fetch, origin,
   *   myKey (the person's Dilithium3 key hex, '' without an identity here),
   *   sign(purpose) -> Promise<{key, timestamp, sig}|null>,
   *   signBytes(bytes) -> Promise<Uint8Array|null> and blake3(bytes) (a report's signature),
   *   session (a /shared/relay-session.js session, or null),
   *   confirm(words) -> Promise<boolean>,
   *   URL (createObjectURL, revokeObjectURL), storage (localStorage), sleep(ms), now(),
   *   AbortController and digest(bytes) (SHA-256), the browser's own when not given,
   *   save(url, name) (saves a file; optional: a hidden link is clicked otherwise),
   *   report (/shared/report.js's names), language (navigator.language),
   *   put ({filename, name} to open Put on Media for a Shared Files upload, or null),
   *   openId (an item's number to open, from /media?id=<n>, or null),
   *   mountAdmin(container) (the admins' Media section, /pages/media-admin.js), optional.
   */
  function createMediaPage(env) {
    const doc = env.document;
    const h = makeH(doc);
    const myKey = env.myKey || '';
    const session = env.session || null;
    const URLs = env.URL || (typeof URL !== 'undefined' ? URL : null);
    const R = env.report || null;
    const later = env.setTimeout || ((fn, ms) => setTimeout(fn, ms));
    const cancelLater = env.clearTimeout || ((t) => clearTimeout(t));
    const now = () => (typeof env.now === 'function' ? env.now() : Date.now());
    const Abort = env.AbortController || (typeof AbortController !== 'undefined' ? AbortController : null);

    const st = {
      items: [],
      featured: [],
      nextBefore: null,
      answered: false,          // a list answer has come back
      membersView: false,
      membersOnlyForYou: false,
      mediaVisibility: 'public',
      categories: [],
      labels: [],
      kinds: [],                // data/media/kinds.json
      seenKinds: [],            // the kinds the items carry, used when the kinds file could not be read
      people: new Map(),        // uploader key -> name, from the answers
      filters: { category: '', kind: '', by: '', q: '', picks: false },
      revealed: new Set(),      // labelled items the person clicked to show
      loading: false,
      listError: '',
      notice: '',
      noticeWarn: false,
      viewer: null,
      viewerKey: '',
      saveWanted: null,         // the item whose Download is asking for its ticket
      reloadKeep: null,         // the items shown before a list loaded afresh: their pictures stay until it answers
      form: null,               // the form not yet uploading (Add to Media, Put on Media, Publish an upload)
      uploads: [],              // forms whose file is uploading or publishing, each with its own `up`
      suggest: null,
      report: null,
      mine: null,               // the person's own use: POST /api/media/mine
      adminMounted: false,
      filtersKey: '',
      lastStatus: '',
    };
    let loadSeq = 0;
    let featuredSeq = 0;
    let mineSeq = 0;
    let uploadSeq = 0;

    // ── The page's fixed parts ──
    const els = {};
    const fileInput = h('input', { id: 'media-file-input', type: 'file', hidden: true, className: 'media-file-input' });
    fileInput.addEventListener('change', () => {
      const file = fileInput.files && fileInput.files[0];
      if (file) openAddForm(file);
      try { fileInput.value = ''; } catch (e) { /* some browsers refuse; harmless */ }
    });
    function build() {
      doc.addEventListener('keydown', (e) => {
        if (e.key === 'Escape' && st.report) { closeReport(); return; }
        if (e.key === 'Escape' && st.viewer) closeViewer();
      });
      els.status = h('div', { id: 'media-status', className: 'media-status' });
      els.notice = h('div', { id: 'media-notice', className: 'media-notice' });
      els.members = h('div', { id: 'media-members', className: 'media-members' });
      els.actions = h('div', { id: 'media-actions', className: 'media-actions' });
      els.form = h('div', { id: 'media-form', className: 'media-form-wrap' });
      els.uploads = h('div', { id: 'media-uploads', className: 'media-form-wrap' });
      els.waiting = h('div', { id: 'media-waiting', className: 'media-form-wrap' });
      els.suggest = h('div', { id: 'media-suggest', className: 'media-form-wrap' });
      els.featured = h('section', { id: 'media-featured', className: 'media-featured' });
      els.filters = h('div', { id: 'media-filters', className: 'media-filters' });
      els.grid = h('div', { id: 'media-grid', className: 'media-grid' });
      els.more = h('div', { id: 'media-more-wrap', className: 'media-more-wrap' });
      els.viewer = h('div', { id: 'media-viewer', className: 'media-viewer-wrap' });
      els.report = h('div', { id: 'media-report', className: 'media-viewer-wrap' });
      els.admin = h('div', { id: 'media-admin-wrap', className: 'media-admin-wrap' });
      env.root.replaceChildren(
        h('div', { className: 'media-head' }, [
          h('h1', { text: 'Media' }),
          h('p', { className: 'media-lede' }, [
            MEDIA_WORDS.lede + ' Files people share for download are in ',
            h('a', { href: '/shared-files', text: 'Shared Files' }),
            '.',
          ]),
        ]),
        els.status, els.notice, els.members, els.actions, els.form, els.uploads, els.waiting, els.suggest,
        els.featured, els.filters, els.grid, els.more, els.viewer, els.report, els.admin, fileInput
      );
    }

    // ── Who the person is here ──
    function sessionState() {
      return session ? session.state : null;
    }
    function token() {
      const s = sessionState();
      return s && s.status === 'bound' ? s.uploadToken || '' : '';
    }
    function role() {
      const s = sessionState();
      return s && s.status === 'bound' ? s.role || '' : '';
    }
    function isAdmin() {
      return isAdminRole(role());
    }
    function isStaff() {
      return isStaffRole(role());
    }
    function settings() {
      const s = sessionState();
      return (s && s.settings) || null;
    }
    function membersOnlyServer() {
      const s = settings();
      if (s && typeof s.media_visibility === 'string') return s.media_visibility !== 'public';
      return st.mediaVisibility !== 'public';
    }
    /** Signed in to this server: the socket is bound, so uploads and the settings work. */
    function bound() {
      const s = sessionState();
      return !!(s && s.status === 'bound');
    }
    /** The person's role as the server's role list describes it, or null until that list has come. */
    function roleDef() {
      const s = sessionState();
      if (!s || !Array.isArray(s.roles)) return null;
      return s.roles.find((r) => r && r.id === s.role) || null;
    }
    /**
     * Whether the person may upload at all: the relay refuses anyone with no
     * role ("only verified users can upload files") and any role whose upload
     * right is off (upload_limits.rs upload_role_check). Unknown (the role
     * list not here yet) counts as no, so nothing is offered that would fail.
     */
    function canUpload() {
      if (!bound() || !role()) return false;
      const def = roleDef();
      return !!(def && def.can_upload === true);
    }
    /** Known not to upload: signed in, the role list here, and the role cannot. */
    function knownCannotUpload() {
      if (!bound()) return false;
      if (!role()) return true;
      const def = roleDef();
      return !!(def && def.can_upload !== true);
    }

    // ── Status and notices ──
    function say(text, warn) {
      st.notice = text || '';
      st.noticeWarn = !!warn;
      renderNotice();
    }
    function renderNotice() {
      els.notice.replaceChildren();
      els.notice.className = 'media-notice' + (st.noticeWarn ? ' media-warn' : '');
      if (st.notice) els.notice.appendChild(h('p', { className: 'media-notice-text', text: st.notice }));
    }
    function renderStatus() {
      els.status.replaceChildren();
      let words = '';
      let retry = null;
      const s = sessionState();
      if (!myKey) {
        words = MEDIA_WORDS.noIdentity;
        retry = env.retryIdentity || null;
      } else if (s) {
        if (s.status === 'connecting' || s.status === 'idle') words = MEDIA_WORDS.signingIn;
        else if (s.status === 'refused') { words = 'The server did not accept this page\'s sign-in: ' + s.refusal; retry = () => session.retry(); }
        else if (s.status === 'closed') words = 'Lost the connection to the server. Trying again in ' + (s.retryInSeconds || 5) + ' seconds.';
        else if (s.status === 'no_identity') { words = MEDIA_WORDS.noIdentity; retry = env.retryIdentity || null; }
      }
      if (!words) return;
      const p = h('p', { className: 'media-status-text', text: words });
      if (retry) {
        p.appendChild(doc.createTextNode(' '));
        p.appendChild(h('button', { type: 'button', className: 'media-btn media-btn-small', id: 'media-retry', text: 'Try again', on: { click: retry } }));
      }
      els.status.appendChild(p);
    }
    function renderMembers() {
      els.members.replaceChildren();
      if (!st.answered || st.membersView) return;
      els.members.appendChild(h('p', { id: 'media-members-line', className: 'media-members-line', text: MEDIA_WORDS.membersLine }));
    }

    // ── Actions bar ──
    function renderActions() {
      els.actions.replaceChildren();
      if (!myKey) return;
      if (st.membersView && canUpload()) {
        els.actions.appendChild(h('button', {
          type: 'button', id: 'media-add-btn', className: 'media-btn media-btn-primary', text: 'Add to Media',
          on: { click: () => fileInput.click() },
        }));
      }
      if (st.membersView) {
        els.actions.appendChild(h('button', {
          type: 'button', id: 'media-suggest-btn', className: 'media-btn', text: 'Suggest a category',
          on: { click: openSuggest },
        }));
      }
      if (st.membersView && knownCannotUpload()) {
        els.actions.appendChild(h('p', { id: 'media-cannot-upload', className: 'media-hint media-actions-line', text: MEDIA_WORDS.cannotUpload }));
      }
    }

    // ── Filters ──
    function kindsOffered() {
      if (st.kinds.length) return st.kinds.map((k) => k.id);
      return st.seenKinds;
    }
    function renderFilters() {
      const peopleList = Array.from(st.people.entries()).sort((a, b) => String(a[1] || '').localeCompare(String(b[1] || '')));
      const key = JSON.stringify([st.categories.map((c) => [c.id, c.name]), kindsOffered(), st.kinds.map((k) => k.label), peopleList, !!myKey, st.filters]);
      if (key === st.filtersKey) return;
      st.filtersKey = key;
      const keepQ = els.search ? els.search.value : st.filters.q;
      const option = (value, text, chosen) => h('option', { value: String(value), text, selected: String(chosen) === String(value) });
      const category = h('select', { id: 'media-filter-category', className: 'media-select', attrs: { 'aria-label': 'Category' } },
        [option('', 'All categories', st.filters.category)].concat(st.categories.map((c) => option(c.id, c.name, st.filters.category))));
      category.addEventListener('change', () => setFilter('category', category.value));
      const kind = h('select', { id: 'media-filter-kind', className: 'media-select', attrs: { 'aria-label': 'Kind' } },
        [option('', 'All kinds', st.filters.kind)].concat(kindsOffered().map((k) => option(k, kindWords(k, st.kinds).many, st.filters.kind))));
      kind.addEventListener('change', () => setFilter('kind', kind.value));
      const person = h('select', { id: 'media-filter-person', className: 'media-select', attrs: { 'aria-label': 'Person' } },
        [option('', 'Everyone', st.filters.by)].concat(peopleList.map(([k, name]) => option(k, name || 'Someone without a name', st.filters.by))));
      person.addEventListener('change', () => setFilter('by', person.value));
      const kids = [category, kind, person];
      if (myKey) {
        const picks = h('input', { type: 'checkbox', id: 'media-filter-picks', checked: !!st.filters.picks });
        picks.addEventListener('change', () => setFilter('picks', !!picks.checked));
        kids.push(h('label', { className: 'media-check' }, [picks, ' My picks']));
      }
      els.search = h('input', { type: 'search', id: 'media-search', className: 'media-search', value: keepQ || '', placeholder: 'Search titles', attrs: { 'aria-label': 'Search titles' } });
      els.search.addEventListener('keydown', (e) => { if (e.key === 'Enter') setFilter('q', els.search.value); });
      kids.push(els.search);
      kids.push(h('button', { type: 'button', id: 'media-search-btn', className: 'media-btn', text: 'Search', on: { click: () => setFilter('q', els.search.value) } }));
      els.filters.replaceChildren(...kids);
    }
    function setFilter(name, value) {
      st.filters[name] = value;
      load(true);
    }

    // ── Members-only pictures: fetched with the token in a header, shown through object URLs ──
    const blobs = new Map();   // item id -> {path, state: 'waiting'|'loading'|'ready'|'error', url, why, ctl}
    const blobQueue = [];
    let blobsActive = 0;

    /** The cache entry for a members-only picture, asked for when it is first needed; null for anything else. */
    function blobFor(item) {
      const route = fileRoute(item, env.origin);
      if (!route.members || item.kind !== 'picture') return null;
      let e = blobs.get(item.id);
      if (e && e.path === route.members) return e;
      if (e) dropBlob(item.id);
      e = { path: route.members, state: 'waiting', url: '', why: '', ctl: null };
      blobs.set(item.id, e);
      blobQueue.push({ id: item.id, e });
      pumpBlobs();
      return e;
    }
    function pumpBlobs() {
      while (blobsActive < BLOB_FETCHES_AT_ONCE && blobQueue.length) {
        const t = token();
        if (!t) return; // signed in again later: renderAll pumps again
        const { id, e } = blobQueue.shift();
        if (blobs.get(id) !== e || e.state !== 'waiting') continue;
        blobsActive++;
        e.state = 'loading';
        fetchBlob(id, e, t).then(() => { blobsActive--; pumpBlobs(); }, () => { blobsActive--; pumpBlobs(); });
      }
    }
    async function fetchBlob(id, e, t) {
      let res = null;
      e.ctl = Abort ? new Abort() : null;
      try {
        res = await env.fetch(e.path, Object.assign({ headers: { 'X-Upload-Token': t } }, e.ctl ? { signal: e.ctl.signal } : {}));
        if (blobs.get(id) !== e) return; // no longer shown (and stopped)
        if (!res || !res.ok) {
          // A token from before the page signed in again: wait for the new one.
          if (res && res.status === 403 && token() !== t) { e.state = 'waiting'; e.ctl = null; blobQueue.push({ id, e }); return; }
          throw new Error(res ? await refusalWords(res) : 'The server could not be reached.');
        }
        const blob = await res.blob();
        if (blobs.get(id) !== e) return; // no longer shown
        e.url = URLs.createObjectURL(blob);
        e.state = 'ready';
      } catch (err) {
        if (blobs.get(id) !== e) return;
        e.state = 'error';
        e.why = MEDIA_WORDS.fileFailed + (err && err.message ? err.message : String(err));
      } finally {
        e.ctl = null;
      }
      blobChanged(id);
    }
    /** Forget a picture: stop its fetch if it is on its way, and revoke its object URL. */
    function dropBlob(id) {
      const e = blobs.get(id);
      if (!e) return;
      blobs.delete(id);
      if (e.ctl) { try { e.ctl.abort(); } catch (err) { /* already ended */ } }
      if (e.url && URLs) { try { URLs.revokeObjectURL(e.url); } catch (err) { /* already gone */ } }
    }
    /**
     * Stop and revoke every picture whose item is no longer shown (the grid, the strip and the
     * viewer). While a list loads afresh, what it showed before counts as shown until it answers,
     * so a reload does not throw away and fetch again the pictures it still shows.
     */
    function sweepBlobs() {
      const shown = new Set(st.reloadKeep || []);
      for (const i of st.items) shown.add(i.id);
      for (const i of st.featured) shown.add(i.id);
      if (st.viewer) shown.add(st.viewer.id);
      for (const id of Array.from(blobs.keys())) if (!shown.has(id)) dropBlob(id);
    }
    function blobChanged() {
      renderGrid();
      renderFeatured();
      renderViewerIfChanged();
    }

    // ── Tickets (the relay's R3-8): a members-only video or Download, streamed by the browser ──
    const tickets = new Map(); // item id -> {state: 'loading'|'ready'|'error', url, why, token, at, expiresAt, renewing}

    /** A ticket's address, when it is this server's members' file route for this item with a ticket. */
    function ticketPath(url, id) {
      if (typeof url !== 'string' || !url) return null;
      let u;
      try { u = new URL(url, env.origin); } catch (e) { return null; }
      if (u.origin !== new URL(env.origin).origin) return null;
      if (u.pathname !== '/api/media/file/' + id || !u.searchParams.get('ticket')) return null;
      return u.pathname + u.search;
    }
    /** Ask for a ticket: {path, expiresIn} (seconds), or throws Error(the relay's words). */
    async function fetchTicket(item, t) {
      const filename = itemFileName(item);
      if (!filename) throw new Error(MEDIA_WORDS.noFileName);
      const a = await answerOf(await env.fetch('/api/media/ticket', {
        method: 'POST', headers: { 'Content-Type': 'application/json', 'X-Upload-Token': t },
        body: JSON.stringify({ id: item.id, filename }),
      }));
      const path = ticketPath(a && a.url, item.id);
      if (!path) throw new Error(MEDIA_WORDS.ticketOdd);
      return { path, expiresIn: num(a.expires_in_s) || 600 };
    }
    /**
     * The viewer's ticket for a members-only video, asked for when there is none that can still
     * be used (one ending within half a minute is not). Null while the page is not signed in.
     */
    function ticketFor(item) {
      let e = tickets.get(item.id);
      const t = token();
      if (e && e.state === 'loading') return e;
      if (e && e.state === 'ready' && (e.renewing || e.expiresAt - TICKET_MARGIN_MS > now())) return e;
      if (e && e.state === 'error' && e.token === t) return e;
      if (!t) return e || null;
      e = { state: 'loading', url: '', why: '', token: t, at: now(), expiresAt: 0, renewing: false };
      tickets.set(item.id, e);
      fetchTicket(item, t).then((got) => {
        if (tickets.get(item.id) !== e) return;
        e.url = got.path;
        e.at = now();
        e.expiresAt = e.at + got.expiresIn * 1000;
        e.state = 'ready';
        ticketChanged(item.id);
      }, (err) => {
        if (tickets.get(item.id) !== e) return;
        e.state = 'error';
        e.why = MEDIA_WORDS.fileFailed + (err && err.message ? err.message : String(err));
        ticketChanged(item.id);
      });
      return e;
    }
    function ticketChanged() {
      renderViewerIfChanged();
    }
    /**
     * Download a members-only file through a ticket of its own (not the viewer's, so a video
     * playing beside it is never drawn again): the browser saves it from the ticket's address.
     */
    const downloadErrors = new Map(); // item id -> the words of a Download that could not start
    async function downloadMembers(item) {
      const t = token();
      if (!t || (st.saveWanted && st.saveWanted.id === item.id)) return;
      st.saveWanted = item;
      downloadErrors.delete(item.id);
      renderDownloadBox(item, '');
      let got = null;
      try {
        got = await fetchTicket(item, t);
      } catch (err) {
        downloadErrors.set(item.id, MEDIA_WORDS.fileFailed + (err && err.message ? err.message : String(err)));
      }
      // The viewer closed (or another item opened) meanwhile: nothing is saved.
      if (st.saveWanted !== item) return;
      st.saveWanted = null;
      if (got) saveFile(got.path, downloadName(item, st.kinds));
      renderDownloadBox(item, '');
    }
    /**
     * A playing video whose ticket has run out: a new ticket, the same video element, from where
     * it stopped. Not for a ticket younger than a minute (then it is the file that fails). A new
     * ticket refused (a ban 403, the item gone 404 or changed 409, too many open 429, or no answer)
     * leaves nothing to play, so the ticket becomes an error with the relay's words and the viewer
     * shows them where the video was (round 4, W4-2: it used to stay a dead video with no words).
     */
    function renewTicket(item, video) {
      const e = tickets.get(item.id);
      const t = token();
      if (!e || e.state !== 'ready' || e.renewing || !t || now() - e.at < TICKET_RENEW_AFTER_MS) return;
      e.renewing = true;
      let pos = 0;
      let playing = false;
      try { pos = Number(video.currentTime) || 0; playing = video.paused === false; } catch (err) { pos = 0; }
      fetchTicket(item, t).then((got) => {
        e.renewing = false;
        if (tickets.get(item.id) !== e) return;
        e.url = got.path;
        e.at = now();
        e.expiresAt = e.at + got.expiresIn * 1000;
        e.token = t;
        video.src = got.path;
        try { video.currentTime = pos; } catch (err) { /* the browser seeks once it can */ }
        if (playing && typeof video.play === 'function') { try { Promise.resolve(video.play()).catch(() => {}); } catch (err) { /* it waits for a press */ } }
      }, (err) => {
        e.renewing = false;
        if (tickets.get(item.id) !== e) return;
        e.state = 'error';
        e.token = t;
        e.why = MEDIA_WORDS.fileFailed + (err && err.message ? err.message : String(err));
        ticketChanged(item.id);
      });
    }
    function saveFile(url, name) {
      if (typeof env.save === 'function') { env.save(url, name); return; }
      const a = h('a', { href: url, hidden: true, attrs: { download: name } });
      (doc.body || env.root).appendChild(a);
      a.click();
      a.remove();
    }

    /** Where a picture is drawn from now: {src} or {why} (or {wait} while it loads). */
    function pictureSource(item) {
      const route = fileRoute(item, env.origin);
      if (route.why) return { why: route.why };
      if (route.public) return { src: route.public };
      if (!token() && !(blobs.get(item.id) && blobs.get(item.id).state === 'ready')) return { why: MEDIA_WORDS.waitingToken };
      const e = blobFor(item);
      if (e.state === 'ready') return { src: e.url };
      if (e.state === 'error') return { why: e.why };
      return { wait: MEDIA_WORDS.loadingFile };
    }

    // ── Tiles ──
    function copiesOf(id) {
      return st.items.filter((i) => i.id === id).concat(st.featured.filter((i) => i.id === id));
    }
    function tileActions(item) {
      const row = h('div', { className: 'media-tile-actions' });
      const mine = !!myKey && item.uploader_key === myKey;
      if (myKey && st.membersView && !mine) {
        const on = item.picked_by_me === true;
        row.appendChild(h('button', {
          type: 'button', className: 'media-btn media-btn-small', dataset: { action: on ? 'unpick' : 'pick' },
          text: on ? 'Unpick' : 'Pick', on: { click: () => pick(item, !on) },
        }));
      }
      if (mine || isAdmin()) {
        row.appendChild(h('button', {
          type: 'button', className: 'media-btn media-btn-small media-btn-danger', dataset: { action: 'remove' },
          text: 'Remove', on: { click: () => remove(item) },
        }));
      }
      if (isAdmin()) {
        row.appendChild(h('button', {
          type: 'button', className: 'media-btn media-btn-small', dataset: { action: item.featured ? 'unfeature' : 'feature' },
          text: item.featured ? 'Unfeature' : 'Feature', on: { click: () => feature(item, !item.featured) },
        }));
      }
      if (isStaff()) {
        row.appendChild(h('button', {
          type: 'button', className: 'media-btn media-btn-small', dataset: { action: item.kept ? 'unkeep' : 'keep' },
          text: item.kept ? MEDIA_WORDS.stopKeeping : MEDIA_WORDS.keep, on: { click: () => keep(item, !item.kept) },
        }));
      }
      if (myKey && !mine && R && typeof item.uploader_key === 'string' && item.uploader_key) {
        row.appendChild(h('button', {
          type: 'button', className: 'media-btn media-btn-small', dataset: { action: 'report' },
          text: 'Report', on: { click: () => openReport(item) },
        }));
      }
      return row.childNodes.length ? row : null;
    }

    function thumb(item) {
      const labels = labelsOf(item, st.labels);
      if (labels.length && !st.revealed.has(item.id)) {
        // The label instead of the picture until clicked: nothing is fetched before then.
        return h('button', {
          type: 'button', className: 'media-thumb media-veil', dataset: { veil: item.id },
          on: { click: () => { st.revealed.add(item.id); renderGrid(); renderFeatured(); } },
        }, [
          h('span', { className: 'media-veil-labels', text: labels.map((l) => l.label).join(', ') }),
          h('span', { className: 'media-veil-show', text: MEDIA_WORDS.clickToShow }),
        ]);
      }
      const open = () => openViewer(item);
      if (item.kind === 'picture') {
        const where = pictureSource(item);
        if (where.why) return h('div', { className: 'media-thumb media-thumb-missing', text: where.why });
        if (where.wait) return h('button', { type: 'button', className: 'media-thumb media-thumb-loading', text: where.wait, on: { click: open } });
        return h('button', { type: 'button', className: 'media-thumb', on: { click: open } }, [
          h('img', { className: 'media-img', src: where.src, alt: itemName(item, st.kinds), loading: 'lazy' }),
        ]);
      }
      const words = kindWords(item.kind, st.kinds);
      return h('button', { type: 'button', className: 'media-thumb media-mark media-mark-' + words.mark, on: { click: open } }, [
        h('span', { className: 'media-mark-word', text: words.one }),
        h('span', { className: 'media-mark-size', text: sizeWords(item.size_bytes) }),
      ]);
    }

    function tile(item) {
      const badges = h('div', { className: 'media-badges' });
      for (const l of labelsOf(item, st.labels)) {
        badges.appendChild(h('span', { className: 'media-badge media-badge-label', text: l.label, title: l.desc || l.label }));
      }
      if (item.visibility === 'members') badges.appendChild(h('span', { className: 'media-badge', text: 'Members only' }));
      if (item.kept) badges.appendChild(h('span', { className: 'media-badge media-badge-kept', dataset: { kept: item.id }, text: MEDIA_WORDS.keptBadge }));
      const picks = Number(item.picks) || 0;
      return h('div', { className: 'media-tile', dataset: { id: item.id } }, [
        thumb(item),
        badges.childNodes.length ? badges : null,
        h('div', { className: 'media-tile-title', text: itemName(item, st.kinds) }),
        h('div', { className: 'media-tile-meta', text: 'by ' + (item.uploader_name || 'someone') + ' · ' + picks + (picks === 1 ? ' pick' : ' picks') }),
        tileActions(item),
      ]);
    }

    function renderFeatured() {
      els.featured.replaceChildren();
      if (!st.featured.length) return;
      els.featured.appendChild(h('h2', { text: 'Featured' }));
      els.featured.appendChild(h('div', { id: 'media-featured-row', className: 'media-featured-row' }, st.featured.map(tile)));
    }

    function renderGrid() {
      els.grid.replaceChildren();
      if (st.listError) {
        els.grid.appendChild(h('p', { className: 'media-empty media-warn', text: st.listError }));
      } else if (!st.items.length) {
        let words = MEDIA_WORDS.loading;
        if (!st.loading) {
          const filtered = !!(st.filters.category || st.filters.kind || st.filters.by || st.filters.q || st.filters.picks);
          words = st.membersOnlyForYou ? MEDIA_WORDS.membersOnlyServer : filtered ? MEDIA_WORDS.emptyFiltered : MEDIA_WORDS.empty;
        }
        els.grid.appendChild(h('p', { className: 'media-empty', text: words }));
      } else {
        for (const item of st.items) els.grid.appendChild(tile(item));
      }
      els.more.replaceChildren();
      if (st.nextBefore != null && !st.listError) {
        els.more.appendChild(h('button', {
          type: 'button', id: 'media-more', className: 'media-btn', text: st.loading ? MEDIA_WORDS.loading : 'More',
          disabled: st.loading, on: { click: () => load(false) },
        }));
      }
    }

    // ── The viewer: drawn again only when what it shows changes ──
    function openViewer(item) {
      // A ticket refused earlier is asked for afresh when the person opens the item again (the
      // relay's words for too many open say to do just that); while it stays open it is not asked
      // again, so a refusal never becomes a loop of requests.
      const was = item && tickets.get(item.id);
      if (was && was.state === 'error') tickets.delete(item.id);
      st.viewer = item;
      renderViewerIfChanged();
    }
    function closeViewer() {
      st.viewer = null;
      st.saveWanted = null;
      renderViewerIfChanged();
      sweepBlobs();
    }
    /** A labelled item not yet clicked: its label instead of its file, nothing fetched. */
    function veiled(item) {
      return labelsOf(item, st.labels).length > 0 && !st.revealed.has(item.id);
    }
    /** What the viewer shows, as a key: the item, whether it is veiled, and for a members-only file where its loading is. */
    function viewerKey() {
      const item = st.viewer;
      if (!item) return '';
      const route = fileRoute(item, env.origin);
      let file = route.why ? 'missing' : route.public || '';
      if (route.members) {
        // A picture's object URL, or a ticket's state (not its address: a ticket renewed under a
        // playing video does not draw the video again).
        // A ticket refused under an earlier sign-in is asked for again under the new one.
        const e = item.kind === 'picture' ? blobs.get(item.id) : tickets.get(item.id);
        const stale = e && item.kind !== 'picture' && e.state === 'error' && e.token !== token();
        file = e ? (stale ? 'stale' : e.state) + ':' + (item.kind === 'picture' ? e.url : '') : token() ? 'members' : 'no-token';
      }
      // A Download on its way is not part of it: its box is drawn on its own (renderDownloadBox),
      // so pressing Download beside a playing video never starts the video again.
      return [item.id, itemFileName(item), veiled(item) ? 'veiled' : 'shown', file].join('|');
    }
    function renderViewerIfChanged() {
      const key = viewerKey();
      if (key === st.viewerKey) return;
      renderViewer();
      // Drawing it may have asked for its file (its state is then part of the key).
      st.viewerKey = viewerKey();
    }
    /**
     * The Download for an item in the viewer: a link for a public file; for a members-only one,
     * the fetched picture when it is here, else a ticket asked for on the press, which the browser
     * then saves itself (never through the page's memory).
     */
    function downloadControl(item, ready) {
      const route = fileRoute(item, env.origin);
      const name = downloadName(item, st.kinds);
      if (route.public) return h('a', { className: 'media-btn media-btn-primary', id: 'media-download', href: route.public, text: 'Download', attrs: { download: name } });
      if (!route.members) return null;
      if (ready) return h('a', { className: 'media-btn media-btn-primary', id: 'media-download', href: ready, text: 'Download', attrs: { download: name } });
      const saving = st.saveWanted && st.saveWanted.id === item.id;
      return h('button', {
        type: 'button', className: 'media-btn media-btn-primary', id: 'media-download', text: saving ? MEDIA_WORDS.loadingFile : 'Download', disabled: !!saving || !token(),
        on: { click: () => downloadMembers(item) },
      });
    }
    function renderViewer() {
      els.viewer.replaceChildren();
      const item = st.viewer;
      if (!item) return;
      const name = itemName(item, st.kinds);
      const route = fileRoute(item, env.origin);
      const covered = veiled(item);
      let body;
      let ready = '';
      if (route.why) {
        body = h('p', { className: 'media-warn', text: route.why });
      } else if (covered) {
        // The label cover, as on its tile (/media?id=<n> opens the viewer directly): nothing is
        // fetched until it is clicked.
        body = h('button', {
          type: 'button', id: 'media-viewer-veil', className: 'media-thumb media-veil media-viewer-veil', dataset: { veil: item.id },
          on: { click: () => { st.revealed.add(item.id); renderViewerIfChanged(); renderGrid(); renderFeatured(); } },
        }, [
          h('span', { className: 'media-veil-labels', text: labelsOf(item, st.labels).map((l) => l.label).join(', ') }),
          h('span', { className: 'media-veil-show', text: MEDIA_WORDS.clickToShow }),
        ]);
      } else if (item.kind === 'picture' || item.kind === 'video') {
        let src = route.public || '';
        let why = '';
        let ticketed = false;
        if (route.members) {
          // A picture through its object URL; a video through a ticket, streamed by the browser.
          const e = item.kind === 'picture'
            ? blobs.get(item.id) || (token() ? blobFor(item) : null)
            : ticketFor(item);
          if (e && e.state === 'ready') {
            src = e.url;
            if (item.kind === 'picture') ready = e.url;
            else ticketed = true;
          } else if (e && e.state === 'error') why = e.why;
          else why = token() ? MEDIA_WORDS.loadingFile + ' ' + sizeWords(item.size_bytes) : MEDIA_WORDS.waitingToken;
        }
        if (!src) body = h('p', { className: 'media-viewer-wait', id: 'media-viewer-wait', text: why });
        else if (item.kind === 'picture') body = h('img', { className: 'media-viewer-img', src, alt: name });
        else {
          body = h('video', { className: 'media-viewer-video', src, controls: true, preload: 'metadata' });
          if (ticketed) body.addEventListener('error', () => renewTicket(item, body));
        }
      } else {
        body = h('div', { className: 'media-viewer-file' }, [
          h('div', { className: 'media-viewer-name', text: name }),
          h('div', { className: 'media-viewer-size', text: sizeWords(item.size_bytes) }),
        ]);
      }
      const labels = labelsOf(item, st.labels).map((l) => l.label);
      const overlay = h('div', { id: 'media-viewer-open', className: 'media-viewer', attrs: { role: 'dialog', 'aria-label': name } }, [
        h('div', { className: 'media-viewer-box' }, [
          h('button', { type: 'button', id: 'media-viewer-close', className: 'media-btn media-viewer-close', text: 'Close', on: { click: closeViewer } }),
          body,
          h('h2', { className: 'media-viewer-title', text: name }),
          item.description ? h('p', { className: 'media-viewer-desc', text: item.description }) : null,
          h('p', { className: 'media-viewer-meta', text: [
            'by ' + (item.uploader_name || 'someone'),
            item.category && item.category.name ? item.category.name : '',
            kindWords(item.kind, st.kinds).one,
            sizeWords(item.size_bytes),
            labels.length ? labels.join(', ') : '',
            item.kept ? MEDIA_WORDS.keptBadge : '',
          ].filter(Boolean).join(' · ') }),
          route.why || covered ? null : (els.downloadBox = h('div', { id: 'media-download-box' })),
        ]),
      ]);
      overlay.addEventListener('click', (e) => { if (e.target === overlay) closeViewer(); });
      els.viewer.appendChild(overlay);
      if (!route.why && !covered) renderDownloadBox(item, ready);
    }
    /** The viewer's Download and what it says when it could not start: drawn without the rest of the viewer. */
    function renderDownloadBox(item, ready) {
      const box = els.downloadBox;
      if (!box || !st.viewer || st.viewer.id !== item.id) return;
      box.replaceChildren(h('div', { className: 'media-form-buttons' }, [downloadControl(item, ready)]));
      if (downloadErrors.has(item.id)) box.appendChild(h('p', { className: 'media-warn', id: 'media-download-error', text: downloadErrors.get(item.id) }));
    }

    // ── Loading ──
    function takeAnswer(a) {
      const items = Array.isArray(a && a.items) ? a.items : [];
      st.items = st.items.concat(items);
      st.nextBefore = a && a.next_before != null ? a.next_before : null;
      st.answered = true;
      st.membersView = !!(a && a.members_view);
      st.membersOnlyForYou = !!(a && a.members_only_for_you);
      if (a && typeof a.media_visibility === 'string') st.mediaVisibility = a.media_visibility;
      if (Array.isArray(a && a.categories) && (a.categories.length || !st.membersView)) st.categories = a.categories;
      if (Array.isArray(a && a.labels) && (a.labels.length || !st.membersView)) st.labels = a.labels;
      takePeople(items);
    }
    function takePeople(items) {
      const kinds = st.seenKinds.slice();
      for (const i of items) {
        if (typeof i.kind === 'string' && i.kind && !kinds.includes(i.kind)) kinds.push(i.kind);
        if (typeof i.uploader_key === 'string' && i.uploader_key && !st.people.has(i.uploader_key)) st.people.set(i.uploader_key, i.uploader_name || '');
      }
      st.seenKinds = kinds;
    }

    /** The members' list, signed, when the person has an identity here; else the public one. */
    async function fetchList(params) {
      if (myKey) {
        const auth = await env.sign(purposes.list());
        if (auth) return env.fetch('/api/media/list', jsonPost(Object.assign({}, params, auth)));
      }
      return env.fetch(publicListUrl(params));
    }

    /** The person's own picks, signed like the list (the relay's A3), members-only items included. */
    async function fetchPicks(params) {
      const auth = await env.sign(purposes.list());
      if (!auth) throw new Error(MEDIA_WORDS.cannotSign);
      return env.fetch('/api/media/picks', jsonPost(Object.assign({}, params, { of: myKey }, auth)));
    }

    async function load(reset) {
      const seq = ++loadSeq;
      if (reset) {
        // The pictures shown until now stay until this answers (sweepBlobs), so a reload keeps
        // the ones it still shows instead of fetching each again.
        st.reloadKeep = new Set(Array.from(st.reloadKeep || []).concat(st.items.map((i) => i.id)));
        st.items = [];
        st.nextBefore = null;
      }
      st.loading = true;
      st.listError = '';
      renderGrid();
      const params = listParams(st.filters, reset ? null : st.nextBefore);
      const fromPicks = !!(st.filters.picks && myKey);
      try {
        const res = fromPicks ? await fetchPicks(params) : await fetchList(params);
        const a = await answerOf(res);
        if (seq !== loadSeq) return;
        takeAnswer(a);
      } catch (e) {
        if (seq !== loadSeq) return;
        st.listError = 'The Media list could not be loaded: ' + (e && e.message ? e.message : String(e));
      }
      st.loading = false;
      st.reloadKeep = null;
      renderAll();
    }

    /** Every featured item (`featured=1`), not only those on the pages loaded. */
    async function loadFeatured() {
      const seq = ++featuredSeq;
      let featured = [];
      try {
        const a = await answerOf(await fetchList(listParams({ featured: 1 }, null, FEATURED_LIMIT)));
        featured = Array.isArray(a.items) ? a.items : [];
      } catch (e) {
        featured = [];
      }
      if (seq !== featuredSeq) return;
      st.featured = featured;
      takePeople(featured);
      renderFeatured();
      sweepBlobs();
    }

    /** The kinds file: the filter's choices and each kind's name (data/media/kinds.json). */
    async function loadKinds() {
      try {
        const res = await env.fetch(KINDS_URL);
        if (res && res.ok) st.kinds = parseKinds(await res.json());
      } catch (e) {
        st.kinds = [];
      }
    }

    /**
     * The person's own use of the Media space, counted by the relay exactly as
     * its room check counts it (A4): used, the share, what the budget has left,
     * uploads still arriving, the role's largest Media file, and the uploads
     * waiting to be published.
     */
    async function loadMine() {
      if (!myKey || !st.membersView) return null;
      const seq = ++mineSeq;
      try {
        const auth = await env.sign(purposes.mine());
        if (!auth) return null;
        const a = await answerOf(await env.fetch('/api/media/mine', jsonPost(auth)));
        if (seq === mineSeq) st.mine = a;
      } catch (e) {
        if (seq === mineSeq && e && e.status === 403) st.mine = null;
      }
      renderWaiting();
      return st.mine;
    }

    // ── Pick, remove, feature, keep ──
    async function signed(purpose) {
      const auth = await env.sign(purpose);
      if (!auth) say(MEDIA_WORDS.cannotSign, true);
      return auth;
    }

    /**
     * A write on an item, naming its number and its file (W5). A tile left on a
     * page that is out of date is answered 409, and the page loads again.
     * Resolves to the relay's answer, or null.
     */
    async function act(path, purpose, item, body) {
      const filename = itemFileName(item);
      if (!filename) { say(MEDIA_WORDS.noFileName, true); return null; }
      const auth = await signed(purpose);
      if (!auth) return null;
      let res;
      try {
        res = await env.fetch(path, jsonPost(Object.assign({ id: item.id, filename }, body || {}, auth)));
      } catch (e) {
        say('The server could not be reached: ' + (e && e.message ? e.message : String(e)), true);
        return null;
      }
      if (res && res.status === 409) {
        const words = await refusalWords(res);
        if (/not the Media item you were looking at/i.test(words)) {
          say(MEDIA_WORDS.reloaded, true);
          if (st.viewer && st.viewer.id === item.id) closeViewer();
          load(true);
          loadFeatured();
          return null;
        }
        say(words, true);
        return null;
      }
      try {
        return await answerOf(res);
      } catch (e) {
        say(e.message, true);
        return null;
      }
    }

    async function pick(item, on) {
      const a = await act(on ? '/api/media/pick' : '/api/media/unpick', purposes.pick(item.id), item);
      if (a) {
        for (const c of copiesOf(item.id)) { c.picks = Number(a.picks) || 0; c.picked_by_me = on; }
        say(on ? 'Picked. It is on your Picks now.' : 'Unpicked.');
      }
      renderGrid();
      renderFeatured();
    }

    async function remove(item) {
      const ok = await env.confirm('"' + itemName(item, st.kinds) + '"\n\n' + removeWords(item));
      if (!ok) return;
      const a = await act('/api/media/remove', purposes.remove(item.id), item);
      if (!a) return;
      st.items = st.items.filter((i) => i.id !== item.id);
      st.featured = st.featured.filter((i) => i.id !== item.id);
      if (st.viewer && st.viewer.id === item.id) st.viewer = null;
      say(removedWords(a));
      renderAll();
      loadMine();
    }

    async function feature(item, on) {
      const a = await act('/api/media/feature', purposes.feature(item.id), item, { featured: on });
      if (a) {
        for (const c of copiesOf(item.id)) c.featured = on;
        say(on ? 'Featured: it leads the Media page now.' : 'Taken off the featured row.');
        loadFeatured();
      }
      renderGrid();
      renderFeatured();
    }

    async function keep(item, on) {
      const a = await act('/api/media/keep', purposes.keep(item.id, on), item, { kept: on });
      if (a) {
        for (const c of copiesOf(item.id)) c.kept = on;
        say(on ? MEDIA_WORDS.keptNow : MEDIA_WORDS.keptStopped);
      }
      renderGrid();
      renderFeatured();
    }

    // ── The person's uploads waiting to be published ──
    const cancellingUnfinished = new Set(); // ids of the unfinished uploads whose cancel is on its way
    function renderWaiting() {
      els.waiting.replaceChildren();
      // What this page's own uploads list shows already is not listed twice.
      const publishing = new Set(st.uploads.map((u) => u.uploadedName).filter(Boolean));
      const tracked = new Set(st.uploads.map((u) => u.up && u.up.state.id).filter(Boolean));
      const waiting = (st.mine && Array.isArray(st.mine.waiting) ? st.mine.waiting : []).filter((w) => !publishing.has(w.filename));
      const unfinished = (st.mine && Array.isArray(st.mine.uploads_in_progress) ? st.mine.uploads_in_progress : [])
        .filter((p) => p && typeof p.id === 'string' && p.id && !tracked.has(p.id));
      if (waiting.length) {
        const rows = waiting.map((w) => h('div', { className: 'media-admin-row', dataset: { waiting: w.filename } }, [
          h('div', { className: 'media-admin-row-text', text: (w.original_name || w.filename) + ' · ' + sizeWords(w.size_bytes) }),
          h('button', { type: 'button', className: 'media-btn media-btn-small', dataset: { waitingPublish: w.filename }, text: 'Publish', on: { click: () => openWaitingForm(w) } }),
          h('button', { type: 'button', className: 'media-btn media-btn-small media-btn-danger', dataset: { waitingRemove: w.filename }, text: 'Remove', on: { click: () => removeWaiting(w) } }),
        ]));
        els.waiting.appendChild(h('div', { className: 'media-panel', id: 'media-waiting-list' }, [
          h('h2', { text: MEDIA_WORDS.waitingTitle }),
          h('p', { className: 'media-hint', text: MEDIA_WORDS.waitingHint }),
          h('div', { className: 'media-admin-rows' }, rows),
        ]));
      }
      if (unfinished.length) {
        // The person's own resumable sessions the server holds (the relay's R3-7) that no upload
        // on this page is carrying: one left by a closed tab, another browser or a failure.
        const rows = unfinished.map((p) => h('div', { className: 'media-admin-row', dataset: { unfinished: p.id } }, [
          h('div', { className: 'media-admin-row-text', text: String(p.filename || '') + ' · ' + sizeWords(p.offset) + ' of ' + sizeWords(p.size) + ' sent' }),
          h('button', {
            type: 'button', className: 'media-btn media-btn-small media-btn-danger', dataset: { unfinishedCancel: p.id },
            text: cancellingUnfinished.has(p.id) ? MEDIA_WORDS.cancellingUnfinished : 'Cancel', disabled: cancellingUnfinished.has(p.id),
            on: { click: () => cancelUnfinished(p) },
          }),
        ]));
        els.waiting.appendChild(h('div', { className: 'media-panel', id: 'media-unfinished-list' }, [
          h('h2', { text: MEDIA_WORDS.unfinishedTitle }),
          h('p', { className: 'media-hint', text: MEDIA_WORDS.unfinishedHint }),
          h('div', { className: 'media-admin-rows' }, rows),
        ]));
      }
    }

    /**
     * What a cancel the server answered means, said on the page (round 4's cancel answer,
     * media-upload.js `cancelSession`): nothing for a plain cancel, which the list shows. A cancel
     * never confirmed is one the server never heard, or one it answered with an error every time
     * (its 500 when the stored file could not be deleted keeps everything, to ask again): its
     * last words are said then.
     */
    function sayCancelled(answer, words) {
      if (answer === 'removed_stored') say(MEDIA_WORDS.cancelledStored);
      else if (answer === 'published') say(MEDIA_WORDS.cancelledPublished);
      else if (answer === 'unconfirmed') say(words ? MEDIA_WORDS.cancelNotFinished + words : MEDIA_WORDS.cancelUnconfirmed, true);
      else if (answer === 'refused') say(words, true);
    }

    /**
     * Cancel an unfinished upload the server holds: DELETE with the token in the header, asked
     * again while the server is busy with it (`cancelSession`). Its row says so meanwhile.
     */
    async function cancelUnfinished(p) {
      if (cancellingUnfinished.has(p.id)) return;
      if (!await env.confirm('"' + String(p.filename || '') + '"\n\n' + MEDIA_WORDS.unfinishedCancel)) return;
      if (!token()) { say(MEDIA_WORDS.notSignedInCancel, true); return; }
      cancellingUnfinished.add(p.id);
      renderWaiting();
      let told;
      try {
        told = await cancelSession({ fetch: env.fetch, token, sleep: env.sleep }, p.id);
      } finally {
        cancellingUnfinished.delete(p.id);
      }
      if (told.answer === 'unconfirmed' || told.answer === 'refused') {
        sayCancelled(told.answer, told.words);
      } else {
        if (forgetUpload) forgetUpload(env.storage, p.id);
        if (told.answer === 'published') sayCancelled(told.answer, '');
        else say(MEDIA_WORDS.unfinishedCancelled);
      }
      renderWaiting();
      loadMine();
    }

    async function removeWaiting(w) {
      if (!await env.confirm('"' + (w.original_name || w.filename) + '"\n\n' + MEDIA_WORDS.removeWaiting)) return;
      const auth = await signed(purposes.remove(w.filename));
      if (!auth) return;
      try {
        await answerOf(await env.fetch('/api/media/remove', jsonPost(Object.assign({ filename: w.filename }, auth))));
        say(MEDIA_WORDS.removedWaiting);
        if (st.form && st.form.mode === 'waiting' && st.form.filename === w.filename) closeForm();
      } catch (e) {
        say(e.message, true);
      }
      loadMine();
    }

    // ── Add to Media, Put on Media, and publishing an upload that waits ──
    //
    // `st.form` is the one form not yet uploading. Pressing "Upload and add" moves that form,
    // fields and all, into the uploads list (`st.uploads`, drawn in #media-uploads): there it
    // shows its own bar, words, Pause, Resume and Cancel, and publishes ITSELF once its file is
    // up, with what was typed in it (round 3, review 2). So the form area is free again at once,
    // and opening another form never leaves an upload without the form that publishes it.

    /** Phases in which an upload is still under way (not ended, not being cancelled). */
    function uploadLive(f) {
      const p = f && f.up ? f.up.state.phase : '';
      return !!p && p !== 'failed' && p !== 'cancelling' && p !== 'cancelled' && p !== 'done';
    }
    const sameFile = (a, b) => !!(a && b && a.name === b.name && a.size === b.size && a.lastModified === b.lastModified);

    async function openAddForm(file) {
      const f = { mode: 'add', file, name: file.name, stop: null, shareLine: '', checking: true, busy: false, error: '', uploadedName: '', up: null, fp: '' };
      st.form = f;
      renderForm();
      if (!canUpload()) {
        f.checking = false;
        f.stop = bound() ? MEDIA_WORDS.cannotUpload : MEDIA_WORDS.notSignedInUpload;
        renderForm();
        return;
      }
      // The same file already going up on this page: never two uploads of it at once.
      if (st.uploads.some((u) => sameFile(u.file, file) && uploadLive(u))) {
        f.checking = false;
        f.stop = '"' + file.name + '"' + MEDIA_WORDS.alreadyUploading;
        renderForm();
        return;
      }
      // Its fingerprint (the first and last megabyte), which a resume is matched by.
      try { f.fp = fingerprint ? await fingerprint(file, env.digest) : ''; } catch (e) { f.fp = ''; }
      if (st.form !== f) return;
      // Until the relay has said what is left, nothing can be uploaded, so an upload over it never starts.
      const mine = await loadMine();
      if (st.form !== f) return;
      f.checking = false;
      if (mine) {
        // An upload of this same file already part way up is counted by the relay as in progress
        // already; its own bytes are not "more" on top of that.
        const known = f.fp ? findUpload(env.storage, myKey, file, f.fp, now()) : null;
        const inProgress = Math.max(0, num(mine.in_progress_bytes) - (known ? file.size : 0));
        const used = num(mine.used_bytes) + inProgress;
        const share = num(mine.share_bytes);
        if (share) f.shareLine = shareLeftWords(used, share);
        f.stop = limitStop(file, num(mine.file_limit_bytes)) || shareStop(file, used, share) ||
          (Number.isFinite(Number(mine.budget_left_bytes)) ? budgetStop(file, Number(mine.budget_left_bytes)) : null);
      }
      renderForm();
    }

    function openPutForm(put) {
      st.form = { mode: 'put', filename: storedName(put.filename), name: put.name || storedName(put.filename), stop: null, shareLine: '', busy: false, error: '', uploadedName: '' };
      renderForm();
    }

    function openWaitingForm(w) {
      st.form = { mode: 'waiting', filename: w.filename, name: w.original_name || w.filename, size: w.size_bytes, stop: null, shareLine: '', busy: false, error: '', uploadedName: w.filename };
      renderForm();
    }

    /** Close the form not yet uploading (an upload in the list goes by its own Cancel or Close). */
    function closeForm() {
      st.form = null;
      renderForm();
    }

    /** What the form is drawn from, so it is drawn again when that changes. */
    function formKey() {
      return JSON.stringify([st.categories.map((c) => [c.id, c.name]), st.labels.map((l) => l.id), membersOnlyServer()]);
    }

    let formSeq = 0;
    function renderForm() {
      els.form.replaceChildren();
      const f = st.form;
      if (!f) return;
      f.key = formKey();
      f.formId = f.formId || ++formSeq;
      const put = f.mode === 'put';
      const kids = [h('h2', { text: put ? 'Put on Media' : f.mode === 'waiting' ? 'Publish an upload' : 'Add to Media' })];
      const size = f.file ? f.file.size : f.size;
      kids.push(h('p', { className: 'media-form-file', text: size != null ? f.name + ' (' + sizeWords(size) + ')' : f.name }));
      if (f.shareLine) kids.push(h('p', { id: 'media-share-line', className: 'media-share-line', text: f.shareLine }));
      f.actionsEl = h('div', { id: 'media-form-actions', className: 'media-form-actions' });
      if (f.stop) {
        kids.push(h('p', { id: 'media-form-stop', className: 'media-warn', text: f.stop }));
        kids.push(f.actionsEl);
        f.panel = h('div', { className: 'media-panel media-form' }, kids);
        els.form.appendChild(f.panel);
        renderFormActions(f);
        return;
      }
      const title = h('input', { type: 'text', id: 'media-form-title', className: 'media-input', maxLength: 120, value: f.title || '' });
      const category = h('select', { id: 'media-form-category', className: 'media-select' },
        st.categories.map((c) => h('option', { value: String(c.id), text: c.name, selected: String(f.category || '') === String(c.id) })));
      const description = h('textarea', { id: 'media-form-description', className: 'media-input', maxLength: 2000, rows: 3, value: f.description || '' });
      // What is typed is kept as it is typed, so drawing the form again loses nothing, and an
      // upload publishes with what its form says when its file is up.
      title.addEventListener('input', () => { f.title = title.value; });
      category.addEventListener('change', () => { f.category = category.value; });
      description.addEventListener('input', () => { f.description = description.value; });
      kids.push(h('label', { className: 'media-field' }, ['Title (optional)', title]));
      if (st.categories.length) kids.push(h('label', { className: 'media-field' }, ['Category', category]));
      kids.push(h('label', { className: 'media-field' }, ['Description (optional)', description]));
      f.ticks = [];
      if (st.labels.length) {
        const box = h('fieldset', { className: 'media-field media-labels', id: 'media-form-labels' }, [h('legend', { text: 'Labels (the picture stays hidden until someone clicks it)' })]);
        for (const l of st.labels) {
          const tick = h('input', { type: 'checkbox', value: l.id, dataset: { label: l.id }, checked: !!(f.labels && f.labels.includes(l.id)) });
          tick.addEventListener('change', () => { f.labels = f.ticks.filter((t) => t.checked).map((t) => t.value); });
          f.ticks.push(tick);
          box.appendChild(h('label', { className: 'media-check', title: l.desc || '' }, [tick, ' ' + l.label]));
          if (l.desc) box.appendChild(h('div', { className: 'media-hint', text: l.desc }));
        }
        kids.push(box);
      }
      // Who sees it: a file already public goes on Media as public only (the relay's
      // `was_public`), and a members-only server takes members only. Each form's choice is its
      // own group (its own name), so another form open beside an upload never changes it.
      f.visibilityEls = null;
      if (put) {
        kids.push(h('p', { id: 'media-form-public-only', className: 'media-hint', text: MEDIA_WORDS.alreadyPublic }));
      } else if (membersOnlyServer()) {
        kids.push(h('p', { id: 'media-form-members-only', className: 'media-hint', text: MEDIA_WORDS.serverMembersOnly }));
      } else {
        const group = 'media-form-visibility-' + f.formId;
        const pub = h('input', { type: 'radio', name: group, value: 'public', id: 'media-form-vis-public', checked: f.visibility !== 'members' });
        const mem = h('input', { type: 'radio', name: group, value: 'members', id: 'media-form-vis-members', checked: f.visibility === 'members' });
        f.visibilityEls = { pub, mem };
        pub.addEventListener('change', () => { if (pub.checked) f.visibility = 'public'; });
        mem.addEventListener('change', () => { if (mem.checked) f.visibility = 'members'; });
        kids.push(h('fieldset', { className: 'media-field', id: 'media-form-visibility' }, [
          h('legend', { text: 'Who sees it' }),
          h('label', { className: 'media-check' }, [pub, ' Public']),
          h('label', { className: 'media-check' }, [mem, ' Members only']),
        ]));
      }
      f.inputs = { title, category, description };
      kids.push(f.actionsEl);
      f.panel = h('div', { className: 'media-panel media-form' }, kids);
      els.form.appendChild(f.panel);
      renderFormActions(f);
    }

    /** Give every id inside a moved form a suffix of its own, so the next form's ids stay unique. */
    function suffixIds(el, suffix) {
      for (const c of Array.from(el.children || [])) {
        if (c.id) c.id = c.id + suffix;
        suffixIds(c, suffix);
      }
    }

    /** The upload's words for where it is. */
    function uploadWords(u) {
      const s = u.state;
      const sent = sizeWords(s.offset) + ' of ' + sizeWords(s.size) + ' sent';
      const seconds = (n) => (n || 1) + (n === 1 ? ' second.' : ' seconds.');
      switch (s.phase) {
        case 'idle':
        case 'starting': return MEDIA_WORDS.uploadStarting;
        case 'sending': return (s.resumed ? MEDIA_WORDS.uploadResumed + ' ' : '') + sent + '.';
        case 'finishing': return MEDIA_WORDS.uploadFinishing;
        case 'pausing': return sent + '. ' + MEDIA_WORDS.uploadPausing;
        case 'paused': return sent + '. ' + MEDIA_WORDS.uploadPaused;
        case 'reconnecting': return sent + '. The connection dropped. Trying again in ' + seconds(s.retryIn);
        case 'server_wait': return sent + '. ' + (s.error ? s.error + ' ' : '') + 'Trying again in ' + seconds(s.retryIn);
        case 'waiting_connection': return sent + '. ' + MEDIA_WORDS.uploadWaitingConnection;
        case 'allowance': return [s.error, resetsWords(s.resetsAt), MEDIA_WORDS.uploadAllowanceThen].filter(Boolean).join(' ');
        case 'server_error': return s.error + MEDIA_WORDS.uploadServerErrorTail;
        case 'no_room': return s.error;
        case 'failed': return s.error;
        case 'cancelling': return MEDIA_WORDS.uploadCancelling;
        case 'cancelled': return MEDIA_WORDS.uploadCancelling;
        case 'done': return MEDIA_WORDS.uploadPublishing;
        default: return sent + '.';
      }
    }

    /**
     * The form's progress, words and buttons: drawn again on every change without touching the
     * fields. A form in the uploads list names its controls by `data-upload-*` (there may be
     * several); the form not yet uploading keeps its ids.
     */
    function renderFormActions(f) {
      const box = f.actionsEl;
      if (!box) return;
      box.replaceChildren();
      const listed = !!f.n;
      const button = (action, text, onClick, cls) => h('button', {
        type: 'button', id: listed ? undefined : 'media-form-' + action, dataset: listed ? { uploadAction: action } : undefined,
        className: 'media-btn' + (cls ? ' ' + cls : ''), text, on: { click: onClick },
      });
      if (f.stop) {
        box.appendChild(h('div', { className: 'media-form-buttons' }, [button('cancel', 'Close', closeForm)]));
        return;
      }
      if (f.error) box.appendChild(h('p', { id: listed ? undefined : 'media-form-error', dataset: listed ? { uploadError: f.n } : undefined, className: 'media-warn', text: f.error }));
      const u = f.up;
      if (u) {
        const s = u.state;
        const pct = s.size > 0 ? Math.min(100, Math.round((s.offset / s.size) * 1000) / 10) : 0;
        const fill = h('div', { className: 'media-meter-fill' });
        fill.style.width = pct + '%';
        box.appendChild(h('div', { className: 'media-meter', dataset: { uploadBar: f.n }, attrs: { role: 'progressbar', 'aria-valuemin': 0, 'aria-valuemax': 100, 'aria-valuenow': pct, 'aria-label': 'Upload of ' + f.name } }, [fill]));
        const warn = s.phase === 'allowance' || s.phase === 'no_room' || s.phase === 'failed' || s.phase === 'server_error';
        box.appendChild(h('p', { dataset: { uploadWords: f.n }, className: warn ? 'media-warn' : 'media-hint', text: uploadWords(u) }));
        const buttons = [];
        const p = s.phase;
        if (p === 'starting' || p === 'sending' || p === 'finishing' || p === 'reconnecting' || p === 'server_wait' || p === 'waiting_connection' || p === 'allowance') {
          // Pause stops it in every phase: the request on its way and every wait.
          buttons.push(button('pause', 'Pause', () => u.pause()));
        } else if (p === 'paused') {
          buttons.push(button('resume', 'Resume', () => u.resume(), 'media-btn-primary'));
        } else if (p === 'no_room' || p === 'server_error') {
          buttons.push(button('retry', 'Retry', () => u.resume(), 'media-btn-primary'));
        }
        if (p === 'failed') {
          // Close ends it on the server too: nothing is left holding part of the person's share.
          buttons.push(button('close', 'Close', () => { u.cancel(); dropUpload(f); }));
        } else if (p !== 'done' && p !== 'cancelling' && p !== 'cancelled') {
          buttons.push(button('cancel', 'Cancel', () => u.cancel()));
        }
        box.appendChild(h('div', { className: 'media-form-buttons' }, buttons));
        return;
      }
      const put = f.mode === 'put';
      const go = h('button', {
        type: 'button', id: listed ? undefined : 'media-form-go', dataset: listed ? { uploadAction: 'go' } : undefined,
        className: 'media-btn media-btn-primary', disabled: f.busy || !!f.checking,
        text: f.checking ? 'Working out your share...' : f.busy ? 'Working...' : put || f.uploadedName ? 'Put on Media' : 'Upload and add',
      });
      go.addEventListener('click', () => {
        // Keep what was typed for a second try.
        if (f.inputs) {
          f.title = f.inputs.title.value;
          f.category = f.inputs.category.value;
          f.description = f.inputs.description.value;
        }
        if (f.ticks) f.labels = f.ticks.filter((t) => t.checked).map((t) => t.value);
        f.visibility = put ? 'public' : membersOnlyServer() ? 'members' : f.visibilityEls && f.visibilityEls.mem.checked ? 'members' : 'public';
        submitForm(f);
      });
      // A form in the list whose publish failed: Close leaves its upload waiting (listed below with
      // Publish and Remove); the form not yet uploading just closes.
      const close = listed ? button('close', 'Close', () => dropUpload(f)) : button('cancel', 'Cancel', closeForm);
      box.appendChild(h('div', { className: 'media-form-buttons' }, [go, close]));
    }

    function submitForm(f) {
      if (!f || f.busy || f.stop || f.checking || f.up) return;
      f.error = '';
      if (f.mode === 'add' && !f.uploadedName) { startUpload(f); return; }
      publish(f);
    }

    /** Take a form out of the uploads list (its upload ended, or it was published or closed). */
    function dropUpload(f) {
      st.uploads = st.uploads.filter((u) => u !== f);
      if (f.panel) f.panel.remove();
      renderWaiting();
    }

    /** Send the file in pieces (src/relay/resumable.rs), then publish it with its own form. */
    function startUpload(f) {
      if (!token()) {
        f.error = MEDIA_WORDS.notSignedInUpload;
        renderFormActions(f);
        return;
      }
      // The form moves into the uploads list; the form area is free for another at once.
      f.n = ++uploadSeq;
      if (st.form === f) st.form = null;
      st.uploads.push(f);
      if (f.panel) {
        suffixIds(f.panel, '-' + f.n);
        f.panel.dataset.upload = String(f.n);
        els.uploads.appendChild(f.panel);
      }
      renderForm();
      f.up = createUploader({
        fetch: env.fetch, token, file: f.file, owner: myKey, storage: env.storage, fp: f.fp,
        sleep: env.sleep, now, digest: env.digest, AbortController: Abort,
        onChange: (s) => {
          if (s.phase === 'cancelled') {
            // Deleted on the server now (Cancel, or Close on a failed one), a file its finish had
            // stored with it (round 4's cancel answer): the page says what the server answered,
            // and the share is asked again.
            if (st.uploads.includes(f)) dropUpload(f);
            sayCancelled(s.cancelAnswer, s.error);
            loadMine();
            return;
          }
          if (st.uploads.includes(f)) renderFormActions(f);
        },
        onDone: (answer) => {
          f.uploadedName = storedName((answer && (answer.filename || answer.url)) || '');
          loadMine();
          publish(f);
        },
      });
      renderFormActions(f);
      f.up.start();
    }

    async function publish(f) {
      if (f.busy) return;
      f.busy = true;
      renderFormActions(f);
      try {
        const filename = f.mode === 'put' ? f.filename : f.uploadedName;
        const auth = await env.sign(purposes.publish(filename));
        if (!auth) throw new Error(MEDIA_WORDS.cannotSign);
        const body = Object.assign({
          filename,
          title: (f.title || '').trim(),
          description: (f.description || '').trim(),
          labels: f.labels || [],
          visibility: f.visibility || 'public',
        }, f.category ? { category_id: Number(f.category) } : {}, auth);
        await answerOf(await env.fetch('/api/media', jsonPost(body)));
        if (st.form === f) closeForm();
        if (st.uploads.includes(f)) dropUpload(f);
        say(MEDIA_WORDS.published);
        load(true);
        loadMine();
        return;
      } catch (e) {
        f.error = (e && e.message ? e.message : String(e)) + (f.uploadedName && f.mode !== 'put' ? MEDIA_WORDS.uploadWaits : '');
        f.up = null;
        loadMine();
      }
      f.busy = false;
      if (st.form === f || st.uploads.includes(f)) renderFormActions(f);
    }

    // ── Report a Media item (context `media`, evidence {kind: "media", id}; /shared/report.js) ──
    let reasonsCache = null;
    let helpCache = null;
    async function loadReasons() {
      if (reasonsCache && reasonsCache.length) return reasonsCache;
      try {
        const res = await env.fetch(R.REPORT_REASONS_URL, { cache: 'no-cache' });
        const list = res && res.ok ? R.reportReasonsFrom(await res.json()) : [];
        if (list.length) reasonsCache = list;
        return list;
      } catch (e) {
        return [];
      }
    }
    async function loadOutsideHelp() {
      if (helpCache) return helpCache;
      try {
        const res = await env.fetch(R.OUTSIDE_HELP_URL, { cache: 'no-cache' });
        helpCache = res && res.ok ? R.outsideHelpFrom(await res.json()) : null;
      } catch (e) {
        helpCache = null;
      }
      return helpCache;
    }
    function savedCountry() {
      try { return env.storage ? env.storage.getItem(R.OUTSIDE_HELP_SAVED_KEY) : null; } catch (e) { return null; }
    }
    function saveCountry(code) {
      try { if (env.storage) env.storage.setItem(R.OUTSIDE_HELP_SAVED_KEY, code); } catch (e) { /* storage refused: the pick lasts this dialog */ }
    }

    function openReport(item) {
      if (!R || !myKey) return;
      const r = { item, reason: '', note: '', reasons: null, help: null, country: null, sending: false, error: '', timer: null };
      st.report = r;
      renderReport();
      loadOutsideHelp().then((help) => {
        if (st.report !== r || !help) return;
        r.help = help;
        r.country = R.outsideHelpFirstCountry(help, savedCountry(), env.language || '');
        renderReport();
      });
      loadReasons().then((list) => {
        if (st.report !== r) return;
        r.reasons = list;
        renderReport();
      });
    }
    function closeReport() {
      const r = st.report;
      if (r && r.timer) cancelLater(r.timer);
      st.report = null;
      renderReport();
    }
    /** The outside help block (10e-ii) for the danger reasons, built with createElement. */
    function outsideHelpBlock(r) {
      const v = R.outsideHelpView(r.help, r.reason, r.country);
      if (!v) return null;
      const select = h('select', { className: 'media-select', id: 'media-report-country', attrs: { 'aria-label': 'Country' } },
        v.choices.map((c) => h('option', { value: c.code, text: c.name, selected: c.code === v.code })));
      select.addEventListener('change', () => { r.country = select.value; saveCountry(select.value); renderReport(); });
      const kids = [
        h('strong', { text: v.title }),
        h('label', { className: 'media-field' }, ['Country', select]),
        v.emergency ? h('p', { className: 'media-report-emergency', text: 'Emergency: ' + v.emergency }) : h('p', { className: 'media-report-emergency', text: v.emergencyText }),
      ];
      for (const a of v.also) kids.push(h('p', { className: 'media-hint', text: a.number + (a.for ? ': ' + a.for : '') }));
      if (v.child) {
        kids.push(h('p', { className: v.childSmall ? 'media-hint' : 'media-report-child' }, [
          v.childLead + ' ',
          h('a', { href: v.child.url, text: v.child.name, target: '_blank', rel: 'noopener noreferrer', title: v.child.title || undefined }),
        ]));
      }
      if (v.note) kids.push(h('p', { className: 'media-hint', text: v.note }));
      if (v.dateLine) kids.push(h('p', { className: 'media-hint', text: v.dateLine }));
      return h('div', { className: 'media-report-help', id: 'media-report-outside-help' }, kids);
    }
    function renderReport() {
      els.report.replaceChildren();
      const r = st.report;
      if (!r) return;
      const name = r.item.uploader_name || 'The person who put it there';
      const kids = [
        h('button', { type: 'button', id: 'media-report-close', className: 'media-btn media-viewer-close', text: 'Close', on: { click: closeReport } }),
        h('h2', { className: 'media-viewer-title', text: 'Report "' + itemName(r.item, st.kinds) + '"' }),
        h('p', { className: 'media-hint', id: 'media-report-lead', text: "This goes to this server's admins and moderators. " + name + MEDIA_WORDS.reportLeadTail }),
        h('p', { className: 'media-hint', text: MEDIA_WORDS.reportWhat }),
        h('h3', { text: 'What happened' }),
      ];
      if (r.reasons === null) {
        kids.push(h('p', { className: 'media-hint', text: 'Loading the reasons...' }));
      } else if (!r.reasons.length) {
        kids.push(h('p', { className: 'media-warn', id: 'media-report-no-reasons', text: MEDIA_WORDS.reportReasonsFailed }));
      } else {
        const box = h('div', { className: 'media-field', id: 'media-report-reasons', attrs: { role: 'radiogroup', 'aria-label': 'What happened' } });
        for (const reason of r.reasons) {
          const radio = h('input', { type: 'radio', name: 'media-report-reason', value: reason.id, dataset: { reportReason: reason.id }, checked: reason.id === r.reason });
          radio.addEventListener('change', () => { if (radio.checked) { r.reason = reason.id; r.error = ''; renderReport(); } });
          box.appendChild(h('label', { className: 'media-check' }, [radio, ' ' + reason.label]));
        }
        kids.push(box);
        const chosen = r.reasons.find((x) => x.id === r.reason);
        if (chosen && chosen.help) kids.push(h('p', { className: 'media-report-reason-help', id: 'media-report-reason-help', text: chosen.help }));
        if (chosen && r.help) {
          const block = outsideHelpBlock(r);
          if (block) kids.push(block);
        }
      }
      const note = h('textarea', { id: 'media-report-note', className: 'media-input', maxLength: R.REPORT_NOTE_MAX, rows: 3, value: r.note, placeholder: 'What the admins should know', attrs: { 'aria-label': 'A note for the admins' } });
      note.addEventListener('input', () => { r.note = note.value; });
      kids.push(h('label', { className: 'media-field' }, ['Anything to add (optional)', note]));
      if (r.error) kids.push(h('p', { className: 'media-warn', id: 'media-report-error', attrs: { role: 'alert' }, text: r.error }));
      const send = h('button', {
        type: 'button', id: 'media-report-send', className: 'media-btn media-btn-danger', text: r.sending ? 'Sending...' : 'Send report',
        disabled: r.sending || !r.reason || !(r.reasons && r.reasons.length),
      });
      send.addEventListener('click', sendReport);
      kids.push(h('div', { className: 'media-form-buttons' }, [
        send,
        h('button', { type: 'button', id: 'media-report-cancel', className: 'media-btn', text: 'Cancel', on: { click: closeReport } }),
      ]));
      const overlay = h('div', { id: 'media-report-open', className: 'media-viewer', attrs: { role: 'dialog', 'aria-label': 'Report' } }, [
        h('div', { className: 'media-viewer-box media-report-box' }, kids),
      ]);
      overlay.addEventListener('click', (e) => { if (e.target === overlay) closeReport(); });
      els.report.appendChild(overlay);
    }
    async function sendReport() {
      const r = st.report;
      if (!r || r.sending) return;
      const failWith = (words) => { r.sending = false; r.error = words; if (st.report === r) renderReport(); };
      if (!r.reason) return failWith('Choose a reason first.');
      if (!bound() || !session) return failWith(MEDIA_WORDS.reportNotSignedIn);
      r.sending = true;
      r.error = '';
      renderReport();
      const evidence = R.mediaEvidenceItem(r.item.id);
      const built = await R.buildReportFrame({
        reporter: myKey, target: r.item.uploader_key, context: 'media', reason: r.reason, note: r.note,
        evidence: evidence ? [evidence] : [], ts: now(),
      }, { blake3: env.blake3, sign: env.signBytes });
      if (st.report !== r) return;
      if (!built || !built.frame) return failWith((built && built.error) || 'Your report could not be built.');
      if (!session.send(built.frame)) return failWith(MEDIA_WORDS.reportNotSignedIn);
      r.timer = later(() => { if (st.report === r && r.sending) failWith(MEDIA_WORDS.reportNoAnswer); }, ANSWER_WAIT_MS);
    }
    /** The relay's answer to a report: `report_received`, or a refusal as a notice. True when it was one. */
    function reportAnswer(m) {
      const r = st.report;
      if (!r || !r.sending || !m) return false;
      if (m.type === 'report_received') {
        closeReport();
        say(R.REPORT_RECEIVED_LINE);
        return true;
      }
      if (m.type === 'system' && typeof m.message === 'string' && !isOtherNotice(m.message)) {
        if (r.timer) cancelLater(r.timer);
        r.timer = null;
        r.sending = false;
        r.error = m.message;
        renderReport();
        return true;
      }
      return false;
    }
    /** Notices the server sends for other things: never read as the answer to a report. */
    function isOtherNotice(text) {
      return text.startsWith('Your category suggestion') || text.startsWith('Server settings updated') || text.startsWith('__game__:');
    }

    // ── Suggest a category ──
    function openSuggest() {
      st.suggest = { busy: false, error: '', mine: null };
      renderSuggest();
      loadMySuggestions();
    }
    async function loadMySuggestions() {
      const auth = await env.sign(purposes.suggestions());
      if (!auth || !st.suggest) return;
      try {
        const a = await answerOf(await env.fetch('/api/media/suggestions', jsonPost(auth)));
        if (!st.suggest) return;
        st.suggest.mine = (a.suggestions || []).filter((s) => s.suggester_key === myKey);
      } catch (e) {
        if (st.suggest) st.suggest.mine = [];
      }
      renderSuggest();
    }
    function renderSuggest() {
      els.suggest.replaceChildren();
      const s = st.suggest;
      if (!s) return;
      const name = h('input', { type: 'text', id: 'media-suggest-name', className: 'media-input', maxLength: 60, value: s.name || '' });
      const note = h('textarea', { id: 'media-suggest-note', className: 'media-input', maxLength: 500, rows: 2, value: s.note || '' });
      const go = h('button', { type: 'button', id: 'media-suggest-go', className: 'media-btn media-btn-primary', text: s.busy ? 'Sending...' : 'Send suggestion', disabled: s.busy });
      go.addEventListener('click', () => { s.name = name.value; s.note = note.value; sendSuggestion(); });
      const kids = [
        h('h2', { text: 'Suggest a category' }),
        h('p', { className: 'media-hint', text: "This server's admins choose its categories. Suggest one, and they accept or decline it." }),
        h('label', { className: 'media-field' }, ['Name', name]),
        h('label', { className: 'media-field' }, ['Note (optional)', note]),
      ];
      if (s.error) kids.push(h('p', { className: 'media-warn', id: 'media-suggest-error', text: s.error }));
      kids.push(h('div', { className: 'media-form-buttons' }, [
        go,
        h('button', { type: 'button', className: 'media-btn', text: 'Close', on: { click: () => { st.suggest = null; renderSuggest(); } } }),
      ]));
      if (s.mine && s.mine.length) {
        const words = { open: 'waiting for an admin', accepted: 'accepted', declined: 'declined' };
        kids.push(h('h3', { text: 'Your suggestions' }));
        kids.push(h('ul', { className: 'media-list', id: 'media-suggest-mine' },
          s.mine.map((m) => h('li', { text: m.name + ': ' + (words[m.state] || m.state) }))));
      }
      els.suggest.appendChild(h('div', { className: 'media-panel media-form' }, kids));
    }
    async function sendSuggestion() {
      const s = st.suggest;
      if (!s || s.busy) return;
      s.busy = true;
      s.error = '';
      renderSuggest();
      try {
        const auth = await env.sign(purposes.suggest());
        if (!auth) throw new Error(MEDIA_WORDS.cannotSign);
        await answerOf(await env.fetch('/api/media/suggest', jsonPost(Object.assign({ name: (s.name || '').trim(), note: (s.note || '').trim() }, auth))));
        st.suggest = null;
        renderSuggest();
        say(MEDIA_WORDS.suggestSent);
        return;
      } catch (e) {
        s.error = e.message;
      }
      s.busy = false;
      renderSuggest();
    }

    // ── Admins ──
    function renderAdmin() {
      if (st.adminMounted || !isAdmin() || typeof env.mountAdmin !== 'function') return;
      st.adminMounted = true;
      const body = h('div', { id: 'media-admin-body', className: 'media-admin-body' });
      els.admin.appendChild(h('details', { className: 'media-panel media-admin', id: 'media-admin' }, [
        h('summary', { text: 'Media settings, categories and space (admins)' }),
        body,
      ]));
      env.mountAdmin(body);
    }

    function renderAll() {
      renderStatus();
      renderNotice();
      renderMembers();
      renderActions();
      renderFilters();
      renderFeatured();
      renderGrid();
      renderViewerIfChanged();
      renderWaiting();
      if (st.form && !st.form.busy && !st.form.up && st.form.key !== formKey()) renderForm();
      renderAdmin();
      sweepBlobs();
      pumpBlobs();
    }

    // ── The session's news ──
    function onSession(ev) {
      if (ev.type === 'state') {
        const was = st.lastStatus;
        st.lastStatus = session.state.status;
        // Signed in now: signing in makes the person a member of this server, so a list
        // read before that (or still on its way) may have missed the members' items.
        if (session.state.status === 'bound' && was !== 'bound' && (!st.answered || !st.membersView)) {
          load(true).then(() => loadMine());
          loadFeatured();
        } else {
          renderAll();
        }
        return;
      }
      const m = ev.msg;
      if (reportAnswer(m)) return;
      // The answer to a category suggestion comes as a notice (the relay's words).
      if (m && m.type === 'system' && typeof m.message === 'string' && m.message.startsWith('Your category suggestion')) {
        say(m.message);
        if (st.suggest) loadMySuggestions();
      }
    }

    /** /media?id=<n>: the item by its number (the admins' Reports view opens one this way). */
    async function openById(id) {
      try {
        const a = await answerOf(await fetchList(listParams({ id }, null, 1)));
        const item = Array.isArray(a.items) ? a.items.find((i) => String(i.id) === String(id)) : null;
        if (!item) { say(MEDIA_WORDS.notFound, true); return; }
        takePeople([item]);
        if (Array.isArray(a.labels) && a.labels.length) st.labels = a.labels;
        openViewer(item);
      } catch (e) {
        say(MEDIA_WORDS.notFound + ' ' + (e && e.message ? e.message : ''), true);
      }
    }

    async function start() {
      build();
      if (session) {
        session.subscribe(onSession);
        if (myKey) session.start();
      }
      renderAll();
      await loadKinds();
      loadFeatured();
      await load(true);
      loadMine();
      // Put on Media (a Shared Files row's link) once the categories and labels are known.
      if (env.put && myKey) openPutForm(env.put);
      if (env.openId) await openById(env.openId);
    }

    /** Revoke every object URL (the page is going away). */
    function dispose() {
      for (const id of Array.from(blobs.keys())) dropBlob(id);
      tickets.clear();
    }

    return {
      start, state: st, load, loadFeatured, loadMine, openAddForm, openPutForm, removeItem: remove, pickItem: pick,
      featureItem: feature, keepItem: keep, openReport, openViewer, closeViewer, dispose, blobs, tickets,
    };
  }

  // ── In the browser ──
  function bootInBrowser() {
    const doc = root.document;
    const rootEl = doc && doc.getElementById('media-root');
    if (!rootEl) return;
    const params = new URLSearchParams(root.location.search || '');
    const put = params.get('put') ? { filename: params.get('put'), name: params.get('name') || '' } : null;
    const openId = /^\d+$/.test(params.get('id') || '') ? Number(params.get('id')) : null;
    const go = async () => {
      let identity = null;
      try { identity = typeof root.getPqIdentity === 'function' ? await root.getPqIdentity() : null; } catch (e) { identity = null; }
      const session = identity && typeof root.createRelaySession === 'function'
        ? root.createRelaySession({
          getIdentity: async () => identity,
          signBytes: (secret, bytes) => root.pqSignMessage(secret, bytes),
          WebSocket: root.WebSocket,
          location: root.location,
          storage: root.localStorage,
        })
        : null;
      const sign = (purpose) => (typeof root.getPqSignedAuth === 'function' ? root.getPqSignedAuth(purpose) : Promise.resolve(null));
      const confirm = (words) => (typeof root.holdConfirm === 'function'
        ? root.holdConfirm(words, { seconds: 2, confirmLabel: 'Hold to remove' })
        : Promise.resolve(root.confirm(words)));
      let storage = null;
      try { storage = root.localStorage; } catch (e) { storage = null; }
      let page = null;
      page = createMediaPage({
        document: doc,
        root: rootEl,
        fetch: (u, i) => root.fetch(u, i),
        origin: root.location.origin,
        myKey: identity ? identity.dilithiumPublicHex : '',
        sign,
        signBytes: (bytes) => (identity && typeof root.pqSignMessage === 'function' ? root.pqSignMessage(identity.dilithiumSecret, bytes) : Promise.resolve(null)),
        blake3: (bytes) => root.pqBlake3(bytes),
        session,
        confirm,
        URL: root.URL,
        storage,
        sleep: (ms) => new Promise((r) => root.setTimeout(r, ms)),
        report: typeof root.buildReportFrame === 'function' ? root : null,
        language: (root.navigator && root.navigator.language) || '',
        put,
        openId,
        retryIdentity: () => root.location.reload(),
        // The admins' section (/pages/media-admin.js, loaded after this file).
        mountAdmin: (container) => {
          if (typeof root.mountMediaAdmin !== 'function') return;
          root.mountMediaAdmin(container, {
            document: doc, fetch: (u, i) => root.fetch(u, i), sign, confirm, session,
            blake3: (bytes) => root.pqBlake3(bytes),
            onChange: () => { if (page) { page.load(true); page.loadFeatured(); } },
          });
        },
      });
      root.addEventListener('pagehide', () => page.dispose());
      page.start();
    };
    if (doc.readyState === 'loading') doc.addEventListener('DOMContentLoaded', go);
    else go();
  }

  const api = {
    MEDIA_WORDS, PAGE_SIZE, FEATURED_LIMIT, KINDS_URL, RESUME_KEY, parseKinds, kindWords, sizeWords, shareLeftWords,
    shareStop, limitStop, budgetStop, meterWords, keptMeterWords, removeWords, removedWords, resetsWords, ownFilePath,
    fileRoute, storedName, itemFileName, itemName, downloadName, labelsOf, listParams, publicListUrl, purposes, jsonPost,
    refusalOf, refusalWords, answerOf, makeH, isAdminRole, isStaffRole, rememberUpload, findUpload, forgetUpload,
    createUploader, fingerprint, createMediaPage,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else {
    Object.assign(root, { MediaPage: api });
    bootInBrowser();
  }
})(typeof globalThis !== 'undefined' ? globalThis : this);
