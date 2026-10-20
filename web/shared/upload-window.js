// ── upload-window.js ──────────────────────────────────────────────────────
// The upload window on the website (2026-10-17): the Media page's own Add to Media,
// in a window over another page. The desktop app opens its upload window over
// any page when files are dropped (src/gui/drop_files.rs, media/add.rs
// upload_dropped); the website's chat has no Add to Media of its own, so a
// drop on one of this server's channels opens THIS window, and in it the Media
// page itself (/media?window=upload, web/pages/media-app.js in its upload-window
// form): the same form with every check it makes (the person's share, what is
// hidden in a picture and the switch, the licence), never a second copy of it.
// The operator, 2026-10-17: "user drag and drops file onto HumanityOS. First
// window popups with all the controls of what to do with the file ... If we're
// also in a chat channel ... upon clicking upload it also posts the image with
// the title and description to the general chat."
//
// Two halves, and the messages between them (each carries `kind: KIND`, and
// each side takes only messages from the other, on this same origin):
//   the host (the chat):  creates the window and its frame once, and hands
//                         the dropped files over: {event: 'files', files,
//                         postTo} (`postTo`: the channel to tick "Also post in
//                         chat" for, '' for none). Files are kept until the
//                         page in the frame says it is there.
//   the window (Media):   {event: 'ready'} once it can take files;
//                         {event: 'busy', busy} while an upload, a publish or
//                         a post is under way; {event: 'status', text} with
//                         each line its page says (published, posted in
//                         #general); {event: 'close'} when its form is
//                         cancelled, or Escape is pressed, with nothing going.
// Close (or Escape) on the host hides the window. While something is going
// its frame stays, hidden, so the upload carries on, and the lines it says
// are handed to `onStatus` (the chat shows them as notices); once nothing is
// going the frame is removed, its signed-in connection with it.
//
// Files are handed to the frame with postMessage, which copies a File within
// one origin without reading it. The frame is the site's own page (the relay's
// X-Frame-Options SAMEORIGIN and CSP frame-src 'self' allow exactly that).
//
// The item window (2026-10-17): the same window holding one Media item's own
// page (/media/<id>?window=item, web/pages/media-app.js in its item-window form),
// brought up over the chat by a Media card's Open (web/chat/chat-media-card.js;
// the operator: "when someone clicks on the file in chat it brings up the full
// popup window for that item"). The host is sent to it with showPage(page); the
// page in the frame says busy while an Edit's file is uploading, and close when
// its own Close (or Escape) is pressed. A window brought back on the page its
// frame already holds (hidden while its upload goes, after that Close) tells
// the page {event: 'open'}: it opens its item again, so the window is never
// empty (2026-10-17, review).
//
// Signing for the item window (2026-10-18). The Media page signs its reads and
// writes ("media_list\n<timestamp>" and the rest) with the copy of the key this
// browser keeps (/shared/pq-relay-auth.js); a passphrase-wrapped identity keeps
// no such copy, so a members-only item opened from a chat card listed unsigned
// and read "not there". So a host given `sign` (the chat's item windows,
// chat-media-card.js) signs for the page in its frame: the page asks
// {event: 'identity', ask} and {event: 'sign', ask, purpose}; the host answers
// {event: 'identity', ask, key} and {event: 'signed', ask, key, timestamp, sig},
// or {..., ask, refused: true} when the chat has no unlocked identity either (or
// the purpose is not one of the Media page's, or the frame has left the page it
// was given). Only the frame this host made may ask, from this site, while it
// is there; anyone else gets no answer at all. Only a Media purpose is signed (SIGNABLE):
// the chat signs its messages over the same "<words>\n<timestamp>", so it never
// signs words of the frame's choosing.
//
// The upload token for the item window (2026-10-18). A members-only item's files
// (its cover, the video's Play here, each Download, and the tickets those ask
// for) are fetched with the session's upload token, which a page gets from its
// own sign-in to the relay; the item window signed for by the chat has no
// sign-in of its own, so it saw the item and none of its files. So a host given
// `token` lends the page in its frame the chat's own upload token for this
// server (the chat signs in to the site it is on, and the frame is on that same
// site): the page asks {event: 'token', ask}, the host answers {event: 'token',
// ask, token}, or {..., ask, refused: true} when the chat holds no token (not
// signed in, its connection down) or the frame has left the page it was given.
// The same guards as signing: only the frame this host made, from this site,
// while it is on its page; anyone else gets no answer. The token goes to that
// frame alone (posted to it, for this origin only), never into an address; the
// page keeps it in memory for its life and asks again when the server says it
// no longer knows it (the chat signed in again: a token ends with its socket).
//
// A classic script in the browser (HosUploadWindow on window), a CommonJS module
// under Node, where scripts/tests/post-in-chat-web.test.js drives both halves
// with stand-in windows (and scripts/tests/media-card-web.test.js the item window).
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  /** Every message between the two halves carries this. */
  const KIND = 'hos-upload-window';
  /** The Files page (the Media page until 2026-10-19, still at /media) in its upload-window form. */
  const PAGE = '/media?window=upload';
  const WORDS = Object.freeze({
    title: 'Add to Files',
    itemTitle: 'Item on the Files page',
    close: 'Close',
    closeTip: 'Close this window. An upload under way carries on, and what happens to it is said in the chat.',
  });

  // What a host signs for the page in its frame: one of the Media page's purposes (media-app.js
  // `purposes`, media-edit.js, media-upload.js attachPurpose): "media_<name>", then words after
  // colons, none holding a space or a line break. Never a chat message's words.
  const SIGNABLE = /^media_[a-z_]+(?::[^\s:]+)*$/;
  const SIGNABLE_MAX = 512;
  /** Would a host sign `purpose` for the page in its frame? */
  function signable(purpose) {
    return typeof purpose === 'string' && purpose.length <= SIGNABLE_MAX && SIGNABLE.test(purpose);
  }
  // How long the page in the frame waits for its host's answer before it goes on unsigned: to sign
  // (a signature can take a moment on a slow device), and to be told who it signs as (answered at
  // once; the page waits for it before it is drawn).
  const ASK_WAIT_MS = 10000;
  const IDENTITY_WAIT_MS = 3000;
  // How long it waits for the chat's upload token (answered at once, like who it signs as).
  const TOKEN_WAIT_MS = 3000;
  // What an upload token may look like to be used: the relay's are 32 hex characters; anything with a
  // space, a line break or another character a header cannot carry is not one.
  const TOKEN_SHAPE = /^[\x21-\x7e]{1,256}$/;
  /** Could `t` be an upload token (one a request header can carry)? */
  function tokenShaped(t) {
    return typeof t === 'string' && TOKEN_SHAPE.test(t);
  }

  /** Is this the upload-window form of the Media page (its address's `window=upload`)? */
  function isWindowAddress(search) {
    try { return new URLSearchParams(String(search || '')).get('window') === 'upload'; } catch (e) { return false; }
  }

  /** Is this the item-window form of an item's page (its address's `window=item`)? */
  function isItemWindowAddress(search) {
    try { return new URLSearchParams(String(search || '')).get('window') === 'item'; } catch (e) { return false; }
  }

  /** Item `id`'s own page in its item-window form. */
  function itemPage(id) {
    return '/media/' + encodeURIComponent(String(id)) + '?window=item';
  }

  function originOf(win, given) {
    if (given) return given;
    try { return (win && win.location && win.location.origin) || ''; } catch (e) { return ''; }
  }

  /**
   * The host half, on the page that opens the window (the chat). `opts`:
   *   window, document   the host page's (its message and keydown listeners, and where the
   *                      window is drawn)
   *   origin             this site's origin (the window's own location.origin when not given)
   *   page               the address the frame opens (PAGE when not given)
   *   label              what the window is called to a screen reader (WORDS.title when not given)
   *   onStatus(text)     a line the page in the frame said while the window was closed
   *   sign(purpose)      -> Promise<{key, timestamp, sig}|null>: signs a Media purpose for the
   *                      page in the frame (the item window); optional, without it every ask
   *                      is refused
   *   myKey()            the key `sign` signs as ('' when there is no unlocked identity)
   *   token()            the chat's own upload token for this server ('' when it holds none: not
   *                      signed in, or its connection down), lent to the page in the frame for a
   *                      members-only item's files (the item window); optional, without it every
   *                      ask is refused
   * Returns {show(files, postTo), showPage(page, label), close(), isOpen(), isBusy(), page(),
   * element(), frame(), pending()}.
   */
  function createHost(opts) {
    const o = opts || {};
    const win = o.window || null;
    const doc = o.document || (win && win.document) || null;
    const origin = originOf(win, o.origin);
    let page = o.page || PAGE;
    let label = o.label || WORDS.title;
    let dialog = null;
    let frame = null;
    let ready = false;
    let busy = false;
    let open = false;
    const queue = []; // files waiting for the frame to say it is there

    function build() {
      if (dialog || !doc || typeof doc.createElement !== 'function') return dialog;
      dialog = doc.createElement('div');
      dialog.className = 'hos-upload-window';
      dialog.hidden = true;
      if (typeof dialog.setAttribute === 'function') {
        dialog.setAttribute('role', 'dialog');
        dialog.setAttribute('aria-modal', 'true');
        dialog.setAttribute('aria-label', label);
      }
      const panel = doc.createElement('div');
      panel.className = 'hos-upload-window-panel';
      const bar = doc.createElement('div');
      bar.className = 'hos-upload-window-bar';
      const shut = doc.createElement('button');
      shut.type = 'button';
      shut.className = 'hos-upload-window-close';
      shut.textContent = WORDS.close;
      shut.title = WORDS.closeTip;
      if (typeof shut.addEventListener === 'function') shut.addEventListener('click', () => close());
      bar.appendChild(shut);
      frame = doc.createElement('iframe');
      frame.className = 'hos-upload-window-frame';
      if (typeof frame.setAttribute === 'function') frame.setAttribute('title', label);
      frame.src = page;
      panel.appendChild(bar);
      panel.appendChild(frame);
      dialog.appendChild(panel);
      const body = doc.body || null;
      if (body && typeof body.appendChild === 'function') body.appendChild(dialog);
      return dialog;
    }

    function frameWindow() {
      return frame ? frame.contentWindow || null : null;
    }

    /** Hand the waiting files over, once the page in the frame is there. */
    function flush() {
      const to = frameWindow();
      if (!ready || !to) return;
      while (queue.length) {
        const job = queue.shift();
        try { to.postMessage({ kind: KIND, event: 'files', files: job.files, postTo: job.postTo }, origin); } catch (e) { /* the frame went away: nothing to hand to */ }
      }
    }

    /** Open the window with `files`, "Also post in chat" ticked for channel `postTo` ('' for none). */
    function show(files, postTo) {
      const list = Array.prototype.slice.call(files || []).filter(Boolean);
      if (!build()) return false;
      if (list.length) queue.push({ files: list, postTo: typeof postTo === 'string' ? postTo : '' });
      dialog.hidden = false;
      open = true;
      flush();
      try { if (frame && typeof frame.focus === 'function') frame.focus(); } catch (e) { /* not focusable yet */ }
      return true;
    }

    /**
     * Open the window on another page of this site (`to`: an item's own page, itemPage), called
     * `name`. A window showing another page is taken away first, unless something is going in
     * it: then nothing changes and the answer is false (make another host for the new page).
     */
    function showPage(to, name) {
      if (!to) return false;
      if (dialog && to !== page) {
        if (busy) return false;
        destroy();
      }
      // The frame is on this page already (shown, or hidden while something goes in it).
      const again = !!dialog;
      page = to;
      if (name) label = name;
      const shown = show([], '');
      // Its page may have closed what it showed (its own Close hid the window): open it again.
      if (shown && again) tell('open');
      return shown;
    }

    /** Tell the page in the frame `event` (it takes only its host's messages, from this site). */
    function tell(event) {
      const to = frameWindow();
      if (!to) return;
      try { to.postMessage({ kind: KIND, event }, origin); } catch (e) { /* the frame went away */ }
    }

    /** Take the window and its frame away (nothing is going in it). */
    function destroy() {
      if (dialog && typeof dialog.remove === 'function') dialog.remove();
      dialog = null;
      frame = null;
      ready = false;
      busy = false;
      open = false;
      queue.length = 0;
    }

    /** Close: hidden while something is going in it (it carries on), else gone. */
    function close() {
      if (!dialog) return;
      open = false;
      if (busy) dialog.hidden = true;
      else destroy();
    }

    function onMessage(e) {
      if (!e || !frame || e.source !== frameWindow() || (origin && e.origin !== origin)) return;
      const m = e.data;
      if (!m || typeof m !== 'object' || m.kind !== KIND) return;
      if (m.event === 'ready') { ready = true; flush(); }
      else if (m.event === 'busy') {
        busy = m.busy === true;
        // Closed while it was going, and now nothing is: nothing is left to keep it for.
        if (!busy && !open) destroy();
      } else if (m.event === 'close') close();
      else if (m.event === 'status' && !open && typeof m.text === 'string' && m.text && typeof o.onStatus === 'function') o.onStatus(m.text);
      else if (m.event === 'identity' || m.event === 'sign' || m.event === 'token') answerAsk(e.source, m);
    }

    /** Is the frame still on the page this host put in it (not navigated somewhere else)? */
    function onOwnPage(w) {
      try {
        const l = w && w.location;
        return !!l && String(l.pathname || '') + String(l.search || '') === page;
      } catch (e) {
        return false; // another site's page: its address cannot be read
      }
    }

    /**
     * Answer the page in the frame (`from`) asking who it signs as, to sign a purpose, or for the
     * chat's upload token (see the file's head). Nothing at all when the frame has gone or been
     * replaced by the time the answer is ready; a refusal when there is nothing to sign with, the
     * purpose is not a Media one, the chat holds no upload token, or the frame is no longer on the
     * page it was given (never silence to the frame itself: its page waits for the answer before it
     * is drawn).
     */
    async function answerAsk(from, m) {
      const ask = m.ask;
      if (!origin || !Number.isSafeInteger(ask)) return;
      let out = null;
      try {
        const me = typeof o.myKey === 'function' ? String(o.myKey() || '') : '';
        if (!onOwnPage(from)) {
          out = null; // the frame has left the page it was given: refused
        } else if (m.event === 'token') {
          // The chat's own token for this server, read now (it changes each time the chat signs in).
          const t = typeof o.token === 'function' ? String(o.token() || '') : '';
          if (tokenShaped(t)) out = { token: t };
        } else if (m.event === 'identity') {
          if (me && typeof o.sign === 'function') out = { key: me };
        } else if (me && typeof o.sign === 'function' && signable(m.purpose)) {
          const a = await o.sign(m.purpose);
          if (a && typeof a.key === 'string' && a.key && typeof a.sig === 'string' && a.sig && Number.isFinite(a.timestamp)) {
            out = { key: a.key, timestamp: a.timestamp, sig: a.sig };
          }
        }
      } catch (e) {
        out = null;
      }
      // Only while that frame is still there; refused when it left its page while this was signed.
      if (!frame || frameWindow() !== from) return;
      if (!onOwnPage(from)) out = null;
      const event = m.event === 'identity' ? 'identity' : m.event === 'token' ? 'token' : 'signed';
      const reply = Object.assign({ kind: KIND, event, ask }, out || { refused: true });
      try { from.postMessage(reply, origin); } catch (e) { /* the frame went away */ }
    }
    function onKey(e) {
      if (open && e && e.key === 'Escape') close();
    }

    if (win && typeof win.addEventListener === 'function') win.addEventListener('message', onMessage);
    if (doc && typeof doc.addEventListener === 'function') doc.addEventListener('keydown', onKey);

    return {
      show,
      showPage,
      close,
      isOpen: () => open,
      isBusy: () => busy,
      page: () => page,
      element: () => dialog,
      frame: () => frame,
      pending: () => queue.slice(),
    };
  }

  /**
   * The window's half, in the Media page opened in the frame. `opts`:
   *   window             the page's own window (its parent is the host)
   *   origin             this site's origin (the window's own location.origin when not given)
   *   onFiles(files, postTo)   the files the host handed over
   *   onOpen()           the host brought the window back on this page: open its item again
   *                      (the item window; also set later with onOpen(fn))
   *   setTimeout, clearTimeout   the window's own when not given (how long an ask waits)
   * Returns {send(event, data), ready(), onOpen(fn), identity(), sign(purpose), token()}, or null
   * when the page is not in a frame. identity() -> Promise<string>: the key the host signs as ('' when
   * it refused, or did not answer within IDENTITY_WAIT_MS); sign(purpose) -> Promise<{key, timestamp,
   * sig}|null>: the host's signature over "purpose\ntimestamp" (null when refused or unanswered);
   * token() -> Promise<string>: the chat's upload token for this server, lent to this page for a
   * members-only item's files ('' when refused, or unanswered within TOKEN_WAIT_MS). The caller keeps
   * it in memory only, and asks again when the server no longer knows it.
   */
  function connectFrame(opts) {
    const o = opts || {};
    const win = o.window || null;
    let parent = null;
    try { parent = win ? win.parent : null; } catch (e) { parent = null; }
    if (!parent || parent === win) return null;
    const origin = originOf(win, o.origin);
    let opened = typeof o.onOpen === 'function' ? o.onOpen : null;
    function send(event, data) {
      try { parent.postMessage(Object.assign({ kind: KIND, event }, data || {}), origin); } catch (e) { /* the host went away */ }
    }

    // The asks waiting for the host's answer: ask number -> {event, done(answer), timer}.
    const asking = new Map();
    let lastAsk = 0;
    const later = typeof o.setTimeout === 'function' ? o.setTimeout
      : (win && typeof win.setTimeout === 'function' ? (fn, ms) => win.setTimeout(fn, ms) : null);
    const unlater = typeof o.clearTimeout === 'function' ? o.clearTimeout
      : (win && typeof win.clearTimeout === 'function' ? (t) => win.clearTimeout(t) : () => {});
    /** Ask the host (`event`: 'identity', 'sign' or 'token'); the answer, or null when none came within `waitMs`. */
    function askHost(event, data, waitMs) {
      return new Promise((resolve) => {
        const ask = ++lastAsk;
        const entry = { event, done: resolve, timer: null };
        asking.set(ask, entry);
        if (later) entry.timer = later(() => answered(ask, null), waitMs);
        send(event, Object.assign({ ask }, data || {}));
      });
    }
    function answered(ask, m) {
      const entry = asking.get(ask);
      if (!entry) return;
      asking.delete(ask);
      if (entry.timer != null) unlater(entry.timer);
      entry.done(m);
    }
    async function identity() {
      const m = await askHost('identity', null, IDENTITY_WAIT_MS);
      return m && !m.refused && typeof m.key === 'string' && /^[0-9a-f]+$/i.test(m.key) ? m.key : '';
    }
    async function sign(purpose) {
      if (!signable(purpose)) return null;
      const m = await askHost('sign', { purpose }, ASK_WAIT_MS);
      if (!m || m.refused || typeof m.key !== 'string' || !m.key || typeof m.sig !== 'string' || !m.sig || !Number.isFinite(m.timestamp)) return null;
      return { key: m.key, timestamp: m.timestamp, sig: m.sig };
    }
    async function token() {
      const m = await askHost('token', null, TOKEN_WAIT_MS);
      return m && !m.refused && tokenShaped(m.token) ? m.token : '';
    }

    if (typeof win.addEventListener === 'function') {
      win.addEventListener('message', (e) => {
        if (!e || e.source !== parent || (origin && e.origin !== origin)) return;
        const m = e.data;
        if (!m || typeof m !== 'object' || m.kind !== KIND) return;
        if (m.event === 'open') { if (opened) opened(); return; }
        // The host's answer to one of this page's asks, to the ask it names and of its kind.
        if (m.event === 'identity' || m.event === 'signed' || m.event === 'token') {
          const entry = asking.get(m.ask);
          if (entry && entry.event === (m.event === 'signed' ? 'sign' : m.event)) answered(m.ask, m);
          return;
        }
        if (m.event !== 'files') return;
        const files = Array.prototype.slice.call(m.files || []).filter(Boolean);
        if (files.length && typeof o.onFiles === 'function') o.onFiles(files, typeof m.postTo === 'string' ? m.postTo : '');
      });
    }
    return { send, ready: () => send('ready'), onOpen: (fn) => { opened = typeof fn === 'function' ? fn : null; }, identity, sign, token };
  }

  const api = { KIND, PAGE, WORDS, ASK_WAIT_MS, IDENTITY_WAIT_MS, TOKEN_WAIT_MS, isWindowAddress, isItemWindowAddress, itemPage, signable, tokenShaped, createHost, connectFrame };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else root.HosUploadWindow = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
