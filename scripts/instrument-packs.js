#!/usr/bin/env node
'use strict';
/**
 * instrument-packs.js (`just instrument-packs`, 2026-11-06; docs/design/music.md rung 6, the
 * operator's "let's build up our free library" and "Let's go all in"): turn the raw CC0 sample
 * libraries into the app's instruments.
 *
 * Reads the raw libraries (downloaded once to E:\HumanityOS\instruments-src, never into git; what
 * was downloaded, from where and when is docs/reference/instrument-sources.md) and the recipes in
 * scripts/instrument-recipes.json, then for each instrument:
 *
 * 1. picks the samples it needs: velocity layers, round robins and a key spread, from the
 *    library's SFZ mapping or its file names;
 * 2. decodes each with ffmpeg, trims trailing silence and over-long tails (with a fade), and
 *    MEASURES its pitch (YIN, scripts/lib/instrument-build.js), so the definition records the
 *    library's real source tuning: nothing is retuned on disk; the engine plays every sample at
 *    the rate that puts it at A = 432 Hz (or 440) when it plays (music.md section 9);
 * 3. encodes it as Ogg Vorbis (kira and symphonia decode it today; a fraction of FLAC's size,
 *    which is what keeps the shipped core set small; deterministic, so packs rebuild byte for byte);
 * 4. writes its definition (data/music/instruments/<id>.ron shape, music.md section 6) with the
 *    licence, the source and the credit, and its sample folder with the library's licence text and a
 *    source.ron saying which original file each kept sample came from.
 *
 * The CORE instruments land in the repository's data/music (they ship inside the app). Every other
 * pack becomes ONE tar archive under --packs-out with its SHA-256, and data/music/packs.ron (the
 * catalog Settings > Audio > Instruments reads) is rewritten with each pack's size and checksum.
 * Uploading the archives is the operator's step: docs/ai/release.md, "Instrument packs".
 *
 * Usage:
 *   node scripts/instrument-packs.js [--src DIR] [--recipes FILE] [--core-out DIR]
 *        [--packs-out DIR] [--catalog FILE] [--only id,id] [--jobs N] [--no-catalog]
 *
 * Needs ffmpeg with libvorbis on PATH.
 */

const fs = require('fs');
const os = require('os');
const path = require('path');
const crypto = require('crypto');
const { spawn, execFileSync } = require('child_process');
const { readSevenZip } = require('./lib/sevenzip');
const B = require('./lib/instrument-build');

const ROOT = path.resolve(__dirname, '..');

/** The tar that reads zip and tar.gz and takes a drive letter: Windows' own bsdtar, not the GNU
 * tar the bash shell puts first on PATH (which reads "E:" as a remote host). */
function tarCommand() {
  const sys = path.join(process.env.SystemRoot || 'C:\\Windows', 'System32', 'tar.exe');
  return process.platform === 'win32' && fs.existsSync(sys) ? sys : 'tar';
}

function parseArgs(argv) {
  const a = {
    src: process.env.HUMANITY_INSTRUMENTS_SRC || 'E:\\HumanityOS\\instruments-src',
    recipes: path.join(ROOT, 'scripts', 'instrument-recipes.json'),
    coreOut: path.join(ROOT, 'data', 'music'),
    packsOut: process.env.HUMANITY_INSTRUMENT_PACKS_OUT || 'E:\\HumanityOS\\instrument-packs',
    catalog: path.join(ROOT, 'data', 'music', 'packs.ron'),
    only: null,
    jobs: Math.max(1, Math.min(4, os.cpus().length - 2)),
    writeCatalog: true,
    quiet: false,
  };
  for (let i = 0; i < argv.length; i++) {
    const k = argv[i];
    const next = () => argv[++i];
    if (k === '--src') a.src = next();
    else if (k === '--recipes') a.recipes = next();
    else if (k === '--core-out') a.coreOut = next();
    else if (k === '--packs-out') a.packsOut = next();
    else if (k === '--catalog') a.catalog = next();
    else if (k === '--only') a.only = new Set(next().split(','));
    else if (k === '--jobs') a.jobs = Math.max(1, Number(next()) || 1);
    else if (k === '--no-catalog') a.writeCatalog = false;
    else if (k === '--quiet') a.quiet = true;
    else throw new Error(`unknown argument ${k}`);
  }
  return a;
}

// ── Running ffmpeg ───────────────────────────────────────────────────────────────────────────

function run(cmd, args, input) {
  return new Promise((resolve, reject) => {
    const p = spawn(cmd, args, { stdio: ['pipe', 'pipe', 'pipe'], windowsHide: true });
    const out = [];
    const err = [];
    p.stdout.on('data', (d) => out.push(d));
    p.stderr.on('data', (d) => err.push(d));
    p.on('error', reject);
    p.on('close', (code) => {
      if (code === 0) resolve(Buffer.concat(out));
      else reject(new Error(`${cmd} ${args.slice(0, 6).join(' ')} ... failed (${code}): ${Buffer.concat(err).toString().slice(0, 400)}`));
    });
    p.stdin.on('error', () => {});
    if (input) p.stdin.end(input);
    else p.stdin.end();
  });
}

function checkFfmpeg() {
  let encoders;
  try {
    encoders = execFileSync('ffmpeg', ['-hide_banner', '-encoders'], { encoding: 'utf8', windowsHide: true });
  } catch (e) {
    throw new Error('the instrument pipeline needs ffmpeg on PATH (with libvorbis): ' + e.message);
  }
  if (!/libvorbis/.test(encoders)) throw new Error('ffmpeg is on PATH but was built without libvorbis');
}

/** Decode `input` (a path, or the bytes of a file) to interleaved float samples. */
async function decode(input, channels, rate) {
  const args = ['-v', 'error', '-i', Buffer.isBuffer(input) ? 'pipe:0' : input, '-f', 'f32le', '-acodec', 'pcm_f32le', '-ac', String(channels)];
  if (rate) args.push('-ar', String(rate));
  args.push('pipe:1');
  const raw = await run('ffmpeg', args, Buffer.isBuffer(input) ? input : null);
  return new Float32Array(raw.buffer, raw.byteOffset, Math.floor(raw.length / 4));
}

/** Encode interleaved float samples to Ogg Vorbis, the same bytes every time (bitexact). */
async function encodeVorbis(samples, channels, rate, quality, outPath) {
  const pcm = Buffer.from(samples.buffer, samples.byteOffset, samples.length * 4);
  await run('ffmpeg', ['-v', 'error', '-f', 'f32le', '-ar', String(rate), '-ac', String(channels), '-i', 'pipe:0',
    '-c:a', 'libvorbis', '-q:a', String(quality), '-map_metadata', '-1', '-fflags', '+bitexact', '-flags:a', '+bitexact',
    '-y', outPath], pcm);
}

async function pool(items, jobs, fn) {
  const results = new Array(items.length);
  let next = 0;
  async function worker() {
    while (next < items.length) {
      const i = next++;
      results[i] = await fn(items[i], i);
    }
  }
  await Promise.all(Array.from({ length: Math.min(jobs, items.length) }, worker));
  return results;
}

// ── The raw libraries ────────────────────────────────────────────────────────────────────────

function sha256File(p) {
  const h = crypto.createHash('sha256');
  const fd = fs.openSync(p, 'r');
  const buf = Buffer.alloc(1 << 22);
  try {
    for (;;) {
      const n = fs.readSync(fd, buf, 0, buf.length, null);
      if (n === 0) break;
      h.update(buf.subarray(0, n));
    }
  } finally {
    fs.closeSync(fd);
  }
  return h.digest('hex');
}

/**
 * A library's files, however it was downloaded: a .7z read into memory, or an archive (zip, tar.gz)
 * unpacked once beside itself. The archive's size and SHA-256 must match what the recipe recorded
 * when it was downloaded (docs/reference/instrument-sources.md); a changed download is refused.
 */
function openLibrary(id, lib, srcRoot, log) {
  const archive = path.join(srcRoot, lib.archive);
  if (!fs.existsSync(archive)) throw new Error(`library ${id}: ${archive} is missing (see docs/reference/instrument-sources.md)`);
  const bytes = fs.statSync(archive).size;
  if (lib.bytes != null && bytes !== lib.bytes) throw new Error(`library ${id}: ${archive} is ${bytes} bytes, the download was ${lib.bytes}`);
  if (lib.sha256) {
    const got = sha256File(archive);
    if (got !== lib.sha256) throw new Error(`library ${id}: ${archive} has SHA-256 ${got}, the download had ${lib.sha256}`);
  }
  if (/\.7z$/i.test(archive)) {
    const files = new Map(readSevenZip(fs.readFileSync(archive)).map((f) => [f.name, f.data]));
    return {
      list: () => [...files.keys()],
      read: (rel) => {
        const d = files.get(rel);
        if (!d) throw new Error(`library ${id}: no ${rel} in the archive`);
        return d;
      },
      input: (rel) => files.get(rel),
    };
  }
  const dir = path.join(path.dirname(archive), 'unpacked');
  const marker = path.join(dir, '.unpacked-from-' + path.basename(archive));
  if (!fs.existsSync(marker)) {
    log(`  unpacking ${path.basename(archive)} (once)...`);
    fs.rmSync(dir, { recursive: true, force: true });
    fs.mkdirSync(dir, { recursive: true });
    execFileSync(tarCommand(), ['-xf', archive, '-C', dir], { stdio: 'inherit', windowsHide: true });
    fs.writeFileSync(marker, new Date().toISOString() + '\n');
  }
  const base = lib.strip ? path.join(dir, lib.strip) : dir;
  const walk = (d, rel, out) => {
    for (const e of fs.readdirSync(d, { withFileTypes: true })) {
      const r = rel ? rel + '/' + e.name : e.name;
      if (e.isDirectory()) walk(path.join(d, e.name), r, out);
      else out.push(r);
    }
    return out;
  };
  let listing = null;
  return {
    list: () => (listing ||= walk(base, '', [])),
    read: (rel) => fs.readFileSync(path.join(base, rel)),
    input: (rel) => path.join(base, rel),
  };
}

// ── Choosing samples ─────────────────────────────────────────────────────────────────────────

/** A minimal SFZ reader: headers, opcodes (a sample= value may hold spaces), inheritance from
 * <global>, <master>, <group> to <region>, #define. Enough for FreePats' mappings. */
function parseSfz(text) {
  const defines = {};
  const regions = [];
  let scope = { global: {}, master: {}, group: {}, control: {} };
  let current = null;
  const lines = text.replace(/\r/g, '').split('\n');
  for (let raw of lines) {
    raw = raw.replace(/\/\/.*$/, '');
    const def = /^\s*#define\s+(\$\w+)\s+(.*?)\s*$/.exec(raw);
    if (def) { defines[def[1]] = def[2]; continue; }
    const line = raw.replace(/\$\w+/g, (m) => (m in defines ? defines[m] : m));
    const re = /<(\w+)>|(\w+)=(.*?)(?=\s+\w+=|\s*<|\s*$)/g;
    let m;
    while ((m = re.exec(line))) {
      if (m[1]) {
        const h = m[1];
        if (current) regions.push(current);
        current = null;
        if (h === 'region') current = { ...scope.control, ...scope.global, ...scope.master, ...scope.group };
        else if (h === 'group') scope.group = {};
        else if (h === 'master') { scope.master = {}; scope.group = {}; }
        else if (h === 'global') { scope.global = {}; scope.master = {}; scope.group = {}; }
        else if (h === 'control') scope.control = {};
        scope.last = h;
      } else {
        const target = current || scope[scope.last] || scope.global;
        target[m[2]] = m[3].trim();
      }
    }
  }
  if (current) regions.push(current);
  return regions;
}

function sfzCandidates(recipe, lib) {
  const sfzPath = recipe.from.sfz;
  const regions = parseSfz(lib.read(sfzPath).toString('utf8'));
  const dir = path.posix.dirname(sfzPath);
  // A key opcode is a number or a note name ("Gb3", "C4"; SFZ's middle C is C4 = 60).
  const num = (v, d) => (v === undefined ? d : /^-?\d+(\.\d+)?$/.test(String(v)) ? Number(v) : B.noteToMidi(v) ?? d);
  return regions.map((r) => {
    const key = num(r.pitch_keycenter, num(r.key, 60));
    return {
      file: path.posix.normalize((dir === '.' ? '' : dir + '/') + (r.default_path || '') + r.sample.replace(/\\/g, '/')),
      key,
      named: key,
      lo_key: num(r.lokey, num(r.key, key)),
      hi_key: num(r.hikey, num(r.key, key)),
      lo_vel: num(r.lovel, 1),
      hi_vel: num(r.hivel, 127),
      layer: num(r.lovel, 1),
      rr: num(r.seq_position, 1),
      tune: num(r.tune, 0),
      loop: r.loop_start !== undefined && r.loop_end !== undefined ? [Number(r.loop_start), Number(r.loop_end)] : null,
    };
  });
}

function patternCandidates(recipe, lib) {
  const dirs = [].concat(recipe.from.dir);
  const re = new RegExp(recipe.from.pattern);
  const out = [];
  for (const rel of lib.list()) {
    const d = path.posix.dirname(rel);
    if (!dirs.includes(d)) continue;
    const m = re.exec(path.posix.basename(rel));
    if (!m) continue;
    const g = m.groups || {};
    // A MIDI number in the name wins over a note name (jSteelDrum names a few A4 files "A5").
    const named = g.midi != null ? Number(g.midi) : g.note != null ? B.noteToMidi(g.note) : null;
    if (g.note != null && named == null) continue;
    let key = named;
    if (key == null && recipe.keys && g.art != null) key = recipe.keys[g.art];
    if (key == null && recipe.key != null) key = recipe.key;
    if (key == null && recipe.note_from !== 'measured') key = 60;
    out.push({ file: rel, key, named, art: g.art || '', layerToken: (g.vel || '').toLowerCase(), rr: g.rr ? Number(g.rr) : 1, loop: null, tune: 0 });
  }
  return out;
}

/** Order a library's velocity words or numbers softest first. */
const DYNAMICS = ['pppp', 'ppp', 'pp', 'p', 'mp', 'mf', 'f', 'ff', 'fff', 'ffff'];
function layerRank(token, order) {
  for (const list of [order, DYNAMICS]) {
    const i = list ? list.indexOf(token) : -1;
    if (i >= 0) return i;
  }
  const n = /(\d+)/.exec(token);
  return n ? Number(n[1]) : 0;
}

/** Keep `n` of a sorted list, spread from the first to the last. */
function spread(list, n) {
  if (n >= list.length) return list;
  if (n <= 1) return [list[Math.floor((list.length - 1) / 2)]];
  return Array.from({ length: n }, (_, i) => list[Math.round((i * (list.length - 1)) / (n - 1))]).filter((x, i, a) => a.indexOf(x) === i);
}

function select(recipe, cands) {
  const keep = recipe.keep || {};
  if (recipe.from.sfz) {
    // An SFZ mapping already is the library's own choice of keys and layers.
    return cands;
  }
  if (keep.only_tokens) cands = cands.filter((c) => keep.only_tokens.includes(c.layerToken));
  if (keep.only_art) cands = cands.filter((c) => keep.only_art.includes(c.art));
  // Group by (note or articulation), then layers, then round robins.
  const groups = new Map();
  for (const c of cands) {
    const g = c.named != null ? 'n' + c.named : 'a' + c.art + ':' + (c.key == null ? '' : c.key);
    if (!groups.has(g)) groups.set(g, []);
    groups.get(g).push(c);
  }
  let names = [...groups.keys()];
  const noteGroups = names.filter((g) => g.startsWith('n')).sort((a, b) => Number(a.slice(1)) - Number(b.slice(1)));
  if (keep.every && keep.every > 1 && noteGroups.length > 2) {
    const kept = noteGroups.filter((_, i) => i % keep.every === 0);
    if (kept[kept.length - 1] !== noteGroups[noteGroups.length - 1]) kept.push(noteGroups[noteGroups.length - 1]);
    names = names.filter((g) => !g.startsWith('n') || kept.includes(g));
  }
  const out = [];
  for (const g of names) {
    const list = groups.get(g);
    const layers = [...new Set(list.map((c) => c.layerToken))].sort((a, b) => layerRank(a, recipe.vel_order) - layerRank(b, recipe.vel_order));
    const keptLayers = spread(layers, keep.layers || 99);
    for (const lt of keptLayers) {
      const rrs = list.filter((c) => c.layerToken === lt).sort((a, b) => a.rr - b.rr);
      out.push(...rrs.slice(0, keep.rr || 99));
    }
  }
  return out;
}

// ── Processing one sample ────────────────────────────────────────────────────────────────────

/** Trim, fade and measure one decoded sample. Returns the frames to keep and what was found. */
function shape(pcm, channels, rate, recipe, loop) {
  const frames = Math.floor(pcm.length / channels);
  let peak = 0;
  for (let i = 0; i < pcm.length; i++) peak = Math.max(peak, Math.abs(pcm[i]));
  const quiet = peak * 0.0003; // about -70 dB under the peak
  let first = 0;
  if (!loop) {
    const lead = peak * 0.0005;
    while (first < frames) {
      let loud = false;
      for (let c = 0; c < channels; c++) if (Math.abs(pcm[first * channels + c]) > lead) loud = true;
      if (loud) break;
      first++;
    }
    first = Math.max(0, first - Math.floor(rate * 0.002));
  }
  let last = frames - 1;
  while (last > first) {
    let loud = false;
    for (let c = 0; c < channels; c++) if (Math.abs(pcm[last * channels + c]) > quiet) loud = true;
    if (loud) break;
    last--;
  }
  let length = last - first + 1;
  const maxFrames = recipe.max_seconds ? Math.floor(recipe.max_seconds * rate) : Infinity;
  let keepLoop = loop;
  if (loop && loop[1] > first + Math.min(length, maxFrames)) keepLoop = null;
  if (keepLoop) length = Math.max(length, keepLoop[1] - first + 1);
  const capped = length > maxFrames;
  if (capped) length = maxFrames;
  const out = pcm.slice(first * channels, (first + length) * channels);
  const fadeFrames = Math.min(length, Math.floor(rate * (capped ? (recipe.fade_ms || 400) : 5) / 1000));
  for (let i = 0; i < fadeFrames; i++) {
    const g = 1 - (i + 1) / fadeFrames;
    const fi = length - fadeFrames + i;
    for (let c = 0; c < channels; c++) out[fi * channels + c] *= g;
  }
  return { samples: out, frames: length, peak, loop: keepLoop ? [keepLoop[0] - first, keepLoop[1] - first] : null };
}

async function processSample(cand, recipe, lib, staging, quality) {
  const raw = lib.read(cand.file);
  const info = B.audioInfo(raw);
  const channels = recipe.channels || (info ? Math.min(2, info.channels) : 2);
  let rate = info ? info.rate : 48000;
  let loop = cand.loop;
  if (rate > 48000) {
    if (loop) loop = loop.map((x) => Math.round((x * 48000) / rate));
    rate = 48000;
  }
  const pcm = await decode(lib.input(cand.file), channels, rate);
  // Other microphones of the same hit, mixed in (Big Rusty Drums: the close mic and the overheads,
  // recorded together, so they line up from the first sample).
  for (const m of recipe.from.mix || []) {
    const other = await decode(lib.input(m.dir + '/' + path.posix.basename(cand.file)), channels, rate);
    const g = Math.pow(10, (m.gain_db || 0) / 20);
    for (let i = 0; i < Math.min(pcm.length, other.length); i++) pcm[i] += other[i] * g;
  }
  // A gain for the whole sample (the same for every layer, so the dynamics between them stay).
  if (recipe.gain_db) {
    const g = Math.pow(10, recipe.gain_db / 20);
    for (let i = 0; i < pcm.length; i++) pcm[i] *= g;
  }
  const shaped = shape(pcm, channels, rate, recipe, loop);
  // The pitch, on the mono mix, near the note the file is named for (or anywhere in a drum's
  // range when the file names no note: the timpani).
  let measured = null;
  if (recipe.pitched !== false && recipe.measure_pitch !== false) {
    const mono = new Float32Array(Math.floor(pcm.length / channels));
    for (let i = 0; i < mono.length; i++) {
      let s = 0;
      for (let c = 0; c < channels; c++) s += pcm[i * channels + c];
      mono[i] = s / channels;
    }
    const expect = cand.named != null ? B.noteHz(cand.named) : (recipe.expect_hz || 110);
    measured = B.measurePitch(mono, rate, expect);
  }
  const outName = path.posix.basename(cand.file).replace(/\.[^.]+$/, '').replace(/[^A-Za-z0-9#_.-]+/g, '_').replace(/#/g, 's') + '.ogg';
  const outPath = path.join(staging, outName);
  await encodeVorbis(shaped.samples, channels, rate, quality, outPath);
  return { ...cand, outName, rate, channels, frames: shaped.frames, measured, loop: shaped.loop, peak: shaped.peak };
}

// ── Laying out the zones ─────────────────────────────────────────────────────────────────────

function median(xs) {
  if (xs.length === 0) return null;
  const s = [...xs].sort((a, b) => a - b);
  return s.length % 2 ? s[(s.length - 1) / 2] : (s[s.length / 2 - 1] + s[s.length / 2]) / 2;
}

const hzToMidi = (hz) => 69 + 12 * Math.log2(hz / 440);

/**
 * The library's tuning, from what was measured: how many octaves its note names are off from
 * C4 = 60 (VCSL's Kawai names the piano's lowest A "A-1"), and its A4 reference (the median of
 * every clearly measured sample's offset from equal temperament at A = 440).
 */
function tuning(samples, recipe) {
  if (recipe.pitched === false) return { shift: 0, a4: null, how: 'unpitched: played at its recorded rate, never retuned' };
  if (recipe.note_from === 'measured') {
    const n = samples.filter((s) => s.measured).length;
    return { shift: 0, a4: 440, how: `${recipe.tuning_note || 'keys from measured pitch'}: each of ${n} samples is played from its own measured frequency` };
  }
  const named = samples.filter((s) => s.named != null && s.measured);
  if (recipe.measure_pitch === false || named.length < 3) {
    const shift = recipe.octave_shift || 0;
    const a4 = recipe.source_a4_hz || 440;
    const why = recipe.measure_pitch === false ? recipe.tuning_note || 'not measured (inharmonic partials)' : 'too few clear measurements';
    return { shift, a4, how: `${why}; A4 = ${a4} Hz assumed${shift ? `, names ${shift / 12} octave(s) off` : ''}` };
  }
  const diffs = named.map((s) => hzToMidi(s.measured) - (s.named - (s.tune || 0) / 100));
  const shift = recipe.octave_shift != null ? recipe.octave_shift : 12 * Math.round(median(diffs) / 12);
  const cents = median(diffs.map((d) => (d - shift) * 100));
  const a4 = Math.round(440 * Math.pow(2, cents / 1200) * 100) / 100;
  const note = recipe.tuning_note ? `; ${recipe.tuning_note}` : '';
  return { shift, a4, how: `measured: median of ${named.length} samples, ${cents >= 0 ? '+' : ''}${cents.toFixed(1)} cents from A = 440${shift ? `; names ${shift / 12} octave(s) off C4 = 60` : ''}${note}` };
}

function layout(samples, recipe, tune) {
  const extend = recipe.extend != null ? recipe.extend : 4;
  for (const s of samples) {
    if (recipe.note_from === 'measured') s.key = s.measured ? Math.round(hzToMidi(s.measured)) : null;
    else if (s.named != null) s.key = s.named + tune.shift;
  }
  const kept = samples.filter((s) => s.key != null);
  if (!recipe.from.sfz) {
    const keys = [...new Set(kept.map((s) => s.key))].sort((a, b) => a - b);
    const pitched = recipe.pitched !== false;
    keys.forEach((k, i) => {
      // Exact keys (the Hang): each note plays only its own key; nothing is stretched to fill the
      // semitones between, which the instrument does not have.
      const spreadKeys = pitched && !recipe.exact_keys;
      const lo = spreadKeys ? (i === 0 ? Math.max(0, k - extend) : Math.floor((keys[i - 1] + k) / 2) + 1) : k;
      const hi = spreadKeys ? (i === keys.length - 1 ? Math.min(127, k + extend) : Math.floor((k + keys[i + 1]) / 2)) : k;
      const atKey = kept.filter((s) => s.key === k);
      const layers = [...new Set(atKey.map((s) => s.layerToken))].sort((a, b) => layerRank(a, recipe.vel_order) - layerRank(b, recipe.vel_order));
      for (const s of atKey) {
        const li = layers.indexOf(s.layerToken);
        s.lo_key = lo;
        s.hi_key = hi;
        s.lo_vel = Math.floor((li * 127) / layers.length) + 1;
        s.hi_vel = Math.floor(((li + 1) * 127) / layers.length);
        s.layer = li + 1;
      }
    });
    // Round robins numbered 1, 2, 3 within each zone, whatever the library called them.
    const zoneOf = (s) => `${s.lo_key}:${s.lo_vel}`;
    const counts = new Map();
    for (const s of kept.sort((a, b) => a.key - b.key || a.lo_vel - b.lo_vel || a.rr - b.rr)) {
      const z = zoneOf(s);
      counts.set(z, (counts.get(z) || 0) + 1);
      s.rr = counts.get(z);
    }
  } else {
    for (const s of kept) {
      s.lo_key += tune.shift;
      s.hi_key += tune.shift;
      s.key += 0; // pitch_keycenter already is the key the sample sounds at
    }
  }
  return kept.sort((a, b) => a.key - b.key || a.lo_vel - b.lo_vel || a.rr - b.rr);
}

// ── Writing ──────────────────────────────────────────────────────────────────────────────────

const HEADER = '// Written by scripts/instrument-packs.js (`just instrument-packs`) from scripts/instrument-recipes.json.\n// Edit the recipe and run it again rather than editing this file.\n';

function definitionRon(recipe, lib, zones, tune) {
  const pitched = recipe.pitched !== false;
  const keys = zones.map((z) => z.key);
  const def = B.struct('Instrument', {
    id: recipe.id,
    words: `instrument_packs.instrument.${recipe.id}`,
    part: recipe.part,
    rank: recipe.rank,
    kind: B.variant('Sampler', {
      folder: recipe.id,
      pitched,
      source_a4_hz: B.f(tune.a4 || 440),
      tuning_note: tune.how,
      release_s: B.f(recipe.release_s != null ? recipe.release_s : 0.5),
      normalized: !!(recipe.normalized ?? lib.normalized),
      ...(recipe.exact_keys ? { exact_keys: true } : {}),
      zones: zones.map((z) => ({
        file: z.outName,
        key: z.key,
        lo_key: Math.max(0, Math.min(127, z.lo_key)),
        hi_key: Math.max(0, Math.min(127, z.hi_key)),
        lo_vel: z.lo_vel,
        hi_vel: z.hi_vel,
        rr: z.rr,
        recorded_hz: pitched ? B.some(B.f(Math.round((recipe.note_from === 'measured' && z.measured ? z.measured : B.noteHz(z.key, tune.a4 || 440) * Math.pow(2, -(z.tune || 0) / 1200)) * 1000) / 1000)) : null,
        measured_hz: z.measured ? B.some(B.f(Math.round(z.measured * 1000) / 1000)) : null,
        loop_start: z.loop ? B.some(z.loop[0]) : null,
        loop_end: z.loop ? B.some(z.loop[1]) : null,
      })),
    }),
    range: { low: B.midiToNote(Math.min(...zones.map((z) => z.lo_key), ...keys)), high: B.midiToNote(Math.max(...zones.map((z) => z.hi_key), ...keys)) },
    reverb_send: B.f(recipe.reverb_send != null ? recipe.reverb_send : 0.3),
    licence: { name: lib.licence_name, words: `instrument_packs.licence.${lib.licence}`, file: 'LICENSE.txt' },
    source: { library: lib.name, url: lib.url, version: lib.version, fetched: lib.fetched },
    credit: lib.credit,
  });
  return HEADER + B.ron(def) + '\n';
}

function sourceRon(recipe, lib, zones) {
  const src = B.struct('SampleSource', {
    library: lib.name,
    url: lib.url,
    version: lib.version,
    licence: lib.licence_name,
    fetched: lib.fetched,
    archive: lib.archive,
    archive_sha256: lib.sha256 || '',
    kept: zones.map((z) => ({ file: z.outName, from: z.file })),
  });
  return HEADER + B.ron(src) + '\n';
}

function writeInstrumentFiles(root, recipe, lib, zones, tune, licenceText) {
  const instrDir = path.join(root, 'instruments');
  const sampleDir = path.join(root, 'samples', recipe.id);
  fs.mkdirSync(instrDir, { recursive: true });
  fs.writeFileSync(path.join(instrDir, recipe.id + '.ron'), definitionRon(recipe, lib, zones, tune));
  fs.writeFileSync(path.join(sampleDir, 'source.ron'), sourceRon(recipe, lib, zones));
  fs.writeFileSync(path.join(sampleDir, 'LICENSE.txt'), licenceText);
}

function dirBytes(d) {
  let n = 0;
  if (!fs.existsSync(d)) return 0;
  for (const e of fs.readdirSync(d, { withFileTypes: true })) {
    const p = path.join(d, e.name);
    n += e.isDirectory() ? dirBytes(p) : fs.statSync(p).size;
  }
  return n;
}

function filesUnder(root, rel, out) {
  for (const e of fs.readdirSync(path.join(root, rel), { withFileTypes: true })) {
    const r = rel ? rel + '/' + e.name : e.name;
    if (e.isDirectory()) filesUnder(root, r, out);
    else out.push(r);
  }
  return out;
}

function catalogRon(recipes, packs, coreBytes, coreIds) {
  const refs = (ids) => ids.map((id) => ({ id, part: recipes.instruments.find((r) => r.id === id).part }));
  const cat = B.struct('PackCatalog', {
    release: recipes.release,
    base_url: recipes.base_url,
    core: B.struct('', { words: 'instrument_packs.pack.core', instruments: refs(coreIds), bytes: coreBytes }),
    packs: packs.map((p) => ({
      id: p.id,
      version: p.version,
      family: p.family,
      words: `instrument_packs.pack.${p.id}`,
      file: p.file,
      bytes: p.bytes,
      sha256: p.sha256,
      licence: `instrument_packs.licence.${p.licence}`,
      instruments: refs(p.instruments),
    })),
  });
  return HEADER + B.ron(cat) + '\n';
}

// ── Main ─────────────────────────────────────────────────────────────────────────────────────

async function main(argv) {
  const args = parseArgs(argv);
  const log = args.quiet ? () => {} : (...m) => console.log(...m);
  checkFfmpeg();
  const recipes = JSON.parse(fs.readFileSync(args.recipes, 'utf8'));
  const instruments = recipes.instruments.filter((r) => !args.only || args.only.has(r.id));
  const scratch = fs.mkdtempSync(path.join(os.tmpdir(), 'hos-instruments-'));
  const opened = new Map();
  const built = [];
  try {
    for (const recipe of instruments) {
      const lib = recipes.libraries[recipe.library];
      if (!lib) throw new Error(`instrument ${recipe.id}: no library ${recipe.library}`);
      if (!opened.has(recipe.library)) opened.set(recipe.library, openLibrary(recipe.library, lib, args.src, log));
      const files = opened.get(recipe.library);
      const pack = recipe.pack;
      const quality = (recipes.packs[pack] || {}).quality ?? 5;
      const cands = recipe.from.sfz ? sfzCandidates(recipe, files) : patternCandidates(recipe, files);
      const chosen = select(recipe, cands);
      if (chosen.length === 0) throw new Error(`instrument ${recipe.id}: no samples matched its recipe`);
      log(`${recipe.id} (${pack}): ${chosen.length} of ${cands.length} samples`);
      const root = path.join(scratch, pack);
      const staging = path.join(root, 'samples', recipe.id);
      fs.mkdirSync(staging, { recursive: true });
      const done = await pool(chosen, args.jobs, (c) => processSample(c, recipe, files, staging, quality));
      const tune = tuning(done, recipe);
      const zones = layout(done, recipe, tune);
      if (zones.length === 0) throw new Error(`instrument ${recipe.id}: no sample kept a key (pitch not measurable?)`);
      // Files of samples that ended up with no key (a timpani hit whose pitch was not clear).
      for (const s of done) if (s.key == null) fs.rmSync(path.join(staging, s.outName), { force: true });
      // The library's licence text, word for word; its line endings made plain LF (Karoryfer's mixes
      // CRLF and LF, which the repository's encoding check refuses).
      const licenceText = files.read(lib.licence_file).toString('utf8').replace(/\r\n?/g, '\n');
      writeInstrumentFiles(root, recipe, lib, zones, tune, licenceText);
      const off = zones.filter((z) => z.measured && Math.abs(1200 * Math.log2(z.measured / B.noteHz(z.key, tune.a4 || 440))) > 30).length;
      log(`  ${zones.length} zones, ${tune.how}${off ? `; ${off} sample(s) measured more than 30 cents off their key` : ''}`);
      // A sample over full scale would clip when played (a mix of microphones can sum past 1.0).
      const hot = zones.filter((z) => z.peak > 1.0).length;
      if (hot) throw new Error(`instrument ${recipe.id}: ${hot} sample(s) peak over full scale; lower the mix gains`);
      built.push({ recipe, root, bytes: dirBytes(staging) });
    }

    // The core set: straight into the repository's data/music (it ships inside the app).
    const coreIds = recipes.instruments.filter((r) => r.pack === 'core').map((r) => r.id);
    const coreBuilt = built.filter((b) => b.recipe.pack === 'core');
    for (const b of coreBuilt) {
      const id = b.recipe.id;
      fs.rmSync(path.join(args.coreOut, 'samples', id), { recursive: true, force: true });
      fs.mkdirSync(path.join(args.coreOut, 'instruments'), { recursive: true });
      fs.cpSync(path.join(b.root, 'samples', id), path.join(args.coreOut, 'samples', id), { recursive: true });
      fs.copyFileSync(path.join(b.root, 'instruments', id + '.ron'), path.join(args.coreOut, 'instruments', id + '.ron'));
    }
    const coreBytes = coreIds.reduce((n, id) => n + dirBytes(path.join(args.coreOut, 'samples', id)) + (fs.existsSync(path.join(args.coreOut, 'instruments', id + '.ron')) ? fs.statSync(path.join(args.coreOut, 'instruments', id + '.ron')).size : 0), 0);
    const limit = (recipes.packs.core || {}).max_bytes;
    if (limit && coreBytes > limit) throw new Error(`the core set is ${coreBytes} bytes, over its ${limit}`);
    if (coreBuilt.length) log(`core set: ${coreIds.length} instruments, ${(coreBytes / 1048576).toFixed(1)} MiB in ${args.coreOut}`);

    // Every other pack: one tar with its checksum.
    const packs = [];
    for (const [id, p] of Object.entries(recipes.packs)) {
      if (id === 'core') continue;
      const ids = recipes.instruments.filter((r) => r.pack === id).map((r) => r.id);
      const file = `humanity-instruments-${id}-v${p.version}.tar`;
      const ready = built.filter((b) => b.recipe.pack === id);
      const entry = { id, version: p.version, family: p.family || id, file, licence: p.licence || 'cc0', instruments: ids };
      if (ready.length === ids.length && ids.length > 0) {
        const root = path.join(scratch, id);
        const manifest = HEADER + B.ron(B.struct('PackManifest', { id, version: p.version, instruments: ids })) + '\n';
        fs.writeFileSync(path.join(root, 'pack.ron'), manifest);
        const entries = filesUnder(root, '', []).map((rel) => ({ path: rel, data: fs.readFileSync(path.join(root, rel)) }));
        const tar = B.tarBuffer(entries);
        fs.mkdirSync(args.packsOut, { recursive: true });
        fs.writeFileSync(path.join(args.packsOut, file), tar);
        entry.bytes = tar.length;
        entry.sha256 = crypto.createHash('sha256').update(tar).digest('hex');
        fs.writeFileSync(path.join(args.packsOut, file + '.sha256'), `${entry.sha256}  ${file}\n`);
        log(`pack ${id}: ${ids.length} instruments, ${(tar.length / 1048576).toFixed(1)} MiB, sha256 ${entry.sha256}`);
      } else if (ready.length && !args.writeCatalog) {
        log(`pack ${id}: ${ready.length} of its ${ids.length} instruments built; no archive (a trial run)`);
        continue;
      } else if (ready.length) {
        throw new Error(`pack ${id}: only ${ready.length} of its ${ids.length} instruments were built (use --only for whole packs, or --no-catalog)`);
      } else {
        // Not rebuilt this run: keep what the catalog says.
        const prev = fs.existsSync(args.catalog) ? fs.readFileSync(args.catalog, 'utf8') : '';
        const m = new RegExp(`id: "${id}",[\\s\\S]*?bytes: (\\d+),\\s*sha256: "([0-9a-f]*)"`).exec(prev);
        entry.bytes = m ? Number(m[1]) : 0;
        entry.sha256 = m ? m[2] : '';
      }
      packs.push(entry);
    }
    if (args.writeCatalog) {
      fs.mkdirSync(path.dirname(args.catalog), { recursive: true });
      fs.writeFileSync(args.catalog, catalogRon(recipes, packs, coreBytes, coreIds));
      log(`catalog: ${args.catalog}`);
    }
    return { coreBytes, packs };
  } finally {
    fs.rmSync(scratch, { recursive: true, force: true });
  }
}

if (require.main === module) {
  main(process.argv.slice(2)).catch((e) => {
    console.error('instrument-packs: ' + e.message);
    process.exit(1);
  });
}

module.exports = { main, parseSfz, select, tuning, layout, patternCandidates };
