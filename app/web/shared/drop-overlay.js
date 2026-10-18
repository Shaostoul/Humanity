// ── drop-overlay.js ───────────────────────────────────────────────────────
// Dragging files from the computer's file manager onto a page (2026-10-17): the
// website's half of the drag and drop the desktop app has, in the desktop's
// words exactly.
//
// While files are dragged over a page that takes them, the whole page is dimmed
// (the dialogs' dimming, var(--bg-modal); /shared/drop-overlay.css) and one large
// line says what a drop will do. A drop opens the upload window, Add to Media with
// all its controls, and over a chat channel it posts there too (the operator,
// 2026-10-17: "First window popups with all the controls of what to do with the
// file ... If we're also in a chat channel, we could upload the file to the active
// chat channel"):
//   - the chat, one of this server's channels open:
//         "Drop to upload to Media and post in #general": the upload window (the
//         Media page's own Add to Media in a window over the chat,
//         /shared/upload-window.js) with "Also post in chat" ticked for it
//   - the chat, a private conversation open (a direct message, a group, the
//     scratch pad): "Drop to send in Ben" (the group's name in a group): the
//         private, encrypted send its Attach uses, never a public upload
//   - the Media page:                     "Drop to upload to Media"
//   - the Media item editor open:         "Drop to add to <the item's title>"
//   - the Shared Files page:              "Drop to upload to Shared Files"
// Nothing else is shown: a browser does not tell a page the dragged files'
// names until they are dropped. Where a drop would do nothing (signed out,
// sending files turned off), the line is "Files cannot be dropped here." with,
// under it, the reason the page already gives for its button being
// unavailable, and the drop does nothing.
//
// Only files: a dragged link or piece of text (no "Files" in
// dataTransfer.types) is left entirely to the browser, so dropping text into a
// box still works. For a file drag, dragover and drop are always cancelled on a
// page that uses this, so the browser never opens the dropped file in the tab
// (which would leave the page); where the drop is refused, dropEffect is
// "none", so the pointer says so and nothing happens.
//
// The overlay goes when the drag leaves the window or the drop happens. Each
// element the drag crosses sends dragenter and dragleave, so they are counted
// (the overlay stays while the count is above nought, and never flickers over a
// child element); and as a browser sends dragover every few tens of
// milliseconds while a drag is over the page, the overlay also goes when none
// has come for STALE_MS, for a drag that left without a last dragleave (the
// element under it redrawn meanwhile).
//
// A classic script in the browser (HosDropOverlay on window), a CommonJS module
// under Node, where scripts/tests/drop-files-web.test.js drives it with fake
// events.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  // The desktop app's words, exactly.
  const WORDS = Object.freeze({
    cannot: 'Files cannot be dropped here.',
    uploadToMedia: 'Drop to upload to Media',
    sharedFiles: 'Drop to upload to Shared Files',
  });
  /** "Drop to send in Ben": `name` is how the chat names the private conversation. */
  function sendIn(name) {
    return 'Drop to send in ' + String(name == null ? '' : name);
  }
  /**
   * "Drop to upload to Media and post in #general": a channel of the chat open (the desktop's
   * media/add.rs drop_and_post_words). `name` is how the chat names the channel.
   */
  function uploadAndPostIn(name) {
    return 'Drop to upload to Media and post in ' + String(name == null ? '' : name);
  }
  /** "Drop to add to Dusk on the beach": `title` is the item's title. */
  function addTo(title) {
    return 'Drop to add to ' + String(title == null ? '' : title);
  }

  // No dragover for this long: the drag has gone (the last dragleave never came).
  const STALE_MS = 400;

  /** Does this drag carry files (and not only a link or some text)? */
  function carriesFiles(dt) {
    if (!dt || !dt.types) return false;
    const t = dt.types;
    if (typeof t.includes === 'function') return t.includes('Files');
    if (typeof t.contains === 'function') return t.contains('Files'); // an older browser's DOMStringList
    return Array.prototype.indexOf.call(t, 'Files') >= 0;
  }

  /** The dropped files, as an array (empty when the drop holds none). */
  function filesOf(dt) {
    const list = dt && dt.files;
    if (!list) return [];
    return Array.prototype.slice.call(list).filter(Boolean);
  }

  /**
   * Put the overlay on a page. `opts`:
   *   window     where the drag events are heard (the page's window)
   *   document   where the overlay is drawn (its body); window.document when not given
   *   decide()   what a drop would do now: {take: true, line} or {take: false, reason}
   *              (asked when the drag comes in, on each dragover, and again at the drop)
   *   onDrop(files)  called with the dropped files when the drop is taken
   *   setTimeout, clearTimeout   (tests); the window's own when not given
   * Returns {detach(), shown(), line(), reason(), element()}.
   */
  function attach(opts) {
    const o = opts || {};
    const win = o.window || null;
    const doc = o.document || (win && win.document) || null;
    const later = o.setTimeout || (win && typeof win.setTimeout === 'function' ? (fn, ms) => win.setTimeout(fn, ms) : (fn, ms) => setTimeout(fn, ms));
    const cancel = o.clearTimeout || (win && typeof win.clearTimeout === 'function' ? (t) => win.clearTimeout(t) : (t) => clearTimeout(t));
    let depth = 0;
    let el = null;
    let lineEl = null;
    let reasonEl = null;
    let shown = false;
    let stale = null;

    function decide() {
      let d = null;
      try { d = typeof o.decide === 'function' ? o.decide() : null; } catch (e) { d = null; }
      if (d && d.take === true) return { take: true, line: String(d.line || ''), reason: '' };
      return { take: false, line: WORDS.cannot, reason: String((d && d.reason) || '') };
    }

    function build() {
      if (el || !doc || typeof doc.createElement !== 'function') return el;
      el = doc.createElement('div');
      el.className = 'hos-drop-overlay';
      el.hidden = true;
      if (typeof el.setAttribute === 'function') {
        el.setAttribute('role', 'status');
        el.setAttribute('aria-live', 'polite');
        el.setAttribute('data-drop-overlay', '');
      }
      lineEl = doc.createElement('p');
      lineEl.className = 'hos-drop-line';
      reasonEl = doc.createElement('p');
      reasonEl.className = 'hos-drop-reason';
      el.appendChild(lineEl);
      el.appendChild(reasonEl);
      const body = doc.body || null;
      if (body && typeof body.appendChild === 'function') body.appendChild(el);
      return el;
    }

    function show(d) {
      if (!build()) return;
      // Only what changed is written, so a dragover every few milliseconds costs nothing.
      if (lineEl.textContent !== d.line) lineEl.textContent = d.line;
      if (reasonEl.textContent !== d.reason) reasonEl.textContent = d.reason;
      reasonEl.hidden = !d.reason;
      el.className = 'hos-drop-overlay' + (d.take ? '' : ' hos-drop-refused');
      el.hidden = false;
      shown = true;
    }

    function hide() {
      depth = 0;
      if (stale != null) { cancel(stale); stale = null; }
      if (!shown) return;
      shown = false;
      if (el) el.hidden = true;
    }

    function armStale() {
      if (stale != null) cancel(stale);
      stale = later(() => { stale = null; hide(); }, STALE_MS);
    }

    function say(e, d) {
      try { if (e.dataTransfer) e.dataTransfer.dropEffect = d.take ? 'copy' : 'none'; } catch (x) { /* read-only in some browsers */ }
    }

    function onEnter(e) {
      if (!e || !carriesFiles(e.dataTransfer)) return;
      e.preventDefault();
      depth += 1;
      const d = decide();
      say(e, d);
      show(d);
      armStale();
    }
    function onOver(e) {
      if (!e || !carriesFiles(e.dataTransfer)) return;
      e.preventDefault();
      const d = decide();
      say(e, d);
      show(d);
      armStale();
    }
    function onLeave(e) {
      if (!e || !carriesFiles(e.dataTransfer)) return;
      depth = Math.max(0, depth - 1);
      if (depth === 0) hide();
    }
    function onDrop(e) {
      if (!e || !carriesFiles(e.dataTransfer)) return;
      // Never the browser's own drop: it would open the file in this tab.
      e.preventDefault();
      hide();
      const d = decide();
      if (!d.take) return;
      const files = filesOf(e.dataTransfer);
      if (files.length && typeof o.onDrop === 'function') o.onDrop(files);
    }

    if (win && typeof win.addEventListener === 'function') {
      win.addEventListener('dragenter', onEnter);
      win.addEventListener('dragover', onOver);
      win.addEventListener('dragleave', onLeave);
      win.addEventListener('drop', onDrop);
    }

    return {
      detach() {
        hide();
        if (win && typeof win.removeEventListener === 'function') {
          win.removeEventListener('dragenter', onEnter);
          win.removeEventListener('dragover', onOver);
          win.removeEventListener('dragleave', onLeave);
          win.removeEventListener('drop', onDrop);
        }
        if (el && typeof el.remove === 'function') el.remove();
        el = null;
      },
      shown: () => shown,
      line: () => (shown && lineEl ? lineEl.textContent : ''),
      reason: () => (shown && reasonEl ? reasonEl.textContent : ''),
      element: () => el,
    };
  }

  const api = { WORDS, STALE_MS, sendIn, uploadAndPostIn, addTo, carriesFiles, filesOf, attach };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else root.HosDropOverlay = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
