// ── media-upload.js ───────────────────────────────────────────────────────
// Add to Media's resumable upload on the website (2026-10-11,
// docs/design/media-showcase.md "Resumable uploads, exact" and its As built
// note; the relay half is src/relay/resumable.rs). A Media file goes up in
// pieces: start (POST /api/upload/resumable, the answer says the largest
// piece, `chunk_max`), one piece after another (PATCH ...?offset=), each
// written and counted by the server before the next, so after a dropped
// connection the page asks how far it got (GET) and carries on from there;
// Cancel deletes it (DELETE). No `blake3` is sent: the web cannot hash a
// gigabyte cheaply, and the relay takes the upload without one.
//
// What it handles, in the relay's own words each time: Pause and Resume; a
// dropped connection (waits 2 s, doubling to 30 s, then asks where it got);
// the page signing in again (the token changes; it waits for the new one); a
// piece at the wrong place (409 with the server's offset); a piece while
// another is written (409 busy: it asks where it got); the role's allowance
// for the day or the hour (429, with when it resets: Resume tries again); no
// room left by the finish (507: the server keeps the file for a day, and
// Retry sends the empty piece that tries the finish again, so nothing is sent
// twice); and an answer lost on the way (the server's `done` on GET).
//
// An upload in progress is remembered in this browser's localStorage under
// its session id with the file's name, size and last change (every read and
// write wrapped, so a browser that refuses storage only loses the resume), so
// picking the same file again after a reload carries on instead of starting
// over. Used by /pages/media-app.js. A classic script in the browser (loaded
// before media-app.js), a CommonJS module under Node, where
// scripts/tests/media-web-round2.test.js drives it against a stand-in server.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  const MIB = 1024 * 1024;
  // Where an unfinished upload is remembered in this browser (with its session id), so
  // picking the same file again after a reload carries on instead of starting over.
  const RESUME_KEY = 'humanity_media_uploads';
  const RESUME_KEEP = 10;
  // The relay removes an upload nobody has touched for a day (its expiry pass).
  const RESUME_MAX_AGE_MS = 24 * 3600 * 1000;
  // A piece's size until the server has said its own (`chunk_max`, which it always does).
  const DEFAULT_CHUNK = 8 * MIB;
  // After a dropped connection: the first wait, doubled each time up to the last.
  const RETRY_FIRST_MS = 2000;
  const RETRY_MAX_MS = 30000;
  // A token the server no longer knows (the page signed in again meanwhile): tries before giving up.
  const STALE_TOKEN_TRIES = 10;

  function jsonPost(body) {
    return { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) };
  }

  // ── Remembering an unfinished upload (localStorage; any failure there changes nothing else) ──

  function readRemembered(storage) {
    try {
      const list = JSON.parse((storage && storage.getItem(RESUME_KEY)) || '[]');
      return Array.isArray(list) ? list.filter((r) => r && typeof r.id === 'string') : [];
    } catch (e) {
      return [];
    }
  }

  function writeRemembered(storage, list) {
    try { if (storage) storage.setItem(RESUME_KEY, JSON.stringify(list.slice(-RESUME_KEEP))); } catch (e) { /* storage refused: nothing else changes */ }
  }

  /** Remember an upload in progress: whose, its session id, and the file by name, size and last change. */
  function rememberUpload(storage, rec) {
    const list = readRemembered(storage).filter((r) => r.id !== rec.id);
    list.push(rec);
    writeRemembered(storage, list);
  }

  /** The remembered upload of this same file by this person, if any and not too old. */
  function findUpload(storage, owner, file, nowMs) {
    if (!file) return null;
    return readRemembered(storage).find((r) => r.owner === owner && r.name === file.name && r.size === file.size &&
      r.lastModified === file.lastModified && !(nowMs - (Number(r.at) || 0) > RESUME_MAX_AGE_MS)) || null;
  }

  function forgetUpload(storage, id) {
    const list = readRemembered(storage);
    const kept = list.filter((r) => r.id !== id);
    if (kept.length !== list.length) writeRemembered(storage, kept);
  }

  // ── The resumable upload (src/relay/resumable.rs) ──

  /**
   * One file sent in pieces. `o`: fetch, token() (the session's current upload
   * token, '' while it is not signed in), file (a File: name, size,
   * lastModified, type, slice), owner (the person's key), storage
   * (localStorage), sleep(ms), now(), onChange(state), onDone(answer).
   * Its `state`: phase ('idle', 'starting', 'sending', 'pausing', 'paused',
   * 'reconnecting', 'waiting_connection', 'allowance', 'no_room', 'failed',
   * 'cancelled', 'done'), id, offset, size, error, resetsAt, retryIn, resumed.
   * The token goes in the address (`?token=`), the way POST /api/upload takes
   * it: these are requests the page makes, never an address anyone sees.
   */
  function createUploader(o) {
    const s = { phase: 'idle', id: '', offset: 0, size: o.file.size, chunkMax: 0, error: '', resetsAt: 0, retryIn: 0, resumed: false, answer: null };
    let running = false;
    let pauseWanted = false;
    let cancelled = false;
    let wake = null;
    const now = () => (typeof o.now === 'function' ? o.now() : Date.now());
    const changed = () => { try { if (o.onChange) o.onChange(s); } catch (e) { /* a drawing failure never stops the upload */ } };
    const set = (phase, extra) => { s.phase = phase; Object.assign(s, extra || {}); changed(); };

    /** A wait that Pause and Cancel cut short. */
    function nap(ms) {
      return new Promise((resolve) => {
        let done = false;
        const fin = () => { if (!done) { done = true; wake = null; resolve(); } };
        wake = fin;
        Promise.resolve(o.sleep ? o.sleep(ms) : new Promise((r) => setTimeout(r, ms))).then(fin, fin);
      });
    }

    function at(id, extra) {
      return '/api/upload/resumable' + (id ? '/' + encodeURIComponent(id) : '') + '?token=' + encodeURIComponent(o.token()) + (extra || '');
    }

    async function read(res) {
      let text = '';
      try { text = String(await res.text()); } catch (e) { text = ''; }
      let json = null;
      try { json = text ? JSON.parse(text) : null; } catch (e) { json = null; }
      const words = json && typeof json.error === 'string' && json.error ? json.error
        : (!text.trim() || text.trim().startsWith('<') || json ? 'The server answered ' + res.status + '.' : text.trim());
      return { json: json && typeof json === 'object' ? json : null, words };
    }

    function fail(words, forget) {
      if (forget && s.id) forgetUpload(o.storage, s.id);
      set('failed', { error: words });
    }

    function finish(answer) {
      if (s.id) forgetUpload(o.storage, s.id);
      s.offset = s.size;
      set('done', { answer });
      if (o.onDone) o.onDone(answer);
    }

    /** Ask the server how far it got: {ok}, {done: answer}, {gone, words} or {retry}. */
    async function status() {
      let res;
      try { res = await o.fetch(at(s.id), { method: 'GET' }); } catch (e) { return { retry: true }; }
      const b = await read(res);
      if (res.ok && b.json) {
        if (Number.isFinite(b.json.offset)) s.offset = b.json.offset;
        if (Number.isFinite(b.json.chunk_max) && b.json.chunk_max > 0) s.chunkMax = b.json.chunk_max;
        if (b.json.done && typeof b.json.done === 'object') return { done: b.json.done };
        changed();
        return { ok: true };
      }
      if (res.status === 404 || res.status === 410) return { gone: true, words: b.words };
      return { retry: true, words: b.words };
    }

    /** Wait until the page is signed in again (the token changes on every sign-in). */
    async function waitForToken() {
      while (!o.token() && !cancelled && !pauseWanted) {
        set('waiting_connection');
        await nap(1000);
      }
    }

    async function begin() {
      if (running) return;
      running = true;
      try {
        set('starting');
        await waitForToken();
        if (cancelled || pauseWanted) return stopped();
        const known = s.id ? null : findUpload(o.storage, o.owner, o.file, now());
        if (known) {
          s.id = known.id;
          const where = await status();
          if (where.done) return finish(where.done);
          if (where.ok) { s.resumed = true; changed(); return await loop(); }
          if (where.retry) return await loop();
          forgetUpload(o.storage, known.id);
          s.id = '';
          s.offset = 0;
        }
        if (s.id) return await loop();
        let backoff = RETRY_FIRST_MS;
        for (;;) {
          if (cancelled || pauseWanted) return stopped();
          await waitForToken();
          let res;
          try {
            res = await o.fetch(at(''), jsonPost({ filename: o.file.name, size: o.file.size, content_type: o.file.type || 'application/octet-stream', media: 1 }));
          } catch (e) {
            const wait = backoff;
            backoff = Math.min(backoff * 2, RETRY_MAX_MS);
            set('reconnecting', { retryIn: Math.round(wait / 1000) });
            await nap(wait);
            continue;
          }
          const b = await read(res);
          if (res.ok && b.json && typeof b.json.id === 'string') {
            s.id = b.json.id;
            s.offset = Number(b.json.offset) || 0;
            if (Number.isFinite(b.json.chunk_max) && b.json.chunk_max > 0) s.chunkMax = b.json.chunk_max;
            rememberUpload(o.storage, { owner: o.owner, id: s.id, name: o.file.name, size: o.file.size, lastModified: o.file.lastModified, at: now() });
            return await loop();
          }
          if (res.status === 429 && b.json) return set('allowance', { error: b.words, resetsAt: Number(b.json.resets_at) || 0 });
          return fail(b.words, false);
        }
      } finally {
        running = false;
        afterRun();
      }
    }

    function stopped() {
      if (cancelled) return undefined;
      if (pauseWanted) set('paused');
      return undefined;
    }

    async function loop() {
      let backoff = RETRY_FIRST_MS;
      let staleTries = 0;
      for (;;) {
        if (cancelled || pauseWanted) return stopped();
        await waitForToken();
        if (cancelled || pauseWanted) return stopped();
        const end = Math.min(s.size, s.offset + (s.chunkMax || DEFAULT_CHUNK));
        // At the end (after a refused finish, or a resume that found every byte there) this is
        // empty: an empty piece at offset = size tries the finish again.
        const piece = o.file.slice(s.offset, end);
        set(s.offset >= s.size ? 'finishing' : 'sending');
        let res;
        try {
          res = await o.fetch(at(s.id, '&offset=' + s.offset), { method: 'PATCH', headers: { 'Content-Type': 'application/octet-stream' }, body: piece });
        } catch (e) {
          // The connection dropped: wait, then ask the server how far it got.
          const wait = backoff;
          backoff = Math.min(backoff * 2, RETRY_MAX_MS);
          set('reconnecting', { retryIn: Math.round(wait / 1000) });
          await nap(wait);
          if (cancelled || pauseWanted) return stopped();
          await waitForToken();
          const where = await status();
          if (where.done) return finish(where.done);
          if (where.gone) return fail(where.words, true);
          continue;
        }
        backoff = RETRY_FIRST_MS;
        const b = await read(res);
        const j = b.json || {};
        if (res.ok) {
          if (typeof j.url === 'string' || typeof j.filename === 'string') return finish(j);
          if (Number.isFinite(j.offset)) s.offset = j.offset;
          staleTries = 0;
          changed();
          continue;
        }
        if (res.status === 409 && j.busy) {
          // The server is still writing an earlier piece (an answer that was lost): ask where it got.
          await nap(1000);
          const where = await status();
          if (where.done) return finish(where.done);
          if (where.gone) return fail(where.words, true);
          continue;
        }
        if (res.status === 409 && Number.isFinite(j.offset)) { s.offset = j.offset; continue; }
        if (res.status === 429) {
          if (Number.isFinite(j.offset)) s.offset = j.offset;
          return set('allowance', { error: b.words, resetsAt: Number(j.resets_at) || 0 });
        }
        if (res.status === 507) {
          // Every byte arrived; the room had gone by the finish. The server keeps the file for a day.
          s.offset = s.size;
          return set('no_room', { error: b.words });
        }
        if (res.status === 403 && /token/i.test(b.words) && staleTries < STALE_TOKEN_TRIES) {
          // A token from before the page signed in again: wait for the new one.
          staleTries++;
          await nap(1000);
          continue;
        }
        return fail(b.words, res.status === 404 || res.status === 410 || res.status === 422);
      }
    }

    function afterRun() {
      if (cancelled && s.id) {
        const id = s.id;
        s.id = '';
        forgetUpload(o.storage, id);
        if (o.token()) {
          Promise.resolve().then(() => o.fetch(at(id), { method: 'DELETE' })).catch(() => { /* gone with the day's expiry anyway */ });
        }
      }
    }

    return {
      state: s,
      start() { if (!running) { pauseWanted = false; begin(); } },
      pause() {
        if (running && !cancelled) {
          pauseWanted = true;
          set('pausing');
          if (wake) wake();
        }
      },
      /** Resume after Pause, try again after an allowance or a refused finish. */
      resume() {
        if (running) { pauseWanted = false; return; }
        pauseWanted = false;
        s.error = '';
        begin();
      },
      cancel() {
        cancelled = true;
        set('cancelled');
        if (wake) wake();
        if (!running) afterRun();
      },
      get running() { return running; },
    };
  }

  const api = { RESUME_KEY, RESUME_MAX_AGE_MS, DEFAULT_CHUNK, RETRY_FIRST_MS, RETRY_MAX_MS, rememberUpload, findUpload, forgetUpload, createUploader };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else Object.assign(root, { MediaUpload: api });
})(typeof globalThis !== 'undefined' ? globalThis : this);
