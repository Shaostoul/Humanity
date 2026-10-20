// ── media-item.js ─────────────────────────────────────────────────────────
// An item's own page on the website (Media round 5, lane 2A, 2026-10-12;
// docs/design/media-round-5-plan.md 3.5, 3.9 and 3.10). It is the Media page's
// viewer grown: a tile's click opens it over the page and pushes /media/<id>
// onto the history, closing goes back, and loading /media/<id> opens it. The
// desktop app's item page is the original (src/gui/pages/media/item_view.rs);
// this one copies its lines and words:
//
// - the picture, the video, or the cover (a labelled item keeps its label
//   cover until clicked, and nothing is fetched before then);
// - the title; "by Ann in Fire flow arts"; its labels; "Kept by the server";
//   "Members only"; "Featured" or "Featured in its category";
// - "4 picks, 12 views, 3 downloads";
// - the whole description, as plain text with its line breaks;
// - the licence by name, linked to its deed, with the server's own line for it,
//   and "Offered for the game." when it is;
// - its files, the item's own first, each with its type's badge (2026-10-17,
//   media-edit.js fileBadge: the kind's word left the row then), its name,
//   size and Download. A Download goes through the relay's counting route,
//   /api/media/download/<id>?file=<n>, never the file's own address, so it is
//   counted; a members-only file's through a ticket asked for that route and
//   that file (`download: true`), which the browser then saves itself; under
//   a file whose kind deserves a word (an archive, a Blender file, a program
//   or a script), that word, by its name (2026-10-17, /shared/file-risk.js);
// - Copy link (`https://<host>/media/<id>`, the page the relay serves with its
//   preview for social sites), Edit (the uploader and admins: media-edit.js),
//   and the tile's own actions (Pick, Remove, Feature, Keep, Report);
// - "Report a copyright problem" (2026-10-13), for anyone, signed in or not:
//   media-takedown.js's form, about this item;
// - View in 3D (2026-10-16, docs/design/model-viewer.md) beside the Download of
//   each .glb or .stl file: the 3D viewer (/shared/model-view.js) takes the
//   place of the cover with Close the 3D view. Nothing is fetched before it is
//   pressed; then the file comes from its own address (`url`), fetched exactly
//   as a members-only picture is, never through the counting route, so looking
//   at a model is not a download.
//
// Opening it counts a view (POST /api/media/viewed, unsigned; the session's
// upload token goes in its header only for a members-only item, which only a
// member's view counts). An item that changed (an edit, a file added or taken
// off) is changed wherever the page shows it (`takeItem`).
//
// IN TWO PANES SINCE 2026-10-19 (the desktop's three panes, src/gui/pages/
// media/panes.rs, viewer.rs and item_view.rs draw_details; docs/design/
// media-showcase.md "The three panes"). It is no longer a window over the page:
// the item is drawn into the Media page's middle and right panes, beside the
// list (/pages/media-panes.js), and in the item window over the chat into the
// same two panes, side by side or one above the other:
// - THE MIDDLE PANE (#media-face) shows the item to fit: a picture as large as
//   fits (with View full size), a video in the browser's player, a sound with
//   Play here and then its waveform (/pages/media-sound.js: nothing is fetched
//   before the press), a model's cover with View in 3D (the model fetched only
//   when pressed) and Download, any other file as its type, name, size, word of
//   caution and Download;
// - THE RIGHT PANE (#media-details) holds everything about it, as above.
// EDIT IS IN PLACE (the operator, 2026-10-19: "When clicking edit, instead of
// popping up another window, just change the details panel on the right to
// accept edits. Once done clicking save undoes the GUI changes for edit mode.
// ... when we switch to edit mode, add the RGB boxed areas so edit mode stands
// out."): Edit turns the right pane into the form (media-edit.js), in its three
// tinted parts, the pane edged in the nav bar's moving colours, Save and Cancel
// always in view at its foot; Save sends it and the pane is the details again.
// Cancel with changes asks first ("Discard the changes" or "Keep editing"), and
// so does anything that would leave the form: another item pressed, Close,
// Escape, Back or Forward, the Gallery; the browser asks itself before the page
// is left (media-app.js beforeunload).
//
// The page is drawn in two parts: the face (the picture or a playing video) is
// drawn again only when what it shows changes, so a video keeps playing
// through a sign-in, a reload or an edit; the rest (`renderInfo`) is drawn
// again whenever the item changes. Everything the server sends goes in with
// textContent; a picture or video loads only from this server's own addresses.
//
// A classic script in the browser (after media-edit.js, before media-app.js),
// a CommonJS module under Node, where scripts/tests/media-web-item.test.js
// runs the page with it.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  const ED = typeof module === 'object' && module && module.exports ? require('./media-edit.js') : root.MediaEdit;
  // The 3D viewer (/shared/model-view.js, loaded before this file). Without it, no View in 3D.
  const MV = typeof module === 'object' && module && module.exports ? require('../shared/model-view.js') : root.HosModelView;
  // The protected setup's state and pictures rule (/shared/protected.js, loaded before this file,
  // which puts its names on the page).
  const PR = typeof module === 'object' && module && module.exports ? require('../shared/protected.js') : root;
  // What kind of file each one is, when that deserves a word (/shared/file-risk.js, loaded before
  // this file, 2026-10-17): a program, a script, an archive, a Blender file. Without it, no line.
  const FR = typeof module === 'object' && module && module.exports ? require('../shared/file-risk.js') : root.HosFileRisk;
  // A sound's view in the middle pane (/pages/media-sound.js), and the panes' words (/pages/media-panes.js).
  const MS = typeof module === 'object' && module && module.exports ? require('./media-sound.js') : root.MediaSound;
  const MP = typeof module === 'object' && module && module.exports ? require('./media-panes.js') : root.MediaPanes;

  // What an item hidden by the protected setup for its content label says (the desktop's
  // tiles.rs LABEL_HIDDEN_LINE). The pictures rule's own line is the preset's
  // (data/gui/safety_presets.json, `picture_hidden_line`).
  const LABEL_HIDDEN_LINE = 'Hidden: the protected setup leaves out items with a content label.';

  /**
   * Does the protected setup (`state`: protected.js's state, null when it is off) leave `item` out?
   * The line to show in its place, or null when it shows as usual. The desktop's rule
   * (src/gui/pages/media/tiles.rs `show_of`, its Hidden branch), in its order: a non-friend's
   * pictures and files under the pictures rule (`pictureLine`), then any labelled item that is not
   * your own. Your own items are never hidden. On this page nobody counts as a friend: the friend
   * list is in Chat's encrypted store, which this page cannot open, so another person's items stay
   * hidden while the rule is on (fail safe, as Chat itself does before its store has loaded).
   */
  function hiddenBy(state, item, myKey, pictureLine) {
    if (!state || !item) return null;
    const key = typeof item.uploader_key === 'string' ? item.uploader_key.trim().toLowerCase() : '';
    const own = !!key && key === String(myKey || '').trim().toLowerCase();
    const hidesPictures = typeof PR.protectedHidesPictures === 'function'
      ? PR.protectedHidesPictures(state, { isMe: own, isFriend: false })
      : true; // the rule could not be loaded: hidden (fail safe)
    if (key && !own && hidesPictures) return pictureLine || '';
    if (Array.isArray(item.labels) && item.labels.length && !own) return LABEL_HIDDEN_LINE;
    return null;
  }

  // A ticket is not used for a new request this close to its end (the relay's are ten minutes).
  const TICKET_MARGIN_MS = 30000;
  // A playing video whose ticket fails gets a new one, but not for a ticket younger than this.
  const TICKET_RENEW_AFTER_MS = 60000;
  // The most characters a card shows of a description's first line, "..." included.
  const CARD_LINE_MAX = 120;

  const WORDS = Object.freeze({
    copyLink: 'Copy link',
    copyTip: "Copy this item's own address, to share it.",
    edit: 'Edit',
    editTip: 'Change its words, licence, files and cover.',
    download: 'Download',
    downloadTip: 'Save it in your downloads folder.',
    editHint: 'Add or remove files and the cover in Edit.',
    // Plain status words only: what the offer allows is the licence's, in the server's own words.
    offeredForGame: 'Offered for the game.',
    removedFromItem: 'Removed from the item.',
    // The copyright form's button (media-takedown.js holds the same words as WORDS.button).
    takedown: 'Report a copyright problem',
    takedownTip: 'Is this your work, put here without your permission? Tell the admins.',
    saved: 'Saved.',
    // The two panes (2026-10-19; the desktop's viewer.rs VIEW_FULL, item_view.rs DISCARD_TITLE,
    // DISCARD_THEM, KEEP_EDITING and discard_words).
    close: 'Close',
    closeTip: 'Close this item.',
    viewFull: 'View full size',
    viewFullTip: 'Open it at its own size, in a new tab.',
    discardTitle: 'Unsaved changes',
    discardThem: 'Discard the changes',
    keepEditing: 'Keep editing',
    faceLabel: 'The item',
    detailsLabel: 'Its details',
  });

  /** The question when an edit with changes would be left (the desktop's discard_words). */
  function discardWords(title) {
    return 'Your changes to "' + title + '" are not saved.';
  }

  /** "On Media since 9 October 2025" (UTC), from the server's time in milliseconds; '' when it gave none (the desktop's since_line). */
  const MONTHS = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'];
  function sinceLine(item) {
    const ms = Number(item && item.published_at) > 0 ? Number(item.published_at) : Number(item && item.uploaded_at) || 0;
    if (!(ms > 0)) return '';
    const d = new Date(ms);
    if (Number.isNaN(d.getTime())) return '';
    return 'On Media since ' + d.getUTCDate() + ' ' + MONTHS[d.getUTCMonth()] + ' ' + d.getUTCFullYear();
  }

  // ── What the item's page and its card say (the desktop's words) ──

  /** A count in words, singular when it is one: "1 pick", "4 picks". */
  function counted(n, one) {
    const k = Number(n) || 0;
    return k === 1 ? '1 ' + one : k + ' ' + one + 's';
  }
  /** "4 picks, 12 views, 3 downloads". */
  function countsLine(item) {
    return [counted(item.picks, 'pick'), counted(item.views, 'view'), counted(item.downloads, 'download')].join(', ');
  }
  /**
   * "by Ann in Fire flow arts" (the category left out when the server named none). The name
   * trimmed, "someone" when it is blank (the desktop's by_name).
   */
  function byLine(item) {
    const name = item && typeof item.uploader_name === 'string' ? item.uploader_name.trim() : '';
    const by = 'by ' + (name || 'someone');
    const cat = item && item.category && typeof item.category.name === 'string' ? item.category.name.trim() : '';
    return cat ? by + ' in ' + cat : by;
  }
  /**
   * What a card shows of a description: its first line, at most 120 characters, cut at a word
   * with "..." when it is longer. The item's page shows the whole description.
   */
  function firstLine(description) {
    const line = String(description || '').trim().split(/\r?\n/)[0].trim();
    const chars = Array.from(line);
    if (chars.length <= CARD_LINE_MAX) return line;
    const cut = chars.slice(0, CARD_LINE_MAX - 3).join('');
    const space = cut.search(/\s\S*$/);
    return (space > 0 ? cut.slice(0, space) : cut).trimEnd() + '...';
  }
  /** "3 files" when an item holds several (its own and the added ones, not the cover), else ''. */
  function filesBadge(item) {
    const n = Number(item && item.file_count) || 0;
    return n > 1 ? n + ' files' : '';
  }
  /** The item's own page on the server at `origin`: what Copy link copies, never /media?id=. */
  function itemLink(origin, id) {
    return String(origin || '').replace(/\/+$/, '') + '/media/' + id;
  }
  /** The relay's counting route for file `file` of item `id`. */
  function downloadPath(id, file) {
    return '/api/media/download/' + id + '?file=' + (Number(file) || 0);
  }
  /** The item's files, its own first: as the server lists them, or its own file alone. */
  function filesOf(item) {
    if (item && Array.isArray(item.files) && item.files.length) return item.files;
    return [{ file: 0, filename: item ? item.filename || '' : '', original_name: item ? item.original_name || '' : '', kind: item ? item.kind : '', size_bytes: item ? item.size_bytes : 0, url: item ? item.url : '' }];
  }
  /** A cover as the page's pictures are drawn: its own key, its address, a picture. */
  function coverOf(item) {
    return item && item.cover && typeof item.cover.url === 'string' && item.cover.url
      ? { id: 'c' + item.id, url: item.cover.url, kind: 'picture', size_bytes: item.cover.size_bytes, title: item.title }
      : null;
  }

  /**
   * The item's page. `c`: env, h, st, els, W (the page's words), lib (its helpers), token() (the
   * token a members-only file is fetched with: in the item window over the chat with no sign-in of
   * its own, the chat's, lent to it), tokenRefused(t) (a token the server no longer knows: a promise
   * of the chat's new one when `t` was lent, else null; optional), myKey,
   * isAdmin(), membersOnlyServer(), blobs, blobFor(item), sweepBlobs(), go(path, push), canPush,
   * back(), rerender() (the grid and the shelf), actions(item) (a tile's actions, for this page),
   * attach(file, item, as) and uploadsFor(id) (media-app.js's resumable upload, attaching), and
   * closed() (called once an open item is closed: the item window over the chat closes with it), or null.
   * Since the panes (2026-10-19): handle() (the handle between the middle and right panes, from
   * media-panes.js), placeholders (draw the two panes' lines while nothing is chosen: the Media page,
   * not the item window), selected() (the item shown changed: the list marks it) and editChanged()
   * (the Edit form opened, closed or was typed in: the item window tells the chat), each optional;
   * els.ask, where the question before letting changes go is drawn.
   */
  /**
   * A file's name as it is shown (2026-10-17): /shared/file-risk.js plainName, its direction marks
   * out (a right-to-left override makes "photo<RLO>gnp.exe" read as "photoexe.png"); should that
   * file not have loaded, every format and control character goes.
   */
  function plainName(name) {
    if (FR && typeof FR.plainName === 'function') return FR.plainName(name);
    return String(name == null ? '' : name).replace(/[\p{Cc}\p{Cf}]/gu, '').trim();
  }

  function createItemView(c) {
    const { env, h, st, els, W, lib } = c;
    const now = () => (typeof env.now === 'function' ? env.now() : Date.now());
    const tickets = new Map(); // item id -> {state: 'loading'|'ready'|'error', url, why, token, at, expiresAt, renewing}
    const asking = new Set(); // "<id>:<file>" while a Download's ticket is on its way
    const downloadErrors = new Map(); // "<id>:<file>" -> the words of a Download that could not start
    // `model`: the open 3D view, {itemId, file, box, handle}, drawn in place of the cover.
    // `sound`: a sound's view in the middle pane, {itemId, view} (media-sound.js).
    // `asking`: the question before an edit's changes are let go, {then, keep}, while it is open.
    const page = { status: '', copied: '', edit: null, editBox: null, infoBox: null, model: null, sound: null, asking: null, facePane: null, detailsPane: null };
    // What opens the 3D view: the page's own stand-in when it hands one in (the tests, which have no
    // WebGL), else /shared/model-view.js. Its words and limits are always that file's.
    const viewer = () => env.modelView || MV || null;

    // ── What kind of file each one is (/shared/file-risk.js) ──
    let riskAsked = false;
    /**
     * The word of caution for a file named `name` ({kind, disguised, words}), or null. By name only:
     * the page never reads a file before its Download. The list is asked for once, the first time
     * a file row is drawn before it is in; the rows are drawn again when it comes.
     */
    function riskOf(name) {
      if (!FR || typeof FR.assess !== 'function') return null;
      if (!typesReady()) return null;
      return FR.assess(name, null);
    }
    /** Is the list of kinds in? Asked for once, the first time it is wanted before it is. */
    function typesReady() {
      if (!FR || typeof FR.ready !== 'function') return false;
      if (FR.ready()) return true;
      if (!riskAsked && typeof FR.load === 'function') {
        riskAsked = true;
        // The rows drawn again once it is in: each line, and each file's badge (its type, which
        // was a plain grey "File" until then), on this page and in an open Edit form; and the
        // middle pane, which shows a sound by its type (its waveform, 2026-10-19).
        Promise.resolve(FR.load(env.fetch))
          .then(() => { if (FR.ready()) { renderIfChanged(); renderInfo(); c.rerender(); if (page.edit) page.edit.drawFiles(); } })
          .catch(() => { /* no list: no line */ });
      }
      return false;
    }

    // ── The protected setup (protected.js): what it leaves out of this page ──
    let presetAsked = false;
    /** The setup as this device keeps it (protected.js's reader: damaged counts as on); null when off. */
    function protectedState() {
      const store = env.storage || null;
      if (typeof PR.protectedStateRead === 'function') return PR.protectedStateRead(store);
      // protected.js could not be loaded: any setup kept on this device counts as on (fail safe).
      let raw = null;
      try { raw = store ? store.getItem('humanity_protected_setup') : null; } catch (e) { raw = null; }
      if (raw == null || raw === '') return null;
      try { const v = JSON.parse(raw); if (v && v.on === false) return null; } catch (e) { /* damaged: on */ }
      return { on: true, pictures: 'never' };
    }
    /** The preset's line for a hidden picture, asked for once, the first time one is hidden ('' until it comes). */
    function pictureLine() {
      if (typeof st.protectedLine === 'string') return st.protectedLine;
      if (!presetAsked && typeof PR.protectedPresetFrom === 'function' && PR.PROTECTED_PRESETS_URL) {
        presetAsked = true;
        Promise.resolve()
          .then(() => env.fetch(PR.PROTECTED_PRESETS_URL))
          .then((res) => (res && res.ok ? res.json() : null))
          .then((data) => {
            const preset = PR.protectedPresetFrom(data);
            if (preset && typeof preset.picture_hidden_line === 'string') {
              st.protectedLine = preset.picture_hidden_line;
              protectionChanged();
            }
          })
          .catch(() => { /* the line stays empty; the item stays hidden */ });
      }
      return '';
    }
    /** What the protected setup shows in place of `item` (its line), or null when it shows as usual. */
    function hiddenLine(item) {
      const state = protectedState();
      if (!state || !item) return null;
      return hiddenBy(state, item, c.myKey, pictureLine());
    }
    /** An open 3D view of an item the setup now hides is closed (a model's textures are pictures too). */
    function closeIfHidden() {
      const m = page.model;
      if (!m || !st.viewer || m.itemId !== st.viewer.id || hiddenLine(st.viewer) === null) return false;
      closeModel(true);
      return true;
    }
    /** The setup changed (in Chat, in another tab): every part of the page follows. */
    function protectionChanged() {
      closeIfHidden();
      renderIfChanged();
      renderInfo();
      c.rerender();
    }
    // Another tab turning the setup on or changing it reaches this page as a storage event.
    const win = env.document && env.document.defaultView;
    if (win && typeof win.addEventListener === 'function') {
      win.addEventListener('storage', (e) => {
        if (!e || !e.key || e.key === (PR.PROTECTED_STORAGE_KEY || 'humanity_protected_setup')) protectionChanged();
      });
    }

    /** The item as the page holds it now (the list's copy, the shelf's, or the one opened by number). */
    function current(id) {
      return st.items.concat(st.featured).find((i) => i.id === id) || (st.viewer && st.viewer.id === id ? st.viewer : null);
    }
    const own = (item) => !!c.myKey && !!item && item.uploader_key === c.myKey;
    /** The uploader or an admin (decision 1: moderators edit nothing; they still Keep). */
    const mayEdit = (item) => !!c.myKey && (own(item) || c.isAdmin());
    /** The name Download saves file `f` under: the uploader's own name for it, else its stored name. */
    function fileName(item, f) {
      if (!f || !f.file) return lib.downloadName(item, st.kinds);
      return (f.original_name && f.original_name.trim()) || (f.filename && f.filename.trim()) || lib.itemName(item, st.kinds) + '-' + item.id + '-' + f.file;
    }
    /**
     * The name file `f` is shown and saved by (the desktop's open.rs shown_file_name): fileName
     * made plain, so people read its real name and its badge judges its real ending; the item's
     * own name when nothing is left of it.
     */
    function shownName(item, f) {
      return plainName(fileName(item, f)) || plainName(lib.itemName(item, st.kinds)) || 'This item';
    }

    // ── Tickets (the relay's R3-8): a members-only video or Download, streamed by the browser ──
    /** A ticket's address, when it is this server's route for this item (the file or the counting route) with a ticket. */
    function ticketPath(url, id, download) {
      if (typeof url !== 'string' || !url) return null;
      let u;
      try { u = new URL(url, env.origin); } catch (e) { return null; }
      if (u.origin !== new URL(env.origin).origin) return null;
      if (u.pathname !== (download ? '/api/media/download/' : '/api/media/file/') + id || !u.searchParams.get('ticket')) return null;
      return u.pathname + u.search;
    }
    /** Ask for a ticket: {path, expiresIn} (seconds), or throws Error(the relay's words). A Download's names its file. */
    async function fetchTicket(item, t, file) {
      const filename = lib.itemFileName(item);
      if (!filename) throw new Error(W.noFileName);
      const download = file != null;
      const a = await lib.answerOf(await env.fetch('/api/media/ticket', {
        method: 'POST', headers: { 'Content-Type': 'application/json', 'X-Upload-Token': t },
        body: JSON.stringify(download ? { id: item.id, filename, file: Number(file) || 0, download: true } : { id: item.id, filename }),
      }));
      const path = ticketPath(a && a.url, item.id, download);
      if (!path) throw new Error(W.ticketOdd);
      return { path, expiresIn: lib.num(a.expires_in_s) || 600 };
    }
    /** Was `err` (a refusal, answerOf's) the server no longer knowing the upload token it was sent? */
    function tokenUnknown(err) {
      return !!err && typeof lib.tokenUnknown === 'function' && lib.tokenUnknown(err.status, err.message);
    }
    /**
     * A ticket asked for with token `t` (fetchTicket's arguments). When the server no longer knows `t`
     * and it was one the chat lent this page (the item window over the chat, media-app.js fileToken:
     * the chat signed in again since), the chat is asked for its new one and the ticket asked for once
     * more with that. Answers {got, token (the one it was given with)}, or throws the relay's words.
     */
    async function ticketWith(item, t, file) {
      try {
        return { got: await fetchTicket(item, t, file), token: t };
      } catch (err) {
        const again = tokenUnknown(err) && typeof c.tokenRefused === 'function' ? c.tokenRefused(t) : null;
        if (!again) throw err;
        const next = await again;
        if (!next || next === t) throw err;
        return { got: await fetchTicket(item, next, file), token: next };
      }
    }
    /** The viewer's ticket for a members-only video, asked for when there is none that can still be used. */
    function ticketFor(item) {
      let e = tickets.get(item.id);
      const t = c.token();
      if (e && e.state === 'loading') return e;
      if (e && e.state === 'ready' && (e.renewing || e.expiresAt - TICKET_MARGIN_MS > now())) return e;
      if (e && e.state === 'error' && e.token === t) return e;
      if (!t) return e || null;
      e = { state: 'loading', url: '', why: '', token: t, at: now(), expiresAt: 0, renewing: false };
      tickets.set(item.id, e);
      ticketWith(item, t).then(({ got, token }) => {
        if (tickets.get(item.id) !== e) return;
        Object.assign(e, { url: got.path, at: now(), expiresAt: now() + got.expiresIn * 1000, state: 'ready', token });
        renderIfChanged();
      }, (err) => {
        if (tickets.get(item.id) !== e) return;
        // The token gone and none to take its place yet: waiting for one, not a failure (asked for
        // afresh when it comes).
        if (tokenUnknown(err) && !c.token()) { tickets.delete(item.id); renderIfChanged(); return; }
        e.state = 'error';
        e.why = W.fileFailed + (err && err.message ? err.message : String(err));
        renderIfChanged();
      });
      return e;
    }
    /**
     * A playing video whose ticket has run out: a new ticket, the same video element, from where it
     * stopped. Not for a ticket younger than a minute (then it is the file that fails). A new ticket
     * refused leaves nothing to play, so the relay's words show where the video was (round 4, W4-2).
     */
    function renewTicket(item, video) {
      const e = tickets.get(item.id);
      const t = c.token();
      if (!e || e.state !== 'ready' || e.renewing || !t || now() - e.at < TICKET_RENEW_AFTER_MS) return;
      e.renewing = true;
      let pos = 0;
      let playing = false;
      try { pos = Number(video.currentTime) || 0; playing = video.paused === false; } catch (err) { pos = 0; }
      ticketWith(item, t).then(({ got, token }) => {
        e.renewing = false;
        if (tickets.get(item.id) !== e) return;
        Object.assign(e, { url: got.path, at: now(), expiresAt: now() + got.expiresIn * 1000, token });
        video.src = got.path;
        try { video.currentTime = pos; } catch (err) { /* the browser seeks once it can */ }
        if (playing && typeof video.play === 'function') { try { Promise.resolve(video.play()).catch(() => {}); } catch (err) { /* it waits for a press */ } }
      }, (err) => {
        e.renewing = false;
        if (tickets.get(item.id) !== e) return;
        // The token gone and none to take its place yet: waiting for one (ticketFor's rule).
        if (tokenUnknown(err) && !c.token()) { tickets.delete(item.id); renderIfChanged(); return; }
        Object.assign(e, { state: 'error', token: t, why: W.fileFailed + (err && err.message ? err.message : String(err)) });
        renderIfChanged();
      });
    }
    function saveFile(url, name) {
      if (typeof env.save === 'function') { env.save(url, name); return; }
      const a = h('a', { href: url, hidden: true, attrs: { download: name } });
      (env.document.body || env.root).appendChild(a);
      a.click();
      a.remove();
    }

    /**
     * Download file `f` of `item` through the counting route. A public file is saved from that
     * address; a members-only one through a ticket made for it (its own, never the viewer's, so a
     * video playing beside it is never drawn again).
     */
    async function download(item, f) {
      const key = item.id + ':' + f.file;
      // Saved under its plain name too: a direction mark in a saved file's name hides its ending there.
      const name = shownName(item, f);
      if (!lib.fileRoute(item, env.origin).members) { saveFile(downloadPath(item.id, f.file), name); return; }
      const t = c.token();
      if (!t || asking.has(key)) return;
      asking.add(key);
      downloadErrors.delete(key);
      renderInfo();
      let got = null;
      try {
        got = (await ticketWith(item, t, f.file)).got;
      } catch (err) {
        downloadErrors.set(key, W.fileFailed + (err && err.message ? err.message : String(err)));
      }
      asking.delete(key);
      // The page closed (or another item opened) meanwhile: nothing is saved.
      if (got && st.viewer && st.viewer.id === item.id) saveFile(got.path, name);
      renderInfo();
    }

    /** Count a view (unsigned): the token in its header only for a members-only item. */
    function countView(item) {
      const headers = { 'Content-Type': 'application/json' };
      const t = item.visibility === 'members' ? c.token() : '';
      if (t) headers['X-Upload-Token'] = t;
      try {
        Promise.resolve(env.fetch('/api/media/viewed', { method: 'POST', headers, body: JSON.stringify({ id: item.id }) })).catch(() => {});
      } catch (e) { /* a view that is not counted is not worth a word */ }
    }

    // ── View in 3D (docs/design/model-viewer.md) ──
    /**
     * The name of file `f` that says it is a 3D model the viewer opens (`.glb` or `.stl`): the
     * uploader's own name for it, its stored name, or the end of its address; '' when none does.
     */
    function modelName(f) {
      if (!MV || !f) return '';
      const last = String(f.url || '').split('?')[0].split('/').pop();
      for (const n of [f.original_name, f.filename, last]) if (typeof n === 'string' && MV.isModelFile(n)) return n.trim();
      return '';
    }
    /**
     * File `f`'s answer from the file's own address: a public file's /uploads/ one as it is, a
     * members-only one from the members' file route with this session's token in its header
     * (exactly as a members-only picture is fetched, `fetchBlob` in media-app.js). Never the
     * counting route: looking at a model, or listening to a sound here, is not a download.
     */
    async function ownFileAnswer(f, signal) {
      const route = lib.fileRoute(f, env.origin);
      if (route.why) throw new Error(route.why);
      const once = async (t) => {
        const init = signal ? { signal } : {};
        if (t) init.headers = { 'X-Upload-Token': t };
        const res = await env.fetch(route.public || route.members, init);
        if (!res || !res.ok) await lib.answerOf(res); // throws the relay's words
        return res;
      };
      let res;
      if (!route.members) res = await once('');
      else {
        const t = c.token();
        if (!t) throw new Error('this page is not signed in to this server yet');
        try {
          res = await once(t);
        } catch (err) {
          // A token the chat lent that the server no longer knows: the chat's new one, tried once.
          const again = tokenUnknown(err) && typeof c.tokenRefused === 'function' ? c.tokenRefused(t) : null;
          const next = again ? await again : '';
          if (!next || next === t) throw err;
          res = await once(next);
        }
      }
      return res;
    }
    /** File `f`'s bytes for the 3D viewer: at most 200 MB, refused while reading too (the page's size could be out of date). */
    async function modelBytes(f, signal) {
      return MV.readBody(await ownFileAnswer(f, signal));
    }
    /** A sound's bytes for its view in the middle pane (media-sound.js), as a Blob: its own address, as a model's. */
    async function soundBlob(f, signal) {
      const res = await ownFileAnswer(f, signal);
      let declared = NaN;
      try { if (res.headers && typeof res.headers.get === 'function') declared = Number(res.headers.get('content-length')); } catch (e) { declared = NaN; }
      // Larger than the view plays: its size alone, which the view refuses in its own words, unread.
      if (MS && Number.isFinite(declared) && declared > MS.MAX_SOUND_BYTES) return { size: declared };
      return res.blob();
    }
    /** Open the 3D view of file `f` (whose model name is `as`) in place of the cover. */
    function openModel(item, f, as) {
      const V = viewer();
      if (!V || typeof V.open !== 'function') return;
      const m = page.model;
      if (m && m.itemId === item.id && m.file === f.file) { reveal(m.box); return; } // already open: nothing fetched again
      closeModel(true);
      const name = shownName(item, f);
      const host = h('div', { id: 'media-model-view', className: 'media-model-view' });
      // In the middle pane, as wide as it is (2026-10-19; it was a window's width).
      const box = h('div', {
        id: 'media-model', className: 'media-viewer-model', dataset: { modelFile: f.file },
        attrs: { style: 'width:100%;max-width:100%;margin:0 auto' },
      }, [
        h('div', { className: 'media-model-head', attrs: { style: 'display:flex;flex-wrap:wrap;gap:var(--space-sm);align-items:center;justify-content:space-between;margin-bottom:var(--space-sm)' } }, [
          h('span', { className: 'media-viewer-name', text: name }),
          h('button', { type: 'button', id: 'media-model-close', className: 'media-btn media-btn-small', text: MV.WORDS.close, on: { click: () => closeModel() } }),
        ]),
        host,
      ]);
      page.model = { itemId: item.id, file: f.file, box, handle: null };
      // The face is drawn again with the view in it, then the viewer starts in its place.
      renderIfChanged();
      const handle = V.open(host, {
        name, fileName: as, size: f.size_bytes, sizeWords: lib.sizeWords,
        fetchBytes: (signal) => modelBytes(f, signal),
      });
      if (page.model && page.model.box === box) page.model.handle = handle;
      else if (handle && typeof handle.close === 'function') handle.close();
      reveal(box);
    }
    /** Close the 3D view (its fetch stopped, its GPU memory given back); the cover comes back. */
    function closeModel(quiet) {
      const m = page.model;
      if (!m) return;
      page.model = null;
      if (m.handle && typeof m.handle.close === 'function') { try { m.handle.close(); } catch (e) { /* already closed */ } }
      if (!quiet) renderIfChanged();
    }
    /** Scroll `el` into view: the button is further down the page than the view it opens. */
    function reveal(el) {
      try { if (el && typeof el.scrollIntoView === 'function') el.scrollIntoView({ block: 'nearest' }); } catch (e) { /* it is still there */ }
    }

    /** A signed write on the item, naming its number and its file (W5): its answer, or throws Error(the relay's words). */
    async function send(path, purpose, item, body) {
      const filename = lib.itemFileName(item);
      if (!filename) throw new Error(W.noFileName);
      const auth = await env.sign(purpose);
      if (!auth) throw new Error(W.cannotSign);
      return lib.answerOf(await env.fetch(path, lib.jsonPost(Object.assign({ id: item.id, filename }, body || {}, auth))));
    }

    /** Put `item` wherever the page shows it: the grid, the shelf and this page. */
    function takeItem(item) {
      if (!item || item.id == null) return;
      st.items = st.items.map((i) => (i.id === item.id ? item : i));
      st.featured = st.featured.map((i) => (i.id === item.id ? item : i));
      if (st.viewer && st.viewer.id === item.id) st.viewer = item;
      c.rerender();
      renderIfChanged();
      renderInfo();
      if (page.edit) page.edit.drawFiles();
    }

    // ── Opening and closing ──
    /** Does the Edit form hold changes Save would send, or a file's new name typed and not saved? */
    function unsaved() {
      return !!(page.edit && typeof page.edit.changed === 'function' && page.edit.changed());
    }
    /**
     * Back or Forward took the open item's address away while its Edit held changes, and the person
     * chose to keep them: its address put back, as a new step (so closing it later goes Back there).
     */
    function addressBack() {
      if (!st.viewer || !c.canPush) return;
      c.go('/media/' + st.viewer.id, true);
      st.pushed = true;
    }
    /** Open an item. `fromAddress`: the address names it already (a load of /media/<id>, Back, Forward). */
    function open(item, fromAddress) {
      // Another item while the Edit form holds changes: asked first (the desktop's Discard::Open).
      if (item && st.viewer && st.viewer.id !== item.id && unsaved()) {
        ask(() => open(item, fromAddress), fromAddress === true ? addressBack : null);
        return;
      }
      // A ticket refused earlier is asked for afresh when the item is opened again.
      const was = item && tickets.get(item.id);
      if (was && was.state === 'error') tickets.delete(item.id);
      const another = !st.viewer || !item || st.viewer.id !== item.id;
      if (item && fromAddress !== true) {
        // Opened from the list: a new step in the history, so Back closes it.
        if (!st.viewer) st.pushed = !!c.canPush;
        c.go('/media/' + item.id, !st.viewer);
      }
      if (another) { page.status = ''; page.copied = ''; closeEdit(true); closeModel(true); closeSound(); }
      st.viewer = item;
      if (item && another) countView(item);
      renderIfChanged();
      if (typeof c.selected === 'function') c.selected();
    }
    /**
     * Close the item. Its address goes too: Back when it was pushed here, else replaced by /media.
     * With the Edit form holding changes it asks first, unless `force` (the item was removed).
     */
    function close(fromAddress, force) {
      if (!force && st.viewer && unsaved()) {
        ask(() => close(fromAddress, true), fromAddress === true ? addressBack : null);
        return;
      }
      const was = !!st.viewer;
      if (st.viewer && fromAddress !== true) {
        if (st.pushed) { st.pushed = false; c.back(); } else c.go('/media', false);
      }
      st.viewer = null;
      closeEdit(true);
      closeModel(true);
      closeSound();
      renderIfChanged();
      c.sweepBlobs();
      if (typeof c.selected === 'function') c.selected();
      if (was && typeof c.closed === 'function') c.closed();
    }

    // ── The question before an edit's changes are let go (the desktop's Discard) ──
    /** Ask before letting the Edit form's changes go: `then` once they are discarded, `keep` (optional) once kept. */
    function ask(then, keep) {
      page.asking = { then, keep: keep || null };
      renderAsk();
    }
    /** The person's answer: discard the changes (true), or keep editing (false). */
    function answer(discard) {
      const a = page.asking;
      if (!a) return;
      page.asking = null;
      renderAsk();
      if (discard) {
        closeEdit();
        if (typeof a.then === 'function') a.then();
      } else if (typeof a.keep === 'function') a.keep();
    }
    function renderAsk() {
      const box = els.ask;
      if (!box) return;
      box.replaceChildren();
      if (!page.asking) return;
      const now = st.viewer ? current(st.viewer.id) || st.viewer : null;
      const title = now ? lib.itemName(now, st.kinds) : '';
      const keep = h('button', { type: 'button', id: 'media-discard-keep', className: 'media-btn', text: WORDS.keepEditing, on: { click: () => answer(false) } });
      const overlay = h('div', {
        id: 'media-discard', className: 'media-viewer',
        attrs: { role: 'alertdialog', 'aria-modal': 'true', 'aria-labelledby': 'media-discard-title', 'aria-describedby': 'media-discard-words' },
      }, [
        h('div', { className: 'media-viewer-box media-ask-box' }, [
          h('h2', { id: 'media-discard-title', className: 'media-ask-title', text: WORDS.discardTitle }),
          h('p', { id: 'media-discard-words', className: 'media-ask-words', text: discardWords(title) }),
          h('div', { className: 'media-form-buttons' }, [
            h('button', { type: 'button', id: 'media-discard-yes', className: 'media-btn media-btn-danger', text: WORDS.discardThem, on: { click: () => answer(true) } }),
            keep,
          ]),
        ]),
      ]);
      // A click beside it keeps editing, as Escape does (media-app.js): nothing is lost by a slip.
      overlay.addEventListener('click', (e) => { if (e.target === overlay) answer(false); });
      box.appendChild(overlay);
      try { keep.focus(); } catch (e) { /* it can still be clicked */ }
    }

    /** A labelled item not yet clicked: its label instead of its file, nothing fetched. */
    function veiled(item) {
      return lib.labelsOf(item, st.labels).length > 0 && !st.revealed.has(item.id);
    }
    /**
     * Is the item a sound or a song: its own file's type, by the file-type list
     * (data/safety/file_risks.json names the endings; the desktop's viewer.rs is_sound)? The list is
     * asked for the first time it is needed; until it is in, no.
     */
    function isSound(item) {
      if (!MS || !typesReady()) return false;
      return FR.fileType(shownName(item, filesOf(item)[0]), null).id === 'sound';
    }
    /** What the face shows, as a key: the item, whether it is veiled, and where a members-only file's loading is. */
    function key() {
      const item = st.viewer;
      if (!item) return 'none';
      const route = lib.fileRoute(item, env.origin);
      let file = route.why ? 'missing' : route.public || '';
      if (route.members) {
        // A picture's object URL, or a ticket's state (not its address: a ticket renewed under a
        // playing video does not draw the video again). One refused under an earlier sign-in is
        // asked for again under the new one.
        const e = item.kind === 'picture' ? c.blobs.get(item.id) : tickets.get(item.id);
        const stale = e && item.kind !== 'picture' && e.state === 'error' && e.token !== c.token();
        file = e ? (stale ? 'stale' : e.state) + ':' + (item.kind === 'picture' ? e.url : '') : c.token() ? 'members' : 'no-token';
      }
      // A file or a model shows its cover: where the cover's loading is, too.
      const cover = item.kind !== 'picture' && item.kind !== 'video' ? coverOf(item) : null;
      const coverAt = cover ? cover.url + ':' + ((c.blobs.get(cover.id) || {}).state || '') + ':' + ((c.blobs.get(cover.id) || {}).url || '') : '';
      // The 3D view open in the cover's place, and which file it shows.
      const model = page.model && page.model.itemId === item.id ? 'model:' + page.model.file : '';
      // Left out by the protected setup, and with which line.
      const hid = hiddenLine(item);
      // A sound (by its type, once the list is in) shows its waveform's view.
      return [item.id, lib.itemFileName(item), veiled(item) ? 'veiled' : 'shown', file, coverAt, model, hid === null ? '' : 'hidden:' + hid, isSound(item) ? 'sound' : ''].join('|');
    }
    function renderIfChanged() {
      closeIfHidden();
      const k = key();
      if (k === st.viewerKey) return;
      render();
      // Drawing it may have asked for its file (its state is then part of the key).
      st.viewerKey = key();
    }

    // ── The middle pane: the item to fit (the desktop's viewer.rs) ──
    /** A sound's view for `item` (media-sound.js): the same one while it is shown, so it plays on through a redraw. */
    function soundFor(item) {
      if (page.sound && page.sound.itemId === item.id) return page.sound.view;
      closeSound();
      const own = filesOf(item)[0];
      const view = MS.createSoundView({
        h, doc: env.document, win: env.window || null, URL: env.URL || (typeof URL !== 'undefined' ? URL : null),
        name: shownName(item, own), size: Number(own.size_bytes) || Number(item.size_bytes) || 0, sizeWords: lib.sizeWords,
        fetchSound: (signal) => soundBlob(own, signal),
        decode: typeof env.decodeAudio === 'function' ? env.decodeAudio : undefined,
        AudioContext: env.AudioContext, Abort: env.AbortController,
      });
      page.sound = { itemId: item.id, view };
      return view;
    }
    /** Stop the sound shown and let it go (its fetch, its sound, its address). */
    function closeSound() {
      const s = page.sound;
      if (!s) return;
      page.sound = null;
      try { s.view.close(); } catch (e) { /* already closed */ }
    }

    /**
     * The middle pane's parts: the picture or the video to fit; a sound's cover and its waveform's
     * view; a model's or another file's cover (or its type), its name and size, its word of caution,
     * View in 3D and Download; the 3D view in the cover's place while it is open.
     */
    function faceParts(item) {
      // Left out by the protected setup: its line instead, and nothing fetched (the desktop's
      // Show::Hidden). Checked before the 3D view, which a hidden item never shows.
      const hid = hiddenLine(item);
      if (hid !== null) return [h('p', { className: 'media-hint media-viewer-hidden', id: 'media-viewer-hidden', text: hid })];
      // The 3D view, when one is open: the same element each time, so the model drawn in it (and
      // its WebGL context) stays while the rest of the page is drawn again around it.
      if (page.model && page.model.itemId === item.id) return [page.model.box];
      const name = lib.itemName(item, st.kinds);
      const route = lib.fileRoute(item, env.origin);
      if (route.why) return [h('p', { className: 'media-warn', text: route.why })];
      if (veiled(item)) {
        // The label cover, as on its tile (/media/<n> opens the page directly): nothing is fetched
        // until it is clicked.
        return [h('button', {
          type: 'button', id: 'media-viewer-veil', className: 'media-thumb media-veil media-viewer-veil', dataset: { veil: item.id },
          on: { click: () => { st.revealed.add(item.id); renderIfChanged(); renderInfo(); c.rerender(); } },
        }, [
          h('span', { className: 'media-veil-labels', text: lib.labelsOf(item, st.labels).map((l) => l.label).join(', ') }),
          h('span', { className: 'media-veil-show', text: W.clickToShow }),
        ])];
      }
      if (item.kind === 'picture' || item.kind === 'video') {
        let src = route.public || '';
        let why = '';
        let ticketed = false;
        if (route.members) {
          // A picture through its object URL; a video through a ticket, streamed by the browser.
          const e = item.kind === 'picture' ? c.blobs.get(item.id) || (c.token() ? c.blobFor(item) : null) : ticketFor(item);
          if (e && e.state === 'ready') { src = e.url; ticketed = item.kind === 'video'; } else if (e && e.state === 'error') why = e.why;
          else why = c.token() ? W.loadingFile + ' ' + lib.sizeWords(item.size_bytes) : W.waitingToken;
        }
        if (!src) return [h('p', { className: 'media-viewer-wait', id: 'media-viewer-wait', text: why })];
        if (item.kind === 'picture') {
          // As large as fits, its whole self; View full size opens it at its own size.
          return [
            h('img', { className: 'media-viewer-img media-face-fit', src, alt: name }),
            h('div', { className: 'media-face-buttons' }, [h('a', {
              href: src, target: '_blank', rel: 'noopener', id: 'media-view-full', className: 'media-btn media-btn-small',
              text: WORDS.viewFull, title: WORDS.viewFullTip,
            })]),
          ];
        }
        // A public cover is the video's poster.
        const cover = coverOf(item);
        const poster = cover && lib.fileRoute(cover, env.origin).public;
        const video = h('video', { className: 'media-viewer-video media-face-fit', src, controls: true, preload: 'metadata', poster: poster || undefined });
        if (ticketed) video.addEventListener('error', () => renewTicket(item, video));
        return [video];
      }
      const cover = coverOf(item);
      const where = cover ? c.pictureSource(cover) : null;
      const coverImg = where && where.src ? h('img', { className: 'media-viewer-img media-face-fit', src: where.src, alt: name }) : null;
      // A sound or a song: its cover above, then Play here (and, once pressed, its waveform).
      if (isSound(item)) {
        if (coverImg) coverImg.className = 'media-viewer-img media-face-cover';
        return [coverImg, soundFor(item).el].filter(Boolean);
      }
      // A model or any other file: its cover (or its type, large), what it is, its name and size, its
      // word of caution, then View in 3D (a model the viewer opens) and Download.
      const own = filesOf(item)[0];
      const ownName = shownName(item, own);
      const typed = FR && typeof FR.fileType === 'function' && typesReady() ? FR.fileType(ownName, null) : null;
      const kind = item.kind === 'file' ? (typed ? typed.label : 'File') : lib.kindWords(item.kind, st.kinds).one;
      const ownAt = lib.fileRoute(own, env.origin);
      const as = modelName(own);
      const risk = riskOf(ownName);
      const buttons = [];
      if (as && !ownAt.why) {
        buttons.push(h('button', {
          type: 'button', id: 'media-face-view3d', className: 'media-btn media-btn-primary',
          title: MV.fill(MV.WORDS.viewTip, { size: lib.sizeWords(own.size_bytes) }), text: MV.WORDS.view,
          disabled: !!ownAt.members && !c.token(), on: { click: () => openModel(item, own, as) },
        }));
      }
      buttons.push(h('button', {
        type: 'button', id: 'media-face-download', className: 'media-btn', title: WORDS.downloadTip, text: WORDS.download,
        disabled: !!route.members && !c.token(), on: { click: () => download(item, own) },
      }));
      return [h('div', { className: 'media-viewer-file media-face-file' }, [
        coverImg || h('div', { className: 'media-face-mark' }, [ED.fileBadge(h, env.document, ownName)]),
        h('div', { className: 'media-face-kind', text: kind }),
        h('div', { className: 'media-viewer-name', text: ownName + ', ' + lib.sizeWords(own.size_bytes || item.size_bytes) }),
        risk ? h('p', { className: 'file-caution' + (risk.disguised ? ' disguised' : ''), id: 'media-face-caution', text: risk.words }) : null,
        h('div', { className: 'media-form-buttons media-face-buttons' }, buttons),
      ])];
    }

    /** The handle between the two panes, from the page's panes (none without them). */
    function handle() {
      return typeof c.handle === 'function' ? c.handle() : null;
    }

    /** Edit mode drawn on the right pane: the details hidden, the form shown, the pane's moving border. */
    function editMode() {
      const editing = !!page.edit;
      if (page.infoBox) page.infoBox.hidden = editing;
      const pane = page.detailsPane;
      if (pane) {
        pane.className = 'media-pane media-details-pane' + (editing ? ' media-pane-editing' : '');
        if (editing) pane.dataset.editing = '1'; else delete pane.dataset.editing;
      }
      if (typeof c.editChanged === 'function') c.editChanged();
    }

    function render() {
      els.viewer.replaceChildren();
      page.infoBox = null;
      page.facePane = null;
      page.detailsPane = null;
      // The tab's title as the relay writes it for the item's page: "Dusk, Media on <host>".
      env.document.title = (st.viewer ? lib.itemName(st.viewer, st.kinds) + ', ' : '') + c.mediaOn;
      const item = st.viewer;
      if (!item) {
        // Nothing chosen: the two panes say what they will hold (the Media page's; the item window
        // shows nothing then, and closes).
        if (c.placeholders) {
          els.viewer.append(...[
            h('div', { id: 'media-choose', className: 'media-pane media-face-pane media-pane-empty', attrs: { role: 'region', 'aria-label': WORDS.faceLabel } }, [
              h('p', { className: 'media-hint media-pane-line', text: MP ? MP.WORDS.choose : '' }),
            ]),
            handle(),
            h('div', { id: 'media-details-empty', className: 'media-pane media-details-pane media-pane-empty', attrs: { role: 'region', 'aria-label': WORDS.detailsLabel } }, [
              h('p', { className: 'media-hint media-pane-line', text: MP ? MP.WORDS.details : '' }),
            ]),
          ].filter(Boolean));
        }
        return;
      }
      page.infoBox = h('div', { id: 'media-item-info', className: 'media-item-info' });
      page.editBox = page.editBox || h('div', { id: 'media-item-edit', className: 'media-item-edit' });
      // The middle pane: the item to fit.
      page.facePane = h('div', { id: 'media-face', className: 'media-pane media-face-pane', attrs: { role: 'region', 'aria-label': WORDS.faceLabel } }, faceParts(item));
      // The right pane: Close, then everything about it, or its Edit form in its place.
      page.detailsPane = h('div', { id: 'media-details', className: 'media-pane media-details-pane', attrs: { role: 'region', 'aria-label': WORDS.detailsLabel } }, [
        h('div', { className: 'media-details-head' }, [
          h('button', { type: 'button', id: 'media-viewer-close', className: 'media-btn media-btn-small media-viewer-close', text: WORDS.close, title: WORDS.closeTip, on: { click: () => close() } }),
        ]),
        page.infoBox,
        page.editBox,
      ]);
      els.viewer.appendChild(h('div', { id: 'media-viewer-open', className: 'media-item-panes', dataset: { item: item.id } },
        [page.facePane, handle(), page.detailsPane].filter(Boolean)));
      editMode();
      renderInfo();
    }

    /** Everything on the page under the face: drawn again whenever the item changes. */
    function renderInfo() {
      const box = page.infoBox;
      const item = st.viewer;
      if (!box || !item) return;
      // An open 3D view the protected setup now hides is closed, and the face drawn again (which
      // draws this part again too).
      if (closeIfHidden()) { renderIfChanged(); return; }
      env.document.title = lib.itemName(item, st.kinds) + ', ' + c.mediaOn;
      const route = lib.fileRoute(item, env.origin);
      const L = st.licences || {};
      const badges = lib.labelsOf(item, st.labels).map((l) => h('span', { className: 'media-badge media-badge-label', text: l.label }));
      if (item.kept) badges.push(h('span', { className: 'media-badge media-badge-kept', text: W.keptBadge }));
      if (item.visibility === 'members') badges.push(h('span', { className: 'media-badge', text: 'Members only' }));
      if (item.featured === 'main' || item.featured === 'category') badges.push(h('span', { className: 'media-badge media-badge-kept', text: item.featured === 'main' ? 'Featured' : 'Featured in its category' }));
      const kids = [
        h('h2', { className: 'media-viewer-title', text: lib.itemName(item, st.kinds) }),
        h('p', { className: 'media-viewer-meta', id: 'media-item-by', text: byLine(item) }),
        badges.length ? h('div', { className: 'media-badges' }, badges) : null,
        h('p', { className: 'media-viewer-meta', id: 'media-item-counts', text: countsLine(item) }),
        // The day it was put on the page (the desktop's since_line), when the server said.
        sinceLine(item) ? h('p', { className: 'media-viewer-meta', id: 'media-item-since', text: sinceLine(item) }) : null,
        // The whole description, as plain text: its own line breaks are kept (white-space: pre-wrap).
        item.description && item.description.trim() ? h('p', { className: 'media-viewer-desc', id: 'media-item-description', text: item.description.trim() }) : null,
      ];
      // The licence by name (linked to its deed), the server's own line for it, and the game's line.
      const id = item.licence || L.default || 'all_rights_reserved';
      const known = ED.offeredLicence(L, id);
      const name = (known && known.name) || id;
      kids.push(h('p', { className: 'media-licence-line', id: 'media-item-licence' }, [
        'Licence: ',
        known && /^https:\/\//.test(known.url || '')
          ? h('a', { href: known.url, target: '_blank', rel: 'noopener noreferrer', text: name })
          : h('span', { text: name }),
      ]));
      if (known && known.line) kids.push(h('p', { className: 'media-hint', id: 'media-item-licence-words', text: known.line }));
      if (item.project_use) {
        const line = L.game_offer && L.game_offer.line ? L.game_offer.line : WORDS.offeredForGame;
        kids.push(h('p', { className: 'media-hint media-item-game', id: 'media-item-game', text: line }));
      }
      // The files, the item's own first, then the cover, each with its counted Download.
      const files = filesOf(item);
      const list = h('div', { id: 'media-item-files', className: 'media-item-files' }, [h('h3', { text: files.length === 1 ? 'File' : 'Files' })]);
      const rows = files.map((f) => [f, false]).concat(item.cover ? [[item.cover, true]] : []);
      const may = !route.why && !veiled(item);
      const hidden = hiddenLine(item);
      for (const [f, cover] of rows) {
        const k = item.id + ':' + f.file;
        const busy = asking.has(k);
        // View in 3D beside the Download of a .glb or .stl file on this server (never the cover).
        // Not where the protected setup leaves the item out: a model's textures are pictures too.
        const as = may && !cover && hidden === null ? modelName(f) : '';
        const fileAt = as ? lib.fileRoute(f, env.origin) : null;
        // Its type first, always (2026-10-17, media-edit.js fileBadge): the badge says what kind of
        // file it is, so the row says only "name, size" ("name, cover picture, size" for the cover).
        // Both by its plain name (shownName): a name cannot hide its real ending.
        const named = shownName(item, f);
        list.appendChild(h('div', { className: 'media-item-file-row', dataset: { file: f.file } }, [
          ED.fileBadge(h, env.document, named),
          h('span', { className: 'media-item-file-name', text: named + (cover ? ', cover picture, ' : ', ') + lib.sizeWords(f.size_bytes) }),
          may ? h('button', {
            type: 'button', className: 'media-btn media-btn-small media-btn-primary', id: f.file ? 'media-download-' + f.file : 'media-download',
            dataset: { download: f.file }, title: WORDS.downloadTip, text: busy ? W.loadingFile : WORDS.download,
            disabled: busy || (!!route.members && !c.token()), on: { click: () => download(item, f) },
          }) : null,
          fileAt && !fileAt.why ? h('button', {
            type: 'button', className: 'media-btn media-btn-small', id: f.file ? 'media-view3d-' + f.file : 'media-view3d',
            dataset: { view3d: f.file }, title: MV.fill(MV.WORDS.viewTip, { size: lib.sizeWords(f.size_bytes) }),
            text: MV.WORDS.view, disabled: !!fileAt.members && !c.token(), on: { click: () => openModel(item, f, as) },
          }) : null,
        ]));
        // What kind of file it is, under its row, when that deserves a word (by its name).
        const risk = riskOf(named);
        if (risk) {
          list.appendChild(h('p', {
            className: 'file-caution' + (risk.disguised ? ' disguised' : ''), id: f.file ? 'media-file-caution-' + f.file : 'media-file-caution',
            dataset: { fileRisk: risk.kind }, text: risk.words,
          }));
        }
        if (downloadErrors.has(k)) list.appendChild(h('p', { className: 'media-warn', id: f.file ? 'media-download-error-' + f.file : 'media-download-error', text: downloadErrors.get(k) }));
      }
      if (mayEdit(item) && (files.length > 1 || item.cover)) list.appendChild(h('p', { className: 'media-hint', text: WORDS.editHint }));
      kids.push(list);
      // Copy link and Edit, then the tile's own actions.
      const buttons = [h('button', { type: 'button', id: 'media-copy-link', className: 'media-btn', text: WORDS.copyLink, title: WORDS.copyTip, on: { click: () => copyLink(item) } })];
      if (mayEdit(item)) buttons.push(h('button', { type: 'button', id: 'media-item-edit-open', className: 'media-btn media-btn-primary', text: WORDS.edit, title: WORDS.editTip, on: { click: () => openEdit(item) } }));
      kids.push(h('div', { className: 'media-form-buttons media-item-buttons' }, buttons.concat([c.actions(item)])));
      // "Report a copyright problem" (2026-10-13, media-takedown.js): for anyone, signed in or not.
      if (typeof c.openTakedown === 'function') {
        kids.push(h('p', { className: 'media-hint media-item-takedown' }, [h('button', {
          type: 'button', id: 'media-item-takedown', className: 'media-btn media-btn-small', text: WORDS.takedown,
          title: WORDS.takedownTip, on: { click: () => c.openTakedown(item) },
        })]));
      }
      if (page.copied) kids.push(h('p', { className: 'media-hint', id: 'media-copied', text: page.copied }));
      if (page.status) kids.push(h('p', { className: 'media-hint', id: 'media-item-status', text: page.status }));
      box.replaceChildren(...kids.filter(Boolean));
    }

    /** Copy the item's own address (`https://<host>/media/<id>`) and say so under the button. */
    function copyLink(item) {
      const link = itemLink(env.origin, item.id);
      let copied = false;
      try {
        if (typeof env.copy === 'function') { env.copy(link); copied = true; } else {
          const clip = root.navigator && root.navigator.clipboard;
          if (clip && typeof clip.writeText === 'function') { Promise.resolve(clip.writeText(link)).catch(() => {}); copied = true; }
        }
      } catch (e) { copied = false; }
      page.copied = (copied ? 'Copied: ' : "This item's address: ") + link;
      renderInfo();
    }

    // ── Edit (media-edit.js), in place of the details (2026-10-19) ──
    /** Turn the right pane into `item`'s Edit form (the item shown first when another, or none, is). */
    function openEdit(item) {
      if (!st.viewer || st.viewer.id !== item.id) open(item);
      // Another item's Edit holds changes: asked about first, and this one is shown once answered.
      if (!st.viewer || st.viewer.id !== item.id) return;
      if (!page.editBox || !mayEdit(item)) return;
      // Already editing it: the form stays as it is, with what was typed (the desktop's open_edit).
      if (page.edit) return;
      page.status = '';
      page.edit = ED.createEditForm(c, view, current(item.id) || item, page.editBox);
      editMode();
    }
    /** The pane is the item's details again (after Save, or the changes let go). */
    function closeEdit(quiet) {
      const was = !!page.edit;
      page.edit = null;
      if (page.editBox) page.editBox.replaceChildren();
      if (was) editMode();
      if (!quiet) renderInfo();
    }
    /** Cancel: straight back with nothing changed; with changes, asked first. */
    function cancelEdit() {
      if (unsaved()) ask(null, null);
      else closeEdit();
    }
    /** The form was typed in (media-edit.js): the item window tells the chat whether it holds changes. */
    function edited() {
      if (typeof c.editChanged === 'function') c.editChanged();
    }
    function say(words) {
      page.status = words || '';
      renderInfo();
    }
    /** An attaching upload moved on: the Edit form's rows follow it. */
    function uploadsChanged() {
      if (page.edit) page.edit.drawFiles();
    }

    const view = {
      open, close, renderIfChanged, renderInfo, takeItem, mayEdit, own, openEdit, closeEdit, cancelEdit, edited, unsaved, say, send, current,
      fileName, shownName, filesOf, uploadsChanged, tickets, page, hiddenLine, protectionChanged,
      // The question before changes are let go: open now, and its answer (Escape keeps editing).
      asking: () => !!page.asking, answer,
    };
    return view;
  }

  const api = { WORDS, LABEL_HIDDEN_LINE, hiddenBy, CARD_LINE_MAX, countsLine, byLine, plainName, firstLine, filesBadge, itemLink, downloadPath, filesOf, coverOf, createItemView, discardWords, sinceLine };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else root.MediaItem = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
