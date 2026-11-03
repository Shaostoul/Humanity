// ── mine-tasks.js ─────────────────────────────────────────────────────────
// Tasks of my own ("Mine", docs/design/tasks.md rung 1, 2026-11-02; fixes
// BUG-217): the website's twin of the desktop app's src/gui/mine_tasks.rs.
// The same records, the same merge and the same note text, held to the desktop
// app's bytes by scripts/tests/fixtures/task-note-v1.json.
//
// - A task is kept in this browser only, in the chat's own encrypted store
//   (IndexedDB `humanity_dms`, its `meta` object store), one record whose scope
//   is SHA-256 of `<identity>\ntasks-mine`, its body AES-256-GCM under
//   HKDF-SHA-256 of the seed with the info `hum/tasks-mine-web/v1`
//   (/shared/pq-relay-auth.js getPqTasksKey). The chat page never opens it.
// - A change goes to my other devices as a sealed note to myself, the block
//   list's own way: an ordinary signed, sealed v2 DM from me to me whose text is
//   `[[hum:task:v1]]{"tasks":[...]}`, put in my own mailbox. A server sees padded
//   sealed mail in my own mailbox, never that it is a task.
// - THE MERGE: per id the record with the larger `updated_at` wins; at the same
//   time a deletion wins, then the later status, then the higher priority;
//   otherwise what is held stays. Order of arrival does not matter and a repeat
//   changes nothing.
//
// A classic script in the browser (its names land on window.HosMineTasks), a
// CommonJS module under Node (scripts/tests/tasks-mine-web.test.js). The
// browser half needs /chat/pq.js loaded first (pqSignMessage, pqVerifyMessage,
// pqDmSeal, pqDmOpen).
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  // Must match native src/net/dm_pq.rs CTL_TASK and web/chat/crypto.js CTL_TASK.
  const CTL_TASK = '[[hum:task:v1]]';
  // The limits (src/gui/mine_tasks.rs).
  const TITLE_MAX = 200;
  const DESCRIPTION_MAX = 3000;
  const LABELS_MAX = 12;
  const LABEL_MAX = 32;
  const NOTE_TEXT_MAX = 19000;
  const STATUSES = ['todo', 'in_progress', 'done'];
  const PRIORITIES = ['low', 'medium', 'high', 'critical'];
  // A DM's inner signature domain and the padding buckets: Chat's own
  // (web/chat/crypto.js DM_SIG_DOMAIN_V2, DM_PAD_BUCKETS), which the test holds
  // this copy to, and the desktop app's (src/net/dm_pq.rs).
  const DM_SIG_DOMAIN_V2 = 'hum/dm/v2';
  const DM_PAD_BUCKETS = [256, 1024, 4096, 16384, 32768];

  const ID_RE = /^[0-9a-f]{32}$/;
  const CHAIN_RE = /^[a-z0-9_-]{1,64}$/;
  // Control characters (Unicode Cc) other than the tab and the line break.
  const CONTROL_RE = /[\u0000-\u0008\u000B-\u001F\u007F-\u009F]/g;

  const isId = (id) => typeof id === 'string' && ID_RE.test(id);
  const isTime = (n) => Number.isSafeInteger(n) && n >= 0;

  /** Words kept: no control characters but line breaks and tabs, at most `max` characters. */
  function clean(text, max) {
    return Array.from(String(text).replace(CONTROL_RE, '')).slice(0, max).join('');
  }

  /** A new task id: 128 random bits. */
  function newId() {
    const bytes = root.crypto.getRandomValues(new Uint8Array(16));
    return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
  }

  /** A deletion: the id and when, nothing else. */
  function gone(id, at) {
    return { id, deleted: true, updated_at: at };
  }

  /**
   * The record as it may be kept, or null when it does not read: the id is not
   * 32 lowercase hex, or a field has the wrong type (the desktop app's reader
   * drops such a record too). A deletion keeps no words; the words are clamped;
   * the status and priority are the known words. The keys come out in the
   * desktop app's order with empty ones left out, so both write the same bytes.
   */
  function clampRecord(r) {
    if (!r || typeof r !== 'object' || Array.isArray(r) || !isId(r.id)) return null;
    const str = (k) => r[k] === undefined || typeof r[k] === 'string';
    if (!['title', 'description', 'status', 'priority'].every(str)) return null;
    if (r.deleted !== undefined && typeof r.deleted !== 'boolean') return null;
    if (r.labels !== undefined && !(Array.isArray(r.labels) && r.labels.every((l) => typeof l === 'string'))) return null;
    if (r.chain !== undefined && r.chain !== null && typeof r.chain !== 'string') return null;
    if (r.created_at !== undefined && !isTime(r.created_at)) return null;
    if (r.updated_at !== undefined && !isTime(r.updated_at)) return null;
    const at = r.updated_at || 0;
    if (r.deleted === true) return gone(r.id, at);
    const out = { id: r.id };
    const title = clean(r.title || '', TITLE_MAX);
    if (title) out.title = title;
    const description = clean(r.description || '', DESCRIPTION_MAX);
    if (description) out.description = description;
    out.status = STATUSES.includes(r.status) ? r.status : 'todo';
    out.priority = PRIORITIES.includes(r.priority) ? r.priority : 'medium';
    const labels = (r.labels || []).map((l) => clean(l, LABEL_MAX).trim()).filter(Boolean).slice(0, LABELS_MAX);
    if (labels.length) out.labels = labels;
    if (typeof r.chain === 'string' && CHAIN_RE.test(r.chain)) out.chain = r.chain;
    if (r.created_at) out.created_at = r.created_at;
    out.updated_at = at;
    return out;
  }

  function rank(r) {
    if (r.deleted) return [r.updated_at || 0, 1, 0, 0];
    return [r.updated_at || 0, 0, STATUSES.indexOf(r.status), PRIORITIES.indexOf(r.priority)];
  }

  /** Does `a` win over `b` (the same task)? A tie keeps `b`. */
  function wins(a, b) {
    const x = rank(a);
    const y = rank(b);
    for (let i = 0; i < x.length; i++) if (x[i] !== y[i]) return x[i] > y[i];
    return false;
  }

  /** Does this DM text start with the marker? Such a text is never shown as a message. */
  function isNote(text) {
    return typeof text === 'string' && text.startsWith(CTL_TASK);
  }

  const utf8Len = (s) => new TextEncoder().encode(s).length;

  /** The notes that carry `records`, in order, as many in each as fit NOTE_TEXT_MAX bytes. */
  function noteTexts(records) {
    const HEAD = '{"tasks":[';
    const TAIL = ']}';
    const notes = [];
    let body = '';
    let bodyLen = 0;
    const fixed = utf8Len(CTL_TASK) + HEAD.length + TAIL.length;
    for (const r of records) {
      const json = JSON.stringify(r);
      const len = utf8Len(json);
      if (body && fixed + bodyLen + 1 + len > NOTE_TEXT_MAX) {
        notes.push(CTL_TASK + HEAD + body + TAIL);
        body = '';
        bodyLen = 0;
      }
      if (body) { body += ','; bodyLen += 1; }
      body += json;
      bodyLen += len;
    }
    if (body) notes.push(CTL_TASK + HEAD + body + TAIL);
    return notes;
  }

  /** The records a note carries, each clamped; those that do not read are left out. */
  function parseNote(text) {
    if (!isNote(text)) return [];
    let v;
    try { v = JSON.parse(text.slice(CTL_TASK.length)); } catch { return []; }
    if (!v || !Array.isArray(v.tasks)) return [];
    return v.tasks.map(clampRecord).filter(Boolean);
  }

  /**
   * The tasks of my own, in memory: a Map id -> record (live and deleted), the
   * ids not sent yet, and the mailbox read position per server.
   */
  class MineTasks {
    constructor() {
      this.records = new Map();
      this.unsent = [];
      this.hw = {};
    }

    /** The live tasks, in the order they were made (then by id), as every device lists them. */
    tasks() {
      return [...this.records.values()].filter((r) => !r.deleted)
        .sort((a, b) => ((a.created_at || 0) - (b.created_at || 0)) || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
    }

    get(id) {
      const r = this.records.get(id);
      return r && !r.deleted ? r : null;
    }

    hasChain(chain) {
      return this.tasks().some((t) => t.chain === chain);
    }

    /** The time a change made here carries: now, or just after the version it changes. */
    stamp(id, now) {
      const before = this.records.get(id);
      return Math.max(now, (before ? before.updated_at || 0 : 0) + 1);
    }

    /** A new task made here at `now`; returns its id. */
    add(fields, now) {
      const id = newId();
      const r = clampRecord({
        id,
        title: String(fields.title || ''),
        description: String(fields.description || ''),
        status: fields.status || 'todo',
        priority: fields.priority || 'medium',
        labels: Array.isArray(fields.labels) ? fields.labels.map(String) : [],
        chain: typeof fields.chain === 'string' ? fields.chain : undefined,
        created_at: now,
        updated_at: now,
      });
      if (r) this.records.set(id, r);
      return id;
    }

    /** Move a task to another column and/or give it another priority. False when nothing changed. */
    change(id, { status, priority }, now) {
      const t = this.get(id);
      if (!t) return false;
      const next = { ...t, status: status || t.status, priority: priority || t.priority };
      if (next.status === t.status && next.priority === t.priority) return false;
      next.updated_at = this.stamp(id, now);
      const r = clampRecord(next);
      if (r) this.records.set(id, r);
      return !!r;
    }

    /** Delete a task: a deletion takes its place. False when no live task has that id. */
    remove(id, now) {
      if (!this.get(id)) return false;
      this.records.set(id, gone(id, this.stamp(id, now)));
      return true;
    }

    /** Apply records (from a note, or the store): each replaces what is held when it wins. */
    apply(records) {
      let changed = false;
      for (const raw of records || []) {
        const r = clampRecord(raw);
        if (!r) continue;
        const held = this.records.get(r.id);
        if (!held || wins(r, held)) {
          this.records.set(r.id, r);
          changed = true;
        }
      }
      return changed;
    }

    record(id) {
      return this.records.get(id) || null;
    }

    markUnsent(ids) {
      for (const id of ids) if (!this.unsent.includes(id)) this.unsent.push(id);
    }

    takeUnsent() {
      const ids = this.unsent;
      this.unsent = [];
      return ids;
    }

    /** What the store keeps (encrypted). */
    body() {
      return { records: [...this.records.values()], unsent: this.unsent.slice(), hw: { ...this.hw } };
    }

    /**
     * Fold a stored body into this one (another tab may have written since):
     * the merge and the furthest read position; the unsent ids only when
     * loading (`withUnsent`), since this page clears the ones it has sent.
     */
    absorb(body, withUnsent) {
      if (!body || typeof body !== 'object') return;
      this.apply(Array.isArray(body.records) ? body.records : []);
      if (withUnsent) this.markUnsent(Array.isArray(body.unsent) ? body.unsent.filter(isId) : []);
      if (body.hw && typeof body.hw === 'object') {
        for (const [server, n] of Object.entries(body.hw)) {
          if (isTime(n) && n > (this.hw[server] || 0)) this.hw[server] = n;
        }
      }
    }
  }

  // ── The browser half: the encrypted record, and the sealed notes ─────────

  const hex = (u8) => Array.from(u8, (b) => b.toString(16).padStart(2, '0')).join('');
  function hexToBytes(h) {
    const out = new Uint8Array(h.length / 2);
    for (let i = 0; i < out.length; i++) out[i] = parseInt(h.substr(i * 2, 2), 16);
    return out;
  }
  function b64(u8) {
    let s = '';
    for (let i = 0; i < u8.length; i++) s += String.fromCharCode(u8[i]);
    return btoa(s);
  }

  function idbReq(req) {
    return new Promise((resolve, reject) => {
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => reject(req.error);
    });
  }

  /** The chat's own database, opened exactly as web/chat/chat-dm-store.js opens it. */
  function openDb(idb) {
    return new Promise((resolve, reject) => {
      const req = idb.open('humanity_dms', 1);
      req.onupgradeneeded = () => {
        const db = req.result;
        if (!db.objectStoreNames.contains('msgs')) {
          const msgs = db.createObjectStore('msgs', { keyPath: 'k' });
          msgs.createIndex('scope', 'scope', { unique: false });
        }
        if (!db.objectStoreNames.contains('meta')) db.createObjectStore('meta', { keyPath: 'scope' });
      };
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => reject(req.error);
    });
  }

  /**
   * My tasks' encrypted record for identity `me`: { list, load(), save(), problem() }.
   * `key` is the AES-GCM CryptoKey (getPqTasksKey). save() reads the record
   * again and merges before it writes, so two open pages lose nothing.
   *
   * A RECORD THAT DOES NOT OPEN IS NEVER WRITTEN OVER (2026-11-02, the review
   * of rung 1). Only a record that is not there is an ordinary empty start.
   * One that is there but does not open (damaged, or not this seed's) is first
   * copied, as it is, to a key of its own in the same store
   * (`<scope>:unreadable-<unix seconds>`), and the list starts empty from
   * there; a read the browser's storage refused, or a copy it refused, keeps
   * this page from saving over the record at all. problem() says which, for
   * the page to say so: null, {kind: 'kept', key} or {kind: 'unreadable'}.
   * Changes still go to my other devices as notes either way.
   */
  async function openStore(me, key, idb) {
    const digest = await root.crypto.subtle.digest('SHA-256', new TextEncoder().encode(`${me}\ntasks-mine`));
    const scope = hex(new Uint8Array(digest));
    const db = await openDb(idb);
    const list = new MineTasks();
    const meta = (mode) => db.transaction('meta', mode).objectStore('meta');
    let problem = null;
    let keptThisVisit = false; // the damaged record was copied aside: writing over it now loses nothing
    // The record as it is: {state: 'none'}, {state: 'ok', body}, {state: 'damaged', row} or {state: 'error'}.
    const read = async () => {
      let row;
      try { row = await idbReq(meta('readonly').get(scope)); } catch { return { state: 'error' }; }
      if (row === undefined || row === null) return { state: 'none' };
      try {
        const plain = await root.crypto.subtle.decrypt({ name: 'AES-GCM', iv: new Uint8Array(row.box.iv) }, key, new Uint8Array(row.box.ct));
        const body = JSON.parse(new TextDecoder().decode(plain));
        if (!body || typeof body !== 'object') throw new Error('not a body');
        return { state: 'ok', body };
      } catch { return { state: 'damaged', row }; }
    };
    // Copy a record that does not open to a key of its own, as it is. False when the store refused.
    const keepAside = async (row) => {
      try {
        const base = `${scope}:unreadable-${Math.floor(Date.now() / 1000)}`;
        let k = base;
        for (let n = 1; await idbReq(meta('readonly').get(k)); n++) k = `${base}-${n}`;
        await idbReq(meta('readwrite').put({ ...row, scope: k, kept_from: scope, kept_at: Date.now() }));
        problem = { kind: 'kept', key: k };
        keptThisVisit = true;
        return true;
      } catch {
        problem = { kind: 'unreadable' };
        return false;
      }
    };
    // What a read found, taken in: false when nothing may be written over the record.
    const take = async (r, withUnsent) => {
      if (r.state === 'error') { problem = { kind: 'unreadable' }; return false; }
      if (r.state === 'damaged' && !keptThisVisit) return keepAside(r.row);
      if (r.state === 'ok') list.absorb(r.body, withUnsent);
      return true;
    };
    let saving = Promise.resolve();
    return {
      list,
      problem: () => problem,
      async load() { await take(await read(), true); },
      save() {
        saving = saving.then(async () => {
          if (problem && problem.kind === 'unreadable') return;
          if (!await take(await read(), false)) return;
          const iv = root.crypto.getRandomValues(new Uint8Array(12));
          const ct = new Uint8Array(await root.crypto.subtle.encrypt({ name: 'AES-GCM', iv }, key, new TextEncoder().encode(JSON.stringify(list.body()))));
          await idbReq(meta('readwrite').put({ scope, box: { iv: Array.from(iv), ct: Array.from(ct) } }));
        }).catch((e) => console.warn('[tasks] my own tasks were not saved:', e && e.message));
        return saving;
      },
    };
  }

  function sigPreimage(from, to, ts, text) {
    return `${DM_SIG_DOMAIN_V2}\n${from}\n${to}\n${ts}\n${text}`;
  }

  /**
   * A note to myself carrying `text`: the `dm_put` to my own mailbox, signed
   * and padded like any DM and sealed to my own DM key, or null when the
   * identity cannot sign or seal here. `ids`: {me, dilithiumSecret, kyberPublicB64}.
   */
  async function sealSelfNote(text, ts, ids) {
    if (!ids || !ids.me || !ids.dilithiumSecret || !ids.kyberPublicB64) return null;
    if (typeof root.pqSignMessage !== 'function' || typeof root.pqDmSeal !== 'function') return null;
    const sig = await root.pqSignMessage(ids.dilithiumSecret, new TextEncoder().encode(sigPreimage(ids.me, ids.me, ts, text)));
    if (!sig) return null;
    const inner = { v: 2, from: ids.me, to: ids.me, ts, text, sig: b64(sig) };
    // Padding as Chat pads (crypto.js pqBuildDmPuts), so a note looks like any DM.
    const bare = JSON.stringify(inner).length;
    const bucket = DM_PAD_BUCKETS.find((b) => bare + 12 <= b) || (bare + 12);
    inner.pad = ' '.repeat(Math.max(0, bucket - bare - 12));
    const sealed = await root.pqDmSeal(ids.kyberPublicB64, JSON.stringify(inner));
    if (!sealed) return null;
    return { type: 'dm_put', to: ids.me, content: JSON.stringify({ v: 2, ek_ct_b64: sealed.ek_ct_b64, nonce_b64: sealed.nonce_b64, ct_b64: sealed.ct_b64 }) };
  }

  /**
   * Open one envelope from my mailbox: the records of a task note from me to me,
   * its signature checked; [] for anything else (another DM, a note from someone
   * else, one that does not open or whose signature fails). Only a task note
   * from me to me has its signature checked, so other mail costs one open.
   * `ids`: {me, kyberSecret}.
   */
  async function takeEnvelope(content, ids) {
    try {
      if (typeof root.pqDmOpen !== 'function' || !ids || !ids.kyberSecret) return [];
      const env = JSON.parse(content);
      if (!env || env.v !== 2 || !env.ek_ct_b64 || !env.nonce_b64 || !env.ct_b64) return [];
      const innerJson = await root.pqDmOpen(ids.kyberSecret, env.ek_ct_b64, env.nonce_b64, env.ct_b64);
      if (!innerJson) return [];
      const inner = JSON.parse(innerJson);
      if (!inner || inner.v !== 2 || !isNote(inner.text)) return [];
      if (inner.from !== ids.me || inner.to !== ids.me || typeof inner.sig !== 'string') return [];
      const sig = Uint8Array.from(atob(inner.sig), (c) => c.charCodeAt(0));
      const ok = await root.pqVerifyMessage(hexToBytes(inner.from), new TextEncoder().encode(sigPreimage(inner.from, inner.to, Number(inner.ts) || 0, String(inner.text))), sig);
      return ok ? parseNote(inner.text) : [];
    } catch { return []; }
  }

  const api = {
    CTL_TASK, TITLE_MAX, DESCRIPTION_MAX, LABELS_MAX, LABEL_MAX, NOTE_TEXT_MAX, STATUSES, PRIORITIES,
    DM_SIG_DOMAIN_V2, DM_PAD_BUCKETS,
    isId, clampRecord, gone, wins, isNote, noteTexts, parseNote, newId, MineTasks,
    openStore, sealSelfNote, takeEnvelope,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else root.HosMineTasks = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
