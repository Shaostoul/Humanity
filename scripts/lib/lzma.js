'use strict';
/**
 * lzma.js: a plain LZMA1 and LZMA2 decoder (the algorithm of Igor Pavlov's public-domain LZMA SDK,
 * following its reference "LzmaSpec" decoder), for the instrument pipeline's 7z reader
 * (scripts/lib/sevenzip.js, 2026-11-06). FreePats publishes its CC0 sample sets as .7z archives
 * and this machine has no 7-Zip, so the pipeline reads them itself rather than asking anyone to
 * install a tool. Slow next to C (tens of MB/s), which is plenty for a build step.
 *
 *   decodeLzma(props5, input, outSize) -> Buffer
 *   decodeLzma2(input, outSize) -> Buffer
 *
 * `props5` is the 5-byte LZMA properties (lc/lp/pb byte, then the dictionary size), as a 7z
 * coder carries them; `outSize` the exact unpacked size (7z streams carry no end marker).
 */

const kNumStates = 12;
const kMatchMinLen = 2;
const kEndPosModelIndex = 14;
const kNumFullDistances = 1 << (kEndPosModelIndex >>> 1);
const kNumAlignBits = 4;
const kNumLenToPosStates = 4;
const kNumPosBitsMax = 4;
const kTopValue = 1 << 24;
const PROB_INIT = 1024;

class RangeDecoder {
  constructor(buf) {
    this.buf = buf;
    this.pos = 0;
    this.range = 0xFFFFFFFF;
    this.code = 0;
    if (this.byte() !== 0) throw new Error('lzma: bad range coder start byte');
    for (let i = 0; i < 4; i++) this.code = ((this.code << 8) | this.byte()) >>> 0;
    if (this.code === this.range) throw new Error('lzma: corrupted stream');
  }
  byte() {
    if (this.pos >= this.buf.length) return 0; // past the end only while flushing the last symbols
    return this.buf[this.pos++];
  }
  normalize() {
    if (this.range < kTopValue) {
      this.range = (this.range << 8) >>> 0;
      this.code = ((this.code << 8) | this.byte()) >>> 0;
    }
  }
  directBits(numBits) {
    let res = 0;
    do {
      this.range = this.range >>> 1;
      this.code = (this.code - this.range) >>> 0;
      const t = 0 - (this.code >>> 31);
      this.code = (this.code + (this.range & t)) >>> 0;
      res = ((res << 1) + (t + 1)) >>> 0;
      this.normalize();
    } while (--numBits);
    return res;
  }
  bit(probs, i) {
    const p = probs[i];
    const bound = (this.range >>> 11) * p;
    let symbol;
    if (this.code < bound) {
      probs[i] = p + ((2048 - p) >>> 5);
      this.range = bound >>> 0;
      symbol = 0;
    } else {
      probs[i] = p - (p >>> 5);
      this.code = (this.code - bound) >>> 0;
      this.range = (this.range - bound) >>> 0;
      symbol = 1;
    }
    this.normalize();
    return symbol;
  }
}

function probs(n) {
  return new Uint16Array(n).fill(PROB_INIT);
}

function bitTree(rc, p, base, numBits) {
  let m = 1;
  for (let i = 0; i < numBits; i++) m = (m << 1) + rc.bit(p, base + m);
  return m - (1 << numBits);
}

function bitTreeReverse(rc, p, base, numBits) {
  let m = 1;
  let symbol = 0;
  for (let i = 0; i < numBits; i++) {
    const bit = rc.bit(p, base + m);
    m = (m << 1) + bit;
    symbol |= bit << i;
  }
  return symbol;
}

class LenDecoder {
  constructor() {
    this.choice = probs(2);
    this.low = probs(1 << (kNumPosBitsMax + 3));
    this.mid = probs(1 << (kNumPosBitsMax + 3));
    this.high = probs(256);
  }
  decode(rc, posState) {
    if (rc.bit(this.choice, 0) === 0) return bitTree(rc, this.low, posState << 3, 3);
    if (rc.bit(this.choice, 1) === 0) return 8 + bitTree(rc, this.mid, posState << 3, 3);
    return 16 + bitTree(rc, this.high, 0, 8);
  }
}

/** The decoder's state, kept across LZMA2 chunks (which may keep or reset it). */
class LzmaDecoder {
  constructor(out) {
    this.out = out;
    this.outPos = 0;
  }
  setProps(propsByte) {
    let d = propsByte;
    if (d >= 9 * 5 * 5) throw new Error('lzma: bad properties');
    this.lc = d % 9;
    d = (d / 9) | 0;
    this.lp = d % 5;
    this.pb = (d / 5) | 0;
  }
  resetState() {
    this.literalProbs = probs(0x300 << (this.lc + this.lp));
    this.posSlot = probs(kNumLenToPosStates << 6);
    this.posDecoders = probs(1 + kNumFullDistances - kEndPosModelIndex);
    this.align = probs(1 << kNumAlignBits);
    this.isMatch = probs(kNumStates << kNumPosBitsMax);
    this.isRep = probs(kNumStates);
    this.isRepG0 = probs(kNumStates);
    this.isRepG1 = probs(kNumStates);
    this.isRepG2 = probs(kNumStates);
    this.isRep0Long = probs(kNumStates << kNumPosBitsMax);
    this.lenDecoder = new LenDecoder();
    this.repLenDecoder = new LenDecoder();
    this.reps = [0, 0, 0, 0];
    this.state = 0;
  }
  /** Decode symbols from `rc` until the output reaches `outEnd` (or an end marker). */
  decode(rc, outEnd) {
    const out = this.out;
    const { lc, literalProbs, posSlot, posDecoders, align, isMatch, isRep, isRepG0, isRepG1, isRepG2, isRep0Long, lenDecoder, repLenDecoder } = this;
    let [rep0, rep1, rep2, rep3] = this.reps;
    let state = this.state;
    let outPos = this.outPos;
    const pbMask = (1 << this.pb) - 1;
    const lpMask = (1 << this.lp) - 1;
    while (outPos < outEnd) {
      const posState = outPos & pbMask;
      if (rc.bit(isMatch, (state << kNumPosBitsMax) + posState) === 0) {
        const prevByte = outPos > 0 ? out[outPos - 1] : 0;
        const litState = ((outPos & lpMask) << lc) + (prevByte >>> (8 - lc));
        const base = 0x300 * litState;
        let symbol = 1;
        if (state >= 7) {
          let matchByte = out[outPos - rep0 - 1];
          do {
            const matchBit = (matchByte >>> 7) & 1;
            matchByte <<= 1;
            const bit = rc.bit(literalProbs, base + ((1 + matchBit) << 8) + symbol);
            symbol = (symbol << 1) | bit;
            if (matchBit !== bit) break;
          } while (symbol < 0x100);
        }
        while (symbol < 0x100) symbol = (symbol << 1) | rc.bit(literalProbs, base + symbol);
        out[outPos++] = symbol - 0x100;
        state = state < 4 ? 0 : state < 10 ? state - 3 : state - 6;
        continue;
      }
      let len;
      if (rc.bit(isRep, state) !== 0) {
        if (outPos === 0) throw new Error('lzma: rep match at the start');
        if (rc.bit(isRepG0, state) === 0) {
          if (rc.bit(isRep0Long, (state << kNumPosBitsMax) + posState) === 0) {
            state = state < 7 ? 9 : 11;
            out[outPos] = out[outPos - rep0 - 1];
            outPos++;
            continue;
          }
        } else {
          let dist;
          if (rc.bit(isRepG1, state) === 0) {
            dist = rep1;
          } else {
            if (rc.bit(isRepG2, state) === 0) {
              dist = rep2;
            } else {
              dist = rep3;
              rep3 = rep2;
            }
            rep2 = rep1;
          }
          rep1 = rep0;
          rep0 = dist;
        }
        len = repLenDecoder.decode(rc, posState);
        state = state < 7 ? 8 : 11;
      } else {
        rep3 = rep2;
        rep2 = rep1;
        rep1 = rep0;
        len = lenDecoder.decode(rc, posState);
        state = state < 7 ? 7 : 10;
        const lenState = Math.min(len, kNumLenToPosStates - 1);
        const slot = bitTree(rc, posSlot, lenState << 6, 6);
        if (slot < 4) {
          rep0 = slot;
        } else {
          const numDirectBits = (slot >>> 1) - 1;
          let dist = ((2 | (slot & 1)) << numDirectBits) >>> 0;
          if (slot < kEndPosModelIndex) {
            dist = (dist + bitTreeReverse(rc, posDecoders, dist - slot, numDirectBits)) >>> 0;
          } else {
            dist = (dist + ((rc.directBits(numDirectBits - kNumAlignBits) << kNumAlignBits) >>> 0)) >>> 0;
            dist = (dist + bitTreeReverse(rc, align, 0, kNumAlignBits)) >>> 0;
          }
          rep0 = dist;
        }
        if (rep0 === 0xFFFFFFFF) break; // the end marker
        if (rep0 >= outPos) throw new Error('lzma: distance past the start');
      }
      len += kMatchMinLen;
      if (outPos + len > outEnd) len = outEnd - outPos;
      const from = outPos - rep0 - 1;
      for (let i = 0; i < len; i++) out[outPos + i] = out[from + i];
      outPos += len;
    }
    this.reps = [rep0, rep1, rep2, rep3];
    this.state = state;
    this.outPos = outPos;
  }
}

/** LZMA1 as a 7z coder carries it: 5 property bytes, no end marker, the size known. */
function decodeLzma(props5, input, outSize) {
  const dec = new LzmaDecoder(Buffer.alloc(outSize));
  dec.setProps(props5[0]);
  dec.resetState();
  dec.decode(new RangeDecoder(input), outSize);
  return dec.out;
}

/** LZMA2: a run of chunks, each LZMA (with its own range coder) or stored, keeping or resetting
 * the state, the properties and the dictionary as its control byte says. */
function decodeLzma2(input, outSize) {
  const dec = new LzmaDecoder(Buffer.alloc(outSize));
  let pos = 0;
  let haveProps = false;
  for (;;) {
    if (pos >= input.length) throw new Error('lzma2: stream ends without its end byte');
    const control = input[pos++];
    if (control === 0x00) break;
    if (control === 0x01 || control === 0x02) {
      const size = ((input[pos] << 8) | input[pos + 1]) + 1;
      pos += 2;
      input.copy(dec.out, dec.outPos, pos, pos + size);
      dec.outPos += size;
      pos += size;
      continue;
    }
    if (control < 0x80) throw new Error(`lzma2: bad control byte ${control}`);
    const unpacked = (((control & 0x1f) << 16) | (input[pos] << 8) | input[pos + 1]) + 1;
    const packed = ((input[pos + 2] << 8) | input[pos + 3]) + 1;
    pos += 4;
    const reset = (control >>> 5) & 3;
    if (reset >= 2) {
      dec.setProps(input[pos++]);
      haveProps = true;
    }
    if (!haveProps) throw new Error('lzma2: a chunk before any properties');
    if (reset >= 1) dec.resetState();
    dec.decode(new RangeDecoder(input.subarray(pos, pos + packed)), dec.outPos + unpacked);
    pos += packed;
  }
  if (dec.outPos !== outSize) throw new Error(`lzma2: ${dec.outPos} bytes, expected ${outSize}`);
  return dec.out;
}

module.exports = { decodeLzma, decodeLzma2 };
