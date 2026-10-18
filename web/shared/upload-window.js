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
// A classic script in the browser (HosUploadWindow on window), a CommonJS module
// under Node, where scripts/tests/post-in-chat-web.test.js drives both halves
// with stand-in windows.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  /** Every message between the two halves carries this. */
  const KIND = 'hos-upload-window';
  /** The Media page in its upload-window form. */
  const PAGE = '/media?window=upload';
  const WORDS = Object.freeze({
    title: 'Add to Media',
    close: 'Close',
    closeTip: 'Close this window. An upload under way carries on, and what happens to it is said in the chat.',
  });

  /** Is this the upload-window form of the Media page (its address's `window=upload`)? */
  function isWindowAddress(search) {
    try { return new URLSearchParams(String(search || '')).get('window') === 'upload'; } catch (e) { return false; }
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
   *   onStatus(text)     a line the page in the frame said while the window was closed
   * Returns {show(files, postTo), close(), isOpen(), isBusy(), element(), frame(), pending()}.
   */
  function createHost(opts) {
    const o = opts || {};
    const win = o.window || null;
    const doc = o.document || (win && win.document) || null;
    const origin = originOf(win, o.origin);
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
        dialog.setAttribute('aria-label', WORDS.title);
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
      if (typeof frame.setAttribute === 'function') frame.setAttribute('title', WORDS.title);
      frame.src = o.page || PAGE;
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
    }
    function onKey(e) {
      if (open && e && e.key === 'Escape') close();
    }

    if (win && typeof win.addEventListener === 'function') win.addEventListener('message', onMessage);
    if (doc && typeof doc.addEventListener === 'function') doc.addEventListener('keydown', onKey);

    return {
      show,
      close,
      isOpen: () => open,
      isBusy: () => busy,
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
   * Returns {send(event, data), ready()}, or null when the page is not in a frame.
   */
  function connectFrame(opts) {
    const o = opts || {};
    const win = o.window || null;
    let parent = null;
    try { parent = win ? win.parent : null; } catch (e) { parent = null; }
    if (!parent || parent === win) return null;
    const origin = originOf(win, o.origin);
    function send(event, data) {
      try { parent.postMessage(Object.assign({ kind: KIND, event }, data || {}), origin); } catch (e) { /* the host went away */ }
    }
    if (typeof win.addEventListener === 'function') {
      win.addEventListener('message', (e) => {
        if (!e || e.source !== parent || (origin && e.origin !== origin)) return;
        const m = e.data;
        if (!m || typeof m !== 'object' || m.kind !== KIND || m.event !== 'files') return;
        const files = Array.prototype.slice.call(m.files || []).filter(Boolean);
        if (files.length && typeof o.onFiles === 'function') o.onFiles(files, typeof m.postTo === 'string' ? m.postTo : '');
      });
    }
    return { send, ready: () => send('ready') };
  }

  const api = { KIND, PAGE, WORDS, isWindowAddress, createHost, connectFrame };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else root.HosUploadWindow = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
