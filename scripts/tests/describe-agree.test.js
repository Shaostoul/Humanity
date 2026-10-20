// The attach step's words about a file's hidden information must be the same in both apps
// (docs/design/files-showcase.md, "Files on the sender's device and in messages, exact", part 1):
// "This photo says where it was taken and which camera took it." The web says it with
// `HosStrip.describe` (web/shared/strip-metadata.js); the desktop with the Rust port of it,
// src/relay/core/describe_metadata.rs. This holds them together two ways:
//
// 1. The fixtures (tests/fixtures/describe/, written by scripts/make-describe-fixtures.js): every
//    file there must get exactly the answer in expected.json from the web here, and from the Rust
//    in its own tests (`describe_metadata_tests.rs`), so either side drifting fails a test.
// 2. Where rustc is installed: a small program is compiled around the Rust file itself (it uses
//    the standard library only for this), and it describes several thousand generated files
//    (scripts/lib/describe-corpus.js: random text with accents, emoji, control characters and
//    byte-order marks, random numbers in every GPS notation, random cuts and flipped bytes). Every
//    answer must equal the web's, field for field. The same program also describes each file from
//    disk (`describe_file`, which reads a video by its box headers only) and requires the same
//    answer as from memory. Set HOS_DESCRIBE_SKIP_RUST=1 to skip it.
//
// Run: node --test scripts/tests/describe-agree.test.js
//
// Seen red once each (2026-10-11), against a deliberately broken copy of the Rust:
// - the latitude of an ISO 6709 position read with three degree digits instead of two: "input 309
//   (mp4, named "a.b.heic"): the Rust's answer differs from the web's", coordinates null against
//   59.28111111111111, -69.00055555555555. The FIRST version of this test passed that broken copy:
//   its generated positions almost never came in the degrees-and-minutes form where the two
//   readings differ, so the generator now writes all three ISO 6709 forms with real values;
// - the text trimmed with Rust's own list of white space instead of JavaScript's: "input 113 (mp4,
//   named "clip.MOV")", place "﻿" against "çPixel 8" (U+FEFF is white space to JavaScript).

'use strict';
const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const REPO = path.join(__dirname, '..', '..');
const WEB = process.env.HOS_WEB_DIR || path.join(REPO, 'web');
const S = require(path.join(WEB, 'shared', 'strip-metadata.js'));
const corpus = require(path.join(REPO, 'scripts', 'lib', 'describe-corpus.js'));
const FIXTURES = path.join(REPO, 'tests', 'fixtures', 'describe');

/**
 * The web's answer with what Rust cannot hold taken out: a lone UTF-16 surrogate (half of an emoji
 * cut by a length limit) is dropped at the end of a string and replaced inside one.
 */
function normalise(v) {
  if (typeof v === 'string') {
    return v.replace(/[\uD800-\uDBFF]$/, '').replace(/[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/g, '�');
  }
  if (Array.isArray(v)) return v.map(normalise);
  if (v && typeof v === 'object') return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, normalise(x)]));
  return v;
}

test('the web gives every fixture the answer recorded for it', () => {
  const expected = JSON.parse(fs.readFileSync(path.join(FIXTURES, 'expected.json'), 'utf8'));
  const names = Object.keys(expected);
  assert.ok(names.length >= 50, `the fixtures are there (${names.length})`);
  for (const file of names) {
    const bytes = new Uint8Array(fs.readFileSync(path.join(FIXTURES, file)));
    const got = S.describe(bytes, expected[file].nameOrType, { coordinates: true });
    assert.deepStrictEqual(got, expected[file].answer, `${file}: the web's answer changed`);
  }
  // A spread worth having: places, cameras, recordings, HEIF, and files that could not be read through.
  const answers = names.map((n) => expected[n].answer);
  assert.ok(answers.filter((a) => a.coordinates).length >= 15, 'files with a position');
  assert.ok(answers.some((a) => a.summary && a.summary.startsWith('This recording')), 'a recording');
  assert.ok(answers.some((a) => a.kind === 'heif'), 'a HEIF photo');
  assert.ok(answers.filter((a) => !a.complete).length >= 10, 'files not read through');
});

function harnessSource() {
  const src = JSON.stringify(path.join(REPO, 'src', 'relay', 'core', 'describe_metadata.rs').replace(/\\/g, '/'));
  return `
#[path = ${src}]
mod describe_metadata;

fn take<'a>(input: &'a [u8], i: &mut usize) -> &'a [u8] {
    let n = u32::from_le_bytes([input[*i], input[*i + 1], input[*i + 2], input[*i + 3]]) as usize;
    let b = &input[*i + 4..*i + 4 + n];
    *i += 4 + n;
    b
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let input = std::fs::read(&args[1]).expect("read the inputs");
    let tmp = std::path::PathBuf::from(&args[3]);
    let mut out = String::new();
    let mut i = 0usize;
    let mut k = 0usize;
    while i < input.len() {
        let name = String::from_utf8_lossy(take(&input, &mut i)).into_owned();
        let data = take(&input, &mut i);
        let mem = describe_metadata::describe(data, &name).to_json(true);
        std::fs::write(&tmp, data).expect("write the file");
        let disk = describe_metadata::describe_file(&tmp, &name).expect("describe the file").to_json(true);
        if disk != mem {
            eprintln!("input {k}: from disk {disk}\\n          from memory {mem}");
            std::process::exit(3);
        }
        out.push_str(&mem);
        out.push('\\n');
        k += 1;
    }
    std::fs::write(&args[2], out).expect("write the answers");
}
`;
}

test("agrees with the Rust's describe on every fixture and thousands of generated files", (t) => {
  if (process.env.HOS_DESCRIBE_SKIP_RUST === '1') return t.skip('HOS_DESCRIBE_SKIP_RUST=1');
  const rustc = spawnSync('rustc', ['--version'], { encoding: 'utf8' });
  if (rustc.status !== 0) return t.skip('rustc is not installed here, so the Rust cannot be run');

  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'hum_describe_agree_'));
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  fs.writeFileSync(path.join(dir, 'main.rs'), harnessSource());
  const exe = path.join(dir, process.platform === 'win32' ? 'harness.exe' : 'harness');
  const built = spawnSync('rustc', ['--edition', '2021', '-A', 'warnings', '-C', 'opt-level=1', '-C', 'debuginfo=0',
    '-o', exe, path.join(dir, 'main.rs')], { encoding: 'utf8', cwd: dir });
  assert.strictEqual(built.status, 0, `the harness around the Rust describe must compile:\n${built.stderr}`);

  const inputs = corpus.fixed().map(([, n, b]) => [n, b]).concat(corpus.random(6000, 0x0de5c41b).map(([, n, b]) => [n, b]));
  const le = (n) => [n & 0xff, (n >>> 8) & 0xff, (n >>> 16) & 0xff, (n >>> 24) & 0xff];
  const packed = corpus.cat(...inputs.flatMap(([n, b]) => { const nb = corpus.utf8(n); return [le(nb.length), nb, le(b.length), b]; }));
  fs.writeFileSync(path.join(dir, 'in.bin'), packed);
  const ran = spawnSync(exe, [path.join(dir, 'in.bin'), path.join(dir, 'out.txt'), path.join(dir, 'one.bin')], { encoding: 'utf8', timeout: 300000 });
  assert.strictEqual(ran.status, 0, `the harness must run, and describe each file the same from disk as from memory:\n${ran.stderr}`);

  const lines = fs.readFileSync(path.join(dir, 'out.txt'), 'utf8').split('\n').filter(Boolean);
  assert.strictEqual(lines.length, inputs.length, 'one answer per file');
  let located = 0, cameras = 0, unfinished = 0, places = 0;
  for (let i = 0; i < inputs.length; i++) {
    const [name, b] = inputs[i];
    const web = normalise(S.describe(b, name, { coordinates: true }));
    const rust = JSON.parse(lines[i]);
    assert.deepStrictEqual(rust, web, `input ${i} (${web.kind}, named ${JSON.stringify(name)}): the Rust's answer differs from the web's`);
    if (web.coordinates) located++;
    if (web.hasCamera) cameras++;
    if (web.place) places++;
    if (!web.complete) unfinished++;
  }
  t.diagnostic(`${inputs.length} files: ${located} with a position, ${places} with a place name, ${cameras} naming a camera, ${unfinished} not read through; all the same in both`);
  assert.ok(located > 400 && cameras > 1500 && places > 200 && unfinished > 300, `a real spread: ${located} ${cameras} ${places} ${unfinished}`);
});
