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
//  - the Sender and the Receiver (section 5, and rule 1 of "...: rules
//    settled by the first review"): hashes, gets that add to what is owed,
//    a window of 16 kept full a piece at a time, piece checks, three tries a
//    piece, resume from the first missing piece, done, pause, resume and
//    cancel; and (rule 8 of "...: settled by the second review") a receiver
//    whose transfer ends for any reason other than done says cancel on the
//    channel, so the sender's row ends too;
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
  // One Sender per data channel. `o`: {offer, file, pieceHashes, io, onEvent}.
  // io: {send(data), buffered() -> bytes queued, drained() -> a promise that
  // settles when the queue has gone down (or soon)}. onEvent(type, detail):
  // 'progress' {piece} after each piece is handed to the channel, 'paused-there',
  // 'resumed-there', and 'end' {reason}: 'done' (the receiver has the whole
  // file and it matched), 'cancelled-here', 'cancelled-there', 'unreadable'
  // (the file could not be read any more; the receiver is told to pause).
  //
  // It sends only what the receiver asks for: the piece hashes when asked
  // ({"t":"hashes"}), and the pieces it is asked for ({"t":"get"}), a piece read
  // from the file only when it is sent, in messages of at most 64 KiB, and
  // never while the channel already holds more than 1 MB.
  //
  // Rule 1 of "Device-to-device big files: rules settled by the first review"
  // (2026-10-12): a get ADDS pieces to what the sender owes, it never replaces
  // what was asked before. The desktop app keeps its window full by asking for
  // one more piece each time one arrives ({"from":n,"count":1}); a sender that
  // took each get as the whole new window dropped the rest of the old one, so
  // 200 pieces took 188 round trips and 30-second stalls (found by the review).
  // What is owed goes in ascending order, each piece once, from offset 0, and
  // is forgotten once its last message has gone. A piece asked for while it is
  // being sent is sent again, whole, after the current one: the receiver may
  // have found the earlier copy bad. A pause from either side drops everything
  // owed (the receiver asks again on resume).

  function Sender(o) {
    this.offer = o.offer;
    this.file = o.file;
    this.pieceHashes = o.pieceHashes;
    this.io = o.io;
    this.onEvent = typeof o.onEvent === 'function' ? o.onEvent : () => {};
    this.owed = [];        // piece numbers owed, ascending, each once
    this.current = null;   // the piece going out now: {index, again, dropped}
    this.hashesWanted = false;
    this.pausedHere = !!o.paused;
    this.pausedThere = false;
    this.pumping = false;
    this.ended = false;
    if (this.pausedHere) this._say({ t: 'pause' });
  }

  /** Put `n` into the ascending list `list` unless it is there. */
  function insertAscending(list, n) {
    let lo = 0;
    let hi = list.length;
    while (lo < hi) {
      const mid = (lo + hi) >>> 1;
      if (list[mid] < n) lo = mid + 1; else hi = mid;
    }
    if (list[lo] !== n) list.splice(lo, 0, n);
  }

  /** Owe piece `index`: added in order, or (the piece going out now) sent again after it. */
  Sender.prototype._owe = function (index) {
    if (this.current && this.current.index === index && !this.current.dropped) {
      this.current.again = true;
      return;
    }
    insertAscending(this.owed, index);
  };

  /** Drop everything owed, the piece going out included (a pause from either side). */
  Sender.prototype._dropOwed = function () {
    this.owed = [];
    if (this.current) this.current.dropped = true;
  };

  Sender.prototype._say = function (obj) {
    if (this.ended) return;
    try { this.io.send(JSON.stringify(obj)); } catch (e) { /* the channel went: its close says so */ }
  };

  Sender.prototype._end = function (reason) {
    if (this.ended) return;
    this.ended = true;
    this._dropOwed();
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
        const end = Math.min(pieces, from + count);
        for (let i = from; i < end; i++) this._owe(i);
        this._pump();
        break;
      }
      case 'pause':
        this.pausedThere = true;
        this._dropOwed();
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
        if (this.pausedHere || this.pausedThere || !this.owed.length) break;
        // The lowest piece owed, from offset 0. A get for it from now on asks for it again.
        const cur = { index: this.owed.shift(), again: false, dropped: false };
        this.current = cur;
        let bytes;
        try {
          bytes = await readPiece(this.file, cur.index, this.offer.piece);
        } catch (e) {
          // The file cannot be read any more (moved, deleted, or the browser let go of it).
          this.current = null;
          this._say({ t: 'pause' });
          this._end('unreadable');
          break;
        }
        if (bytes.length !== pieceLength(this.offer.size, this.offer.piece, cur.index)) {
          this.current = null;
          this._say({ t: 'pause' });
          this._end('unreadable');
          break;
        }
        let whole = true;
        for (let off = 0; off < bytes.length; off += CHUNK_BYTES) {
          if (!(await this._room()) || cur.dropped) { whole = false; break; }
          this.io.send(pieceMessage(cur.index, off, bytes.subarray(off, Math.min(bytes.length, off + CHUNK_BYTES))));
        }
        if (this.current === cur) this.current = null;
        if (!whole) continue; // a pause (or the end) dropped it part way: forgotten, asked again on resume
        this.onEvent('progress', { piece: cur.index });
        if (cur.again) insertAscending(this.owed, cur.index);
      }
    } catch (e) {
      // The channel refused a message (it is closing): its close ends this engine.
    } finally {
      this.pumping = false;
    }
  };

  /** Pause here: everything owed is dropped and the receiver told. */
  Sender.prototype.pause = function () {
    if (this.ended || this.pausedHere) return;
    this.pausedHere = true;
    this._dropOwed();
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

  /** The channel went: stop without a word. */
  Sender.prototype.close = function () {
    this.ended = true;
    this._dropOwed();
  };

  // ── The receiver's end of the channel ──
  //
  // One Receiver per transfer, kept across channels (attach on each new one,
  // detach when it goes). `o`: {offer, store, blake3, onEvent, stallMs, timers}.
  // store: {held() -> the numbers of the pieces kept so far (any order),
  // put(index, bytes), get(index) -> bytes}. blake3: HosBlake3. onEvent(type,
  // detail): 'progress' {done, total, low} (done: pieces kept; low: the first
  // piece not kept), 'piece-failed' {piece, tries}, 'paused-there',
  // 'resumed-there', 'checking', 'complete' (every piece in and the whole
  // file's BLAKE3 matches: the page saves it now), 'end' {reason}:
  // 'cancelled-here', 'cancelled-there', 'mismatch' (the piece hashes do not
  // match the offer's `hashes`), 'mismatch-file' (the whole file's BLAKE3 does
  // not match the offer's), 'damaged' (one piece failed three times),
  // 'storage' (the store refused a piece: store.put threw), 'error' (anything
  // else that went wrong handling a message).
  //
  // Rule 8 ("Settled by the second review", 2026-10-12, under "Device-to-device
  // big files: rules settled by the first review"): an end is always said. Every end
  // but 'cancelled-there' (the sender said it) sends {"t":"cancel"} on the
  // channel before the 'end' event ('cancelled-here' through cancel()). Until
  // then this Receiver ended a damaged, mismatched or unstorable file without a
  // word, so a desktop sender kept waiting for it until the pass ran out, up to
  // 7 days. (The page sends the cancel note as well.)
  //
  // It asks for the piece hashes on a channel when it does not hold them yet,
  // checks them against the offer, then asks for pieces. Rule 1 of "Device-to-
  // device big files: rules settled by the first review" (2026-10-12), the same
  // in both apps:
  //  - it asks for a piece once, and again only after it failed its hash (a get
  //    of that one piece) or after a stall with nothing arriving (`stallMs`);
  //  - it keeps its window (16 pieces) full by asking for pieces it has not
  //    asked for yet, the first ones it lacks, as one get per run: so after the
  //    first window, each piece that arrives is followed by a get of one more,
  //    and the rest of a window is never asked again;
  //  - it ignores messages for a piece it did not ask for or already holds, and
  //    holds partial buffers only for pieces it asked for (at most 16).
  // Each piece is checked against its hash before it is kept, in whatever order
  // it arrives (a piece asked again after a bad hash comes after the ones asked
  // before it); the third failure of one piece ends the transfer. After a drop
  // (or a reload) it starts again from the first piece it lacks.

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
    this.have = new Uint8Array(this.offer.pieces); // 1: the piece is kept
    this.haveCount = 0;
    this.low = 0;                                  // no piece before this one is missing
    this.asked = new Set();                        // asked for on this channel, not in yet
    this.partial = new Map();                      // piece -> {buf, fill}: only pieces asked for
    this.fails = new Map();
    this.whole = this.B3.create();
    this.wholeUpTo = 0;                            // the whole-file hash has taken the pieces before this one
    this.io = null;
    this.phase = 'idle';
    this.pausedHere = !!o.paused;
    this.pausedThere = false;
    this.ended = false;
    this.completed = false;
    this.stallTimer = null;
    this.queue = Promise.resolve();
    this.loaded = false;
  }

  /** Learn from the store which pieces it already holds. */
  Receiver.prototype.load = async function () {
    const n = this.offer.pieces;
    const held = await this.store.held();
    for (const i of Array.isArray(held) ? held : []) {
      if (isInt(i) && i >= 0 && i < n && !this.have[i]) {
        this.have[i] = 1;
        this.haveCount += 1;
      }
    }
    while (this.low < n && this.have[this.low]) this.low += 1;
    this.loaded = true;
    this._progress();
    return this.low;
  };

  Receiver.prototype._progress = function () {
    this.onEvent('progress', { done: this.haveCount, total: this.offer.pieces, low: this.low });
  };

  Receiver.prototype._say = function (obj) {
    if (!this.io || this.ended) return;
    try { this.io.send(JSON.stringify(obj)); } catch (e) { /* the channel went */ }
  };

  Receiver.prototype._end = function (reason) {
    if (this.ended) return;
    // Rule 8: said on the channel while it is there (cancel() has said it already; the sender's own
    // cancel needs no answer).
    if (reason !== 'cancelled-here' && reason !== 'cancelled-there') this._say({ t: 'cancel' });
    this.ended = true;
    this._forget();
    this.onEvent('end', { reason });
  };

  /** Nothing asked for any more, nothing half in, no clock running. */
  Receiver.prototype._forget = function () {
    this.asked.clear();
    this.partial.clear();
    this._unstall();
  };

  Receiver.prototype._unstall = function () {
    if (this.stallTimer !== null) {
      this.timers.clear(this.stallTimer);
      this.stallTimer = null;
    }
  };

  /**
   * The stall clock, started again: nothing arriving for `stallMs` while pieces are asked for, and
   * they are asked for again (nothing is counted against a piece for it).
   */
  Receiver.prototype._watch = function () {
    this._unstall();
    if (!this.stallMs || this.ended || !this.asked.size) return;
    this.stallTimer = this.timers.set(() => {
      this.stallTimer = null;
      if (this.ended || !this.io || !this.asked.size) return;
      this.asked.clear();
      this.partial.clear();
      this._pump();
    }, this.stallMs);
  };

  /** A channel is open: ask for what is needed. */
  Receiver.prototype.attach = async function (io) {
    if (this.ended || this.completed) return;
    if (!this.loaded) await this.load();
    this.io = io;
    this.pausedThere = false;
    this._forget();
    if (this.pausedHere) this._say({ t: 'pause' });
    if (!this.hashes) {
      this.phase = 'hashes';
      this.hashBuf = new Uint8Array(this.offer.pieces * HASH_BYTES);
      this.hashGot = 0;
      this._say({ t: 'hashes' });
      return;
    }
    this.phase = 'data';
    if (this.haveCount >= this.offer.pieces) await this._finish();
    else this._pump();
  };

  /** The channel went: what was asked on it went with it. */
  Receiver.prototype.detach = function () {
    this.io = null;
    this._forget();
    if (this.phase === 'hashes') this.phase = 'idle';
  };

  /** A message from the sender (handled one at a time, in order). */
  Receiver.prototype.receive = function (data) {
    this.queue = this.queue.then(() => this._handle(data)).catch(() => this._end('error'));
    return this.queue;
  };

  /** May it ask for pieces now? */
  Receiver.prototype._mayAsk = function () {
    return !!this.io && !this.ended && !this.pausedHere && !this.pausedThere && this.phase === 'data';
  };

  /**
   * Keep the window full: the first pieces it lacks and has not asked for, until 16 are asked
   * for, one get per run of them. Nothing already asked for is asked again here.
   */
  Receiver.prototype._pump = function () {
    if (!this._mayAsk()) return;
    const n = this.offer.pieces;
    while (this.low < n && this.have[this.low]) this.low += 1;
    let room = WINDOW_MAX - this.asked.size;
    let i = this.low;
    let asked = false;
    while (room > 0 && i < n) {
      while (i < n && (this.have[i] || this.asked.has(i))) i += 1;
      if (i >= n) break;
      const from = i;
      let count = 0;
      while (i < n && count < room && !this.have[i] && !this.asked.has(i)) {
        this.asked.add(i);
        i += 1;
        count += 1;
      }
      room -= count;
      this._say({ t: 'get', from, count });
      asked = true;
    }
    if (asked && this.stallTimer === null) this._watch();
  };

  /** Piece `p` failed (its hash, or more bytes than it has): asked for again alone, or the end. */
  Receiver.prototype._failed = function (p) {
    this.partial.delete(p);
    this.asked.delete(p);
    const tries = (this.fails.get(p) || 0) + 1;
    this.fails.set(p, tries);
    this.onEvent('piece-failed', { piece: p, tries });
    if (tries >= PIECE_TRIES) {
      this._end('damaged');
      return;
    }
    if (!this._mayAsk()) return; // paused: asked again on resume
    this.asked.add(p);
    this._say({ t: 'get', from: p, count: 1 });
    this._watch();
  };

  Receiver.prototype._handle = async function (data) {
    if (this.ended || !this.io) return;
    if (typeof data === 'string') {
      let msg;
      try { msg = JSON.parse(data); } catch (e) { return; }
      if (!msg || typeof msg.t !== 'string') return;
      if (msg.t === 'pause') {
        // The sender dropped what it owed: nothing is asked for until it resumes.
        this.pausedThere = true;
        this._forget();
        this.onEvent('paused-there', {});
      } else if (msg.t === 'resume') {
        this.pausedThere = false;
        this.onEvent('resumed-there', {});
        if (!this.pausedHere) this._pump();
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
      if (this.haveCount >= this.offer.pieces) await this._finish();
      else this._pump();
      return;
    }
    if (this.phase !== 'data') return;
    const m = readPieceMessage(bytes);
    if (!m) return;
    const p = m.piece;
    // Not asked for (left over from before a pause, or never asked), or already kept.
    if (p >= this.offer.pieces || this.have[p] || !this.asked.has(p)) return;
    this._watch();
    const len = pieceLength(this.offer.size, this.offer.piece, p);
    if (m.offset + m.payload.length > len) {
      this._failed(p);
      return;
    }
    let part;
    if (m.offset === 0) {
      // The piece starts (again): a piece sent again starts afresh.
      part = { buf: new Uint8Array(len), fill: 0 };
      this.partial.set(p, part);
    } else {
      part = this.partial.get(p);
      if (!part || part.fill !== m.offset) {
        // Cut off part way (a pause on the other side): forgotten until it starts again.
        this.partial.delete(p);
        return;
      }
    }
    part.buf.set(m.payload, part.fill);
    part.fill += m.payload.length;
    if (part.fill < len) return;
    this.partial.delete(p);
    const want = this.hashes.subarray(p * HASH_BYTES, (p + 1) * HASH_BYTES);
    const got = this.B3.hash(part.buf);
    let same = true;
    for (let i = 0; i < HASH_BYTES; i++) if (got[i] !== want[i]) { same = false; break; }
    if (!same) {
      this._failed(p);
      return;
    }
    try {
      await this.store.put(p, part.buf);
    } catch (e) {
      // The store refused it (the browser's storage is full, or it went): the transfer cannot go on.
      this._end('storage');
      return;
    }
    if (this.ended || this.have[p]) return;
    this.have[p] = 1;
    this.haveCount += 1;
    this.asked.delete(p);
    this.fails.delete(p);
    while (this.low < this.offer.pieces && this.have[this.low]) this.low += 1;
    await this._hashOn(p, part.buf);
    if (this.ended) return;
    this._progress();
    if (this.haveCount >= this.offer.pieces) {
      this._forget();
      await this._finish();
      return;
    }
    if (!this.asked.size) this._unstall();
    this._pump();
  };

  /**
   * The whole-file hash takes the pieces in order as they come: piece `p` (just kept, `bytes`) when
   * it is the next one, then any kept after it that came early, read back from the store (a few at
   * most: only pieces asked for come early). Pieces kept before this page are read back at the end.
   */
  Receiver.prototype._hashOn = async function (p, bytes) {
    if (p !== this.wholeUpTo) return;
    this.whole.update(bytes);
    this.wholeUpTo += 1;
    let more = 0;
    while (this.wholeUpTo < this.offer.pieces && this.have[this.wholeUpTo] && more < 2 * WINDOW_MAX) {
      const b = await this.store.get(this.wholeUpTo);
      if (this.ended) return;
      if (!b || b.length !== pieceLength(this.offer.size, this.offer.piece, this.wholeUpTo)) return; // the end's check says so
      this.whole.update(b);
      this.wholeUpTo += 1;
      more += 1;
    }
  };

  /** Every piece is in: check the whole file's BLAKE3 against the offer's before anything is saved. */
  Receiver.prototype._finish = async function () {
    if (this.ended || this.completed) return;
    this.phase = 'checking';
    this.onEvent('checking', {});
    // Pieces kept before this page (or the hash has not reached yet) are read back from the store, one at a time.
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
    this._forget();
    this._say({ t: 'pause' });
  };

  /** Resume here: the sender is told, and the first pieces it lacks asked for (unless they paused too). */
  Receiver.prototype.resume = function () {
    if (this.ended || !this.pausedHere) return;
    this.pausedHere = false;
    this._say({ t: 'resume' });
    this._pump();
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
    // What each button does, in the hint shown over it (shell.js's rich tooltip reads data-tip-desc;
    // without one it said only "Tap or click to use this control.").
    tipAccept: (name) => `Say yes. The file comes straight from ${who(name)}'s device, and you can choose where to save it.`,
    tipDecline: (name) => `Say no. ${who(name)} is told you declined.`,
    tipPause: 'Hold the transfer here. Nothing more goes until you press Resume.',
    tipResume: 'Carry on from where it stopped.',
    tipCancel: (name) => `Stop this transfer for good. ${who(name)} is told.`,
    tipChoose: 'This page no longer has the file. Choose the same file again to keep sending.',
    tipSaveAgain: 'Save the same file once more, while this browser still holds it.',
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
    choosePlace: 'Choose where to save it',
    notSaved: 'It arrived whole, but this browser could not save it',
    trySaveAgain: 'Try saving again',
    tipTrySaveAgain: 'Save the file again from the copy this page still holds.',
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
    storage: 'This browser could not keep part of the file (its storage may be full), so the transfer stopped.',
    /** The sender's row when the receiver's side closed the transfer's channel before the end (rule 8). */
    stoppedThere: (name) => `${who(name)}'s side stopped it`,
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
