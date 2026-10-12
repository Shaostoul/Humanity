// ── file-transfer.js ──────────────────────────────────────────────────────
// Device-to-device big files (2026-10-11, docs/design/media-showcase.md,
// "Device-to-device big files, exact" and "Device-to-device big files: the
// shapes, exact"): a file bigger than the person's private file limit never
// goes to the server's disk. It goes from one device to the other over a
// WebRTC data channel, once the receiver has said yes.
//
// This file holds what both ends agree on, byte for byte, and the two ends of
// the channel protocol, with nothing of the page in it, so Node can run it
// (scripts/tests/file-transfer-web.test.js) and the chat page can drive it
// (web/chat/chat-file-transfer.js):
//
//  - the file pass, the one-transfer permission the sender signs for the
//    receiver (section 1 of the shapes): its words and its JSON;
//  - the three sealed notes (section 3): the offer, its answer, a cancel;
//  - the channel's messages (section 5): text messages are JSON, data messages
//    binary, at most 64 KiB each, a piece's starting with an 8-byte header;
//  - hashing a file a piece at a time (never the whole file at once);
//  - the Sender and the Receiver (section 5): hashes, get windows, piece
//    checks, three tries a piece, resume from the first missing piece, done,
//    pause, resume and cancel;
//  - the words people see (section 6).
//
// A classic script in the browser (HosFileTransfer lands on window), a
// CommonJS module under Node.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  // ── The fixed numbers (section 5) ──
  const PIECE_BYTES = 262144;            // 256 KiB, the only piece size an offer may name
  const MESSAGE_MAX = 65536;             // the most one binary message carries, header included
  const HEADER_BYTES = 8;                // piece number, offset within the piece: u32 big-endian each
  const CHUNK_BYTES = MESSAGE_MAX - HEADER_BYTES; // a piece's bytes per message
  const HASH_BYTES = 32;
  const WINDOW_MAX = 16;                 // pieces per get: a window of 4 MB
  const BUFFER_HIGH = 1024 * 1024;       // the sender holds back while the channel buffers more than this
  const PIECE_TRIES = 3;                 // three failures of one piece end the transfer
  const MAX_PIECES = 0xffffffff;         // a piece number is a u32
  const RECEIVE_MAX = 262144 + HEADER_BYTES; // a browser caps a message near 256 KiB; anything larger is not ours

  // ── The file pass (section 1) ──
  const FILE_PASS_DOMAIN = 'hum/file-pass/v1';
  const FILE_PASS_VERSION = 1;
  const FILE_PASS_MAX_SECS = 7 * 24 * 3600;   // at most 7 days ahead
  const FILE_PASS_CLOCK_SLACK_SECS = 24 * 3600; // the relay allows one day past that

  // ── The notes (section 3) ──
  const CTL_OFFER = '[[hum:offer:v1]]';
  const CTL_OFFER_ANSWER = '[[hum:offer-answer:v1]]';
  const CTL_OFFER_CANCEL = '[[hum:offer-cancel:v1]]';
  const CHANNEL_PREFIX = 'file:';
  const NAME_MAX = 1024;
  const TYPE_MAX = 255;

  const ID_RE = /^[0-9a-f]{32}$/;
  const HEX64_RE = /^[0-9a-f]{64}$/;
  const SIG_RE = /^[A-Za-z0-9+/]+={0,2}$/;

  const isInt = (n) => typeof n === 'number' && Number.isSafeInteger(n);

  /** Is `s` a transfer id: 16 bytes as 32 lowercase hex characters? */
  function idOk(s) { return typeof s === 'string' && ID_RE.test(s); }
  /** Is `s` a BLAKE3 hash in lowercase hex? */
  function hex64Ok(s) { return typeof s === 'string' && HEX64_RE.test(s); }

  /** A new transfer id from `cryptoObj.getRandomValues`. */
  function newId(cryptoObj) {
    const c = cryptoObj || (typeof crypto !== 'undefined' ? crypto : null);
    const b = c.getRandomValues(new Uint8Array(16));
    return toHex(b);
  }

  /** Lowercase hex of bytes. */
  function toHex(bytes) {
    let s = '';
    for (let i = 0; i < bytes.length; i++) s += (bytes[i] < 16 ? '0' : '') + bytes[i].toString(16);
    return s;
  }

  /** How many pieces a file of `size` bytes is: ceil(size / piece). */
  function piecesFor(size, piece) {
    const p = piece || PIECE_BYTES;
    return Math.ceil(size / p);
  }

  /** The length of piece `index` of a file of `size` bytes. */
  function pieceLength(size, piece, index) {
    const p = piece || PIECE_BYTES;
    return Math.max(0, Math.min(p, size - index * p));
  }

  // ── The pass ──

  /** An id fit to sign: not empty, no line break. */
  function filePassFieldOk(s) {
    return typeof s === 'string' && s.length > 0 && !/[\r\n]/.test(s);
  }

  /** The words a pass signs. Byte for byte the relay's (section 1). */
  function filePassPreimage(server, issuer, grantee, id, expiry) {
    return `${FILE_PASS_DOMAIN}\n${server}\n${issuer}\n${grantee}\n${id}\n${expiry}`;
  }

  /** The pass as it travels, its fields in this order: {"v":1,"id":...,"expiry":...,"sig":...}. */
  function filePassObject(id, expiry, sigB64) {
    return { v: FILE_PASS_VERSION, id, expiry, sig: sigB64 };
  }

  /** Read a pass (an object, or its JSON text) without checking its signature: the pass, or null. */
  function filePassParse(value) {
    let v = value;
    if (typeof v === 'string') {
      if (v.length > 16384) return null;
      try { v = JSON.parse(v); } catch (e) { return null; }
    }
    if (!v || typeof v !== 'object' || Array.isArray(v)) return null;
    if (v.v !== FILE_PASS_VERSION || !idOk(v.id)) return null;
    if (!isInt(v.expiry) || v.expiry <= 0) return null;
    if (typeof v.sig !== 'string' || v.sig.length === 0 || v.sig.length > 16384 || !SIG_RE.test(v.sig)) return null;
    return filePassObject(v.id, v.expiry, v.sig);
  }

  /** A pass's end, `secs` from now (at most 7 days), in Unix seconds. */
  function filePassExpiry(nowSecs, secs) {
    const ahead = Math.min(FILE_PASS_MAX_SECS, isInt(secs) && secs > 0 ? secs : FILE_PASS_MAX_SECS);
    return Math.floor(nowSecs) + ahead;
  }

  /** Is the pass still good at `nowSecs`, and not further ahead than the relay accepts? */
  function filePassTimeOk(pass, nowSecs) {
    if (!pass || !isInt(pass.expiry)) return false;
    return pass.expiry > nowSecs && pass.expiry <= nowSecs + FILE_PASS_MAX_SECS + FILE_PASS_CLOCK_SLACK_SECS;
  }

  // ── The notes ──

  /** Does this text start with one of the three notes' markers? Such text is never shown. */
  function isFileNoteText(text) {
    return typeof text === 'string'
      && (text.startsWith(CTL_OFFER) || text.startsWith(CTL_OFFER_ANSWER) || text.startsWith(CTL_OFFER_CANCEL));
  }

  /** A name fit for an offer: a non-empty string, not too long, no line breaks or other control characters. */
  function nameOk(s) {
    // eslint-disable-next-line no-control-regex
    return typeof s === 'string' && s.length > 0 && s.length <= NAME_MAX && !/[\u0000-\u001f\u007f]/.test(s);
  }
  function typeOk(s) {
    // eslint-disable-next-line no-control-regex
    return typeof s === 'string' && s.length <= TYPE_MAX && !/[\u0000-\u001f\u007f]/.test(s);
  }

  /** Check an offer's fields (the parsed JSON): the offer with its fields in order, or null. */
  function offerChecked(v) {
    if (!v || typeof v !== 'object' || Array.isArray(v)) return null;
    if (!idOk(v.id) || !nameOk(v.name) || !typeOk(v.type)) return null;
    if (!isInt(v.size) || v.size <= 0) return null;
    if (!hex64Ok(v.blake3) || !hex64Ok(v.hashes)) return null;
    if (v.piece !== PIECE_BYTES) return null;
    if (!isInt(v.pieces) || v.pieces !== piecesFor(v.size, v.piece) || v.pieces > MAX_PIECES) return null;
    const pass = filePassParse(v.pass);
    if (!pass || pass.id !== v.id) return null;
    return {
      id: v.id, name: v.name, size: v.size, type: v.type, blake3: v.blake3,
      piece: v.piece, pieces: v.pieces, hashes: v.hashes, pass,
    };
  }

  /** The offer note's text: the marker, then the offer's JSON, its fields in the spec's order. Null when a field is not fit. */
  function offerNoteText(offer) {
    const o = offerChecked(offer);
    if (!o) return null;
    return CTL_OFFER + JSON.stringify(o);
  }

  /** Read an offer note: the offer, or null when the text is not exactly one. */
  function offerNoteParse(text) {
    if (typeof text !== 'string' || !text.startsWith(CTL_OFFER)) return null;
    let v;
    try { v = JSON.parse(text.slice(CTL_OFFER.length)); } catch (e) { return null; }
    return offerChecked(v);
  }

  /** The answer note: [[hum:offer-answer:v1]]{"id":"...","accept":true|false}. */
  function answerNoteText(id, accept) {
    if (!idOk(id) || typeof accept !== 'boolean') return null;
    return CTL_OFFER_ANSWER + JSON.stringify({ id, accept });
  }

  function answerNoteParse(text) {
    if (typeof text !== 'string' || !text.startsWith(CTL_OFFER_ANSWER)) return null;
    let v;
    try { v = JSON.parse(text.slice(CTL_OFFER_ANSWER.length)); } catch (e) { return null; }
    if (!v || !idOk(v.id) || typeof v.accept !== 'boolean') return null;
    return { id: v.id, accept: v.accept };
  }

  /** The cancel note: [[hum:offer-cancel:v1]]{"id":"..."}. */
  function cancelNoteText(id) {
    if (!idOk(id)) return null;
    return CTL_OFFER_CANCEL + JSON.stringify({ id });
  }

  function cancelNoteParse(text) {
    if (typeof text !== 'string' || !text.startsWith(CTL_OFFER_CANCEL)) return null;
    let v;
    try { v = JSON.parse(text.slice(CTL_OFFER_CANCEL.length)); } catch (e) { return null; }
    if (!v || !idOk(v.id)) return null;
    return { id: v.id };
  }

  /** The data channel's label for a transfer: file:<id>. */
  function channelLabel(id) { return CHANNEL_PREFIX + id; }
  /** The transfer a channel label names, or null. */
  function channelLabelId(label) {
    if (typeof label !== 'string' || !label.startsWith(CHANNEL_PREFIX)) return null;
    const id = label.slice(CHANNEL_PREFIX.length);
    return idOk(id) ? id : null;
  }

  // ── Channel messages ──

  /** A piece's data message: the 8-byte header (piece, offset, u32 big-endian), then the bytes. */
  function pieceMessage(piece, offset, bytes) {
    const out = new Uint8Array(HEADER_BYTES + bytes.length);
    const dv = new DataView(out.buffer);
    dv.setUint32(0, piece >>> 0, false);
    dv.setUint32(4, offset >>> 0, false);
    out.set(bytes, HEADER_BYTES);
    return out;
  }

  /** Read a data message: {piece, offset, payload}, or null when it is too short. */
  function readPieceMessage(bytes) {
    if (!bytes || bytes.length < HEADER_BYTES) return null;
    const dv = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    return { piece: dv.getUint32(0, false), offset: dv.getUint32(4, false), payload: bytes.subarray(HEADER_BYTES) };
  }

  /**
   * Bytes from what a channel hands over: an ArrayBuffer, a typed array, or null. Checked by kind,
   * not with instanceof, which fails for bytes made in another realm (another frame or window).
   */
  function asBytes(data) {
    if (data == null || typeof data === 'string') return null;
    if (ArrayBuffer.isView(data)) return new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
    if (Object.prototype.toString.call(data) === '[object ArrayBuffer]') return new Uint8Array(data);
    return null;
  }

  // ── Hashing a file a piece at a time ──

  /** Read piece `index` of `file` (a Blob): only that slice is ever read. */
  async function readPiece(file, index, piece) {
    const p = piece || PIECE_BYTES;
    const start = index * p;
    const end = Math.min(Number(file.size), start + p);
    return new Uint8Array(await file.slice(start, end).arrayBuffer());
  }

  /**
   * Hash `file` (a Blob) a piece at a time: the whole file's BLAKE3, each
   * piece's, and the BLAKE3 of the piece hashes (each 32 raw bytes, in order).
   * `o.blake3` is the hash (HosBlake3: create(), hash()); `o.onProgress(done,
   * total)` after each piece; `o.stopped()` true stops it (null is returned).
   * Nothing larger than one piece is read at a time.
   */
  async function hashFile(file, o) {
    const B3 = o && o.blake3;
    if (!B3) throw new Error('hashFile needs a BLAKE3');
    const size = Number(file.size);
    const pieces = piecesFor(size, PIECE_BYTES);
    const pieceHashes = new Uint8Array(pieces * HASH_BYTES);
    const whole = B3.create();
    for (let i = 0; i < pieces; i++) {
      if (o.stopped && o.stopped()) return null;
      const bytes = await readPiece(file, i, PIECE_BYTES);
      whole.update(bytes);
      pieceHashes.set(B3.hash(bytes), i * HASH_BYTES);
      if (o.onProgress) o.onProgress(i + 1, pieces);
    }
    return { blake3: toHex(whole.digest()), pieceHashes, hashes: toHex(B3.hash(pieceHashes)), pieces, size };
  }

  // ── The sender's end of the channel ──
  //
  // One Sender per connection. `o`: {offer, file, pieceHashes, io, onEvent}.
  // io: {send(data), buffered() -> bytes queued, drained() -> a promise that
  // settles when the queue has gone down (or soon)}. onEvent(type, detail):
  // 'progress' {piece} after each piece is handed to the channel, 'paused-there',
  // 'resumed-there', and 'end' {reason}: 'done' (the receiver has the whole
  // file and it matched), 'cancelled-here', 'cancelled-there', 'unreadable'
  // (the file could not be read any more; the receiver is told to pause).
  //
  // It sends only what the receiver asks for: the piece hashes when asked
  // ({"t":"hashes"}), and each window of pieces ({"t":"get"}), a piece read
  // from the file only when it is sent, in messages of at most 64 KiB, and
  // never while the channel already holds more than 1 MB. A new get replaces
  // the window being sent (the receiver asks again after a bad piece); a pause
  // from either side drops it (the receiver asks again on resume).

  function Sender(o) {
    this.offer = o.offer;
    this.file = o.file;
    this.pieceHashes = o.pieceHashes;
    this.io = o.io;
    this.onEvent = typeof o.onEvent === 'function' ? o.onEvent : () => {};
    this.window = null;
    this.hashesWanted = false;
    this.pausedHere = !!o.paused;
    this.pausedThere = false;
    this.pumping = false;
    this.ended = false;
    if (this.pausedHere) this._say({ t: 'pause' });
  }

  Sender.prototype._say = function (obj) {
    if (this.ended) return;
    try { this.io.send(JSON.stringify(obj)); } catch (e) { /* the channel went: its close says so */ }
  };

  Sender.prototype._end = function (reason) {
    if (this.ended) return;
    this.ended = true;
    this.window = null;
    this.onEvent('end', { reason });
  };

  /** A message from the receiver. */
  Sender.prototype.receive = function (data) {
    if (this.ended || typeof data !== 'string') return; // the receiver sends no binary
    let msg;
    try { msg = JSON.parse(data); } catch (e) { return; }
    if (!msg || typeof msg.t !== 'string') return;
    const pieces = this.offer.pieces;
    switch (msg.t) {
      case 'hashes':
        this.hashesWanted = true;
        this._pump();
        break;
      case 'get': {
        const from = msg.from;
        const count = msg.count;
        if (!isInt(from) || !isInt(count) || from < 0 || from >= pieces || count < 1 || count > WINDOW_MAX) return;
        this.window = { from, end: Math.min(pieces, from + count), next: from };
        this._pump();
        break;
      }
      case 'pause':
        this.pausedThere = true;
        this.window = null;
        this.onEvent('paused-there', {});
        break;
      case 'resume':
        this.pausedThere = false;
        this.onEvent('resumed-there', {});
        break;
      case 'cancel':
        this._end('cancelled-there');
        break;
      case 'done':
        this._end('done');
        break;
      default:
        break;
    }
  };

  /** Wait while the channel holds more than BUFFER_HIGH. False when the transfer ended meanwhile. */
  Sender.prototype._room = async function () {
    while (!this.ended && this.io.buffered() > BUFFER_HIGH) await this.io.drained();
    return !this.ended;
  };

  Sender.prototype._sendHashes = async function () {
    const all = this.pieceHashes;
    for (let off = 0; off < all.length; off += MESSAGE_MAX) {
      if (!(await this._room())) return;
      this.io.send(all.slice(off, Math.min(all.length, off + MESSAGE_MAX)));
    }
  };

  Sender.prototype._pump = async function () {
    if (this.pumping) return;
    this.pumping = true;
    try {
      while (!this.ended) {
        if (this.hashesWanted) {
          this.hashesWanted = false;
          await this._sendHashes();
          continue;
        }
        const w = this.window;
        if (!w || this.pausedHere || this.pausedThere) break;
        if (w.next >= w.end) {
          if (this.window === w) this.window = null;
          break;
        }
        const index = w.next;
        let bytes;
        try {
          bytes = await readPiece(this.file, index, this.offer.piece);
        } catch (e) {
          // The file cannot be read any more (moved, deleted, or the browser let go of it).
          this._say({ t: 'pause' });
          this._end('unreadable');
          break;
        }
        if (bytes.length !== pieceLength(this.offer.size, this.offer.piece, index)) {
          this._say({ t: 'pause' });
          this._end('unreadable');
          break;
        }
        if (this.window !== w) continue; // a new get, or a pause, came while it was read
        let whole = true;
        for (let off = 0; off < bytes.length; off += CHUNK_BYTES) {
          if (!(await this._room()) || this.window !== w) { whole = false; break; }
          this.io.send(pieceMessage(index, off, bytes.subarray(off, Math.min(bytes.length, off + CHUNK_BYTES))));
        }
        if (!whole) continue;
        if (this.window === w) w.next = index + 1;
        this.onEvent('progress', { piece: index });
      }
    } finally {
      this.pumping = false;
    }
  };

  /** Pause here: the window is dropped and the receiver told. */
  Sender.prototype.pause = function () {
    if (this.ended || this.pausedHere) return;
    this.pausedHere = true;
    this.window = null;
    this._say({ t: 'pause' });
  };

  /** Resume here: the receiver is told, and asks again from the first piece it lacks. */
  Sender.prototype.resume = function () {
    if (this.ended || !this.pausedHere) return;
    this.pausedHere = false;
    this._say({ t: 'resume' });
  };

  /** Cancel: the receiver is told. (The page also sends the cancel note.) */
  Sender.prototype.cancel = function () {
    if (this.ended) return;
    this._say({ t: 'cancel' });
    this._end('cancelled-here');
  };

  /** The connection went: stop without a word. */
  Sender.prototype.close = function () {
    this.ended = true;
    this.window = null;
  };

  // ── The receiver's end of the channel ──
  //
  // One Receiver per transfer, kept across connections (attach on each new
  // one, detach when it goes). `o`: {offer, store, blake3, onEvent, stallMs,
  // timers}. store: {firstMissing() -> how many pieces from the first are kept,
  // put(index, bytes), get(index) -> bytes}. blake3: HosBlake3. onEvent(type,
  // detail): 'progress' {done, total}, 'piece-failed' {piece, tries},
  // 'paused-there', 'resumed-there', 'checking', 'complete' (every piece in and
  // the whole file's BLAKE3 matches: the page saves it now), 'end' {reason}:
  // 'cancelled-here', 'cancelled-there', 'mismatch' (the piece hashes do not
  // match the offer's `hashes`), 'mismatch-file' (the whole file's BLAKE3 does
  // not match the offer's), 'damaged' (one piece failed three times).
  //
  // It asks for the piece hashes on a connection when it does not hold them
  // yet, checks them against the offer, then asks for windows of at most 16
  // pieces from the first piece it lacks. Each piece is checked against its
  // hash before it is kept; a bad one is asked for again, and the third failure
  // of one piece ends the transfer. Pieces are kept strictly in order, so after
  // a drop (or a reload) the first piece it lacks is where it starts again.

  function Receiver(o) {
    this.offer = o.offer;
    this.store = o.store;
    this.B3 = o.blake3;
    this.onEvent = typeof o.onEvent === 'function' ? o.onEvent : () => {};
    this.stallMs = Number(o.stallMs) > 0 ? Number(o.stallMs) : 0;
    this.timers = o.timers || {
      set: (fn, ms) => setTimeout(fn, ms),
      clear: (t) => clearTimeout(t),
    };
    this.hashes = null;
    this.hashBuf = null;
    this.hashGot = 0;
    this.next = 0;
    this.fill = 0;
    this.buf = new Uint8Array(this.offer.piece);
    this.fails = new Map();
    this.whole = this.B3.create();
    this.wholeUpTo = 0;
    this.io = null;
    this.phase = 'idle';
    this.window = null;
    this.pausedHere = !!o.paused;
    this.pausedThere = false;
    this.ended = false;
    this.completed = false;
    this.stallTimer = null;
    this.queue = Promise.resolve();
    this.loaded = false;
  }

  /** Learn from the store how far it already got. */
  Receiver.prototype.load = async function () {
    const n = Number(await this.store.firstMissing());
    this.next = Number.isFinite(n) && n > 0 ? Math.min(n, this.offer.pieces) : 0;
    this.loaded = true;
    this.onEvent('progress', { done: this.next, total: this.offer.pieces });
    return this.next;
  };

  Receiver.prototype._say = function (obj) {
    if (!this.io || this.ended) return;
    try { this.io.send(JSON.stringify(obj)); } catch (e) { /* the channel went */ }
  };

  Receiver.prototype._end = function (reason) {
    if (this.ended) return;
    this.ended = true;
    this.window = null;
    this._unstall();
    this.onEvent('end', { reason });
  };

  Receiver.prototype._unstall = function () {
    if (this.stallTimer !== null) {
      this.timers.clear(this.stallTimer);
      this.stallTimer = null;
    }
  };

  /** Ask again from the first piece it lacks when nothing came for a while (nothing is counted against a piece). */
  Receiver.prototype._armStall = function () {
    this._unstall();
    if (!this.stallMs || !this.window || this.ended) return;
    this.stallTimer = this.timers.set(() => {
      this.stallTimer = null;
      if (this.window && !this.ended && this.io) {
        this.fill = 0;
        this._ask();
      }
    }, this.stallMs);
  };

  /** A connection is open: ask for what is needed. */
  Receiver.prototype.attach = async function (io) {
    if (this.ended || this.completed) return;
    if (!this.loaded) await this.load();
    this.io = io;
    this.pausedThere = false;
    this.window = null;
    this.fill = 0;
    if (this.pausedHere) this._say({ t: 'pause' });
    if (!this.hashes) {
      this.phase = 'hashes';
      this.hashBuf = new Uint8Array(this.offer.pieces * HASH_BYTES);
      this.hashGot = 0;
      this._say({ t: 'hashes' });
      return;
    }
    this.phase = 'data';
    if (this.next >= this.offer.pieces) await this._finish();
    else this._ask();
  };

  /** The connection went. */
  Receiver.prototype.detach = function () {
    this.io = null;
    this.window = null;
    this.fill = 0;
    this._unstall();
    if (this.phase === 'hashes') this.phase = 'idle';
  };

  /** A message from the sender (handled one at a time, in order). */
  Receiver.prototype.receive = function (data) {
    this.queue = this.queue.then(() => this._handle(data)).catch(() => this._end('error'));
    return this.queue;
  };

  Receiver.prototype._ask = function () {
    if (!this.io || this.ended || this.pausedHere || this.pausedThere || this.phase !== 'data') return;
    const count = Math.min(WINDOW_MAX, this.offer.pieces - this.next);
    if (count <= 0) return;
    this.window = { from: this.next, end: this.next + count };
    this._say({ t: 'get', from: this.next, count });
    this._armStall();
  };

  Receiver.prototype._failed = function () {
    const i = this.next;
    const tries = (this.fails.get(i) || 0) + 1;
    this.fails.set(i, tries);
    this.fill = 0;
    this.onEvent('piece-failed', { piece: i, tries });
    if (tries >= PIECE_TRIES) {
      this._end('damaged');
      return;
    }
    this._ask();
  };

  Receiver.prototype._handle = async function (data) {
    if (this.ended || !this.io) return;
    if (typeof data === 'string') {
      let msg;
      try { msg = JSON.parse(data); } catch (e) { return; }
      if (!msg || typeof msg.t !== 'string') return;
      if (msg.t === 'pause') {
        this.pausedThere = true;
        this.window = null;
        this.fill = 0;
        this._unstall();
        this.onEvent('paused-there', {});
      } else if (msg.t === 'resume') {
        this.pausedThere = false;
        this.onEvent('resumed-there', {});
        if (!this.pausedHere) this._ask();
      } else if (msg.t === 'cancel') {
        this._end('cancelled-there');
      }
      return;
    }
    const bytes = asBytes(data);
    if (!bytes || bytes.length > RECEIVE_MAX) return;
    if (this.phase === 'hashes') {
      if (this.hashGot + bytes.length > this.hashBuf.length) {
        this._end('mismatch');
        return;
      }
      this.hashBuf.set(bytes, this.hashGot);
      this.hashGot += bytes.length;
      if (this.hashGot < this.hashBuf.length) return;
      if (toHex(this.B3.hash(this.hashBuf)) !== this.offer.hashes) {
        this._end('mismatch');
        return;
      }
      this.hashes = this.hashBuf;
      this.hashBuf = null;
      this.phase = 'data';
      if (this.next >= this.offer.pieces) await this._finish();
      else this._ask();
      return;
    }
    if (this.phase !== 'data') return;
    const m = readPieceMessage(bytes);
    if (!m || !this.window || m.piece !== this.next) return; // not asked for, or left over from before a new ask
    if (this.fill === 0 && m.offset !== 0) return;          // waiting for the piece to start again
    const len = pieceLength(this.offer.size, this.offer.piece, this.next);
    if (m.offset !== this.fill || this.fill + m.payload.length > len) {
      this._failed();
      return;
    }
    this.buf.set(m.payload, this.fill);
    this.fill += m.payload.length;
    this._armStall();
    if (this.fill < len) return;
    const piece = this.buf.subarray(0, len);
    const want = this.hashes.subarray(this.next * HASH_BYTES, (this.next + 1) * HASH_BYTES);
    const got = this.B3.hash(piece);
    let same = true;
    for (let i = 0; i < HASH_BYTES; i++) if (got[i] !== want[i]) { same = false; break; }
    if (!same) {
      this._failed();
      return;
    }
    const index = this.next;
    await this.store.put(index, piece.slice());
    if (this.ended) return;
    if (this.wholeUpTo === index) {
      this.whole.update(piece);
      this.wholeUpTo = index + 1;
    }
    this.fill = 0;
    this.next = index + 1;
    this.onEvent('progress', { done: this.next, total: this.offer.pieces });
    if (this.next >= this.offer.pieces) {
      this.window = null;
      this._unstall();
      await this._finish();
      return;
    }
    if (this.window && this.next >= this.window.end) this._ask();
  };

  /** Every piece is in: check the whole file's BLAKE3 against the offer's before anything is saved. */
  Receiver.prototype._finish = async function () {
    if (this.ended || this.completed) return;
    this.phase = 'checking';
    this.onEvent('checking', {});
    // Pieces kept before this page (or before a gap) are read back from the store, one at a time.
    while (this.wholeUpTo < this.offer.pieces) {
      const bytes = await this.store.get(this.wholeUpTo);
      if (this.ended) return;
      if (!bytes || bytes.length !== pieceLength(this.offer.size, this.offer.piece, this.wholeUpTo)) {
        this._end('mismatch-file');
        return;
      }
      this.whole.update(bytes);
      this.wholeUpTo += 1;
    }
    if (toHex(this.whole.digest()) !== this.offer.blake3) {
      this._end('mismatch-file');
      return;
    }
    this.completed = true;
    this.phase = 'done';
    this._say({ t: 'done' });
    this.onEvent('complete', {});
  };

  /** Pause here: what is half in is dropped, the sender told. */
  Receiver.prototype.pause = function () {
    if (this.ended || this.pausedHere) return;
    this.pausedHere = true;
    this.window = null;
    this.fill = 0;
    this._unstall();
    this._say({ t: 'pause' });
  };

  /** Resume here: the sender is told, and the next window asked for (unless they paused too). */
  Receiver.prototype.resume = function () {
    if (this.ended || !this.pausedHere) return;
    this.pausedHere = false;
    this._say({ t: 'resume' });
    this._ask();
  };

  /** Cancel: the sender is told. (The page also sends the cancel note.) */
  Receiver.prototype.cancel = function () {
    if (this.ended) return;
    this._say({ t: 'cancel' });
    this._end('cancelled-here');
  };

  // ── Words (section 6) ──

  /** A size in plain words: "2.3 GB", "6 MB", "512 KB", "40 bytes". */
  function sizeWords(bytes) {
    const n = Number(bytes) || 0;
    const one = (x) => {
      const s = x.toFixed(1);
      return s.endsWith('.0') ? s.slice(0, -2) : s;
    };
    if (n < 1024) return `${n} bytes`;
    if (n < 1024 * 1024) return `${Math.round(n / 1024)} KB`;
    if (n < 1024 * 1024 * 1024) return `${one(n / 1024 / 1024)} MB`;
    return `${one(n / 1024 / 1024 / 1024)} GB`;
  }

  /** A limit in whole megabytes: "6 MB". */
  function limitWords(bytes) {
    return `${Math.round((Number(bytes) || 0) / 1024 / 1024)} MB`;
  }

  const who = (name) => String(name || '').trim() || 'They';

  const WORDS = Object.freeze({
    /** The sender's attach step: why the file goes this way. */
    directLine: (limitBytes, name) => `Files over ${limitWords(limitBytes)} go straight to ${who(name)}'s device instead of through the server.`,
    /** The sender's attach step: the page must stay open. */
    keepOpen: 'Keep this page open until it is sent: the file goes from this device, not from the server.',
    /** The receiver's question. */
    question: (name, fileName, size) => `${who(name)} wants to send you ${fileName} (${sizeWords(size)}). Accept?`,
    accept: 'Accept',
    decline: 'Decline',
    pause: 'Pause',
    resume: 'Resume',
    cancel: 'Cancel',
    chooseAgain: 'Choose the file again',
    saveAgain: 'Save again',
    // The states (section 6), and the few this page adds.
    preparing: 'Getting the file ready',
    waitingAccept: (name) => `Waiting for ${who(name)} to accept`,
    waitingOnline: (name) => `Waiting for ${who(name)} to come online`,
    connecting: 'Connecting',
    throughRelay: 'Sending through the server\'s call relay',
    receivingThroughRelay: 'Receiving through the server\'s call relay',
    direct: 'Sending straight to their device',
    receivingDirect: 'Receiving straight from their device',
    paused: 'Paused',
    pausedThere: (name) => `Paused by ${who(name)}`,
    checking: 'Checking the file',
    sent: 'Sent',
    saved: 'Saved',
    declined: (name) => `${who(name)} declined`,
    youDeclined: 'You declined',
    cancelled: 'Cancelled',
    notSetUp: 'This server is not set up for that yet',
    noAnswer: (name) => `${who(name)} did not answer in time`,
    ranOut: 'This offer ran out. Ask for it to be sent again.',
    ranOutSender: 'The offer ran out before the file went. Send it again.',
    notSigned: 'The offer could not be signed: this page has not finished signing in. Try again in a moment.',
    mismatch: (name) => `The file ${who(name)} offered does not match its description`,
    damaged: 'Part of the file arrived damaged three times, so the transfer stopped.',
    failed: 'The transfer stopped because something went wrong on this page.',
    unreadable: 'This device could not read the file any more. Choose it again to keep sending.',
    noFile: 'This page was closed or reloaded, so it no longer has the file.',
    wrongFile: 'That is not the same file. Choose the one you offered.',
    sendKeepOpen: 'The file goes from this device: keep this page open until it is sent.',
    notDelivered: 'The offer was not delivered.',
    notDeliveredReach: (name) => `The offer was not delivered: ${who(name)}'s settings do not let you reach them.`,
    notDeliveredRate: 'The offer was not delivered: too many messages at once. Try again in a minute.',
    answeredElsewhere: 'Answered on your other device',
    refused: (name, fileName, size, accept) => `${who(name)} offered you ${fileName} (${sizeWords(size)}). `
      + (accept === 'nobody' ? 'You do not accept files in messages' : 'You accept files only from friends')
      + ', so you were not asked.',
    refusedBefore: (name, accept) => (accept === 'nobody'
      ? `${who(name)} does not accept files in messages, so their app will not ask them about this one.`
      : `${who(name)} accepts files only from friends, so their app will not ask them about this one.`),
    directSetting: 'Let files from friends go straight between devices',
    directSettingOn: 'A file you send a friend, or a friend sends you, may go straight from one device to the other, which is faster, but each of you can then see the other\'s network address, which shows roughly where you are.',
    directSettingOff: 'Big files go through the server\'s call relay, so only the server sees your network address. Nothing is kept on the server.',
    directProtected: 'With the protected setup on, big files always go through the server\'s call relay.',
  });

  const api = {
    PIECE_BYTES, MESSAGE_MAX, HEADER_BYTES, CHUNK_BYTES, HASH_BYTES, WINDOW_MAX, BUFFER_HIGH, PIECE_TRIES,
    FILE_PASS_DOMAIN, FILE_PASS_VERSION, FILE_PASS_MAX_SECS, FILE_PASS_CLOCK_SLACK_SECS,
    CTL_OFFER, CTL_OFFER_ANSWER, CTL_OFFER_CANCEL, CHANNEL_PREFIX, WORDS,
    idOk, hex64Ok, newId, toHex, piecesFor, pieceLength,
    filePassFieldOk, filePassPreimage, filePassObject, filePassParse, filePassExpiry, filePassTimeOk,
    isFileNoteText, offerChecked, offerNoteText, offerNoteParse, answerNoteText, answerNoteParse,
    cancelNoteText, cancelNoteParse, channelLabel, channelLabelId,
    pieceMessage, readPieceMessage, asBytes, readPiece, hashFile, sizeWords, limitWords,
    Sender, Receiver,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else root.HosFileTransfer = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
