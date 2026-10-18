// ── shared-files-upload.js ────────────────────────────────────────────────
// Upload a file to Shared Files from the website (2026-10-17), and files
// dragged from the computer onto the page. The desktop app's Files page is the
// original (src/gui/pages/files.rs draw_shared_files_section, with
// files_upload_step.rs); this mirrors its button, its rules and its words:
//
// - "Upload a file" (several at once) is offered once this page is signed in to
//   the server with a role that may upload. Until then the page says why, in the
//   words the overlay of a refused drop uses too.
// - Sizes first, for all of them: those over the person's limit on this server
//   are named in one line ("Not uploaded, over the 6 MB limit for you on this
//   server: a.mp4, b.zip.") and the rest go, one at a time. The limit is the one
//   the server says in the person's own peer-list entry (relay-session.js
//   uploadLimitBytes), 6 MB until it has.
// - Each file is cleaned on this device first (/shared/upload-clean.js over
//   /shared/strip-metadata.js): where it was taken and which camera took it are
//   removed unless the person turns off "Remove location and camera details".
//   When there is something to say (hidden details found, or a file that could
//   not be checked) the step shows it, with the switch, Upload and Cancel; a
//   file with nothing hidden in it uploads at once. Shared Files is public, so a
//   file that could not be checked is never uploaded while details are to be
//   removed (the cleaning's own words say so).
// - The upload is POST /api/upload?token=<this session's token>&share=1 (and
//   keep_metadata=1 when the person kept the details by choice), the chat's
//   and the desktop's own route; the server's words when it refuses.
// - Files dropped on the page (/shared/drop-overlay.js): "Drop to upload to
//   Shared Files" while they are over it, and the drop takes exactly the
//   button's path (takeFiles). While the button is not offered, "Files cannot
//   be dropped here." and the reason it is not.
//
// Everything is built with createElement and textContent. A classic script in
// the browser (SharedFilesUpload on window; the page's own script calls boot),
// a CommonJS module under Node, where scripts/tests/drop-files-web.test.js
// drives it against stand-ins.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  const NODE = typeof module === 'object' && module && module.exports;
  const DROP = NODE ? require('../shared/drop-overlay.js') : root.HosDropOverlay;

  const MIB = 1024 * 1024;
  // The largest file until the server has said the person's own limit (the chat's and the
  // desktop's fallback, ATTACH_MAX_BYTES).
  const FALLBACK_LIMIT = 6 * MIB;
  // What the picker offers: the chat's attach button's kinds (the server refuses any other).
  const ACCEPT = '.png,.jpg,.jpeg,.gif,.webp,.heic,.heif,.avif,.pdf,.txt,.md,.json,.zip,.tar.gz,.mp3,.mp4,.webm,.ogg,.wav,.blend,.stl,.obj,.gltf,.glb';

  const WORDS = Object.freeze({
    upload: 'Upload a file',
    uploadTip: 'Upload files from this device to Shared Files, for anyone to download. You can also drop files on this page.',
    // The desktop's step (files_upload_step.rs).
    checking: 'Checking the file for hidden details...',
    go: 'Upload',
    cancel: 'Cancel',
    // Why the button is not offered (and why a drop does nothing).
    noIdentity: 'Not signed in on this page. Open Chat in this browser first (it keeps your identity here), then load this page again. A passphrase-protected identity cannot be read by this page yet.',
    signingIn: 'Signing in to this server...',
    cannotUpload: 'Your role on this server cannot upload files, so Upload a file is not offered. An admin can change that in Server Settings > Roles.',
    // The chat's and the desktop's own (attach_send.rs NOT_SIGNED_IN_YET).
    notSignedInYet: 'The file was not sent: this server has not finished signing you in yet. Try again in a moment.',
    checkerMissing: 'This page could not load its file checker, so nothing can be uploaded. Reload the page and try again.',
    unreachable: 'the server could not be reached.',
  });

  function mbOf(bytes) {
    return Math.round((Number(bytes) || 0) / MIB);
  }
  /** Several over the limit, named in one line (files.rs, a picked set). */
  function overLimitWords(limit, names) {
    return 'Not uploaded, over the ' + mbOf(limit) + ' MB limit for you on this server: ' + names.join(', ') + '.';
  }
  /** One file over the limit (files.rs start_shared_upload). */
  function tooLargeWords(name, limit) {
    return name + ' is too large (' + mbOf(limit) + ' MB max for you on this server).';
  }
  function uploadingWords(name) {
    return 'Uploading ' + name + '...';
  }
  function uploadedWords(name) {
    return 'Uploaded ' + name + '. It is in the list below.';
  }
  function failedWords(why) {
    return 'Not uploaded: ' + why;
  }
  /** The server's refusal in its own words, or plain ones for an empty body or a proxy's HTML page. */
  function refusalWords(status, body) {
    const text = String(body == null ? '' : body).trim();
    return text && !text.startsWith('<') ? text : 'the server refused it (' + status + ').';
  }

  /**
   * The upload, built in `env.root`. `env`:
   *   document, root, fetch, FormData,
   *   session (a /shared/relay-session.js session, or null without an identity here),
   *   clean (/shared/upload-clean.js, or false when the page could not load it),
   *   storage (localStorage, for the switch's last choice and the protected setup),
   *   window (where files dragged from the computer are heard; optional),
   *   onUploaded() (the list is loaded again; optional).
   * Returns {start, takeFiles, dropDecision, state, dispose}.
   */
  function createSharedFilesUpload(env) {
    const doc = env.document;
    const session = env.session || null;
    const cleaner = env.clean || null;
    const storage = env.storage || null;
    const FD = env.FormData || (typeof FormData !== 'undefined' ? FormData : null);
    const st = { status: '', notice: '', queue: [], running: null, step: null };
    const els = {};
    let drops = null;

    function mk(tag, props, kids) {
      const el = doc.createElement(tag);
      for (const k of Object.keys(props || {})) {
        const v = props[k];
        if (v === undefined || v === null) continue;
        if (k === 'text') el.textContent = String(v);
        else if (k === 'attrs') { for (const a of Object.keys(v)) el.setAttribute(a, String(v[a])); }
        else el[k] = v;
      }
      for (const c of kids || []) if (c) el.appendChild(c);
      return el;
    }

    // ── Who the person is here ──
    function state() {
      return session ? session.state : null;
    }
    function token() {
      const s = state();
      return s && s.status === 'bound' ? s.uploadToken || '' : '';
    }
    function roleDef() {
      if (session && typeof session.roleDef === 'function') return session.roleDef();
      const s = state();
      return s && Array.isArray(s.roles) ? s.roles.find((r) => r && r.id === s.role) || null : null;
    }
    /** Signed in with a role that may upload (the relay's upload_role_check): unknown counts as no. */
    function canUpload() {
      const s = state();
      if (!s || s.status !== 'bound' || !s.role) return false;
      const def = roleDef();
      return !!(def && def.can_upload === true);
    }
    /** Why Upload a file is not offered now. */
    function whyNot() {
      const s = state();
      if (!s || s.status === 'no_identity') return WORDS.noIdentity;
      if (s.status === 'connecting' || s.status === 'idle') return WORDS.signingIn;
      if (s.status === 'refused') return 'The server did not accept this page\'s sign-in: ' + s.refusal;
      if (s.status === 'closed') return 'Lost the connection to the server. Trying again in ' + (s.retryInSeconds || 5) + ' seconds.';
      if (!s.role) return WORDS.cannotUpload;
      const def = roleDef();
      if (def && def.can_upload !== true) return WORDS.cannotUpload;
      return WORDS.signingIn; // signed in, the role list not here yet
    }
    function limitBytes() {
      const s = state();
      const n = s ? Number(s.uploadLimitBytes) : 0;
      return Number.isFinite(n) && n > 0 ? n : FALLBACK_LIMIT;
    }

    // ── Drawing ──
    function build() {
      els.input = mk('input', { type: 'file', id: 'sf-upload-input', hidden: true, multiple: true, accept: ACCEPT });
      els.input.addEventListener('change', () => {
        const files = Array.from(els.input.files || []);
        try { els.input.value = ''; } catch (e) { /* some browsers refuse; harmless */ }
        takeFiles(files);
      });
      els.button = mk('button', { type: 'button', id: 'sf-upload-btn', className: 'sf-upload-btn', text: WORDS.upload, title: WORDS.uploadTip });
      els.button.addEventListener('click', () => els.input.click());
      els.status = mk('span', { id: 'sf-upload-status', className: 'sf-upload-status' });
      els.why = mk('p', { id: 'sf-upload-why', className: 'sf-upload-why' });
      els.notice = mk('p', { id: 'sf-upload-notice', className: 'sf-upload-notice' });
      els.step = mk('div', { id: 'sf-upload-step' });
      els.row = mk('div', { className: 'sf-upload-row' }, [els.button, els.status, els.input]);
      env.root.replaceChildren(els.row, els.why, els.notice, els.step);
    }
    function render() {
      if (!els.row) return;
      const can = canUpload();
      els.button.hidden = !can;
      els.why.textContent = can ? '' : whyNot();
      els.why.hidden = can;
      els.status.textContent = st.status;
      els.notice.textContent = st.notice;
      els.notice.hidden = !st.notice;
    }
    function say(words) {
      st.status = words || '';
      render();
    }

    // ── Taking files: the button's and a drop's one path ──
    /** Files chosen with Upload a file or dropped on the page. Resolves once they are all done. */
    function takeFiles(files) {
      const list = Array.from(files || []).filter(Boolean);
      if (!list.length) return Promise.resolve();
      st.notice = '';
      if (!canUpload()) { say(whyNot()); return Promise.resolve(); }
      // Sizes first, for all of them: those too large are said now, by name, and the rest go.
      const limit = limitBytes();
      const fits = list.filter((f) => Number(f.size) <= limit);
      const big = list.filter((f) => !(Number(f.size) <= limit));
      if (big.length) st.notice = overLimitWords(limit, big.map((f) => f.name));
      render();
      st.queue.push(...fits);
      return pump();
    }
    /** One file after another, the next once the one before is up, refused or cancelled. */
    function pump() {
      if (!st.running) {
        st.running = (async () => {
          try {
            while (st.queue.length) await one(st.queue.shift());
          } finally {
            st.running = null;
          }
        })();
      }
      return st.running;
    }
    async function one(file) {
      if (!token()) { st.queue.length = 0; say(WORDS.notSignedInYet); return; }
      const limit = limitBytes();
      if (Number(file.size) > limit) { say(tooLargeWords(file.name, limit)); return; }
      if (!cleaner) { st.queue.length = 0; say(WORDS.checkerMissing); return; }
      say(WORDS.checking);
      const protectedOn = cleaner.protectedSetupOn(storage);
      const ctx = { where: 'public', remove: cleaner.removeDetailsChosen(storage), protectedOn };
      let clean;
      try {
        clean = await cleaner.cleanFile(file);
      } catch (e) {
        say(failedWords(file.name + ' could not be read.'));
        return;
      }
      let decided;
      if (cleaner.stepNeeded(clean, ctx)) {
        say('');
        const remove = await askStep(file, clean, ctx);
        if (remove === null) return; // Cancel
        if (!protectedOn) cleaner.rememberRemoveDetails(storage, remove);
        decided = cleaner.decide(clean, Object.assign({}, ctx, { remove }));
      } else {
        decided = cleaner.decide(clean, ctx);
      }
      if (decided.action !== 'send') { say(decided.words || ''); return; }
      await send(file.name, decided.file, decided.keepMetadata);
    }

    /**
     * The step, when the file has something to say: what is hidden in it, the switch (a line under
     * the protected setup), why it cannot go as it is, Upload and Cancel. Resolves with the switch's
     * last state on Upload, null on Cancel.
     */
    function askStep(file, clean, ctx) {
      return new Promise((resolve) => {
        const W = cleaner.WORDS || {};
        let remove = ctx.protectedOn || ctx.remove !== false;
        const something = !!(clean.summary || clean.unchecked);
        const kids = [mk('p', { className: 'sf-step-name', text: file.name })];
        if (clean.summary) {
          kids.push(mk('p', { className: 'sf-step-summary', text: clean.summary }));
          kids.push(mk('ul', { className: 'sf-step-details' }, (clean.details || []).map((d) => mk('li', { text: d.words }))));
        }
        let box = null;
        if (ctx.protectedOn) {
          if (something) kids.push(mk('p', { className: 'sf-step-protected', text: W.protectedLine }));
        } else if (something) {
          box = mk('input', { type: 'checkbox', id: 'sf-upload-remove', checked: remove });
          kids.push(mk('label', { className: 'sf-step-switch' }, [box, doc.createTextNode(' ' + W.switchLabel)]));
        }
        const off = mk('p', { id: 'sf-upload-off', className: 'sf-step-off' });
        const refused = mk('p', { id: 'sf-upload-refused', className: 'sf-step-refused' });
        const go = mk('button', { type: 'button', id: 'sf-upload-go', className: 'sf-upload-btn', text: WORDS.go });
        const cancel = mk('button', { type: 'button', id: 'sf-upload-cancel', className: 'sf-step-cancel', text: WORDS.cancel });
        kids.push(off, refused, mk('div', { className: 'sf-step-buttons' }, [go, cancel]));
        const draw = () => {
          const d = cleaner.decide(clean, Object.assign({}, ctx, { remove }));
          off.textContent = !ctx.protectedOn && !remove && clean.summary ? W.switchOffLine || '' : '';
          refused.textContent = d.action === 'refuse' ? d.words || '' : '';
          go.disabled = d.action === 'refuse';
        };
        const done = (answer) => {
          st.step = null;
          els.step.replaceChildren();
          resolve(answer);
        };
        if (box) box.addEventListener('change', () => { remove = !!box.checked; draw(); });
        go.addEventListener('click', () => { if (!go.disabled) done(remove); });
        cancel.addEventListener('click', () => { say(''); done(null); });
        st.step = { file: file.name };
        els.step.replaceChildren(mk('div', { id: 'sf-upload-step-card', className: 'sf-step' }, kids));
        draw();
      });
    }

    async function send(name, blob, keepMetadata) {
      const t = token();
      if (!t) { st.queue.length = 0; say(WORDS.notSignedInYet); return; }
      say(uploadingWords(name));
      const form = new FD();
      form.append('file', blob, name);
      const url = '/api/upload?token=' + encodeURIComponent(t) + '&share=1' + (keepMetadata ? '&keep_metadata=1' : '');
      let res = null;
      try {
        res = await env.fetch(url, { method: 'POST', body: form });
      } catch (e) {
        say(failedWords(WORDS.unreachable));
        return;
      }
      if (!res || !res.ok) {
        let body = '';
        try { body = res ? await res.text() : ''; } catch (e) { body = ''; }
        say(failedWords(refusalWords(res ? res.status : 0, body)));
        return;
      }
      say(uploadedWords(name));
      if (typeof env.onUploaded === 'function') env.onUploaded();
    }

    // ── Files dragged from the computer onto the page ──
    function dropDecision() {
      if (canUpload()) return { take: true, line: DROP.WORDS.sharedFiles };
      return { take: false, reason: whyNot() };
    }

    function start() {
      build();
      render();
      if (DROP && env.window) drops = DROP.attach({ window: env.window, document: doc, decide: dropDecision, onDrop: (files) => { takeFiles(files); } });
      if (session) {
        session.subscribe(() => render());
        session.start();
      }
    }
    function dispose() {
      if (drops) { drops.detach(); drops = null; }
    }

    return { start, takeFiles, dropDecision, state: st, dispose, render };
  }

  /**
   * In the browser: the upload in #sf-upload, signed in with the identity Chat keeps here (the
   * page's own script hands in the promise it already asked, so the key is derived once).
   */
  function boot(opts) {
    const o = opts || {};
    const doc = root.document;
    const el = doc && doc.getElementById('sf-upload');
    if (!el) return null;
    let storage = null;
    try { storage = root.localStorage; } catch (e) { storage = null; }
    const identity = o.identity || Promise.resolve(null);
    const session = typeof root.createRelaySession === 'function'
      ? root.createRelaySession({
        getIdentity: () => identity,
        signBytes: (secret, bytes) => root.pqSignMessage(secret, bytes),
        WebSocket: root.WebSocket,
        location: root.location,
        storage,
      })
      : null;
    const page = createSharedFilesUpload({
      document: doc, root: el, window: root, session, storage,
      fetch: (u, i) => root.fetch(u, i),
      FormData: root.FormData,
      clean: root.HosUploadClean || false,
      onUploaded: o.onUploaded,
    });
    page.start();
    return page;
  }

  const api = {
    WORDS, ACCEPT, FALLBACK_LIMIT, overLimitWords, tooLargeWords, uploadingWords, uploadedWords, failedWords, refusalWords,
    createSharedFilesUpload, boot,
  };
  if (NODE) module.exports = api;
  else root.SharedFilesUpload = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
