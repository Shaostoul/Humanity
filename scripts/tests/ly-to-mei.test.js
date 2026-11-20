// The LilyPond-to-MEI converter (scripts/ly-to-mei.js; docs/design/score-player.md, rung 2): small
// LilyPond files written here, converted, and the MEI read back for what a musician would check:
// the pitches \relative gives, the bars a \partial and an \alternative make, the accidentals an
// engraver writes (and only those), a tuplet cut at a bar line, notes under an octave line written
// an octave from where they sound, grace notes, tremolos, and the source checked by its SHA-256.
//
// Run: node --test scripts/tests/ly-to-mei.test.js
//
// Seen RED 2026-11-20 with the \relative rule taking the octave of the note before instead of the
// nearest (`oct = ref.oct + marks`): "actual: 'f3 f4 b1 c1 d1 e1 f1 d1 f1', expected: 'f3 f4 b1 c2 d2
// e2 f2 d2 f2'". (The other tests were written against the shipped conversions and pass with it.)

const { test } = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const ly = require('../ly-to-mei.js');

/** Convert LilyPond `text` (a whole file) with `settings`; the MEI text. */
function convert(text, settings = {}) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'ly-to-mei-'));
  const file = path.join(dir, 'piece.ly');
  fs.writeFileSync(file, text);
  try {
    return ly.convert(file, settings);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

/** One staff's music in a \score. */
const score = (music) => `\\version "2.18.2"\n\\score { \\new Staff { ${music} } }\n`;

/** Every note as "pname+oct" in order, with its accidental when written ("f4s"). */
const notes = (mei) =>
  [...mei.matchAll(/<note ([^>]*)\/>/g)].map((m) => {
    const attr = (k) => (new RegExp(` ${k}="([^"]*)"`).exec(' ' + m[1]) || [])[1] || '';
    return attr('pname') + attr('oct') + attr('accid');
  });

test('relative pitches: each note within a fourth of the one before, chords from their first note', () => {
  // The Mountain King's opening, as the Mutopia file writes it.
  const mei = convert(score('\\relative c\'\' { \\key b \\minor \\time 4/4 <fis,, fis\'>1 b,,8 cis d e fis d fis4 }'));
  assert.strictEqual(notes(mei).join(' '), 'f3 f4 b1 c2 d2 e2 f2 d2 f2');
  assert.match(mei, /keysig="2s"/, 'B minor is two sharps, so F sharp and C sharp need no accidental');
});

test('a pickup bar, repeat signs and endings: the bars as the score prints them', () => {
  const mei = convert(score('\\time 3/8 \\repeat volta 2 { \\partial 8 e\'\'16 dis\'\' e\'\'8 a\'4 } \\alternative { { a\'4 } { a\'8 r16 b\' c\'\' d\'\' } } c\'\'4.'));
  const bars = [...mei.matchAll(/<measure n="(\d+)"([^>]*)>/g)].map((m) => m[1] + m[2]);
  assert.deepStrictEqual(bars, ['0', '1', '2 right="rptend"', '3', '4 right="end"']);
  assert.match(mei, /<ending n="1">\s*<measure n="2" right="rptend">/);
  assert.match(mei, /<ending n="2">\s*<measure n="3">/);
});

test('accidentals as an engraver writes them: the key, then the bar remembers; a tie carries it over', () => {
  // A minor: an F sharp, the next F in the bar with no sign, the bar after a natural is needed.
  // The tied G sharp crosses the bar line with no new sign; the next G in that bar is sharp too.
  const mei = convert(score('\\key a \\minor \\time 2/4 fis\'8 fis\' gis\'4~ gis\'8 gis\' f\'4'));
  assert.deepStrictEqual(notes(mei), ['f4s', 'f4', 'g4s', 'g4', 'g4s', 'f4']);
  assert.match(mei, /tie="i"/);
  assert.match(mei, /tie="t"/);
  // Flats and the Dutch names: ees, as, bes in C minor need none; a natural is written.
  const flats = convert(score('\\key c \\minor \\time 4/4 ees\'4 as\' bes\' e\'4'));
  assert.deepStrictEqual(notes(flats), ['e4', 'a4', 'b4', 'e4n']);
});

test('a tuplet across a bar line is cut into one tuplet per bar', () => {
  const mei = convert(score('\\time 3/8 r8 \\tuplet 3/2 { a16 c\' e\' a\' c\'\' e\'\' d\'\' c\'\' b\' } r8'));
  const bar1 = mei.split('<measure n="1"')[1].split('</measure>')[0];
  const bar2 = mei.split('<measure n="2"')[1].split('</measure>')[0];
  assert.strictEqual((bar1.match(/<note /g) || []).length, 6, 'six triplet sixteenths fill the first bar after the rest');
  assert.match(bar1, /<tuplet num="3" numbase="2">/);
  assert.match(bar2, /<tuplet num="3" numbase="2">/);
  assert.strictEqual((bar2.match(/<note /g) || []).length, 3);
});

test('under an octave line a note is written an octave below where it sounds', () => {
  // LilyPond writes the sounding pitch (a''' is A6); MEI writes what the player reads (A5) and the line.
  const mei = convert(score('\\time 2/4 a\'\'4 \\ottava #1 a\'\'\'4 \\ottava #0 a\'\'2'));
  assert.deepStrictEqual(notes(mei), ['a5', 'a5', 'a5']);
  assert.match(mei, /<octave staff="1" dis="8" dis.place="above" startid="#(\w+)" endid="#\1"\/>/);
});

test('grace notes, a measured tremolo and a written-out fingered one', () => {
  const mei = convert(score('\\time 4/4 \\appoggiatura { gis\'16 a\' } b\'4 \\grace c\'\'16 d\'\'4 c\'2:8'));
  assert.strictEqual((mei.match(/grace="acc"/g) || []).length, 2);
  assert.strictEqual((mei.match(/grace="unacc"/g) || []).length, 1);
  assert.match(mei, /<bTrem unitdur="8"><note [^>]*pname="c" oct="4" dur="2"/);
  const fingered = convert(score('\\time 2/4 \\repeat tremolo 4 { b16 b\' }'));
  assert.strictEqual((fingered.match(/<note /g) || []).length, 8, 'four times two sixteenths');
});

test('a dynamic, a hairpin to the next dynamic, articulations, a fermata and a tempo', () => {
  const mei = convert(score('\\time 4/4 \\tempo "Allegro" 4=120 c\'4\\p\\< d\' e\'-. f\'-> g\'1\\f\\fermata'));
  assert.match(mei, /<tempo staff="1" tstamp="1" mm="120" mm.unit="4" midi.bpm="120">Allegro<\/tempo>/);
  assert.match(mei, /<dynam staff="1" tstamp="1" startid="#\w+">p<\/dynam>/);
  assert.match(mei, /<hairpin form="cres" staff="1" tstamp="1" startid="#\w+" endid="#\w+"\/>/);
  assert.match(mei, /artic="stacc"/);
  assert.match(mei, /artic="acc"/);
  assert.match(mei, /<fermata staff="1" tstamp="1" startid="#\w+"\/>/);
});

test('two voices on one staff are two layers; a transposing staff says so', () => {
  const mei = convert('\\score { \\new Staff { \\transposition bes \\time 2/4 << { c\'\'2 } \\\\ { e\'4 r4 } >> } }');
  assert.match(mei, /<layer n="1"><note [^>]*pname="c" oct="5"/);
  assert.match(mei, /<layer n="2"><note [^>]*pname="e" oct="4"[^>]*\/><rest/);
  assert.match(mei, /trans.semi="-2" trans.diat="-1"/, 'a clarinet in B flat sounds a tone below what it reads');
});

test('the pieces file names a SHA-256 for every source, and a changed file is refused', () => {
  const conf = JSON.parse(fs.readFileSync(path.join(__dirname, '..', 'ly-to-mei.pieces.json'), 'utf8'));
  for (const [id, p] of Object.entries(conf.pieces)) {
    assert.ok(p.sha256[p.main], `${id}: its main file has a SHA-256`);
    for (const h of Object.values(p.sha256)) assert.match(h, /^[0-9a-f]{64}$/, `${id}: a SHA-256`);
    assert.ok(fs.existsSync(path.join(__dirname, '..', '..', 'data', 'music', 'scores', `${id}.mei`)), `${id}: its MEI is shipped`);
  }
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'ly-to-mei-src-'));
  try {
    fs.writeFileSync(path.join(dir, 'fur_Elise_WoO59.ly'), 'not the file');
    const { spawnSync } = require('node:child_process');
    const r = spawnSync(process.execPath, [path.join(__dirname, '..', 'ly-to-mei.js'), '--piece', 'fur_elise', '--src', dir], { encoding: 'utf8' });
    assert.notStrictEqual(r.status, 0);
    assert.match(r.stderr, /SHA-256/);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});
