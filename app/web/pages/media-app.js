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
// paragraph, in three tinted parts since 2026-10-17: "Files" first, each chosen
// file with its type's badge, then "About it" and "Who can see it", as the
// desktop's; then a resumable upload with a progress bar, Pause and Resume; a file
// picked in an item's Edit form is the same upload, attached to that item when it
// is up instead of published), Put on Media for a Shared Files row (public
// only), and Suggest a category. Files dragged from the computer onto the page
// (2026-10-17, /shared/drop-overlay.js) go the same way as chosen ones: "Drop to
// upload to Media" opens Add to Media with them, and with an item's Edit open
// "Drop to add to <its title>" adds them as its Add a file does. Add to Media
// ends with "Also post in chat" (2026-10-17, the desktop's media/add.rs): a
// tick, the channel (the one open in the chat first, then favourites, then the
// rest) and Add to favorites, greyed while the tick is off; once the item is
// published its title, description and file are posted there, signed as the
// chat signs, the chat's recovery-phrase guard first, and the line says
// "Posted in #general." or why not. Its Upload button wears the nav bar's RGB
// border. At /media?window=upload in a frame (/shared/upload-window.js) the
// page is the upload window over the chat: Add to Media and its uploads alone,
// opened with the files dropped on a channel there, posting ticked for it.
// "Report a copyright problem" (2026-10-13,
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
    // Why files dropped on the Admin tab do nothing (2026-10-17): Add to Media is not on it.
    addOnMediaTab: 'Add to Media is on the Media tab.',
  });

  // Add to Media, its uploads, publishing and "Also post in chat": /pages/media-add.js (2026-10-17).
  const ADD = typeof module === 'object' && module && module.exports ? require('./media-add.js') : root.MediaAdd;
  const { POST_WORDS, STAR, POST_FAVORITES_KEY, CHAT_CHANNEL_KEY, POST_GRACE_MS, postFavoriteKey, postFavorites, togglePostFavorite, postChoices, postText, postedIn, notPostedIn } = ADD;

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

  /** What the Add to Media form says above the further files chosen with the first (each on its own row, with its type's badge). */
  const MORE_LEAD = 'Also on this item, each added once the item is up:';
  /** The same in one line. */
  function moreWords(more) {
    return MORE_LEAD + ' ' + more.map((m) => m.name + ' (' + sizeWords(m.size) + ')').join(', ');
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
  // Files dragged from the computer onto the page (2026-10-17, /shared/drop-overlay.js).
  const DROP = typeof module === 'object' && module && module.exports ? require('../shared/drop-overlay.js') : root.HosDropOverlay;

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
   *   as videos, which the cleaning would rightly refuse),
   *   window (where files dragged from the computer are heard, /shared/drop-overlay.js; optional:
   *   without it nothing can be dropped on the page),
   *   phraseIn(texts) -> Promise<true|false|null> (the recovery-phrase guard for "Also post in chat",
   *   /shared/pq-relay-auth.js pqRecoveryPhraseIn: null when it cannot be checked, and then nothing
   *   is posted),
   *   uploadWindow ({send(event, data)}, /shared/upload-window.js connectFrame: the page is the
   *   upload window over another page, /media?window=upload. It shows only Add to Media and its
   *   uploads, takes the files that page hands over (takeFiles), and tells it what it says and
   *   whether something is going; optional).
   */
  function createMediaPage(env) {
    const doc = env.document;
    const h = makeH(doc);
    const myKey = env.myKey || '';
    const session = env.session || null;
    // The upload window (2026-10-17): this page in a frame over the chat, Add to Media alone.
    const upWin = env.uploadWindow || null;
    const windowMode = !!upWin;
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
      channels: [],             // this server's chat channels (its channel_list): "Also post in chat"
      channelsKnown: false,     // a channel_list has come
      pending: null,            // {files, postTo} handed to the upload window before it could take them
    };
    let loadSeq = 0;
    let featuredSeq = 0;
    let mineSeq = 0;

    // ── The page's fixed parts ──
    const els = {};
    const fileInput = h('input', { id: 'media-file-input', type: 'file', hidden: true, multiple: true, className: 'media-file-input' });
    fileInput.addEventListener('change', () => {
      const files = Array.from(fileInput.files || []);
      try { fileInput.value = ''; } catch (e) { /* some browsers refuse; harmless */ }
      chooseFiles(files);
    });
    /**
     * Files chosen with Add to Media, or dropped on the page (2026-10-17): the same path for both.
     * Several at once (2026-10-16): one item holds them all; the first is its own file.
     */
    function chooseFiles(files) {
      if (files && files[0]) openAddForm(files[0], files.slice(1));
    }
    function build() {
      doc.addEventListener('keydown', (e) => {
        if (e.key === 'Escape' && takedown && takedown.isOpen()) { takedown.close(); return; }
        if (e.key === 'Escape' && st.report) { closeReport(); return; }
        if (e.key === 'Escape' && st.viewer) { itemView.close(); return; }
        // The upload window: Escape closes it, as the page it is over does (it stays while
        // something is going in it, and carries on).
        if (e.key === 'Escape' && upWin) upWin.send('close');
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
      if (windowMode) {
        // The upload window (/shared/upload-window.js): Add to Media and its uploads, under the
        // page's sign-in line and its notices, and nothing of the list, its tabs or its dialogs (the
        // parts above are made, never shown, so everything that draws into them can stay as it is).
        els.main = h('div', { id: 'media-main', className: 'media-window' }, [els.form, els.uploads]);
        env.root.replaceChildren(els.status, els.notice, els.main, fileInput);
        return;
      }
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
      // The upload window tells the page it is over each line it says: shown there while the
      // window is closed and an upload carries on (the chat's notices).
      if (upWin && st.notice) upWin.send('status', { text: st.notice });
    }
    function renderNotice() {
      els.notice.replaceChildren();
      els.notice.className = 'media-notice' + (st.noticeWarn ? ' media-warn' : '');
      if (st.notice) els.notice.appendChild(h('p', { className: 'media-notice-text', text: st.notice }));
    }
    /** The status line's words about signing in ('' once signed in), and its Try again (or null). */
    function statusWords() {
      const s = sessionState();
      if (!myKey) return { words: MEDIA_WORDS.noIdentity, retry: env.retryIdentity || null };
      if (s) {
        if (s.status === 'connecting' || s.status === 'idle') return { words: MEDIA_WORDS.signingIn, retry: null };
        if (s.status === 'refused') return { words: 'The server did not accept this page\'s sign-in: ' + s.refusal, retry: () => session.retry() };
        if (s.status === 'closed') return { words: 'Lost the connection to the server. Trying again in ' + (s.retryInSeconds || 5) + ' seconds.', retry: null };
        if (s.status === 'no_identity') return { words: MEDIA_WORDS.noIdentity, retry: env.retryIdentity || null };
      }
      return { words: '', retry: null };
    }
    function renderStatus() {
      els.status.replaceChildren();
      const { words, retry } = statusWords();
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
      // The upload window shows no list (and so fetches none of its pictures).
      if (windowMode) return;
      els.featured.replaceChildren();
      if (!st.featured.length) return;
      // The main shelf, or the shelf of the category it was asked for (3.9: "Featured in Fire flow arts").
      const cat = st.shelfOf ? st.categories.find((c) => String(c.id) === String(st.shelfOf)) : null;
      els.featured.appendChild(h('h2', { id: 'media-featured-title', text: cat ? 'Featured in ' + cat.name : 'Featured' }));
      els.featured.appendChild(h('div', { id: 'media-featured-row', className: 'media-featured-row' }, st.featured.map(tile)));
    }

    function renderGrid() {
      if (windowMode) return; // the upload window shows no list (renderFeatured)
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
      // The upload window reads the list only for what its answer carries besides the items (the
      // categories, the labels, the licences, and whether this person sees the members' side): one item.
      const params = listParams(st.filters, reset ? null : st.nextCursor, windowMode ? 1 : undefined);
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
      if (windowMode) return; // the upload window shows no shelf
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

    // ── Add to Media, Put on Media, publishing, "Also post in chat" and the upload window's side ──
    // In /pages/media-add.js (moved there 2026-10-17): made here with what it shares with the page.
    const addForm = ADD.createAddForm({
      st, els, h, doc, env, session, myKey, upWin, cleaner, later, cancelLater, now, Abort, itemView,
      canUpload, bound, token, sessionState, knownCannotUpload, isAdmin, membersOnlyServer, loadMine, load, say, renderWaiting, sayCancelled,
      lib: {
        MEDIA_WORDS, MORE_LEAD, ANSWER_WAIT_MS, DEFAULT_LICENCE, num, sizeWords, shareLeftWords, limitStop, shareStop, budgetStop,
        togetherStop, resetsWords, storedName, itemFileName, itemName, purposes, jsonPost, answerOf, findUpload, createUploader,
        fingerprint, U, ED,
      },
    });
    const { openAddForm, openPutForm, openWaitingForm, closeForm, formKey, renderForm, renderPostRow, takePending, reportBusy,
      takeChannels, takeFiles, uploadWords, startAttach } = addForm;

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
      if (!windowMode) {
        renderMembers();
        renderActions();
        renderFilters();
        renderFeatured();
        renderGrid();
        itemView.renderIfChanged();
        renderWaiting();
      }
      if (st.form && !st.form.busy && !st.form.up && st.form.key !== formKey()) renderForm();
      if (st.form) renderPostRow(st.form); // the person's role may have come (admins see read-only channels)
      if (!windowMode) {
        renderAdmin();
        sweepBlobs();
        pumpBlobs();
      }
      takePending();
      reportBusy();
    }

    // ── The session's news ──
    function onSession(ev) {
      // "Also post in chat" waiting for its answer hears everything first (media-add.js postPublished).
      addForm.heard(ev);
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
      // This server's chat channels, as it lists them to every signed-in connection.
      if (m && m.type === 'channel_list' && Array.isArray(m.channels)) { takeChannels(m.channels); return; }
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

    // ── Files dragged from the computer onto the page (2026-10-17, /shared/drop-overlay.js) ──
    // The desktop's words while they are over it: "Drop to upload to Media", which opens Add to
    // Media with them exactly as choosing them does (chooseFiles); with an item's Edit open, "Drop
    // to add to <its title>", which adds them as its Add a file does. Where a drop would do nothing,
    // "Files cannot be dropped here." and the words the page shows for why its button is not there.
    let drops = null;
    /** Why Add to Media is not offered now: the page's own words for it. */
    function whyNoAdd() {
      const signIn = statusWords().words;
      if (signIn) return signIn;
      if (knownCannotUpload()) return MEDIA_WORDS.cannotUpload;
      if (!st.membersView) return MEDIA_WORDS.membersLine;
      return MEDIA_WORDS.signingIn; // signed in, the role list not here yet
    }
    /** What files dropped now would do: {take, line} or {take: false, reason}. */
    function dropDecision() {
      const edit = itemView.page.edit;
      if (edit && st.viewer) {
        if (edit.addFiles) return { take: true, line: DROP.addTo(itemName(itemView.current(st.viewer.id) || st.viewer, st.kinds)) };
        return { take: false, reason: edit.addFilesWhyNot || '' };
      }
      if (isAdmin() && st.tab === 'admin') return { take: false, reason: MEDIA_WORDS.addOnMediaTab };
      if (myKey && st.membersView && canUpload()) {
        // The upload window over a channel of the chat: a drop on it posts there too, as one on the chat.
        const ch = upWin && st.windowPostTo ? st.channels.find((c) => c.id === st.windowPostTo) : null;
        return { take: true, line: ch ? DROP.uploadAndPostIn('#' + ch.name) : DROP.WORDS.uploadToMedia };
      }
      return { take: false, reason: whyNoAdd() };
    }
    function dropped(files) {
      const edit = itemView.page.edit;
      if (edit && st.viewer) {
        if (edit.addFiles) edit.addFiles(files, 'file');
        return;
      }
      // An item open (and not being edited) covers the page: it closes, so Add to Media is seen.
      if (st.viewer) itemView.close();
      if (upWin && st.windowPostTo && files && files[0]) { openAddForm(files[0], files.slice(1), st.windowPostTo); return; }
      chooseFiles(files);
    }

    async function start() {
      build();
      if (DROP && env.window) drops = DROP.attach({ window: env.window, document: doc, decide: dropDecision, onDrop: dropped });
      if (session) {
        session.subscribe(onSession);
        if (myKey) session.start();
      }
      renderAll();
      // The upload window: only what Add to Media needs (the list's categories, labels and
      // licences, and the person's share).
      if (windowMode) {
        await load(true);
        loadMine();
        return;
      }
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
      if (drops) { drops.detach(); drops = null; }
      for (const id of Array.from(blobs.keys())) dropBlob(id);
      itemView.tickets.clear();
    }

    return {
      start, state: st, load, loadFeatured, loadMine, openAddForm, openPutForm, removeItem: remove, pickItem: pick,
      featureItem: feature, keepItem: keep, openReport, openViewer: itemView.open, closeViewer: itemView.close, route, dispose, blobs, tickets: itemView.tickets, itemView,
      takeFiles,
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
      // The upload window (2026-10-17, /shared/upload-window.js): this page at /media?window=upload,
      // in a frame over the chat, which hands it the files dropped on a channel. Opened any other way
      // it is the Media page as usual.
      const UW = root.HosUploadWindow || null;
      const uploadWindow = UW && UW.isWindowAddress(loc.search)
        ? UW.connectFrame({ window: root, onFiles: (files, postTo) => { if (page) page.takeFiles(files, postTo); } })
        : null;
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
        // Where files dragged from the computer are heard (2026-10-17, /shared/drop-overlay.js).
        window: root,
        // "Also post in chat" never sends my recovery phrase: the chat's own guard
        // (/shared/pq-relay-auth.js pqRecoveryPhraseIn, over /shared/warnings.js and the word list).
        phraseIn: (texts) => (typeof root.pqRecoveryPhraseIn === 'function' ? root.pqRecoveryPhraseIn(texts) : Promise.resolve(null)),
        uploadWindow,
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
      // Built now (start builds the page at once): the files can be handed over.
      if (uploadWindow) uploadWindow.ready();
    };
    if (doc.readyState === 'loading') doc.addEventListener('DOMContentLoaded', go);
    else go();
  }

  const api = {
    MEDIA_WORDS, PAGE_SIZE, FEATURED_LIMIT, KINDS_URL, RESUME_KEY, parseKinds, kindWords, sizeWords, shareLeftWords,
    shareStop, limitStop, budgetStop, togetherStop, MORE_LEAD, moreWords, meterWords, keptMeterWords, removeWords, removedWords, resetsWords, ownFilePath,
    fileRoute, storedName, itemFileName, itemName, downloadName, labelsOf, listParams, publicListUrl, purposes, jsonPost, isFeatured, itemIdOf, DEFAULT_LICENCE,
    refusalOf, refusalWords, answerOf, makeH, isAdminRole, isStaffRole, rememberUpload, findUpload, forgetUpload,
    createUploader, fingerprint, createMediaPage,
    POST_WORDS, STAR, POST_FAVORITES_KEY, CHAT_CHANNEL_KEY, POST_GRACE_MS, postFavoriteKey, postFavorites, togglePostFavorite,
    postChoices, postText, postedIn, notPostedIn,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else {
    Object.assign(root, { MediaPage: api });
    bootInBrowser();
  }
})(typeof globalThis !== 'undefined' ? globalThis : this);
