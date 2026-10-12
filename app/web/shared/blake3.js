// ── blake3.js ─────────────────────────────────────────────────────────────
// BLAKE3 (the plain hash, 32-byte output) in plain JavaScript, written for
// speed, for hashing big files a piece at a time in the browser: device-to-
// device big files (2026-10-11, docs/design/media-showcase.md, "Device-to-
// device big files: the shapes, exact") hash every 256 KiB piece and the
// whole file on both ends.
//
// Why a second BLAKE3 when the vendored noble bundle (shared/vendor/
// noble-pq.bundle.js) has one: that one measured about 12 MB/s on this
// machine (2026-10-11), which is over three minutes for a 2.3 GB file on the
// sender and caps a transfer at about 12 MB/s on the receiver. This one keeps
// the state in local variables and runs the seven rounds in a loop over them,
// which V8 and the other engines turn into tight code. It does only what the
// transfers need: the default hash mode, streamed (update as often as you
// like, then digest once), 32 bytes out. No keyed mode, no derive_key, no
// longer output; for those, use the noble bundle.
//
// Held to the noble bundle (itself held to the Rust blake3 crate by
// scripts/pq-kat.mjs) at every length around the block, chunk and tree
// boundaries and for every way of splitting the input across update calls:
// scripts/tests/file-transfer-web.test.js.
//
// A classic script in the browser (HosBlake3 lands on window), a CommonJS
// module under Node.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  const IV0 = 0x6a09e667, IV1 = 0xbb67ae85, IV2 = 0x3c6ef372, IV3 = 0xa54ff53a;
  const IV4 = 0x510e527f, IV5 = 0x9b05688c, IV6 = 0x1f83d9ab, IV7 = 0x5be0cd19;
  const IV = new Uint32Array([IV0, IV1, IV2, IV3, IV4, IV5, IV6, IV7]);
  // Flags (the BLAKE3 paper, section 2.1).
  const CHUNK_START = 1;
  const CHUNK_END = 2;
  const PARENT = 4;
  const ROOT = 8;
  const BLOCK_LEN = 64;
  const CHUNK_LEN = 1024;
  const OUT_LEN = 32;

  /**
   * The compression function: chaining value `cv` (8 words from `cvOff`), the
   * message block `m` (16 words), the 64-bit counter, the block's length and
   * flags. Writes the 8-word result (the first half of the output, all a
   * chaining value or a 32-byte hash needs) into `out` from `outOff`.
   */
  function compress(cv, cvOff, m, counter, blockLen, flags, out, outOff) {
    let m0 = m[0], m1 = m[1], m2 = m[2], m3 = m[3], m4 = m[4], m5 = m[5], m6 = m[6], m7 = m[7];
    let m8 = m[8], m9 = m[9], m10 = m[10], m11 = m[11], m12 = m[12], m13 = m[13], m14 = m[14], m15 = m[15];
    let v0 = cv[cvOff] | 0, v1 = cv[cvOff + 1] | 0, v2 = cv[cvOff + 2] | 0, v3 = cv[cvOff + 3] | 0;
    let v4 = cv[cvOff + 4] | 0, v5 = cv[cvOff + 5] | 0, v6 = cv[cvOff + 6] | 0, v7 = cv[cvOff + 7] | 0;
    let v8 = IV0 | 0, v9 = IV1 | 0, v10 = IV2 | 0, v11 = IV3 | 0;
    let v12 = counter % 4294967296 | 0;
    let v13 = Math.floor(counter / 4294967296) | 0;
    let v14 = blockLen | 0;
    let v15 = flags | 0;
    for (let r = 0; r < 7; r++) {
      // Columns.
      v0 = (v0 + v4 + m0) | 0; v12 ^= v0; v12 = (v12 >>> 16) | (v12 << 16);
      v8 = (v8 + v12) | 0; v4 ^= v8; v4 = (v4 >>> 12) | (v4 << 20);
      v0 = (v0 + v4 + m1) | 0; v12 ^= v0; v12 = (v12 >>> 8) | (v12 << 24);
      v8 = (v8 + v12) | 0; v4 ^= v8; v4 = (v4 >>> 7) | (v4 << 25);

      v1 = (v1 + v5 + m2) | 0; v13 ^= v1; v13 = (v13 >>> 16) | (v13 << 16);
      v9 = (v9 + v13) | 0; v5 ^= v9; v5 = (v5 >>> 12) | (v5 << 20);
      v1 = (v1 + v5 + m3) | 0; v13 ^= v1; v13 = (v13 >>> 8) | (v13 << 24);
      v9 = (v9 + v13) | 0; v5 ^= v9; v5 = (v5 >>> 7) | (v5 << 25);

      v2 = (v2 + v6 + m4) | 0; v14 ^= v2; v14 = (v14 >>> 16) | (v14 << 16);
      v10 = (v10 + v14) | 0; v6 ^= v10; v6 = (v6 >>> 12) | (v6 << 20);
      v2 = (v2 + v6 + m5) | 0; v14 ^= v2; v14 = (v14 >>> 8) | (v14 << 24);
      v10 = (v10 + v14) | 0; v6 ^= v10; v6 = (v6 >>> 7) | (v6 << 25);

      v3 = (v3 + v7 + m6) | 0; v15 ^= v3; v15 = (v15 >>> 16) | (v15 << 16);
      v11 = (v11 + v15) | 0; v7 ^= v11; v7 = (v7 >>> 12) | (v7 << 20);
      v3 = (v3 + v7 + m7) | 0; v15 ^= v3; v15 = (v15 >>> 8) | (v15 << 24);
      v11 = (v11 + v15) | 0; v7 ^= v11; v7 = (v7 >>> 7) | (v7 << 25);

      // Diagonals.
      v0 = (v0 + v5 + m8) | 0; v15 ^= v0; v15 = (v15 >>> 16) | (v15 << 16);
      v10 = (v10 + v15) | 0; v5 ^= v10; v5 = (v5 >>> 12) | (v5 << 20);
      v0 = (v0 + v5 + m9) | 0; v15 ^= v0; v15 = (v15 >>> 8) | (v15 << 24);
      v10 = (v10 + v15) | 0; v5 ^= v10; v5 = (v5 >>> 7) | (v5 << 25);

      v1 = (v1 + v6 + m10) | 0; v12 ^= v1; v12 = (v12 >>> 16) | (v12 << 16);
      v11 = (v11 + v12) | 0; v6 ^= v11; v6 = (v6 >>> 12) | (v6 << 20);
      v1 = (v1 + v6 + m11) | 0; v12 ^= v1; v12 = (v12 >>> 8) | (v12 << 24);
      v11 = (v11 + v12) | 0; v6 ^= v11; v6 = (v6 >>> 7) | (v6 << 25);

      v2 = (v2 + v7 + m12) | 0; v13 ^= v2; v13 = (v13 >>> 16) | (v13 << 16);
      v8 = (v8 + v13) | 0; v7 ^= v8; v7 = (v7 >>> 12) | (v7 << 20);
      v2 = (v2 + v7 + m13) | 0; v13 ^= v2; v13 = (v13 >>> 8) | (v13 << 24);
      v8 = (v8 + v13) | 0; v7 ^= v8; v7 = (v7 >>> 7) | (v7 << 25);

      v3 = (v3 + v4 + m14) | 0; v14 ^= v3; v14 = (v14 >>> 16) | (v14 << 16);
      v9 = (v9 + v14) | 0; v4 ^= v9; v4 = (v4 >>> 12) | (v4 << 20);
      v3 = (v3 + v4 + m15) | 0; v14 ^= v3; v14 = (v14 >>> 8) | (v14 << 24);
      v9 = (v9 + v14) | 0; v4 ^= v9; v4 = (v4 >>> 7) | (v4 << 25);

      // The message permutation, between rounds:
      // [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8].
      if (r < 6) {
        const t0 = m0, t1 = m1, t2 = m2, t3 = m3, t4 = m4, t5 = m5, t6 = m6, t7 = m7;
        const t8 = m8, t9 = m9, t10 = m10, t11 = m11, t12 = m12, t13 = m13, t14 = m14, t15 = m15;
        m0 = t2; m1 = t6; m2 = t3; m3 = t10; m4 = t7; m5 = t0; m6 = t4; m7 = t13;
        m8 = t1; m9 = t11; m10 = t12; m11 = t5; m12 = t9; m13 = t14; m14 = t15; m15 = t8;
      }
    }
    out[outOff] = v0 ^ v8;
    out[outOff + 1] = v1 ^ v9;
    out[outOff + 2] = v2 ^ v10;
    out[outOff + 3] = v3 ^ v11;
    out[outOff + 4] = v4 ^ v12;
    out[outOff + 5] = v5 ^ v13;
    out[outOff + 6] = v6 ^ v14;
    out[outOff + 7] = v7 ^ v15;
  }

  /** Read 64 bytes (little-endian words) from `bytes` at `off` into the 16-word `m`. */
  function loadBlock(bytes, off, m) {
    for (let i = 0; i < 16; i++) {
      const o = off + i * 4;
      m[i] = bytes[o] | (bytes[o + 1] << 8) | (bytes[o + 2] << 16) | (bytes[o + 3] << 24);
    }
  }

  /** One streaming hash. update() as often as wanted, then digest() once. */
  function Hasher() {
    this.cv = new Uint32Array(IV);        // the current chunk's chaining value
    this.block = new Uint8Array(BLOCK_LEN); // the block not compressed yet
    this.blockLen = 0;
    this.blocksCompressed = 0;            // blocks of the current chunk compressed so far
    this.chunkCounter = 0;                // chunks finished before this one
    this.stack = [];                      // the chaining values of finished subtrees
    this.m = new Uint32Array(16);         // scratch: one block's words
    this.done = false;
  }

  Hasher.prototype._startFlag = function () {
    return this.blocksCompressed === 0 ? CHUNK_START : 0;
  };

  /** Add a finished chunk's chaining value, merging every completed subtree (the paper's 5.1.2). */
  Hasher.prototype._pushChunk = function (chunkCv, totalChunks) {
    let cv = chunkCv;
    let total = totalChunks;
    const m = this.m;
    while (total % 2 === 0) {
      const left = this.stack.pop();
      for (let i = 0; i < 8; i++) { m[i] = left[i]; m[i + 8] = cv[i]; }
      const parent = new Uint32Array(8);
      compress(IV, 0, m, 0, BLOCK_LEN, PARENT, parent, 0);
      cv = parent;
      total /= 2;
    }
    this.stack.push(cv);
  };

  /** Hash `input` (a Uint8Array, or an ArrayBuffer) into this hasher. Returns this. */
  Hasher.prototype.update = function (input) {
    if (this.done) throw new Error('BLAKE3: update after digest');
    // Any view (or an ArrayBuffer), from any realm, read in place.
    const bytes = ArrayBuffer.isView(input) ? new Uint8Array(input.buffer, input.byteOffset, input.byteLength) : new Uint8Array(input);
    const len = bytes.length;
    const m = this.m;
    let pos = 0;
    while (pos < len) {
      // The current chunk is full and more input follows: it is not the last, so finish it.
      if (this.blocksCompressed * BLOCK_LEN + this.blockLen === CHUNK_LEN) {
        loadBlock(this.block, 0, m);
        const chunkCv = new Uint32Array(8);
        compress(this.cv, 0, m, this.chunkCounter, BLOCK_LEN, CHUNK_END | this._startFlag(), chunkCv, 0);
        this.chunkCounter += 1;
        this._pushChunk(chunkCv, this.chunkCounter);
        this.cv.set(IV);
        this.blockLen = 0;
        this.blocksCompressed = 0;
      }
      // A full block buffered and more input follows (within this chunk): compress it.
      if (this.blockLen === BLOCK_LEN) {
        loadBlock(this.block, 0, m);
        compress(this.cv, 0, m, this.chunkCounter, BLOCK_LEN, this._startFlag(), this.cv, 0);
        this.blocksCompressed += 1;
        this.blockLen = 0;
      }
      // Whole blocks straight from the input, while more input follows each and the block is
      // not its chunk's sixteenth (which is kept: it may be the chunk's last).
      while (this.blockLen === 0 && this.blocksCompressed < 15 && len - pos > BLOCK_LEN) {
        loadBlock(bytes, pos, m);
        compress(this.cv, 0, m, this.chunkCounter, BLOCK_LEN, this._startFlag(), this.cv, 0);
        this.blocksCompressed += 1;
        pos += BLOCK_LEN;
      }
      const room = Math.min(BLOCK_LEN - this.blockLen, CHUNK_LEN - (this.blocksCompressed * BLOCK_LEN + this.blockLen));
      const take = Math.min(room, len - pos);
      this.block.set(bytes.subarray(pos, pos + take), this.blockLen);
      this.blockLen += take;
      pos += take;
    }
    return this;
  };

  /** The 32-byte hash. The hasher is finished after this. */
  Hasher.prototype.digest = function () {
    if (this.done) throw new Error('BLAKE3: digest twice');
    this.done = true;
    const m = new Uint32Array(16);
    const padded = new Uint8Array(BLOCK_LEN);
    padded.set(this.block.subarray(0, this.blockLen));
    loadBlock(padded, 0, m);
    // The last output: the current chunk's last block, then a parent per subtree on the stack.
    let inCv = this.cv;
    let counter = this.chunkCounter;
    let blockLen = this.blockLen;
    let flags = CHUNK_END | this._startFlag();
    for (let i = this.stack.length - 1; i >= 0; i--) {
      const right = new Uint32Array(8);
      compress(inCv, 0, m, counter, blockLen, flags, right, 0);
      const left = this.stack[i];
      for (let k = 0; k < 8; k++) { m[k] = left[k]; m[k + 8] = right[k]; }
      inCv = IV;
      counter = 0;
      blockLen = BLOCK_LEN;
      flags = PARENT;
    }
    const words = new Uint32Array(8);
    // The root: its output block counter is 0, whatever the chunk's counter was.
    compress(inCv, 0, m, 0, blockLen, flags | ROOT, words, 0);
    const out = new Uint8Array(OUT_LEN);
    for (let i = 0; i < 8; i++) {
      const w = words[i];
      out[i * 4] = w & 0xff;
      out[i * 4 + 1] = (w >>> 8) & 0xff;
      out[i * 4 + 2] = (w >>> 16) & 0xff;
      out[i * 4 + 3] = (w >>> 24) & 0xff;
    }
    return out;
  };

  /** A new streaming hasher. */
  function create() {
    return new Hasher();
  }

  /** BLAKE3 of `bytes`, 32 bytes. */
  function hash(bytes) {
    return create().update(bytes).digest();
  }

  /** Lowercase hex of bytes. */
  function toHex(bytes) {
    let s = '';
    for (let i = 0; i < bytes.length; i++) s += (bytes[i] < 16 ? '0' : '') + bytes[i].toString(16);
    return s;
  }

  const api = { create, hash, toHex, OUT_LEN };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else root.HosBlake3 = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
