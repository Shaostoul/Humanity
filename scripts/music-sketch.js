#!/usr/bin/env node
// HumanityOS music: a DIRECTION SKETCH of "mothership calm", rendered offline (2026-11-06).
//
// This is NOT the game's music engine. It is a listenable sketch, written to answer "what could
// a generative score made only from our own synthesis sound like?" before the real engine is
// built in Rust (docs/design/music.md). Everything here is synthesised from maths: no samples,
// no downloads, no models.
//
// Kept as permanent dev tooling (docs/design/music.md, section 19): the engine's first rung is
// compared against it. The WAV files are never committed; they go to --outdir.
//
// What it renders (all at 44.1 kHz, 16-bit, stereo):
//   mus-mothership-calm-432.wav   90 s, A4 = 432 Hz, with the ship hum under it
//   mus-mothership-calm-440.wav   the same 90 s note for note at A4 = 440 Hz (same seed)
//   mus-ship-hum-432.wav          30 s of the ship hum alone, at the level it has in the mix
//
// The layers: a slow chord progression in D (pads of detuned saws through a slowly moving
// low-pass), a wordless choir ("ah" drifting to "oh") made by formant synthesis, sparse
// celesta-like bells on a half-second grid (a 60 bpm feel), a low drone on D, an 8-line feedback
// delay network reverb, and the ship hum: brown noise, low-passed, with soft resonances tuned to
// the key's root and fifth at the chosen tuning, breathing over tens of seconds.
//
// Every pitch comes from ONE function, noteHz(midi, a4), so the tuning applies to every voice.
// The report proves it two ways: the table of notes used, and the frequencies actually measured
// in the rendered audio (a test tone, and the drone found by a Goertzel scan of the final mix).
//
// Levels are measured with ITU-R BS.1770 (K-weighting, 400 ms blocks, absolute and relative
// gates) and a 4x oversampled true-peak estimate; the meter checks itself against the standard's
// reference relation first (a 997 Hz sine at -23 dBFS in both channels reads -23 LUFS).
//
// Usage: node scripts/music-sketch.js [--outdir <dir>] [--seconds 90] [--music-lufs -18] [--hum-lu -18] [--stems]
// (--outdir defaults to <system temp>/humanityos-music-sketch; --stems also prints each layer's loudness.)

'use strict';
const fs = require('fs');
const path = require('path');

const SR = 44100;
const args = process.argv.slice(2);
function arg(name, dflt) {
  const i = args.indexOf(name);
  return i >= 0 && i + 1 < args.length ? args[i + 1] : dflt;
}
const OUTDIR = arg('--outdir', path.join(require('os').tmpdir(), 'humanityos-music-sketch'));
fs.mkdirSync(OUTDIR, { recursive: true });
const SECONDS = Number(arg('--seconds', '90'));
const TARGET_LUFS = Number(arg('--music-lufs', '-18')); // the listening files' loudness
const HUM_LU = Number(arg('--hum-lu', '-18'));          // hum loudness relative to the music
const SEED = 20261106;
const BLOCK = 64;

// ---------------------------------------------------------------------------------------------
// Pitch: the one place a frequency is made.
// ---------------------------------------------------------------------------------------------
function noteHz(midi, a4) {
  return a4 * Math.pow(2, (midi - 69) / 12);
}
const NOTE_NAMES = ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B'];
function noteName(midi) {
  return NOTE_NAMES[midi % 12] + (Math.floor(midi / 12) - 1);
}
const usedNotes = new Set();

// ---------------------------------------------------------------------------------------------
// Seeded randomness (mulberry32), so the 432 and 440 renders are the same piece.
// ---------------------------------------------------------------------------------------------
function mulberry32(a) {
  return function () {
    a |= 0; a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

// ---------------------------------------------------------------------------------------------
// Building blocks
// ---------------------------------------------------------------------------------------------
// Band-limited sawtooth correction (polyBLEP).
function polyblep(t, dt) {
  if (t < dt) { t /= dt; return t + t - t * t - 1; }
  if (t > 1 - dt) { t = (t - 1) / dt; return t * t + t + t + 1; }
  return 0;
}

// Topology-preserving state variable filter (Zavalishin): stable under fast modulation, so the
// pads' cutoff and the choir's vowel can move without zipper noise or blow-ups.
class SVF {
  constructor() { this.ic1 = 0; this.ic2 = 0; this.bp = 0; }
}
function svfCoefs(fc, q) {
  const g = Math.tan(Math.PI * Math.min(fc, SR * 0.45) / SR);
  const k = 1 / q;
  const a1 = 1 / (1 + g * (g + k));
  return { k, a1, a2: g * a1, a3: g * g * a1 };
}
// Returns the low-pass output; the band-pass output is left in s.bp (no allocation per sample).
function svfTick(s, c, v0) {
  const v3 = v0 - s.ic2;
  const v1 = c.a1 * s.ic1 + c.a2 * v3;
  const v2 = s.ic2 + c.a2 * s.ic1 + c.a3 * v3;
  s.ic1 = 2 * v1 - s.ic1;
  s.ic2 = 2 * v2 - s.ic2;
  s.bp = v1;
  return v2;
}

// A soft envelope: raised-cosine attack, exponential release.
class Env {
  constructor(attackS, releaseS) {
    this.att = Math.max(1, Math.round(attackS * SR));
    this.relCoef = Math.exp(Math.log(1e-4) / (releaseS * SR)); // -80 dB over releaseS
    this.n = 0; this.level = 0; this.state = 0; // 0 attack, 1 sustain, 2 release, 3 done
  }
  next() {
    if (this.state === 0) {
      this.n++;
      if (this.n >= this.att) { this.state = 1; this.level = 1; }
      else this.level = 0.5 - 0.5 * Math.cos(Math.PI * this.n / this.att);
    } else if (this.state === 2) {
      this.level *= this.relCoef;
      if (this.level < 1e-5) { this.state = 3; this.level = 0; }
    }
    return this.level;
  }
  release() { if (this.state < 2) this.state = 2; }
  get done() { return this.state === 3; }
}

function panGains(p) { // p in -1..1, constant power
  const a = (p + 1) * Math.PI / 4;
  return [Math.cos(a), Math.sin(a)];
}
const dbToAmp = db => Math.pow(10, db / 20);

// ---------------------------------------------------------------------------------------------
// Instruments
// ---------------------------------------------------------------------------------------------
// PAD: three detuned saws, a slowly breathing low-pass, wide stereo.
class Pad {
  constructor(midi, a4, rng, amp) {
    usedNotes.add(midi);
    const f = noteHz(midi, a4);
    this.oscs = [-7, 0.5, 6.5].map((cents, i) => {
      const fr = f * Math.pow(2, (cents + (rng() - 0.5) * 3) / 1200);
      const [gl, gr] = panGains([-0.65, 0, 0.65][i]);
      return { ph: rng(), dt: fr / SR, gl, gr };
    });
    this.f = f;
    this.env = new Env(3.5, 5.5);
    this.svfL = new SVF(); this.svfR = new SVF();
    // Lower notes a little quieter so the chord's top voices lead.
    this.amp = amp * (midi < 48 ? 0.6 : 1.0);
    this.lfoPh = rng() * Math.PI * 2;
  }
  render(L, R, sL, sR, n, t0, send) {
    const lfo = Math.sin(2 * Math.PI * 0.045 * t0 + this.lfoPh);
    const fc = (850 + 650 * (0.5 + 0.5 * lfo)) * (1 + Math.min(this.f / 300, 1.5));
    const c = svfCoefs(fc, 0.75);
    for (let i = 0; i < n; i++) {
      const e = this.env.next();
      if (e === 0 && this.env.state === 3) break;
      let xl = 0, xr = 0;
      for (const o of this.oscs) {
        const s = 2 * o.ph - 1 - polyblep(o.ph, o.dt);
        o.ph += o.dt; if (o.ph >= 1) o.ph -= 1;
        xl += s * o.gl; xr += s * o.gr;
      }
      const yl = svfTick(this.svfL, c, xl) * e * this.amp;
      const yr = svfTick(this.svfR, c, xr) * e * this.amp;
      L[i] += yl; R[i] += yr; sL[i] += yl * send; sR[i] += yr * send;
    }
  }
}

// CHOIR: formant synthesis. Each note is two "singers" (an ensemble), each a glottal-like source
// (band-limited saw, extra roll-off, a breath of noise) with vibrato and slow pitch drift, through
// five parallel band-pass formants. Formant values are the classic soprano vowel table published
// in the Csound manual's formant appendix ("a" and "o").
const VOWEL_A = { f: [800, 1150, 2900, 3900, 4950], db: [0, -6, -32, -20, -50], bw: [80, 90, 120, 130, 140] };
const VOWEL_O = { f: [450, 800, 2830, 3800, 4950], db: [0, -11, -22, -22, -50], bw: [70, 80, 100, 130, 135] };
function vowelAt(m) { // m 0 = "ah", 1 = "oh"
  const out = [];
  for (let k = 0; k < 5; k++) {
    const f = VOWEL_A.f[k] + (VOWEL_O.f[k] - VOWEL_A.f[k]) * m;
    const bw = VOWEL_A.bw[k] + (VOWEL_O.bw[k] - VOWEL_A.bw[k]) * m;
    const db = VOWEL_A.db[k] + (VOWEL_O.db[k] - VOWEL_A.db[k]) * m;
    // Band output of the SVF times k (= 1/Q) has unity gain at the centre.
    const c = svfCoefs(f, f / bw);
    out.push({ c, g: dbToAmp(db) * c.k });
  }
  return out;
}
class ChoirNote {
  constructor(midi, a4, rng, amp) {
    usedNotes.add(midi);
    const f = noteHz(midi, a4);
    this.singers = [0, 1].map(j => {
      const det = (j === 0 ? -5 : 5) + (rng() - 0.5) * 4;
      const [gl, gr] = panGains((j === 0 ? -0.35 : 0.35) + (rng() - 0.5) * 0.3);
      return {
        f: f * Math.pow(2, det / 1200), ph: rng(), lp: 0,
        vibPh: rng() * 6.283, vibRate: 5.0 + rng() * 0.6, drift: 0, driftV: 0,
        forms: [new SVF(), new SVF(), new SVF(), new SVF(), new SVF()], gl, gr,
        noise: mulberry32((rng() * 1e9) | 0),
      };
    });
    this.env = new Env(4.0, 6.5);
    this.amp = amp;
    this.age = 0;
  }
  render(L, R, sL, sR, n, vowel, send) {
    const lpA = Math.exp(-2 * Math.PI * 1300 / SR);
    for (let i = 0; i < n; i++) {
      const e = this.env.next();
      if (e === 0 && this.env.state === 3) break;
      this.age++;
      // Vibrato fades in over 1.5 s; depth about 6 cents.
      const vibDepth = 0.0035 * Math.min(1, this.age / (1.5 * SR));
      for (const s of this.singers) {
        s.vibPh += 2 * Math.PI * s.vibRate / SR;
        if ((this.age & 255) === 0) { // slow random drift, about +-4 cents
          s.driftV += (s.noise() - 0.5) * 0.0004 - s.drift * 0.02;
          s.driftV *= 0.9;
        }
        s.drift += s.driftV / 256;
        const fr = s.f * (1 + vibDepth * Math.sin(s.vibPh) + s.drift);
        const dt = fr / SR;
        let src = 2 * s.ph - 1 - polyblep(s.ph, dt);
        s.ph += dt; if (s.ph >= 1) s.ph -= 1;
        src += (s.noise() - 0.5) * 0.06; // breath
        s.lp = s.lp * lpA + src * (1 - lpA); // extra glottal roll-off
        let y = 0;
        for (let k = 0; k < 5; k++) { const f = s.forms[k]; svfTick(f, vowel[k].c, s.lp); y += f.bp * vowel[k].g; }
        y *= e * this.amp;
        L[i] += y * s.gl; R[i] += y * s.gr;
        sL[i] += y * s.gl * send; sR[i] += y * s.gr * send;
      }
    }
  }
}

// BELL: a soft celesta-like tone by modal synthesis, each partial a decaying rotating phasor
// (no sine calls per sample). Partial ratios are slightly stretched, like a stiff bar or string.
const BELL_PARTIALS = [
  [1.0, 1.0, 3.2], [2.003, 0.32, 2.0], [3.01, 0.13, 1.2], [4.17, 0.07, 0.8], [5.43, 0.04, 0.5], [6.8, 0.02, 0.35],
];
class Bell {
  constructor(midi, a4, vel, pan) {
    usedNotes.add(midi);
    const f = noteHz(midi, a4);
    this.parts = BELL_PARTIALS.filter(p => f * p[0] < SR * 0.4).map(([r, a, dec]) => {
      const w = 2 * Math.PI * f * r / SR;
      const rad = Math.exp(Math.log(1e-3) / (dec * SR)); // -60 dB over dec seconds
      return { re: 0, im: a, cr: Math.cos(w) * rad, ci: Math.sin(w) * rad };
    });
    [this.gl, this.gr] = panGains(pan);
    this.vel = vel; this.n = 0; this.attack = Math.round(0.004 * SR);
    this.done = false;
  }
  render(L, R, sL, sR, n, send) {
    for (let i = 0; i < n; i++) {
      let y = 0;
      for (const p of this.parts) {
        const re = p.re * p.cr - p.im * p.ci;
        p.im = p.re * p.ci + p.im * p.cr;
        p.re = re;
        y += p.im;
      }
      const a = this.n < this.attack ? this.n / this.attack : 1;
      this.n++;
      y *= a * this.vel;
      L[i] += y * this.gl; R[i] += y * this.gr;
      sL[i] += y * this.gl * send; sR[i] += y * this.gr * send;
    }
    if (this.n > 4 * SR) this.done = true;
  }
}

// DRONE: the key's root two octaves down with its fifth, breathing slowly.
class Drone {
  constructor(rootMidi, a4) {
    usedNotes.add(rootMidi); usedNotes.add(rootMidi + 7);
    this.parts = [[noteHz(rootMidi, a4), 1.0], [noteHz(rootMidi, a4) * 2, 0.18], [noteHz(rootMidi + 7, a4), 0.25]]
      .map(([f, a]) => ({ re: 0, im: a, cr: Math.cos(2 * Math.PI * f / SR), ci: Math.sin(2 * Math.PI * f / SR) }));
    this.n = 0;
  }
  render(L, R, sL, sR, n, amp, send) {
    for (let i = 0; i < n; i++) {
      const t = (this.n++) / SR;
      const breath = 0.8 + 0.2 * Math.sin(2 * Math.PI * t / 19);
      let y = 0;
      for (const p of this.parts) {
        const re = p.re * p.cr - p.im * p.ci;
        p.im = p.re * p.ci + p.im * p.cr; p.re = re; y += p.im;
      }
      // Renormalise the phasors now and then so rounding never drifts their amplitude.
      if ((this.n & 4095) === 0) for (const p of this.parts) {
        const m = Math.hypot(p.re, p.im), want = p === this.parts[0] ? 1 : (p === this.parts[1] ? 0.18 : 0.25);
        p.re *= want / m; p.im *= want / m;
      }
      y *= amp * breath;
      L[i] += y; R[i] += y; sL[i] += y * send; sR[i] += y * send;
    }
  }
}

// REVERB: 8-line feedback delay network with a Hadamard mix, per-line damping and slowly
// modulated lengths (so the tail never rings metallic), after a pre-delay and four diffusers.
class Reverb {
  constructor(t60, rng) {
    const base = [1657, 2129, 2441, 2887, 3301, 3769, 4391, 4943];
    this.lines = base.map((d, i) => ({
      buf: new Float32Array(8192), w: 0, d, g: Math.pow(10, -3 * d / (t60 * SR)), lp: 0,
      modPh: rng() * 6.283, modRate: 0.07 + 0.05 * i / 7,
    }));
    this.pre = { buf: new Float32Array(4096), w: 0, d: Math.round(0.028 * SR) };
    this.ap = [225, 341, 441, 556].map(d => ({ buf: new Float32Array(d), w: 0, g: 0.6 }));
    this.damp = 0.25;
  }
  process(inL, inR, L, R, n, wet) {
    const lines = this.lines, x = new Float64Array(8);
    for (let i = 0; i < n; i++) {
      // pre-delay + diffusion on the mono-ish input
      const p = this.pre;
      p.buf[p.w] = (inL[i] + inR[i]) * 0.5;
      let s = p.buf[(p.w - p.d + p.buf.length) % p.buf.length];
      p.w = (p.w + 1) % p.buf.length;
      for (const a of this.ap) {
        const z = a.buf[a.w];
        const y = -a.g * s + z;
        a.buf[a.w] = s + a.g * y;
        a.w = (a.w + 1) % a.buf.length;
        s = y;
      }
      const side = (inL[i] - inR[i]) * 0.3;
      for (let k = 0; k < 8; k++) {
        const ln = lines[k];
        ln.modPh += 2 * Math.PI * ln.modRate / SR;
        const dd = ln.d + 6 * Math.sin(ln.modPh);
        let rp = ln.w - dd; if (rp < 0) rp += 8192;
        const i0 = Math.floor(rp), fr = rp - i0;
        const a = ln.buf[i0 & 8191], b = ln.buf[(i0 + 1) & 8191];
        let v = a + (b - a) * fr;
        ln.lp = v * (1 - this.damp) + ln.lp * this.damp; // high frequencies die sooner
        x[k] = ln.lp * ln.g;
      }
      // Fast Walsh-Hadamard, normalised.
      for (let h = 1; h < 8; h <<= 1) for (let a = 0; a < 8; a += h << 1) for (let b = a; b < a + h; b++) {
        const u = x[b], v = x[b + h]; x[b] = u + v; x[b + h] = u - v;
      }
      const norm = 1 / Math.sqrt(8);
      for (let k = 0; k < 8; k++) {
        const ln = lines[k];
        ln.buf[ln.w] = x[k] * norm + s * (k & 1 ? 0.5 : 0.5) + (k < 4 ? side : -side) * 0.25;
        ln.w = (ln.w + 1) & 8191;
      }
      const outL = (x[0] - x[2] + x[4] - x[6]) * norm;
      const outR = (x[1] - x[3] + x[5] - x[7]) * norm;
      L[i] += outL * wet; R[i] += outR * wet;
    }
  }
}

// SHIP HUM (the ambient bed): brown noise (1/f^2), low-passed, with soft resonances at the key
// root and fifth (at the chosen tuning) so it reads as engines and structure, not plain noise,
// and a slow breathing over tens of seconds. Brown noise is a leaky integral of white noise.
class ShipHum {
  constructor(rootMidi, a4, rng) {
    this.rng = rng;
    this.bL = 0; this.bR = 0;
    this.lpL = new SVF(); this.lpR = new SVF(); this.hpL = new SVF(); this.hpR = new SVF();
    this.lpC = svfCoefs(420, 0.6); this.hpC = svfCoefs(24, 0.6);
    // Resonances: root an octave below the drone, the drone's root, the fifth above it, the root
    // and fifth an octave up (so small speakers that cannot play 36-72 Hz still carry the hum).
    const res = [[-24, 0.5, 10], [-12, 1.0, 16], [-5, 0.7, 18], [0, 0.55, 22], [7, 0.3, 24], [12, 0.18, 26]];
    this.res = res.map(([semi, g, q]) => {
      const midi = rootMidi + semi;
      usedNotes.add(midi);
      const f = noteHz(midi, a4);
      return { f, c: svfCoefs(f, q), g, sL: new SVF(), sR: new SVF(), ph: rng() * 6.283 };
    });
    // A faint tonal engine line at the drone's root, two oscillators 0.07 Hz apart (slow beating).
    const fT = noteHz(rootMidi - 12, a4);
    this.tone = [fT, fT + 0.07].map(f => ({ re: 0, im: 1, cr: Math.cos(2 * Math.PI * f / SR), ci: Math.sin(2 * Math.PI * f / SR) }));
    this.n = 0;
  }
  render(L, R, n, amp) {
    const t0 = this.n / SR;
    const breath = 1 + 0.16 * Math.sin(2 * Math.PI * t0 / 27) + 0.07 * Math.sin(2 * Math.PI * t0 / 41 + 1);
    for (let i = 0; i < n; i++) {
      this.n++;
      const c = (this.rng() - 0.5), wl = c * 0.75 + (this.rng() - 0.5) * 0.66, wr = c * 0.75 + (this.rng() - 0.5) * 0.66;
      this.bL = this.bL * 0.9975 + wl * 0.05;
      this.bR = this.bR * 0.9975 + wr * 0.05;
      let xl = svfTick(this.hpL, this.hpC, this.bL); xl = this.bL - xl; // high-pass = in - low
      let xr = svfTick(this.hpR, this.hpC, this.bR); xr = this.bR - xr;
      xl = svfTick(this.lpL, this.lpC, xl);
      xr = svfTick(this.lpR, this.lpC, xr);
      let yl = xl * 0.55, yr = xr * 0.55;
      for (const r of this.res) {
        svfTick(r.sL, r.c, xl); yl += r.sL.bp * r.c.k * r.g * 2.2;
        svfTick(r.sR, r.c, xr); yr += r.sR.bp * r.c.k * r.g * 2.2;
      }
      let tone = 0;
      for (const p of this.tone) {
        const re = p.re * p.cr - p.im * p.ci; p.im = p.re * p.ci + p.im * p.cr; p.re = re; tone += p.im;
      }
      if ((this.n & 4095) === 0) for (const p of this.tone) { const m = Math.hypot(p.re, p.im); p.re /= m; p.im /= m; }
      tone *= 0.012;
      L[i] += (yl + tone) * amp * breath;
      R[i] += (yr + tone) * amp * breath;
    }
  }
}

// ---------------------------------------------------------------------------------------------
// The piece: "mothership calm", in D (Lydian colour), 60 bpm feel, chords every two bars.
// Pad voicings and choir notes are written out so the voice leading is smooth (shared notes are
// held across the change, the rest move by step).
// ---------------------------------------------------------------------------------------------
const ROOT = 38; // D2
const CHORDS = [
  { t: 0,  name: 'Dmaj9',    pad: [50, 57, 61, 64, 66], choir: null },
  { t: 8,  name: 'Bm9',      pad: [47, 54, 57, 61, 62], choir: [66, 69, 73] },
  { t: 16, name: 'Gmaj7#11', pad: [43, 54, 59, 61, 62], choir: [66, 71, 73] },
  { t: 24, name: 'Asus2',    pad: [45, 52, 57, 59, 64], choir: [64, 69, 71] },
  { t: 32, name: 'F#m7',     pad: [42, 52, 57, 61, 64], choir: [64, 69, 73] },
  { t: 40, name: 'Gmaj9',    pad: [43, 54, 57, 59, 62], choir: [66, 69, 71] },
  { t: 48, name: 'Em9',      pad: [40, 54, 55, 59, 62], choir: null },
  { t: 56, name: 'Asus4',    pad: [45, 52, 57, 62, 64], choir: [62, 64, 69] },
  { t: 64, name: 'Bm11',     pad: [47, 52, 57, 62, 66], choir: [64, 66, 71] },
  { t: 72, name: 'Gmaj7#11', pad: [43, 54, 59, 61, 66], choir: [66, 71, 73] },
  { t: 78, name: 'Dadd9',    pad: [38, 50, 57, 64, 66], choir: [66, 69, 74] },
];
const PENTA = [2, 4, 6, 9, 11]; // D major pentatonic pitch classes: D E F# A B
function pentaAbove(midi, steps) { // move `steps` pentatonic steps from the nearest pentatonic note
  let m = midi;
  while (!PENTA.includes(m % 12)) m++;
  let s = steps;
  while (s > 0) { m++; if (PENTA.includes(m % 12)) s--; }
  while (s < 0) { m--; if (PENTA.includes(m % 12)) s++; }
  return m;
}

function render(a4, seconds, opts = {}) {
  const N = Math.round(seconds * SR);
  const music = [new Float32Array(N), new Float32Array(N)];
  const hum = [new Float32Array(N), new Float32Array(N)];
  const rng = mulberry32(SEED);
  const humRng = mulberry32(SEED + 1);
  const reverb = new Reverb(4.8, mulberry32(SEED + 2));
  const drone = new Drone(ROOT, a4);
  const shipHum = new ShipHum(ROOT + 12, a4, humRng); // root D3; resonances from D1 to D4
  const bellRng = mulberry32(SEED + 3);

  // Bell events: one short motif per chord from the third on (most of the time), on a 0.5 s grid,
  // the motif kept and varied so it sounds composed rather than random.
  const motif = [0, 2, 1]; // pentatonic steps
  const bells = [];
  if (!opts.humOnly) {
    for (let c = 2; c < CHORDS.length; c++) {
      if (bellRng() < 0.2 && c !== CHORDS.length - 1) continue;
      const ch = CHORDS[c];
      const start = ch.t + 1 + Math.floor(bellRng() * 6) * 0.5;
      const anchor = ch.pad[2 + Math.floor(bellRng() * 3)] + 24; // a chord tone two octaves up
      const vary = Math.floor(bellRng() * 3) - 1;
      let tt = start;
      motif.forEach((st, k) => {
        const midi = pentaAbove(anchor, st + (k === 2 ? vary : 0));
        bells.push({ t: tt, midi: Math.min(midi, 93), vel: 0.10 + bellRng() * 0.06, pan: (bellRng() - 0.5) * 0.9 });
        tt += bellRng() < 0.5 ? 1.0 : 1.5;
      });
    }
    bells.sort((a, b) => a.t - b.t);
  }

  const pads = [], choir = [], ringing = [];
  let chordIdx = -1, bellIdx = 0;
  const L = new Float32Array(BLOCK), R = new Float32Array(BLOCK);
  const sL = new Float32Array(BLOCK), sR = new Float32Array(BLOCK);
  const hL = new Float32Array(BLOCK), hR = new Float32Array(BLOCK);

  for (let b0 = 0; b0 < N; b0 += BLOCK) {
    const n = Math.min(BLOCK, N - b0);
    const t0 = b0 / SR;
    L.fill(0); R.fill(0); sL.fill(0); sR.fill(0); hL.fill(0); hR.fill(0);

    if (!opts.humOnly) {
      // Chord changes: hold shared notes, release the rest, start the new ones.
      while (chordIdx + 1 < CHORDS.length && CHORDS[chordIdx + 1].t <= t0) {
        chordIdx++;
        const ch = CHORDS[chordIdx];
        for (const v of pads) if (!ch.pad.includes(v.midi)) v.voice.env.release();
        for (const m of ch.pad) if (!pads.some(v => v.midi === m && v.voice.env.state < 2)) {
          pads.push({ midi: m, voice: new Pad(m, a4, rng, 0.05) });
        }
        const want = ch.choir || [];
        for (const v of choir) if (!want.includes(v.midi)) v.voice.env.release();
        for (const m of want) if (!choir.some(v => v.midi === m && v.voice.env.state < 2)) {
          choir.push({ midi: m, voice: new ChoirNote(m, a4, rng, 0.14) });
        }
      }
      while (bellIdx < bells.length && bells[bellIdx].t <= t0) {
        const e = bells[bellIdx++];
        ringing.push(new Bell(e.midi, a4, e.vel, e.pan));
      }

      // The choir's vowel drifts between "ah" and "oh" over about 40 s.
      const vowel = vowelAt(0.5 - 0.5 * Math.cos(2 * Math.PI * t0 / 40));
      const on = k => !opts.only || opts.only === k; // --stems renders one layer at a time
      if (on('pads')) for (const v of pads) v.voice.render(L, R, sL, sR, n, t0, 0.4);
      if (on('bells')) for (const b of ringing) b.render(L, R, sL, sR, n, 0.75);
      if (on('choir')) for (const v of choir) v.voice.render(L, R, sL, sR, n, vowel, 0.6);
      if (on('drone')) drone.render(L, R, sL, sR, n, 0.05, 0.15);
      for (let i = pads.length - 1; i >= 0; i--) {
        if (pads[i].voice.env.done) pads.splice(i, 1);
      }
      for (let i = ringing.length - 1; i >= 0; i--) {
        if (ringing[i].done) ringing.splice(i, 1);
      }
      for (let i = choir.length - 1; i >= 0; i--) if (choir[i].voice.env.done) choir.splice(i, 1);
      reverb.process(sL, sR, L, R, n, 0.55);
    }
    shipHum.render(hL, hR, n, 0.06);

    // Fades: music in over 8 s, out from 82 s; the hum follows the file's edges too.
    for (let i = 0; i < n; i++) {
      const t = (b0 + i) / SR;
      let g = 1;
      const fadeIn = opts.humOnly ? 3 : 8, fadeOutStart = seconds - (opts.humOnly ? 3 : 8);
      if (t < fadeIn) g = 0.5 - 0.5 * Math.cos(Math.PI * t / fadeIn);
      if (t > fadeOutStart) g *= 0.5 + 0.5 * Math.cos(Math.PI * Math.min(1, (t - fadeOutStart) / (seconds - fadeOutStart)));
      music[0][b0 + i] = L[i] * g; music[1][b0 + i] = R[i] * g;
      hum[0][b0 + i] = hL[i] * g; hum[1][b0 + i] = hR[i] * g;
    }
  }
  return { music, hum, bells };
}

// ---------------------------------------------------------------------------------------------
// Measurement: BS.1770 loudness, true peak, sample peak, RMS.
// ---------------------------------------------------------------------------------------------
function kCoefs(fs) {
  // Stage 1, the head's high shelf; stage 2, the RLB high-pass. Analog prototype parameters
  // that reproduce BS.1770's published 48 kHz coefficients through the bilinear transform.
  let f0 = 1681.974450955533, G = 3.999843853973347, Q = 0.7071752369554196;
  let K = Math.tan(Math.PI * f0 / fs);
  const Vh = Math.pow(10, G / 20), Vb = Math.pow(Vh, 0.4996667741545416);
  let a0 = 1 + K / Q + K * K;
  const s1 = {
    b0: (Vh + Vb * K / Q + K * K) / a0, b1: 2 * (K * K - Vh) / a0, b2: (Vh - Vb * K / Q + K * K) / a0,
    a1: 2 * (K * K - 1) / a0, a2: (1 - K / Q + K * K) / a0,
  };
  f0 = 38.13547087602444; Q = 0.5003270373238773; K = Math.tan(Math.PI * f0 / fs);
  a0 = 1 + K / Q + K * K;
  const s2 = { b0: 1, b1: -2, b2: 1, a1: 2 * (K * K - 1) / a0, a2: (1 - K / Q + K * K) / a0 };
  return [s1, s2];
}
function biquadRun(x, c) {
  const y = new Float64Array(x.length);
  let x1 = 0, x2 = 0, y1 = 0, y2 = 0;
  for (let i = 0; i < x.length; i++) {
    const v = c.b0 * x[i] + c.b1 * x1 + c.b2 * x2 - c.a1 * y1 - c.a2 * y2;
    x2 = x1; x1 = x[i]; y2 = y1; y1 = v; y[i] = v;
  }
  return y;
}
function integratedLufs(chans, fs = SR) {
  const [s1, s2] = kCoefs(fs);
  const kw = chans.map(c => biquadRun(biquadRun(c, s1), s2));
  const blk = Math.round(0.4 * fs), hop = Math.round(0.1 * fs);
  const z = [];
  for (let s = 0; s + blk <= kw[0].length; s += hop) {
    let sum = 0;
    for (const c of kw) { let m = 0; for (let i = s; i < s + blk; i++) m += c[i] * c[i]; sum += m / blk; }
    z.push(sum);
  }
  const lk = v => -0.691 + 10 * Math.log10(v);
  const abs = z.filter(v => lk(v) > -70);
  if (!abs.length) return -Infinity;
  const relGate = lk(abs.reduce((a, b) => a + b, 0) / abs.length) - 10;
  const rel = abs.filter(v => lk(v) > relGate);
  return lk(rel.reduce((a, b) => a + b, 0) / rel.length);
}
// 4x oversampled true peak with a 48-tap windowed-sinc interpolator (BS.1770 Annex 2 asks for
// at least 4x; this is an estimate in the same spirit, not the standard's exact filter).
const TP_TAPS = (() => {
  const taps = 48, os = 4, h = [];
  for (let n = 0; n < taps; n++) {
    const x = (n - (taps - 1) / 2) / os;
    const sinc = x === 0 ? 1 : Math.sin(Math.PI * x) / (Math.PI * x);
    const w = 0.5 - 0.5 * Math.cos(2 * Math.PI * (n + 0.5) / taps);
    h.push(sinc * w);
  }
  const phases = [];
  for (let p = 0; p < os; p++) phases.push(h.filter((_, n) => n % os === p));
  return phases;
})();
function truePeak(chans) {
  let peak = 0;
  for (const c of chans) {
    for (let i = 12; i < c.length - 12; i++) {
      for (const ph of TP_TAPS) {
        let s = 0;
        for (let k = 0; k < ph.length; k++) s += ph[k] * c[i - 6 + k];
        const a = Math.abs(s); if (a > peak) peak = a;
      }
    }
  }
  return 20 * Math.log10(peak);
}
function samplePeak(chans) {
  let p = 0; for (const c of chans) for (let i = 0; i < c.length; i++) { const a = Math.abs(c[i]); if (a > p) p = a; }
  return 20 * Math.log10(p);
}
function rmsDb(chans) {
  let s = 0, n = 0; for (const c of chans) for (let i = 0; i < c.length; i++) { s += c[i] * c[i]; n++; }
  return 10 * Math.log10(s / n);
}
function scale(chans, g) { return chans.map(c => { const o = new Float32Array(c.length); for (let i = 0; i < c.length; i++) o[i] = c[i] * g; return o; }); }
function add(a, b) { return a.map((c, k) => { const o = new Float32Array(c.length); for (let i = 0; i < c.length; i++) o[i] = c[i] + b[k][i]; return o; }); }

// Frequency of the strongest component between lo and hi Hz (Goertzel scan over a Hann-windowed
// stretch of the mono sum). Resolution about 1 / stretch length.
function goertzelPeak(chans, t0, t1, lo, hi, step) {
  const s0 = Math.round(t0 * SR), s1 = Math.round(t1 * SR), n = s1 - s0;
  const x = new Float64Array(n);
  for (let i = 0; i < n; i++) x[i] = (chans[0][s0 + i] + chans[1][s0 + i]) * (0.5 - 0.5 * Math.cos(2 * Math.PI * i / n));
  let best = lo, bestP = -1;
  for (let f = lo; f <= hi; f += step) {
    const w = 2 * Math.PI * f / SR, cw = 2 * Math.cos(w);
    let q1 = 0, q2 = 0;
    for (let i = 0; i < n; i++) { const q0 = cw * q1 - q2 + x[i]; q2 = q1; q1 = q0; }
    const p = q1 * q1 + q2 * q2 - cw * q1 * q2;
    if (p > bestP) { bestP = p; best = f; }
  }
  return best;
}
// Frequency of a test tone by counting interpolated upward zero crossings.
function zeroCrossHz(x) {
  let first = -1, last = -1, count = 0;
  for (let i = 1; i < x.length; i++) {
    if (x[i - 1] < 0 && x[i] >= 0) {
      const t = i - 1 + (-x[i - 1]) / (x[i] - x[i - 1]);
      if (first < 0) first = t; else count++;
      last = t;
    }
  }
  return count * SR / (last - first);
}

// ---------------------------------------------------------------------------------------------
// WAV writer: 16-bit PCM with TPDF dither.
// ---------------------------------------------------------------------------------------------
function writeWav(file, chans) {
  const n = chans[0].length, ch = chans.length;
  const buf = Buffer.alloc(44 + n * ch * 2);
  buf.write('RIFF', 0); buf.writeUInt32LE(36 + n * ch * 2, 4); buf.write('WAVE', 8);
  buf.write('fmt ', 12); buf.writeUInt32LE(16, 16); buf.writeUInt16LE(1, 20); buf.writeUInt16LE(ch, 22);
  buf.writeUInt32LE(SR, 24); buf.writeUInt32LE(SR * ch * 2, 28); buf.writeUInt16LE(ch * 2, 32); buf.writeUInt16LE(16, 34);
  buf.write('data', 36); buf.writeUInt32LE(n * ch * 2, 40);
  const d = mulberry32(SEED + 9);
  let o = 44, clipped = 0;
  for (let i = 0; i < n; i++) for (let k = 0; k < ch; k++) {
    let s = Math.round(chans[k][i] * 32767 + (d() - d()));
    if (s > 32767) { s = 32767; clipped++; } else if (s < -32768) { s = -32768; clipped++; }
    buf.writeInt16LE(s, o); o += 2;
  }
  fs.writeFileSync(file, buf);
  return clipped;
}

// ---------------------------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------------------------
// A listen check for ears we do not have: loudness over time (no sudden changes), clicks, DC,
// stereo width, and where the energy sits in the spectrum.
function listenCheck(mix, label) {
  const [s1, s2] = kCoefs(SR);
  const kw = mix.map(c => biquadRun(biquadRun(c, s1), s2));
  const win = 3 * SR, hop = SR, st = [];
  for (let s = 0; s + win <= kw[0].length; s += hop) {
    let z = 0;
    for (const c of kw) { let m = 0; for (let i = s; i < s + win; i++) m += c[i] * c[i]; z += m / win; }
    st.push({ t: s / SR + 3, l: -0.691 + 10 * Math.log10(z) });
  }
  const body = st.filter(x => x.t >= 11 && x.t <= SECONDS - 8);
  let maxStep = 0, at = 0;
  for (let i = 1; i < body.length; i++) {
    const d = Math.abs(body[i].l - body[i - 1].l);
    if (d > maxStep) { maxStep = d; at = body[i].t; }
  }
  const ls = body.map(x => x.l);
  console.log(`\nListen check, ${label}:`);
  console.log(`  short-term loudness (3 s) between the fades: ${Math.min(...ls).toFixed(1)} to ${Math.max(...ls).toFixed(1)} LUFS; largest change in one second ${maxStep.toFixed(2)} LU (at ${at.toFixed(0)} s)`);
  console.log('  short-term loudness every 6 s: ' + st.filter((x, i) => i % 6 === 0).map(x => `${x.t.toFixed(0)}s ${x.l.toFixed(1)}`).join(', '));
  let jump = 0, sLL = 0, sRR = 0, sLR = 0;
  const dc = [0, 0];
  for (let i = 1; i < mix[0].length; i++) {
    for (let k = 0; k < 2; k++) { const d = Math.abs(mix[k][i] - mix[k][i - 1]); if (d > jump) jump = d; dc[k] += mix[k][i]; }
    sLL += mix[0][i] * mix[0][i]; sRR += mix[1][i] * mix[1][i]; sLR += mix[0][i] * mix[1][i];
  }
  console.log(`  largest sample-to-sample step ${(20 * Math.log10(jump)).toFixed(1)} dBFS (a click would sit near the peak level); DC ${(dc[0] / mix[0].length).toExponential(1)} / ${(dc[1] / mix[1].length).toExponential(1)}; L-R correlation ${(sLR / Math.sqrt(sLL * sRR)).toFixed(2)}`);
  // Band energies from Hann-windowed FFT frames, one a second.
  const nfft = 4096, bands = [[20, 60], [60, 250], [250, 2000], [2000, 6000], [6000, 16000]], e = bands.map(() => 0);
  const mono = new Float64Array(mix[0].length);
  for (let i = 0; i < mono.length; i++) mono[i] = (mix[0][i] + mix[1][i]) / 2;
  for (let s = 10 * SR; s + nfft < mono.length - 8 * SR; s += SR) {
    const re = new Float64Array(nfft), im = new Float64Array(nfft);
    for (let i = 0; i < nfft; i++) re[i] = mono[s + i] * (0.5 - 0.5 * Math.cos(2 * Math.PI * i / nfft));
    fft(re, im);
    for (let k = 1; k < nfft / 2; k++) {
      const f = k * SR / nfft, p = re[k] * re[k] + im[k] * im[k];
      bands.forEach(([lo, hi], j) => { if (f >= lo && f < hi) e[j] += p; });
    }
  }
  const tot = e.reduce((a, b) => a + b, 0);
  console.log('  energy by band: ' + bands.map(([lo, hi], j) => `${lo}-${hi} Hz ${(100 * e[j] / tot).toFixed(1)}%`).join(', '));
}
function fft(re, im) {
  const n = re.length;
  for (let i = 1, j = 0; i < n; i++) {
    let bit = n >> 1;
    for (; j & bit; bit >>= 1) j ^= bit;
    j ^= bit;
    if (i < j) { [re[i], re[j]] = [re[j], re[i]]; [im[i], im[j]] = [im[j], im[i]]; }
  }
  for (let len = 2; len <= n; len <<= 1) {
    const ang = -2 * Math.PI / len, wr = Math.cos(ang), wi = Math.sin(ang), h = len / 2;
    for (let i = 0; i < n; i += len) {
      let cr = 1, ci = 0;
      for (let j = 0; j < h; j++) {
        const ur = re[i + j], ui = im[i + j];
        const vr = re[i + j + h] * cr - im[i + j + h] * ci, vi = re[i + j + h] * ci + im[i + j + h] * cr;
        re[i + j] = ur + vr; im[i + j] = ui + vi; re[i + j + h] = ur - vr; im[i + j + h] = ui - vi;
        const t = cr * wr - ci * wi; ci = cr * wi + ci * wr; cr = t;
      }
    }
  }
}
function fmt(x, d = 2) { return (x >= 0 ? '+' : '') + x.toFixed(d); }
function main() {
  console.log('HumanityOS music sketch: "mothership calm" (a direction sketch, not the engine)\n');

  // 1. The meter checks itself first.
  const [s1, s2] = kCoefs(48000);
  console.log('K-weighting at 48 kHz (BS.1770 table: b0 1.53512486, a1 -1.69065929, a2 0.73248077; stage 2 a1 -1.99004745, a2 0.99007225):');
  console.log(`  ours: b0 ${s1.b0.toFixed(8)}, a1 ${s1.a1.toFixed(8)}, a2 ${s1.a2.toFixed(8)}; stage 2 a1 ${s2.a1.toFixed(8)}, a2 ${s2.a2.toFixed(8)}`);
  const ref = [new Float32Array(10 * SR), new Float32Array(10 * SR)];
  const amp = dbToAmp(-23);
  for (let i = 0; i < ref[0].length; i++) ref[0][i] = ref[1][i] = amp * Math.sin(2 * Math.PI * 997 * i / SR);
  const refL = integratedLufs(ref);
  console.log(`  997 Hz sine, -23 dBFS peak in both channels: ${refL.toFixed(2)} LUFS (expected -23.00)\n`);
  if (Math.abs(refL + 23) > 0.1) throw new Error('loudness meter failed its reference check');

  // 2. Render both tunings from the same seed.
  const out = {};
  for (const a4 of [432, 440]) {
    const t = Date.now();
    const r = render(a4, SECONDS);
    const ms = Date.now() - t;
    const musicL = integratedLufs(r.music), humL = integratedLufs(r.hum);
    // Hum sits HUM_LU below the music, then the mix is brought to TARGET_LUFS, then held under a
    // true peak of -1 dBTP if it would exceed it (no limiter: the gain just comes down).
    const humGain = dbToAmp(musicL + HUM_LU - humL);
    let mix = add(r.music, scale(r.hum, humGain));
    let g = dbToAmp(TARGET_LUFS - integratedLufs(mix));
    mix = scale(mix, g);
    let tp = truePeak(mix);
    if (tp > -1.2) { const back = dbToAmp(-1.2 - tp); mix = scale(mix, back); g *= back; tp = truePeak(mix); }
    const file = path.join(OUTDIR, `mus-mothership-calm-${a4}.wav`);
    const clipped = writeWav(file, mix);
    out[a4] = { file, ms, mix, humGainFinal: humGain * g, musicL, humL, r };
    console.log(`A4 = ${a4} Hz: rendered ${SECONDS} s in ${(ms / 1000).toFixed(1)} s of JavaScript (${(ms / SECONDS).toFixed(0)} ms per second of audio, one core)`);
    console.log(`  ${file}`);
    console.log(`  integrated loudness ${integratedLufs(mix).toFixed(1)} LUFS, true peak ${tp.toFixed(2)} dBTP, sample peak ${samplePeak(mix).toFixed(2)} dBFS, RMS ${rmsDb(mix).toFixed(1)} dBFS, clipped samples ${clipped}`);
    console.log(`  hum vs music: ${fmt(humL + 20 * Math.log10(humGain) - musicL, 1)} LU (the hum alone measured against the music alone)`);
  }

  // 3. The hum alone, 30 s, at exactly the gain it has in the 432 mix.
  {
    const th = Date.now();
    const r = render(432, 30, { humOnly: true });
    const humMs = Date.now() - th;
    const h = scale(r.hum, out[432].humGainFinal);
    const file = path.join(OUTDIR, 'mus-ship-hum-432.wav');
    const clipped = writeWav(file, h);
    const hl = integratedLufs(h);
    console.log(`\nShip hum alone, A4 = 432 Hz, 30 s at its in-mix level:\n  ${file}`);
    console.log(`  rendered in ${(humMs / 30).toFixed(1)} ms per second of audio in JavaScript (one core; includes the per-block fade bookkeeping)`);
    console.log(`  integrated loudness ${hl.toFixed(1)} LUFS (${fmt(hl - TARGET_LUFS, 1)} LU against the -18 LUFS mix), true peak ${truePeak(h).toFixed(2)} dBTP, sample peak ${samplePeak(h).toFixed(2)} dBFS, RMS ${rmsDb(h).toFixed(1)} dBFS, clipped samples ${clipped}`);
  }

  // 4. Proof of tuning.
  console.log('\nEvery note used, as noteHz(midi, a4) = a4 * 2^((midi - 69) / 12):');
  console.log('  note   midi   A4=432 Hz   A4=440 Hz   difference');
  for (const m of [...usedNotes].sort((a, b) => a - b)) {
    const f432 = noteHz(m, 432), f440 = noteHz(m, 440);
    console.log(`  ${noteName(m).padEnd(5)} ${String(m).padStart(4)}   ${f432.toFixed(3).padStart(9)}   ${f440.toFixed(3).padStart(9)}   ${(1200 * Math.log2(f432 / f440)).toFixed(2)} cents`);
  }
  for (const a4 of [432, 440]) {
    const tone = new Float32Array(2 * SR); const f = noteHz(69, a4);
    for (let i = 0; i < tone.length; i++) tone[i] = Math.sin(2 * Math.PI * f * i / SR);
    console.log(`  test tone for A4 through noteHz at ${a4}: measured ${zeroCrossHz(tone).toFixed(3)} Hz`);
  }
  for (const a4 of [432, 440]) {
    const want = noteHz(ROOT, a4);
    const got = goertzelPeak(out[a4].mix, 20, 60, want - 3, want + 3, 0.01);
    const want5 = noteHz(ROOT + 7, a4);
    const got5 = goertzelPeak(out[a4].mix, 20, 60, want5 - 3, want5 + 3, 0.01);
    console.log(`  in the ${a4} mix (20-60 s): the drone's D2 measured at ${got.toFixed(2)} Hz (expected ${want.toFixed(2)}), its A2 at ${got5.toFixed(2)} Hz (expected ${want5.toFixed(2)})`);
  }
  listenCheck(out[432].mix, 'the 432 mix');
  if (args.includes('--stems')) {
    const full = out[432].musicL;
    for (const k of ['pads', 'choir', 'bells', 'drone']) {
      const r = render(432, SECONDS, { only: k });
      console.log(`  stem ${k.padEnd(6)} (with its reverb): ${fmt(integratedLufs(r.music) - full, 1)} LU against all the music`);
    }
  }
  console.log('\nChords: ' + CHORDS.map(c => `${c.t}s ${c.name}`).join(', '));
  console.log(`Bells: ${out[432].r.bells.length} notes on a 0.5 s grid (a 60 bpm feel).`);
}

main();
