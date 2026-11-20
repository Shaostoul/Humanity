#!/usr/bin/env node
/**
 * ly-to-mei.js: our own converter from the LilyPond subset that the Mutopia Project's public-domain
 * scores use to MEI Basic (docs/design/score-player.md, rung 2; the operator, 2026-11-20: "MEI
 * sounds right unless there's better"). It READS the LilyPond text as data: it never runs LilyPond,
 * Scheme or anything the file names. What it understands (everything the three shipped pieces use):
 *
 *   notes, chords, rests, skips, whole-bar rests (R), durations with dots and multipliers, ties,
 *   absolute and \relative pitch, \key, \time, \partial, \clef, \tempo, \transposition,
 *   \times and \tuplet (split at bar lines), \grace / \appoggiatura / \acciaccatura, \ottava,
 *   \repeat volta with \alternative (as repeat signs and endings), \repeat unfold and tremolo
 *   (written out), stem tremolos (c2:8, as bTrem), dynamics, hairpins (\< \> \! \cresc \dim),
 *   articulations (-. -> -^ -- -! and their \names), \fermata, slurs, << { } \\ { } >> voices,
 *   \partcombine, \cadenzaOn/Off, Timing.measurePosition, variables and \include.
 *
 * Everything else (markup, overrides, \set, Scheme, pedals, beams, trills) is skipped; markup text
 * is kept only to find tempo words a piece's settings name. The output keeps to MEI Basic 5.1: the
 * sounding pitch is never written (no accid.ges), so the key signature and the bar's accidentals
 * are worked out as an engraver writes them, and the reader (crates/humanity-music/src/mei.rs) works
 * them back out as a musician reads them.
 *
 * Usage:
 *   node scripts/ly-to-mei.js --piece <id> --src <folder>   convert one piece of ly-to-mei.pieces.json
 *   node scripts/ly-to-mei.js --all --src <folder>          every piece (each file named by its id)
 * The folder holds the downloaded LilyPond files (the pieces file names each, with its SHA-256,
 * which is checked first). Writes data/music/scores/<id>.mei.
 */
'use strict';

const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

// ---------------------------------------------------------------------------------------------
// Exact fractions.
// ---------------------------------------------------------------------------------------------

function gcd(a, b) {
  a = Math.abs(a);
  b = Math.abs(b);
  while (b) [a, b] = [b, a % b];
  return a || 1;
}

class F {
  constructor(n, d = 1) {
    if (d === 0) throw new Error('a fraction over zero');
    if (d < 0) [n, d] = [-n, -d];
    const g = gcd(n, d);
    this.n = n / g;
    this.d = d / g;
  }
  add(o) { return new F(this.n * o.d + o.n * this.d, this.d * o.d); }
  sub(o) { return new F(this.n * o.d - o.n * this.d, this.d * o.d); }
  mul(o) { return new F(this.n * o.n, this.d * o.d); }
  div(o) { return new F(this.n * o.d, this.d * o.n); }
  cmp(o) { return this.n * o.d - o.n * this.d; }
  eq(o) { return this.cmp(o) === 0; }
  lt(o) { return this.cmp(o) < 0; }
  le(o) { return this.cmp(o) <= 0; }
  gt(o) { return this.cmp(o) > 0; }
  ge(o) { return this.cmp(o) >= 0; }
  isZero() { return this.n === 0; }
  num() { return this.n / this.d; }
  toString() { return this.d === 1 ? `${this.n}` : `${this.n}/${this.d}`; }
}
const ZERO = new F(0);
const fmin = (a, b) => (a.le(b) ? a : b);

// ---------------------------------------------------------------------------------------------
// The tokenizer.
// ---------------------------------------------------------------------------------------------

/** Skip a Scheme expression after its '#' (at `i`, just past the '#'): returns the index after it. */
function skipScheme(s, i) {
  if (s[i] === '#') i++; // ##t, ##x01C0
  if (s[i] === "'" || s[i] === '`') i++;
  if (s[i] === '(') {
    let depth = 0;
    for (; i < s.length; i++) {
      const c = s[i];
      if (c === '"') {
        i++;
        while (i < s.length && s[i] !== '"') i += s[i] === '\\' ? 2 : 1;
      } else if (c === '(') depth++;
      else if (c === ')' && --depth === 0) return i + 1;
    }
    return i;
  }
  if (s[i] === '"') {
    i++;
    while (i < s.length && s[i] !== '"') i += s[i] === '\\' ? 2 : 1;
    return i + 1;
  }
  while (i < s.length && !/[\s{}<>()[\]]/.test(s[i])) i++;
  return i;
}

/**
 * The file as tokens: {t: kind, v: value, at: offset}. Kinds: 'cmd' (\name, or \\ \< \> \! \( \)
 * \[ \]), 'str', 'word' (note names, variable names, numbers with their marks), 'sym' (one of
 * { } << >> < > [ ] ( ) ~ | = . * / - ^ _ ' , ! ? :), and 'scheme' (skipped Scheme, kept for
 * \set Timing.measurePosition).
 */
function tokenize(s) {
  const out = [];
  let i = 0;
  while (i < s.length) {
    const c = s[i];
    if (/\s/.test(c)) { i++; continue; }
    if (c === '%') {
      if (s[i + 1] === '{') {
        const e = s.indexOf('%}', i + 2);
        i = e < 0 ? s.length : e + 2;
      } else {
        const e = s.indexOf('\n', i);
        i = e < 0 ? s.length : e + 1;
      }
      continue;
    }
    if (c === '"') {
      let j = i + 1;
      let v = '';
      while (j < s.length && s[j] !== '"') {
        if (s[j] === '\\') { v += s[j + 1]; j += 2; } else { v += s[j]; j++; }
      }
      out.push({ t: 'str', v, at: i });
      i = j + 1;
      continue;
    }
    if (c === '#') {
      const e = skipScheme(s, i + 1);
      out.push({ t: 'scheme', v: s.slice(i, e), at: i });
      i = e;
      continue;
    }
    if (c === '\\') {
      const n = s[i + 1];
      if (/[a-zA-Z]/.test(n)) {
        let j = i + 1;
        while (j < s.length && /[a-zA-Z]/.test(s[j])) j++;
        out.push({ t: 'cmd', v: s.slice(i + 1, j), at: i });
        i = j;
      } else {
        out.push({ t: 'cmd', v: n, at: i });
        i += 2;
      }
      continue;
    }
    if (s.startsWith('<<', i) || s.startsWith('>>', i)) {
      out.push({ t: 'sym', v: s.slice(i, i + 2), at: i });
      i += 2;
      continue;
    }
    if (/[a-zA-Z]/.test(c)) {
      let j = i;
      while (j < s.length && /[a-zA-Z]/.test(s[j])) j++;
      out.push({ t: 'word', v: s.slice(i, j), at: i });
      i = j;
      continue;
    }
    if (/[0-9]/.test(c)) {
      let j = i;
      while (j < s.length && /[0-9]/.test(s[j])) j++;
      out.push({ t: 'num', v: s.slice(i, j), at: i });
      i = j;
      continue;
    }
    out.push({ t: 'sym', v: c, at: i });
    i++;
  }
  return out;
}

// ---------------------------------------------------------------------------------------------
// Reading the file: top-level assignments, the header, and the first \score's expression.
// ---------------------------------------------------------------------------------------------

/** Read `file` and every file it \include-s (beside it) into one token list. */
function loadTokens(file, seen = new Set()) {
  const full = path.resolve(file);
  if (seen.has(full)) return [];
  seen.add(full);
  const toks = tokenize(fs.readFileSync(full, 'utf8'));
  const out = [];
  for (let i = 0; i < toks.length; i++) {
    if (toks[i].t === 'cmd' && toks[i].v === 'include' && toks[i + 1] && toks[i + 1].t === 'str') {
      const name = toks[i + 1].v;
      const inc = path.join(path.dirname(full), name);
      if (fs.existsSync(inc)) out.push(...loadTokens(inc, seen));
      i++;
      continue;
    }
    out.push(toks[i]);
  }
  return out;
}

class Reader {
  constructor(toks) {
    this.toks = toks;
    this.i = 0;
  }
  peek(k = 0) { return this.toks[this.i + k]; }
  next() { return this.toks[this.i++]; }
  is(t, v, k = 0) { const x = this.peek(k); return !!x && x.t === t && (v === undefined || x.v === v); }
  sym(v, k = 0) { return this.is('sym', v, k); }
  cmd(v, k = 0) { return this.is('cmd', v, k); }
  expect(t, v) {
    const x = this.next();
    if (!x || x.t !== t || (v !== undefined && x.v !== v)) throw new Error(`expected ${t} ${v || ''} at offset ${x ? x.at : 'end'}, found ${x ? x.t + ' ' + x.v : 'the end'}`);
    return x;
  }
  /** Skip a balanced { ... } (the opening brace next). */
  skipBraces() {
    this.expect('sym', '{');
    let depth = 1;
    while (depth > 0 && this.peek()) {
      const x = this.next();
      if (x.t === 'sym' && x.v === '{') depth++;
      else if (x.t === 'sym' && x.v === '}') depth--;
    }
  }
  /** A markup's argument (after \markup or a markup command): its plain words, skipped. */
  markupArg() {
    const x = this.peek();
    if (!x) return '';
    if (x.t === 'sym' && x.v === '{') {
      this.next();
      const words = [];
      let depth = 1;
      while (depth > 0 && this.peek()) {
        const y = this.next();
        if (y.t === 'sym' && y.v === '{') depth++;
        else if (y.t === 'sym' && y.v === '}') depth--;
        else if (y.t === 'str' || y.t === 'word') words.push(y.v);
      }
      return words.join(' ');
    }
    if (x.t === 'str' || x.t === 'word' || x.t === 'num') {
      this.next();
      return x.v;
    }
    if (x.t === 'scheme') {
      this.next();
      return this.markupArg();
    }
    if (x.t === 'cmd') {
      this.next();
      return this.markupArg();
    }
    return '';
  }
}

/** A pitch word ("fis", "bes", "ees", "as") as letter (0 = c .. 6 = b) and alteration, or null. */
function pitchWord(w) {
  const m = /^([a-g])((?:is|es|s)*)$/.exec(w);
  if (!m) return null;
  const letter = 'cdefgab'.indexOf(m[1]);
  let alter = 0;
  let rest = m[2];
  if (rest === 's' && (m[1] === 'e' || m[1] === 'a')) return { letter, alter: -1 };
  if (rest.startsWith('s') && (m[1] === 'e' || m[1] === 'a')) {
    alter = -1;
    rest = rest.slice(1);
  }
  while (rest.length) {
    if (rest.startsWith('is')) { alter++; rest = rest.slice(2); }
    else if (rest.startsWith('es')) { alter--; rest = rest.slice(2); }
    else return null;
  }
  return { letter, alter };
}

const PC = [0, 2, 4, 5, 7, 9, 11];

/** A mode's key signature in fifths, from its tonic and its mode word. */
function keyFifths(letter, alter, mode) {
  const majorTonic = mode === 'minor' ? (PC[letter] + alter + 3 + 12) % 12 : (PC[letter] + alter + 12) % 12;
  const table = { 0: 0, 7: 1, 2: 2, 9: 3, 4: 4, 11: 5, 6: 6, 1: 7, 5: -1, 10: -2, 3: -3, 8: -4 };
  // C sharp and D flat, F sharp and G flat share a pitch class: the spelling decides.
  let f = table[majorTonic];
  const flatSpelled = alter < 0 || (alter === 0 && mode !== 'minor' && letter === 3);
  if (majorTonic === 1 && flatSpelled) f = -5;
  if (majorTonic === 6 && (flatSpelled || (mode === 'minor' && alter < 0))) f = -6;
  if (majorTonic === 11 && alter < 0) f = -7;
  return f;
}

/** The alteration a key of `fifths` gives a letter. */
function keyAlter(fifths, letter) {
  const SHARPS = [3, 0, 4, 1, 5, 2, 6];
  const FLATS = [6, 2, 5, 1, 4, 0, 3];
  if (fifths > 0 && SHARPS.slice(0, fifths).includes(letter)) return 1;
  if (fifths < 0 && FLATS.slice(0, -fifths).includes(letter)) return -1;
  return 0;
}

// ---------------------------------------------------------------------------------------------
// The interpreter: a voice's music into events with exact written times.
// ---------------------------------------------------------------------------------------------

const DYNAMICS = new Set(['ppp', 'pp', 'p', 'mp', 'mf', 'f', 'ff', 'fff', 'sf', 'sfz', 'sffz', 'fz', 'rfz', 'rf', 'fp', 'sfp']);
const ARTIC_SHORT = { '.': 'stacc', '>': 'acc', '^': 'marc', '-': 'ten', '!': 'stacciss', _: 'ten-stacc' };
const ARTIC_NAMED = { staccato: 'stacc', accent: 'acc', marcato: 'marc', tenuto: 'ten', staccatissimo: 'stacciss', portato: 'ten-stacc' };
const IGNORED_POST = new Set(['sustainOn', 'sustainOff', 'startTrillSpan', 'stopTrillSpan', 'trill', 'arpeggio', 'prall', 'mordent', 'turn', 'upbow', 'downbow', 'open', 'stopped', 'flageolet', 'laissezVibrer']);
const IGNORED_CMD = new Set([
  'break', 'pageBreak', 'noBreak', 'stemUp', 'stemDown', 'stemNeutral', 'compressFullBarRests', 'RemoveEmptyStaves', 'oneVoice', 'voiceOne', 'voiceTwo', 'voiceThree', 'voiceFour',
  'slurUp', 'slurDown', 'slurNeutral', 'tieUp', 'tieDown', 'tieNeutral', 'dynamicUp', 'dynamicDown', 'dynamicNeutral', 'tupletUp', 'tupletDown', 'autoBeamOff', 'autoBeamOn', 'cadenzaOnEmpty',
  'partPageBreaks', 'unfoldRepeats', 'articulate',
]);

class Interp {
  constructor(vars, settings) {
    this.vars = vars;
    this.settings = settings || {};
    this.problems = [];
  }

  /** Interpret a voice: its token list (a music expression), from written time `start`. */
  voice(toks, ctx) {
    const st = {
      w: ZERO, dur: { base: new F(1, 4), dots: 0 }, rel: null, scale: new F(1), tuplet: null, grace: null,
      events: [], timing: [], layer: 0, ottava: 0, ottavaOpen: null, pendingTie: [], slurOpen: [], hairpinOpen: null, ...ctx,
    };
    st.layerAlloc = ctx.layerAlloc;
    this.seq(new Reader(toks), st, true);
    return st;
  }

  /** One music expression from `r`. */
  expr(r, st) {
    const x = r.peek();
    if (!x) return;
    if (x.t === 'sym' && x.v === '{') {
      r.next();
      this.seq(r, st, false);
      return;
    }
    if (x.t === 'sym' && x.v === '<<') {
      r.next();
      this.simul(r, st);
      return;
    }
    this.item(r, st);
  }

  /** A sequence: items until the closing brace (or the end, for a whole voice). */
  seq(r, st, top) {
    while (r.peek()) {
      if (r.sym('}')) {
        r.next();
        return;
      }
      this.expr(r, st);
    }
    if (!top) throw new Error('a { without its }');
  }

  /** << A \\ B >>: voices together; the first goes on in this layer, the others in layers of their own. */
  simul(r, st) {
    const branches = [];
    let cur = [];
    let depth = 0;
    while (r.peek()) {
      const x = r.peek();
      if (x.t === 'sym' && (x.v === '{' || x.v === '<<')) depth++;
      if (x.t === 'sym' && (x.v === '}' || x.v === '>>')) {
        if (depth === 0 && x.v === '>>') { r.next(); break; }
        depth--;
      }
      if (depth === 0 && x.t === 'cmd' && x.v === '\\') {
        r.next();
        branches.push(cur);
        cur = [];
        continue;
      }
      cur.push(r.next());
    }
    branches.push(cur);
    // Without \\, each expression at the top of << >> is a voice of its own.
    let voices = branches;
    if (branches.length === 1) {
      voices = [];
      const rr = new Reader(branches[0]);
      while (rr.peek()) {
        const s0 = rr.i;
        this.skipExpr(rr);
        voices.push(branches[0].slice(s0, rr.i));
      }
    }
    const start = st.w;
    let end = start;
    voices.forEach((toks, k) => {
      const sub = k === 0 ? st : { ...st, events: st.events, timing: st.timing, layer: st.layerAlloc(st.layer, k), pendingTie: [], slurOpen: [], hairpinOpen: null, ottavaOpen: null };
      sub.w = start;
      this.seq(new Reader(toks), sub, true);
      if (k === 0) {
        end = sub.w;
      } else if (sub.w.gt(end)) {
        end = sub.w;
      }
    });
    st.w = end;
  }

  /** Skip one expression (to split << A B >> into its voices). */
  skipExpr(r) {
    const x = r.next();
    if (!x) return;
    if (x.t === 'sym' && (x.v === '{' || x.v === '<<')) {
      const close = x.v === '{' ? '}' : '>>';
      let depth = 1;
      while (depth > 0 && r.peek()) {
        const y = r.next();
        if (y.t === 'sym' && y.v === x.v) depth++;
        else if (y.t === 'sym' && y.v === close) depth--;
      }
      return;
    }
    if (x.t === 'cmd' && (x.v === 'context' || x.v === 'new')) {
      r.next();
      if (r.sym('=')) { r.next(); r.next(); }
      if (r.cmd('with')) { r.next(); r.skipBraces(); }
      this.skipExpr(r);
      return;
    }
    if (x.t === 'cmd' && x.v === 'relative') {
      if (r.is('word')) { r.next(); while (r.sym("'") || r.sym(',')) r.next(); }
      this.skipExpr(r);
    }
  }

  /** A duration ("4", "8.", "\breve", "1*28", "2*3/4") if one is next. */
  duration(r) {
    let base = null;
    if (r.is('num')) {
      const v = parseInt(r.next().v, 10);
      if (v <= 0 || (v & (v - 1)) !== 0) throw new Error(`a duration of ${v}`);
      base = new F(1, v);
    } else if (r.cmd('breve')) {
      r.next();
      base = new F(2);
    } else if (r.cmd('longa')) {
      r.next();
      base = new F(4);
    } else return null;
    let dots = 0;
    while (r.sym('.')) { r.next(); dots++; }
    let mult = new F(1);
    while (r.sym('*')) {
      r.next();
      const n = parseInt(r.expect('num').v, 10);
      let d = 1;
      if (r.sym('/')) { r.next(); d = parseInt(r.expect('num').v, 10); }
      mult = mult.mul(new F(n, d));
    }
    return { base, dots, mult };
  }

  /** A duration's length in whole notes (dots and multiplier, before any tuplet). */
  static len(d) {
    const p = 1 << d.dots;
    return d.base.mul(new F(2 * p - 1, p)).mul(d.mult || new F(1));
  }

  /** A pitch at `r` (word and octave marks, a forced accidental), resolved against \relative. */
  pitch(r, st, chordRef) {
    const w = r.next();
    const p = pitchWord(w.v);
    if (!p) throw new Error(`"${w.v}" is not a pitch (offset ${w.at})`);
    let marks = 0;
    while (r.sym("'") || r.sym(',')) marks += r.next().v === "'" ? 1 : -1;
    let force = false;
    while (r.sym('!') || r.sym('?')) { r.next(); force = true; }
    let oct;
    const ref = chordRef || st.rel;
    if (ref) {
      const refStep = ref.oct * 7 + ref.letter;
      let best = null;
      for (const o of [ref.oct - 1, ref.oct, ref.oct + 1]) {
        const d = o * 7 + p.letter - refStep;
        if (Math.abs(d) <= 3 && (best === null || Math.abs(d) < Math.abs(best.d))) best = { o, d };
      }
      oct = best.o + marks;
    } else {
      oct = 3 + marks;
    }
    return { letter: p.letter, alter: p.alter, oct, force };
  }

  /** The words a post-event markup holds (\markup { ... }, a string) after its ^ _ or -. */
  postMarkup(r) {
    if (r.cmd('markup')) { r.next(); return r.markupArg(); }
    if (r.is('str')) return r.next().v;
    return null;
  }

  /** Post-events after a note, chord or rest: ties, slurs, dynamics, articulations, a fermata. */
  post(r, ev, st) {
    for (;;) {
      const x = r.peek();
      if (!x) return;
      if (x.t === 'sym' && x.v === '~') { r.next(); ev.tieAll = true; continue; }
      if (x.t === 'sym' && (x.v === '[' || x.v === ']')) { r.next(); continue; }
      if (x.t === 'sym' && x.v === '(') { r.next(); ev.slurStart = (ev.slurStart || 0) + 1; continue; }
      if (x.t === 'sym' && x.v === ')') { r.next(); ev.slurEnd = (ev.slurEnd || 0) + 1; continue; }
      if (x.t === 'sym' && x.v === ':') {
        r.next();
        const n = r.is('num') ? parseInt(r.next().v, 10) : 8;
        ev.tremolo = n;
        continue;
      }
      if (x.t === 'sym' && (x.v === '-' || x.v === '^' || x.v === '_')) {
        const y = r.peek(1);
        if (y && y.t === 'sym' && ARTIC_SHORT[y.v]) {
          r.next();
          r.next();
          ev.artic.push(ARTIC_SHORT[y.v]);
          continue;
        }
        if (y && y.t === 'num') { r.next(); r.next(); continue; } // fingering
        r.next();
        const m = this.postMarkup(r);
        if (m !== null) { ev.words.push(m); continue; }
        continue; // the direction before a \command: read on
      }
      if (x.t === 'cmd') {
        const v = x.v;
        if (v === '\\' || v === 'markup') return;
        if (DYNAMICS.has(v)) { r.next(); ev.dyn.push(v); continue; }
        if (v === '<' || v === '>' || v === 'cresc' || v === 'decresc' || v === 'dim') { r.next(); ev.hairpin = v === '<' || v === 'cresc' ? 'cres' : 'dim'; ev.hairpinWords = v.length > 1; continue; }
        if (v === '!') { r.next(); ev.hairpinEnd = true; continue; }
        // \~ is LilyPond's chant ligature mark (it prints nothing in modern notation); in the
        // symphony's bassoon part (fagotti.ly line 68) it stands where its other voice has a tie
        // (~), so it is read as the tie it was typed for.
        if (v === '~') { r.next(); ev.tieAll = true; continue; }
        if (v === '(' ) { r.next(); ev.slurStart = (ev.slurStart || 0) + 1; continue; }
        if (v === ')') { r.next(); ev.slurEnd = (ev.slurEnd || 0) + 1; continue; }
        if (v === '[' || v === ']') { r.next(); continue; }
        if (v === 'fermata') { r.next(); ev.fermata = true; continue; }
        if (ARTIC_NAMED[v]) { r.next(); ev.artic.push(ARTIC_NAMED[v]); continue; }
        if (IGNORED_POST.has(v)) { r.next(); continue; }
        if (v === 'rest') { r.next(); ev.rest = true; continue; }
        // A user variable holding only a markup or an override, used as a post-event (\pizz).
        if (this.vars[v] && this.vars[v].markup !== undefined) { r.next(); ev.words.push(this.vars[v].markup); continue; }
      }
      return;
    }
  }

  newEvent(st, kind) {
    return { kind, layer: st.layer, w: st.w, pitches: [], artic: [], dyn: [], words: [], grace: st.grace, scale: st.scale, tuplet: st.tuplet };
  }

  /** Place an event: its length (a grace note none), the time moves on. */
  place(st, ev, d) {
    ev.written = d;
    ev.len = st.grace ? ZERO : Interp.len(d).mul(st.scale);
    ev.ottava = st.ottava;
    st.events.push(ev);
    st.w = st.w.add(ev.len);
  }

  /** A note, a chord, a rest or a skip, or a command. */
  item(r, st) {
    const x = r.peek();
    if (x.t === 'word') {
      const w = x.v;
      if (w === 'r' || w === 's' || w === 'R') {
        r.next();
        const d = this.duration(r) || st.dur;
        if (d !== st.dur) st.dur = { base: d.base, dots: d.dots };
        const ev = this.newEvent(st, w === 'r' ? 'rest' : w === 's' ? 'space' : 'mrest');
        this.post(r, ev, st);
        this.place(st, ev, d);
        this.attach(st, ev);
        return;
      }
      if (w === 'q') throw new Error('chord repetition (q) is not read');
      if (pitchWord(w)) {
        r.next();
        r.i--;
        const p = this.pitch(r, st, null);
        if (st.rel) st.rel = { letter: p.letter, oct: p.oct };
        const d = this.duration(r) || st.dur;
        if (d !== st.dur) st.dur = { base: d.base, dots: d.dots };
        const ev = this.newEvent(st, 'note');
        ev.pitches.push(p);
        this.post(r, ev, st);
        if (ev.rest) ev.kind = 'rest';
        if (ev.tremolo) ev.kind = 'note';
        this.place(st, ev, d);
        this.attach(st, ev);
        return;
      }
      // A variable's name standing alone (a music variable written without its backslash is not LilyPond).
      throw new Error(`"${w}" (offset ${x.at}) is not a note, a rest or a skip`);
    }
    if (x.t === 'sym' && x.v === '<') {
      r.next();
      const ev = this.newEvent(st, 'chord');
      let ref = st.rel;
      let first = null;
      while (!r.sym('>')) {
        if (r.is('word')) {
          const p = this.pitch(r, st, ref);
          if (st.rel) ref = { letter: p.letter, oct: p.oct };
          if (!first) first = p;
          if (r.sym('~')) { r.next(); p.tie = true; }
          // Articulations and fingerings written inside the chord, on one note.
          while (r.sym('-') || r.sym('^') || r.sym('_')) {
            r.next();
            if (r.is('num') || r.is('sym')) r.next();
          }
          ev.pitches.push(p);
        } else if (r.cmd()) {
          r.next();
        } else {
          throw new Error(`unexpected ${r.peek().v} in a chord (offset ${r.peek().at})`);
        }
      }
      r.next();
      if (st.rel && first) st.rel = { letter: first.letter, oct: first.oct };
      const d = this.duration(r) || st.dur;
      if (d !== st.dur) st.dur = { base: d.base, dots: d.dots };
      this.post(r, ev, st);
      this.place(st, ev, d);
      this.attach(st, ev);
      return;
    }
    if (x.t === 'sym' && (x.v === '|' || x.v === '~')) { r.next(); return; }
    if (x.t === 'sym' && (x.v === '[' || x.v === ']' || x.v === '(' || x.v === ')')) { r.next(); return; }
    if (x.t === 'cmd') return this.command(r, st);
    if (x.t === 'scheme' || x.t === 'str') { r.next(); return; }
    throw new Error(`unexpected "${x.v}" at offset ${x.at}`);
  }

  /** Ties, slurs, hairpins and octave lines that join this event to those around it. */
  attach(st, ev) {
    if (ev.kind === 'space') return;
    ev.id = null; // given when the MEI is written
    // Ties: from the event before in this layer.
    if (ev.kind === 'note' || ev.kind === 'chord') {
      for (const p of ev.pitches) {
        const from = st.pendingTie.find((q) => q.letter === p.letter && q.oct === p.oct && q.alter === p.alter);
        if (from) {
          p.tieFrom = from;
          from.tieTo = p;
        }
      }
      st.pendingTie = ev.pitches.filter((p) => p.tie || ev.tieAll);
    } else if (ev.kind !== 'space') {
      st.pendingTie = [];
    }
    if (ev.slurEnd) for (let k = 0; k < ev.slurEnd && st.slurOpen.length; k++) st.events.push({ kind: 'slur', layer: st.layer, from: st.slurOpen.pop(), to: ev });
    if (ev.slurStart) for (let k = 0; k < ev.slurStart; k++) st.slurOpen.push(ev);
    // A hairpin ends at \!, at the next dynamic or at the next hairpin.
    if (st.hairpinOpen && (ev.hairpinEnd || ev.dyn.length || ev.hairpin)) {
      st.events.push({ kind: 'hairpin', layer: st.layer, form: st.hairpinOpen.form, words: st.hairpinOpen.words, from: st.hairpinOpen.ev, to: ev });
      st.hairpinOpen = null;
    }
    if (ev.hairpin) st.hairpinOpen = { form: ev.hairpin, words: ev.hairpinWords, ev };
    if (ev.kind !== 'rest' && ev.kind !== 'mrest') {
      if (st.ottava && !st.ottavaOpen) st.ottavaOpen = { by: st.ottava, from: ev };
      if (st.ottavaOpen) st.ottavaOpen.to = ev;
    }
  }

  /** End an octave line (at \ottava #0 or a new \ottava). */
  closeOttava(st) {
    if (st.ottavaOpen && st.ottavaOpen.to) st.events.push({ kind: 'octave', layer: st.layer, by: st.ottavaOpen.by, from: st.ottavaOpen.from, to: st.ottavaOpen.to });
    st.ottavaOpen = null;
  }

  timing(st, kind, extra) {
    st.timing.push({ w: st.w, kind, ...extra });
  }

  command(r, st) {
    const c = r.next().v;
    switch (c) {
      case 'relative': {
        let ref = { letter: 0, oct: 3 }; // \relative with no pitch: f below middle C in 2.18+; c in older files
        if (r.is('word') && pitchWord(r.peek().v)) {
          const p = this.pitch(r, { rel: null }, null);
          ref = { letter: p.letter, oct: p.oct };
        }
        const outer = st.rel;
        st.rel = ref;
        this.expr(r, st);
        st.rel = outer;
        return;
      }
      case 'key': {
        const p = pitchWord(r.expect('word').v);
        const mode = r.next().v;
        const fifths = keyFifths(p.letter, p.alter, mode === 'minor' ? 'minor' : 'major');
        st.events.push({ kind: 'key', layer: st.layer, w: st.w, fifths });
        return;
      }
      case 'time': {
        const n = parseInt(r.expect('num').v, 10);
        r.expect('sym', '/');
        const d = parseInt(r.expect('num').v, 10);
        this.timing(st, 'time', { n, d });
        return;
      }
      case 'partial': {
        const d = this.duration(r);
        this.timing(st, 'partial', { len: Interp.len(d) });
        return;
      }
      case 'clef': {
        const v = r.next().v;
        st.events.push({ kind: 'clef', layer: st.layer, w: st.w, clef: v });
        return;
      }
      case 'tempo': {
        let words = '';
        if (r.is('str')) words = r.next().v;
        else if (r.cmd('markup')) { r.next(); words = r.markupArg(); }
        let qpm = null;
        if (r.is('num')) {
          const unit = parseInt(r.next().v, 10);
          let dots = 0;
          while (r.sym('.')) { r.next(); dots++; }
          r.expect('sym', '=');
          const bpm = parseInt(r.expect('num').v, 10);
          qpm = { mm: bpm, unit, dots };
        }
        st.events.push({ kind: 'tempo', layer: st.layer, w: st.w, words, qpm });
        return;
      }
      case 'transposition': {
        const p = this.pitch(r, { rel: null }, null);
        st.transposition = PC[p.letter] + p.alter + 12 * (p.oct + 1) - 60;
        st.events.push({ kind: 'transposition', layer: st.layer, w: st.w, semis: st.transposition, diat: p.letter + 7 * (p.oct - 4) });
        return;
      }
      case 'ottava': {
        const v = parseInt(r.expect('scheme').v.replace(/#/g, ''), 10);
        this.closeOttava(st);
        st.ottava = v;
        return;
      }
      case 'times':
      case 'tuplet': {
        const n = parseInt(r.expect('num').v, 10);
        r.expect('sym', '/');
        const d = parseInt(r.expect('num').v, 10);
        if (c === 'tuplet' && r.is('num')) r.next(); // a grouping duration
        // \times 2/3 scales by 2/3; \tuplet 3/2 plays 3 in the time of 2.
        const scale = c === 'times' ? new F(n, d) : new F(d, n);
        const outer = { scale: st.scale, tuplet: st.tuplet };
        st.scale = st.scale.mul(scale);
        st.tuplet = { id: (Interp.tupletIds = (Interp.tupletIds || 0) + 1), num: scale.d, numbase: scale.n, outer: outer.tuplet };
        this.expr(r, st);
        st.scale = outer.scale;
        st.tuplet = outer.tuplet;
        return;
      }
      case 'grace':
      case 'appoggiatura':
      case 'acciaccatura': {
        const outer = st.grace;
        st.grace = c === 'appoggiatura' ? 'acc' : 'unacc';
        this.expr(r, st);
        st.grace = outer;
        return;
      }
      case 'repeat': {
        const kind = r.expect('word').v;
        const times = parseInt(r.expect('num').v, 10);
        if (kind === 'volta') {
          this.timing(st, 'repeatStart');
          this.expr(r, st);
          this.timing(st, 'repeatEnd', { times });
          if (r.cmd('alternative')) {
            r.next();
            r.expect('sym', '{');
            let k = 0;
            while (!r.sym('}')) {
              k++;
              this.timing(st, 'altStart', { n: k });
              this.expr(r, st);
              this.timing(st, 'altEnd', { n: k });
            }
            r.next();
          }
          return;
        }
        // unfold and tremolo: written out, each time through the same notes.
        const s0 = r.i;
        const before = st.events.length;
        this.expr(r, st);
        const body = st.events.slice(before).filter((e) => e.len !== undefined);
        const span = body.reduce((a, e) => a.add(e.len), ZERO);
        let scaleEach = new F(1);
        if (kind === 'tremolo') {
          // \repeat tremolo 4 {b16 b'}: the notes as written, four times over.
          scaleEach = new F(1);
        }
        for (let k = 1; k < times; k++) {
          for (const e of body) {
            const copy = { ...e, w: st.w, pitches: e.pitches.map((p) => ({ ...p, tieFrom: undefined, tieTo: undefined })), artic: [...e.artic], dyn: [], words: [] };
            copy.len = e.len.mul(scaleEach);
            st.events.push(copy);
            st.w = st.w.add(copy.len);
          }
        }
        void s0;
        void span;
        return;
      }
      case 'partcombine':
      case 'partCombine': {
        const a = [];
        const b = [];
        const grab = (into) => {
          const s0 = r.i;
          this.skipExpr(r);
          into.push(...r.toks.slice(s0, r.i));
        };
        grab(a);
        grab(b);
        const start = st.w;
        const second = { ...st, layer: st.layerAlloc(st.layer, 1), pendingTie: [], slurOpen: [], hairpinOpen: null, ottavaOpen: null };
        this.seq(new Reader(this.expandVars(a)), st, true);
        const endA = st.w;
        second.w = start;
        this.seq(new Reader(this.expandVars(b)), second, true);
        st.w = endA.gt(second.w) ? endA : second.w;
        return;
      }
      case 'cadenzaOn':
        this.timing(st, 'cadenzaOn');
        return;
      case 'cadenzaOff':
        this.timing(st, 'cadenzaOff');
        return;
      case 'set':
      case 'override':
      case 'revert':
      case 'unset':
      case 'once': {
        if (c === 'once') return; // the \override after it is read next
        // \set A.b = value | \override A.b-c = #value | \override A #'b = #value | \revert A.b
        const words = [];
        if (c === 'revert' || c === 'unset') {
          while (r.is('word') || r.sym('.') || r.sym('-') || r.is('scheme')) words.push(r.next().v);
          return;
        }
        while (r.peek() && !r.sym('=')) words.push(r.next().v);
        if (r.sym('=')) {
          r.next();
          const v = r.peek();
          if (v.t === 'cmd' && v.v === 'markup') { r.next(); r.markupArg(); }
          else {
            r.next();
            if (words.join('') === 'Timing.measurePosition' && v.t === 'scheme') {
              const m = /ly:make-moment\s+(-?\d+)\s*\/?\s*(\d+)?/.exec(v.v);
              if (m) this.timing(st, 'measurePosition', { pos: new F(parseInt(m[1], 10), parseInt(m[2] || '1', 10)) });
            }
            if (words.join('') === 'Staff.instrumentName' && v.t === 'str') st.label = st.label || v.v;
            if (words.join('') === 'Staff.midiInstrument' && v.t === 'str') st.midi = v.v;
          }
        }
        return;
      }
      case 'tupletSpan': {
        this.duration(r);
        return;
      }
      case 'bar':
      case 'mark': {
        if (r.is('str') || r.is('scheme')) r.next();
        else if (r.cmd('default')) r.next();
        else if (r.cmd('markup')) { r.next(); r.markupArg(); }
        return;
      }
      case 'markup':
        r.markupArg();
        return;
      case 'context':
      case 'new': {
        r.next(); // the context type
        if (r.sym('=')) { r.next(); r.next(); }
        if (r.cmd('with')) { r.next(); r.skipBraces(); }
        this.expr(r, st);
        return;
      }
      case '\\':
        return;
      default: {
        if (IGNORED_CMD.has(c) || IGNORED_POST.has(c)) return;
        if (DYNAMICS.has(c)) return; // a dynamic with no note before it
        const v = this.vars[c];
        if (v && v.toks) {
          this.seq(new Reader(v.toks), st, true);
          return;
        }
        if (v && v.markup !== undefined) return;
        throw new Error(`\\${c} is not read (offset ${r.peek(-1).at})`);
      }
    }
  }

  /** A token list with its \variables written in (for \partcombine's two arguments). */
  expandVars(toks) {
    const out = [];
    for (const t of toks) {
      if (t.t === 'cmd' && this.vars[t.v] && this.vars[t.v].toks) out.push({ t: 'sym', v: '{' }, ...this.expandVars(this.vars[t.v].toks), { t: 'sym', v: '}' });
      else out.push(t);
    }
    return out;
  }
}

/** The file's top level: variables, the header, and the first \score's staves. */
function readFile(file) {
  const toks = loadTokens(file);
  const r = new Reader(toks);
  const vars = {};
  const header = {};
  let score = null;
  while (r.peek()) {
    const x = r.peek();
    if (x.t === 'word' && r.sym('=', 1)) {
      r.next();
      r.next();
      const s0 = r.i;
      if (r.cmd('markup')) {
        r.next();
        vars[x.v] = { markup: r.markupArg() };
        continue;
      }
      if (r.is('str')) { vars[x.v] = { markup: r.next().v }; continue; }
      // A music value: one expression (\relative p { }, { }, \override ..., \set ...).
      skipValue(r);
      vars[x.v] = { toks: toks.slice(s0, r.i) };
      continue;
    }
    if (x.t === 'cmd' && x.v === 'header') {
      r.next();
      readHeader(r, header);
      continue;
    }
    if (x.t === 'cmd' && (x.v === 'paper' || x.v === 'layout' || x.v === 'midi')) {
      r.next();
      r.skipBraces();
      continue;
    }
    if (x.t === 'cmd' && x.v === 'version') { r.next(); r.next(); continue; }
    if (x.t === 'cmd' && x.v === 'score') {
      r.next();
      const s0 = r.i;
      r.skipBraces();
      if (!score) score = toks.slice(s0 + 1, r.i - 1);
      continue;
    }
    r.next();
  }
  if (!score) throw new Error(`${file}: no \\score`);
  return { vars, header, score };
}

/** Skip one assigned value (a music expression, possibly a command with its arguments). */
function skipValue(r) {
  const x = r.peek();
  if (x.t === 'sym' && (x.v === '{' || x.v === '<<')) {
    const close = x.v === '{' ? '}' : '>>';
    r.next();
    let depth = 1;
    while (depth > 0 && r.peek()) {
      const y = r.next();
      if (y.t === 'sym' && y.v === x.v) depth++;
      else if (y.t === 'sym' && y.v === close) depth--;
    }
    return;
  }
  if (x.t === 'cmd' && x.v === 'relative') {
    r.next();
    if (r.is('word')) { r.next(); while (r.sym("'") || r.sym(',')) r.next(); }
    skipValue(r);
    return;
  }
  if (x.t === 'cmd' && (x.v === 'override' || x.v === 'set' || x.v === 'once')) {
    r.next();
    if (x.v === 'once') { skipValue(r); return; }
    while (r.peek() && !r.sym('=')) r.next();
    r.next();
    r.next();
    return;
  }
  r.next();
}

function readHeader(r, header) {
  r.expect('sym', '{');
  while (!r.sym('}')) {
    const k = r.next();
    if (k.t === 'word' && r.sym('=')) {
      r.next();
      if (r.is('str')) header[k.v] = r.next().v;
      else if (r.cmd('markup')) { r.next(); r.markupArg(); }
      else r.next();
    } else if (k.t === 'sym' && k.v === '{') {
      let depth = 1;
      while (depth > 0) {
        const y = r.next();
        if (y.t === 'sym' && y.v === '{') depth++;
        if (y.t === 'sym' && y.v === '}') depth--;
      }
    }
  }
  r.next();
}

/**
 * The staves of a \score expression, in order: each `\new Staff` with the voices inside it (a
 * `\context Voice` each, or the one sequence); `\new Dynamics` (the pedal line) is left out.
 */
function findStaves(toks) {
  const staves = [];
  const r = new Reader(toks);
  const walk = (rr) => {
    while (rr.peek()) {
      const x = rr.next();
      if (x.t === 'cmd' && x.v === 'new') {
        const type = rr.next().v;
        let name = null;
        if (rr.sym('=')) { rr.next(); name = rr.next().v; }
        if (rr.cmd('with')) { rr.next(); rr.skipBraces(); }
        const s0 = rr.i;
        skipValue(rr);
        const body = toks.slice(s0 + (rr.toks === toks ? 0 : 0), rr.i);
        if (type === 'Staff') staves.push({ name, body: rr.toks.slice(s0, rr.i) });
        else if (type !== 'Dynamics' && type !== 'Lyrics') walk(new Reader(rr.toks.slice(s0, rr.i)));
        void body;
      }
    }
  };
  walk(r);
  return staves;
}

// ---------------------------------------------------------------------------------------------
// Bars: where the bar lines fall, from every voice's timing (\time, \partial, repeats and their
// endings, cadenzas, Timing.measurePosition), in written time.
// ---------------------------------------------------------------------------------------------

/**
 * The bars of the piece: each {at, len, meter: [n, d], ending, left, right}. A repeat's start and
 * end and each ending's start and end always fall on a bar line (MEI keeps them on bar lines); a
 * cadenza is one bar whatever its length; each ending starts where the first one started in its bar.
 */
function bars(timing, end) {
  const seen = new Set();
  const evs = timing
    .filter((e) => {
      const k = `${e.w}|${e.kind}|${e.n || ''}|${e.len || ''}|${e.pos || ''}`;
      if (seen.has(k)) return false;
      seen.add(k);
      return true;
    })
    .sort((a, b) => a.w.cmp(b.w));
  const out = [];
  let meter = [4, 4];
  let M = new F(1);
  let P = ZERO;
  let W = ZERO;
  let cad = false;
  let altPos = ZERO;
  const lines = [{ at: ZERO, meter }];
  const line = (w) => {
    const last = lines[lines.length - 1];
    if (last.at.eq(w)) last.meter = meter;
    else lines.push({ at: w, meter });
  };
  const repeats = [];
  let k = 0;
  while (W.lt(end) || (k < evs.length && evs[k].w.le(end))) {
    const nextEvt = k < evs.length ? evs[k].w : null;
    const nextBar = cad ? null : W.add(M.sub(P));
    if (nextEvt && (!nextBar || nextEvt.le(nextBar))) {
      if (!cad) P = P.add(nextEvt.sub(W));
      W = nextEvt;
      // A bar that ends here ends before what happens here (a cadenza beginning on a bar line).
      if (!cad && P.ge(M)) {
        line(W);
        P = P.sub(M);
      }
      while (k < evs.length && evs[k].w.eq(W)) {
        const e = evs[k++];
        switch (e.kind) {
          case 'time':
            meter = [e.n, e.d];
            M = new F(e.n, e.d);
            if (!P.isZero() && P.lt(M)) line(W);
            P = ZERO;
            line(W);
            break;
          case 'partial':
            P = M.sub(e.len);
            break;
          case 'repeatStart':
            line(W);
            repeats.push({ start: W, end: null, alts: [] });
            break;
          case 'repeatEnd': {
            line(W);
            const rp = repeats.filter((x) => !x.end).pop();
            if (rp) rp.end = W;
            break;
          }
          case 'altStart': {
            line(W);
            if (e.n === 1) altPos = P;
            else P = altPos;
            const rp = repeats[repeats.length - 1];
            if (rp) rp.alts.push({ n: e.n, start: W, end: null });
            break;
          }
          case 'altEnd': {
            line(W);
            const rp = repeats[repeats.length - 1];
            if (rp && rp.alts.length) rp.alts[rp.alts.length - 1].end = W;
            break;
          }
          // A cadenza draws no bar lines and the bar does not move on through it (as LilyPond
          // counts it): the bar it is in lasts as long as it does.
          case 'cadenzaOn':
            cad = true;
            break;
          case 'cadenzaOff':
            cad = false;
            break;
          case 'measurePosition':
            P = e.pos.lt(ZERO) ? M.add(e.pos) : e.pos;
            break;
          default:
            break;
        }
      }
      if (!cad && P.ge(M)) {
        line(W);
        P = P.sub(M);
      }
      continue;
    }
    if (!nextBar || nextBar.gt(end)) break;
    W = nextBar;
    line(W);
    P = ZERO;
  }
  if (!lines[lines.length - 1].at.lt(end)) lines.pop();
  for (let i = 0; i < lines.length; i++) {
    const at = lines[i].at;
    const to = i + 1 < lines.length ? lines[i + 1].at : end;
    if (!to.gt(at)) continue;
    const bar = { at, len: to.sub(at), meter: lines[i].meter, ending: null, left: null, right: null };
    for (const rp of repeats) {
      if (rp.start.eq(at) && at.gt(ZERO)) bar.left = 'rptstart';
      for (const a of rp.alts) if (a.end && at.ge(a.start) && at.lt(a.end)) bar.ending = a.n;
      if (!rp.alts.length && rp.end && to.eq(rp.end)) bar.right = 'rptend';
      rp.alts.forEach((a, i2) => {
        if (i2 < rp.alts.length - 1 && a.end && to.eq(a.end)) bar.right = 'rptend';
      });
    }
    out.push(bar);
  }
  if (out.length) out[out.length - 1].right = out[out.length - 1].right || 'end';
  return out;
}

// ---------------------------------------------------------------------------------------------
// Writing MEI.
// ---------------------------------------------------------------------------------------------

const esc = (s) => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

/** A length (whole notes) as notated values: each {dur, dots}, tied (a power of two at most). */
function notate(len) {
  const out = [];
  let rest = len;
  let guard = 0;
  while (rest.gt(ZERO) && guard++ < 16) {
    let v = new F(4);
    while (v.gt(rest)) v = v.div(new F(2));
    if (v.lt(new F(1, 2048))) throw new Error(`a length of ${len} cannot be written as notes`);
    let dots = 0;
    let take = v;
    if (rest.ge(v.mul(new F(3, 2)))) { dots = 1; take = v.mul(new F(3, 2)); }
    out.push({ v, dots });
    rest = rest.sub(take);
  }
  if (rest.gt(ZERO)) throw new Error(`a length of ${len} cannot be written as notes`);
  return out;
}

function durAttr(v) {
  if (v.eq(new F(4))) return 'long';
  if (v.eq(new F(2))) return 'breve';
  return String(v.d / v.n);
}

const ACCID = { 1: 's', '-1': 'f', 0: 'n', 2: 'x', '-2': 'ff' };
const CLEFS = { treble: ['G', 2], violin: ['G', 2], G: ['G', 2], bass: ['F', 4], F: ['F', 4], tenor: ['C', 4], alto: ['C', 3], C: ['C', 3], soprano: ['C', 1], percussion: ['perc', 3] };

/** Convert one piece: its main file and its settings (ly-to-mei.pieces.json). The MEI text. */
function convert(mainFile, settings) {
  const { vars, header, score } = readFile(mainFile);
  const interp = new Interp(vars, settings);
  const staves = findStaves(score);
  const timing = [];
  const out = [];
  staves.forEach((sf, si) => {
    const slots = new Map();
    let nextSlot = 1;
    const layerAlloc = (parent, k) => {
      const key = `${parent}:${k}`;
      if (!slots.has(key)) slots.set(key, nextSlot++);
      return slots.get(key);
    };
    const st = interp.voice(sf.body, { layerAlloc, timing });
    interp.closeOttava(st);
    out.push({ n: si + 1, name: sf.name, st });
  });
  const end = out.reduce((a, s) => (s.st.w.gt(a) ? s.st.w : a), ZERO);
  const barList = bars(timing, end);
  return writeMei({ staves: out, bars: barList, header, settings, end });
}

/** The bar holding written time `w` (an index), the last bar for the very end. */
function barAt(barList, w) {
  let lo = 0;
  let hi = barList.length - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (barList[mid].at.le(w)) lo = mid;
    else hi = mid - 1;
  }
  return lo;
}

function writeMei({ staves, bars: barList, header, settings }) {
  let nextId = 0;
  const newId = (p) => `${p}${++nextId}`;
  const lines = [];
  const problems = [];
  // Per staff: its layers (slots with something written, by first appearance), its first clef, key,
  // transposition and label; its events cut at the bar lines.
  const staffInfo = staves.map((s) => {
    const evs = s.st.events;
    const sounding = evs.filter((e) => ['note', 'chord', 'rest', 'mrest'].includes(e.kind));
    const order = [];
    for (const e of sounding) if (!order.includes(e.layer)) order.push(e.layer);
    const layerOf = (slot) => (order.indexOf(slot) >= 0 ? order.indexOf(slot) + 1 : 1);
    const firstSound = sounding.length ? sounding[0].w : ZERO;
    const clefs = evs.filter((e) => e.kind === 'clef');
    const initialClef = clefs.find((c) => c.w.le(firstSound) && c.w.isZero()) || { clef: 'treble' };
    const keys = evs.filter((e) => e.kind === 'key');
    const key = keys.length ? keys[0].fifths : 0;
    const trans = evs.find((e) => e.kind === 'transposition');
    const conf = (settings.staves || {})[String(s.n)] || {};
    return { s, evs, layerOf, layers: order.length, initialClef, key, trans, conf, laterClefs: clefs.filter((c) => c !== initialClef) };
  });
  // Ids for everything a control event may point at.
  for (const info of staffInfo) {
    for (const e of info.evs) {
      if (e.kind === 'note' || e.kind === 'chord' || e.kind === 'rest' || e.kind === 'mrest') {
        e.id = newId(e.kind === 'rest' || e.kind === 'mrest' ? 'r' : e.kind === 'chord' ? 'c' : 'n');
        for (const p of e.pitches) p.id = e.kind === 'chord' ? newId('n') : e.id;
      }
    }
  }
  const refId = (e) => (e.kind === 'chord' ? e.pitches[0].id : e.id);
  const tstamp = (e, bar) => {
    const beats = e.w.sub(bar.at).mul(new F(bar.meter[1])).add(new F(1));
    // Only a time a decimal writes exactly (the reader then reads it back exactly).
    let d = beats.d;
    while (d % 2 === 0) d /= 2;
    while (d % 5 === 0) d /= 5;
    if (d !== 1) return '';
    const v = beats.num();
    return ` tstamp="${Number(v.toFixed(6))}"`;
  };
  const fifthsAttr = (f) => (f === 0 ? '0' : f > 0 ? `${f}s` : `${-f}f`);
  // The header.
  const title = settings.title || header.title || '';
  lines.push('<?xml version="1.0" encoding="UTF-8"?>');
  lines.push('<?xml-model href="https://music-encoding.org/schema/5.1/mei-basic.rng" type="application/xml" schematypens="http://relaxng.org/ns/structure/1.0"?>');
  lines.push('<!--');
  for (const l of settings.comment || []) lines.push(`  ${l.replace(/--/g, '- -')}`);
  lines.push('-->');
  lines.push('<mei xmlns="http://www.music-encoding.org/ns/mei" meiversion="5.1+basic">');
  lines.push('  <meiHead>');
  lines.push('    <fileDesc>');
  lines.push('      <titleStmt>');
  lines.push(`        <title>${esc(title)}</title>`);
  lines.push(`        <composer>${esc(settings.composer || header.composer || '')}</composer>`);
  lines.push('      </titleStmt>');
  lines.push('      <pubStmt>');
  lines.push(`        <availability>${esc(settings.availability || '')}</availability>`);
  lines.push('      </pubStmt>');
  lines.push('    </fileDesc>');
  lines.push('  </meiHead>');
  lines.push('  <music>');
  lines.push('    <body>');
  lines.push('      <mdiv>');
  lines.push('        <score>');
  const m0 = barList[0] ? barList.find((b) => b.len.eq(new F(b.meter[0], b.meter[1]))) || barList[0] : { meter: [4, 4] };
  lines.push(`          <scoreDef keysig="${fifthsAttr(staffInfo[0] ? staffInfo[0].key : 0)}" meter.count="${m0.meter[0]}" meter.unit="${m0.meter[1]}">`);
  lines.push('            <staffGrp>');
  for (const info of staffInfo) {
    const [shape, lineNo] = CLEFS[info.initialClef.clef] || CLEFS.treble;
    const label = info.conf.label || info.s.st.label || info.s.name || `Staff ${info.s.n}`;
    let trans = '';
    const semis = info.conf.transSemi !== undefined ? info.conf.transSemi : info.trans ? info.trans.semis : 0;
    const diat = info.conf.transDiat !== undefined ? info.conf.transDiat : info.trans ? info.trans.diat : 0;
    if (semis) trans = ` trans.semi="${semis}" trans.diat="${diat}"`;
    lines.push(`              <staffDef n="${info.s.n}" lines="5" clef.shape="${shape}" clef.line="${lineNo}" keysig="${fifthsAttr(info.key)}"${trans} label="${esc(label)}"/>`);
  }
  lines.push('            </staffGrp>');
  lines.push('          </scoreDef>');
  lines.push('          <section>');
  // Cut each staff's events at the bar lines: a whole-bar rest becomes a whole-bar rest in each bar,
  // a note held over a bar line two notes tied.
  const perBar = staffInfo.map(() => barList.map(() => []));
  staffInfo.forEach((info, si) => {
    let seq = 0;
    for (const e of info.evs) {
      e.seq = seq++;
      if (e.len === undefined || e.kind === 'space') continue;
      let b = barAt(barList, e.w);
      for (const p of e.pitches) p.bar = b;
      if (e.len.isZero()) {
        perBar[si][b].push(e);
        continue;
      }
      let w = e.w;
      let left = e.len;
      let piece = 0;
      while (left.gt(ZERO)) {
        const bar = barList[b];
        const room = bar.at.add(bar.len).sub(w);
        const take = fmin(left, room);
        const part = piece === 0 && take.eq(left) ? e : { ...e, w, len: take, split: true, pitches: e.pitches.map((p) => ({ ...p })) };
        if (part !== e) {
          part.id = newId(e.id ? e.id[0] : 'n');
          part.pitches.forEach((p, i) => {
            p.id = e.kind === 'chord' ? newId('n') : part.id;
            p.splitFrom = piece > 0 ? true : p.splitFrom;
            p.splitTo = take.lt(left) ? true : undefined;
            if (piece === 0) e.pitches[i].firstPiece = p;
            else e.pitches[i].lastPiece = p;
          });
          if (piece === 0) { e.firstPart = part; }
          e.lastPart = part;
        }
        perBar[si][b].push(part);
        w = w.add(take);
        left = left.sub(take);
        piece++;
        b++;
        if (left.gt(ZERO) && b >= barList.length) throw new Error(`staff ${info.s.n}: music past the last bar`);
      }
    }
  });
  assignAccidentals(staffInfo, perBar, problems);
  // Control events, by the bar their first event is in. A tempo is written once for the piece at
  // each moment (every voice of a piano part may carry the same \tempo).
  const controls = barList.map(() => []);
  const tempoAt = new Set();
  const startOf = (e) => e.firstPart || e;
  const endOf = (e) => e.lastPart || e;
  staffInfo.forEach((info) => {
    const n = info.s.n;
    for (const e of info.evs) {
      if (e.kind === 'slur' || e.kind === 'hairpin' || e.kind === 'octave') {
        const a = startOf(e.from);
        const z = endOf(e.to);
        if (!a.id || !z.id) continue;
        const bi = barAt(barList, a.w);
        if (e.kind === 'slur') controls[bi].push(`<slur staff="${n}" startid="#${refId(a)}" endid="#${refId(z)}"/>`);
        else if (e.kind === 'hairpin') {
          if (e.words) controls[bi].push(`<dynam staff="${n}"${tstamp(a, barList[bi])} startid="#${refId(a)}" endid="#${refId(z)}">${e.form === 'cres' ? 'cresc.' : 'dim.'}</dynam>`);
          else controls[bi].push(`<hairpin form="${e.form}" staff="${n}"${tstamp(a, barList[bi])} startid="#${refId(a)}" endid="#${refId(z)}"/>`);
        } else {
          const dis = { 1: 8, 2: 15, 3: 22 }[Math.abs(e.by)];
          if (dis) controls[bi].push(`<octave staff="${n}" dis="${dis}" dis.place="${e.by > 0 ? 'above' : 'below'}" startid="#${refId(a)}" endid="#${refId(z)}"/>`);
        }
        continue;
      }
      if (e.kind === 'tempo') {
        if (tempoAt.has(e.w.toString())) continue;
        tempoAt.add(e.w.toString());
        const bi = barAt(barList, e.w);
        let attrs = '';
        if (e.qpm) {
          const qpm = e.qpm.mm * (4 / e.qpm.unit) * (2 - Math.pow(0.5, e.qpm.dots));
          attrs = ` mm="${e.qpm.mm}" mm.unit="${e.qpm.unit}"${e.qpm.dots ? ` mm.dots="${e.qpm.dots}"` : ''} midi.bpm="${Number(qpm.toFixed(3))}"`;
        }
        controls[bi].push(`<tempo staff="${n}"${tstamp(e, barList[bi]) || ' tstamp="1"'}${attrs}>${esc(e.words || '')}</tempo>`);
        continue;
      }
      if (e.kind === 'space' && e.words && e.words.length) {
        // A tempo word over a skip (a conductor's line of skips carries the movement's tempo).
        for (const word of e.words) {
          const bpm = (settings.tempoWords || {})[word.trim()];
          if (!bpm || tempoAt.has(e.w.toString())) continue;
          tempoAt.add(e.w.toString());
          const bi = barAt(barList, e.w);
          controls[bi].push(`<tempo staff="${n}"${tstamp(e, barList[bi]) || ' tstamp="1"'} midi.bpm="${bpm}">${esc(word.trim())}</tempo>`);
        }
        continue;
      }
      if (!e.id) continue;
      const a = startOf(e);
      const bi = barAt(barList, a.w);
      for (const d of e.dyn) controls[bi].push(`<dynam staff="${n}"${tstamp(a, barList[bi])} startid="#${refId(a)}">${d}</dynam>`);
      if (e.fermata) controls[bi].push(`<fermata staff="${n}"${tstamp(a, barList[bi])} startid="#${refId(a)}"/>`);
      for (const word of e.words) {
        const bpm = (settings.tempoWords || {})[word.trim()];
        if (!bpm || tempoAt.has(a.w.toString())) continue;
        tempoAt.add(a.w.toString());
        controls[bi].push(`<tempo staff="${n}"${tstamp(a, barList[bi])} startid="#${refId(a)}" midi.bpm="${bpm}">${esc(word.trim())}</tempo>`);
      }
    }
  });
  // A tempo back after a cadenza, when the settings give one (the edition marks the cadenza's tempo
  // in words only).
  if (settings.afterCadenzaQpm) {
    const ends = new Set();
    for (const info of staffInfo) for (const t of info.s.st.timing) if (t.kind === 'cadenzaOff') ends.add(t.w.toString());
    for (const w of ends) {
      const [a, b] = w.split('/').map(Number);
      const at = new F(a, b || 1);
      const bi = barAt(barList, at);
      controls[bi].push(`<tempo staff="1"${tstamp({ w: at }, barList[bi]) || ' tstamp="1"'} midi.bpm="${settings.afterCadenzaQpm}"/>`);
    }
  }
  // The bars.
  let barNo = barList.length && barList[0].len.lt(new F(barList[0].meter[0], barList[0].meter[1])) ? 0 : 1;
  let openEnding = null;
  const ind = '            ';
  barList.forEach((bar, bi) => {
    if (bi > 0 && (bar.meter[0] !== barList[bi - 1].meter[0] || bar.meter[1] !== barList[bi - 1].meter[1]) && bar.len.eq(new F(bar.meter[0], bar.meter[1]))) {
      lines.push(`${ind}<scoreDef meter.count="${bar.meter[0]}" meter.unit="${bar.meter[1]}"/>`);
    }
    if (openEnding !== null && bar.ending !== openEnding) {
      lines.push(`${ind}</ending>`);
      openEnding = null;
    }
    if (bar.ending !== null && openEnding === null) {
      lines.push(`${ind}<ending n="${bar.ending}">`);
      openEnding = bar.ending;
    }
    const attrs = `${bar.left ? ` left="${bar.left}"` : ''}${bar.right ? ` right="${bar.right}"` : ''}`;
    lines.push(`${ind}<measure n="${barNo}"${attrs}>`);
    barNo++;
    staffInfo.forEach((info, si) => {
      const evs = perBar[si][bi];
      const layers = new Map();
      for (const e of evs) {
        const l = info.layerOf(e.layer);
        if (!layers.has(l)) layers.set(l, []);
        layers.get(l).push(e);
      }
      for (const c of info.laterClefs) {
        if (barAt(barList, c.w) !== bi) continue;
        const l = info.layerOf(c.layer);
        if (!layers.has(l)) layers.set(l, []);
        layers.get(l).push({ ...c, len: ZERO, seq: c.seq === undefined ? -1 : c.seq });
      }
      lines.push(`${ind}  <staff n="${info.s.n}">`);
      const keys = [...layers.keys()].sort((a, b) => a - b);
      if (!keys.length) {
        lines.push(`${ind}    <layer n="1"><mRest/></layer>`);
      }
      for (const l of keys) {
        const list = layers.get(l).sort((a, b) => a.w.cmp(b.w) || a.seq - b.seq);
        lines.push(`${ind}    <layer n="${l}">${writeLayer(list, bar, info, problems)}</layer>`);
      }
      lines.push(`${ind}  </staff>`);
    });
    for (const c of controls[bi]) lines.push(`${ind}  ${c}`);
    lines.push(`${ind}</measure>`);
  });
  if (openEnding !== null) lines.push(`${ind}</ending>`);
  lines.push('          </section>');
  lines.push('        </score>');
  lines.push('      </mdiv>');
  lines.push('    </body>');
  lines.push('  </music>');
  lines.push('</mei>');
  if (problems.length) throw new Error(problems.join('\n'));
  return lines.join('\n') + '\n';
}

/** One layer of one bar: its events in order, gaps as spaces, tuplets around their notes, and each
 * accidental written where an engraver writes it (where the key and the bar so far do not give it). */
function writeLayer(list, bar, info, problems) {
  let t = bar.at;
  const parts = [];
  let tup = null;
  let tupItems = [];
  const flush = () => {
    if (tup) parts.push(`<tuplet num="${tup.num}" numbase="${tup.numbase}">${tupItems.join('')}</tuplet>`);
    tup = null;
    tupItems = [];
  };
  const emit = (s, tuplet) => {
    const id = tuplet && !tuplet.scaleOne ? tuplet.id : null;
    if ((tup ? tup.id : null) !== id) flush();
    if (id) {
      if (!tup) tup = tuplet;
      tupItems.push(s);
    } else parts.push(s);
  };
  const accidOf = (p) => (p.accid ? ` accid="${p.accid}"` : '');
  for (const e of list) {
    if (e.kind === 'clef') {
      const [shape, line] = CLEFS[e.clef] || CLEFS.treble;
      emit(`<clef shape="${shape}" line="${line}"/>`, null);
      continue;
    }
    if (e.w.gt(t)) {
      flush();
      for (const n of notate(e.w.sub(t))) parts.push(`<space dur="${durAttr(n.v)}"${n.dots ? ` dots="${n.dots}"` : ''}/>`);
      t = e.w;
    }
    const scale = e.scale || new F(1);
    const tuplet = e.tuplet ? { ...e.tuplet, scaleOne: scale.eq(new F(1)) } : null;
    // Written values: as the source writes them, unless the event was cut at a bar line.
    let durs;
    if (!e.split && e.written && (!e.written.mult || e.written.mult.eq(new F(1))) && e.kind !== 'mrest') {
      durs = [{ v: e.written.base, dots: e.written.dots }];
    } else if (e.kind === 'mrest' && e.len.eq(bar.len)) {
      durs = null;
    } else {
      durs = notate(e.len.div(scale));
    }
    const grace = e.grace ? ` grace="${e.grace}"` : '';
    const artic = [...new Set(e.artic || [])].join(' ');
    const articA = artic ? ` artic="${artic}"` : '';
    if (e.kind === 'mrest') {
      if (!durs) emit(`<mRest xml:id="${e.id}"/>`, null);
      else durs.forEach((d, i) => emit(`<rest${i === 0 ? ` xml:id="${e.id}"` : ''} dur="${durAttr(d.v)}"${d.dots ? ` dots="${d.dots}"` : ''}/>`, tuplet));
    } else if (e.kind === 'rest') {
      durs.forEach((d, i) => emit(`<rest${i === 0 ? ` xml:id="${e.id}"` : ''} dur="${durAttr(d.v)}"${d.dots ? ` dots="${d.dots}"` : ''}/>`, tuplet));
    } else {
      if (durs.length > 1) problems.push(`staff ${info.s.n}: a note of ${e.len} in one bar needs ${durs.length} tied values (not written yet)`);
      const d = durs[0];
      const durA = ` dur="${durAttr(d.v)}"${d.dots ? ` dots="${d.dots}"` : ''}`;
      const tieOf = (p) => {
        const from = p.tieFrom || p.splitFrom;
        const to = p.tieTo || p.splitTo;
        return from && to ? ' tie="m"' : from ? ' tie="t"' : to ? ' tie="i"' : '';
      };
      const noteXml = (p, own) => `<note xml:id="${p.id}" pname="${'cdefgab'[p.letter]}" oct="${p.oct - (e.ottava || 0)}"${own ? durA : ''}${accidOf(p)}${tieOf(p)}${own ? grace + articA : ''}/>`;
      let xml;
      if (e.kind === 'chord') {
        xml = `<chord xml:id="${e.id}"${durA}${grace}${articA}>${e.pitches.map((p) => noteXml(p, false)).join('')}</chord>`;
      } else {
        xml = noteXml(e.pitches[0], true);
      }
      if (e.tremolo && !e.grace) xml = `<bTrem unitdur="${e.tremolo}">${xml}</bTrem>`;
      emit(xml, tuplet);
    }
    t = e.w.add(e.len);
  }
  flush();
  // Each layer reaches its bar line (a reader takes a bar's length from its longest layer; a source's
  // closing skips, like the symphony's half bar after the cadenza, make no events of their own).
  const end = bar.at.add(bar.len);
  if (t.lt(end) && list.some((e) => e.kind !== 'clef')) {
    for (const n of notate(end.sub(t))) parts.push(`<space dur="${durAttr(n.v)}"${n.dots ? ` dots="${n.dots}"` : ''}/>`);
  }
  return parts.join('');
}

/**
 * Each note's written accidental, as an engraver writes it and as the reader reads it back
 * (crates/humanity-music/src/mei.rs, resolve_pitches): per staff and bar, in time order (the first
 * layer before the second at one moment), an accidental where the key and the accidentals earlier in
 * the bar on that letter and octave do not already give the pitch; none on a note tied in from the
 * bar before (the tie carries its pitch, and leaves the bar's accidentals alone).
 */
function assignAccidentals(staffInfo, perBar, problems) {
  staffInfo.forEach((info, si) => {
    perBar[si].forEach((evs, bi) => {
      const list = evs.filter((e) => e.kind === 'note' || e.kind === 'chord').sort((a, b) => a.w.cmp(b.w) || info.layerOf(a.layer) - info.layerOf(b.layer) || a.seq - b.seq);
      const state = new Map();
      for (const e of list) {
        for (const p of e.pitches) {
          // Under an octave line the note is written an octave from where it sounds (LilyPond writes
          // the sounding pitch; MEI's oct is the written one): its accidental is the written note's.
          const key = `${p.letter}:${p.oct - (e.ottava || 0)}`;
          const tiedIn = p.splitFrom || (p.tieFrom && p.tieFrom.bar !== undefined && p.tieFrom.bar < bi);
          p.accid = '';
          if (tiedIn) continue;
          const cur = state.has(key) ? state.get(key) : keyAlter(info.key, p.letter);
          if (p.alter !== cur || p.force) {
            state.set(key, p.alter);
            p.accid = ACCID[p.alter];
            if (p.accid === undefined) problems.push(`staff ${info.s.n}: an alteration of ${p.alter} cannot be written`);
          }
        }
      }
    });
  });
}

// ---------------------------------------------------------------------------------------------
// The command line.
// ---------------------------------------------------------------------------------------------

function sha256(file) {
  return crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
}

function main(argv) {
  const ROOT = path.resolve(__dirname, '..');
  const conf = JSON.parse(fs.readFileSync(path.join(__dirname, 'ly-to-mei.pieces.json'), 'utf8'));
  const arg = (k) => { const i = argv.indexOf(k); return i >= 0 ? argv[i + 1] : null; };
  const src = arg('--src');
  if (!src) throw new Error('--src <folder with the downloaded LilyPond files> is needed');
  const ids = argv.includes('--all') ? Object.keys(conf.pieces) : [arg('--piece')];
  for (const id of ids) {
    const p = conf.pieces[id];
    if (!p) throw new Error(`no piece ${id} in ly-to-mei.pieces.json`);
    for (const [file, want] of Object.entries(p.sha256)) {
      const got = sha256(path.join(src, file));
      if (got !== want) throw new Error(`${file}: SHA-256 ${got}, the pieces file says ${want}`);
    }
    const mei = convert(path.join(src, p.main), p);
    const out = path.join(ROOT, 'data', 'music', 'scores', `${id}.mei`);
    fs.writeFileSync(out, mei);
    console.log(`wrote ${path.relative(ROOT, out)} (${mei.length} bytes)`);
  }
}

if (require.main === module) {
  try {
    main(process.argv.slice(2));
  } catch (e) {
    console.error(e.message);
    process.exit(1);
  }
}

module.exports = { F, tokenize, pitchWord, keyFifths, keyAlter, Interp, readFile, findStaves, Reader, bars, notate, convert };
