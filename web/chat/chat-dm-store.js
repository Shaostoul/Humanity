// ── chat-dm-store.js ──────────────────────────────────────────────────────
// Local encrypted DM history (sealed-sender cutover, 2026-08-23).
//
// The relay's dm_mailbox is a delivery window: envelopes carry no sender
// and expire after the server's TTL. Long-term DM history therefore lives
// HERE, in IndexedDB, with every record body AES-GCM-encrypted under a
// seed-derived key (crypto.js getDmStoreKey) — a copied IndexedDB is
// unreadable without the seed, which genuinely protects wrapped-key users.
//
// Scope: one logical store per (identity, server), keyed by a SHA-256 tag
// so neither the identity nor the server URL appears in IndexedDB in the
// clear. The mailbox fetch high-water mark is per scope (row ids are
// per-relay).
//
// Depends on: crypto.js (getDmStoreKey). Loaded before app.js so the
// message handlers can use it. All methods are safe to call before
// init() — they no-op / return empties.
// ─────────────────────────────────────────────────────────────────────────

const hosDmStore = {
  _db: null,
  _key: null,          // CryptoKey (AES-GCM, non-extractable)
  scope: null,         // SHA-256 hex tag of `${me}\n${server}`
  me: null,            // our identity hex
  highWater: 0,        // last fetched mailbox row id for this scope
  conversations: new Map(), // peer -> [{from,to,ts,text,sig,dedupe}] sorted by ts
  lastRead: {},        // peer -> ts
  _seen: new Set(),    // dedupe tags in memory
  // ── Client-side social graph (follows removal, 2026-08-24): the server
  // stores no edges; these sets ARE the user's social state, persisted in
  // the encrypted meta box and built from sealed control messages.
  following: new Set(),
  followers: new Set(),
  // ── Friendship passes v2 (2026-10-09, blocking-and-safe-mode.md 10b). Saved
  // under new names (passesFrom / passesSent): the v1 certsFrom / certsSent a
  // store from before holds are not read, so they are gone on the next save and
  // the pass sweep (chat-social.js sweepFriendPasses) mints v2 for every mutual
  // follow.
  passServer: '',      // did:hum of the server these passes name
  certsFrom: {},       // peer -> the pass THEY gave ME (v2 JSON), presented whenever I reach them
  certsSent: {},       // peer -> [{serial, may}] passes I gave them that still stand
  withdrawalsPending: [], // serials I withdrew that the relay has not confirmed yet
  // ── Contact requests ("who can reach me", step B, 2026-10-09, 10c): people
  // who asked to reach me, shown by name only with Accept and Ignore, keyed by
  // their (signed) key. `pass` is the pass their request carried, which my
  // reply presents to their relay on Accept; null for a DM my settings refused
  // (no pass came with it). Never holds any text.
  contactRequests: {}, // key -> {key, name, pass, ts}
  // ── Blocked people (step C, 2026-10-09, blocking-and-safe-mode.md 10d):
  // identity keys (lowercase hex), never names, each with when it was
  // blocked. Kept only here, encrypted; my other devices learn of a change
  // through a sealed note to myself (chat-privacy.js), the server never.
  blocked: {},         // key -> {ts}
  // Notes to myself not sent yet (Block or Unblock while not connected),
  // oldest first, at most one per key: [{action, key}].
  blockNotesPending: [],
  // ── Warnings on messages (step F, 2026-10-10, blocking-and-safe-mode.md
  // 10g): the Safety switch, On unless the person turned it off. Kept here,
  // encrypted, with the block list.
  warningsOn: true,
  // ── Reports about my groups (10j, 2026-10-10, blocking-and-safe-mode.md):
  // reports members of a group I created sent me, each item already checked
  // against my own copy of the group. Kept here, encrypted, until I dismiss
  // them; never sent to any server.
  groupReports: {},    // id -> {id, from, ts, group_id, group_name, target, reason, note, items, removed}

  async _sha256hex(s) {
    const d = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(s));
    return Array.from(new Uint8Array(d)).map((b) => b.toString(16).padStart(2, '0')).join('');
  },

  _openDb() {
    return new Promise((resolve, reject) => {
      const req = indexedDB.open('humanity_dms', 1);
      req.onupgradeneeded = () => {
        const db = req.result;
        if (!db.objectStoreNames.contains('msgs')) {
          const msgs = db.createObjectStore('msgs', { keyPath: 'k' });
          msgs.createIndex('scope', 'scope', { unique: false });
        }
        if (!db.objectStoreNames.contains('meta')) {
          db.createObjectStore('meta', { keyPath: 'scope' });
        }
      };
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => reject(req.error);
    });
  },

  async _encrypt(obj) {
    const iv = crypto.getRandomValues(new Uint8Array(12));
    const ct = new Uint8Array(await crypto.subtle.encrypt(
      { name: 'AES-GCM', iv }, this._key,
      new TextEncoder().encode(JSON.stringify(obj))));
    return { iv: Array.from(iv), ct: Array.from(ct) };
  },

  async _decrypt(box) {
    try {
      const plain = await crypto.subtle.decrypt(
        { name: 'AES-GCM', iv: new Uint8Array(box.iv) }, this._key,
        new Uint8Array(box.ct));
      return JSON.parse(new TextDecoder().decode(plain));
    } catch { return null; } // wrong seed / corrupt record → skip
  },

  _tx(store, mode) {
    return this._db.transaction(store, mode).objectStore(store);
  },

  _idb(req) {
    return new Promise((resolve, reject) => {
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => reject(req.error);
    });
  },

  /** Load (or start empty) the store for this identity on this server. */
  async init(meHex, serverUrl) {
    try {
      const keyPromise = (typeof window.getDmStoreKey === 'function') ? window.getDmStoreKey() : null;
      if (!keyPromise || !meHex) return false;
      this._key = await keyPromise;
      if (!this._key) return false;
      this.me = meHex;
      this.scope = await this._sha256hex(`${meHex}\n${serverUrl || ''}`);
      this._db = this._db || await this._openDb();
      this.conversations = new Map();
      this.lastRead = {};
      this._seen = new Set();
      this.highWater = 0;
      this.following = new Set();
      this.followers = new Set();
      this.passServer = '';
      this.certsFrom = {};
      this.certsSent = {};
      this.withdrawalsPending = [];
      this.contactRequests = {};
      this.blocked = {};
      this.blockNotesPending = [];
      this.warningsOn = true;
      this.groupReports = {};
      // Meta first (high-water + read marks + social sets).
      const meta = await this._idb(this._tx('meta', 'readonly').get(this.scope)).catch(() => null);
      if (meta) {
        this.highWater = Number(meta.hw) || 0;
        if (meta.box) {
          const m = await this._decrypt(meta.box);
          if (m && m.lastRead) this.lastRead = m.lastRead;
          if (m && Array.isArray(m.following)) this.following = new Set(m.following);
          if (m && Array.isArray(m.followers)) this.followers = new Set(m.followers);
          if (m && typeof m.passServer === 'string') this.passServer = m.passServer;
          if (m && m.passesFrom && typeof m.passesFrom === 'object') this.certsFrom = m.passesFrom;
          if (m && m.passesSent && typeof m.passesSent === 'object') this.certsSent = m.passesSent;
          if (m && Array.isArray(m.withdrawalsPending)) this.withdrawalsPending = m.withdrawalsPending;
          if (m && m.contactRequests && typeof m.contactRequests === 'object') this.contactRequests = m.contactRequests;
          if (m && m.blocked && typeof m.blocked === 'object') this.blocked = m.blocked;
          if (m && Array.isArray(m.blockNotesPending)) this.blockNotesPending = m.blockNotesPending;
          if (m && typeof m.warningsOn === 'boolean') this.warningsOn = m.warningsOn;
          if (m && m.groupReports && typeof m.groupReports === 'object') this.groupReports = m.groupReports;
        }
      }
      // All records in this scope.
      const rows = await this._idb(this._tx('msgs', 'readonly').index('scope').getAll(this.scope)).catch(() => []);
      for (const row of rows || []) {
        const m = await this._decrypt(row);
        if (!m || !m.dedupe) continue;
        if (this._seen.has(m.dedupe)) continue;
        this._seen.add(m.dedupe);
        const peer = m.from === this.me ? m.to : m.from;
        if (!this.conversations.has(peer)) this.conversations.set(peer, []);
        this.conversations.get(peer).push(m);
      }
      for (const list of this.conversations.values()) list.sort((a, b) => a.ts - b.ts);
      return true;
    } catch (e) {
      console.warn('hosDmStore.init failed:', e && e.message);
      return false;
    }
  },

  get ready() { return !!(this._db && this._key && this.scope); },

  async _persistMeta() {
    if (!this.ready) return;
    const box = await this._encrypt({
      lastRead: this.lastRead,
      following: Array.from(this.following),
      followers: Array.from(this.followers),
      passServer: this.passServer,
      passesFrom: this.certsFrom,
      passesSent: this.certsSent,
      withdrawalsPending: this.withdrawalsPending,
      contactRequests: this.contactRequests,
      blocked: this.blocked,
      blockNotesPending: this.blockNotesPending,
      warningsOn: this.warningsOn,
      groupReports: this.groupReports,
    });
    await this._idb(this._tx('meta', 'readwrite').put({ scope: this.scope, hw: this.highWater, box })).catch(() => {});
  },

  // ── Social graph API (follows removal, 2026-08-24) ──
  setFollowing(peer, on) {
    if (on) this.following.add(peer); else this.following.delete(peer);
    this._persistMeta();
  },
  setFollower(peer, on) {
    if (on) this.followers.add(peer); else this.followers.delete(peer);
    this._persistMeta();
  },
  isFriendPeer(peer) { return this.following.has(peer) && this.followers.has(peer); },

  // ── Friendship passes v2 (2026-10-09), mirroring native net/dm_store.rs ──
  /**
   * Record the server's did:hum. A different one than before voids every pass
   * held or given here and every withdrawal still waiting (they name the old
   * identity). Returns true when it changed.
   */
  setPassServer(did) {
    if (!did || this.passServer === did) return false;
    if (this.passServer) {
      this.certsFrom = {};
      this.certsSent = {};
      this.withdrawalsPending = [];
    }
    this.passServer = did;
    this._persistMeta();
    return true;
  },
  certFor(peer) { return this.certsFrom[peer] || null; },
  storeCertFrom(peer, cert) { this.certsFrom[peer] = cert; this._persistMeta(); },
  forgetCertFrom(peer) { delete this.certsFrom[peer]; this._persistMeta(); },
  certSentTo(peer) { return Array.isArray(this.certsSent[peer]) && this.certsSent[peer].length > 0; },
  /** Record a pass I gave `peer` (minted here or echoed from my other device), once per serial. */
  recordPassSent(peer, serial, may) {
    const list = this.certsSent[peer] || (this.certsSent[peer] = []);
    if (!list.some((p) => p.serial === serial)) list.push({ serial, may });
    this._persistMeta();
  },
  /** Take back every pass I gave `peer`: their serials wait for the relay to confirm. */
  withdrawPassesTo(peer) {
    const gone = (this.certsSent[peer] || []).map((p) => p.serial);
    delete this.certsSent[peer];
    for (const s of gone) if (!this.withdrawalsPending.includes(s)) this.withdrawalsPending.push(s);
    this._persistMeta();
    return gone;
  },
  /**
   * Replace every pass I gave `peer` with this one (what a friend may do
   * changed, and the new pass is already on its way). The old serials wait for
   * the relay to confirm their withdrawal.
   */
  replacePassTo(peer, serial, may) {
    const old = (this.certsSent[peer] || []).map((p) => p.serial).filter((s) => s !== serial);
    this.certsSent[peer] = [{ serial, may }];
    for (const s of old) if (!this.withdrawalsPending.includes(s)) this.withdrawalsPending.push(s);
    this._persistMeta();
    return old;
  },
  /** What the passes I gave `peer` that still stand let them do (sorted, comma-joined), or null when none stands. */
  passMayTo(peer) {
    const words = new Set();
    for (const p of this.certsSent[peer] || []) for (const w of String(p.may || '').split(',')) if (w) words.add(w);
    return words.size ? Array.from(words).sort().join(',') : null;
  },
  /**
   * The relay confirmed a withdrawal. The serial also leaves the passes I gave,
   * so another of my devices that still listed it (it learns of a withdrawal
   * made elsewhere only from this answer) stops counting it as standing.
   */
  withdrawalConfirmed(serial) {
    let changed = false;
    const before = this.withdrawalsPending.length;
    this.withdrawalsPending = this.withdrawalsPending.filter((s) => s !== serial);
    if (this.withdrawalsPending.length !== before) changed = true;
    for (const peer of Object.keys(this.certsSent)) {
      const list = this.certsSent[peer] || [];
      const kept = list.filter((p) => p.serial !== serial);
      if (kept.length !== list.length) {
        changed = true;
        if (kept.length) this.certsSent[peer] = kept; else delete this.certsSent[peer];
      }
    }
    if (changed) this._persistMeta();
  },

  // ── Contact requests (step B) ──
  /**
   * Add or refresh a request ({key, name, pass?, ts}); returns true when it is
   * new. A pass already held for them is kept when a later one comes without.
   * Never stores text.
   */
  addContactRequest(req) {
    if (!req || !req.key) return false;
    const old = this.contactRequests[req.key];
    this.contactRequests[req.key] = {
      key: req.key,
      name: String(req.name || ''),
      pass: req.pass || (old && old.pass) || null,
      ts: Number(req.ts) || Date.now(),
    };
    this._persistMeta();
    return !old;
  },
  removeContactRequest(id) {
    if (!this.contactRequests[id]) return false;
    delete this.contactRequests[id];
    this._persistMeta();
    return true;
  },
  /** The requests, newest first, each with its id (their key). */
  contactRequestList() {
    return Object.entries(this.contactRequests)
      .map(([id, r]) => ({ id, key: r.key || id, name: r.name, hasPass: !!r.pass, ts: Number(r.ts) || 0 }))
      .sort((a, b) => b.ts - a.ts);
  },
  /** Mutual follows with no standing pass from me, and not blocked: the ones the sweep mints for. */
  friendsWithoutPass() {
    return Array.from(this.following).filter((p) => this.followers.has(p) && !this.certSentTo(p) && !this.isBlocked(p)).sort();
  },

  // ── Blocked people (step C) ──
  _blockKey(key) { return typeof key === 'string' ? key.trim().toLowerCase() : ''; },
  isBlocked(key) {
    const k = this._blockKey(key);
    return !!k && Object.prototype.hasOwnProperty.call(this.blocked, k);
  },
  /**
   * Put `key` on the list (with when) or take it off. Returns true when the
   * list changed: blocking someone already blocked keeps the first date.
   */
  setBlocked(key, on, ts) {
    const k = this._blockKey(key);
    if (!k) return false;
    if (on) {
      if (this.isBlocked(k)) return false;
      this.blocked[k] = { ts: Number(ts) || Date.now() };
    } else {
      if (!this.isBlocked(k)) return false;
      delete this.blocked[k];
    }
    this._persistMeta();
    return true;
  },
  /** Everyone blocked, newest first: [{key, ts}]. */
  blockedList() {
    return Object.keys(this.blocked)
      .map((key) => ({ key, ts: Number(this.blocked[key] && this.blocked[key].ts) || 0 }))
      .sort((a, b) => b.ts - a.ts);
  },
  /** Queue a note to myself; a newer one for the same key replaces the older. */
  queueBlockNote(action, key) {
    const k = this._blockKey(key);
    this.blockNotesPending = this.blockNotesPending.filter((n) => n.key !== k);
    this.blockNotesPending.push({ action, key: k });
    this._persistMeta();
  },
  /** A queued note went out. */
  blockNoteSent(action, key) {
    const before = this.blockNotesPending.length;
    this.blockNotesPending = this.blockNotesPending.filter((n) => !(n.action === action && n.key === key));
    if (this.blockNotesPending.length !== before) this._persistMeta();
  },

  // ── Warnings on messages (step F) ──
  /** Turn the warnings switch on or off. Returns true when it changed. */
  setWarningsOn(on) {
    const v = !!on;
    if (this.warningsOn === v) return false;
    this.warningsOn = v;
    this._persistMeta();
    return true;
  },

  // ── Reports about my groups (10j) ──
  /** Keep a report (a checked record with its own id). Returns true when it is new. */
  addGroupReport(rec) {
    if (!rec || !rec.id || this.groupReports[rec.id]) return false;
    this.groupReports[rec.id] = rec;
    this._persistMeta();
    return true;
  },
  /** Dismiss: the report is gone from this device. */
  removeGroupReport(id) {
    if (!this.groupReports[id]) return false;
    delete this.groupReports[id];
    this._persistMeta();
    return true;
  },
  /** Mark that the reported person was removed from the group. */
  setGroupReportRemoved(id) {
    const r = this.groupReports[id];
    if (!r) return false;
    r.removed = true;
    this._persistMeta();
    return true;
  },
  /** Every kept report, newest first. */
  groupReportList() {
    return Object.values(this.groupReports).filter((r) => r && r.id).sort((a, b) => (Number(b.ts) || 0) - (Number(a.ts) || 0));
  },

  setHighWater(id) {
    const n = Number(id) || 0;
    if (n > this.highWater) {
      this.highWater = n;
      this._persistMeta();
    }
  },

  peerOf(inner) { return inner.from === this.me ? inner.to : inner.from; },

  /** Insert a VERIFIED inner payload. Returns false on duplicate. */
  async insert(inner) {
    if (!this.ready || !inner || !inner.sig) return false;
    const dedupe = await this._sha256hex(inner.sig);
    if (this._seen.has(dedupe)) return false;
    this._seen.add(dedupe);
    // The signature is kept: it is what lets me hand a message to this server's
    // admins as evidence they can check (Report, step D, blocking-and-safe-mode.md 10e).
    const rec = { from: inner.from, to: inner.to, ts: Number(inner.ts) || 0, text: String(inner.text ?? ''), sig: inner.sig, dedupe };
    const peer = this.peerOf(inner);
    if (!this.conversations.has(peer)) this.conversations.set(peer, []);
    const list = this.conversations.get(peer);
    list.push(rec);
    list.sort((a, b) => a.ts - b.ts);
    const box = await this._encrypt(rec);
    await this._idb(this._tx('msgs', 'readwrite').put({
      k: `${this.scope}:${dedupe}`, scope: this.scope, iv: box.iv, ct: box.ct,
    })).catch(() => {});
    return true;
  },

  conversation(peer) { return this.conversations.get(peer) || []; },

  markRead(peer, ts) {
    const cur = Number(this.lastRead[peer]) || 0;
    if (ts > cur) {
      this.lastRead[peer] = ts;
      this._persistMeta();
    }
  },

  hasUnread(peer) {
    const readTs = Number(this.lastRead[peer]) || 0;
    return this.conversation(peer).some((m) => m.from !== this.me && m.ts > readTs);
  },

  /** Sidebar summaries, newest first. */
  summaries() {
    const out = [];
    for (const [peer, msgs] of this.conversations) {
      const last = msgs[msgs.length - 1];
      if (!last) continue;
      out.push({
        peer,
        lastText: last.text,
        lastTs: last.ts,
        lastFromMe: last.from === this.me,
        unread: this.hasUnread(peer),
      });
    }
    out.sort((a, b) => b.lastTs - a.lastTs);
    return out;
  },

  /** Delete one whole conversation locally. */
  async deleteConversation(peer) {
    const msgs = this.conversations.get(peer) || [];
    this.conversations.delete(peer);
    delete this.lastRead[peer];
    for (const m of msgs) {
      this._seen.delete(m.dedupe);
      await this._idb(this._tx('msgs', 'readwrite').delete(`${this.scope}:${m.dedupe}`)).catch(() => {});
    }
    await this._persistMeta();
  },
};

window.hosDmStore = hosDmStore;
