// The encoding check (scripts/check-encoding.js): text garbled by a PowerShell 5.1 round
// trip never reaches a commit. Pure node plus git, in throwaway repositories under the OS
// temp folder (never this checkout), so it runs in `just rig-tests`.
//
// The garbling here is made with ICU's windows-1252 decoder (TextDecoder), NOT with the
// script's own table, so a wrong entry in that table shows up as a failure instead of
// agreeing with itself. Every test string is built from code points: this file holds no
// garbled text of its own, so `check-encoding.js --all` stays clean over it.
//
// Mutation checks, run 2026-10-19 when this was written (each applied to the script, the
// suite run, the script restored):
//   no byte-order-mark check at all      -> 3 fail (round trip, new file, UTF-16)
//   ignore whether HEAD already had one  -> "a file that already had a byte-order mark ..."
//   findMojibake always returns []       -> 6 fail, the exhaustive sweep first
//   the per-line exemption dropped       -> "a line marked encoding-exempt passes"
//   any one of the four UTF-8 validity   -> "a run that would not be valid UTF-8 ..."
//     guards dropped (E0, ED, F0, F4)       (it survived until that test was added)
//   one wrong Windows-1252 table entry   -> the exhaustive sweep (0x99 -> U+2123)

const { test } = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');

const SCRIPT = path.join(__dirname, '..', 'check-encoding.js');
const { findMojibake } = require(SCRIPT);

const cp = (...codes) => String.fromCodePoint(...codes);
const EM_DASH = cp(0x2014);
const ARROW = cp(0x2192);
const E_ACUTE = cp(0xe9);
const SEEDLING = cp(0x1f331);
const KANJI = cp(0x6f22, 0x5b57);
const NBSP = cp(0xa0);
const DEGREE = cp(0xb0);
const BOX = cp(0x2500);
const S_CIRCUMFLEX = cp(0x15d); // UTF-8 C5 9D: 9D is a byte Windows-1252 leaves undefined

/** What PowerShell 5.1 Get-Content does to UTF-8 without a byte-order mark. */
const asWindows1252 = (text) => new TextDecoder('windows-1252').decode(Buffer.from(text, 'utf8'));
/** The whole round trip: garbled, CRLF, and a UTF-8 byte-order mark in front. */
const roundTrip = (text) => {
  const lines = asWindows1252(text).split(/\r?\n/);
  if (lines[lines.length - 1] === '') lines.pop();
  return Buffer.concat([Buffer.from([0xef, 0xbb, 0xbf]), Buffer.from(lines.join('\r\n') + '\r\n', 'utf8')]);
};

const CLEAN = [
  `# Notes ${EM_DASH} the plan`,
  `step one ${ARROW} step two`,
  `caf${E_ACUTE} au lait, 20${DEGREE}C,${NBSP}non-breaking`,
  `grow it ${SEEDLING} ${KANJI}`,
  `${BOX}${BOX}${BOX} a rule ${BOX}${BOX}${BOX}`,
  `Esperanto ${S_CIRCUMFLEX}ipo`,
  '',
].join('\n');

function repo() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-encoding-'));
  const g = (...args) => {
    const r = spawnSync('git', args, { cwd: dir, encoding: 'utf8' });
    if (r.status !== 0) throw new Error(`git ${args.join(' ')}: ${r.stderr}`);
    return r.stdout;
  };
  g('init', '-q');
  g('config', 'user.name', 'test');
  g('config', 'user.email', 'test@example.invalid');
  g('config', 'core.autocrlf', 'false');
  g('config', 'commit.gpgsign', 'false');
  const write = (f, content) => {
    fs.mkdirSync(path.dirname(path.join(dir, f)), { recursive: true });
    fs.writeFileSync(path.join(dir, f), content);
  };
  const run = (...args) => {
    const r = spawnSync(process.execPath, [SCRIPT, ...args], { cwd: dir, encoding: 'utf8' });
    return { code: r.status, out: r.stdout + r.stderr };
  };
  const cleanup = () => fs.rmSync(dir, { recursive: true, force: true });
  return { dir, g, write, run, cleanup };
}

function withCommittedNotes(fn) {
  const r = repo();
  try {
    r.write('notes.md', CLEAN);
    r.g('add', 'notes.md');
    r.g('commit', '-q', '--no-verify', '-m', 'clean notes');
    fn(r);
  } finally {
    r.cleanup();
  }
}

test('every character U+0080 to U+FFFF, garbled the PowerShell way, is caught and named', () => {
  let checked = 0;
  for (let c = 0x80; c <= 0xffff; c++) {
    if (c >= 0xd800 && c <= 0xdfff) continue; // surrogates are not characters
    const ch = cp(c);
    const hits = findMojibake(`x ${asWindows1252(ch)} y`);
    assert.strictEqual(hits.length, 1, `U+${c.toString(16)} garbled as ${JSON.stringify(asWindows1252(ch))}: ${JSON.stringify(hits)}`);
    assert.strictEqual(hits[0].original, ch);
    checked++;
  }
  for (const c of [0x10000, 0x1f331, 0x1f600, 0x10ffff]) {
    const hits = findMojibake(asWindows1252(cp(c)));
    assert.strictEqual(hits.length, 1);
    assert.strictEqual(hits[0].original, cp(c));
    checked++;
  }
  assert.ok(checked > 63000, `swept ${checked}`);
});

test('ordinary text in many languages is never called garbled', () => {
  const samples = [
    CLEAN,
    `S${cp(0xc3)}O PAULO, ${cp(0xc3)}gua, na${cp(0xe7, 0xe3)}o`, // Portuguese capitals: A-tilde before a letter
    `${cp(0xbf)}Qu${E_ACUTE}? ${cp(0xa1)}Ol${cp(0xe1)}! a${cp(0xf1)}o, ma${cp(0xf1)}ana`,
    `Stra${cp(0xdf)}e, ${cp(0xfc)}ber, ${cp(0xe0)} la carte, gar${cp(0xe7)}on, na${cp(0xef)}ve`,
    `${cp(0x41f, 0x440, 0x438, 0x432, 0x435, 0x442)} ${cp(0x3b1, 0x3b2, 0x3b3)} ${KANJI} ${cp(0x3053, 0x3093)}`,
    `price ${cp(0xa3)}5 ${cp(0xa9)} 2026 ${cp(0xae)} ${cp(0x2122)} ${cp(0xb1)}0.5 ${cp(0xd7)} ${cp(0xf7)}`,
    `${cp(0x2018)}quoted${cp(0x2019)} ${cp(0x201c)}double${cp(0x201d)} ${cp(0x2026)} ${cp(0x2022)} ${cp(0x2013)}`,
  ];
  for (const s of samples) assert.deepStrictEqual(findMojibake(s), [], s);
});

test('a run that would not be valid UTF-8 is not called garbled (fewer false alarms)', () => {
  const cont = (b) => cp(b < 0xa0 ? [0x20ac, 0x81, 0x201a][b - 0x80] || b : b);
  // E0 80 80: overlong. ED A0 80: a surrogate. F0 80 80 80: overlong. F4 90 80 80: past U+10FFFF.
  assert.deepStrictEqual(findMojibake(cp(0xe0) + cont(0x80) + cont(0x80)), []);
  assert.deepStrictEqual(findMojibake(cp(0xed) + cont(0xa0) + cont(0x80)), []);
  assert.deepStrictEqual(findMojibake(cp(0xf0) + cont(0x80) + cont(0x80) + cont(0x80)), []);
  assert.deepStrictEqual(findMojibake(cp(0xf4, 0x90) + cont(0x80) + cont(0x80)), []);
  // A lead with too few continuations, as real accented text has.
  assert.deepStrictEqual(findMojibake(`${cp(0xe9)}${cp(0x2026)} fin`), []);
});

test('a tracked file round-tripped through PowerShell is refused: the mark and the garbling', () => {
  withCommittedNotes((r) => {
    r.write('notes.md', roundTrip(CLEAN));
    r.g('add', 'notes.md');
    const { code, out } = r.run('--staged');
    assert.strictEqual(code, 1, out);
    assert.match(out, /byte-order mark \(EF BB BF\) is at the start of the file and was not before/);
    assert.match(out, new RegExp(`is "${EM_DASH}" read as Windows-1252`));
    // Every non-ASCII character garbled, plus the mark (the report lists the first six).
    const nonAscii = [...CLEAN].filter((c) => c.codePointAt(0) > 0x7f).length;
    assert.match(out, new RegExp(`x check-encoding: ${nonAscii + 1} problem\\(s\\)`));
    assert.match(out, /--fix notes\.md/);
  });
});

test('a clean edit with plenty of non-ASCII passes', () => {
  withCommittedNotes((r) => {
    r.write('notes.md', CLEAN + `more ${EM_DASH} ${ARROW} ${SEEDLING}\n`);
    r.g('add', 'notes.md');
    const { code, out } = r.run('--staged');
    assert.strictEqual(code, 0, out);
  });
});

test('garbling in one added line is refused with its line number; old lines are not re-judged', () => {
  withCommittedNotes((r) => {
    r.write('notes.md', CLEAN + `pasted ${asWindows1252(EM_DASH)} here\n`);
    r.g('add', 'notes.md');
    const { code, out } = r.run('--staged');
    assert.strictEqual(code, 1, out);
    assert.match(out, /line 7:8 /);
    assert.doesNotMatch(out, /byte-order mark \(EF BB BF\) is at the start/);
  });
});

test('a line marked encoding-exempt passes', () => {
  withCommittedNotes((r) => {
    r.write('notes.md', CLEAN + `the em dash garbles to ${asWindows1252(EM_DASH)} <!-- encoding-exempt: documenting it -->\n`);
    r.g('add', 'notes.md');
    const { code, out } = r.run('--staged');
    assert.strictEqual(code, 0, out);
  });
});

test('a NEW file with a byte-order mark is refused, unless its first line is exempt', () => {
  withCommittedNotes((r) => {
    r.write('new.txt', Buffer.concat([Buffer.from([0xef, 0xbb, 0xbf]), Buffer.from('hello\n')]));
    r.g('add', 'new.txt');
    let res = r.run('--staged');
    assert.strictEqual(res.code, 1, res.out);
    assert.match(res.out, /new\.txt\n    a UTF-8 byte-order mark \(EF BB BF\) is at the start of the file\n/);
    r.write('new.txt', Buffer.concat([Buffer.from([0xef, 0xbb, 0xbf]), Buffer.from('# encoding-exempt: PowerShell 5.1 needs it\nhello\n')]));
    r.g('add', 'new.txt');
    res = r.run('--staged');
    assert.strictEqual(res.code, 0, res.out);
  });
});

test('a file that already had a byte-order mark may keep it through an ordinary edit', () => {
  const r = repo();
  try {
    const bom = Buffer.from([0xef, 0xbb, 0xbf]);
    r.write('legacy.ps1', Buffer.concat([bom, Buffer.from('Write-Host one\n')]));
    r.g('add', 'legacy.ps1');
    r.g('commit', '-q', '--no-verify', '-m', 'had one already');
    r.write('legacy.ps1', Buffer.concat([bom, Buffer.from('Write-Host one\nWrite-Host two\n')]));
    r.g('add', 'legacy.ps1');
    const { code, out } = r.run('--staged');
    assert.strictEqual(code, 0, out);
  } finally {
    r.cleanup();
  }
});

test('a file rewritten as UTF-16 (PowerShell > and Out-File) is refused', () => {
  withCommittedNotes((r) => {
    r.write('notes.md', Buffer.concat([Buffer.from([0xff, 0xfe]), Buffer.from(CLEAN, 'utf16le')]));
    r.g('add', 'notes.md');
    const { code, out } = r.run('--staged');
    assert.strictEqual(code, 1, out);
    assert.match(out, /the file is now UTF-16 \(utf16le, byte-order mark FF FE\)/);
  });
});

test('a binary file is not read for garbling', () => {
  withCommittedNotes((r) => {
    r.write('blob.bin', Buffer.concat([Buffer.from([0, 1, 2]), Buffer.from(asWindows1252(EM_DASH.repeat(20)), 'utf8')]));
    r.g('add', 'blob.bin');
    const { code, out } = r.run('--staged');
    assert.strictEqual(code, 0, out);
  });
});

test('without --staged it sees unstaged and untracked damage, and --all sees the tracked files', () => {
  withCommittedNotes((r) => {
    assert.strictEqual(r.run().code, 0);
    assert.strictEqual(r.run('--all').code, 0);
    r.write('notes.md', roundTrip(CLEAN)); // not staged
    r.write('loose.md', `loose ${asWindows1252(ARROW)}\n`); // untracked
    const wt = r.run();
    assert.strictEqual(wt.code, 1, wt.out);
    assert.match(wt.out, /notes\.md/);
    assert.match(wt.out, /loose\.md/);
    assert.strictEqual(r.run('--staged').code, 0, 'nothing is staged');
    const all = r.run('--all');
    assert.strictEqual(all.code, 1, all.out);
    assert.match(all.out, /notes\.md/);
    assert.doesNotMatch(all.out, /loose\.md/, '--all reads tracked files only');
  });
});

test('--fix turns a round-tripped file back into the original, byte for byte apart from line ends', () => {
  withCommittedNotes((r) => {
    r.write('notes.md', roundTrip(CLEAN));
    const res = r.run('--fix', 'notes.md');
    assert.strictEqual(res.code, 0, res.out);
    assert.match(res.out, /removed the byte-order mark/);
    const back = fs.readFileSync(path.join(r.dir, 'notes.md'), 'utf8').replace(/\r\n/g, '\n');
    assert.strictEqual(back, CLEAN);
    r.write('notes.md', Buffer.concat([Buffer.from([0xff, 0xfe]), Buffer.from(asWindows1252(CLEAN), 'utf16le')]));
    r.run('--fix', 'notes.md');
    assert.strictEqual(fs.readFileSync(path.join(r.dir, 'notes.md'), 'utf8'), CLEAN);
  });
});

test('a repository with no commits yet is checked against nothing', () => {
  const r = repo();
  try {
    r.write('first.md', `first ${asWindows1252(E_ACUTE)}\n`);
    r.g('add', 'first.md');
    const { code, out } = r.run('--staged');
    assert.strictEqual(code, 1, out);
    assert.match(out, /first\.md/);
  } finally {
    r.cleanup();
  }
});
