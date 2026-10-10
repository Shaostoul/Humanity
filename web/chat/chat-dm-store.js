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
// Choice notes (10n) remembered by signature hash, so each is applied once.
// They come from the person's own clicks, so this is years of them (the
// desktop app keeps as many block notes, src/net/block_list.rs).
const SELF_NOTES_REMEMBERED = 4096;
// The scratch pad's notes kept (10n N8; the desktop app's SCRATCHPAD_KEPT).
const SCRATCH_KEPT = 500;

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
  // ── The choice for each friend (10n, 2026-10-10, blocking-and-safe-mode.md) ──
  // What I chose each friend's pass to let them do (the ticks on "People I
  // choose", 10c-ii), with when it was made (ms). The same on every device of
  // mine: a choice made here goes to my other devices as a sealed note to
  // myself (chat-social.js), one made there arrives the same way, and the
  // newest wins everywhere (friend-pass.js choiceWins). The passes follow it:
  // a pass granting beyond it is withdrawn, and the sweep gives a friend who
  // holds none carrying it a new one. It stays when a pass carrying it is
  // taken (until 10n it was cleared then, and rebuilt from echoes of passes,
  // which arrive late, out of order, or never). Unfollow and Block clear it: an
  // empty `may` with the time they were made, kept so an older note cannot
  // bring it back, and a friendship begun again starts from the defaults. No
  // entry means the defaults.
  passChoice: {},      // peer -> {may, at}
  // Choice notes not sent yet (made while not connected, or before the
  // identity could seal one), at most one per friend, a newer replacing an
  // older: [{peer, may, at}]. They go first on the next connection, before
  // that friend's withdrawals and passes.
  choiceNotesPending: [],
  // Unfollows not sent yet (made while not connected): both copies, to them
  // and to my own mailbox for my other devices, wait: [{peer, at}].
  unfollowsPending: [],
  // Signature hashes of the choice notes applied or sent here, newest last, at
  // most SELF_NOTES_REMEMBERED: each note is applied once.
  selfNotesSeen: [],
  // ── Passes across my own devices (10m R3, 2026-10-10) ──
  // Friends whose pass another of my devices withdrew (the relay's
  // `cert_revoked` for a serial this device held and did not withdraw itself),
  // leaving them none from me (withdrawalConfirmed). That device made a choice
  // this one may not have seen yet, so this one gives them no pass by itself
  // until it hears what it was (its choice note, or the echo of a pass to
  // them) or the person decides here (ticks, follow, accept): nothing
  // withdrawn is ever given back by a device that did not see the choice.
  passChangedElsewhere: {}, // peer -> when it was marked (ms)
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
  // oldest first, at most one per key: [{action, key, at}], `at` when it was
  // made, which the note is signed with (so every device dates it the same,
  // and clears the choice for them as of that time, 10n).
  blockNotesPending: [],
  // ── The scratch pad (10n N8, 2026-10-10): its notes, oldest first, at most
  // SCRATCH_KEPT, each {content, timestamp, replyTo?} (every note is mine, so
  // no key is kept on each: a key is 3,904 characters). Kept here, encrypted,
  // for this identity on this server, in their own record beside the meta box
  // (so the box written on every pass change does not carry them). Until 10n
  // they sat in localStorage `hos_scratch_msgs`, in the clear, file keys and
  // all, shared by every identity in the browser.
  scratch: [],
  _scratchSaving: null, // the save in progress (one at a time; changes meanwhile go in the next)
  _scratchDirty: false,
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
      this.passChoice = {};
      this.choiceNotesPending = [];
      this.unfollowsPending = [];
      this.selfNotesSeen = [];
      this.passChangedElsewhere = {};
      this.contactRequests = {};
      this.blocked = {};
      this.blockNotesPending = [];
      this.warningsOn = true;
      this.groupReports = {};
      this.scratch = [];
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
          if (m && m.passChoice && typeof m.passChoice === 'object') this.passChoice = m.passChoice;
          if (m && Array.isArray(m.choiceNotesPending)) this.choiceNotesPending = m.choiceNotesPending;
          if (m && Array.isArray(m.unfollowsPending)) this.unfollowsPending = m.unfollowsPending;
          if (m && Array.isArray(m.selfNotesSeen)) this.selfNotesSeen = m.selfNotesSeen;
          if (m && m.passChangedElsewhere && typeof m.passChangedElsewhere === 'object') this.passChangedElsewhere = m.passChangedElsewhere;
          if (m && m.contactRequests && typeof m.contactRequests === 'object') this.contactRequests = m.contactRequests;
          if (m && m.blocked && typeof m.blocked === 'object') this.blocked = m.blocked;
          if (m && Array.isArray(m.blockNotesPending)) this.blockNotesPending = m.blockNotesPending;
          if (m && typeof m.warningsOn === 'boolean') this.warningsOn = m.warningsOn;
          if (m && m.groupReports && typeof m.groupReports === 'object') this.groupReports = m.groupReports;
        }
      }
      if (this.readOnly) return true;
      // The scratch pad's notes (10n N8), in their own encrypted record.
      const pad = await this._idb(this._tx('meta', 'readonly').get(this._scratchScope())).catch(() => null);
      if (pad && pad.box) {
        const notes = await this._decrypt(pad.box);
        if (Array.isArray(notes)) this.scratch = notes.slice(-SCRATCH_KEPT);
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
      passChoice: this.passChoice,
      choiceNotesPending: this.choiceNotesPending,
      unfollowsPending: this.unfollowsPending,
      selfNotesSeen: this.selfNotesSeen,
      passChangedElsewhere: this.passChangedElsewhere,
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
      // Passes still waiting for an answer named the old identity too.
      this.passesUnsure = {};
      // And what my other devices did with them (10m R3) is about the old
      // passes. What I chose for each friend (10n) is not: it is mine, not the
      // server's, my other devices keep theirs, and the sweep gives each
      // friend a new pass carrying it.
      this.passChangedElsewhere = {};
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
   * of them may be standing on the server. The choice of what they may do is
   * cleared too (10n N3, clearChoice, as of `at`: the Unfollow's or the
   * Block's own time when it has one; `force` for a Block), so a friendship
   * begun again starts from the defaults.
   */
  withdrawPassesTo(peer, at, force) {
    const gone = (this.certsSent[peer] || []).concat(this.passesUnsure[peer] || []).map((p) => p.serial);
    delete this.certsSent[peer];
    delete this.passesUnsure[peer];
    this.clearChoice(peer, at, force);
    delete this.passChangedElsewhere[peer];
    for (const s of gone) if (!this.withdrawalsPending.includes(s)) this.withdrawalsPending.push(s);
    this._persistMeta();
    return gone;
  },
  /**
   * Replace every pass I gave `peer` with this one (the server took a pass
   * carrying my choice for them: passTaken). The old serials wait for the
   * relay to confirm their withdrawal.
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
   * A pass I gave `peer`, echoed from another of my devices (its self-copy,
   * sent once the server took it, 10l). Since 10n the choice travels in its
   * own note, so an echo is only a pass: it is recorded as standing, unless it
   * grants beyond my choice for them (then it is withdrawn at once: the choice
   * is newer than that pass) or this device is already withdrawing it (10m R5:
   * this device's own pass echoing back after a newer one replaced it). An
   * echo never changes the choice and never withdraws another pass. It does
   * clear a mark "changed on my other device" (10m R3): a pass of mine stands
   * with them again. Returns the serials withdrawn (this one, or none).
   */
  adoptEchoedPass(peer, serial, may) {
    if (this.withdrawalsPending.includes(serial)) return [];
    // Already on record here: this device's own pass coming back, or one
    // recorded before. It says nothing new.
    if ((this.certsSent[peer] || []).some((p) => p.serial === serial)) return [];
    // This device's own pass still waiting for the server's answer: the answer
    // decides here, never the echo.
    if ((this.passesUnsure[peer] || []).some((p) => p.serial === serial)) return [];
    if (this.grantsBeyondChoice(peer, may)) {
      this.withdrawalsPending.push(serial);
      this._persistMeta();
      return [serial];
    }
    this.recordPassSent(peer, serial, may);
    return [];
  },
  /**
   * Would a pass allowing `may` let `peer` through for something my choice for
   * them does not (a kind the relay checks: message, call, trade)? The rule is
   * /shared/reach.js reachGrantsBeyond, the desktop app's `grants_beyond`.
   */
  grantsBeyondChoice(peer, may) {
    const want = this.choiceMay(peer);
    if (typeof reachGrantsBeyond === 'function') return reachGrantsBeyond(may, want);
    // (Without reach.js, any word the choice lacks counts: never less safe.)
    const have = String(want || '').split(',');
    return String(may || '').split(',').some((w) => w && !have.includes(w));
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
   * on its way or never answered, or a choice of mine for someone I follow
   * (10n: the choice stays, so a friend whose pass carrying it was refused, or
   * whose pass a tick taken away withdrew, stays on the list). Not anyone I
   * blocked.
   */
  passFriends() {
    const keys = new Set();
    for (const map of [this.certsSent, this.passesUnsure]) {
      for (const p of Object.keys(map)) if (Array.isArray(map[p]) && map[p].length) keys.add(p);
    }
    for (const p of Object.keys(this.passChoice)) {
      // (A marked friend only while still a mutual follow, below: 10n N6.)
      if (this.choiceOf(p) && this.following.has(p) && !this.passChangedOnOtherDevice(p)) keys.add(p);
    }
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
   * stands, stands; my choice for them stays, for the next sweep. (Its
   * self-copy never went: every pass's self-copy waits for `dm_put_ok`, 10n N5.)
   */
  passRefused(peer, serial) {
    if (this._dropUnsure(peer, serial)) this._persistMeta();
  },
  /**
   * The server took the put (`dm_put_ok`): from now the pass is given. One
   * carrying my choice for them replaces every pass to them, whose serials are
   * withdrawn (10l: only now are the passes it replaces withdrawn); one that
   * does not (the choice changed while it was on its way, adding something) is
   * recorded beside them, and the sweep sends one carrying the choice. Every
   * other pass still waiting for them is withdrawn too: they hold this one, so
   * none of those is needed. Returns false, recording nothing, for a pass no
   * longer waiting: withdrawn meanwhile (Unfollow, Block, a choice that took
   * away what it allows) or voided by a new server identity.
   */
  passTaken(peer, serial, may) {
    if (!this._dropUnsure(peer, serial)) return false;
    const others = (this.passesUnsure[peer] || []).map((p) => p.serial);
    delete this.passesUnsure[peer];
    for (const s of others) if (!this.withdrawalsPending.includes(s)) this.withdrawalsPending.push(s);
    if (this._canonMay(may) === this.choiceMay(peer)) this.replacePassTo(peer, serial, may);
    else this.recordPassSent(peer, serial, may);
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
  // ── The choice for each friend (10n) ──
  /** `may` (words, or comma-joined text) in the pass's canonical form, or null when it is not a pass's. */
  _canonMay(may) {
    const words = Array.isArray(may) ? may : String(may || '').split(',').filter(Boolean);
    if (typeof friendPassMay === 'function') return friendPassMay(words);
    return words.length ? Array.from(new Set(words)).sort().join(',') : null;
  },
  /** Does a choice allowing `may`, made at `at`, replace the one kept (friend-pass.js choiceWins)? */
  _choiceWins(may, at, kept) {
    return typeof choiceWins === 'function' && choiceWins(may, at, kept || null);
  },
  /** The choice kept for `peer`, {may, at}; null for none (or one Unfollow or Block cleared). */
  choiceOf(peer) {
    const c = this.passChoice[peer];
    return (c && typeof c.may === 'string' && c.may) ? { may: c.may, at: Number(c.at) || 0 } : null;
  },
  /** What `peer`'s pass should let them do: my choice, else the defaults for two new friends (canonical). */
  choiceMay(peer) {
    const c = this.choiceOf(peer);
    if (c) return c.may;
    return this._canonMay(typeof FRIEND_PASS_DEFAULT_MAY !== 'undefined' ? FRIEND_PASS_DEFAULT_MAY : ['invite', 'message', 'trade', 'voice_message']);
  },
  /**
   * The time for a change about `peer` made on this device now: never earlier
   * than the one kept, so it wins here (a tick and an Unfollow within the same
   * millisecond still land in the order they were made).
   */
  choiceTimeNow(peer) {
    const kept = this.passChoice[peer];
    return Math.max(Date.now(), kept ? (Number(kept.at) || 0) + 1 : 0);
  },
  /**
   * The person chose here (N1): `may` for `peer`, made at `at` (default now).
   * Kept, and queued as a note for my other devices (replacing an older one
   * still waiting). A mark "changed on my other device" goes: this is the
   * person's own word. Returns the canonical `may`, or null for words that
   * are not a pass's.
   */
  makeChoice(peer, may, at) {
    const canonical = this._canonMay(may);
    if (!peer || !canonical) return null;
    const when = Number(at) || this.choiceTimeNow(peer);
    this.passChoice[peer] = { may: canonical, at: when };
    this.choiceNotesPending = this.choiceNotesPending.filter((n) => n.peer !== peer);
    this.choiceNotesPending.push({ peer, may: canonical, at: when });
    delete this.passChangedElsewhere[peer];
    this._persistMeta();
    return canonical;
  },
  /**
   * A choice note from my other device (N2), already checked to be mine and
   * seen here for the first time: it replaces the choice kept only when it
   * wins (newer; at an equal time the larger `may` text). Win or lose, it is
   * that device's word, so a mark "changed on my other device" goes (N6). A
   * note still waiting here that it beats is dropped (it would lose on every
   * device). Returns true when what they may do changed.
   */
  applyChoiceNote(peer, may, at) {
    const canonical = this._canonMay(may);
    if (!peer || !canonical) return false;
    const kept = this.passChoice[peer] || null;
    let changed = false;
    if (this._choiceWins(canonical, at, kept)) {
      changed = this.choiceMay(peer) !== canonical;
      this.passChoice[peer] = { may: canonical, at: Number(at) || 0 };
      this.choiceNotesPending = this.choiceNotesPending.filter((n) => n.peer !== peer);
    }
    delete this.passChangedElsewhere[peer];
    this._persistMeta();
    return changed;
  },
  /**
   * Unfollow or Block (N3): the choice for `peer` is cleared as of `at` (their
   * own time, the same on every device: the unfollow's or the block note's
   * signed time), kept as an empty one so an older note cannot bring it back.
   * Without `at`, a time that wins here now, and nothing when there is no
   * choice to clear. An Unfollow clears it only when it is the newer word, as
   * any choice note would (so a tick made on my other device after it, before
   * that device heard of it, ends the same on every device); a Block (`force`)
   * clears it whatever its time, because a choice note about someone I blocked
   * is ignored (N2), so every device ends cleared. A choice note still waiting
   * for them goes. Returns true when it cleared one.
   */
  clearChoice(peer, at, force) {
    if (!peer) return false;
    const kept = this.passChoice[peer] || null;
    let when = Number(at) || 0;
    if (!when) {
      if (!this.choiceOf(peer)) return false;
      when = this.choiceTimeNow(peer);
    }
    if (force) {
      when = Math.max(when, kept ? Number(kept.at) || 0 : 0);
      if (kept && kept.may === '' && (Number(kept.at) || 0) === when) return false;
    } else if (!this._choiceWins('', when, kept)) {
      return false;
    }
    this.passChoice[peer] = { may: '', at: when };
    this.choiceNotesPending = this.choiceNotesPending.filter((n) => n.peer !== peer);
    this._persistMeta();
    return true;
  },
  /** A queued choice note went out (one made since stays queued). */
  choiceNoteSent(peer, at) {
    const before = this.choiceNotesPending.length;
    this.choiceNotesPending = this.choiceNotesPending.filter((n) => !(n.peer === peer && n.at === at));
    if (this.choiceNotesPending.length !== before) this._persistMeta();
  },
  /**
   * A choice note's signature (one sent here, or one received): true the
   * first time, and it is remembered (by hash, the newest
   * SELF_NOTES_REMEMBERED), so each note is applied once.
   */
  async selfNoteFirstSight(sig) {
    if (typeof sig !== 'string' || !sig) return false;
    // 128 bits of the hash: plenty to tell my notes apart, and half the room
    // in the meta box, which is written on every change.
    const h = (await this._sha256hex(sig)).slice(0, 32);
    if (this.selfNotesSeen.includes(h)) return false;
    this.selfNotesSeen.push(h);
    this.selfNotesSeen.splice(0, Math.max(0, this.selfNotesSeen.length - SELF_NOTES_REMEMBERED));
    this._persistMeta();
    return true;
  },
  /**
   * Does `peer` hold no standing pass from me carrying my choice for them (so
   * the sweep owes them one, N4)?
   */
  passOwed(peer) {
    const want = this.choiceMay(peer);
    return !(this.certsSent[peer] || []).some((p) => this._canonMay(p.may) === want);
  },
  /**
   * The people the sweep owes a pass carrying my choice (N4): every mutual
   * follow, and anyone who holds a pass of mine (standing, or sent and never
   * answered), so it keeps up with my choice for them; each holding no
   * standing pass carrying it, not blocked. (Not one marked "changed on my
   * other device", nor one with a pass on its way: the sweep checks those.)
   */
  passesOwed() {
    const keys = new Set();
    for (const p of this.following) if (this.followers.has(p)) keys.add(p);
    for (const map of [this.certsSent, this.passesUnsure]) {
      for (const p of Object.keys(map)) if (Array.isArray(map[p]) && map[p].length) keys.add(p);
    }
    return Array.from(keys)
      .filter((p) => !this.isBlocked(p) && this.passOwed(p))
      .sort();
  },
  // ── Unfollows made while not connected (N3) ──
  /** Queue an unfollow of `peer` made at `at` (one per person). */
  queueUnfollow(peer, at) {
    this.unfollowsPending = this.unfollowsPending.filter((u) => u.peer !== peer);
    this.unfollowsPending.push({ peer, at: Number(at) || Date.now() });
    this._persistMeta();
  },
  /** A queued unfollow went (both copies), or a follow made since replaced it. Returns true when one was queued. */
  dropQueuedUnfollow(peer) {
    const before = this.unfollowsPending.length;
    this.unfollowsPending = this.unfollowsPending.filter((u) => u.peer !== peer);
    if (this.unfollowsPending.length === before) return false;
    this._persistMeta();
    return true;
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
   * (passChangedElsewhere): that device may have made a choice this one has
   * not heard yet, so this one gives them nothing by itself until its note
   * (or an echo of its pass) arrives or the person decides here. Returns the
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
        if (this.certSentTo(peer)) continue; // another pass of mine still stands with them
        if (this.isBlocked(peer) || this.passChangedOnOtherDevice(peer)) continue;
        // A pass of mine still on its way to them was minted without that
        // device's choice: it is withdrawn too, so it cannot give back what
        // the other device took away (the desktop app's withdrawal_confirmed
        // does the same). The caller sends the withdrawals.
        for (const p of (this.passesUnsure[peer] || [])) {
          if (!this.withdrawalsPending.includes(p.serial)) this.withdrawalsPending.push(p.serial);
        }
        delete this.passesUnsure[peer];
        changed = true;
        this.passChangedElsewhere[peer] = Date.now();
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
  /** Queue a note to myself made at `at` (default now); a newer one for the same key replaces the older. */
  queueBlockNote(action, key, at) {
    const k = this._blockKey(key);
    this.blockNotesPending = this.blockNotesPending.filter((n) => n.key !== k);
    this.blockNotesPending.push({ action, key: k, at: Number(at) || Date.now() });
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

  // ── The scratch pad (10n N8) ──
  /** The scratch pad's own record, beside the meta box: this identity on this server. */
  _scratchScope() { return `${this.scope}:scratch`; },
  /**
   * Write the scratch pad's record. One write at a time: notes kept while one
   * is being written go in one more write after it, never one write each (a
   * burst of notes would otherwise encrypt the whole pad once per note, all
   * at once). Each write takes the notes and the scope as they are when it
   * starts.
   */
  _persistScratch() {
    if (!this.ready || this.readOnly) return Promise.resolve();
    if (this._scratchSaving) {
      this._scratchDirty = true;
      return this._scratchSaving;
    }
    const run = async () => {
      try {
        do {
          this._scratchDirty = false;
          if (!this.ready || this.readOnly) break;
          const scope = this._scratchScope();
          const box = await this._encrypt(this.scratch.slice());
          await this._idb(this._tx('meta', 'readwrite').put({ scope, box })).catch(() => {});
        } while (this._scratchDirty);
      } finally {
        this._scratchSaving = null;
      }
    };
    this._scratchSaving = run();
    return this._scratchSaving;
  },
  /** The scratch pad's notes, oldest first (a copy). */
  scratchNotes() { return this.scratch.slice(); },
  /** Keep a note ({content, timestamp, replyTo?}); past SCRATCH_KEPT the oldest go. */
  addScratchNote(note) {
    if (!this.ready || this.readOnly || !note) return false;
    this.scratch.push(note);
    this.scratch.splice(0, Math.max(0, this.scratch.length - SCRATCH_KEPT));
    this._persistScratch();
    return true;
  },
  /** The scratch pad's /clear. */
  clearScratch() {
    if (!this.ready || this.readOnly) return false;
    this.scratch = [];
    this._persistScratch();
    return true;
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
