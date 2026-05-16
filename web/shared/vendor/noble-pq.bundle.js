/* HumanityOS vendored post-quantum bundle — DO NOT EDIT BY HAND.
 * Regenerate: node scripts/build-noble-bundle.mjs
 * Source: @noble/post-quantum@0.6.1 + @noble/hashes@2.2.0
 * Exports: ml_dsa65 (Dilithium3/ML-DSA-65), ml_kem768 (Kyber768),
 *          blake3. Self-contained ESM, served same-origin so a
 *          primary-identity dependency never relies on a CDN.
 * KAT: BLAKE3-derive(hum/dilithium3/v1, 32x0x07) ->
 *      f0dfc6e8cc3eebd2e0f0265d2aae0f339090f2d4f92726884e385a48e81e0cc4
 *      ml_dsa65.keygen(seed).publicKey blake3 = 3f4ff5c7e6505ca7b0dd6cb32c53839f8cff19772e291d4f18b082d1f7dc0126
 *      (matches src/relay/core/pq_crypto.rs::dilithium_cross_language_kat)
 * sha256(body) = 81861d591fd8d5ff7cb3332dfd576ad6b7200c6e60e5cb96658ed34837c8e7c3
 */
// node_modules/@noble/hashes/utils.js
function isBytes(a) {
  return a instanceof Uint8Array || ArrayBuffer.isView(a) && a.constructor.name === "Uint8Array" && "BYTES_PER_ELEMENT" in a && a.BYTES_PER_ELEMENT === 1;
}
function anumber(n, title = "") {
  if (typeof n !== "number") {
    const prefix = title && `"${title}" `;
    throw new TypeError(`${prefix}expected number, got ${typeof n}`);
  }
  if (!Number.isSafeInteger(n) || n < 0) {
    const prefix = title && `"${title}" `;
    throw new RangeError(`${prefix}expected integer >= 0, got ${n}`);
  }
}
function abytes(value, length, title = "") {
  const bytes = isBytes(value);
  const len = value?.length;
  const needsLen = length !== void 0;
  if (!bytes || needsLen && len !== length) {
    const prefix = title && `"${title}" `;
    const ofLen = needsLen ? ` of length ${length}` : "";
    const got = bytes ? `length=${len}` : `type=${typeof value}`;
    const message = prefix + "expected Uint8Array" + ofLen + ", got " + got;
    if (!bytes)
      throw new TypeError(message);
    throw new RangeError(message);
  }
  return value;
}
function copyBytes(bytes) {
  return Uint8Array.from(abytes(bytes));
}
function aexists(instance, checkFinished = true) {
  if (instance.destroyed)
    throw new Error("Hash instance has been destroyed");
  if (checkFinished && instance.finished)
    throw new Error("Hash#digest() has already been called");
}
function aoutput(out, instance) {
  abytes(out, void 0, "digestInto() output");
  const min = instance.outputLen;
  if (out.length < min) {
    throw new RangeError('"digestInto() output" expected to be of length >=' + min);
  }
}
function u8(arr) {
  return new Uint8Array(arr.buffer, arr.byteOffset, arr.byteLength);
}
function u32(arr) {
  return new Uint32Array(arr.buffer, arr.byteOffset, Math.floor(arr.byteLength / 4));
}
function clean(...arrays) {
  for (let i = 0; i < arrays.length; i++) {
    arrays[i].fill(0);
  }
}
function rotr(word, shift) {
  return word << 32 - shift | word >>> shift;
}
var isLE = /* @__PURE__ */ (() => new Uint8Array(new Uint32Array([287454020]).buffer)[0] === 68)();
function byteSwap(word) {
  return word << 24 & 4278190080 | word << 8 & 16711680 | word >>> 8 & 65280 | word >>> 24 & 255;
}
var swap8IfBE = isLE ? (n) => n : (n) => byteSwap(n) >>> 0;
function byteSwap32(arr) {
  for (let i = 0; i < arr.length; i++) {
    arr[i] = byteSwap(arr[i]);
  }
  return arr;
}
var swap32IfBE = isLE ? (u) => u : byteSwap32;
function concatBytes(...arrays) {
  let sum = 0;
  for (let i = 0; i < arrays.length; i++) {
    const a = arrays[i];
    abytes(a);
    sum += a.length;
  }
  const res = new Uint8Array(sum);
  for (let i = 0, pad = 0; i < arrays.length; i++) {
    const a = arrays[i];
    res.set(a, pad);
    pad += a.length;
  }
  return res;
}
function createHasher(hashCons, info = {}) {
  const hashC = (msg, opts2) => hashCons(opts2).update(msg).digest();
  const tmp = hashCons(void 0);
  hashC.outputLen = tmp.outputLen;
  hashC.blockLen = tmp.blockLen;
  hashC.canXOF = tmp.canXOF;
  hashC.create = (opts2) => hashCons(opts2);
  Object.assign(hashC, info);
  return Object.freeze(hashC);
}
function randomBytes(bytesLength = 32) {
  anumber(bytesLength, "bytesLength");
  const cr = typeof globalThis === "object" ? globalThis.crypto : null;
  if (typeof cr?.getRandomValues !== "function")
    throw new Error("crypto.getRandomValues must be defined");
  if (bytesLength > 65536)
    throw new RangeError(`"bytesLength" expected <= 65536, got ${bytesLength}`);
  return cr.getRandomValues(new Uint8Array(bytesLength));
}
var oidNist = (suffix) => ({
  // Current NIST hashAlgs suffixes used here fit in one DER subidentifier octet.
  // Larger suffix values would need base-128 OID encoding and a different length byte.
  oid: Uint8Array.from([6, 9, 96, 134, 72, 1, 101, 3, 4, 2, suffix])
});

// node_modules/@noble/curves/utils.js
function abool(value, title = "") {
  if (typeof value !== "boolean") {
    const prefix = title && `"${title}" `;
    throw new TypeError(prefix + "expected boolean, got type=" + typeof value);
  }
  return value;
}

// node_modules/@noble/hashes/_u64.js
var U32_MASK64 = /* @__PURE__ */ BigInt(2 ** 32 - 1);
var _32n = /* @__PURE__ */ BigInt(32);
function fromBig(n, le = false) {
  if (le)
    return { h: Number(n & U32_MASK64), l: Number(n >> _32n & U32_MASK64) };
  return { h: Number(n >> _32n & U32_MASK64) | 0, l: Number(n & U32_MASK64) | 0 };
}
function split(lst, le = false) {
  const len = lst.length;
  let Ah = new Uint32Array(len);
  let Al = new Uint32Array(len);
  for (let i = 0; i < len; i++) {
    const { h, l } = fromBig(lst[i], le);
    [Ah[i], Al[i]] = [h, l];
  }
  return [Ah, Al];
}
var rotlSH = (h, l, s) => h << s | l >>> 32 - s;
var rotlSL = (h, l, s) => l << s | h >>> 32 - s;
var rotlBH = (h, l, s) => l << s - 32 | h >>> 64 - s;
var rotlBL = (h, l, s) => h << s - 32 | l >>> 64 - s;

// node_modules/@noble/hashes/sha3.js
var _0n = BigInt(0);
var _1n = BigInt(1);
var _2n = BigInt(2);
var _7n = BigInt(7);
var _256n = BigInt(256);
var _0x71n = BigInt(113);
var SHA3_PI = [];
var SHA3_ROTL = [];
var _SHA3_IOTA = [];
for (let round = 0, R = _1n, x = 1, y = 0; round < 24; round++) {
  [x, y] = [y, (2 * x + 3 * y) % 5];
  SHA3_PI.push(2 * (5 * y + x));
  SHA3_ROTL.push((round + 1) * (round + 2) / 2 % 64);
  let t = _0n;
  for (let j = 0; j < 7; j++) {
    R = (R << _1n ^ (R >> _7n) * _0x71n) % _256n;
    if (R & _2n)
      t ^= _1n << (_1n << BigInt(j)) - _1n;
  }
  _SHA3_IOTA.push(t);
}
var IOTAS = split(_SHA3_IOTA, true);
var SHA3_IOTA_H = IOTAS[0];
var SHA3_IOTA_L = IOTAS[1];
var rotlH = (h, l, s) => s > 32 ? rotlBH(h, l, s) : rotlSH(h, l, s);
var rotlL = (h, l, s) => s > 32 ? rotlBL(h, l, s) : rotlSL(h, l, s);
function keccakP(s, rounds = 24) {
  anumber(rounds, "rounds");
  if (rounds < 1 || rounds > 24)
    throw new Error('"rounds" expected integer 1..24');
  const B = new Uint32Array(5 * 2);
  for (let round = 24 - rounds; round < 24; round++) {
    for (let x = 0; x < 10; x++)
      B[x] = s[x] ^ s[x + 10] ^ s[x + 20] ^ s[x + 30] ^ s[x + 40];
    for (let x = 0; x < 10; x += 2) {
      const idx1 = (x + 8) % 10;
      const idx0 = (x + 2) % 10;
      const B0 = B[idx0];
      const B1 = B[idx0 + 1];
      const Th = rotlH(B0, B1, 1) ^ B[idx1];
      const Tl = rotlL(B0, B1, 1) ^ B[idx1 + 1];
      for (let y = 0; y < 50; y += 10) {
        s[x + y] ^= Th;
        s[x + y + 1] ^= Tl;
      }
    }
    let curH = s[2];
    let curL = s[3];
    for (let t = 0; t < 24; t++) {
      const shift = SHA3_ROTL[t];
      const Th = rotlH(curH, curL, shift);
      const Tl = rotlL(curH, curL, shift);
      const PI = SHA3_PI[t];
      curH = s[PI];
      curL = s[PI + 1];
      s[PI] = Th;
      s[PI + 1] = Tl;
    }
    for (let y = 0; y < 50; y += 10) {
      const b0 = s[y], b1 = s[y + 1], b2 = s[y + 2], b3 = s[y + 3];
      s[y] ^= ~s[y + 2] & s[y + 4];
      s[y + 1] ^= ~s[y + 3] & s[y + 5];
      s[y + 2] ^= ~s[y + 4] & s[y + 6];
      s[y + 3] ^= ~s[y + 5] & s[y + 7];
      s[y + 4] ^= ~s[y + 6] & s[y + 8];
      s[y + 5] ^= ~s[y + 7] & s[y + 9];
      s[y + 6] ^= ~s[y + 8] & b0;
      s[y + 7] ^= ~s[y + 9] & b1;
      s[y + 8] ^= ~b0 & b2;
      s[y + 9] ^= ~b1 & b3;
    }
    s[0] ^= SHA3_IOTA_H[round];
    s[1] ^= SHA3_IOTA_L[round];
  }
  clean(B);
}
var Keccak = class _Keccak {
  state;
  pos = 0;
  posOut = 0;
  finished = false;
  state32;
  destroyed = false;
  blockLen;
  suffix;
  outputLen;
  canXOF;
  enableXOF = false;
  rounds;
  // NOTE: we accept arguments in bytes instead of bits here.
  constructor(blockLen, suffix, outputLen, enableXOF = false, rounds = 24) {
    this.blockLen = blockLen;
    this.suffix = suffix;
    this.outputLen = outputLen;
    this.enableXOF = enableXOF;
    this.canXOF = enableXOF;
    this.rounds = rounds;
    anumber(outputLen, "outputLen");
    if (!(0 < blockLen && blockLen < 200))
      throw new Error("only keccak-f1600 function is supported");
    this.state = new Uint8Array(200);
    this.state32 = u32(this.state);
  }
  clone() {
    return this._cloneInto();
  }
  keccak() {
    swap32IfBE(this.state32);
    keccakP(this.state32, this.rounds);
    swap32IfBE(this.state32);
    this.posOut = 0;
    this.pos = 0;
  }
  update(data) {
    aexists(this);
    abytes(data);
    const { blockLen, state } = this;
    const len = data.length;
    for (let pos = 0; pos < len; ) {
      const take = Math.min(blockLen - this.pos, len - pos);
      for (let i = 0; i < take; i++)
        state[this.pos++] ^= data[pos++];
      if (this.pos === blockLen)
        this.keccak();
    }
    return this;
  }
  finish() {
    if (this.finished)
      return;
    this.finished = true;
    const { state, suffix, pos, blockLen } = this;
    state[pos] ^= suffix;
    if ((suffix & 128) !== 0 && pos === blockLen - 1)
      this.keccak();
    state[blockLen - 1] ^= 128;
    this.keccak();
  }
  writeInto(out) {
    aexists(this, false);
    abytes(out);
    this.finish();
    const bufferOut = this.state;
    const { blockLen } = this;
    for (let pos = 0, len = out.length; pos < len; ) {
      if (this.posOut >= blockLen)
        this.keccak();
      const take = Math.min(blockLen - this.posOut, len - pos);
      out.set(bufferOut.subarray(this.posOut, this.posOut + take), pos);
      this.posOut += take;
      pos += take;
    }
    return out;
  }
  xofInto(out) {
    if (!this.enableXOF)
      throw new Error("XOF is not possible for this instance");
    return this.writeInto(out);
  }
  xof(bytes) {
    anumber(bytes);
    return this.xofInto(new Uint8Array(bytes));
  }
  digestInto(out) {
    aoutput(out, this);
    if (this.finished)
      throw new Error("digest() was already called");
    this.writeInto(out.subarray(0, this.outputLen));
    this.destroy();
  }
  digest() {
    const out = new Uint8Array(this.outputLen);
    this.digestInto(out);
    return out;
  }
  destroy() {
    this.destroyed = true;
    clean(this.state);
  }
  _cloneInto(to) {
    const { blockLen, suffix, outputLen, rounds, enableXOF } = this;
    to ||= new _Keccak(blockLen, suffix, outputLen, enableXOF, rounds);
    to.blockLen = blockLen;
    to.state32.set(this.state32);
    to.pos = this.pos;
    to.posOut = this.posOut;
    to.finished = this.finished;
    to.rounds = rounds;
    to.suffix = suffix;
    to.outputLen = outputLen;
    to.enableXOF = enableXOF;
    to.canXOF = this.canXOF;
    to.destroyed = this.destroyed;
    return to;
  }
};
var genKeccak = (suffix, blockLen, outputLen, info = {}) => createHasher(() => new Keccak(blockLen, suffix, outputLen), info);
var sha3_256 = /* @__PURE__ */ genKeccak(
  6,
  136,
  32,
  /* @__PURE__ */ oidNist(8)
);
var sha3_512 = /* @__PURE__ */ genKeccak(
  6,
  72,
  64,
  /* @__PURE__ */ oidNist(10)
);
var genShake = (suffix, blockLen, outputLen, info = {}) => createHasher((opts2 = {}) => new Keccak(blockLen, suffix, opts2.dkLen === void 0 ? outputLen : opts2.dkLen, true), info);
var shake128 = /* @__PURE__ */ genShake(31, 168, 16, /* @__PURE__ */ oidNist(11));
var shake256 = /* @__PURE__ */ genShake(31, 136, 32, /* @__PURE__ */ oidNist(12));

// node_modules/@noble/curves/abstract/fft.js
function checkU32(n) {
  if (!Number.isSafeInteger(n) || n < 0 || n > 4294967295)
    throw new Error("wrong u32 integer:" + n);
  return n;
}
function isPowerOfTwo(x) {
  checkU32(x);
  return (x & x - 1) === 0 && x !== 0;
}
function reverseBits(n, bits) {
  checkU32(n);
  if (!Number.isSafeInteger(bits) || bits < 0 || bits > 32)
    throw new Error(`expected integer 0 <= bits <= 32, got ${bits}`);
  let reversed = 0;
  for (let i = 0; i < bits; i++, n >>>= 1)
    reversed = reversed << 1 | n & 1;
  return reversed >>> 0;
}
function log2(n) {
  checkU32(n);
  return 31 - Math.clz32(n);
}
function bitReversalInplace(values) {
  const n = values.length;
  if (!isPowerOfTwo(n))
    throw new Error("expected positive power-of-two length, got " + n);
  const bits = log2(n);
  for (let i = 0; i < n; i++) {
    const j = reverseBits(i, bits);
    if (i < j) {
      const tmp = values[i];
      values[i] = values[j];
      values[j] = tmp;
    }
  }
  return values;
}
var FFTCore = (F3, coreOpts) => {
  const { N: N3, roots, dit, invertButterflies = false, skipStages = 0, brp = true } = coreOpts;
  const bits = log2(N3);
  if (!isPowerOfTwo(N3))
    throw new Error("FFT: Polynomial size should be power of two");
  if (roots.length !== N3)
    throw new Error(`FFT: wrong roots length: expected ${N3}, got ${roots.length}`);
  const isDit = dit !== invertButterflies;
  isDit;
  return (values) => {
    if (values.length !== N3)
      throw new Error("FFT: wrong Polynomial length");
    if (dit && brp)
      bitReversalInplace(values);
    for (let i = 0, g = 1; i < bits - skipStages; i++) {
      const s = dit ? i + 1 + skipStages : bits - i;
      const m = 1 << s;
      const m2 = m >> 1;
      const stride = N3 >> s;
      for (let k = 0; k < N3; k += m) {
        for (let j = 0, grp = g++; j < m2; j++) {
          const rootPos = invertButterflies ? dit ? N3 - grp : grp : j * stride;
          const i0 = k + j;
          const i1 = k + j + m2;
          const omega = roots[rootPos];
          const b = values[i1];
          const a = values[i0];
          if (isDit) {
            const t = F3.mul(b, omega);
            values[i0] = F3.add(a, t);
            values[i1] = F3.sub(a, t);
          } else if (invertButterflies) {
            values[i0] = F3.add(b, a);
            values[i1] = F3.mul(F3.sub(b, a), omega);
          } else {
            values[i0] = F3.add(a, b);
            values[i1] = F3.mul(F3.sub(a, b), omega);
          }
        }
      }
    }
    if (!dit && brp)
      bitReversalInplace(values);
    return values;
  };
};

// node_modules/@noble/post-quantum/utils.js
var abytesDoc = abytes;
var randomBytes2 = randomBytes;
function equalBytes(a, b) {
  if (a.length !== b.length)
    return false;
  let diff = 0;
  for (let i = 0; i < a.length; i++)
    diff |= a[i] ^ b[i];
  return diff === 0;
}
function copyBytes2(bytes) {
  return Uint8Array.from(abytes(bytes));
}
function validateOpts(opts2) {
  if (Object.prototype.toString.call(opts2) !== "[object Object]")
    throw new TypeError("expected valid options object");
}
function validateVerOpts(opts2) {
  validateOpts(opts2);
  if (opts2.context !== void 0)
    abytes(opts2.context, void 0, "opts.context");
}
function validateSigOpts(opts2) {
  validateVerOpts(opts2);
  if (opts2.extraEntropy !== false && opts2.extraEntropy !== void 0)
    abytes(opts2.extraEntropy, void 0, "opts.extraEntropy");
}
function splitCoder(label, ...lengths) {
  const getLength = (c) => typeof c === "number" ? c : c.bytesLen;
  const bytesLen = lengths.reduce((sum, a) => sum + getLength(a), 0);
  return {
    bytesLen,
    encode: (bufs) => {
      const res = new Uint8Array(bytesLen);
      for (let i = 0, pos = 0; i < lengths.length; i++) {
        const c = lengths[i];
        const l = getLength(c);
        const b = typeof c === "number" ? bufs[i] : c.encode(bufs[i]);
        abytes(b, l, label);
        res.set(b, pos);
        if (typeof c !== "number")
          b.fill(0);
        pos += l;
      }
      return res;
    },
    decode: (buf) => {
      abytes(buf, bytesLen, label);
      const res = [];
      for (const c of lengths) {
        const l = getLength(c);
        const b = buf.subarray(0, l);
        res.push(typeof c === "number" ? b : c.decode(b));
        buf = buf.subarray(l);
      }
      return res;
    }
  };
}
function vecCoder(c, vecLen) {
  const coder = c;
  const bytesLen = vecLen * coder.bytesLen;
  return {
    bytesLen,
    encode: (u) => {
      if (u.length !== vecLen)
        throw new RangeError(`vecCoder.encode: wrong length=${u.length}. Expected: ${vecLen}`);
      const res = new Uint8Array(bytesLen);
      for (let i = 0, pos = 0; i < u.length; i++) {
        const b = coder.encode(u[i]);
        res.set(b, pos);
        b.fill(0);
        pos += b.length;
      }
      return res;
    },
    decode: (a) => {
      abytes(a, bytesLen);
      const r = [];
      for (let i = 0; i < a.length; i += coder.bytesLen)
        r.push(coder.decode(a.subarray(i, i + coder.bytesLen)));
      return r;
    }
  };
}
function cleanBytes(...list) {
  for (const t of list) {
    if (Array.isArray(t))
      for (const b of t)
        b.fill(0);
    else
      t.fill(0);
  }
}
function getMask(bits) {
  if (!Number.isSafeInteger(bits) || bits < 0 || bits > 32)
    throw new RangeError(`expected bits in [0..32], got ${bits}`);
  return bits === 32 ? 4294967295 : ~(-1 << bits) >>> 0;
}
var EMPTY = /* @__PURE__ */ Uint8Array.of();
function getMessage(msg, ctx = EMPTY) {
  abytes(msg);
  abytes(ctx);
  if (ctx.length > 255)
    throw new RangeError("context should be 255 bytes or less");
  return concatBytes(new Uint8Array([0, ctx.length]), ctx, msg);
}
var oidNistP = /* @__PURE__ */ Uint8Array.from([6, 9, 96, 134, 72, 1, 101, 3, 4, 2]);
function checkHash(hash, requiredStrength = 0) {
  if (!hash.oid || !equalBytes(hash.oid.subarray(0, 10), oidNistP))
    throw new Error("hash.oid is invalid: expected NIST hash");
  const collisionResistance = hash.outputLen * 8 / 2;
  if (requiredStrength > collisionResistance) {
    throw new Error("Pre-hash security strength too low: " + collisionResistance + ", required: " + requiredStrength);
  }
}
function getMessagePrehash(hash, msg, ctx = EMPTY) {
  abytes(msg);
  abytes(ctx);
  if (ctx.length > 255)
    throw new RangeError("context should be 255 bytes or less");
  const hashed = hash(msg);
  return concatBytes(new Uint8Array([1, ctx.length]), ctx, hash.oid, hashed);
}

// node_modules/@noble/post-quantum/_crystals.js
var genCrystals = (opts2) => {
  const { newPoly: newPoly2, N: N3, Q: Q3, F: F3, ROOT_OF_UNITY: ROOT_OF_UNITY3, brvBits, isKyber } = opts2;
  const mod = (a, modulo = Q3) => {
    const result = a % modulo | 0;
    return (result >= 0 ? result | 0 : modulo + result | 0) | 0;
  };
  const smod = (a, modulo = Q3) => {
    const r = mod(a, modulo) | 0;
    return (r > modulo >> 1 ? r - modulo | 0 : r) | 0;
  };
  function getZettas() {
    const out = newPoly2(N3);
    for (let i = 0; i < N3; i++) {
      const b = reverseBits(i, brvBits);
      const p = BigInt(ROOT_OF_UNITY3) ** BigInt(b) % BigInt(Q3);
      out[i] = Number(p) | 0;
    }
    return out;
  }
  const nttZetas = getZettas();
  const field = {
    add: (a, b) => mod((a | 0) + (b | 0)) | 0,
    sub: (a, b) => mod((a | 0) - (b | 0)) | 0,
    mul: (a, b) => mod((a | 0) * (b | 0)) | 0,
    inv: (_a) => {
      throw new Error("not implemented");
    }
  };
  const nttOpts = {
    N: N3,
    roots: nttZetas,
    invertButterflies: true,
    skipStages: isKyber ? 1 : 0,
    brp: false
  };
  const dif = FFTCore(field, { dit: false, ...nttOpts });
  const dit = FFTCore(field, { dit: true, ...nttOpts });
  const NTT = {
    encode: (r) => {
      return dif(r);
    },
    decode: (r) => {
      dit(r);
      for (let i = 0; i < r.length; i++)
        r[i] = mod(F3 * r[i]);
      return r;
    }
  };
  const bitsCoder = (d, c) => {
    const mask = getMask(d);
    const bytesLen = d * (N3 / 8);
    return {
      bytesLen,
      encode: (poly_) => {
        const poly = poly_;
        const r = new Uint8Array(bytesLen);
        for (let i = 0, buf = 0, bufLen = 0, pos = 0; i < poly.length; i++) {
          buf |= (c.encode(poly[i]) & mask) << bufLen;
          bufLen += d;
          for (; bufLen >= 8; bufLen -= 8, buf >>= 8)
            r[pos++] = buf & getMask(bufLen);
        }
        return r;
      },
      decode: (bytes) => {
        const r = newPoly2(N3);
        for (let i = 0, buf = 0, bufLen = 0, pos = 0; i < bytes.length; i++) {
          buf |= bytes[i] << bufLen;
          bufLen += 8;
          for (; bufLen >= d; bufLen -= d, buf >>= d)
            r[pos++] = c.decode(buf & mask);
        }
        return r;
      }
    };
  };
  return {
    mod,
    smod,
    nttZetas,
    NTT: {
      encode: (r) => NTT.encode(r),
      decode: (r) => NTT.decode(r)
    },
    bitsCoder
  };
};
var createXofShake = (shake) => (seed, blockLen) => {
  if (!blockLen)
    blockLen = shake.blockLen;
  const _seed = new Uint8Array(seed.length + 2);
  _seed.set(seed);
  const seedLen = seed.length;
  const buf = new Uint8Array(blockLen);
  let h = shake.create({});
  let calls = 0;
  let xofs = 0;
  return {
    stats: () => ({ calls, xofs }),
    get: (x, y) => {
      _seed[seedLen + 0] = x;
      _seed[seedLen + 1] = y;
      h.destroy();
      h = shake.create({}).update(_seed);
      calls++;
      return () => {
        xofs++;
        return h.xofInto(buf);
      };
    },
    clean: () => {
      h.destroy();
      cleanBytes(buf, _seed);
    }
  };
};
var XOF128 = /* @__PURE__ */ createXofShake(shake128);
var XOF256 = /* @__PURE__ */ createXofShake(shake256);

// node_modules/@noble/post-quantum/ml-dsa.js
function validateInternalOpts(opts2) {
  validateOpts(opts2);
  if (opts2.externalMu !== void 0)
    abool(opts2.externalMu, "opts.externalMu");
}
var N = 256;
var Q = 8380417;
var ROOT_OF_UNITY = 1753;
var F = 8347681;
var D = 13;
var GAMMA2_1 = Math.floor((Q - 1) / 88) | 0;
var GAMMA2_2 = Math.floor((Q - 1) / 32) | 0;
var PARAMS = /* @__PURE__ */ (() => Object.freeze({
  2: Object.freeze({
    K: 4,
    L: 4,
    D,
    GAMMA1: 2 ** 17,
    GAMMA2: GAMMA2_1,
    TAU: 39,
    ETA: 2,
    OMEGA: 80
  }),
  3: Object.freeze({
    K: 6,
    L: 5,
    D,
    GAMMA1: 2 ** 19,
    GAMMA2: GAMMA2_2,
    TAU: 49,
    ETA: 4,
    OMEGA: 55
  }),
  5: Object.freeze({
    K: 8,
    L: 7,
    D,
    GAMMA1: 2 ** 19,
    GAMMA2: GAMMA2_2,
    TAU: 60,
    ETA: 2,
    OMEGA: 75
  })
}))();
var newPoly = (n) => new Int32Array(n);
var crystals = /* @__PURE__ */ genCrystals({
  N,
  Q,
  F,
  ROOT_OF_UNITY,
  newPoly,
  isKyber: false,
  brvBits: 8
});
var id = (n) => n;
var polyCoder = (d, compress3 = id, verify = id) => crystals.bitsCoder(d, {
  encode: (i) => compress3(verify(i)),
  decode: (i) => verify(compress3(i))
});
var polyAdd = (a_, b_) => {
  const a = a_;
  const b = b_;
  for (let i = 0; i < a.length; i++)
    a[i] = crystals.mod(a[i] + b[i]);
  return a;
};
var polySub = (a_, b_) => {
  const a = a_;
  const b = b_;
  for (let i = 0; i < a.length; i++)
    a[i] = crystals.mod(a[i] - b[i]);
  return a;
};
var polyShiftl = (p_) => {
  const p = p_;
  for (let i = 0; i < N; i++)
    p[i] <<= D;
  return p;
};
var polyChknorm = (p_, B) => {
  const p = p_;
  for (let i = 0; i < N; i++)
    if (Math.abs(crystals.smod(p[i])) >= B)
      return true;
  return false;
};
var MultiplyNTTs = (a_, b_) => {
  const a = a_;
  const b = b_;
  const c = newPoly(N);
  for (let i = 0; i < a.length; i++)
    c[i] = crystals.mod(a[i] * b[i]);
  return c;
};
function RejNTTPoly(xof_) {
  const xof = xof_;
  const r = newPoly(N);
  for (let j = 0; j < N; ) {
    const b = xof();
    if (b.length % 3)
      throw new Error("RejNTTPoly: unaligned block");
    for (let i = 0; j < N && i <= b.length - 3; i += 3) {
      const t = (b[i + 0] | b[i + 1] << 8 | b[i + 2] << 16) & 8388607;
      if (t < Q)
        r[j++] = t;
    }
  }
  return r;
}
function getDilithium(opts_) {
  const opts2 = opts_;
  const { K, L, GAMMA1, GAMMA2, TAU, ETA, OMEGA } = opts2;
  const { CRH_BYTES, TR_BYTES, C_TILDE_BYTES, XOF128: XOF1282, XOF256: XOF2562, securityLevel } = opts2;
  if (![2, 4].includes(ETA))
    throw new Error("Wrong ETA");
  if (![1 << 17, 1 << 19].includes(GAMMA1))
    throw new Error("Wrong GAMMA1");
  if (![GAMMA2_1, GAMMA2_2].includes(GAMMA2))
    throw new Error("Wrong GAMMA2");
  const BETA = TAU * ETA;
  const decompose = (r) => {
    const rPlus = crystals.mod(r);
    const r0 = crystals.smod(rPlus, 2 * GAMMA2) | 0;
    if (rPlus - r0 === Q - 1)
      return { r1: 0 | 0, r0: r0 - 1 | 0 };
    const r1 = Math.floor((rPlus - r0) / (2 * GAMMA2)) | 0;
    return { r1, r0 };
  };
  const HighBits = (r) => decompose(r).r1;
  const LowBits = (r) => decompose(r).r0;
  const MakeHint = (z, r) => {
    const res0 = z <= GAMMA2 || z > Q - GAMMA2 || z === Q - GAMMA2 && r === 0 ? 0 : 1;
    return res0;
  };
  const UseHint = (h, r) => {
    const m = Math.floor((Q - 1) / (2 * GAMMA2));
    const { r1, r0 } = decompose(r);
    if (h === 1)
      return r0 > 0 ? crystals.mod(r1 + 1, m) | 0 : crystals.mod(r1 - 1, m) | 0;
    return r1 | 0;
  };
  const Power2Round = (r) => {
    const rPlus = crystals.mod(r);
    const r0 = crystals.smod(rPlus, 2 ** D) | 0;
    return { r1: Math.floor((rPlus - r0) / 2 ** D) | 0, r0 };
  };
  const hintCoder = {
    bytesLen: OMEGA + K,
    encode: (h_) => {
      const h = h_;
      if (h === false)
        throw new Error("hint.encode: hint is false");
      const res = new Uint8Array(OMEGA + K);
      for (let i = 0, k = 0; i < K; i++) {
        for (let j = 0; j < N; j++)
          if (h[i][j] !== 0)
            res[k++] = j;
        res[OMEGA + i] = k;
      }
      return res;
    },
    decode: (buf) => {
      const h = [];
      let k = 0;
      for (let i = 0; i < K; i++) {
        const hi = newPoly(N);
        if (buf[OMEGA + i] < k || buf[OMEGA + i] > OMEGA)
          return false;
        for (let j = k; j < buf[OMEGA + i]; j++) {
          if (j > k && buf[j] <= buf[j - 1])
            return false;
          hi[buf[j]] = 1;
        }
        k = buf[OMEGA + i];
        h.push(hi);
      }
      for (let j = k; j < OMEGA; j++)
        if (buf[j] !== 0)
          return false;
      return h;
    }
  };
  const ETACoder = polyCoder(ETA === 2 ? 3 : 4, (i) => ETA - i, (i) => {
    if (!(-ETA <= i && i <= ETA))
      throw new Error(`malformed key s1/s3 ${i} outside of ETA range [${-ETA}, ${ETA}]`);
    return i;
  });
  const T0Coder = polyCoder(13, (i) => (1 << D - 1) - i);
  const T1Coder = polyCoder(10);
  const ZCoder = polyCoder(GAMMA1 === 1 << 17 ? 18 : 20, (i) => crystals.smod(GAMMA1 - i));
  const W1Coder = polyCoder(GAMMA2 === GAMMA2_1 ? 6 : 4);
  const W1Vec = vecCoder(W1Coder, K);
  const publicCoder = splitCoder("publicKey", 32, vecCoder(T1Coder, K));
  const secretCoder = splitCoder("secretKey", 32, 32, TR_BYTES, vecCoder(ETACoder, L), vecCoder(ETACoder, K), vecCoder(T0Coder, K));
  const sigCoder = splitCoder("signature", C_TILDE_BYTES, vecCoder(ZCoder, L), hintCoder);
  const CoefFromHalfByte = ETA === 2 ? (n) => n < 15 ? 2 - n % 5 : false : (n) => n < 9 ? 4 - n : false;
  function RejBoundedPoly(xof_) {
    const xof = xof_;
    const r = newPoly(N);
    for (let j = 0; j < N; ) {
      const b = xof();
      for (let i = 0; j < N && i < b.length; i += 1) {
        const d1 = CoefFromHalfByte(b[i] & 15);
        const d2 = CoefFromHalfByte(b[i] >> 4 & 15);
        if (d1 !== false)
          r[j++] = d1;
        if (j < N && d2 !== false)
          r[j++] = d2;
      }
    }
    return r;
  }
  const SampleInBall = (seed) => {
    const pre = newPoly(N);
    const s = shake256.create({}).update(seed);
    const buf = new Uint8Array(shake256.blockLen);
    s.xofInto(buf);
    const masks = buf.slice(0, 8);
    for (let i = N - TAU, pos = 8, maskPos = 0, maskBit = 0; i < N; i++) {
      let b = i + 1;
      for (; b > i; ) {
        b = buf[pos++];
        if (pos < shake256.blockLen)
          continue;
        s.xofInto(buf);
        pos = 0;
      }
      pre[i] = pre[b];
      pre[b] = 1 - ((masks[maskPos] >> maskBit++ & 1) << 1);
      if (maskBit >= 8) {
        maskPos++;
        maskBit = 0;
      }
    }
    return pre;
  };
  const polyPowerRound = (p_) => {
    const p = p_;
    const res0 = newPoly(N);
    const res1 = newPoly(N);
    for (let i = 0; i < p.length; i++) {
      const { r0, r1 } = Power2Round(p[i]);
      res0[i] = r0;
      res1[i] = r1;
    }
    return { r0: res0, r1: res1 };
  };
  const polyUseHint = (u_, h_) => {
    const u = u_;
    const h = h_;
    for (let i = 0; i < N; i++)
      u[i] = UseHint(h[i], u[i]);
    return u;
  };
  const polyMakeHint = (a_, b_) => {
    const a = a_;
    const b = b_;
    const v = newPoly(N);
    let cnt = 0;
    for (let i = 0; i < N; i++) {
      const h = MakeHint(a[i], b[i]);
      v[i] = h;
      cnt += h;
    }
    return { v, cnt };
  };
  const signRandBytes = 32;
  const seedCoder = splitCoder("seed", 32, 64, 32);
  const internal = Object.freeze({
    info: Object.freeze({ type: "internal-ml-dsa" }),
    lengths: Object.freeze({
      secretKey: secretCoder.bytesLen,
      publicKey: publicCoder.bytesLen,
      seed: 32,
      signature: sigCoder.bytesLen,
      signRand: signRandBytes
    }),
    keygen: (seed) => {
      const seedDst = new Uint8Array(32 + 2);
      const randSeed = seed === void 0;
      if (randSeed)
        seed = randomBytes2(32);
      abytesDoc(seed, 32, "seed");
      seedDst.set(seed);
      if (randSeed)
        cleanBytes(seed);
      seedDst[32] = K;
      seedDst[33] = L;
      const [rho, rhoPrime, K_] = seedCoder.decode(shake256(seedDst, { dkLen: seedCoder.bytesLen }));
      const xofPrime = XOF2562(rhoPrime);
      const s1 = [];
      for (let i = 0; i < L; i++)
        s1.push(RejBoundedPoly(xofPrime.get(i & 255, i >> 8 & 255)));
      const s2 = [];
      for (let i = L; i < L + K; i++)
        s2.push(RejBoundedPoly(xofPrime.get(i & 255, i >> 8 & 255)));
      const s1Hat = s1.map((i) => crystals.NTT.encode(i.slice()));
      const t0 = [];
      const t1 = [];
      const xof = XOF1282(rho);
      const t = newPoly(N);
      for (let i = 0; i < K; i++) {
        cleanBytes(t);
        for (let j = 0; j < L; j++) {
          const aij = RejNTTPoly(xof.get(j, i));
          polyAdd(t, MultiplyNTTs(aij, s1Hat[j]));
        }
        crystals.NTT.decode(t);
        const { r0, r1 } = polyPowerRound(polyAdd(t, s2[i]));
        t0.push(r0);
        t1.push(r1);
      }
      const publicKey = publicCoder.encode([rho, t1]);
      const tr = shake256(publicKey, { dkLen: TR_BYTES });
      const secretKey = secretCoder.encode([rho, K_, tr, s1, s2, t0]);
      xof.clean();
      xofPrime.clean();
      cleanBytes(rho, rhoPrime, K_, s1, s2, s1Hat, t, t0, t1, tr, seedDst);
      return {
        publicKey,
        secretKey
      };
    },
    getPublicKey: (secretKey) => {
      const [rho, _K, _tr, s1, s2, _t0] = secretCoder.decode(secretKey);
      const xof = XOF1282(rho);
      const s1Hat = s1.map((p) => crystals.NTT.encode(p.slice()));
      const t1 = [];
      const tmp = newPoly(N);
      for (let i = 0; i < K; i++) {
        tmp.fill(0);
        for (let j = 0; j < L; j++) {
          const aij = RejNTTPoly(xof.get(j, i));
          polyAdd(tmp, MultiplyNTTs(aij, s1Hat[j]));
        }
        crystals.NTT.decode(tmp);
        polyAdd(tmp, s2[i]);
        const { r1 } = polyPowerRound(tmp);
        t1.push(r1);
      }
      xof.clean();
      cleanBytes(tmp, s1Hat, _t0, s1, s2);
      return publicCoder.encode([rho, t1]);
    },
    // NOTE: random is optional.
    sign: (msg, secretKey, opts3 = {}) => {
      validateSigOpts(opts3);
      validateInternalOpts(opts3);
      let { extraEntropy: random, externalMu = false } = opts3;
      const [rho, _K, tr, s1, s2, t0] = secretCoder.decode(secretKey);
      const A = [];
      const xof = XOF1282(rho);
      for (let i = 0; i < K; i++) {
        const pv = [];
        for (let j = 0; j < L; j++)
          pv.push(RejNTTPoly(xof.get(j, i)));
        A.push(pv);
      }
      xof.clean();
      for (let i = 0; i < L; i++)
        crystals.NTT.encode(s1[i]);
      for (let i = 0; i < K; i++) {
        crystals.NTT.encode(s2[i]);
        crystals.NTT.encode(t0[i]);
      }
      const mu = externalMu ? msg : (
        // 6: µ ← H(tr||M, 512)
        //    ▷ Compute message representative µ
        shake256.create({ dkLen: CRH_BYTES }).update(tr).update(msg).digest()
      );
      const rnd = random === false ? new Uint8Array(32) : random === void 0 ? randomBytes2(signRandBytes) : random;
      abytesDoc(rnd, 32, "extraEntropy");
      const rhoprime = shake256.create({ dkLen: CRH_BYTES }).update(_K).update(rnd).update(mu).digest();
      abytesDoc(rhoprime, CRH_BYTES);
      const x256 = XOF2562(rhoprime, ZCoder.bytesLen);
      main_loop: for (let kappa = 0; ; ) {
        const y = [];
        for (let i = 0; i < L; i++, kappa++)
          y.push(ZCoder.decode(x256.get(kappa & 255, kappa >> 8)()));
        const z = y.map((i) => crystals.NTT.encode(i.slice()));
        const w = [];
        for (let i = 0; i < K; i++) {
          const wi = newPoly(N);
          for (let j = 0; j < L; j++)
            polyAdd(wi, MultiplyNTTs(A[i][j], z[j]));
          crystals.NTT.decode(wi);
          w.push(wi);
        }
        const w1 = w.map((j) => j.map(HighBits));
        const cTilde = shake256.create({ dkLen: C_TILDE_BYTES }).update(mu).update(W1Vec.encode(w1)).digest();
        const cHat = crystals.NTT.encode(SampleInBall(cTilde));
        const cs1 = s1.map((i) => MultiplyNTTs(i, cHat));
        for (let i = 0; i < L; i++) {
          polyAdd(crystals.NTT.decode(cs1[i]), y[i]);
          if (polyChknorm(cs1[i], GAMMA1 - BETA))
            continue main_loop;
        }
        let cnt = 0;
        const h = [];
        for (let i = 0; i < K; i++) {
          const cs2 = crystals.NTT.decode(MultiplyNTTs(s2[i], cHat));
          const r0 = polySub(w[i], cs2).map(LowBits);
          if (polyChknorm(r0, GAMMA2 - BETA))
            continue main_loop;
          const ct0 = crystals.NTT.decode(MultiplyNTTs(t0[i], cHat));
          if (polyChknorm(ct0, GAMMA2))
            continue main_loop;
          polyAdd(r0, ct0);
          const hint = polyMakeHint(r0, w1[i]);
          h.push(hint.v);
          cnt += hint.cnt;
        }
        if (cnt > OMEGA)
          continue;
        x256.clean();
        const res = sigCoder.encode([cTilde, cs1, h]);
        cleanBytes(cTilde, cs1, h, cHat, w1, w, z, y, rhoprime, s1, s2, t0, ...A);
        if (!externalMu)
          cleanBytes(mu);
        return res;
      }
      throw new Error("Unreachable code path reached, report this error");
    },
    verify: (sig, msg, publicKey, opts3 = {}) => {
      validateInternalOpts(opts3);
      const { externalMu = false } = opts3;
      const [rho, t1] = publicCoder.decode(publicKey);
      const tr = shake256(publicKey, { dkLen: TR_BYTES });
      if (sig.length !== sigCoder.bytesLen)
        return false;
      const [cTilde, z, h] = sigCoder.decode(sig);
      if (h === false)
        return false;
      for (let i = 0; i < L; i++)
        if (polyChknorm(z[i], GAMMA1 - BETA))
          return false;
      const mu = externalMu ? msg : (
        // 7: µ ← H(tr||M, 512)
        shake256.create({ dkLen: CRH_BYTES }).update(tr).update(msg).digest()
      );
      const c = crystals.NTT.encode(SampleInBall(cTilde));
      const zNtt = z.map((i) => i.slice());
      for (let i = 0; i < L; i++)
        crystals.NTT.encode(zNtt[i]);
      const wTick1 = [];
      const xof = XOF1282(rho);
      for (let i = 0; i < K; i++) {
        const ct12d = MultiplyNTTs(crystals.NTT.encode(polyShiftl(t1[i])), c);
        const Az = newPoly(N);
        for (let j = 0; j < L; j++) {
          const aij = RejNTTPoly(xof.get(j, i));
          polyAdd(Az, MultiplyNTTs(aij, zNtt[j]));
        }
        const wApprox = crystals.NTT.decode(polySub(Az, ct12d));
        wTick1.push(polyUseHint(wApprox, h[i]));
      }
      xof.clean();
      const c2 = shake256.create({ dkLen: C_TILDE_BYTES }).update(mu).update(W1Vec.encode(wTick1)).digest();
      for (const t of h) {
        const sum = t.reduce((acc, i) => acc + i, 0);
        if (!(sum <= OMEGA))
          return false;
      }
      for (const t of z)
        if (polyChknorm(t, GAMMA1 - BETA))
          return false;
      return equalBytes(cTilde, c2);
    }
  });
  return Object.freeze({
    info: Object.freeze({ type: "ml-dsa" }),
    internal,
    securityLevel,
    keygen: internal.keygen,
    lengths: internal.lengths,
    getPublicKey: internal.getPublicKey,
    sign: (msg, secretKey, opts3 = {}) => {
      validateSigOpts(opts3);
      const M = getMessage(msg, opts3.context);
      const res = internal.sign(M, secretKey, opts3);
      cleanBytes(M);
      return res;
    },
    verify: (sig, msg, publicKey, opts3 = {}) => {
      validateVerOpts(opts3);
      return internal.verify(sig, getMessage(msg, opts3.context), publicKey);
    },
    prehash: (hash) => {
      checkHash(hash, securityLevel);
      return Object.freeze({
        info: Object.freeze({ type: "hashml-dsa" }),
        securityLevel,
        lengths: internal.lengths,
        keygen: internal.keygen,
        getPublicKey: internal.getPublicKey,
        sign: (msg, secretKey, opts3 = {}) => {
          validateSigOpts(opts3);
          const M = getMessagePrehash(hash, msg, opts3.context);
          const res = internal.sign(M, secretKey, opts3);
          cleanBytes(M);
          return res;
        },
        verify: (sig, msg, publicKey, opts3 = {}) => {
          validateVerOpts(opts3);
          return internal.verify(sig, getMessagePrehash(hash, msg, opts3.context), publicKey);
        }
      });
    }
  });
}
var ml_dsa65 = /* @__PURE__ */ (() => getDilithium({
  ...PARAMS[3],
  CRH_BYTES: 64,
  TR_BYTES: 64,
  C_TILDE_BYTES: 48,
  XOF128,
  XOF256,
  securityLevel: 192
}))();

// node_modules/@noble/post-quantum/ml-kem.js
var N2 = 256;
var Q2 = 3329;
var F2 = 3303;
var ROOT_OF_UNITY2 = 17;
var crystals2 = /* @__PURE__ */ genCrystals({
  N: N2,
  Q: Q2,
  F: F2,
  ROOT_OF_UNITY: ROOT_OF_UNITY2,
  newPoly: (n) => new Uint16Array(n),
  brvBits: 7,
  isKyber: true
});
var PARAMS2 = /* @__PURE__ */ (() => Object.freeze({
  512: Object.freeze({ N: N2, Q: Q2, K: 2, ETA1: 3, ETA2: 2, du: 10, dv: 4, RBGstrength: 128 }),
  768: Object.freeze({ N: N2, Q: Q2, K: 3, ETA1: 2, ETA2: 2, du: 10, dv: 4, RBGstrength: 192 }),
  1024: Object.freeze({ N: N2, Q: Q2, K: 4, ETA1: 2, ETA2: 2, du: 11, dv: 5, RBGstrength: 256 })
}))();
var compress = (d) => {
  if (d >= 12)
    return { encode: (i) => i, decode: (i) => i >= Q2 ? i - Q2 : i };
  const a = 2 ** (d - 1);
  return {
    // This only matches standalone Compress_d after bitsCoder masks the result into Z_(2^d).
    encode: (i) => ((i << d) + Q2 / 2) / Q2,
    // const decompress = (i: number) => round((Q / 2 ** d) * i);
    decode: (i) => i * Q2 + a >>> d
  };
};
var byteCoder = (d) => crystals2.bitsCoder(d, d === 12 ? { encode: (i) => i, decode: (i) => i >= Q2 ? i - Q2 : i } : { encode: (i) => i, decode: (i) => i });
var polyCoder2 = (d) => d === 12 ? byteCoder(12) : crystals2.bitsCoder(d, compress(d));
function polyAdd2(a_, b_) {
  const a = a_;
  const b = b_;
  for (let i = 0; i < N2; i++)
    a[i] = crystals2.mod(a[i] + b[i]);
}
function polySub2(a_, b_) {
  const a = a_;
  const b = b_;
  for (let i = 0; i < N2; i++)
    a[i] = crystals2.mod(a[i] - b[i]);
}
function BaseCaseMultiply(a0, a1, b0, b1, zeta) {
  const c0 = crystals2.mod(a1 * b1 * zeta + a0 * b0);
  const c1 = crystals2.mod(a0 * b1 + a1 * b0);
  return { c0, c1 };
}
function MultiplyNTTs2(f_, g_) {
  const f = f_;
  const g = g_;
  for (let i = 0; i < N2 / 2; i++) {
    let z = crystals2.nttZetas[64 + (i >> 1)];
    if (i & 1)
      z = -z;
    const { c0, c1 } = BaseCaseMultiply(f[2 * i + 0], f[2 * i + 1], g[2 * i + 0], g[2 * i + 1], z);
    f[2 * i + 0] = c0;
    f[2 * i + 1] = c1;
  }
  return f;
}
function SampleNTT(xof_) {
  const xof = xof_;
  const r = new Uint16Array(N2);
  for (let j = 0; j < N2; ) {
    const b = xof();
    if (b.length % 3)
      throw new Error("SampleNTT: unaligned block");
    for (let i = 0; j < N2 && i + 3 <= b.length; i += 3) {
      const d1 = (b[i + 0] >> 0 | b[i + 1] << 8) & 4095;
      const d2 = (b[i + 1] >> 4 | b[i + 2] << 4) & 4095;
      if (d1 < Q2)
        r[j++] = d1;
      if (j < N2 && d2 < Q2)
        r[j++] = d2;
    }
  }
  return r;
}
var sampleCBDBytes = (buf, eta) => {
  const r = new Uint16Array(N2);
  const b32 = u32(buf);
  swap32IfBE(b32);
  let len = 0;
  for (let i = 0, p = 0, bb = 0, t0 = 0; i < b32.length; i++) {
    let b = b32[i];
    for (let j = 0; j < 32; j++) {
      bb += b & 1;
      b >>= 1;
      len += 1;
      if (len === eta) {
        t0 = bb;
        bb = 0;
      } else if (len === 2 * eta) {
        r[p++] = crystals2.mod(t0 - bb);
        bb = 0;
        len = 0;
      }
    }
  }
  swap32IfBE(b32);
  if (len)
    throw new Error(`sampleCBD: leftover bits: ${len}`);
  return r;
};
function sampleCBD(PRF_, seed, nonce, eta) {
  const PRF = PRF_;
  return sampleCBDBytes(PRF(eta * N2 / 4, seed, nonce), eta);
}
var genKPKE = (opts_) => {
  const opts2 = opts_;
  const { K, PRF, XOF, HASH512, ETA1, ETA2, du, dv } = opts2;
  const poly1 = polyCoder2(1);
  const polyV = polyCoder2(dv);
  const polyU = polyCoder2(du);
  const publicCoder = splitCoder("publicKey", vecCoder(polyCoder2(12), K), 32);
  const secretCoder = vecCoder(polyCoder2(12), K);
  const cipherCoder = splitCoder("ciphertext", vecCoder(polyU, K), polyV);
  const seedCoder = splitCoder("seed", 32, 32);
  return {
    secretCoder,
    lengths: {
      secretKey: secretCoder.bytesLen,
      publicKey: publicCoder.bytesLen,
      cipherText: cipherCoder.bytesLen
    },
    keygen: (seed) => {
      abytesDoc(seed, 32, "seed");
      const seedDst = new Uint8Array(33);
      seedDst.set(seed);
      seedDst[32] = K;
      const seedHash = HASH512(seedDst);
      const [rho, sigma] = seedCoder.decode(seedHash);
      const sHat = [];
      const tHat = [];
      for (let i = 0; i < K; i++)
        sHat.push(crystals2.NTT.encode(sampleCBD(PRF, sigma, i, ETA1)));
      const x = XOF(rho);
      for (let i = 0; i < K; i++) {
        const e = crystals2.NTT.encode(sampleCBD(PRF, sigma, K + i, ETA1));
        for (let j = 0; j < K; j++) {
          const aji = SampleNTT(x.get(j, i));
          polyAdd2(e, MultiplyNTTs2(aji, sHat[j]));
        }
        tHat.push(e);
      }
      x.clean();
      const res = {
        publicKey: publicCoder.encode([tHat, rho]),
        secretKey: secretCoder.encode(sHat)
      };
      cleanBytes(rho, sigma, sHat, tHat, seedDst, seedHash);
      return res;
    },
    encrypt: (publicKey, msg, seed) => {
      const [tHat, rho] = publicCoder.decode(publicKey);
      const rHat = [];
      for (let i = 0; i < K; i++)
        rHat.push(crystals2.NTT.encode(sampleCBD(PRF, seed, i, ETA1)));
      const x = XOF(rho);
      const tmp2 = new Uint16Array(N2);
      const u = [];
      for (let i = 0; i < K; i++) {
        const e1 = sampleCBD(PRF, seed, K + i, ETA2);
        const tmp = new Uint16Array(N2);
        for (let j = 0; j < K; j++) {
          const aij = SampleNTT(x.get(i, j));
          polyAdd2(tmp, MultiplyNTTs2(aij, rHat[j]));
        }
        polyAdd2(e1, crystals2.NTT.decode(tmp));
        u.push(e1);
        polyAdd2(tmp2, MultiplyNTTs2(tHat[i], rHat[i]));
        cleanBytes(tmp);
      }
      x.clean();
      const e2 = sampleCBD(PRF, seed, 2 * K, ETA2);
      polyAdd2(e2, crystals2.NTT.decode(tmp2));
      const v = poly1.decode(msg);
      polyAdd2(v, e2);
      cleanBytes(tHat, rHat, tmp2, e2);
      return cipherCoder.encode([u, v]);
    },
    decrypt: (cipherText, privateKey) => {
      const [u, v] = cipherCoder.decode(cipherText);
      const sk = secretCoder.decode(privateKey);
      const tmp = new Uint16Array(N2);
      for (let i = 0; i < K; i++)
        polyAdd2(tmp, MultiplyNTTs2(sk[i], crystals2.NTT.encode(u[i])));
      polySub2(v, crystals2.NTT.decode(tmp));
      cleanBytes(tmp, sk, u);
      return poly1.encode(v);
    }
  };
};
function createKyber(opts2) {
  const rawOpts = opts2;
  const KPKE = genKPKE(rawOpts);
  const { HASH256, HASH512, KDF } = rawOpts;
  const { secretCoder: KPKESecretCoder, lengths } = KPKE;
  const secretCoder = splitCoder("secretKey", lengths.secretKey, lengths.publicKey, 32, 32);
  const msgLen = 32;
  const seedLen = 64;
  const kemLengths = Object.freeze({
    ...lengths,
    seed: 64,
    msg: msgLen,
    msgRand: msgLen,
    secretKey: secretCoder.bytesLen
  });
  return Object.freeze({
    info: Object.freeze({ type: "ml-kem" }),
    lengths: kemLengths,
    keygen: (seed = randomBytes2(seedLen)) => {
      abytesDoc(seed, seedLen, "seed");
      const { publicKey, secretKey: sk } = KPKE.keygen(seed.subarray(0, 32));
      const publicKeyHash = HASH256(publicKey);
      const secretKey = secretCoder.encode([sk, publicKey, publicKeyHash, seed.subarray(32)]);
      cleanBytes(sk, publicKeyHash);
      return {
        publicKey,
        secretKey
      };
    },
    getPublicKey: (secretKey) => {
      const [_sk, publicKey, _publicKeyHash, _z] = secretCoder.decode(secretKey);
      return Uint8Array.from(publicKey);
    },
    encapsulate: (publicKey, msg = randomBytes2(msgLen)) => {
      abytesDoc(publicKey, lengths.publicKey, "publicKey");
      abytesDoc(msg, msgLen, "message");
      const eke = publicKey.subarray(0, 384 * opts2.K);
      const ek = KPKESecretCoder.encode(KPKESecretCoder.decode(copyBytes2(eke)));
      if (!equalBytes(ek, eke)) {
        cleanBytes(ek);
        throw new Error("ML-KEM.encapsulate: wrong publicKey modulus");
      }
      cleanBytes(ek);
      const kr = HASH512.create().update(msg).update(HASH256(publicKey)).digest();
      const cipherText = KPKE.encrypt(publicKey, msg, kr.subarray(32, 64));
      cleanBytes(kr.subarray(32));
      return {
        cipherText,
        sharedSecret: kr.subarray(0, 32)
      };
    },
    decapsulate: (cipherText, secretKey) => {
      abytesDoc(secretKey, secretCoder.bytesLen, "secretKey");
      abytesDoc(cipherText, lengths.cipherText, "cipherText");
      const k768 = secretCoder.bytesLen - 96;
      const start = k768 + 32;
      const test = HASH256(secretKey.subarray(k768 / 2, start));
      if (!equalBytes(test, secretKey.subarray(start, start + 32)))
        throw new Error("invalid secretKey: hash check failed");
      const [sk, publicKey, publicKeyHash, z] = secretCoder.decode(secretKey);
      const msg = KPKE.decrypt(cipherText, sk);
      const kr = HASH512.create().update(msg).update(publicKeyHash).digest();
      const Khat = kr.subarray(0, 32);
      const cipherText2 = KPKE.encrypt(publicKey, msg, kr.subarray(32, 64));
      const isValid = equalBytes(cipherText, cipherText2);
      const Kbar = KDF.create({ dkLen: 32 }).update(z).update(cipherText).digest();
      cleanBytes(msg, cipherText2, !isValid ? Khat : Kbar);
      return isValid ? Khat : Kbar;
    }
  });
}
function shakePRF(dkLen, key, nonce) {
  return shake256.create({ dkLen }).update(key).update(new Uint8Array([nonce])).digest();
}
var opts = /* @__PURE__ */ (() => ({
  HASH256: sha3_256,
  HASH512: sha3_512,
  KDF: shake256,
  XOF: XOF128,
  PRF: shakePRF
}))();
var mk = (params) => createKyber({
  ...opts,
  ...params
});
var ml_kem768 = /* @__PURE__ */ (() => mk(PARAMS2[768]))();

// node_modules/@noble/hashes/_md.js
var SHA256_IV = /* @__PURE__ */ Uint32Array.from([
  1779033703,
  3144134277,
  1013904242,
  2773480762,
  1359893119,
  2600822924,
  528734635,
  1541459225
]);

// node_modules/@noble/hashes/_blake.js
function G1s(a, b, c, d, x) {
  a = a + b + x | 0;
  d = rotr(d ^ a, 16);
  c = c + d | 0;
  b = rotr(b ^ c, 12);
  return { a, b, c, d };
}
function G2s(a, b, c, d, x) {
  a = a + b + x | 0;
  d = rotr(d ^ a, 8);
  c = c + d | 0;
  b = rotr(b ^ c, 7);
  return { a, b, c, d };
}

// node_modules/@noble/hashes/blake2.js
var _BLAKE2 = class {
  buffer;
  buffer32;
  finished = false;
  destroyed = false;
  length = 0;
  pos = 0;
  blockLen;
  outputLen;
  canXOF = false;
  constructor(blockLen, outputLen) {
    anumber(blockLen);
    anumber(outputLen);
    this.blockLen = blockLen;
    this.outputLen = outputLen;
    this.buffer = new Uint8Array(blockLen);
    this.buffer32 = u32(this.buffer);
  }
  update(data) {
    aexists(this);
    abytes(data);
    const { blockLen, buffer, buffer32 } = this;
    const len = data.length;
    const offset = data.byteOffset;
    const buf = data.buffer;
    for (let pos = 0; pos < len; ) {
      if (this.pos === blockLen) {
        swap32IfBE(buffer32);
        this.compress(buffer32, 0, false);
        swap32IfBE(buffer32);
        this.pos = 0;
      }
      const take = Math.min(blockLen - this.pos, len - pos);
      const dataOffset = offset + pos;
      if (take === blockLen && !(dataOffset % 4) && pos + take < len) {
        const data32 = new Uint32Array(buf, dataOffset, Math.floor((len - pos) / 4));
        swap32IfBE(data32);
        for (let pos32 = 0; pos + blockLen < len; pos32 += buffer32.length, pos += blockLen) {
          this.length += blockLen;
          this.compress(data32, pos32, false);
        }
        swap32IfBE(data32);
        continue;
      }
      buffer.set(data.subarray(pos, pos + take), this.pos);
      this.pos += take;
      this.length += take;
      pos += take;
    }
    return this;
  }
  digestInto(out) {
    aexists(this);
    aoutput(out, this);
    const { pos, buffer32 } = this;
    this.finished = true;
    clean(this.buffer.subarray(pos));
    swap32IfBE(buffer32);
    this.compress(buffer32, 0, true);
    swap32IfBE(buffer32);
    if (out.byteOffset & 3)
      throw new RangeError('"digestInto() output" expected 4-byte aligned byteOffset, got ' + out.byteOffset);
    const state = this.get();
    const out32 = u32(out);
    const full = Math.floor(this.outputLen / 4);
    for (let i = 0; i < full; i++)
      out32[i] = swap8IfBE(state[i]);
    const tail = this.outputLen % 4;
    if (!tail)
      return;
    const off = full * 4;
    const word = state[full];
    for (let i = 0; i < tail; i++)
      out[off + i] = word >>> 8 * i;
  }
  digest() {
    const { buffer, outputLen } = this;
    this.digestInto(buffer);
    const res = buffer.slice(0, outputLen);
    this.destroy();
    return res;
  }
  _cloneInto(to) {
    const { buffer, length, finished, destroyed, outputLen, pos } = this;
    to ||= new this.constructor({ dkLen: outputLen });
    to.set(...this.get());
    to.buffer.set(buffer);
    to.destroyed = destroyed;
    to.finished = finished;
    to.length = length;
    to.pos = pos;
    to.outputLen = outputLen;
    return to;
  }
  clone() {
    return this._cloneInto();
  }
};
function compress2(s, offset, msg, rounds, v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15) {
  let j = 0;
  for (let i = 0; i < rounds; i++) {
    ({ a: v0, b: v4, c: v8, d: v12 } = G1s(v0, v4, v8, v12, msg[offset + s[j++]]));
    ({ a: v0, b: v4, c: v8, d: v12 } = G2s(v0, v4, v8, v12, msg[offset + s[j++]]));
    ({ a: v1, b: v5, c: v9, d: v13 } = G1s(v1, v5, v9, v13, msg[offset + s[j++]]));
    ({ a: v1, b: v5, c: v9, d: v13 } = G2s(v1, v5, v9, v13, msg[offset + s[j++]]));
    ({ a: v2, b: v6, c: v10, d: v14 } = G1s(v2, v6, v10, v14, msg[offset + s[j++]]));
    ({ a: v2, b: v6, c: v10, d: v14 } = G2s(v2, v6, v10, v14, msg[offset + s[j++]]));
    ({ a: v3, b: v7, c: v11, d: v15 } = G1s(v3, v7, v11, v15, msg[offset + s[j++]]));
    ({ a: v3, b: v7, c: v11, d: v15 } = G2s(v3, v7, v11, v15, msg[offset + s[j++]]));
    ({ a: v0, b: v5, c: v10, d: v15 } = G1s(v0, v5, v10, v15, msg[offset + s[j++]]));
    ({ a: v0, b: v5, c: v10, d: v15 } = G2s(v0, v5, v10, v15, msg[offset + s[j++]]));
    ({ a: v1, b: v6, c: v11, d: v12 } = G1s(v1, v6, v11, v12, msg[offset + s[j++]]));
    ({ a: v1, b: v6, c: v11, d: v12 } = G2s(v1, v6, v11, v12, msg[offset + s[j++]]));
    ({ a: v2, b: v7, c: v8, d: v13 } = G1s(v2, v7, v8, v13, msg[offset + s[j++]]));
    ({ a: v2, b: v7, c: v8, d: v13 } = G2s(v2, v7, v8, v13, msg[offset + s[j++]]));
    ({ a: v3, b: v4, c: v9, d: v14 } = G1s(v3, v4, v9, v14, msg[offset + s[j++]]));
    ({ a: v3, b: v4, c: v9, d: v14 } = G2s(v3, v4, v9, v14, msg[offset + s[j++]]));
  }
  return { v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15 };
}

// node_modules/@noble/hashes/blake3.js
var B3_Flags = {
  CHUNK_START: 1,
  CHUNK_END: 2,
  PARENT: 4,
  ROOT: 8,
  KEYED_HASH: 16,
  DERIVE_KEY_CONTEXT: 32,
  DERIVE_KEY_MATERIAL: 64
};
var B3_IV = /* @__PURE__ */ SHA256_IV.slice();
var B3_SIGMA = /* @__PURE__ */ (() => {
  const Id = Array.from({ length: 16 }, (_, i) => i);
  const permute = (arr) => [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8].map((i) => arr[i]);
  const res = [];
  for (let i = 0, v = Id; i < 7; i++, v = permute(v))
    res.push(...v);
  return Uint8Array.from(res);
})();
var _BLAKE3 = class __BLAKE3 extends _BLAKE2 {
  canXOF = true;
  chunkPos = 0;
  // Position of current block in chunk
  // How many chunks we already have; exact while this stays within
  // JS's safe-integer range.
  chunksDone = 0;
  flags = 0 | 0;
  IV;
  state;
  stack = [];
  // Output
  posOut = 0;
  bufferOut32 = new Uint32Array(16);
  bufferOut;
  // Index of output chunk; exact while this stays within JS's
  // safe-integer range.
  chunkOut = 0;
  enableXOF = true;
  constructor(opts2 = {}, flags = 0) {
    super(64, opts2.dkLen === void 0 ? 32 : opts2.dkLen);
    const { key, context } = opts2;
    const hasContext = context !== void 0;
    if (key !== void 0) {
      if (hasContext)
        throw new Error('Only "key" or "context" can be specified at same time');
      abytes(key, 32, "key");
      const k = copyBytes(key);
      this.IV = u32(k);
      swap32IfBE(this.IV);
      this.flags = flags | B3_Flags.KEYED_HASH;
    } else if (hasContext) {
      abytes(context, void 0, "context");
      const ctx = context;
      const contextKey = new __BLAKE3({ dkLen: 32 }, B3_Flags.DERIVE_KEY_CONTEXT).update(ctx).digest();
      this.IV = u32(contextKey);
      swap32IfBE(this.IV);
      this.flags = flags | B3_Flags.DERIVE_KEY_MATERIAL;
    } else {
      this.IV = B3_IV.slice();
      this.flags = flags;
    }
    this.state = this.IV.slice();
    this.bufferOut = u8(this.bufferOut32);
  }
  // _BLAKE2's scalar-state hooks are unused here: BLAKE3 keeps its tree/XOF state in arrays and
  // copies it directly in _cloneInto().
  get() {
    return [];
  }
  set() {
  }
  // Truncated chunk/parent compression: seed v8..v15 as IV[0..3], t0, t1,
  // block length, and flags, then keep only the first 8 output words.
  b2Compress(counter, flags, buf, bufPos = 0) {
    const { state: s, pos } = this;
    const { h, l } = fromBig(BigInt(counter), true);
    const { v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15 } = compress2(B3_SIGMA, bufPos, buf, 7, s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7], B3_IV[0], B3_IV[1], B3_IV[2], B3_IV[3], h, l, pos, flags);
    s[0] = v0 ^ v8;
    s[1] = v1 ^ v9;
    s[2] = v2 ^ v10;
    s[3] = v3 ^ v11;
    s[4] = v4 ^ v12;
    s[5] = v5 ^ v13;
    s[6] = v6 ^ v14;
    s[7] = v7 ^ v15;
  }
  compress(buf, bufPos = 0, isLast = false) {
    let flags = this.flags;
    if (!this.chunkPos)
      flags |= B3_Flags.CHUNK_START;
    if (this.chunkPos === 15 || isLast)
      flags |= B3_Flags.CHUNK_END;
    if (!isLast)
      this.pos = this.blockLen;
    this.b2Compress(this.chunksDone, flags, buf, bufPos);
    this.chunkPos += 1;
    if (this.chunkPos === 16 || isLast) {
      let chunk = this.state;
      this.state = this.IV.slice();
      for (let last, chunks = this.chunksDone + 1; isLast || !(chunks & 1); chunks >>= 1) {
        if (!(last = this.stack.pop()))
          break;
        this.buffer32.set(last, 0);
        this.buffer32.set(chunk, 8);
        this.pos = this.blockLen;
        this.b2Compress(0, this.flags | B3_Flags.PARENT, this.buffer32, 0);
        chunk = this.state;
        this.state = this.IV.slice();
      }
      this.chunksDone++;
      this.chunkPos = 0;
      this.stack.push(chunk);
    }
    this.pos = 0;
  }
  _cloneInto(to) {
    to = super._cloneInto(to);
    const { IV, flags, state, chunkPos, posOut, chunkOut, stack, chunksDone } = this;
    to.state.set(state.slice());
    to.stack = stack.map((i) => Uint32Array.from(i));
    to.IV.set(IV);
    to.flags = flags;
    to.chunkPos = chunkPos;
    to.chunksDone = chunksDone;
    to.posOut = posOut;
    to.chunkOut = chunkOut;
    to.enableXOF = this.enableXOF;
    to.bufferOut32.set(this.bufferOut32);
    return to;
  }
  destroy() {
    this.destroyed = true;
    clean(this.state, this.buffer32, this.IV, this.bufferOut32);
    clean(...this.stack);
  }
  // Root/XOF compression: rerun the same ROOT inputs with incrementing output
  // counter `t` and materialize all 16 output words.
  // Same as b2Compress, but doesn't modify state and returns 16 u32 array (instead of 8)
  b2CompressOut() {
    const { state: s, pos, flags, buffer32, bufferOut32: out32 } = this;
    const { h, l } = fromBig(BigInt(this.chunkOut++));
    swap32IfBE(buffer32);
    const { v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15 } = compress2(B3_SIGMA, 0, buffer32, 7, s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7], B3_IV[0], B3_IV[1], B3_IV[2], B3_IV[3], l, h, pos, flags);
    out32[0] = v0 ^ v8;
    out32[1] = v1 ^ v9;
    out32[2] = v2 ^ v10;
    out32[3] = v3 ^ v11;
    out32[4] = v4 ^ v12;
    out32[5] = v5 ^ v13;
    out32[6] = v6 ^ v14;
    out32[7] = v7 ^ v15;
    out32[8] = s[0] ^ v8;
    out32[9] = s[1] ^ v9;
    out32[10] = s[2] ^ v10;
    out32[11] = s[3] ^ v11;
    out32[12] = s[4] ^ v12;
    out32[13] = s[5] ^ v13;
    out32[14] = s[6] ^ v14;
    out32[15] = s[7] ^ v15;
    swap32IfBE(buffer32);
    swap32IfBE(out32);
    this.posOut = 0;
  }
  finish() {
    if (this.finished)
      return;
    this.finished = true;
    clean(this.buffer.subarray(this.pos));
    let flags = this.flags | B3_Flags.ROOT;
    if (this.stack.length) {
      flags |= B3_Flags.PARENT;
      swap32IfBE(this.buffer32);
      this.compress(this.buffer32, 0, true);
      swap32IfBE(this.buffer32);
      this.chunksDone = 0;
      this.pos = this.blockLen;
    } else {
      flags |= (!this.chunkPos ? B3_Flags.CHUNK_START : 0) | B3_Flags.CHUNK_END;
    }
    this.flags = flags;
    this.b2CompressOut();
  }
  writeInto(out) {
    aexists(this, false);
    abytes(out);
    this.finish();
    const { blockLen, bufferOut } = this;
    for (let pos = 0, len = out.length; pos < len; ) {
      if (this.posOut >= blockLen)
        this.b2CompressOut();
      const take = Math.min(blockLen - this.posOut, len - pos);
      out.set(bufferOut.subarray(this.posOut, this.posOut + take), pos);
      this.posOut += take;
      pos += take;
    }
    return out;
  }
  xofInto(out) {
    if (!this.enableXOF)
      throw new Error("XOF is not possible after digest call");
    return this.writeInto(out);
  }
  xof(bytes) {
    anumber(bytes);
    return this.xofInto(new Uint8Array(bytes));
  }
  digestInto(out) {
    aoutput(out, this);
    if (this.finished)
      throw new Error("digest() was already called");
    this.enableXOF = false;
    this.writeInto(out.subarray(0, this.outputLen));
    this.destroy();
  }
  digest() {
    const out = new Uint8Array(this.outputLen);
    this.digestInto(out);
    return out;
  }
};
var blake3 = /* @__PURE__ */ createHasher((opts2 = {}) => new _BLAKE3(opts2));
export {
  blake3,
  ml_dsa65,
  ml_kem768
};
