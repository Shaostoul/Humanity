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
// - its files, the item's own first, each with its name, kind, size and
//   Download. A Download goes through the relay's counting route,
//   /api/media/download/<id>?file=<n>, never the file's own address, so it is
//   counted; a members-only file's through a ticket asked for that route and
//   that file (`download: true`), which the browser then saves itself;
// - Copy link (`https://<host>/media/<id>`, the page the relay serves with its
//   preview for social sites), Edit (the uploader and admins: media-edit.js),
//   and the tile's own actions (Pick, Remove, Feature, Keep, Report);
// - "Report a copyright problem" (2026-10-13), for anyone, signed in or not:
//   media-takedown.js's form, about this item.
//
// Opening it counts a view (POST /api/media/viewed, unsigned; the session's
// upload token goes in its header only for a members-only item, which only a
// member's view counts). An item that changed (an edit, a file added or taken
// off) is changed wherever the page shows it (`takeItem`).
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
  });

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
  /** "by Ann in Fire flow arts" (the category left out when the server named none). */
  function byLine(item) {
    const by = 'by ' + ((item && item.uploader_name) || 'someone');
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
   * The item's page. `c`: env, h, st, els, W (the page's words), lib (its helpers), token(), myKey,
   * isAdmin(), membersOnlyServer(), blobs, blobFor(item), sweepBlobs(), go(path, push), canPush,
   * back(), rerender() (the grid and the shelf), actions(item) (a tile's actions, for this page),
   * attach(file, item, as) and uploadsFor(id) (media-app.js's resumable upload, attaching).
   */
  function createItemView(c) {
    const { env, h, st, els, W, lib } = c;
    const now = () => (typeof env.now === 'function' ? env.now() : Date.now());
    const tickets = new Map(); // item id -> {state: 'loading'|'ready'|'error', url, why, token, at, expiresAt, renewing}
    const asking = new Set(); // "<id>:<file>" while a Download's ticket is on its way
    const downloadErrors = new Map(); // "<id>:<file>" -> the words of a Download that could not start
    const page = { status: '', copied: '', edit: null, editBox: null, infoBox: null };

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
      fetchTicket(item, t).then((got) => {
        if (tickets.get(item.id) !== e) return;
        Object.assign(e, { url: got.path, at: now(), expiresAt: now() + got.expiresIn * 1000, state: 'ready' });
        renderIfChanged();
      }, (err) => {
        if (tickets.get(item.id) !== e) return;
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
      fetchTicket(item, t).then((got) => {
        e.renewing = false;
        if (tickets.get(item.id) !== e) return;
        Object.assign(e, { url: got.path, at: now(), expiresAt: now() + got.expiresIn * 1000, token: t });
        video.src = got.path;
        try { video.currentTime = pos; } catch (err) { /* the browser seeks once it can */ }
        if (playing && typeof video.play === 'function') { try { Promise.resolve(video.play()).catch(() => {}); } catch (err) { /* it waits for a press */ } }
      }, (err) => {
        e.renewing = false;
        if (tickets.get(item.id) !== e) return;
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
      const name = fileName(item, f);
      if (!lib.fileRoute(item, env.origin).members) { saveFile(downloadPath(item.id, f.file), name); return; }
      const t = c.token();
      if (!t || asking.has(key)) return;
      asking.add(key);
      downloadErrors.delete(key);
      renderInfo();
      let got = null;
      try {
        got = await fetchTicket(item, t, f.file);
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
    /** Open an item. `fromAddress`: the address names it already (a load of /media/<id>, Back, Forward). */
    function open(item, fromAddress) {
      // A ticket refused earlier is asked for afresh when the item is opened again.
      const was = item && tickets.get(item.id);
      if (was && was.state === 'error') tickets.delete(item.id);
      const another = !st.viewer || !item || st.viewer.id !== item.id;
      if (item && fromAddress !== true) {
        // Opened from the list: a new step in the history, so Back closes it.
        if (!st.viewer) st.pushed = !!c.canPush;
        c.go('/media/' + item.id, !st.viewer);
      }
      if (another) { page.status = ''; page.copied = ''; closeEdit(true); }
      st.viewer = item;
      if (item && another) countView(item);
      renderIfChanged();
    }
    /** Close the item. Its address goes too: Back when it was pushed here, else replaced by /media. */
    function close(fromAddress) {
      if (st.viewer && fromAddress !== true) {
        if (st.pushed) { st.pushed = false; c.back(); } else c.go('/media', false);
      }
      st.viewer = null;
      closeEdit(true);
      renderIfChanged();
      c.sweepBlobs();
    }
    /** A labelled item not yet clicked: its label instead of its file, nothing fetched. */
    function veiled(item) {
      return lib.labelsOf(item, st.labels).length > 0 && !st.revealed.has(item.id);
    }
    /** What the face shows, as a key: the item, whether it is veiled, and where a members-only file's loading is. */
    function key() {
      const item = st.viewer;
      if (!item) return '';
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
      return [item.id, lib.itemFileName(item), veiled(item) ? 'veiled' : 'shown', file, coverAt].join('|');
    }
    function renderIfChanged() {
      const k = key();
      if (k === st.viewerKey) return;
      render();
      // Drawing it may have asked for its file (its state is then part of the key).
      st.viewerKey = key();
    }

    /** The picture, the video, or (for a file or a model) its cover, else its name and size. */
    function face(item) {
      const name = lib.itemName(item, st.kinds);
      const route = lib.fileRoute(item, env.origin);
      if (route.why) return h('p', { className: 'media-warn', text: route.why });
      if (veiled(item)) {
        // The label cover, as on its tile (/media/<n> opens the page directly): nothing is fetched
        // until it is clicked.
        return h('button', {
          type: 'button', id: 'media-viewer-veil', className: 'media-thumb media-veil media-viewer-veil', dataset: { veil: item.id },
          on: { click: () => { st.revealed.add(item.id); renderIfChanged(); renderInfo(); c.rerender(); } },
        }, [
          h('span', { className: 'media-veil-labels', text: lib.labelsOf(item, st.labels).map((l) => l.label).join(', ') }),
          h('span', { className: 'media-veil-show', text: W.clickToShow }),
        ]);
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
        if (!src) return h('p', { className: 'media-viewer-wait', id: 'media-viewer-wait', text: why });
        if (item.kind === 'picture') return h('img', { className: 'media-viewer-img', src, alt: name });
        // A public cover is the video's poster.
        const cover = coverOf(item);
        const poster = cover && lib.fileRoute(cover, env.origin).public;
        const video = h('video', { className: 'media-viewer-video', src, controls: true, preload: 'metadata', poster: poster || undefined });
        if (ticketed) video.addEventListener('error', () => renewTicket(item, video));
        return video;
      }
      const cover = coverOf(item);
      const where = cover ? c.pictureSource(cover) : null;
      return h('div', { className: 'media-viewer-file' }, [
        where && where.src ? h('img', { className: 'media-viewer-img', src: where.src, alt: name }) : null,
        h('div', { className: 'media-viewer-name', text: name }),
        h('div', { className: 'media-viewer-size', text: lib.sizeWords(item.size_bytes) }),
      ]);
    }

    function render() {
      els.viewer.replaceChildren();
      page.infoBox = null;
      // The tab's title as the relay writes it for the item's page: "Dusk, Media on <host>".
      env.document.title = (st.viewer ? lib.itemName(st.viewer, st.kinds) + ', ' : '') + c.mediaOn;
      const item = st.viewer;
      if (!item) return;
      const name = lib.itemName(item, st.kinds);
      page.infoBox = h('div', { id: 'media-item-info', className: 'media-item-info' });
      page.editBox = page.editBox || h('div', { id: 'media-item-edit', className: 'media-item-edit' });
      const overlay = h('div', { id: 'media-viewer-open', className: 'media-viewer', attrs: { role: 'dialog', 'aria-label': name } }, [
        h('div', { className: 'media-viewer-box' }, [
          h('button', { type: 'button', id: 'media-viewer-close', className: 'media-btn media-viewer-close', text: 'Close', on: { click: () => close() } }),
          face(item),
          page.infoBox,
          page.editBox,
        ]),
      ]);
      overlay.addEventListener('click', (e) => { if (e.target === overlay) close(); });
      els.viewer.appendChild(overlay);
      renderInfo();
    }

    /** Everything on the page under the face: drawn again whenever the item changes. */
    function renderInfo() {
      const box = page.infoBox;
      const item = st.viewer;
      if (!box || !item) return;
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
      for (const [f, cover] of rows) {
        const k = item.id + ':' + f.file;
        const what = cover ? 'cover picture' : String(lib.kindWords(f.kind, st.kinds).one || f.kind || 'file').toLowerCase();
        const busy = asking.has(k);
        list.appendChild(h('div', { className: 'media-item-file-row', dataset: { file: f.file } }, [
          h('span', { className: 'media-item-file-name', text: fileName(item, f) + ', ' + what + ', ' + lib.sizeWords(f.size_bytes) }),
          may ? h('button', {
            type: 'button', className: 'media-btn media-btn-small media-btn-primary', id: f.file ? 'media-download-' + f.file : 'media-download',
            dataset: { download: f.file }, title: WORDS.downloadTip, text: busy ? W.loadingFile : WORDS.download,
            disabled: busy || (!!route.members && !c.token()), on: { click: () => download(item, f) },
          }) : null,
        ]));
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

    // ── Edit (media-edit.js) ──
    function openEdit(item) {
      if (!st.viewer || st.viewer.id !== item.id) open(item);
      if (!page.editBox || !mayEdit(item)) return;
      page.edit = ED.createEditForm(c, view, current(item.id) || item, page.editBox);
    }
    function closeEdit(quiet) {
      page.edit = null;
      if (page.editBox) page.editBox.replaceChildren();
      if (!quiet) renderInfo();
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
      open, close, renderIfChanged, renderInfo, takeItem, mayEdit, own, openEdit, closeEdit, say, send, current,
      fileName, filesOf, uploadsChanged, tickets, page,
    };
    return view;
  }

  const api = { WORDS, CARD_LINE_MAX, countsLine, byLine, firstLine, filesBadge, itemLink, downloadPath, filesOf, coverOf, createItemView };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else root.MediaItem = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
