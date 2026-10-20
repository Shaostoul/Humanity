// ── kept-files.js ─────────────────────────────────────────────────────────
// A copy, on this device, of every private file the person opened (2026-10-11,
// docs/design/files-showcase.md, "Files on the sender's device and in
// messages, exact", part 4; the operator's answer 9). A file sent in a DM or a
// group is removed from the server after the DM mailbox's lifetime (30 days
// by default), so a picture already looked at, or a file already saved, is
// kept here and still opens after that. Opening a private file looks here
// first (web/chat/chat-dms.js openPrivateFile); only when it is not here is it
// fetched from the server.
//
// How it is kept:
//   - in its own IndexedDB database, `humanity_kept_files`, beside the
//     encrypted DM history (`humanity_dms`, web/chat/chat-dm-store.js), never
//     inside it: a new store there would need a version change that an open
//     chat tab from before would block;
//   - encrypted at rest with the DM store's own key (AES-256-GCM, derived from
//     the person's identity, crypto.js getDmStoreKey), so a copied database is
//     unreadable without the recovery phrase, as the DM history is; each
//     record's id is bound into its encryption, so records cannot be swapped;
//   - under an id that names nothing: SHA-256 of the store's scope (identity
//     and server, itself a hash), the file's address and its key, so neither
//     the address nor the key is in the database in the clear;
//   - with only its size and an opening order beside it (`index`), so the
//     Settings page can show how much is kept, enforce the cap and clear it,
//     without the key;
//   - up to a cap per device (default 200 MB, Settings > Storage, "Files kept
//     from messages"; 0 keeps nothing), the oldest opened going first. A file
//     larger than the cap is not kept.
//
// Clear kept files (the Settings page) removes every copy on this device.
//
// A classic script in the browser (HosKeptFiles lands on window), a CommonJS
// module under Node, where scripts/tests/kept-files-web.test.js holds it.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  const DB_NAME = 'humanity_kept_files';
  const DB_VERSION = 1;
  const FILES = 'files';
  const INDEX = 'index';
  const CAP_KEY = 'hos_kept_files_cap_mb';
  const MIB = 1024 * 1024;
  const DEFAULT_CAP_MB = 200;
  // The caps offered in Settings (MB); 0 keeps nothing.
  const CAP_CHOICES_MB = Object.freeze([0, 50, 100, 200, 500, 1000]);
  const ID_DOMAIN = 'hum/kept-file/v1';

  const WORDS = Object.freeze({
    title: 'Files kept from messages',
    explain: 'Pictures and files you open in a direct message or a group are kept on this device, encrypted with your identity, so they still open after the server removes its copy (after 30 days on most servers). When the space below is full, the ones opened longest ago go first.',
    capLabel: 'Keep at most',
    capNone: 'Nothing (do not keep copies)',
    clear: 'Clear kept files',
    clearConfirm: 'Clear every file kept from messages on this device? Files the server no longer holds will not open again.',
    cleared: 'Cleared. No files from messages are kept on this device now.',
    nothing: 'Nothing is kept on this device yet.',
    unavailable: 'This browser cannot keep files here (its storage is turned off or full).',
  });

  const enc = new TextEncoder();

  function cryptoOf(c) {
    return c || root.crypto;
  }

  function mbWords(mb) {
    if (mb >= 1000 && mb % 1000 === 0) return (mb / 1000) + ' GB';
    return mb + ' MB';
  }

  function sizeWords(bytes) {
    const n = Number(bytes) || 0;
    if (n < 1024) return n + ' B';
    if (n < MIB) return Math.round(n / 1024) + ' KB';
    return (n / MIB).toFixed(1) + ' MB';
  }

  /** The cap in MB this device keeps (Settings), the default when nothing is kept or it is unreadable. */
  function capMb(storage) {
    let raw = null;
    try { raw = storage ? storage.getItem(CAP_KEY) : null; } catch (e) { raw = null; }
    if (raw == null || raw === '') return DEFAULT_CAP_MB;
    const n = Number(raw);
    return Number.isFinite(n) && n >= 0 ? n : DEFAULT_CAP_MB;
  }

  function capBytes(storage) {
    return Math.floor(capMb(storage) * MIB);
  }

  function setCapMb(storage, mb) {
    const n = Number(mb);
    if (!Number.isFinite(n) || n < 0) return false;
    try { if (storage) storage.setItem(CAP_KEY, String(n)); return true; } catch (e) { return false; }
  }

  // ── IndexedDB ──────────────────────────────────────────────────────────

  function req(r) {
    return new Promise((resolve, reject) => {
      r.onsuccess = () => resolve(r.result);
      r.onerror = () => reject(r.error);
    });
  }

  const opened = new WeakMap(); // the IndexedDB factory -> Promise<db>

  function openDb(idb) {
    if (!idb || typeof idb.open !== 'function') return Promise.reject(new Error('no IndexedDB'));
    if (opened.has(idb)) return opened.get(idb);
    const p = new Promise((resolve, reject) => {
      const r = idb.open(DB_NAME, DB_VERSION);
      r.onupgradeneeded = () => {
        const db = r.result;
        if (!db.objectStoreNames.contains(FILES)) db.createObjectStore(FILES, { keyPath: 'k' });
        if (!db.objectStoreNames.contains(INDEX)) db.createObjectStore(INDEX, { keyPath: 'k' });
      };
      r.onsuccess = () => resolve(r.result);
      r.onerror = () => reject(r.error);
    });
    p.catch(() => opened.delete(idb));
    opened.set(idb, p);
    return p;
  }

  function store(db, name, mode) {
    return db.transaction(name, mode).objectStore(name);
  }

  // The opening order: one counter per database, from the highest kept.
  const seqs = new WeakMap(); // db -> next seq

  async function nextSeq(db) {
    if (!seqs.has(db)) {
      const rows = (await req(store(db, INDEX, 'readonly').getAll())) || [];
      let top = 0;
      for (const r of rows) if (Number(r.seq) > top) top = Number(r.seq);
      seqs.set(db, top + 1);
    }
    const n = seqs.get(db);
    seqs.set(db, n + 1);
    return n;
  }

  /** The record id for a file in a scope: hex SHA-256 of the scope, the address and the key. */
  async function recordId(scope, meta, c) {
    const words = `${ID_DOMAIN}\n${scope || ''}\n${String(meta && meta.url)}\n${String(meta && meta.k)}`;
    const d = await cryptoOf(c).subtle.digest('SHA-256', enc.encode(words));
    return Array.from(new Uint8Array(d)).map((b) => b.toString(16).padStart(2, '0')).join('');
  }

  /**
   * The kept copy of a file, opened with `key` (the DM store's key), or null when there is none
   * (or it does not open: it is then removed). A copy found counts as the newest opened.
   * o: {indexedDB, key, scope, meta, crypto?}
   */
  async function get(o) {
    try {
      if (!o || !o.key || !o.scope || !o.meta) return null;
      const c = cryptoOf(o.crypto);
      const db = await openDb(o.indexedDB);
      const id = await recordId(o.scope, o.meta, c);
      const rec = await req(store(db, FILES, 'readonly').get(id));
      if (!rec || !rec.iv || !rec.ct) return null;
      let plain;
      try {
        plain = await c.subtle.decrypt({ name: 'AES-GCM', iv: new Uint8Array(rec.iv), additionalData: enc.encode(id) }, o.key, rec.ct);
      } catch (e) {
        await forget(db, id);
        return null;
      }
      // Opened now: it goes last when space is needed.
      const row = await req(store(db, INDEX, 'readonly').get(id));
      const seq = await nextSeq(db);
      await req(store(db, INDEX, 'readwrite').put({ k: id, seq, size: row && Number(row.size) ? Number(row.size) : rec.ct.byteLength }));
      return new Uint8Array(plain);
    } catch (e) {
      return null;
    }
  }

  async function forget(db, id) {
    try { await req(store(db, FILES, 'readwrite').delete(id)); } catch (e) { /* already gone */ }
    try { await req(store(db, INDEX, 'readwrite').delete(id)); } catch (e) { /* already gone */ }
  }

  // One change at a time on this page: put, trim and clear wait for each other. Two files opened at
  // once (a DM's pictures open together) used to interleave, and the first put's trim, having read
  // the sizes before the second wrote its own, took the second's copy for a leftover and removed it
  // while that put still answered true (the web review of 2026-10-11).
  let queue = Promise.resolve();
  function queued(fn) {
    const run = queue.then(fn, fn);
    queue = run.then(() => undefined, () => undefined);
    return run;
  }

  /**
   * Keep an opened file, encrypted with `key`, then hold the whole to `cap` bytes (the oldest
   * opened first out). Returns true when it was kept. A file larger than the cap, or a cap of 0,
   * keeps nothing.
   * o: {indexedDB, key, scope, meta, bytes, cap, crypto?}
   */
  function put(o) {
    return queued(() => putNow(o));
  }

  async function putNow(o) {
    try {
      if (!o || !o.key || !o.scope || !o.meta || !o.bytes) return false;
      const cap = Number(o.cap);
      if (!(cap > 0)) return false;
      if (o.bytes.byteLength + 16 > cap) return false;
      const c = cryptoOf(o.crypto);
      const db = await openDb(o.indexedDB);
      const id = await recordId(o.scope, o.meta, c);
      const iv = c.getRandomValues(new Uint8Array(12));
      const ct = new Uint8Array(await c.subtle.encrypt({ name: 'AES-GCM', iv, additionalData: enc.encode(id) }, o.key, o.bytes));
      const seq = await nextSeq(db);
      await req(store(db, FILES, 'readwrite').put({ k: id, iv, ct }));
      await req(store(db, INDEX, 'readwrite').put({ k: id, seq, size: ct.byteLength }));
      await trimNow({ indexedDB: o.indexedDB, cap });
      return true;
    } catch (e) {
      return false;
    }
  }

  /**
   * Hold what is kept to `cap` bytes: the oldest opened go first. A copy with no size beside it
   * (left by a write that did not finish) goes too. Returns {count, bytes} kept after.
   * o: {indexedDB, cap}
   */
  function trim(o) {
    return queued(() => trimNow(o));
  }

  async function trimNow(o) {
    const db = await openDb(o.indexedDB);
    const cap = Math.max(0, Number(o.cap) || 0);
    const rows = ((await req(store(db, INDEX, 'readonly').getAll())) || []).slice();
    rows.sort((a, b) => Number(a.seq) - Number(b.seq));
    let total = rows.reduce((n, r) => n + (Number(r.size) || 0), 0);
    let i = 0;
    while (total > cap && i < rows.length) {
      await forget(db, rows[i].k);
      total -= Number(rows[i].size) || 0;
      i++;
    }
    const known = new Set(rows.slice(i).map((r) => r.k));
    const keys = (await req(store(db, FILES, 'readonly').getAllKeys())) || [];
    for (const k of keys) if (!known.has(k)) await forget(db, k);
    return { count: rows.length - i, bytes: total };
  }

  /** How much is kept: {count, bytes}. */
  async function usage(o) {
    const db = await openDb(o.indexedDB);
    const rows = (await req(store(db, INDEX, 'readonly').getAll())) || [];
    return { count: rows.length, bytes: rows.reduce((n, r) => n + (Number(r.size) || 0), 0) };
  }

  /** Remove every kept file on this device. */
  function clear(o) {
    return queued(() => clearNow(o));
  }

  async function clearNow(o) {
    const db = await openDb(o.indexedDB);
    await req(store(db, FILES, 'readwrite').clear());
    await req(store(db, INDEX, 'readwrite').clear());
    return true;
  }

  // ── Settings > Storage: the controls ───────────────────────────────────

  function usageWords(u) {
    if (!u || !u.count) return WORDS.nothing;
    return `Kept on this device: ${u.count} ${u.count === 1 ? 'file' : 'files'}, ${sizeWords(u.bytes)}.`;
  }

  /**
   * Draw the controls into `container` (the web Settings page's Storage section): what is kept,
   * the cap, and Clear kept files. env: {document, storage, indexedDB, confirm(words) -> Promise<bool>}.
   * Returns {refresh, el}.
   */
  function mountControls(container, env) {
    const doc = env.document;
    const h = (tag, cls, text) => {
      const el = doc.createElement(tag);
      if (cls) el.className = cls;
      if (text != null) el.textContent = text;
      return el;
    };
    const box = h('div', 'kept-files');
    const title = h('div', 'settings-group-title', WORDS.title);
    const explain = h('p', 'kept-files-explain', WORDS.explain);
    const used = h('p', 'kept-files-usage', '');
    used.dataset.keptUsage = '1';
    const row = h('div', 'setting-row');
    const label = h('label', 'setting-label', WORDS.capLabel);
    const select = h('select', 'kept-files-cap');
    select.dataset.keptCap = '1';
    const current = capMb(env.storage);
    const choices = CAP_CHOICES_MB.includes(current) ? CAP_CHOICES_MB : CAP_CHOICES_MB.concat([current]).sort((a, b) => a - b);
    for (const mb of choices) {
      const opt = h('option', null, mb === 0 ? WORDS.capNone : mbWords(mb));
      opt.value = String(mb);
      if (mb === current) opt.selected = true;
      select.appendChild(opt);
    }
    row.appendChild(label);
    row.appendChild(select);
    const clearBtn = h('button', 'btn-danger', WORDS.clear);
    clearBtn.type = 'button';
    clearBtn.dataset.keptClear = '1';
    const said = h('p', 'kept-files-said', '');
    box.appendChild(title);
    box.appendChild(explain);
    box.appendChild(used);
    box.appendChild(row);
    box.appendChild(clearBtn);
    box.appendChild(said);
    container.appendChild(box);

    async function refresh() {
      try {
        used.textContent = usageWords(await usage({ indexedDB: env.indexedDB }));
      } catch (e) {
        used.textContent = WORDS.unavailable;
      }
    }
    select.addEventListener('change', async () => {
      const mb = Number(select.value);
      setCapMb(env.storage, mb);
      try { await trim({ indexedDB: env.indexedDB, cap: Math.floor(mb * MIB) }); } catch (e) { /* refresh says so */ }
      await refresh();
    });
    clearBtn.addEventListener('click', async () => {
      const yes = typeof env.confirm === 'function' ? await env.confirm(WORDS.clearConfirm) : true;
      if (!yes) return;
      clearBtn.disabled = true;
      try {
        await clear({ indexedDB: env.indexedDB });
        said.textContent = WORDS.cleared;
      } catch (e) {
        said.textContent = WORDS.unavailable;
      }
      clearBtn.disabled = false;
      await refresh();
    });
    const ready = refresh();
    return { refresh, el: box, ready };
  }

  const api = {
    DB_NAME, CAP_KEY, DEFAULT_CAP_MB, CAP_CHOICES_MB, ID_DOMAIN, WORDS,
    capMb, capBytes, setCapMb, recordId, get, put, trim, usage, clear, usageWords, mountControls,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else root.HosKeptFiles = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
