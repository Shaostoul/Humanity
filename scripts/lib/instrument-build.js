'use strict';
/**
 * instrument-build.js: the pieces of the instrument pipeline (scripts/instrument-packs.js,
 * 2026-11-06, docs/design/music.md rung 6) that are worth testing on their own:
 *
 * - noteToMidi / midiToNote: note names as the libraries write them ("A#3", "c4", "A#-1").
 * - yin(): the fundamental of a stretch of sound (de Cheveigne and Kawahara's YIN, 2002), used
 *   to record each sample's real source tuning (some libraries are A = 440, some are not, and
 *   some number their octaves differently).
 * - Tar: a deterministic ustar writer (sorted paths, fixed times and owners), so the same samples
 *   always make byte-identical packs and the checksum in data/music/packs.ron stays meaningful.
 * - ron(): writes the instrument definitions and the catalog the app reads.
 * - audioInfo(): the sample rate and channels of a WAV or FLAC file, read from its header.
 */

const NOTE_INDEX = { c: 0, d: 2, e: 4, f: 5, g: 7, a: 9, b: 11 };

/** "A#3" -> 58 with C4 = 60 (scientific pitch). Octave offsets between libraries are found by
 * measurement, not assumed (instrument-packs.js `octaveShift`). Null for anything else. */
function noteToMidi(name) {
  const m = /^([A-Ga-g])(#|b|s)?(-?\d)$/.exec(String(name));
  if (!m) return null;
  let n = NOTE_INDEX[m[1].toLowerCase()];
  if (m[2] === '#' || m[2] === 's') n += 1;
  if (m[2] === 'b') n -= 1;
  return (Number(m[3]) + 1) * 12 + n;
}

const NAMES = ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B'];
function midiToNote(midi) {
  return NAMES[((midi % 12) + 12) % 12] + (Math.floor(midi / 12) - 1);
}

/** The frequency of MIDI note `midi` with A4 at `a4` Hz, equal temperament (music.md section 9). */
function noteHz(midi, a4 = 440) {
  return a4 * Math.pow(2, (midi - 69) / 12);
}

/**
 * YIN over `x` (mono samples) at `sr`, searching periods between `fmin` and `fmax` Hz.
 * Returns { hz, clarity } (clarity 0..1, 1 = a perfectly periodic stretch) or null.
 */
function yin(x, sr, fmin, fmax, threshold = 0.12) {
  const maxTau = Math.min(Math.floor(sr / fmin), Math.floor(x.length / 2) - 1);
  const minTau = Math.max(2, Math.floor(sr / fmax));
  if (maxTau <= minTau + 2) return null;
  const w = x.length - maxTau;
  const d = new Float64Array(maxTau + 1);
  for (let tau = 1; tau <= maxTau; tau++) {
    let sum = 0;
    for (let j = 0; j < w; j++) {
      const diff = x[j] - x[j + tau];
      sum += diff * diff;
    }
    d[tau] = sum;
  }
  const cm = new Float64Array(maxTau + 1);
  cm[0] = 1;
  let running = 0;
  for (let tau = 1; tau <= maxTau; tau++) {
    running += d[tau];
    cm[tau] = running > 0 ? (d[tau] * tau) / running : 1;
  }
  let tau = -1;
  for (let t = minTau; t <= maxTau; t++) {
    if (cm[t] < threshold) {
      while (t + 1 <= maxTau && cm[t + 1] < cm[t]) t++;
      tau = t;
      break;
    }
  }
  if (tau < 0) {
    // No dip under the threshold: the deepest one, reported with its (low) clarity.
    let best = minTau;
    for (let t = minTau; t <= maxTau; t++) if (cm[t] < cm[best]) best = t;
    tau = best;
  }
  let refined = tau;
  if (tau > 1 && tau < maxTau) {
    const a = cm[tau - 1], b = cm[tau], c = cm[tau + 1];
    const denom = a - 2 * b + c;
    if (denom !== 0) refined = tau + (a - c) / (2 * denom);
  }
  return { hz: sr / refined, clarity: Math.max(0, 1 - cm[tau]) };
}

/**
 * The pitch of a recorded note: YIN over a few windows after the attack (the attack itself is
 * noise), the median of the clear ones. `expectHz` bounds the search to a little over an octave
 * either way, which is how a library that numbers its octaves differently is caught.
 */
function measurePitch(mono, sr, expectHz) {
  let peak = 0;
  for (let i = 0; i < mono.length; i++) peak = Math.max(peak, Math.abs(mono[i]));
  if (peak <= 0) return null;
  let onset = 0;
  while (onset < mono.length && Math.abs(mono[onset]) < peak * 0.1) onset++;
  const fmin = expectHz / 2.3;
  const fmax = expectHz * 2.3;
  const win = Math.max(2048, Math.ceil((sr / fmin) * 2.5));
  const found = [];
  for (const after of [0.15, 0.35, 0.6]) {
    const start = onset + Math.floor(after * sr);
    if (start + win > mono.length) break;
    const r = yin(mono.subarray(start, start + win), sr, fmin, fmax);
    if (r && r.clarity > 0.8) found.push(r.hz);
  }
  if (found.length === 0) return null;
  found.sort((a, b) => a - b);
  return found[Math.floor(found.length / 2)];
}

// ── Tar ──────────────────────────────────────────────────────────────────────────────────────

/** A ustar archive written the same way every time: the caller's entries sorted by path, mode
 * 0644, owner 0, time 0. Paths must be relative, forward-slashed, without "..", at most 100 bytes. */
function tarBuffer(entries) {
  const sorted = [...entries].sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0));
  const parts = [];
  for (const e of sorted) {
    const name = Buffer.from(e.path, 'utf8');
    if (name.length > 100 || e.path.startsWith('/') || e.path.split('/').includes('..')) {
      throw new Error(`tar: path not allowed: ${e.path}`);
    }
    const h = Buffer.alloc(512);
    name.copy(h, 0);
    h.write('0000644\0', 100, 'ascii');
    h.write('0000000\0', 108, 'ascii');
    h.write('0000000\0', 116, 'ascii');
    h.write(e.data.length.toString(8).padStart(11, '0') + '\0', 124, 'ascii');
    h.write('00000000000\0', 136, 'ascii');
    h.write('        ', 148, 'ascii');
    h.write('0', 156, 'ascii');
    h.write('ustar\0', 257, 'ascii');
    h.write('00', 263, 'ascii');
    let sum = 0;
    for (let i = 0; i < 512; i++) sum += h[i];
    h.write(sum.toString(8).padStart(6, '0') + '\0 ', 148, 'ascii');
    parts.push(h, e.data);
    const pad = (512 - (e.data.length % 512)) % 512;
    if (pad) parts.push(Buffer.alloc(pad));
  }
  parts.push(Buffer.alloc(1024));
  return Buffer.concat(parts);
}

/** The entries of a ustar archive written by `tarBuffer` (for tests). */
function readTar(buf) {
  const out = [];
  let pos = 0;
  while (pos + 512 <= buf.length) {
    const h = buf.subarray(pos, pos + 512);
    if (h.every((b) => b === 0)) break;
    const path = h.subarray(0, 100).toString('utf8').replace(/\0.*$/s, '');
    const size = parseInt(h.subarray(124, 136).toString('ascii').replace(/\0.*$/s, '').trim(), 8);
    pos += 512;
    out.push({ path, data: buf.subarray(pos, pos + size) });
    pos += Math.ceil(size / 512) * 512;
  }
  return out;
}

// ── RON ──────────────────────────────────────────────────────────────────────────────────────

/** Marks a float so it is always written with a decimal point. */
class F { constructor(v) { this.v = v; } }
const f = (v) => new F(v);
/** A named struct: `Name(field: value, ...)`; an unnamed one is a plain object. */
const struct = (name, fields) => ({ $struct: name, ...fields });
/** An enum variant holding a struct: `Name((...))`. */
const variant = (name, value) => ({ $variant: name, value });
/** `Some(v)`; null and undefined are written `None`. */
const some = (v) => ({ $some: v });

function ronString(s) {
  return '"' + String(s).replace(/\\/g, '\\\\').replace(/"/g, '\\"').replace(/\n/g, '\\n') + '"';
}

function ronFloat(v) {
  if (!Number.isFinite(v)) throw new Error(`ron: not a finite number: ${v}`);
  const s = String(Math.round(v * 1e6) / 1e6);
  return s.includes('.') || s.includes('e') ? s : s + '.0';
}

/** RON text for `v`. Objects whose fields are all simple stay on one line (the zones). */
function ron(v, indent = '') {
  if (v === null || v === undefined) return 'None';
  if (v instanceof F) return ronFloat(v.v);
  if (typeof v === 'number') {
    if (!Number.isInteger(v)) throw new Error(`ron: use f() for the float ${v}`);
    return String(v);
  }
  if (typeof v === 'boolean') return v ? 'true' : 'false';
  if (typeof v === 'string') return ronString(v);
  if (Array.isArray(v)) {
    if (v.length === 0) return '[]';
    const inner = indent + '    ';
    return '[\n' + v.map((x) => inner + ron(x, inner) + ',\n').join('') + indent + ']';
  }
  if (v.$some !== undefined) return 'Some(' + ron(v.$some, indent) + ')';
  if (v.$variant) return v.$variant + '(' + ron(v.value, indent) + ')';
  const name = v.$struct || '';
  const keys = Object.keys(v).filter((k) => k !== '$struct');
  const simple = keys.every((k) => {
    const x = v[k];
    return x === null || x === undefined || x instanceof F || typeof x !== 'object' || (x.$some !== undefined && typeof x.$some !== 'object') || (x.$some instanceof F);
  });
  if (simple && keys.length <= 14 && !name) {
    return '(' + keys.map((k) => `${k}: ${ron(v[k], indent)}`).join(', ') + ')';
  }
  const inner = indent + '    ';
  return name + '(\n' + keys.map((k) => `${inner}${k}: ${ron(v[k], inner)},\n`).join('') + indent + ')';
}

// ── Audio file headers ───────────────────────────────────────────────────────────────────────

/** { rate, channels, frames } of a WAV or FLAC file's bytes, from its header; null if neither. */
function audioInfo(buf) {
  if (buf.subarray(0, 4).toString('ascii') === 'fLaC') {
    // STREAMINFO is the first metadata block: 20 bits of rate, 3 of channels-1, 5 of bits-1, 36 of
    // total samples, starting at byte 18.
    const b = buf.subarray(18, 26);
    const rate = (b[0] << 12) | (b[1] << 4) | (b[2] >> 4);
    const channels = ((b[2] >> 1) & 7) + 1;
    const frames = (b[3] & 0x0f) * 2 ** 32 + b.readUInt32BE(4);
    return { rate, channels, frames };
  }
  if (buf.subarray(0, 4).toString('ascii') === 'RIFF' && buf.subarray(8, 12).toString('ascii') === 'WAVE') {
    let pos = 12;
    let rate = 0, channels = 0, blockAlign = 0, dataBytes = 0;
    while (pos + 8 <= buf.length) {
      const id = buf.subarray(pos, pos + 4).toString('ascii');
      const size = buf.readUInt32LE(pos + 4);
      if (id === 'fmt ') {
        channels = buf.readUInt16LE(pos + 10);
        rate = buf.readUInt32LE(pos + 12);
        blockAlign = buf.readUInt16LE(pos + 20);
      } else if (id === 'data') {
        dataBytes = Math.min(size, buf.length - pos - 8);
        break;
      }
      pos += 8 + size + (size & 1);
    }
    if (!rate) return null;
    return { rate, channels, frames: blockAlign ? Math.floor(dataBytes / blockAlign) : 0 };
  }
  return null;
}

module.exports = {
  noteToMidi, midiToNote, noteHz, yin, measurePitch,
  tarBuffer, readTar,
  ron, f, struct, variant, some,
  audioInfo,
};
