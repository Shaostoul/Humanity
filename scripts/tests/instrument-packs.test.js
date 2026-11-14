'use strict';
/**
 * The instrument pipeline (scripts/instrument-packs.js, `just instrument-packs`; docs/design/
 * music.md rung 6) on a FIXTURE library made here: tones at known pitches, written as WAV files
 * into a tar.gz the way GitHub serves VCSL, with a licence file. The real libraries are gigabytes
 * on one machine's E: drive; this proves the same code on a few kilobytes anywhere ffmpeg is.
 *
 * It checks that the pipeline:
 * - picks the samples, lays out keys and velocity layers, and MEASURES the source tuning: one
 *   instrument is recorded at A = 442 Hz with its notes named an octave low (as VSCO 2 names its
 *   strings), and its definition must say 442 and correct the octave;
 * - writes each definition with the licence, the source and the credit, and each sample folder
 *   with the library's licence text and a source.ron naming every kept sample's original file;
 * - puts the core set straight into the core folder and the pack into ONE tar whose SHA-256 is in
 *   the .sha256 file and the catalog, with the right size; the same input makes the same bytes;
 * - refuses a library download whose checksum is not the recorded one.
 *
 * With HUMANITY_WRITE_INSTRUMENT_FIXTURE=1 it also writes what it built to
 * tests/fixtures/instrument_packs/, the pack the Rust tests install from a loopback server
 * (src/instrument_packs/install_tests.rs, library_tests.rs, and Settings' instruments_tests.rs).
 *
 * Seen RED first (2026-11-06), each under a mutation of the pipeline put back after (a scratch
 * script applied each, ran this file, restored it):
 * - `tuning()` returning A4 = 440 always: "source_a4_hz is the measured 442 (got 440)";
 * - the octave shift left out of `layout()`: "keys corrected an octave up" (the keys were 48,57,64);
 * - the pack's checksum taken from its FIRST file instead of the whole tar: "the .sha256 file is
 *   the tar's";
 * - the library checksum check skipped: "Missing expected rejection: a changed library download
 *   is refused".
 */

const test = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const os = require('os');
const path = require('path');
const zlib = require('zlib');
const crypto = require('crypto');
const B = require('../lib/instrument-build');
const pipeline = require('../instrument-packs');

const ROOT = path.resolve(__dirname, '..', '..');

/** A 16-bit mono WAV of `seconds` of a decaying tone at `hz` (0 = noise), then `tail` of silence. */
function wav(hz, amp, seconds, tail, seed) {
  const rate = 48000;
  const n = Math.round((seconds + tail) * rate);
  const data = Buffer.alloc(n * 2);
  let s = seed || 1;
  for (let i = 0; i < n; i++) {
    const t = i / rate;
    let v = 0;
    if (t < seconds) {
      const env = Math.exp(-t * 2.5) * Math.min(1, t * 200);
      if (hz > 0) v = amp * env * (Math.sin(2 * Math.PI * hz * t) + 0.2 * Math.sin(4 * Math.PI * hz * t));
      else {
        s = (s * 1103515245 + 12345) % 2147483648;
        v = amp * env * (s / 1073741824 - 1) * Math.exp(-t * 20);
      }
    }
    data.writeInt16LE(Math.max(-32767, Math.min(32767, Math.round(v * 32767))), i * 2);
  }
  const h = Buffer.alloc(44);
  h.write('RIFF', 0);
  h.writeUInt32LE(36 + data.length, 4);
  h.write('WAVE', 8);
  h.write('fmt ', 12);
  h.writeUInt32LE(16, 16);
  h.writeUInt16LE(1, 20);
  h.writeUInt16LE(1, 22);
  h.writeUInt32LE(rate, 24);
  h.writeUInt32LE(rate * 2, 28);
  h.writeUInt16LE(2, 32);
  h.writeUInt16LE(16, 34);
  h.write('data', 36);
  h.writeUInt32LE(data.length, 40);
  return Buffer.concat([h, data]);
}

const LICENCE = 'CC0 1.0 Universal (fixture licence text for the instrument pipeline test)\n';

/** The fixture library: a tar.gz like GitHub's archive of a commit. */
function makeLibrary(dir) {
  const entries = [{ path: 'FixtureLib/LICENSE', data: Buffer.from(LICENCE) }];
  // A grand piano at A = 440, three notes, two velocity layers.
  for (const [note, midi] of [['A3', 57], ['A4', 69], ['A5', 81]]) {
    for (const [v, amp] of [[1, 0.25], [2, 0.7]]) {
      entries.push({ path: `FixtureLib/Grand/Fix_${note}_v${v}_rr1.wav`, data: wav(B.noteHz(midi, 440), amp, 1.2, 0.4) });
    }
  }
  // An upright recorded at A = 442 whose files name each note an octave low (C3 is really C4).
  for (const [name, midi] of [['C3', 60], ['A3', 69], ['E4', 76]]) {
    entries.push({ path: `FixtureLib/Upright/Up_${name}_v1.wav`, data: wav(B.noteHz(midi, 442), 0.5, 1.2, 0.2) });
  }
  // An unpitched drum, two layers.
  for (const [v, amp] of [[1, 0.3], [2, 0.8]]) {
    entries.push({ path: `FixtureLib/Drum/Boom_v${v}_rr1.wav`, data: wav(0, amp, 0.6, 0.1, v) });
  }
  const archive = path.join(dir, 'fixlib', 'FixtureLib.tar.gz');
  fs.mkdirSync(path.dirname(archive), { recursive: true });
  fs.writeFileSync(archive, zlib.gzipSync(B.tarBuffer(entries), { level: 6 }));
  return archive;
}

function recipes(archive) {
  const bytes = fs.statSync(archive).size;
  const sha256 = crypto.createHash('sha256').update(fs.readFileSync(archive)).digest('hex');
  return {
    release: 'assets-instruments-fixture',
    base_url: 'http://127.0.0.1/',
    packs: {
      core: { quality: 0, max_bytes: 2000000 },
      fixture: { version: 1, family: 'keys', quality: 0, licence: 'cc0' },
    },
    libraries: {
      fixlib: {
        name: 'Fixture Library', url: 'https://example.invalid/fixture', version: 'test', fetched: '2026-11-06',
        licence: 'cc0', licence_name: 'CC0 1.0', archive: 'fixlib/FixtureLib.tar.gz', strip: 'FixtureLib',
        bytes, sha256, licence_file: 'LICENSE', credit: 'Tones made by the pipeline test.',
      },
    },
    instruments: [
      { id: 'fixture_upright', pack: 'core', part: 'piano', rank: 10, library: 'fixlib',
        from: { dir: 'Upright', pattern: '^Up_(?<note>[A-G]#?\\d)_(?<vel>v\\d)\\.wav$' }, max_seconds: 2 },
      { id: 'fixture_grand', pack: 'fixture', part: 'piano', rank: 90, library: 'fixlib',
        from: { dir: 'Grand', pattern: '^Fix_(?<note>[A-G]#?\\d)_(?<vel>v\\d)_rr(?<rr>\\d)\\.wav$' }, max_seconds: 2 },
      { id: 'fixture_drum', pack: 'fixture', part: 'deep_drum', rank: 50, library: 'fixlib', pitched: false, key: 36,
        from: { dir: 'Drum', pattern: '^Boom_(?<vel>v\\d)_rr(?<rr>\\d)\\.wav$' }, max_seconds: 1 },
    ],
  };
}

const zones = (ron) => [...ron.matchAll(/\(file: "([^"]+)", key: (\d+), lo_key: (\d+), hi_key: (\d+), lo_vel: (\d+), hi_vel: (\d+), rr: (\d+), recorded_hz: (None|Some\(([\d.]+)\))/g)]
  .map((m) => ({ file: m[1], key: +m[2], lo_key: +m[3], hi_key: +m[4], lo_vel: +m[5], hi_vel: +m[6], rr: +m[7], hz: m[9] ? +m[9] : null }));

async function build(work, recipeObj, packsOut) {
  const recipePath = path.join(work, 'recipes.json');
  fs.writeFileSync(recipePath, JSON.stringify(recipeObj));
  const out = {
    core: path.join(work, 'data', 'music'),
    packs: packsOut || path.join(work, 'packs'),
    catalog: path.join(work, 'data', 'music', 'packs.ron'),
  };
  await pipeline.main(['--src', work, '--recipes', recipePath, '--core-out', out.core, '--packs-out', out.packs, '--catalog', out.catalog, '--jobs', '2', '--quiet']);
  return out;
}

test('the pipeline turns a fixture library into a core set and a checked pack', async () => {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'hos-instr-test-'));
  try {
    const archive = makeLibrary(work);
    const r = recipes(archive);
    const out = await build(work, r);

    // The core instrument: measured at 442, its octave-low names corrected.
    const upright = fs.readFileSync(path.join(out.core, 'instruments', 'fixture_upright.ron'), 'utf8');
    const a4 = Number(/source_a4_hz: ([\d.]+)/.exec(upright)[1]);
    assert.ok(Math.abs(a4 - 442) < 0.5, `source_a4_hz is the measured 442 (got ${a4})`);
    assert.match(upright, /names 1 octave\(s\) off C4 = 60/);
    const uz = zones(upright);
    assert.deepStrictEqual(uz.map((z) => z.key), [60, 69, 76], `keys corrected an octave up (got ${uz.map((z) => z.key)})`);
    assert.ok(Math.abs(uz[1].hz - 442) < 0.05, `A4 was recorded at 442 Hz (got ${uz[1].hz})`);
    assert.match(upright, /licence: \(name: "CC0 1.0", words: "instrument_packs\.licence\.cc0", file: "LICENSE\.txt"\)/);
    assert.match(upright, /credit: "Tones made by the pipeline test\."/);
    assert.match(upright, /source: \(library: "Fixture Library"/);
    const usamples = path.join(out.core, 'samples', 'fixture_upright');
    assert.strictEqual(fs.readFileSync(path.join(usamples, 'LICENSE.txt'), 'utf8'), LICENCE, 'the library licence text travels with the samples');
    const src = fs.readFileSync(path.join(usamples, 'source.ron'), 'utf8');
    assert.match(src, /from: "Upright\/Up_C3_v1\.wav"/);
    for (const z of uz) assert.ok(fs.statSync(path.join(usamples, z.file)).size > 500, `${z.file} was encoded`);
    assert.ok(uz.every((z) => /\.ogg$/.test(z.file)), 'the playback format is Ogg Vorbis');

    // The pack: one tar, its checksum in the .sha256 file and the catalog.
    const tarPath = path.join(out.packs, 'humanity-instruments-fixture-v1.tar');
    const tar = fs.readFileSync(tarPath);
    const sha = crypto.createHash('sha256').update(tar).digest('hex');
    assert.strictEqual(fs.readFileSync(tarPath + '.sha256', 'utf8').split(/\s+/)[0], sha, 'the .sha256 file is the tar\'s');
    const catalog = fs.readFileSync(out.catalog, 'utf8');
    assert.match(catalog, new RegExp(`sha256: "${sha}"`), 'the catalog\'s sha256 is the tar\'s');
    assert.match(catalog, new RegExp(`bytes: ${tar.length},`), 'the catalog\'s size is the tar\'s');
    assert.match(catalog, /\(id: "fixture_grand", part: "piano"\)/);
    assert.match(catalog, /core: \(\s*words: "instrument_packs\.pack\.core",\s*instruments: \[\s*\(id: "fixture_upright", part: "piano"\)/);
    const inTar = B.readTar(tar).map((e) => e.path);
    for (const want of ['pack.ron', 'instruments/fixture_grand.ron', 'instruments/fixture_drum.ron', 'samples/fixture_grand/LICENSE.txt', 'samples/fixture_grand/source.ron', 'samples/fixture_drum/LICENSE.txt']) {
      assert.ok(inTar.includes(want), `the pack holds ${want}`);
    }
    assert.ok(!inTar.some((p) => p.includes('fixture_upright')), 'the core instrument is not in the pack');
    const grand = B.readTar(tar).find((e) => e.path === 'instruments/fixture_grand.ron').data.toString('utf8');
    const gz = zones(grand);
    assert.strictEqual(gz.length, 6, 'three notes, two layers');
    assert.deepStrictEqual([...new Set(gz.map((z) => z.key))], [57, 69, 81]);
    assert.deepStrictEqual(gz.filter((z) => z.key === 69).map((z) => [z.lo_vel, z.hi_vel]), [[1, 63], [64, 127]], 'two velocity layers split the range');
    for (const z of gz) assert.ok(Math.abs(z.hz - B.noteHz(z.key, 440)) < 0.3, `${z.file} recorded at ${z.hz}`);
    assert.ok(inTar.filter((p) => p.startsWith('samples/fixture_grand/') && p.endsWith('.ogg')).length === 6);
    const drum = B.readTar(tar).find((e) => e.path === 'instruments/fixture_drum.ron').data.toString('utf8');
    assert.match(drum, /pitched: false/);
    assert.ok(zones(drum).every((z) => z.key === 36 && z.hz === null), 'the drum is unpitched, on its key');

    // The same input makes the same bytes, so the published checksum keeps meaning something.
    const again = await build(work, r, path.join(work, 'packs-again'));
    const sha2 = crypto.createHash('sha256').update(fs.readFileSync(path.join(again.packs, 'humanity-instruments-fixture-v1.tar'))).digest('hex');
    assert.strictEqual(sha2, sha, 'a rebuild makes a byte-identical pack');

    if (process.env.HUMANITY_WRITE_INSTRUMENT_FIXTURE === '1') {
      const fx = path.join(ROOT, 'tests', 'fixtures', 'instrument_packs');
      fs.rmSync(fx, { recursive: true, force: true });
      fs.mkdirSync(fx, { recursive: true });
      fs.copyFileSync(tarPath, path.join(fx, 'humanity-instruments-fixture-v1.tar'));
      fs.cpSync(path.join(work, 'data'), path.join(fx, 'data'), { recursive: true });
    }

    // A library download that is not the recorded one is refused.
    const changed = JSON.parse(JSON.stringify(r));
    changed.libraries.fixlib.sha256 = '0'.repeat(64);
    await assert.rejects(build(work, changed, path.join(work, 'packs-refused')), /has SHA-256/, 'a changed library download is refused');
  } finally {
    fs.rmSync(work, { recursive: true, force: true });
  }
});

/** The peak of an encoded sample, decoded by ffmpeg. */
function peakOf(file) {
  const raw = require('child_process').execFileSync('ffmpeg', ['-v', 'error', '-i', file, '-f', 'f32le', '-ac', '1', 'pipe:1'], { maxBuffer: 1 << 26 });
  const x = new Float32Array(raw.buffer, raw.byteOffset, Math.floor(raw.length / 4));
  let p = 0;
  for (const v of x) p = Math.max(p, Math.abs(v));
  return p;
}

/**
 * The 2026-11-07 additions, for the steel pan, the Hang and the big toms: a MIDI number in a file
 * name wins over its note name (jSteelDrum names two A4 files "A5"); `exact_keys` gives each note
 * its own key only and says so in the definition (the Hang's nine notes); `from.mix` adds another
 * microphone's file of the same hit, and a sample that would then peak over full scale is refused.
 *
 * Seen RED first (2026-11-07), each under a mutation put back after: the MIDI group ignored, "the
 * MIDI number wins over the note name"; the exact-key spread left as for other instruments, "each
 * note plays only its own key"; the mix loop skipped, "the overhead is mixed in (x1.00: 0.442
 * against 0.442 alone)"; the full-scale check removed, "Missing expected rejection: a mix past full
 * scale is refused".
 */
test('a MIDI number, exact keys and mixed microphones', async () => {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'hos-instr-test2-'));
  try {
    const entries = [{ path: 'Kit/LICENSE', data: Buffer.from(LICENCE) }];
    // Two pan notes, one mislabelled the way jSteelDrum's are: MIDI 069 named "A5".
    entries.push({ path: 'Kit/Pan/Pan_060__C4_1.wav', data: wav(B.noteHz(60, 440), 0.5, 1.0, 0.1) });
    entries.push({ path: 'Kit/Pan/Pan_069__A5_1.wav', data: wav(B.noteHz(69, 440), 0.5, 1.0, 0.1) });
    // A hang with two notes and a gap between them.
    entries.push({ path: 'Kit/Hang/A3_01.wav', data: wav(B.noteHz(57, 440), 0.5, 1.0, 0.1) });
    entries.push({ path: 'Kit/Hang/D4_01.wav', data: wav(B.noteHz(62, 440), 0.5, 1.0, 0.1) });
    // A drum hit (a low tone, which the encoder keeps cleanly) on two microphones, the same take.
    const hit = wav(110, 0.4, 0.5, 0.05);
    entries.push({ path: 'Kit/Tom/cl/t_vl1_rr1.wav', data: hit });
    entries.push({ path: 'Kit/Tom/oh/t_vl1_rr1.wav', data: hit });
    const archive = path.join(work, 'kit', 'Kit.tar.gz');
    fs.mkdirSync(path.dirname(archive), { recursive: true });
    fs.writeFileSync(archive, zlib.gzipSync(B.tarBuffer(entries)));
    const lib = {
      name: 'Kit', url: 'https://example.invalid/kit', version: 'test', fetched: '2026-11-07', licence: 'cc0', licence_name: 'CC0 1.0',
      archive: 'kit/Kit.tar.gz', strip: 'Kit', bytes: fs.statSync(archive).size,
      sha256: crypto.createHash('sha256').update(fs.readFileSync(archive)).digest('hex'), licence_file: 'LICENSE', credit: 'Test tones.',
    };
    const tom = (mixDb) => ({ id: 'tom', pack: 'core', part: 'deep_drum', rank: 10, library: 'kit', pitched: false, key: 41,
      from: { dir: 'Tom/cl', pattern: '^t_(?<vel>vl\\d)_rr(?<rr>\\d)\\.wav$', mix: [{ dir: 'Tom/oh', gain_db: mixDb }] }, max_seconds: 1 });
    const recipe = (tomRecipe) => ({
      release: 'r', base_url: 'http://127.0.0.1/', packs: { core: { quality: 0 } }, libraries: { kit: lib },
      instruments: [
        { id: 'pan', pack: 'core', part: 'steel_pan', rank: 10, library: 'kit', measure_pitch: false,
          from: { dir: 'Pan', pattern: '^Pan_(?<midi>\\d{3})_+(?<note>[A-G]b?\\d)_(?<vel>\\d)\\.wav$' }, max_seconds: 1 },
        { id: 'hang', pack: 'core', part: 'handpan', rank: 10, library: 'kit', exact_keys: true, measure_pitch: false,
          from: { dir: 'Hang', pattern: '^(?<note>[A-G]b?\\d)_(?<rr>\\d+)\\.wav$' }, max_seconds: 1 },
        tomRecipe,
      ],
    });
    // The tom alone first (the overhead 120 dB down), then with the overhead at -6 dB.
    const alone = peakOf(path.join((await build(work, recipe(tom(-120)))).core, 'samples', 'tom', 't_vl1_rr1.ogg'));
    const out = await build(work, recipe(tom(-6)));
    const pan = zones(fs.readFileSync(path.join(out.core, 'instruments', 'pan.ron'), 'utf8'));
    assert.deepStrictEqual(pan.map((z) => z.key), [60, 69], 'the MIDI number wins over the note name');
    const hangRon = fs.readFileSync(path.join(out.core, 'instruments', 'hang.ron'), 'utf8');
    assert.match(hangRon, /exact_keys: true/);
    assert.deepStrictEqual(zones(hangRon).map((z) => [z.lo_key, z.hi_key]), [[57, 57], [62, 62]], 'each note plays only its own key');
    assert.doesNotMatch(fs.readFileSync(path.join(out.core, 'instruments', 'pan.ron'), 'utf8'), /exact_keys/, 'only an exact-key instrument says so');
    // The same take on both microphones: the overhead at -6 dB adds half again (x1.50).
    const peak = peakOf(path.join(out.core, 'samples', 'tom', 't_vl1_rr1.ogg'));
    const ratio = peak / alone;
    assert.ok(ratio > 1.4 && ratio < 1.6, `the overhead is mixed in (x${ratio.toFixed(2)}: ${peak.toFixed(3)} against ${alone.toFixed(3)} alone)`);
    // At +12 dB the mix passes full scale, and the pipeline refuses it rather than clip.
    await assert.rejects(build(work, recipe(tom(12)), path.join(work, 'packs-hot')), /peak over full scale/, 'a mix past full scale is refused');
  } finally {
    fs.rmSync(work, { recursive: true, force: true });
  }
});

test('the tar writer is plain ustar and reads back', () => {
  const t = B.tarBuffer([{ path: 'b.txt', data: Buffer.from('bee') }, { path: 'a/c.bin', data: Buffer.alloc(700, 7) }]);
  assert.strictEqual(t.length % 512, 0);
  const back = B.readTar(t);
  assert.deepStrictEqual(back.map((e) => e.path), ['a/c.bin', 'b.txt'], 'sorted');
  assert.strictEqual(back[1].data.toString(), 'bee');
  assert.throws(() => B.tarBuffer([{ path: '../evil', data: Buffer.alloc(1) }]), /not allowed/);
});

/** A fixture library: `entries` (paths under Lib/) as a tar.gz, with its recorded size and SHA-256. */
function fixtureLib(work, entries) {
  const archive = path.join(work, 'lib', 'Lib.tar.gz');
  fs.mkdirSync(path.dirname(archive), { recursive: true });
  fs.writeFileSync(archive, zlib.gzipSync(B.tarBuffer([{ path: 'Lib/LICENSE', data: Buffer.from(LICENCE) }, ...entries])));
  return {
    name: 'Lib', url: 'https://example.invalid/lib', version: 'test', fetched: '2026-11-13', licence: 'cc0', licence_name: 'CC0 1.0',
    archive: 'lib/Lib.tar.gz', strip: 'Lib', bytes: fs.statSync(archive).size,
    sha256: crypto.createHash('sha256').update(fs.readFileSync(archive)).digest('hex'), licence_file: 'LICENSE', credit: 'Test sounds.',
  };
}

/** One 600-frame single cycle (AKWF's shape): 16-bit mono PCM at 44.1 kHz. */
function cycle(seed) {
  const data = Buffer.alloc(1200);
  for (let i = 0; i < 600; i++) data.writeInt16LE(Math.round(20000 * Math.sin((2 * Math.PI * i) / 600) + seed * 37), i * 2);
  const h = Buffer.alloc(44);
  h.write('RIFF', 0);
  h.writeUInt32LE(36 + data.length, 4);
  h.write('WAVE', 8);
  h.write('fmt ', 12);
  h.writeUInt32LE(16, 16);
  h.writeUInt16LE(1, 20);
  h.writeUInt16LE(1, 22);
  h.writeUInt32LE(44100, 24);
  h.writeUInt32LE(88200, 28);
  h.writeUInt16LE(2, 32);
  h.writeUInt16LE(16, 34);
  h.write('data', 36);
  h.writeUInt32LE(data.length, 40);
  return Buffer.concat([h, data]);
}

/**
 * 2026-11-13, the operator-approved FreePats and steel pan downloads: an SFZ's RELEASE regions are
 * not notes (the accordion has a release sample under each), and takes it chooses at random
 * (lorand) are numbered round robins 1, 2 rather than all claiming 1. Seen RED first against the
 * pipeline before the change (HEAD's instrument-packs.js copied over it, this file run, put back):
 * "the release sample is not a note: 3 zones".
 */
test('an SFZ: release regions left out, random takes numbered as round robins', async () => {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'hos-instr-test3-'));
  try {
    const sfz = [
      '<group> lokey=58 hikey=62 pitch_keycenter=60',
      '<region> sample=s/a1.wav lorand=0.5 hirand=1',
      '<region> sample=s/a2.wav lorand=0 hirand=0.5',
      '<group> trigger=release lokey=58 hikey=62 pitch_keycenter=60',
      '<region> sample=s/rel.wav',
      '<group> trigger=attack key=70',
      '<region> sample=s/tap.wav',
    ].join('\n');
    const lib = fixtureLib(work, [
      { path: 'Lib/inst.sfz', data: Buffer.from(sfz) },
      { path: 'Lib/s/a1.wav', data: wav(B.noteHz(60, 440), 0.5, 0.8, 0.1) },
      { path: 'Lib/s/a2.wav', data: wav(B.noteHz(60, 440), 0.4, 0.8, 0.1) },
      { path: 'Lib/s/rel.wav', data: wav(0, 0.2, 0.2, 0.05, 3) },
      { path: 'Lib/s/tap.wav', data: wav(0, 0.3, 0.2, 0.05, 4) },
    ]);
    const r = {
      release: 'x', base_url: 'http://127.0.0.1/', packs: { core: { quality: 0, max_bytes: 2000000 } }, libraries: { lib },
      instruments: [{ id: 'sfz_inst', pack: 'core', part: 'piano', rank: 10, library: 'lib', from: { sfz: 'inst.sfz', exclude: 'tap' }, max_seconds: 1 }],
    };
    const out = await build(work, r);
    const z = zones(fs.readFileSync(path.join(out.core, 'instruments', 'sfz_inst.ron'), 'utf8'));
    assert.strictEqual(z.length, 2, `the release sample is not a note: ${z.length} zones`);
    assert.deepStrictEqual(z.map((x) => x.rr).sort(), [1, 2], 'the two random takes are round robins 1 and 2');
    const src = fs.readFileSync(path.join(out.core, 'samples', 'sfz_inst', 'source.ron'), 'utf8');
    assert.match(src, /from: "s\/a2\.wav"/);
    assert.doesNotMatch(src, /rel\.wav/);
    assert.doesNotMatch(src, /tap\.wav/, 'a sample the recipe excludes is left out');
  } finally {
    fs.rmSync(work, { recursive: true, force: true });
  }
});

/**
 * 2026-11-13, AKWF: a wave set keeps single cycles as the library's own PCM bytes (a lossy codec
 * smears a cycle), a slice of each folder for the core set and every wave for a pack, with a
 * waves.ron index, the licence text, the count the recipe promises (a different count is refused),
 * and the catalog saying what each pack adds and whose credit it needs. Seen RED first against the
 * pipeline before wave sets (HEAD's instrument-packs.js copied over it, this file run, put back):
 * "a core wave set lands in data/music/waves".
 */
test('a wave set: single cycles kept byte for byte, in the core set and in a pack', async () => {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'hos-instr-test4-'));
  try {
    const entries = [];
    for (let i = 1; i <= 4; i++) entries.push({ path: `Lib/W/Saw/saw_000${i}.wav`, data: cycle(i) });
    for (let i = 1; i <= 2; i++) entries.push({ path: `Lib/W/Sq/sq_000${i}.wav`, data: cycle(10 + i) });
    const lib = fixtureLib(work, entries);
    const r = {
      release: 'x', base_url: 'http://127.0.0.1/', libraries: { lib }, instruments: [],
      packs: { core: { quality: 0, max_bytes: 2000000 }, synth: { version: 1, family: 'synth', licence: 'cc0', about: true, credits: ['someone'] } },
      wavesets: [
        { id: 'w_core', pack: 'core', library: 'lib', from: { root: 'W', groups: { Saw: 2 } }, count: 2 },
        { id: 'w_all', pack: 'synth', library: 'lib', from: { root: 'W' }, count: 6 },
      ],
    };
    const out = await build(work, r);
    const core = path.join(out.core, 'waves', 'w_core');
    assert.ok(fs.existsSync(path.join(core, 'waves.ron')), 'a core wave set lands in data/music/waves');
    assert.deepStrictEqual(fs.readdirSync(path.join(core, 'Saw')).sort(), ['saw_0001.wav', 'saw_0004.wav'], 'two spread through the folder');
    assert.ok(fs.readFileSync(path.join(core, 'Saw', 'saw_0004.wav')).equals(cycle(4)), 'byte for byte the library\'s PCM');
    assert.strictEqual(fs.readFileSync(path.join(core, 'LICENSE.txt'), 'utf8'), LICENCE);
    const index = fs.readFileSync(path.join(core, 'waves.ron'), 'utf8');
    assert.match(index, /\(file: "Saw\/saw_0001\.wav", frames: 600, rate: 44100, channels: 1, bits: 16\)/);
    assert.match(index, /words: "instrument_packs\.waveset\.w_core"/);
    const tar = B.readTar(fs.readFileSync(path.join(out.packs, 'humanity-instruments-synth-v1.tar')));
    const names = tar.map((e) => e.path);
    assert.ok(names.includes('waves/w_all/Sq/sq_0002.wav') && names.includes('waves/w_all/waves.ron'), 'the pack holds every wave');
    assert.match(tar.find((e) => e.path === 'pack.ron').data.toString(), /waves: \[\s*"w_all",?\s*\]/);
    const catalog = fs.readFileSync(out.catalog, 'utf8');
    assert.match(catalog, /waves: \[\s*\(id: "w_core", count: 2\),?\s*\]/);
    assert.match(catalog, /about: Some\("instrument_packs\.pack_about\.synth"\)/);
    assert.match(catalog, /credits: \[\s*"someone",?\s*\]/);
    r.wavesets[1].count = 5;
    await assert.rejects(build(work, r, path.join(work, 'packs-refused')), /6 waves chosen, the recipe says 5/, 'a count that does not match is refused');
  } finally {
    fs.rmSync(work, { recursive: true, force: true });
  }
});
