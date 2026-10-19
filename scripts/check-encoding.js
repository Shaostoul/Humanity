#!/usr/bin/env node
/**
 * check-encoding.js: refuse text that a PowerShell round trip has garbled.
 *
 * THE FAILURE (CLAUDE.md "PowerShell Get-Content and Set-Content", twice in one hour on
 * 2026-07-06, both on chat.rs): PowerShell 5.1's Get-Content reads a UTF-8 file that has no
 * byte-order mark as Windows-1252, so every non-ASCII character arrives as two to four wrong
 * ones; Set-Content -Encoding utf8 then writes those wrong characters back as UTF-8 and puts
 * a byte-order mark (EF BB BF) in front. A one-line append became a 570-line diff of
 * garbage. The `>` operator and Out-File do worse: they rewrite the file as UTF-16.
 * Until now only a sentence in CLAUDE.md stood between that and a commit.
 *
 * WHAT IT REFUSES
 *   1. A file that GAINS a byte-order mark (UTF-8 EF BB BF, or UTF-16 FF FE / FE FF) it did
 *      not have. A new file counts as not having had one. On 2026-10-19 not one of the
 *      4,390 tracked files had a byte-order mark, so any that appears is new.
 *   2. Mojibake in ADDED lines: a run of characters that are exactly the bytes of one UTF-8
 *      character read as Windows-1252 (the em dash, E2 80 94, comes back as the three
 *      characters U+00E2 U+20AC U+201D; e-acute, C3 A9, as U+00C3 U+00A9).
 *      Detected generically, not from a short list: a UTF-8 lead byte (C2-F4) followed by
 *      exactly the continuation bytes (80-BF) UTF-8 requires, all in their Windows-1252
 *      dress, and valid UTF-8 once turned back into bytes. Ordinary text almost never has
 *      that shape: on 2026-10-19 `--all` over all 4,390 tracked files (French, Spanish,
 *      Japanese and Chinese locale files, box drawing, emoji, arrows) found not one.
 *   Binary files (a NUL byte in the first 8000, git's own test) are skipped for (2).
 *
 * EXEMPTION: a line containing `encoding-exempt: <reason>` (in whatever comment the file
 * uses) is not checked. For a byte-order mark the file actually needs, put the marker on
 * its first line. This script names its examples by code point (U+00E2) so it needs none.
 *
 * MODES
 *   node scripts/check-encoding.js            working tree vs HEAD: staged, unstaged and
 *                                             untracked changes (what `just check-encoding` runs)
 *   node scripts/check-encoding.js --staged   the staged changes only (the pre-commit hook)
 *   node scripts/check-encoding.js --all      every tracked file, whole content (`just lints`)
 *   node scripts/check-encoding.js --fix <file>...
 *                                             repair in place: drop a byte-order mark HEAD did
 *                                             not have, turn UTF-16 back into UTF-8, and turn
 *                                             every garbled run back into the character it was
 * Exit 0 clean, 1 problems found, 2 could not run (not a git checkout, bad arguments).
 *
 * PROVEN RED, 2026-10-19, in a worktree. The agent sandbox refused to start powershell.exe,
 * so the round trip was reproduced in node exactly as PowerShell 5.1 does it: bytes decoded
 * with ICU's windows-1252 (not this script's table), lines joined with CRLF, written as
 * UTF-8 with a byte-order mark. Three runs, each staged and checked with `--staged`:
 *   a garbled COPY of docs/history/gotcha-incidents.md (a new file), exit 1:
 *     x check-encoding: 13 problem(s) in 1 file(s), in the staged changes.
 *       scratch-encoding-red.md
 *         a UTF-8 byte-order mark (EF BB BF) is at the start of the file
 *         line 30:436  "..." (U+00E2 U+20AC U+201D) is "<em dash>" read as Windows-1252 and saved again
 *         line 54:392  "..." (U+00E2 U+2020 U+2019) is "<right arrow>" read as Windows-1252 ...
 *   the tracked Justfile garbled IN PLACE, exit 1:
 *     x check-encoding: 2540 problem(s) in 1 file(s), in the staged changes.
 *       Justfile
 *         a UTF-8 byte-order mark (EF BB BF) is at the start of the file and was not before
 *         line 5:3  "..." (U+00E2 U+201D U+20AC) is "<box drawing>" read as Windows-1252 ...
 *   the Justfile as UTF-16 (`>` / Out-File), exit 1:
 *         the file is now UTF-16 (utf16le, byte-order mark FF FE); PowerShell's > and Out-File write that
 * After each, `--fix` gave back the original exactly: 0 lines of staged diff on the Justfile.
 * To repeat it with PowerShell itself, on a scratch copy:
 *   powershell -NoProfile -Command "Get-Content x.md | Set-Content -Encoding utf8 x.md"
 *   git add x.md; node scripts/check-encoding.js --staged
 * scripts/tests/check-encoding.test.js repeats the garbling on every run, in a throwaway repo.
 */
'use strict';

const { execFileSync, spawnSync } = require('child_process');
const fs = require('fs');
const path = require('path');

// ---------------------------------------------------------------------------------------
// The Windows-1252 table. Bytes 0x80-0x9F are where it differs from Latin-1; the five bytes
// it leaves undefined (81 8D 8F 90 9D) come through Windows' decoder as the C1 control of
// the same number, which is what PowerShell then writes back.
// ---------------------------------------------------------------------------------------
const CP1252_80_9F = [
  0x20ac, 0x0081, 0x201a, 0x0192, 0x201e, 0x2026, 0x2020, 0x2021,
  0x02c6, 0x2030, 0x0160, 0x2039, 0x0152, 0x008d, 0x017d, 0x008f,
  0x0090, 0x2018, 0x2019, 0x201c, 0x201d, 0x2022, 0x2013, 0x2014,
  0x02dc, 0x2122, 0x0161, 0x203a, 0x0153, 0x009d, 0x017e, 0x0178,
];
/** Character code -> the Windows-1252 byte it stands for (0x80-0xFF only). */
const CHAR_TO_BYTE = new Map();
for (let b = 0x80; b <= 0xff; b++) CHAR_TO_BYTE.set(b < 0xa0 ? CP1252_80_9F[b - 0x80] : b, b);

const EXEMPT = /encoding-exempt:\s*\S/;

/**
 * Every garbled run in one line of text: [{ col, seq, original }]. `col` is 1-based.
 */
function findMojibake(line) {
  const hits = [];
  for (let i = 0; i < line.length; i++) {
    const lead = line.charCodeAt(i);
    let need = 0;
    if (lead >= 0xc2 && lead <= 0xdf) need = 1;
    else if (lead >= 0xe0 && lead <= 0xef) need = 2;
    else if (lead >= 0xf0 && lead <= 0xf4) need = 3;
    if (!need) continue;
    const bytes = [lead];
    for (let k = 1; k <= need; k++) {
      const b = CHAR_TO_BYTE.get(line.charCodeAt(i + k));
      if (b === undefined || b < 0x80 || b > 0xbf) break;
      bytes.push(b);
    }
    if (bytes.length !== need + 1) continue;
    // The second byte's narrower ranges, so only VALID UTF-8 counts.
    const b1 = bytes[1];
    if (lead === 0xe0 && b1 < 0xa0) continue;
    if (lead === 0xed && b1 > 0x9f) continue;
    if (lead === 0xf0 && b1 < 0x90) continue;
    if (lead === 0xf4 && b1 > 0x8f) continue;
    hits.push({
      col: i + 1,
      seq: line.slice(i, i + need + 1),
      original: Buffer.from(bytes).toString('utf8'),
    });
    i += need;
  }
  return hits;
}

/** 'utf8' | 'utf16le' | 'utf16be' | null */
function bomKind(buf) {
  if (!buf) return null;
  if (buf.length >= 3 && buf[0] === 0xef && buf[1] === 0xbb && buf[2] === 0xbf) return 'utf8';
  if (buf.length >= 2 && buf[0] === 0xff && buf[1] === 0xfe) return 'utf16le';
  if (buf.length >= 2 && buf[0] === 0xfe && buf[1] === 0xff) return 'utf16be';
  return null;
}

function isBinary(buf) {
  return buf.subarray(0, 8000).includes(0);
}

/** Decode a file's bytes to text the way an editor would (honouring a byte-order mark). */
function decodeText(buf) {
  const kind = bomKind(buf);
  if (kind === 'utf8') return buf.subarray(3).toString('utf8');
  if (kind === 'utf16le') return buf.subarray(2).toString('utf16le');
  if (kind === 'utf16be') {
    const swapped = Buffer.from(buf.subarray(2));
    swapped.swap16();
    return swapped.toString('utf16le');
  }
  return buf.toString('utf8');
}

// ---------------------------------------------------------------------------------------
// git plumbing
// ---------------------------------------------------------------------------------------
const EMPTY_TREE = '4b825dc642cb6eb9a060e54bf8d69288fbee4904';

function git(root, args, input) {
  const r = spawnSync('git', ['-c', 'core.quotePath=false', ...args], {
    cwd: root,
    input,
    maxBuffer: 1 << 30,
  });
  if (r.status !== 0) {
    throw new Error(`git ${args.join(' ')} failed: ${(r.stderr || '').toString().trim()}`);
  }
  return r.stdout;
}

function repoRoot() {
  try {
    return execFileSync('git', ['rev-parse', '--show-toplevel'], { stdio: ['ignore', 'pipe', 'ignore'] })
      .toString()
      .trim();
  } catch {
    return null;
  }
}

function baseRef(root) {
  const r = spawnSync('git', ['rev-parse', '--verify', '-q', 'HEAD'], { cwd: root });
  return r.status === 0 ? 'HEAD' : EMPTY_TREE;
}

/** `git diff --raw -z` -> [{ status, oldOid, newOid, oldPath, path }] */
function rawEntries(root, diffArgs) {
  const out = git(root, ['diff', '--raw', '-z', '--abbrev=40', '--no-ext-diff', ...diffArgs]).toString('utf8');
  const parts = out.split('\0');
  const entries = [];
  for (let i = 0; i < parts.length; i++) {
    const head = parts[i];
    if (!head.startsWith(':')) continue;
    const [, , oldOid, newOid, status] = head.slice(1).split(' ');
    const letter = status[0];
    if (letter === 'R' || letter === 'C') {
      entries.push({ status: letter, oldOid, newOid, oldPath: parts[i + 1], path: parts[i + 2] });
      i += 2;
    } else {
      entries.push({ status: letter, oldOid, newOid, oldPath: parts[i + 1], path: parts[i + 1] });
      i += 1;
    }
  }
  return entries;
}

/** Read many blobs in one `git cat-file --batch`. */
function readBlobs(root, oids) {
  const want = [...new Set(oids.filter((o) => o && !/^0+$/.test(o)))];
  const map = new Map();
  if (!want.length) return map;
  const out = git(root, ['cat-file', '--batch'], want.join('\n') + '\n');
  let pos = 0;
  while (pos < out.length) {
    const nl = out.indexOf(0x0a, pos);
    if (nl < 0) break;
    const header = out.subarray(pos, nl).toString('utf8').split(' ');
    pos = nl + 1;
    if (header[1] === 'missing') continue;
    const size = Number(header[2]);
    map.set(header[0], out.subarray(pos, pos + size));
    pos += size + 1;
  }
  return map;
}

/** Added lines per path from a unified diff: Map(path -> [{ line, text }]). */
function addedLines(root, diffArgs) {
  const out = git(root, ['diff', '-U0', '--no-color', '--no-ext-diff', '--no-textconv', ...diffArgs]).toString('utf8');
  const map = new Map();
  let cur = null;
  let lineNo = 0;
  for (const raw of out.split('\n')) {
    const l = raw.replace(/\r$/, '');
    if (l.startsWith('diff --git ')) {
      cur = null;
      continue;
    }
    if (l.startsWith('+++ ')) {
      const p = l.slice(4);
      cur = p === '/dev/null' ? null : p.replace(/^b\//, '');
      if (cur && !map.has(cur)) map.set(cur, []);
      continue;
    }
    if (l.startsWith('--- ')) continue;
    const h = /^@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@/.exec(l);
    if (h) {
      lineNo = Number(h[1]);
      continue;
    }
    if (cur && l.startsWith('+')) {
      map.get(cur).push({ line: lineNo, text: l.slice(1) });
      lineNo += 1;
    }
  }
  return map;
}

function allLines(text) {
  return text.split(/\r?\n/).map((t, i) => ({ line: i + 1, text: t }));
}

// ---------------------------------------------------------------------------------------
// Checking one file
// ---------------------------------------------------------------------------------------

/**
 * Problems in one file. `newBuf` is its new bytes, `oldBuf` its bytes before (null when it
 * is new, or in --all mode, `undefined` meaning "no comparison: any mark counts"), `lines`
 * the lines to scan for garbling (null = all of them).
 */
function checkFile(file, newBuf, oldBuf, lines) {
  const problems = [];
  const text = decodeText(newBuf);
  const firstLine = text.split(/\r?\n/, 1)[0] || '';
  const gained = bomKind(newBuf);
  if (gained && !(oldBuf && bomKind(oldBuf) === gained) && !EXEMPT.test(firstLine)) {
    problems.push(
      gained === 'utf8'
        ? `a UTF-8 byte-order mark (EF BB BF) is at the start of the file${oldBuf ? ' and was not before' : ''}`
        : `the file is now UTF-16 (${gained}, byte-order mark ${gained === 'utf16le' ? 'FF FE' : 'FE FF'}); PowerShell's > and Out-File write that`,
    );
  }
  // A file that turned into UTF-16 is binary to git; garbling is checked on its text anyway.
  if (isBinary(newBuf) && !gained) return problems;
  for (const { line, text: t } of lines || allLines(text)) {
    if (EXEMPT.test(t)) continue;
    for (const hit of findMojibake(t)) {
      problems.push({ line, col: hit.col, seq: hit.seq, original: hit.original });
    }
  }
  return problems;
}

// ---------------------------------------------------------------------------------------
// Modes
// ---------------------------------------------------------------------------------------

function runStaged(root) {
  const base = baseRef(root);
  const entries = rawEntries(root, ['--cached', base]).filter((e) => 'ACMRT'.includes(e.status));
  const blobs = readBlobs(root, entries.flatMap((e) => [e.newOid, e.oldOid]));
  const added = addedLines(root, ['--cached', base]);
  const report = [];
  for (const e of entries) {
    const newBuf = blobs.get(e.newOid);
    if (!newBuf) continue;
    const oldBuf = e.status === 'A' ? null : blobs.get(e.oldOid) || null;
    const p = checkFile(e.path, newBuf, oldBuf, added.get(e.path) || []);
    if (p.length) report.push({ file: e.path, problems: p });
  }
  return { report, where: 'the staged changes', checked: entries.length };
}

function runWorktree(root) {
  const base = baseRef(root);
  const entries = rawEntries(root, [base]).filter((e) => 'ACMRT'.includes(e.status));
  const blobs = readBlobs(root, entries.map((e) => e.oldOid));
  const added = addedLines(root, [base]);
  const report = [];
  for (const e of entries) {
    let newBuf;
    try {
      newBuf = fs.readFileSync(path.join(root, e.path));
    } catch {
      continue;
    }
    const oldBuf = e.status === 'A' ? null : blobs.get(e.oldOid) || null;
    const p = checkFile(e.path, newBuf, oldBuf, added.get(e.path) || []);
    if (p.length) report.push({ file: e.path, problems: p });
  }
  const untracked = git(root, ['ls-files', '--others', '--exclude-standard', '-z']).toString('utf8').split('\0').filter(Boolean);
  for (const f of untracked) {
    let buf;
    try {
      buf = fs.readFileSync(path.join(root, f));
    } catch {
      continue;
    }
    if (buf.length > 16 * 1024 * 1024) continue;
    const p = checkFile(f, buf, null, null);
    if (p.length) report.push({ file: f, problems: p });
  }
  return { report, where: 'the changes against HEAD (staged, unstaged and untracked)', checked: entries.length + untracked.length };
}

/**
 * A file's bytes, or null for a binary one, reading only its first 8000 bytes to tell
 * (the repo holds hundreds of MB of models, textures and star catalogues).
 */
function readIfText(abs) {
  let fd;
  try {
    fd = fs.openSync(abs, 'r');
  } catch {
    return null; // deleted in the working tree
  }
  try {
    const head = Buffer.alloc(8000);
    const n = fs.readSync(fd, head, 0, 8000, 0);
    const sniff = head.subarray(0, n);
    // UTF-16 is full of NULs but is exactly what this check must see.
    if (isBinary(sniff) && !bomKind(sniff)) return null;
    if (n < 8000) return Buffer.from(sniff);
    return fs.readFileSync(abs);
  } finally {
    fs.closeSync(fd);
  }
}

function runAll(root) {
  const files = git(root, ['ls-files', '-z']).toString('utf8').split('\0').filter(Boolean);
  const report = [];
  for (const f of files) {
    const buf = readIfText(path.join(root, f));
    if (!buf) continue;
    const p = checkFile(f, buf, undefined, null);
    if (p.length) report.push({ file: f, problems: p });
  }
  return { report, where: 'the tracked files', checked: files.length };
}

function runFix(root, files) {
  let changed = 0;
  for (const f of files) {
    const abs = path.resolve(f);
    const buf = fs.readFileSync(abs);
    const rel = path.relative(root, abs).split(path.sep).join('/');
    let head = null;
    const r = spawnSync('git', ['show', `HEAD:${rel}`], { cwd: root, maxBuffer: 1 << 30 });
    if (r.status === 0) head = r.stdout;
    const kind = bomKind(buf);
    const keepBom = kind === 'utf8' && head && bomKind(head) === 'utf8';
    let text = decodeText(buf);
    let repaired = 0;
    text = text
      .split('\n')
      .map((l) => {
        if (EXEMPT.test(l)) return l;
        const hits = findMojibake(l);
        if (!hits.length) return l;
        let out = '';
        let at = 0;
        for (const h of hits) {
          out += l.slice(at, h.col - 1) + h.original;
          at = h.col - 1 + h.seq.length;
          repaired++;
        }
        return out + l.slice(at);
      })
      .join('\n');
    const bytes = Buffer.from(text, 'utf8');
    const outBuf = keepBom ? Buffer.concat([Buffer.from([0xef, 0xbb, 0xbf]), bytes]) : bytes;
    if (!outBuf.equals(buf)) {
      fs.writeFileSync(abs, outBuf);
      changed++;
    }
    const bomNote = kind && !keepBom ? `, ${kind === 'utf8' ? 'removed the byte-order mark' : `converted from ${kind} to UTF-8`}` : '';
    console.log(`${rel}: repaired ${repaired} garbled run(s)${bomNote}`);
  }
  console.log(changed ? `Now look at \`git diff\` before staging: a character Windows-1252 cannot hold may have become "?" and cannot be restored.` : 'Nothing to repair.');
  return 0;
}

// ---------------------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------------------

function describe(p) {
  if (typeof p === 'string') return p;
  const cps = [...p.seq].map((c) => 'U+' + c.codePointAt(0).toString(16).toUpperCase().padStart(4, '0')).join(' ');
  return `line ${p.line}:${p.col}  "${p.seq}" (${cps}) is "${p.original}" read as Windows-1252 and saved again`;
}

function printReport({ report, where }, mode) {
  const total = report.reduce((n, r) => n + r.problems.length, 0);
  console.log(`x check-encoding: ${total} problem(s) in ${report.length} file(s), in ${where}.`);
  console.log('');
  for (const { file, problems } of report) {
    console.log(`  ${file}`);
    for (const p of problems.slice(0, 6)) console.log(`    ${describe(p)}`);
    if (problems.length > 6) console.log(`    ... and ${problems.length - 6} more in this file`);
  }
  const files = report.map((r) => r.file).join(' ');
  console.log('');
  console.log('  Why it matters: this is what a PowerShell 5.1 round trip does to a UTF-8 file.');
  console.log('  Get-Content reads it as Windows-1252, so every non-ASCII character turns into two');
  console.log('  to four wrong ones, and Set-Content -Encoding utf8 saves them with a byte-order mark');
  console.log('  in front (> and Out-File save UTF-16 instead). Committed, every reader sees garbage,');
  console.log('  and the diff buries the real change under hundreds of rewritten lines.');
  console.log('');
  console.log('  Fix: repair it in place, then check the diff:');
  console.log(`    node scripts/check-encoding.js --fix ${files}`);
  console.log('    git diff -- <file>');
  console.log('  Edit files with the Edit tool or a node script, never Get-Content/Set-Content.');
  console.log('  A line that must hold such text: add `encoding-exempt: <reason>` in a comment on it.');
  if (mode === 'staged') console.log('  (bypass in a real emergency: git commit --no-verify)');
}

function main(argv) {
  const root = repoRoot();
  if (!root) {
    console.error('check-encoding: not inside a git checkout');
    return 2;
  }
  const args = argv.slice(2);
  if (args[0] === '--fix') {
    if (args.length < 2) {
      console.error('usage: node scripts/check-encoding.js --fix <file>...');
      return 2;
    }
    return runFix(root, args.slice(1));
  }
  const mode = args.includes('--staged') ? 'staged' : args.includes('--all') ? 'all' : 'worktree';
  const result = mode === 'staged' ? runStaged(root) : mode === 'all' ? runAll(root) : runWorktree(root);
  if (result.report.length) {
    printReport(result, mode);
    return 1;
  }
  console.log(`ok check-encoding: no byte-order marks gained and no garbled text in ${result.where} (${result.checked} file(s)).`);
  return 0;
}

module.exports = { findMojibake, bomKind, checkFile, CHAR_TO_BYTE };

if (require.main === module) {
  try {
    process.exitCode = main(process.argv);
  } catch (e) {
    console.error(`check-encoding: could not run: ${e.message}`);
    process.exitCode = 2;
  }
}
