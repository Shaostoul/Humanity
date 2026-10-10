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
// init() — they no-op / return empties. The Trade page loads this file
// without crypto.js: it reads the store read-only (init's `readOnly`) and
// supplies window.getDmStoreKey itself, the same key derived from the same
// seed (/shared/pq-relay-auth.js getPqDmStoreKey).
// ─────────────────────────────────────────────────────────────────────────

// Passes kept per friend whose answer never came (10l, passSending); older
// ones are withdrawn. The desktop app's UNANSWERED_KEPT in src/net/dm_store.rs.
const PASSES_UNSURE_KEPT = 4;
// Refused passes whose self-copy already went (10m R2) remembered, so their
// echo is not adopted here (passesRefusedEchoed).
const PASSES_REFUSED_KEPT = 32;

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
  // ── A pass counts as given only once the server took it (10l, 2026-10-10).
  // A pass is in certsSent only after the relay's `dm_put_ok`. Until then it
  // waits here: sent, perhaps given. The relay may have stored it without the
  // answer reaching this page (30 seconds of silence, a closed socket, a
  // reload), and a pass nobody knows was given could never be withdrawn: after
  // an Unfollow that friend could still reach me with it. So each one is kept
  // until the server answers or it is withdrawn: Unfollow and Block withdraw
  // these too, and a pass the server did take withdraws every other one still
  // waiting for that friend (they hold the taken one, so none is needed).
  passesUnsure: {},    // peer -> [{serial, may}] passes sent that the relay has not said it took
  // What I want each friend's pass to let them do (the ticks on "People I
  // choose", 10c-ii), kept apart from the record so a refused or unanswered
  // re-issue never puts the ticks back: the pass sweep keeps re-sending this
  // `may` until a pass carrying it is taken. Cleared once it is, and by
  // Unfollow and Block (a friendship begun again starts from the defaults).
  passIntent: {},      // peer -> may (sorted, comma-joined)
  // ── Passes across my own devices (10m R2 and R3, 2026-10-10) ──
  // Friends whose pass another of my devices withdrew (the relay's
  // `cert_revoked` for a serial this device held and did not withdraw itself),
  // leaving them none from me (withdrawalConfirmed). That device made a choice
  // this one did not see, so this one gives them no pass by itself until it
  // hears what it was (the echo of a pass to them, adoptEchoedPass) or the
  // person decides here (ticks, follow, accept): nothing withdrawn is ever
  // given back by a device that did not see the choice.
  passChangedElsewhere: {}, // peer -> when it was marked (ms)
  // Serials of re-issued passes whose self-copy went out at once (an untick,
  // 10m R2) and which the server then refused: my own echo of one is not
  // adopted here, where the refusal is known (the friend never held it, and
  // the next sweep sends it again). Newest last, at most PASSES_REFUSED_KEPT.
  passesRefusedEchoed: [],
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

  // True when this page only reads the store (init's `readOnly`): nothing is ever written.
  readOnly: false,

  /**
   * Load (or start empty) the store for this identity on this server.
   *
   * `opts.readOnly` (the Trade page, 2026-10-10): load only the encrypted meta
   * box (the passes, follows and settings), never the message records, and
   * never write. Chat owns the store; a write from another page would put that
   * page's older copy of the meta box over Chat's newer one, and a page that
   * only needs a friend's pass has no business decrypting the messages.
   */
  async init(meHex, serverUrl, opts) {
    try {
      this.readOnly = !!(opts && opts.readOnly);
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
      this.passesUnsure = {};
      this.passIntent = {};
      this.passChangedElsewhere = {};
      this.passesRefusedEchoed = [];
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
          if (m && m.passesUnsure && typeof m.passesUnsure === 'object') this.passesUnsure = m.passesUnsure;
          if (m && m.passIntent && typeof m.passIntent === 'object') this.passIntent = m.passIntent;
          if (m && m.passChangedElsewhere && typeof m.passChangedElsewhere === 'object') this.passChangedElsewhere = m.passChangedElsewhere;
          if (m && Array.isArray(m.passesRefusedEchoed)) this.passesRefusedEchoed = m.passesRefusedEchoed;
          if (m && m.contactRequests && typeof m.contactRequests === 'object') this.contactRequests = m.contactRequests;
          if (m && m.blocked && typeof m.blocked === 'object') this.blocked = m.blocked;
          if (m && Array.isArray(m.blockNotesPending)) this.blockNotesPending = m.blockNotesPending;
          if (m && typeof m.warningsOn === 'boolean') this.warningsOn = m.warningsOn;
          if (m && m.groupReports && typeof m.groupReports === 'object') this.groupReports = m.groupReports;
        }
      }
      if (this.readOnly) return true;
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
    if (!this.ready || this.readOnly) return;
    const box = await this._encrypt({
      lastRead: this.lastRead,
      following: Array.from(this.following),
      followers: Array.from(this.followers),
      passServer: this.passServer,
      passesFrom: this.certsFrom,
      passesSent: this.certsSent,
      withdrawalsPending: this.withdrawalsPending,
      passesUnsure: this.passesUnsure,
      passIntent: this.passIntent,
      passChangedElsewhere: this.passChangedElsewhere,
      passesRefusedEchoed: this.passesRefusedEchoed,
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
      // Passes still waiting for an answer named the old identity too, and
      // what each friend may do starts again from the defaults with them.
      this.passesUnsure = {};
      this.passIntent = {};
      // And what my other devices did with them (10m) is about the old passes.
      this.passChangedElsewhere = {};
      this.passesRefusedEchoed = [];
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
    // A pass to them stands again on record here: this device knows what they hold (10m R3).
    delete this.passChangedElsewhere[peer];
    this._persistMeta();
  },
  /**
   * Take back every pass I gave `peer`: their serials wait for the relay to
   * confirm. That includes the ones sent whose answer has not come (10l): one
   * of them may be standing on the server. The choice of what they may do goes
   * too, so a friendship begun again starts from the defaults.
   */
  withdrawPassesTo(peer) {
    const gone = (this.certsSent[peer] || []).concat(this.passesUnsure[peer] || []).map((p) => p.serial);
    delete this.certsSent[peer];
    delete this.passesUnsure[peer];
    delete this.passIntent[peer];
    delete this.passChangedElsewhere[peer];
    for (const s of gone) if (!this.withdrawalsPending.includes(s)) this.withdrawalsPending.push(s);
    this._persistMeta();
    return gone;
  },
  /**
   * Replace every pass I gave `peer` with this one (what a friend may do
   * changed, and the server took the new pass: passTaken). The old serials
   * wait for the relay to confirm their withdrawal.
   */
  replacePassTo(peer, serial, may) {
    const old = (this.certsSent[peer] || []).map((p) => p.serial).filter((s) => s !== serial);
    this.certsSent[peer] = [{ serial, may }];
    delete this.passChangedElsewhere[peer];
    for (const s of old) if (!this.withdrawalsPending.includes(s)) this.withdrawalsPending.push(s);
    this._persistMeta();
    return old;
  },
  /**
   * A pass I gave `peer`, echoed from another of my devices: it is that
   * device's latest word on what they may do, so it is recorded and every
   * other pass to them whose `may` differs is withdrawn (their serials wait
   * for the relay to confirm), the desktop app's rule (src/net/dm_store.rs
   * withdraw_passes_to_except). One with the same `may` stands beside it:
   * withdrawing it would take nothing away. Returns the serials withdrawn.
   * A pass this device has already taken back (its withdrawal still waiting
   * for the relay) is never standing again: that is this device's own pass
   * echoing back after a newer one replaced it, and adopting it would
   * withdraw the newer pass the friend now holds.
   */
  adoptEchoedPass(peer, serial, may) {
    if (this.withdrawalsPending.includes(serial)) return [];
    // Already on record here: this device's own pass coming back (its
    // self-copy goes out once the server took it, 10l), or one adopted
    // before. It says nothing new, and acting on it again would withdraw a
    // newer pass this device has sent since.
    if ((this.certsSent[peer] || []).some((p) => p.serial === serial)) return [];
    // This device's own pass, whose self-copy went out with it (an untick,
    // 10m R2): still waiting for the server's answer, or refused by it. The
    // answer decides here, never the echo: adopting a refused one would count
    // a pass the friend does not hold, and the sweep would stop sending it.
    if ((this.passesUnsure[peer] || []).some((p) => p.serial === serial)) return [];
    if (this.passesRefusedEchoed.includes(serial)) return [];
    const norm = (m) => String(m || '').split(',').filter(Boolean).sort().join(',');
    const want = norm(may);
    const list = this.certsSent[peer] || [];
    // A pass this device sent whose answer has not come is withdrawn by the
    // same rule when it says otherwise (10l): the echo is the newer word, and
    // if the server took this device's pass too it would outlive the choice.
    const unsure = (this.passesUnsure[peer] || []).filter((p) => p.serial !== serial);
    const gone = list.concat(unsure).filter((p) => p.serial !== serial && norm(p.may) !== want).map((p) => p.serial);
    const kept = list.filter((p) => p.serial !== serial && norm(p.may) === want);
    this.certsSent[peer] = kept.concat([{ serial, may }]);
    const unsureKept = unsure.filter((p) => norm(p.may) === want);
    if (unsureKept.length) this.passesUnsure[peer] = unsureKept; else delete this.passesUnsure[peer];
    // What this device meant to give them gives way to it as well, and a
    // friend marked as changed on my other device is not any more (10m R3):
    // this is that device's word.
    delete this.passIntent[peer];
    delete this.passChangedElsewhere[peer];
    for (const s of gone) if (!this.withdrawalsPending.includes(s)) this.withdrawalsPending.push(s);
    this._persistMeta();
    return gone;
  },
  // ── Passes on their way (10l) ──
  /**
   * A pass put is going out to `peer`: perhaps given from now until the server
   * answers. At most PASSES_UNSURE_KEPT wait per friend: beyond that the
   * oldest are withdrawn (the desktop app's UNANSWERED_KEPT), so a server that
   * never answers cannot grow the list without end. Returns the serials
   * withdrawn, for the caller to send.
   */
  passSending(peer, serial, may) {
    const list = this.passesUnsure[peer] || (this.passesUnsure[peer] = []);
    if (!list.some((p) => p.serial === serial)) list.push({ serial, may });
    const over = list.splice(0, Math.max(0, list.length - PASSES_UNSURE_KEPT)).map((p) => p.serial);
    for (const s of over) if (!this.withdrawalsPending.includes(s)) this.withdrawalsPending.push(s);
    this._persistMeta();
    return over;
  },
  /**
   * The friends on the "People I choose" list: a pass from me standing, one
   * on its way or never answered, or a choice of what it lets them do still
   * waiting for a pass the server takes (10l). Not only the standing passes:
   * a tick taken away withdraws those at once (withdrawPassesWhere), and the
   * friend stays on the list while the new pass is on its way. Not anyone I
   * blocked.
   */
  passFriends() {
    const keys = new Set();
    for (const map of [this.certsSent, this.passesUnsure]) {
      for (const p of Object.keys(map)) if (Array.isArray(map[p]) && map[p].length) keys.add(p);
    }
    for (const p of Object.keys(this.passIntent)) keys.add(p);
    // And a friend whose pass my other device just changed (10m R3): still mine
    // to choose for, while they are a friend here (an Unfollow made there
    // clears the mark when its note arrives, withdrawPassesTo).
    for (const p of Object.keys(this.passChangedElsewhere)) if (this.isFriendPeer(p)) keys.add(p);
    return Array.from(keys).filter((p) => !this.isBlocked(p));
  },
  /** Did another of my devices withdraw `peer`'s pass, leaving them none from me (10m R3)? */
  passChangedOnOtherDevice(peer) {
    return Object.prototype.hasOwnProperty.call(this.passChangedElsewhere, peer);
  },
  /**
   * The person decided for `peer` on this device (ticks, follow, accept), or
   * this device heard my other device's choice: the mark goes. Returns true
   * when there was one.
   */
  clearPassChangedElsewhere(peer) {
    if (!this.passChangedOnOtherDevice(peer)) return false;
    delete this.passChangedElsewhere[peer];
    this._persistMeta();
    return true;
  },
  /**
   * Withdraw at once every pass to `peer`, standing or still unanswered, for
   * which `pred({serial, may})` is true: a tick taken away takes effect now,
   * whether or not the re-issued pass goes out (10l; the desktop app's
   * src/engine/dm.rs reissue_pass). Their serials wait for the relay to
   * confirm. Returns the serials withdrawn.
   */
  withdrawPassesWhere(peer, pred) {
    const gone = [];
    for (const map of [this.certsSent, this.passesUnsure]) {
      const list = map[peer] || [];
      const kept = list.filter((p) => !pred(p));
      for (const p of list) if (pred(p)) gone.push(p.serial);
      if (kept.length) map[peer] = kept; else delete map[peer];
    }
    for (const s of gone) if (!this.withdrawalsPending.includes(s)) this.withdrawalsPending.push(s);
    if (gone.length) this._persistMeta();
    return gone;
  },
  _dropUnsure(peer, serial) {
    const list = this.passesUnsure[peer] || [];
    const kept = list.filter((p) => p.serial !== serial);
    if (kept.length === list.length) return false;
    if (kept.length) this.passesUnsure[peer] = kept; else delete this.passesUnsure[peer];
    return true;
  },
  /**
   * The server refused the put (`dm_put_refused`): the pass was never given,
   * so nothing about it is kept and nothing is withdrawn. Their pass, if one
   * stands, stands; what I meant to give them stays, for the next sweep.
   */
  passRefused(peer, serial, selfCopySent) {
    let changed = this._dropUnsure(peer, serial);
    // Its self-copy already went to my mailbox (10m R2): its echo, when it
    // comes back here, is not a pass the friend holds (adoptEchoedPass).
    if (selfCopySent && !this.passesRefusedEchoed.includes(serial)) {
      this.passesRefusedEchoed.push(serial);
      this.passesRefusedEchoed.splice(0, Math.max(0, this.passesRefusedEchoed.length - PASSES_REFUSED_KEPT));
      changed = true;
    }
    if (changed) this._persistMeta();
  },
  /**
   * The server took the put (`dm_put_ok`): from now the pass is given. A
   * re-issue (`replace`) replaces every pass to them, whose serials are
   * withdrawn; a first pass is added. Every other pass still waiting for them
   * is withdrawn too: they hold this one, so none of those is needed. Returns
   * false, recording nothing, for a pass no longer waiting: withdrawn
   * meanwhile (Unfollow, Block, a newer word from another device) or voided by
   * a new server identity.
   */
  passTaken(peer, serial, may, replace) {
    if (!this._dropUnsure(peer, serial)) return false;
    const others = (this.passesUnsure[peer] || []).map((p) => p.serial);
    delete this.passesUnsure[peer];
    for (const s of others) if (!this.withdrawalsPending.includes(s)) this.withdrawalsPending.push(s);
    const norm = (m) => String(m || '').split(',').filter(Boolean).sort().join(',');
    if (this.passIntent[peer] !== undefined && norm(this.passIntent[peer]) === norm(may)) delete this.passIntent[peer];
    if (replace) this.replacePassTo(peer, serial, may); else this.recordPassSent(peer, serial, may);
    return true;
  },
  /**
   * What the passes `peer` may hold from me let them do: the ones that stand
   * and the ones whose answer has not come, either of which the relay honours
   * if it stored it (sorted, comma-joined), or null for none. For deciding
   * whether a message from them got through as the relay would: a friend whose
   * pass went unanswered must not have their words dropped as a stranger's.
   */
  passMayHeld(peer) {
    const words = new Set();
    for (const p of (this.certsSent[peer] || []).concat(this.passesUnsure[peer] || [])) {
      for (const w of String(p.may || '').split(',')) if (w) words.add(w);
    }
    return words.size ? Array.from(words).sort().join(',') : null;
  },
  /** Keep what I want `peer`'s pass to let them do (sorted, comma-joined), until a pass carrying it is taken. */
  setPassIntent(peer, may) {
    const words = String(may || '').split(',').filter(Boolean).sort().join(',');
    if (!words) return;
    this.passIntent[peer] = words;
    this._persistMeta();
  },
  /** What `peer` should be able to do: what I last chose for them, else what the passes I gave them let them do; null for neither. */
  passMayIntended(peer) {
    const intent = this.passIntent[peer];
    return (typeof intent === 'string' && intent) ? intent : this.passMayTo(peer);
  },
  /**
   * Friends holding a pass that does not carry what I chose for them (a
   * re-issue refused or unanswered): [{peer, may}], for the sweep to send
   * again. Not anyone I blocked.
   */
  passReissuesOwed() {
    const norm = (m) => String(m || '').split(',').filter(Boolean).sort().join(',');
    return Object.keys(this.passIntent)
      .filter((p) => this.certSentTo(p) && !this.isBlocked(p) && norm(this.passIntent[p]) !== norm(this.passMayTo(p)))
      .sort()
      .map((p) => ({ peer: p, may: this.passIntent[p] }));
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
   *
   * A serial this device did not withdraw itself was withdrawn by another of
   * my devices (10m R3). When that leaves the friend with no pass from me, on
   * record or on its way, they are marked "changed on my other device"
   * (passChangedElsewhere): that device made a choice this one did not see,
   * so this one gives them nothing by itself, and what it meant to give them
   * before (passIntent) is dropped as older than that choice. Returns the
   * friends marked now.
   */
  withdrawalConfirmed(serial) {
    let changed = false;
    const mine = this.withdrawalsPending.includes(serial);
    const before = this.withdrawalsPending.length;
    this.withdrawalsPending = this.withdrawalsPending.filter((s) => s !== serial);
    if (this.withdrawalsPending.length !== before) changed = true;
    const touched = [];
    for (const peer of Object.keys(this.certsSent)) {
      const list = this.certsSent[peer] || [];
      const kept = list.filter((p) => p.serial !== serial);
      if (kept.length !== list.length) {
        changed = true;
        touched.push(peer);
        if (kept.length) this.certsSent[peer] = kept; else delete this.certsSent[peer];
      }
    }
    // A withdrawn pass is not waiting for anything any more (10l).
    for (const peer of Object.keys(this.passesUnsure)) {
      if (this._dropUnsure(peer, serial)) { changed = true; touched.push(peer); }
    }
    const marked = [];
    if (!mine) {
      for (const peer of touched) {
        if (this.certSentTo(peer) || (this.passesUnsure[peer] || []).length) continue;
        if (this.isBlocked(peer) || this.passChangedOnOtherDevice(peer)) continue;
        this.passChangedElsewhere[peer] = Date.now();
        delete this.passIntent[peer];
        marked.push(peer);
      }
    }
    if (changed) this._persistMeta();
    return marked;
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
  /**
   * Mutual follows with no standing pass from me, and not blocked: the ones the
   * sweep mints for (it skips one whose pass my other device withdrew, 10m R3,
   * chat-social.js sweepFriendPasses; they are still friends, so the protected
   * setup's review lists them from here).
   */
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
    if (!this.ready || this.readOnly || !inner || !inner.sig) return false;
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
    if (this.readOnly) return;
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
