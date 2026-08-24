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
  conversations: new Map(), // peer -> [{from,to,ts,text,dedupe}] sorted by ts
  lastRead: {},        // peer -> ts
  _seen: new Set(),    // dedupe tags in memory
  // ── Client-side social graph (follows removal, 2026-08-24): the server
  // stores no edges; these sets ARE the user's social state, persisted in
  // the encrypted meta box and built from sealed control messages.
  following: new Set(),
  followers: new Set(),
  certsFrom: {},       // peer -> cert THEY issued authorizing ME to DM them
  certsSent: new Set(),// peers I've issued MY cert to

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
      this.certsFrom = {};
      this.certsSent = new Set();
      // Meta first (high-water + read marks + social sets).
      const meta = await this._idb(this._tx('meta', 'readonly').get(this.scope)).catch(() => null);
      if (meta) {
        this.highWater = Number(meta.hw) || 0;
        if (meta.box) {
          const m = await this._decrypt(meta.box);
          if (m && m.lastRead) this.lastRead = m.lastRead;
          if (m && Array.isArray(m.following)) this.following = new Set(m.following);
          if (m && Array.isArray(m.followers)) this.followers = new Set(m.followers);
          if (m && m.certsFrom) this.certsFrom = m.certsFrom;
          if (m && Array.isArray(m.certsSent)) this.certsSent = new Set(m.certsSent);
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
      certsFrom: this.certsFrom,
      certsSent: Array.from(this.certsSent),
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
  certFor(peer) { return this.certsFrom[peer] || null; },
  storeCertFrom(peer, cert) { this.certsFrom[peer] = cert; this._persistMeta(); },
  certSentTo(peer) { return this.certsSent.has(peer); },
  markCertSent(peer) { this.certsSent.add(peer); this._persistMeta(); },

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
    const rec = { from: inner.from, to: inner.to, ts: Number(inner.ts) || 0, text: String(inner.text ?? ''), dedupe };
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
