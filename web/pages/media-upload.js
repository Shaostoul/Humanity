// ── media-upload.js ───────────────────────────────────────────────────────
// Add to Media's resumable upload on the website (2026-10-11,
// docs/design/media-showcase.md "Resumable uploads, exact" and its As built
// note, then "Round 3: fixes from the round 2 review"; the relay half is
// src/relay/resumable.rs). A Media file goes up in pieces: start (POST
// /api/upload/resumable, the answer says the largest piece, `chunk_max`), one
// piece after another (PATCH ...?offset=), each written and counted by the
// server before the next, so after a dropped connection the page asks how far
// it got (GET) and carries on from there; Cancel deletes it (DELETE). No
// `blake3` is sent: the web cannot hash a gigabyte cheaply, and the relay
// takes the upload without one. The session's upload token goes in the
// `X-Upload-Token` header on every one of these requests (the relay's R3-9),
// never in an address.
//
// What it handles, in the relay's own words each time:
// - Pause, in every phase: the request on its way is stopped (an
//   AbortController), no other is made, and Resume first asks the server
//   where it got, since a stopped piece may have arrived after all.
// - A dropped connection: waits 2 s, doubling to 30 s, then asks where it got.
// - A server error (any 5xx but 507) or a 429 without the allowance's JSON (a
//   proxy's rate limit): waits the same way and tries again; the same error
//   ten times running stops with the server's own words and Retry.
// - The server still writing an earlier piece (409 busy): asks again after
//   the same growing wait, never once a second.
// - The page signing in again (the token changes; it waits for the new one).
// - A piece at the wrong place (409 with the server's offset).
// - The role's allowance for the day or the hour (429 with `allowance`): when
//   `left_bytes` is smaller than the piece, the piece is cut to it and sent
//   at once; otherwise it waits the server's own `resets_in_s` (at least 5
//   seconds) and carries on by itself.
// - No room left by the finish (507: the server keeps the file for a day, and
//   Retry sends the empty piece that tries the finish again, so nothing is
//   sent twice), and an answer lost on the way (the server's `done` on GET).
// - A file that can no longer be read (moved, changed, its drive unplugged):
//   each piece is read before it is sent, so a read that fails ends the
//   upload with words saying so, instead of looking like a dropped connection
//   for ever.
// - Cancel at any point, the finish included: the session is deleted on the
//   server (`cancelSession`). Round 4's cancel answer: a cancel means nothing
//   stays, so when the finish had already stored the file the server deletes
//   that too (`removed_stored`) and nothing is removed by hand; a file already
//   published stays (`published`); while a piece or the finish is being
//   written the server says busy and is asked again until it is not.
// - Every wait (`nap`) returns at once when Pause or Cancel has been pressed,
//   even one that would begin after the press (round 4, W4-1: a Cancel during
//   the start, answered with the allowance, used to sleep out the server's
//   hour).
//
// An upload in progress is remembered in this browser's localStorage under
// its session id with the file's name, size, last change and a fingerprint
// (SHA-256 of its first and last megabyte), dated by its LAST progress, the
// way the relay expires an untouched session a day after its last piece
// (every read and write wrapped, so a browser that refuses storage only loses
// the resume). Picking the same file again after a reload carries on instead
// of starting over; the record is forgotten when the upload is done, fails or
// is cancelled. Used by /pages/media-app.js. A classic script in the browser
// (loaded before media-app.js), a CommonJS module under Node, where
// scripts/tests/media-web-round2.test.js and media-web-round3.test.js drive
// it against a stand-in server, and media-web-round4.test.js holds round 4.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  const MIB = 1024 * 1024;
  // Where an unfinished upload is remembered in this browser (with its session id), so
  // picking the same file again after a reload carries on instead of starting over.
  const RESUME_KEY = 'humanity_media_uploads';
  const RESUME_KEEP = 10;
  // The relay removes an upload nobody has touched for a day (its expiry pass), counted
  // from its last piece, which is what a record's `at` is.
  const RESUME_MAX_AGE_MS = 24 * 3600 * 1000;
  // A piece's size until the server has said its own (`chunk_max`, which it always does).
  const DEFAULT_CHUNK = 8 * MIB;
  // After a dropped connection or a server error: the first wait, doubled each time up to the last.
  const RETRY_FIRST_MS = 2000;
  const RETRY_MAX_MS = 30000;
  // The same server error this many times running stops the upload with its words and Retry.
  const SAME_ERROR_TRIES = 10;
  // The shortest wait for an allowance, whatever the server says is left of it.
  const ALLOWANCE_MIN_MS = 5000;
  // A token the server no longer knows (the page signed in again meanwhile): tries before giving up.
  const STALE_TOKEN_TRIES = 10;
  // Tries at telling the server about a cancel when it cannot be reached (a dropped connection, a
  // server error, no token yet). A server busy writing a piece or the finish is not counted: it is
  // asked again until it is not.
  const CANCEL_TRIES = 10;
  // How much of each end of a file its fingerprint covers.
  const FINGERPRINT_BYTES = MIB;

  function hex(buf) {
    return Array.from(new Uint8Array(buf), (b) => b.toString(16).padStart(2, '0')).join('');
  }

  /** Read bytes `a` to `b` of a File; throws when the file can no longer be read or is shorter. */
  async function readSlice(file, a, b) {
    const bytes = new Uint8Array(await file.slice(a, b).arrayBuffer());
    if (bytes.length !== b - a) throw new Error('short read');
    return bytes;
  }

  function defaultDigest(bytes) {
    const c = root.crypto || (typeof globalThis !== 'undefined' ? globalThis.crypto : null);
    return c.subtle.digest('SHA-256', bytes);
  }

  /**
   * The file's fingerprint: SHA-256 (WebCrypto) of its first and last megabyte, one after the
   * other, as hex. Name, size and date alone match another file with the same ones (a copy
   * edited in place, a camera's IMG_0001.MOV); its bytes do not. Throws when the file cannot be
   * read.
   */
  async function fingerprint(file, digest) {
    const size = Number(file.size) || 0;
    const head = await readSlice(file, 0, Math.min(size, FINGERPRINT_BYTES));
    const tail = await readSlice(file, Math.max(0, size - FINGERPRINT_BYTES), size);
    const both = new Uint8Array(head.length + tail.length);
    both.set(head, 0);
    both.set(tail, head.length);
    return hex(await (digest || defaultDigest)(both));
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

  /** Remember an upload in progress: whose, its session id, the file (name, size, last change, fingerprint) and when it last moved. */
  function rememberUpload(storage, rec) {
    const list = readRemembered(storage).filter((r) => r.id !== rec.id);
    list.push(rec);
    writeRemembered(storage, list);
  }

  /** Date a remembered upload by its latest progress (the relay counts its day from the last piece). */
  function touchUpload(storage, id, nowMs) {
    const list = readRemembered(storage);
    const r = list.find((x) => x.id === id);
    if (!r) return;
    r.at = nowMs;
    writeRemembered(storage, list);
  }

  /**
   * The remembered upload of this same file by this person, if any and not too old: the same
   * name, size and last change, and the same fingerprint (`fp`, from `fingerprint`).
   */
  function findUpload(storage, owner, file, fp, nowMs) {
    if (!file || typeof fp !== 'string' || !fp) return null;
    return readRemembered(storage).find((r) => r.owner === owner && r.name === file.name && r.size === file.size &&
      r.lastModified === file.lastModified && r.fp === fp && !(nowMs - (Number(r.at) || 0) > RESUME_MAX_AGE_MS)) || null;
  }

  function forgetUpload(storage, id) {
    const list = readRemembered(storage);
    const kept = list.filter((r) => r.id !== id);
    if (kept.length !== list.length) writeRemembered(storage, kept);
  }

  /** A 429 that is the role's allowance (the relay's JSON), not a proxy's rate limit. */
  function isAllowance(json) {
    return !!(json && (typeof json.allowance === 'string' || Number.isFinite(json.resets_in_s)));
  }

  /**
   * An answer read: {json, words}. The relay's `error` when it gave one; a proxy's HTML page, an
   * empty body or JSON without words is "The server answered <status>."; else its plain words.
   */
  async function readAnswer(res) {
    let text = '';
    try { text = String(await res.text()); } catch (e) { text = ''; }
    let json = null;
    try { json = text ? JSON.parse(text) : null; } catch (e) { json = null; }
    const words = json && typeof json.error === 'string' && json.error ? json.error
      : (!text.trim() || text.trim().startsWith('<') || json ? 'The server answered ' + res.status + '.' : text.trim());
    return { json: json && typeof json === 'object' ? json : null, words };
  }

  /**
   * Cancel a resumable upload on the server: DELETE /api/upload/resumable/<id>, the token in
   * X-Upload-Token. Round 4's cancel answer (a cancel means nothing stays): `{ok: true}`, with
   * `removed_stored: true` when the finish had stored the file already (the server deleted that
   * too, so nothing is removed by hand) or `published: true` when the file was already on the
   * Media page (the person's own choice, which a cancel leaves). While a piece or the finish is
   * being written the server answers 409 `{busy: true}`, and it is asked again after a growing
   * wait (2 s doubling to 30 s) for as long as it says so. A dropped connection, a server error,
   * no token yet, or a token from before the page signed in again is tried again, CANCEL_TRIES
   * times in all. `o`: fetch, token() and sleep(ms). Answers {answer, words}: answer is
   * 'removed_stored', 'published', 'deleted', 'gone' (the server has no such upload: it ended
   * already), 'unconfirmed' (the server could not be told; `words` are its last answer, if any)
   * or 'refused' (`words` are the server's).
   */
  async function cancelSession(o, id) {
    const sleep = (ms) => Promise.resolve(o.sleep ? o.sleep(ms) : new Promise((r) => setTimeout(r, ms)));
    const url = '/api/upload/resumable/' + encodeURIComponent(id);
    let wait = RETRY_FIRST_MS;
    const later = async () => {
      await sleep(wait);
      wait = Math.min(wait * 2, RETRY_MAX_MS);
    };
    let tries = 0;
    let words = '';
    for (;;) {
      const t = o.token();
      let res = null;
      if (t) {
        try { res = await o.fetch(url, { method: 'DELETE', headers: { 'X-Upload-Token': t } }); } catch (e) { res = null; }
      }
      if (res) {
        const b = await readAnswer(res);
        const j = b.json || {};
        if (res.ok) return { answer: j.removed_stored ? 'removed_stored' : j.published ? 'published' : 'deleted', words: '' };
        if (res.status === 404 || res.status === 410) return { answer: 'gone', words: '' };
        // Busy with a piece or the finish: asked again until it is not, never counted as a try.
        if (res.status === 409 && j.busy) { await later(); continue; }
        const again = res.status >= 500 || res.status === 429 || (res.status === 403 && /token/i.test(b.words));
        if (!again) return { answer: 'refused', words: b.words };
        words = b.words;
      }
      if (++tries >= CANCEL_TRIES) return { answer: 'unconfirmed', words };
      if (t) await later();
      else await sleep(1000); // the page signing in again: its new token comes in a moment
    }
  }

  // ── The resumable upload (src/relay/resumable.rs) ──

  /**
   * One file sent in pieces. `o`: fetch, token() (the session's current upload
   * token, '' while it is not signed in), file (a File: name, size,
   * lastModified, type, slice), owner (the person's key), storage
   * (localStorage), sleep(ms), now(), fp (the file's fingerprint when the page
   * has worked it out already), digest(bytes) (SHA-256, WebCrypto's by
   * default), AbortController (the browser's by default), onChange(state),
   * onDone(answer), keepMetadata (the person chose to keep the file's hidden
   * details: the start says keep_metadata: 1).
   * Its `state`: phase ('idle', 'starting', 'sending', 'finishing',
   * 'pausing', 'paused', 'reconnecting', 'server_wait', 'waiting_connection',
   * 'allowance', 'no_room', 'server_error', 'failed', 'cancelling',
   * 'cancelled', 'done'), id, offset, size, error, resetsAt (this device's
   * time when an allowance wait ends), retryIn (seconds), resumed, unreadable,
   * and once cancelled `cancelAnswer` (`cancelSession`'s answer, '' when the
   * server never had a session) with `error` its words.
   */
  function createUploader(o) {
    const s = { phase: 'idle', id: '', offset: 0, size: o.file.size, chunkMax: 0, error: '', resetsAt: 0, retryIn: 0, resumed: false, unreadable: false, answer: null, cancelAnswer: '' };
    const Abort = o.AbortController || root.AbortController || null;
    let running = false;
    let pauseWanted = false;
    let resumeWanted = false;
    let cancelled = false;
    let cleaning = false;
    let wake = null;
    let inFlight = null;       // the AbortController of the request on its way, when it may be stopped
    let needStatus = false;    // ask the server where it got before the next piece
    let pieceCap = 0;          // what is left of the role's allowance, when a piece is cut to it
    let fp = typeof o.fp === 'string' ? o.fp : ''; // the file's fingerprint, worked out here when not given
    const now = () => (typeof o.now === 'function' ? o.now() : Date.now());
    const changed = () => { try { if (o.onChange) o.onChange(s); } catch (e) { /* a drawing failure never stops the upload */ } };
    const set = (phase, extra) => { s.phase = phase; Object.assign(s, extra || {}); changed(); };
    const stopping = () => cancelled || pauseWanted;
    const headers = (extra) => Object.assign({ 'X-Upload-Token': o.token() }, extra || {});

    // Back-off for dropped connections and server errors, and the count of one server error running.
    let backoff = RETRY_FIRST_MS;
    let sameError = { status: 0, count: 0 };
    function nextWait() {
      const wait = backoff;
      backoff = Math.min(backoff * 2, RETRY_MAX_MS);
      return wait;
    }
    function steady() {
      backoff = RETRY_FIRST_MS;
      sameError = { status: 0, count: 0 };
    }

    /**
     * Every wait, said on the page as `phase` when one is given. Pause and Cancel end it at once:
     * pressed during it, they cut it short (`wake`); pressed BEFORE it began (while the start was
     * on its way, which is not stopped, or a piece whose answer was already coming), it never
     * begins, so its phase is never shown and nothing sleeps. Round 4, W4-1: a Cancel during the
     * start, answered with the hour's allowance, used to sleep the server's whole `resets_in_s`.
     */
    function nap(ms, phase, extra) {
      if (stopping()) return Promise.resolve();
      return new Promise((resolve) => {
        let done = false;
        const fin = () => { if (!done) { done = true; if (wake === fin) wake = null; resolve(); } };
        wake = fin;
        if (phase) set(phase, extra);
        if (done) return;
        Promise.resolve(o.sleep ? o.sleep(ms) : new Promise((r) => setTimeout(r, ms))).then(fin, fin);
      });
    }

    /** A request Pause and Cancel can stop (every one but the start, whose answer names the session). */
    async function call(url, init) {
      const ctl = Abort ? new Abort() : null;
      inFlight = ctl;
      try {
        return await o.fetch(url, ctl ? Object.assign({}, init, { signal: ctl.signal }) : init);
      } finally {
        if (inFlight === ctl) inFlight = null;
      }
    }

    function at(id, query) {
      return '/api/upload/resumable' + (id ? '/' + encodeURIComponent(id) : '') + (query || '');
    }

    const read = readAnswer;

    function fail(words, extra) {
      if (s.id) forgetUpload(o.storage, s.id);
      set('failed', Object.assign({ error: words }, extra || {}));
    }

    function unreadable() {
      fail('"' + o.file.name + '" can no longer be read on this device (it was moved, changed, or the drive it is on was unplugged), so the upload stopped.', { unreadable: true });
    }

    function finish(answer) {
      if (s.id) forgetUpload(o.storage, s.id);
      s.offset = s.size;
      s.answer = answer;
      set('done', { answer });
      if (o.onDone) o.onDone(answer);
    }

    const isFinished = (j) => !!(j && (typeof j.url === 'string' || typeof j.filename === 'string'));

    /**
     * A server error (5xx, or a 429 a proxy sent): wait and try again. False when the same error
     * has come SAME_ERROR_TRIES times running (the upload then stops with its words and Retry),
     * or when Pause or Cancel came during the wait.
     */
    async function serverWait(status, words) {
      if (sameError.status === status) sameError.count++;
      else sameError = { status, count: 1 };
      if (sameError.count >= SAME_ERROR_TRIES) {
        set('server_error', { error: words });
        return false;
      }
      const wait = nextWait();
      await nap(wait, 'server_wait', { error: words, retryIn: Math.round(wait / 1000) });
      return !stopping();
    }

    /** A dropped connection: wait, then ask where the server got. False when Pause or Cancel came. */
    async function dropWait() {
      const wait = nextWait();
      needStatus = true;
      await nap(wait, 'reconnecting', { retryIn: Math.round(wait / 1000) });
      return !stopping();
    }

    /** How long an allowance makes it wait: the server's own seconds, at least 5. */
    function allowanceWait(j) {
      let ms = Number.isFinite(j.resets_in_s) ? j.resets_in_s * 1000
        : Number.isFinite(j.resets_at) ? j.resets_at - now() : 0;
      if (!(ms > ALLOWANCE_MIN_MS)) ms = ALLOWANCE_MIN_MS;
      return ms;
    }

    /** The allowance is used: wait the server's time. False when Pause or Cancel came during it. */
    async function allowanceStop(b) {
      const ms = allowanceWait(b.json);
      await nap(ms, 'allowance', { error: b.words, resetsAt: now() + ms, retryIn: Math.round(ms / 1000) });
      return !stopping();
    }

    /** Ask the server how far it got: {ok}, {done: answer}, {gone, words}, {retry, status, words} or {dropped}. */
    async function status() {
      let res;
      try { res = await call(at(s.id), { method: 'GET', headers: headers() }); } catch (e) { return { dropped: true }; }
      const b = await read(res);
      if (res.ok && b.json) {
        if (Number.isFinite(b.json.offset)) s.offset = b.json.offset;
        if (Number.isFinite(b.json.chunk_max) && b.json.chunk_max > 0) s.chunkMax = b.json.chunk_max;
        if (b.json.done && typeof b.json.done === 'object') return { done: b.json.done };
        changed();
        return { ok: true };
      }
      if (res.status === 404 || res.status === 410) return { gone: true, words: b.words };
      return { retry: true, status: res.status, words: b.words };
    }

    /** Wait until the page is signed in again (the token changes on every sign-in). */
    async function waitForToken() {
      while (!o.token() && !stopping()) await nap(1000, 'waiting_connection');
    }

    async function begin() {
      if (running) return;
      running = true;
      try {
        set('starting');
        await waitForToken();
        if (stopping()) return stopped();
        if (!s.id) {
          try { fp = fp || await fingerprint(o.file, o.digest); } catch (e) { return unreadable(); }
          const known = findUpload(o.storage, o.owner, o.file, fp, now());
          if (known) {
            // This same file, part way up already: ask the server where it got.
            s.id = known.id;
            for (;;) {
              const where = await status();
              if (stopping()) { needStatus = true; return stopped(); }
              if (where.done) return finish(where.done);
              if (where.ok) { s.resumed = true; steady(); changed(); return await loop(); }
              if (where.gone) {
                // The server no longer has it (a day without a piece, or cancelled elsewhere): start afresh.
                forgetUpload(o.storage, known.id);
                s.id = '';
                s.offset = 0;
                break;
              }
              if (where.dropped) { if (!await dropWait()) return stopped(); continue; }
              if (!await serverWait(where.status, where.words)) return stopped();
            }
          }
        }
        if (s.id) return await loop();
        for (;;) {
          if (stopping()) return stopped();
          await waitForToken();
          if (stopping()) return stopped();
          let res;
          try {
            // Not stopped by Pause or Cancel: its answer names the session, which Cancel then deletes.
            res = await o.fetch(at(''), {
              method: 'POST', headers: headers({ 'Content-Type': 'application/json' }),
              // keep_metadata: 1 only when the person chose to keep the file's hidden details (the
              // switch on the form, B4): the server then skips its own strip too.
              body: JSON.stringify(Object.assign({ filename: o.file.name, size: o.file.size, content_type: o.file.type || 'application/octet-stream', media: 1 }, o.keepMetadata ? { keep_metadata: 1 } : {})),
            });
          } catch (e) {
            if (!await dropWait()) return stopped();
            continue;
          }
          const b = await read(res);
          if (res.ok && b.json && typeof b.json.id === 'string') {
            steady();
            needStatus = false;
            s.id = b.json.id;
            s.offset = Number(b.json.offset) || 0;
            if (Number.isFinite(b.json.chunk_max) && b.json.chunk_max > 0) s.chunkMax = b.json.chunk_max;
            rememberUpload(o.storage, { owner: o.owner, id: s.id, name: o.file.name, size: o.file.size, lastModified: o.file.lastModified, fp, at: now() });
            return await loop();
          }
          if (res.status === 429 && isAllowance(b.json)) {
            if (!await allowanceStop(b)) return stopped();
            continue;
          }
          if ((res.status >= 500 && res.status !== 507) || res.status === 429) {
            if (!await serverWait(res.status, b.words)) return stopped();
            continue;
          }
          return fail(b.words);
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
      let staleTries = 0;
      for (;;) {
        if (stopping()) return stopped();
        await waitForToken();
        if (stopping()) return stopped();
        if (needStatus) {
          const where = await status();
          if (stopping()) { needStatus = true; return stopped(); }
          if (where.done) return finish(where.done);
          if (where.gone) return fail(where.words);
          if (where.dropped) { if (!await dropWait()) return stopped(); continue; }
          if (where.retry) { if (!await serverWait(where.status, where.words)) return stopped(); continue; }
          // The waits keep growing until a piece is taken: a server still busy with an earlier
          // piece is asked less and less often.
          needStatus = false;
        }
        const want = s.chunkMax || DEFAULT_CHUNK;
        const end = Math.min(s.size, s.offset + (pieceCap > 0 ? Math.min(want, pieceCap) : want));
        // At the end (after a refused finish, or a resume that found every byte there) this is
        // empty: an empty piece at offset = size tries the finish again.
        let piece;
        try { piece = await readSlice(o.file, s.offset, end); } catch (e) { return unreadable(); }
        if (stopping()) return stopped();
        set(s.offset >= s.size ? 'finishing' : 'sending');
        let res;
        try {
          res = await call(at(s.id, '?offset=' + s.offset), { method: 'PATCH', headers: headers({ 'Content-Type': 'application/octet-stream' }), body: piece });
        } catch (e) {
          piece = null;
          if (stopping()) { needStatus = true; return stopped(); }
          if (!await dropWait()) return stopped();
          continue;
        }
        const sentLen = piece.length;
        piece = null;
        const b = await read(res);
        const j = b.json || {};
        // Cancelled while this piece was on its way: the cancel's DELETE ends it, and when this
        // was the finish and the file is stored, the server deletes that too (round 4).
        if (cancelled) return undefined;
        if (res.ok) {
          steady();
          pieceCap = 0;
          staleTries = 0;
          if (isFinished(j)) return finish(j);
          if (Number.isFinite(j.offset)) s.offset = j.offset;
          touchUpload(o.storage, s.id, now());
          changed();
          continue;
        }
        if (res.status === 409 && j.busy) {
          // The server is still writing an earlier piece (an answer that was lost): wait, growing
          // each time, then ask where it got.
          const wait = nextWait();
          await nap(wait, 'server_wait', { error: b.words, retryIn: Math.round(wait / 1000) });
          needStatus = true;
          continue;
        }
        if (res.status === 409 && Number.isFinite(j.offset)) { s.offset = j.offset; continue; }
        if (res.status === 429 && isAllowance(j)) {
          if (Number.isFinite(j.offset)) s.offset = j.offset;
          const left = Number(j.left_bytes);
          if (left > 0 && left < sentLen) {
            // Send what still fits at once; the wait comes when nothing does.
            pieceCap = left;
            continue;
          }
          pieceCap = 0;
          if (!await allowanceStop(b)) return stopped();
          continue;
        }
        if (res.status === 507) {
          // Every byte arrived; the room had gone by the finish. The server keeps the file for a day.
          s.offset = s.size;
          return set('no_room', { error: b.words });
        }
        if (res.status >= 500 || res.status === 429) {
          if (!await serverWait(res.status, b.words)) return stopped();
          needStatus = true;
          continue;
        }
        if (res.status === 403 && /token/i.test(b.words) && staleTries < STALE_TOKEN_TRIES) {
          // A token from before the page signed in again: wait for the new one.
          staleTries++;
          await nap(1000);
          continue;
        }
        return fail(b.words);
      }
    }

    /**
     * After a run: a cancelled upload is deleted on the server (`cancelSession`: asked again while
     * the server is busy with a piece or the finish; a file the finish had stored is deleted by the
     * server with it, round 4). The state's `cancelAnswer` and `error` say what the server
     * answered, for the page to say.
     */
    async function afterRun() {
      if (resumeWanted && !cancelled) {
        // Resume was pressed while the pause was still stopping the run.
        resumeWanted = false;
        if (s.phase === 'paused') { pauseWanted = false; begin(); }
        return;
      }
      if (!cancelled || cleaning) return;
      cleaning = true;
      const id = s.id;
      if (id) forgetUpload(o.storage, id);
      const told = id ? await cancelSession(o, id) : { answer: '', words: '' };
      s.id = '';
      set('cancelled', { cancelAnswer: told.answer, error: told.words });
    }

    return {
      state: s,
      start() { if (!running) { pauseWanted = false; begin(); } },
      /** Stop now, in any phase: the request on its way is stopped and no other is made. */
      pause() {
        if (running && !cancelled) {
          pauseWanted = true;
          resumeWanted = false;
          set('pausing');
          if (inFlight) { try { inFlight.abort(); } catch (e) { /* already ended */ } }
          if (wake) wake();
        }
      },
      /** Resume after Pause, or try again after a refused finish or a server error. */
      resume() {
        if (cancelled) return;
        s.error = '';
        if (running) { if (pauseWanted) resumeWanted = true; return; }
        pauseWanted = false;
        // A fresh start for the waits and the count of one server error: Retry tries ten more times.
        steady();
        begin();
      },
      cancel() {
        if (cancelled) return;
        cancelled = true;
        set('cancelling');
        if (inFlight) { try { inFlight.abort(); } catch (e) { /* already ended */ } }
        if (wake) wake();
        if (!running) afterRun();
      },
      get running() { return running; },
    };
  }

  const api = {
    RESUME_KEY, RESUME_MAX_AGE_MS, DEFAULT_CHUNK, RETRY_FIRST_MS, RETRY_MAX_MS, SAME_ERROR_TRIES, ALLOWANCE_MIN_MS,
    CANCEL_TRIES, FINGERPRINT_BYTES, fingerprint, rememberUpload, touchUpload, findUpload, forgetUpload, isAllowance,
    readAnswer, cancelSession, createUploader,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else Object.assign(root, { MediaUpload: api });
})(typeof globalThis !== 'undefined' ? globalThis : this);
