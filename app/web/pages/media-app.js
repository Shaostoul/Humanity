// ── media-app.js ──────────────────────────────────────────────────────────
// The Media page on the website (2026-10-11; docs/design/media-showcase.md,
// "Increment 2, exact", then round 2 against the relay's follow-ups: "Increment
// 2 follow-ups, exact", its As built note, and "Resumable uploads, exact"). The
// desktop app's Media page is the original (src/gui/pages/media.rs); this page
// mirrors its layout and words.
//
// What it shows (round 5, docs/design/media-round-5-plan.md 3.10): "Media on
// <host>", with Media and Admin tabs for admins (/media#admin opens Admin); the
// featured shelf (the main one, or the picked category's, `featured=1&category=`);
// filters (category with its description under them, kind, person, My picks,
// Sort, a search over titles); a grid paged by the answer's cursor, the next page
// asked for once as its end comes into view, with More for keyboards; each card
// with its cover, its description's first line and its file count; the item's own
// page at its address, /media/<id> (/pages/media-item.js since round 5's lane 2A:
// its files with counted Downloads, the counts, the licence, Copy link, and Edit,
// /pages/media-edit.js); and the person's own uploads waiting to be
// published. What it does: Pick and Unpick, Remove
// (with the words naming what happens to the file), Feature (main shelf, its
// category's, or none) for admins, Keep on the server and Stop keeping for admins and moderators, Report,
// Add to Media (the file picker, a short form with the licence and the hosting
// paragraph, then a resumable upload with a progress bar, Pause and Resume; a file
// picked in an item's Edit form is the same upload, attached to that item when it
// is up instead of published), Put on Media for a Shared Files row (public
// only), and Suggest a category. "Report a copyright problem" (2026-10-13,
// /pages/media-takedown.js) is on every item's page and at the page's foot, for
// anyone, signed in or not, and the reports waiting are on the Admin tab. The
// relay half is src/relay/media.rs, src/relay/resumable.rs and
// src/relay/media_takedown.rs.
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
//   (/media/<n> included): its file is not even requested before then.
// - Uploads in progress are a list of their own (each with its bar, Pause,
//   Resume and Cancel), each published by its own form, so opening another
//   form never leaves one behind (round 3, review 2). Uploads the server still
//   holds part of (`uploads_in_progress`, R3-7) are listed with Cancel. A
//   cancel is asked again while the server is busy, and the server deletes a
//   file the finish had stored with it, so nothing is removed by hand; the
//   page says what the server answered (round 4, W4-3).
// - Every write is signed (Dilithium over "purpose\ntimestamp", getPqSignedAuth
//   in /shared/pq-relay-auth.js, which never signs two requests with the same
//   timestamp) with a purpose that names its target:
//   media_publish:<file name>:<licence id>:<0|1> (round 5: the licence and the
//   game tick are signed with it), media_remove:<item id or file name>,
//   media_pick:<item id>, media_feature:<item id>:<main|category|none> (round
//   5), media_keep:<item id>:<on|off>. A write on an item
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
  // How far below the screen the grid's end may still be when its next page is asked for.
  const SENTINEL_MARGIN = '600px';
  // The relay's sorts (3.3; newest first when none is sent) and where an admin may feature an
  // item (3.4), each with the words both apps show.
  const SORTS = [['new', 'Newest'], ['picks', 'Most picked'], ['downloads', 'Most downloaded']];
  const FEATURE_CHOICES = [['none', 'Not featured'], ['main', 'Featured'], ['category', 'Featured in its category']];

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
    checkerMissing: 'This page could not load its file checker, so nothing can be added. Reload the page and try again.',
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

  /** Several files going up together (2026-10-16): the stop when all of them together are more
   *  than what is left of the person's share or of the server's Media space, else null. Each file
   *  on its own is checked as one is. The desktop app's words (api.rs room_problem_together). */
  function togetherStop(files, usedBytes, shareBytes, budgetLeft) {
    if (!files || files.length < 2) return null;
    const total = files.reduce((n, f) => n + ((f && f.size) || 0), 0);
    const n = files.length;
    if (Number.isFinite(shareBytes) && shareBytes > 0 && Number.isFinite(usedBytes)) {
      const left = Math.max(0, shareBytes - usedBytes);
      if (total > left) {
        return 'These ' + n + ' files come to ' + sizeWords(total) + ', more than the ' + sizeWords(left) +
          ' left of your share of this server\'s Media space, so none was uploaded. Choose fewer, remove one of your items, or ask an admin to raise each person\'s share.';
      }
    }
    if (Number.isFinite(budgetLeft) && budgetLeft >= 0 && total > budgetLeft) {
      return 'These ' + n + ' files come to ' + sizeWords(total) + ', more than the ' + sizeWords(budgetLeft) +
        ' left in this server\'s whole Media space, so none was uploaded. Choose fewer, or its admins can make more room.';
    }
    return null;
  }

  /** What the Add to Media form says about the further files chosen with the first. */
  function moreWords(more) {
    return 'Also on this item, each added once the item is up: ' + more.map((m) => m.name + ' (' + sizeWords(m.size) + ')').join(', ');
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
    // Round 5: a members-only item's other files (and its cover) are `?file=<n>` of its route.
    const n = u.searchParams.get('file');
    if (/^\/api\/media\/file\/\d+$/.test(u.pathname)) return u.pathname + (n && /^\d{1,6}$/.test(n) && n !== '0' ? '?file=' + n : '');
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

  /**
   * Whether an item is on a featured shelf (round 5: the relay sends "main", "category" or
   * "none"; it was a bool).
   */
  function isFeatured(item) {
    return !!item && (item.featured === 'main' || item.featured === 'category');
  }

  /** The item an address names: /media/<n> is item n (3.6, the shape nginx sends to the relay); else null. */
  function itemIdOf(path) {
    const m = /^\/media\/(\d{1,15})$/.exec(String(path || ''));
    return m ? Number(m[1]) : null;
  }

  /** The licence an item gets when none is chosen (the relay's DEFAULT_LICENCE). */
  const DEFAULT_LICENCE = 'all_rights_reserved';

  // ── Requests ──

  /**
   * The list's filters as the relay takes them, without the empty ones. `cursor` is the last
   * answer's `next_cursor` (round 5; it replaced `before`), opaque here; `filters.sort` is
   * "picks" or "downloads" (newest first when left out).
   */
  function listParams(filters, cursor, limit) {
    const f = filters || {};
    const p = {};
    if (f.category) p.category = Number(f.category);
    if (f.kind) p.kind = f.kind;
    if (f.by) p.by = f.by;
    if (f.q && f.q.trim()) p.q = f.q.trim();
    if (f.featured) p.featured = 1;
    if (f.id) p.id = Number(f.id);
    if (f.sort && f.sort !== 'new') p.sort = String(f.sort);
    if (cursor != null) p.cursor = String(cursor);
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
    publish: (filename, licence, forGame) => 'media_publish:' + filename + ':' + (licence || DEFAULT_LICENCE) + ':' + (forGame ? 1 : 0),
    remove: (target) => 'media_remove:' + target,
    pick: (id) => 'media_pick:' + id,
    feature: (id, to) => 'media_feature:' + id + ':' + to,
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

  // Every file cleaned on this device before it goes up (2026-10-11, docs/design/media-showcase.md,
  // "Files on the sender's device and in messages, exact", parts 1 and 2): /shared/upload-clean.js.
  const C = typeof module === 'object' && module && module.exports ? require('../shared/upload-clean.js') : root.HosUploadClean;

  // An item's own page (/pages/media-item.js) and its Edit form and the licence picker
  // (/pages/media-edit.js), round 5, lane 2A.
  const MI = typeof module === 'object' && module && module.exports ? require('./media-item.js') : root.MediaItem;
  const ED = typeof module === 'object' && module && module.exports ? require('./media-edit.js') : root.MediaEdit;
  // "Report a copyright problem" (2026-10-13, /pages/media-takedown.js): the form anyone may send,
  // signed in or not, from an item's page and from the page's foot; and the admins' view of them.
  const TD = typeof module === 'object' && module && module.exports ? require('./media-takedown.js') : root.MediaTakedown;

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
   *   path and hash (the address's: /media/<n> opens item n, #admin the Admin tab),
   *   history (pushState, replaceState, back: an opened item's address; optional),
   *   IntersectionObserver (loads the next page as the grid's end comes into view; optional),
   *   mountAdmin(container) (the admins' Media section, /pages/media-admin.js, on the Admin tab), optional,
   *   mountTakedowns(container) (the copyright reports waiting, /pages/media-takedown.js, on the
   *   Admin tab below it), optional,
   *   clean (/shared/upload-clean.js, which cleans each file before it goes up; that module when
   *   not given, and null for no cleaning at all: the older tests' stand-in files are zeros named
   *   as videos, which the cleaning would rightly refuse).
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
    // The cleaning (part 1): false when the page could not load it, so nothing goes up unchecked.
    const cleaner = env.clean !== undefined ? env.clean : (C || false);
    // The heading and the tab's title name the server (3.10): "Media on united-humanity.us".
    let host = '';
    try { host = new URL(env.origin).host; } catch (e) { host = ''; }
    const mediaOn = 'Media on ' + host;
    const hist = env.history || null;
    let watcher = null; // the IntersectionObserver on the sentinel after the grid

    const st = {
      items: [],
      featured: [],
      nextCursor: null,          // the last answer's next_cursor (round 5)
      licences: null,            // the server's licences answer (round 5)
      answered: false,          // a list answer has come back
      membersView: false,
      membersOnlyForYou: false,
      mediaVisibility: 'public',
      categories: [],
      labels: [],
      kinds: [],                // data/media/kinds.json
      seenKinds: [],            // the kinds the items carry, used when the kinds file could not be read
      people: new Map(),        // uploader key -> name, from the answers
      filters: { category: '', kind: '', by: '', q: '', picks: false, sort: 'new' },
      shelfOf: '',              // the category whose shelf st.featured is ('' the main shelf)
      tab: env.hash === '#admin' ? 'admin' : 'media', // shown only to admins
      pushed: false,            // the open item's address was pushed here, so closing goes Back
      revealed: new Set(),      // labelled items the person clicked to show
      loading: false,
      listError: '',
      notice: '',
      noticeWarn: false,
      viewer: null,
      viewerKey: '',
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
    const fileInput = h('input', { id: 'media-file-input', type: 'file', hidden: true, multiple: true, className: 'media-file-input' });
    fileInput.addEventListener('change', () => {
      // Several at once (2026-10-16): one item holds them all; the first is its own file.
      const files = Array.from(fileInput.files || []);
      if (files[0]) openAddForm(files[0], files.slice(1));
      try { fileInput.value = ''; } catch (e) { /* some browsers refuse; harmless */ }
    });
    function build() {
      doc.addEventListener('keydown', (e) => {
        if (e.key === 'Escape' && takedown && takedown.isOpen()) { takedown.close(); return; }
        if (e.key === 'Escape' && st.report) { closeReport(); return; }
        if (e.key === 'Escape' && st.viewer) itemView.close();
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
      els.catDesc = h('p', { id: 'media-category-desc', className: 'media-hint media-category-desc', hidden: true });
      els.grid = h('div', { id: 'media-grid', className: 'media-grid' });
      els.more = h('div', { id: 'media-more-wrap', className: 'media-more-wrap' });
      // Comes into view just after the grid's end: the next page is asked for then (3.10).
      els.sentinel = h('div', { id: 'media-sentinel', className: 'media-sentinel', attrs: { 'aria-hidden': 'true' } });
      els.viewer = h('div', { id: 'media-viewer', className: 'media-viewer-wrap' });
      els.report = h('div', { id: 'media-report', className: 'media-viewer-wrap' });
      // The copyright form's dialog, and the page's foot with its button and the public log's link.
      els.takedown = h('div', { id: 'media-takedown', className: 'media-viewer-wrap' });
      els.foot = h('p', { id: 'media-foot', className: 'media-hint media-foot' }, TD ? [
        h('button', {
          type: 'button', id: 'media-foot-takedown', className: 'media-btn media-btn-small', text: TD.WORDS.button,
          title: TD.WORDS.buttonTip, on: { click: () => openTakedown(null) },
        }),
        ' ',
        h('a', { href: TD.LOG_PAGE, id: 'media-foot-takedown-log', text: TD.WORDS.logLink }),
      ] : []);
      els.tabs = h('div', { id: 'media-tabs-wrap' });
      // The Media tab, and the Admin tab (the admins' section, mounted in renderAdmin).
      els.main = h('div', { id: 'media-main' }, [els.members, els.actions, els.form, els.uploads, els.waiting, els.suggest,
        els.featured, els.filters, els.catDesc, els.grid, els.more, els.sentinel, els.foot]);
      els.admin = h('div', { id: 'media-admin-wrap', className: 'media-admin-wrap', hidden: true });
      doc.title = mediaOn;
      env.root.replaceChildren(
        h('div', { className: 'media-head' }, [
          h('h1', { id: 'media-title', text: mediaOn }),
          h('p', { className: 'media-lede' }, [
            MEDIA_WORDS.lede + ' Files people share for download are in ',
            h('a', { href: '/shared-files', text: 'Shared Files' }),
            '.',
          ]),
        ]),
        els.status, els.notice, els.tabs, els.main, els.admin, els.viewer, els.report, els.takedown, fileInput
      );
      const Watch = env.IntersectionObserver || null;
      if (Watch) {
        watcher = new Watch((seen) => { if (seen.some((e) => e.isIntersecting)) more(); }, { rootMargin: SENTINEL_MARGIN });
        watcher.observe(els.sentinel);
      }
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
      const key = JSON.stringify([st.categories.map((c) => [c.id, c.name, c.description]), kindsOffered(), st.kinds.map((k) => k.label), peopleList, !!myKey, st.filters]);
      if (key === st.filtersKey) return;
      st.filtersKey = key;
      // The picked category's description, under the filters (3.10), as text.
      const picked = st.categories.find((c) => String(c.id) === String(st.filters.category));
      const desc = picked && typeof picked.description === 'string' ? picked.description.trim() : '';
      els.catDesc.textContent = desc;
      els.catDesc.hidden = !desc;
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
      const sort = h('select', { id: 'media-filter-sort', className: 'media-select', attrs: { 'aria-label': 'Sort' } },
        SORTS.map(([v, words]) => option(v, words, st.filters.sort)));
      sort.addEventListener('change', () => setFilter('sort', sort.value));
      const kids = [category, kind, person, sort];
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
    /** A filter changed: the list is asked for afresh (no cursor), and a category its own shelf. */
    function setFilter(name, value) {
      st.filters[name] = value;
      load(true);
      if (name === 'category') loadFeatured();
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
      // An item's cover is kept under 'c' + its id (media-item.js coverOf).
      for (const i of st.items.concat(st.featured, st.viewer ? [st.viewer] : [])) { shown.add(i.id); shown.add('c' + i.id); }
      for (const id of Array.from(st.reloadKeep || [])) shown.add('c' + id);
      for (const id of Array.from(blobs.keys())) if (!shown.has(id)) dropBlob(id);
    }
    function blobChanged() {
      renderGrid();
      renderFeatured();
      itemView.renderIfChanged();
    }

    // ── The item's own page (/pages/media-item.js): the viewer, its tickets and counted downloads ──
    const itemView = MI.createItemView({
      env, h, st, els, W: MEDIA_WORDS, myKey, mediaOn, token, isAdmin, membersOnlyServer, blobs, blobFor, sweepBlobs, pictureSource,
      lib: { fileRoute, itemName, downloadName, itemFileName, labelsOf, kindWords, sizeWords, num, answerOf, jsonPost },
      go, canPush: !!hist, back: () => hist.back(),
      rerender: () => { renderGrid(); renderFeatured(); },
      actions: (item) => tileActions(item, true),
      openTakedown: TD ? (item) => openTakedown(item) : null,
      attach: (file, item, as) => startAttach(file, item, as),
      // Several files added at once (2026-10-16): '' when all of them fit together, else why not.
      together: async (files) => {
        if (!files || files.length < 2) return '';
        const mine = await loadMine();
        if (!mine) return '';
        const budget = Number.isFinite(Number(mine.budget_left_bytes)) ? Number(mine.budget_left_bytes) : NaN;
        return togetherStop(files, num(mine.used_bytes) + num(mine.in_progress_bytes), num(mine.share_bytes), budget) || '';
      },
      uploadsFor: (id) => st.uploads.filter((u) => u.attach && u.attach.id === id).map((u) => ({ heading: u.heading, words: u.up ? uploadWords(u.up) : u.error })),
    });

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
      const all = st.items.filter((i) => i.id === id).concat(st.featured.filter((i) => i.id === id));
      // The item's page may hold its own copy (opened by its number).
      return st.viewer && st.viewer.id === id && !all.includes(st.viewer) ? all.concat([st.viewer]) : all;
    }
    /** A tile's actions; `onPage` on the item's own page, which has its own Edit. */
    function tileActions(item, onPage) {
      const row = h('div', { className: 'media-tile-actions' });
      const mine = !!myKey && item.uploader_key === myKey;
      // Edit, for the uploader and admins (round 5): it opens the item's page with its Edit form.
      if (!onPage && itemView.mayEdit(item)) {
        row.appendChild(h('button', {
          type: 'button', className: 'media-btn media-btn-small media-btn-primary', dataset: { action: 'edit' },
          text: 'Edit', title: MI.WORDS.editTip, on: { click: () => itemView.openEdit(item) },
        }));
      }
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
        // Three ways (round 5): not featured, on the main shelf, or on its category's shelf only.
        const now = isFeatured(item) ? item.featured : 'none';
        const where = h('select', { className: 'media-select media-select-small', dataset: { action: 'feature' }, attrs: { 'aria-label': 'Feature' } },
          FEATURE_CHOICES.map(([v, words]) => h('option', { value: v, text: words, selected: v === now })));
        where.addEventListener('change', () => feature(item, where.value));
        row.appendChild(where);
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
      // Left out by the protected setup (media-item.js hiddenBy, the desktop's Show::Hidden): its
      // line in the picture's place, nothing fetched, and no click through to it.
      const hidden = itemView.hiddenLine(item);
      if (hidden !== null) return h('div', { className: 'media-thumb media-thumb-missing media-thumb-hidden', dataset: { hidden: item.id }, text: hidden });
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
      const open = () => itemView.open(item);
      // The cover first (round 5), else the picture itself.
      const pic = MI.coverOf(item) || (item.kind === 'picture' ? item : null);
      if (pic) {
        const where = pictureSource(pic);
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
      const downloads = Number(item.downloads) || 0;
      // The description's first line, cut at a word (the item's page has the rest), and "3 files".
      const line = MI.firstLine(item.description);
      const files = MI.filesBadge(item);
      return h('div', { className: 'media-tile', dataset: { id: item.id } }, [
        thumb(item),
        badges.childNodes.length ? badges : null,
        h('div', { className: 'media-tile-title', text: itemName(item, st.kinds) }),
        line ? h('div', { className: 'media-tile-line', text: line }) : null,
        h('div', { className: 'media-tile-meta', text: 'by ' + (item.uploader_name || 'someone') + ' · ' + picks + (picks === 1 ? ' pick' : ' picks') +
          (downloads ? ', ' + downloads + (downloads === 1 ? ' download' : ' downloads') : '') + (files ? ' · ' + files : '') }),
        tileActions(item),
      ]);
    }

    function renderFeatured() {
      els.featured.replaceChildren();
      if (!st.featured.length) return;
      // The main shelf, or the shelf of the category it was asked for (3.9: "Featured in Fire flow arts").
      const cat = st.shelfOf ? st.categories.find((c) => String(c.id) === String(st.shelfOf)) : null;
      els.featured.appendChild(h('h2', { id: 'media-featured-title', text: cat ? 'Featured in ' + cat.name : 'Featured' }));
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
      if (st.nextCursor != null && !st.listError) {
        els.more.appendChild(h('button', {
          type: 'button', id: 'media-more', className: 'media-btn', text: st.loading ? MEDIA_WORDS.loading : 'More',
          disabled: st.loading, on: { click: more },
        }));
      }
    }
    // The next page, from More or the sentinel coming into view, asked for once: nothing more is
    // asked while a page is on its way, so a sentinel reporting again never asks for it twice.
    function more() {
      if (st.loading || st.nextCursor == null || st.listError) return;
      load(false);
    }

    // ── The item's address (3.10): an opened item is /media/<id> ──
    /** Put `path` in the address bar: as a new step (Back undoes it) or in place of this one. */
    function go(path, push) {
      if (!hist) return;
      try { if (push) hist.pushState(null, '', path); else hist.replaceState(null, '', path); } catch (e) { /* refused (a sandboxed frame): the page works without */ }
    }
    /** Back or Forward changed the address: show what it names (an item or the list; #admin the Admin tab). */
    function route(path, hash) {
      st.pushed = false;
      st.tab = hash === '#admin' ? 'admin' : 'media';
      renderTabs();
      const id = itemIdOf(path);
      if (id == null) { if (st.viewer) itemView.close(true); return Promise.resolve(); }
      if (st.viewer && st.viewer.id === id) return Promise.resolve();
      // An item the list holds already opens as it is; any other is asked for by its number.
      const known = st.items.concat(st.featured).find((i) => i.id === id);
      if (known) { itemView.open(known, true); return Promise.resolve(); }
      return openById(id);
    }

    // ── Loading ──
    function takeAnswer(a) {
      const items = Array.isArray(a && a.items) ? a.items : [];
      st.items = st.items.concat(items);
      st.nextCursor = a && a.next_cursor != null ? a.next_cursor : null;
      if (a && a.licences && typeof a.licences === 'object') st.licences = a.licences;
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
        st.nextCursor = null;
      }
      st.loading = true;
      st.listError = '';
      renderGrid();
      const params = listParams(st.filters, reset ? null : st.nextCursor);
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
      // Watched afresh, the sentinel reports at once whether it is still in view (a short page),
      // so the next page comes without a scroll; it stops once the sentinel is out of reach.
      if (watcher && st.nextCursor != null) { watcher.unobserve(els.sentinel); watcher.observe(els.sentinel); }
    }

    /**
     * The shelf: every featured item (`featured=1`), not only those on the pages loaded; with a
     * category picked, that category's shelf (`featured=1&category=c`, 3.3).
     */
    async function loadFeatured() {
      const seq = ++featuredSeq;
      const shelf = st.filters.category || '';
      let featured = [];
      try {
        const a = await answerOf(await fetchList(listParams({ featured: 1, category: shelf }, null, FEATURED_LIMIT)));
        featured = Array.isArray(a.items) ? a.items : [];
      } catch (e) {
        featured = [];
      }
      if (seq !== featuredSeq) return;
      st.featured = featured;
      st.shelfOf = shelf;
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
          if (st.viewer && st.viewer.id === item.id) itemView.close();
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
      itemView.renderInfo();
    }

    async function remove(item) {
      const ok = await env.confirm('"' + itemName(item, st.kinds) + '"\n\n' + removeWords(item));
      if (!ok) return;
      const a = await act('/api/media/remove', purposes.remove(item.id), item);
      if (!a) return;
      st.items = st.items.filter((i) => i.id !== item.id);
      st.featured = st.featured.filter((i) => i.id !== item.id);
      if (st.viewer && st.viewer.id === item.id) itemView.close();
      say(removedWords(a));
      renderAll();
      loadMine();
    }

    /** Feature an item on the main shelf ("main"), its category's only ("category") or neither ("none"). */
    async function feature(item, to) {
      const a = await act('/api/media/feature', purposes.feature(item.id, to), item, { featured: to });
      if (a) {
        for (const c of copiesOf(item.id)) c.featured = to;
        say(to === 'main' ? 'Featured: it leads the Media page now.'
          : to === 'category' ? "Featured in its category: it leads that category's shelf now." : 'Taken off the featured shelves.');
        loadFeatured();
      }
      renderGrid();
      renderFeatured();
      itemView.renderInfo();
    }

    async function keep(item, on) {
      const a = await act('/api/media/keep', purposes.keep(item.id, on), item, { kept: on });
      if (a) {
        for (const c of copiesOf(item.id)) c.kept = on;
        say(on ? MEDIA_WORDS.keptNow : MEDIA_WORDS.keptStopped);
      }
      renderGrid();
      renderFeatured();
      itemView.renderInfo();
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

    /**
     * What goes up for an Add to Media form, as its switch stands (part 1): {file, keepMetadata,
     * refused}. The Media page is public, so a file that could not be checked is never added unless
     * the person keeps its details by choice; `refused` holds the words saying so.
     */
    function sendingOf(f) {
      if (!f || !f.clean || !cleaner) return { file: f ? f.file : null, keepMetadata: false, refused: null };
      const d = cleaner.decide(f.clean, { where: 'public', remove: f.remove !== false, protectedOn: !!f.protectedOn });
      if (d.action === 'send') return { file: d.file, keepMetadata: d.keepMetadata, refused: null };
      return { file: null, keepMetadata: false, refused: d.words };
    }

    /** Does the form show what the file holds and the switch: a picture or a video, or a file that could not be checked. */
    function detailsShown(f) {
      const c = f && f.clean;
      return !!(c && cleaner && f.mode === 'add' && (c.unchecked || c.summary || ['jpeg', 'png', 'webp', 'mp4', 'heif'].includes(c.kind)));
    }

    /** The form's part on what the file holds: its hidden details, the switch, and why it cannot go. */
    function detailsBox(f) {
      const W = cleaner.WORDS;
      const c = f.clean;
      const kids = [];
      if (c.summary) {
        kids.push(h('p', { id: 'media-form-hidden', className: 'media-hint', text: c.summary }));
        kids.push(h('ul', { className: 'media-hidden-details' }, (c.details || []).map((d) => h('li', { text: d.words }))));
      }
      f.detailsSwitch = null;
      if (f.protectedOn) {
        kids.push(h('p', { id: 'media-form-protected', className: 'media-hint', text: W.protectedLine }));
      } else {
        const box = h('input', { type: 'checkbox', id: 'media-form-remove-details', checked: f.remove !== false, disabled: !!f.up });
        box.addEventListener('change', () => {
          if (f.up) return;
          f.remove = !!box.checked;
          drawDetailsWords(f);
          renderFormActions(f);
        });
        f.detailsSwitch = box;
        kids.push(h('label', { className: 'media-check' }, [box, ' ' + W.switchLabel]));
      }
      f.offEl = h('p', { id: 'media-form-details-off', className: 'media-hint' });
      f.refusedEl = h('p', { id: 'media-form-unchecked', className: 'media-warn' });
      kids.push(f.offEl, f.refusedEl);
      drawDetailsWords(f);
      return h('div', { id: 'media-form-details', className: 'media-field media-details' }, kids);
    }

    function drawDetailsWords(f) {
      if (!f.offEl || !cleaner) return;
      f.offEl.textContent = !f.protectedOn && f.remove === false ? cleaner.WORDS.switchOffLine : '';
      f.refusedEl.textContent = sendingOf(f).refused || '';
    }

    async function openAddForm(file, more) {
      const f = { mode: 'add', file, more: more || [], name: file.name, stop: null, shareLine: '', checking: true, busy: false, error: '', uploadedName: '', up: null, fp: '' };
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
      // Cleaned on this device first (part 1): what it holds, for the form's words and switch.
      if (cleaner === false) {
        f.checking = false;
        f.stop = MEDIA_WORDS.checkerMissing;
        renderForm();
        return;
      }
      if (cleaner) {
        f.protectedOn = cleaner.protectedSetupOn(env.storage);
        f.remove = cleaner.removeDetailsChosen(env.storage);
        f.clean = await cleaner.cleanFile(file);
        if (st.form !== f) return;
      }
      // Its fingerprint (the first and last megabyte), which a resume is matched by: of the file that
      // goes up (the cleaned one, unless the details are kept).
      f.fpFile = sendingOf(f).file || file;
      try { f.fp = fingerprint ? await fingerprint(f.fpFile, env.digest) : ''; } catch (e) { f.fp = ''; }
      if (st.form !== f) return;
      // Until the relay has said what is left, nothing can be uploaded, so an upload over it never starts.
      const mine = await loadMine();
      if (st.form !== f) return;
      f.checking = false;
      if (mine) {
        // An upload of this same file already part way up is counted by the relay as in progress
        // already; its own bytes are not "more" on top of that.
        const known = f.fp ? findUpload(env.storage, myKey, f.fpFile, f.fp, now()) : null;
        const inProgress = Math.max(0, num(mine.in_progress_bytes) - (known ? file.size : 0));
        const used = num(mine.used_bytes) + inProgress;
        const share = num(mine.share_bytes);
        if (share) f.shareLine = shareLeftWords(used, share);
        const budget = Number.isFinite(Number(mine.budget_left_bytes)) ? Number(mine.budget_left_bytes) : NaN;
        f.stop = limitStop(file, num(mine.file_limit_bytes)) || shareStop(file, used, share) ||
          (Number.isFinite(budget) ? budgetStop(file, budget) : null) ||
          f.more.map((m) => limitStop(m, num(mine.file_limit_bytes))).find(Boolean) ||
          togetherStop([file].concat(f.more), used, share, budget);
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
      if (f.more && f.more.length) kids.push(h('p', { id: 'media-form-more', className: 'media-hint', text: moreWords(f.more) }));
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
      // What the file holds and the switch (part 1), before anything else is asked.
      if (detailsShown(f)) kids.push(detailsBox(f));
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
        const picked = () => { f.visibility = mem.checked ? 'members' : 'public'; if (f.licencePicker) f.licencePicker.refresh(); };
        pub.addEventListener('change', picked);
        mem.addEventListener('change', picked);
        kids.push(h('fieldset', { className: 'media-field', id: 'media-form-visibility' }, [
          h('legend', { text: 'Who sees it' }),
          h('label', { className: 'media-check' }, [pub, ' Public']),
          h('label', { className: 'media-check' }, [mem, ' Members only']),
        ]));
      }
      // The licence, "All rights reserved" preselected, and the hosting paragraph beside it (round 5,
      // as the desktop's Add to Media has them): the server's own words (media-edit.js).
      f.choice = f.choice || { licence: '', project_use: false };
      f.licencePicker = ED.licenceChoice(h, { licences: st.licences, choice: f.choice, id: 'media-form-licence', members: () => membersOnlyServer() || f.visibility === 'members' });
      kids.push(f.licencePicker.el, ED.hostingBlock(h, st.licences));
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
        className: 'media-btn media-btn-primary', disabled: f.busy || !!f.checking || !!(f.mode === 'add' && !f.uploadedName && sendingOf(f).refused),
        text: f.checking ? 'Working out your share...' : f.busy ? 'Working...' : f.attach ? 'Add it to the item' : put || f.uploadedName ? 'Put on Media' : 'Upload and add',
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
      if (f.attach) { attachUp(f); return; }
      publish(f);
    }

    /**
     * A file picked in an item's Edit form ("Add a file", or "Cover picture" as `as` "cover"):
     * checked and cleaned like Add to Media's, then the same resumable upload, listed under Uploads
     * in progress, and added to the item when it is up instead of published (`attachUp`). Resolves
     * to '' once it started, else the words saying why not (shown in the Edit form).
     */
    async function startAttach(file, item, as) {
      const filename = itemFileName(item);
      if (!filename) return MEDIA_WORDS.noFileName;
      if (!canUpload()) return bound() ? MEDIA_WORDS.cannotUpload : MEDIA_WORDS.notSignedInUpload;
      if (st.uploads.some((u) => sameFile(u.file, file) && uploadLive(u))) return '"' + file.name + '"' + MEDIA_WORDS.alreadyUploading;
      if (cleaner === false) return MEDIA_WORDS.checkerMissing;
      const title = itemName(item, st.kinds);
      const f = { mode: 'add', file, name: file.name, attach: { id: item.id, filename, as: as === 'cover' ? 'cover' : 'file', title }, busy: false, error: '', uploadedName: '', up: null, fp: '' };
      f.heading = file.name + ' (' + sizeWords(file.size) + '), to add to ' + title;
      if (cleaner) {
        f.protectedOn = cleaner.protectedSetupOn(env.storage);
        f.remove = cleaner.removeDetailsChosen(env.storage);
        f.clean = await cleaner.cleanFile(file);
      }
      const sending = sendingOf(f);
      if (!sending.file) return sending.refused || '';
      f.fpFile = sending.file;
      try { f.fp = fingerprint ? await fingerprint(f.fpFile, env.digest) : ''; } catch (e) { f.fp = ''; }
      // The person's share and the role's largest Media file, as Add to Media checks them.
      const mine = await loadMine();
      if (mine) {
        const stop = limitStop(file, num(mine.file_limit_bytes)) || shareStop(file, num(mine.used_bytes) + num(mine.in_progress_bytes), num(mine.share_bytes));
        if (stop) return stop;
      }
      f.actionsEl = h('div', { className: 'media-form-actions' });
      f.panel = h('div', { className: 'media-panel media-form' }, [h('h2', { text: f.heading }), f.actionsEl]);
      startUpload(f);
      return '';
    }

    /** An item just published with more files chosen beside its own: each starts its own upload to
     *  the new item, as Edit's Add a file does, and the line says so (uploading.rs attach_more). */
    async function attachMore(answer, more) {
      const item = answer && answer.item;
      if (!item || !itemFileName(item)) {
        say(MEDIA_WORDS.published + ' Its other files were not added: the server\'s answer did not say which item it made. Add them from the item\'s Edit window.');
        return;
      }
      let started = 0;
      const problems = [];
      for (const file of more) {
        const stop = await startAttach(file, item, 'file');
        if (stop) problems.push(file.name + ': ' + stop); else started++;
      }
      let line = MEDIA_WORDS.published;
      if (started) line += ' ' + (started === 1 ? 'Its other file is' : 'Its other ' + started + ' files are') + ' uploading under Uploads in progress, and added to it when up.';
      if (problems.length) line += ' ' + problems.join(' ');
      say(line);
    }

    /** An attaching upload is up: add it to its item (media-upload.js attachUpload), never publish it. */
    async function attachUp(f) {
      if (f.busy) return;
      f.busy = true;
      renderFormActions(f);
      try {
        const a = await U.attachUpload({ fetch: env.fetch, sign: env.sign, to: f.attach, add: f.uploadedName, cannotSign: MEDIA_WORDS.cannotSign });
        if (st.uploads.includes(f)) dropUpload(f);
        if (a && a.item) itemView.takeItem(a.item);
        say('Added to "' + f.attach.title + '".');
        itemView.uploadsChanged();
        loadMine();
        return;
      } catch (e) {
        f.error = (e && e.message ? e.message : String(e)) + MEDIA_WORDS.uploadWaits;
        f.up = null;
        loadMine();
      }
      f.busy = false;
      if (st.uploads.includes(f)) renderFormActions(f);
      itemView.uploadsChanged();
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
      // The cleaned file, or the file as picked when its details are kept by choice (keep_metadata).
      const sending = sendingOf(f);
      if (!sending.file) {
        f.error = sending.refused || '';
        renderFormActions(f);
        return;
      }
      // The switch as it was sent is the last choice on this device.
      if (cleaner && f.clean && !f.protectedOn) cleaner.rememberRemoveDetails(env.storage, f.remove !== false);
      if (f.detailsSwitch) f.detailsSwitch.disabled = true;
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
        fetch: env.fetch, token, file: sending.file, owner: myKey, storage: env.storage,
        fp: sending.file === f.fpFile ? f.fp : '', keepMetadata: sending.keepMetadata,
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
          if (f.attach) itemView.uploadsChanged();
        },
        onDone: (answer) => {
          f.uploadedName = storedName((answer && (answer.filename || answer.url)) || '');
          loadMine();
          // Round 5: a file for an item's Edit form is added to that item, never published.
          if (f.attach) attachUp(f); else publish(f);
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
        // The licence and the game tick (round 5): what the form's picker holds.
        const licence = (f.choice && f.choice.licence) || DEFAULT_LICENCE;
        const forGame = !!(f.choice && f.choice.project_use);
        const auth = await env.sign(purposes.publish(filename, licence, forGame));
        if (!auth) throw new Error(MEDIA_WORDS.cannotSign);
        const body = Object.assign({
          filename,
          title: (f.title || '').trim(),
          description: (f.description || '').trim(),
          labels: f.labels || [],
          visibility: f.visibility || 'public',
          licence,
          project_use: forGame,
        }, f.category ? { category_id: Number(f.category) } : {}, auth);
        const answer = await answerOf(await env.fetch('/api/media', jsonPost(body)));
        if (st.form === f) closeForm();
        if (st.uploads.includes(f)) dropUpload(f);
        say(MEDIA_WORDS.published);
        if (f.more && f.more.length) await attachMore(answer, f.more);
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

    // ── Report a copyright problem (/pages/media-takedown.js): anyone, signed in or not ──
    const takedown = TD ? TD.createTakedownForm({ document: doc, fetch: env.fetch, container: () => els.takedown }) : null;
    /** Open the form about `item`, or (from the page's foot) asking for the item's number. */
    function openTakedown(item) {
      if (!takedown) return;
      takedown.open(item ? { id: item.id, title: itemName(item, st.kinds) } : null);
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

    // ── Admins: the Admin tab (round 5; it was a <details> at the page's foot) ──
    function renderAdmin() {
      if (!st.adminMounted && isAdmin() && typeof env.mountAdmin === 'function') {
        st.adminMounted = true;
        const body = h('div', { id: 'media-admin-body', className: 'media-admin-body' });
        els.admin.appendChild(h('div', { className: 'media-panel media-admin', id: 'media-admin' }, [
          h('h2', { text: 'Media settings, categories and space' }),
          body,
        ]));
        env.mountAdmin(body);
        // The copyright reports waiting for a decision (/pages/media-takedown.js), on the same tab.
        if (typeof env.mountTakedowns === 'function') {
          const reports = h('div', { id: 'media-admin-takedowns-body', className: 'media-admin-body' });
          els.admin.appendChild(h('div', { className: 'media-panel media-admin', id: 'media-admin-takedowns' }, [
            h('h2', { text: 'Copyright reports' }),
            reports,
          ]));
          env.mountTakedowns(reports);
        }
      }
      renderTabs();
    }
    /** The tabs, for admins only: Media (the list) and Admin (the section above). */
    function renderTabs() {
      const admin = isAdmin();
      const tab = admin && st.tab === 'admin' ? 'admin' : 'media';
      els.main.hidden = tab !== 'media';
      els.admin.hidden = tab !== 'admin';
      els.tabs.replaceChildren();
      if (!admin) return;
      const button = (id, words) => h('button', {
        type: 'button', id: 'media-tab-' + id, className: 'media-tab' + (tab === id ? ' media-tab-active' : ''), text: words,
        attrs: { role: 'tab', 'aria-selected': String(tab === id) },
        on: { click: () => { st.tab = id; go(id === 'admin' ? '/media#admin' : '/media', false); renderTabs(); } },
      });
      els.tabs.appendChild(h('div', { id: 'media-tabs', className: 'media-tabs', attrs: { role: 'tablist' } }, [button('media', 'Media'), button('admin', 'Admin')]));
    }

    function renderAll() {
      renderStatus();
      renderNotice();
      renderMembers();
      renderActions();
      renderFilters();
      renderFeatured();
      renderGrid();
      itemView.renderIfChanged();
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

    /** /media/<n>: the item by its number, its address already (the chat's Reports view links there). */
    async function openById(id) {
      try {
        const a = await answerOf(await fetchList(listParams({ id }, null, 1)));
        const item = Array.isArray(a.items) ? a.items.find((i) => String(i.id) === String(id)) : null;
        if (!item) { say(MEDIA_WORDS.notFound, true); return; }
        takePeople([item]);
        if (a.licences && typeof a.licences === 'object') st.licences = a.licences;
        if (Array.isArray(a.labels) && a.labels.length) st.labels = a.labels;
        itemView.open(item, true);
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
      const id = itemIdOf(env.path);
      if (id != null) await openById(id);
    }

    /** Revoke every object URL (the page is going away). */
    function dispose() {
      for (const id of Array.from(blobs.keys())) dropBlob(id);
      itemView.tickets.clear();
    }

    return {
      start, state: st, load, loadFeatured, loadMine, openAddForm, openPutForm, removeItem: remove, pickItem: pick,
      featureItem: feature, keepItem: keep, openReport, openViewer: itemView.open, closeViewer: itemView.close, route, dispose, blobs, tickets: itemView.tickets, itemView,
    };
  }

  // ── In the browser ──
  function bootInBrowser() {
    const doc = root.document;
    const rootEl = doc && doc.getElementById('media-root');
    if (!rootEl) return;
    const loc = root.location;
    const params = new URLSearchParams(loc.search || '');
    const put = params.get('put') ? { filename: params.get('put'), name: params.get('name') || '' } : null;
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
        // /media/<n> opens item n (the relay serves this page there, 3.6); #admin the Admin tab.
        path: loc.pathname, hash: loc.hash, history: root.history,
        IntersectionObserver: root.IntersectionObserver || null,
        retryIdentity: () => root.location.reload(),
        // The admins' section on the Admin tab (/pages/media-admin.js, loaded after this file).
        mountAdmin: (container) => {
          if (typeof root.mountMediaAdmin !== 'function') return;
          root.mountMediaAdmin(container, {
            document: doc, fetch: (u, i) => root.fetch(u, i), sign, confirm, session,
            blake3: (bytes) => root.pqBlake3(bytes),
            onChange: () => { if (page) { page.load(true); page.loadFeatured(); } },
          });
        },
        // The copyright reports on the Admin tab (/pages/media-takedown.js, loaded before this file).
        mountTakedowns: (container) => {
          if (!root.MediaTakedown) return;
          root.MediaTakedown.mountTakedownAdmin(container, {
            document: doc, fetch: (u, i) => root.fetch(u, i), sign, confirm,
            onChange: () => { if (page) { page.load(true); page.loadFeatured(); } },
          });
        },
      });
      root.addEventListener('pagehide', () => page.dispose());
      // Back and Forward between /media and /media/<n> (and #admin) show what the address names.
      root.addEventListener('popstate', () => page.route(loc.pathname, loc.hash));
      page.start();
    };
    if (doc.readyState === 'loading') doc.addEventListener('DOMContentLoaded', go);
    else go();
  }

  const api = {
    MEDIA_WORDS, PAGE_SIZE, FEATURED_LIMIT, KINDS_URL, RESUME_KEY, parseKinds, kindWords, sizeWords, shareLeftWords,
    shareStop, limitStop, budgetStop, togetherStop, moreWords, meterWords, keptMeterWords, removeWords, removedWords, resetsWords, ownFilePath,
    fileRoute, storedName, itemFileName, itemName, downloadName, labelsOf, listParams, publicListUrl, purposes, jsonPost, isFeatured, itemIdOf, DEFAULT_LICENCE,
    refusalOf, refusalWords, answerOf, makeH, isAdminRole, isStaffRole, rememberUpload, findUpload, forgetUpload,
    createUploader, fingerprint, createMediaPage,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else {
    Object.assign(root, { MediaPage: api });
    bootInBrowser();
  }
})(typeof globalThis !== 'undefined' ? globalThis : this);
