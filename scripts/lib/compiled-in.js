// Which files the binary carries inside it, and whether the source stamp sees them
// (BUG-133, the third gap).
//
// build.rs stamps a fingerprint of the compiled-in sources into the exe, and the
// freshness gate (scripts/check-fresh-exe.js) refuses a rig boot when the stamp and
// the tree disagree. The stamp only covers what build.rs's FINGERPRINT_INPUTS list
// names. A file pulled in with include_str!/include_bytes! from anywhere ELSE is in
// the binary but invisible to the gate: edit it after a build and the gate still
// says "this binary is the current build". This module finds every such include in
// src/ and puts each target in one class:
//
//   fingerprinted   under a FINGERPRINT_INPUTS entry, so the stamp sees it.
//   stamp           OUT_DIR/hos_src_stamp.txt, the stamp itself (build.rs writes it
//                   from the fingerprinted inputs).
//   test-only       nothing outside test code reads it (#[cfg(test)], #[test], a file
//                   declared under #[cfg(test)], or a const only tests use), so it
//                   cannot change what the game does.
//   data-disk-first a data/ file the game reads from DISK first; the embedded copy is
//                   only the fallback for an exe shipped without data/. The rigs
//                   junction data/ live, so the disk copy is what a rig runs, and a
//                   data edit needs no rebuild. HOW THAT IS DETECTED is below.
//   allowlisted     on ALLOWLIST, with the reason it may stay out of the fingerprint.
//   UNCLASSIFIED    none of the above: the check fails and says what to do.
//
// HOW "DISK FIRST" IS DETECTED. Every place the shipped game READS an embedded copy
// is found, by following the bytes from the include to their readers:
//   - an include inside a fn is read by that fn;
//   - an include that defines a module-level const (`const X: &str = include_str!`)
//     is read wherever X is named outside test code: in its own file, qualified
//     (`m::X`) anywhere, and bare in any other file that does not define an X of
//     its own (a `use m::{.., X}`, a `use m::*` and a `use m::X as Y` all reach a
//     bare name; a `pub use` re-export is refused as a problem, not followed);
//   - an embedded_data.rs const is also read through get_embedded("<its path>"),
//     both by the get_embedded table (which read_data_or_embedded consults only
//     AFTER the disk read fails: checked on that fn's body, in order) and by any
//     direct get_embedded("<path>") call.
// A read counts as disk-first when the fn it sits in
//   (a) reads the disk BEFORE it (fs::read, fs::read_to_string, File::open,
//       read_data_or_embedded, .exists()): the shape of every loader here, "disk
//       copy, else the embedded one"; and
//   (b) calls embedded_data::note_builtin_copy("<that file>", ..), the one log line
//       that says at run time "this run served the copy built into the exe". It
//       names the file, which is how the analysis knows the disk read is about THAT
//       file and not some other one (a `Path::new("portable.txt").exists()` in the
//       same fn is not a disk read of the data file), and it is what the rigs look
//       for in run.log (scripts/lib/game-launch.js builtinDataLines): a disk copy
//       that fails to parse makes the loader serve the old built-in copy, which a
//       rebuild would not have, so a run that logs the line is refused.
// A read that is not disk-first is an EMBEDDED-ONLY read: on that path the game runs
// the compiled-in bytes whatever the disk says. A data/ file is data-disk-first only
// when it has at least one disk-first read and NO embedded-only read, except reads
// a human reviewed and listed on FALLBACK_SITES (a helper only ever reached after a
// disk miss; it must still call note_builtin_copy). Anything else is compiled-in-only
// on some path and must go into FINGERPRINT_INPUTS (build.rs), onto ALLOWLIST with a
// reason, or be made to read the disk first.
//
// RUST SOURCE OUTSIDE src/. Cargo compiles more than src/: a path dependency or a
// [patch] entry in Cargo.toml (`path = "vendor/rav1d"`, BUG-093's patched decoder),
// a `build = ".."` script, and any `#[path = ".."]` module that points out of the
// tree. Each is a target too: fingerprinted when under FINGERPRINT_INPUTS, else
// UNCLASSIFIED (there is no "disk first" for code).
//
// Run: node scripts/lib/compiled-in.js          (the table)
//      node scripts/lib/compiled-in.js --json   (machine-readable)
// The test that fails on an unclassified file: scripts/tests/compiled-in.test.js.

const fs = require("fs");
const path = require("path");
const FP = require("./src-fingerprint.js");

const REPO = path.resolve(__dirname, "..", "..");

// Compiled-in files that may stay OUT of the fingerprint, each with the reason.
// Keyed by repo-relative path. An entry is a claim that editing the file after a
// build cannot make a rig judge a binary the stamp calls current but that behaves
// differently from a rebuild; if that is not true, put the file in
// FINGERPRINT_INPUTS instead.
const ALLOWLIST = {};

// Shipped fns that read an embedded copy with no disk read in their own body, but
// that a human reviewed: each is only reached after a disk miss (or only writes the
// embedded copy out to disk where none exists). Keyed "file::fn". Reviewed
// 2026-10-03 by reading every caller; a new caller of one of these is NOT re-checked
// automatically, so a fn that gains a disk-less caller belongs off this list. That
// gap is closed at run time instead: every fn here must call note_builtin_copy (the
// analysis refuses one that does not), so whichever caller reaches it, the run's log
// says the built-in copy was served and a rig refuses the run.
const FALLBACK_SITES = {
  "src/storage.rs::extract_data_if_needed":
    "first-run extraction: writes every built-in copy out to a data dir that does not exist yet, then notes it once; a rig's data dir always exists (a junction to the tree's data/), so a rig never reaches it",
  "src/assets/mod.rs::parse_embedded_csv":
    "private; called only by AssetManager::load_csv_or_embedded, after the disk file is absent, unreadable or does not parse",
  "src/assets/mod.rs::parse_embedded_toml":
    "private; called only by AssetManager::load_toml_or_embedded, after the disk file is absent, unreadable or does not parse",
  "src/assets/mod.rs::parse_embedded_ron":
    "private; called only by AssetManager::load_ron_or_embedded, after the disk file is absent, unreadable or does not parse",
  "src/assets/mod.rs::parse_embedded_json":
    "private; called only by AssetManager::load_json_or_embedded, after the disk file is absent, unreadable or does not parse",
  "src/assets/mod.rs::get_embedded_str": "public but has no caller in src/",
  "src/systems/env_layer1.rs::shipped":
    "the climate table for a reader whose DataStore has none; engine::registries puts data/environment/climate.ron (disk first) in the game's DataStore at boot, so in the game it serves only tests",
  "src/systems/farming/weeds.rs::shipped": "called by WeedData::load after the disk copy is missing or does not parse, and by tests",
  "src/systems/farming/picking.rs::shipped":
    "called by HarvestWindows::load after the disk copy is missing or does not parse, and by DataStore readers when nothing registered the data; picking::register puts load() (disk first) there at boot; the no-DataStore model (self_sufficiency) uses current(), which is load()",
};

// ── A small Rust lexer ───────────────────────────────────────────────────────
//
// Returns { noComments, codeOnly }, both the same length as `src` so offsets line
// up: `noComments` blanks every comment and keeps string literals; `codeOnly` also
// blanks the INSIDE of every string and char literal, so brackets, keywords and
// `include_str!` are only ever found in real code.
function lexRust(src) {
  const comments = []; // [start, end)
  const strings = []; // [start, end) of literal contents
  const n = src.length;
  const isIdent = (c) => c !== undefined && /[A-Za-z0-9_]/.test(c);
  let i = 0;
  while (i < n) {
    const c = src[i];
    const d = src[i + 1];
    if (c === "/" && d === "/") {
      const end = src.indexOf("\n", i);
      const stop = end < 0 ? n : end;
      comments.push([i, stop]);
      i = stop;
      continue;
    }
    if (c === "/" && d === "*") {
      let depth = 0;
      let j = i;
      do {
        if (src[j] === "/" && src[j + 1] === "*") {
          depth++;
          j += 2;
        } else if (src[j] === "*" && src[j + 1] === "/") {
          depth--;
          j += 2;
        } else j++;
      } while (j < n && depth > 0);
      comments.push([i, j]);
      i = j;
      continue;
    }
    // Raw strings: r"..", r#".."#, br#".."# (the prefix must not end an identifier).
    if ((c === "r" || (c === "b" && d === "r")) && !isIdent(src[i - 1])) {
      let j = c === "b" ? i + 2 : i + 1;
      let hashes = 0;
      while (src[j] === "#") {
        hashes++;
        j++;
      }
      if (src[j] === '"') {
        const close = '"' + "#".repeat(hashes);
        const end = src.indexOf(close, j + 1);
        const stop = end < 0 ? n : end;
        strings.push([j + 1, stop]);
        i = end < 0 ? n : end + close.length;
        continue;
      }
    }
    if (c === '"') {
      let j = i + 1;
      while (j < n && src[j] !== '"') j += src[j] === "\\" ? 2 : 1;
      strings.push([i + 1, Math.min(j, n)]);
      i = j + 1;
      continue;
    }
    if (c === "'") {
      // A char literal ('x', '\n', '\u{..}', a surrogate pair) or a lifetime/label.
      if (d === "\\") {
        const end = src.indexOf("'", i + 3);
        if (end > 0) {
          strings.push([i + 1, end]);
          i = end + 1;
          continue;
        }
      } else if (src[i + 2] === "'") {
        strings.push([i + 1, i + 2]);
        i += 3;
        continue;
      } else if (/[\uD800-\uDBFF]/.test(d || "") && src[i + 3] === "'") {
        strings.push([i + 1, i + 3]);
        i += 4;
        continue;
      }
    }
    i++;
  }
  const blankRanges = (ranges) => {
    const parts = [];
    let at = 0;
    for (const [s, e] of ranges) {
      parts.push(src.slice(at, s), src.slice(s, e).replace(/[^\r\n]/g, " "));
      at = e;
    }
    parts.push(src.slice(at));
    return parts.join("");
  };
  const both = [...comments, ...strings].sort((x, y) => x[0] - y[0]);
  return { noComments: blankRanges(comments), codeOnly: blankRanges(both) };
}

/** The offset of the bracket that closes the one at `open` (in codeOnly text), or -1. */
function matchBracket(code, open) {
  const pairs = { "(": ")", "[": "]", "{": "}" };
  const stack = [pairs[code[open]]];
  for (let i = open + 1; i < code.length; i++) {
    const ch = code[i];
    if (pairs[ch]) stack.push(pairs[ch]);
    else if (ch === ")" || ch === "]" || ch === "}") {
      if (stack.pop() !== ch) return -1;
      if (!stack.length) return i;
    }
  }
  return -1;
}

/** The string literals in `text` (a slice of noComments). */
function stringLiterals(text) {
  const out = [];
  for (const m of text.matchAll(/"((?:[^"\\]|\\.)*)"/g)) out.push(m[1].replace(/\\\\/g, "\\").replace(/\\"/g, '"'));
  return out;
}

function lineIndex(src) {
  const starts = [0];
  for (let i = 0; i < src.length; i++) if (src[i] === "\n") starts.push(i + 1);
  return (offset) => {
    let lo = 0;
    let hi = starts.length - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (starts[mid] <= offset) lo = mid;
      else hi = mid - 1;
    }
    return lo + 1;
  };
}

/** Where an attributed item ends: its `;` or the `}` closing its body, at bracket depth 0. */
function itemEnd(code, from) {
  let depth = 0;
  for (let i = from; i < code.length; i++) {
    const ch = code[i];
    if (ch === "(" || ch === "[") depth++;
    else if (ch === ")" || ch === "]") depth--;
    else if (depth === 0 && ch === ";") return { end: i, open: -1 };
    else if (depth === 0 && ch === "{") return { end: matchBracket(code, i), open: i };
  }
  return { end: code.length, open: -1 };
}

// `#[cfg(test)]`, `#[cfg(all(test, ...))]` (the two forms src/ uses) and `#[test]`.
// Any OTHER cfg mentioning test (not via `not(`) is reported, never guessed at.
const TEST_ATTR = /#\[\s*(?:test|cfg\s*\(\s*test\s*\)|cfg\s*\(\s*all\s*\(\s*test\s*,[^\]]*\)\s*\))\s*\]/g;
const KNOWN_TEST_CFG = /^#\[\s*(?:cfg\s*\(\s*test\s*\)|cfg\s*\(\s*all\s*\(\s*test\s*,[^\]]*\)\s*\))\s*\]$/;
const ANY_TEST_CFG = /#\[\s*cfg\s*\((?![^\]]*\bnot\s*\()[^\]]*\btest\b[^\]]*\]/g;

// A disk read in a fn body: the shape of every "disk copy, else embedded" loader.
const DISK_READ = /\b(?:std::)?fs::read(?:_to_string)?\s*\(|\bread_data_or_embedded\s*\(|\bFile::open\s*\(|\.exists\s*\(\s*\)/;
// The run-time line a fallback writes (src/embedded_data.rs note_builtin_copy).
const NOTE_FN = "note_builtin_copy";
const NOTE_CALL = /\bnote_builtin_copy\s*\(/g;

/** A data/ target's path as a note names it ("ground/materials.ron"); a note may
 *  also spell it with the data/ prefix. */
const dataRel = (p) => String(p).replace(/^data\//, "");

/** Parse one .rs file into what the analysis needs. */
function parseFile(root, rel) {
  const src = fs.readFileSync(path.join(root, rel), "utf8");
  const { noComments, codeOnly } = lexRust(src);
  const lineOf = lineIndex(src);
  const dir = path.posix.dirname(rel);
  const base = path.posix.basename(rel);
  // Where this file's child modules live (mod.rs, lib.rs and main.rs own their folder).
  const childDir = base === "mod.rs" || base === "lib.rs" || base === "main.rs" ? dir : `${dir}/${base.replace(/\.rs$/, "")}`;
  const childFiles = (name, item) => {
    const pathAttr = item.match(/#\[\s*path\s*=\s*"([^"]+)"\s*\]/);
    if (pathAttr) return [path.posix.normalize(`${dir}/${pathAttr[1]}`)];
    return [`${childDir}/${name}.rs`, `${childDir}/${name}/mod.rs`];
  };

  const oddCfg = [];
  for (const m of codeOnly.matchAll(ANY_TEST_CFG)) {
    const t = noComments.slice(m.index, m.index + m[0].length);
    if (!KNOWN_TEST_CFG.test(t)) oddCfg.push({ line: lineOf(m.index), text: t });
  }
  // An item's attributes run back to the previous item boundary (`;`, `{`, `}`),
  // which is how a #[path] written BEFORE #[cfg(test)] is still seen.
  const itemStart = (at) => {
    let i = at - 1;
    while (i >= 0 && codeOnly[i] !== ";" && codeOnly[i] !== "}" && codeOnly[i] !== "{") i--;
    return i + 1;
  };
  const testRanges = [];
  const testChildren = [];
  for (const m of codeOnly.matchAll(TEST_ATTR)) {
    const { end, open } = itemEnd(codeOnly, m.index + m[0].length);
    testRanges.push([m.index, end]);
    if (open < 0) {
      const item = noComments.slice(itemStart(m.index), end);
      const mm = item.match(/\bmod\s+([A-Za-z_][A-Za-z0-9_]*)\s*$/);
      if (mm) testChildren.push(childFiles(mm[1], item));
    }
  }
  // Every `mod x;` (for propagating test-only through a test file's own children),
  // and where each `#[path = ".."]` one points (it may point out of src/).
  const children = [];
  const pathMods = [];
  for (const m of codeOnly.matchAll(/\bmod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;/g)) {
    const attrs = noComments.slice(itemStart(m.index), m.index);
    children.push(childFiles(m[1], attrs));
    if (/#\[\s*path\s*=/.test(attrs)) pathMods.push({ line: lineOf(m.index), target: childFiles(m[1], attrs)[0], offset: m.index });
  }

  // Every note_builtin_copy(..) call: where, and the file it names (null when its
  // first argument is not a string literal, e.g. a helper passing its own path).
  const notes = [];
  for (const m of codeOnly.matchAll(NOTE_CALL)) {
    const open = m.index + m[0].length - 1;
    const close = matchBracket(codeOnly, open);
    const arg = noComments.slice(open + 1, close < 0 ? open + 1 : close);
    const lit = arg.match(/^\s*"((?:[^"\\]|\\.)*)"\s*,/);
    // The first argument as written (to match a get_embedded(<same expr>) call).
    let depth = 0;
    let cut = arg.length;
    for (let i = 0; i < arg.length; i++) {
      const ch = codeOnly[open + 1 + i];
      if ("([{".includes(ch)) depth++;
      else if (")]}".includes(ch)) depth--;
      else if (ch === "," && depth === 0) {
        cut = i;
        break;
      }
    }
    notes.push({ offset: m.index, rel: lit ? dataRel(lit[1]) : null, first: arg.slice(0, cut).trim() });
  }

  // `use ..;` declarations (their names are imports, not reads), and the consts and
  // statics this file defines itself (a bare name it defines is its own).
  const uses = [];
  for (const m of codeOnly.matchAll(/\b(pub(?:\s*\([^)]*\))?\s+)?use\s+[^;]*;/g)) {
    uses.push({ start: m.index, end: m.index + m[0].length, text: noComments.slice(m.index, m.index + m[0].length), pub: !!m[1] });
  }
  const defines = new Set();
  for (const m of codeOnly.matchAll(/\b(?:const|static)\s+(?:mut\s+)?([A-Z_][A-Z0-9_]*)\s*:/g)) defines.add(m[1]);
  // Inline modules (`mod x { .. }`): an item inside one is reached as `x::ITEM`.
  const inlineMods = [];
  for (const m of codeOnly.matchAll(/\bmod\s+([A-Za-z_][A-Za-z0-9_]*)\s*\{/g)) {
    const open = m.index + m[0].length - 1;
    inlineMods.push({ name: m[1], open, close: matchBracket(codeOnly, open) });
  }

  // Every fn with a body.
  const fns = [];
  for (const m of codeOnly.matchAll(/\bfn\s+([A-Za-z_][A-Za-z0-9_]*)/g)) {
    const { end, open } = itemEnd(codeOnly, m.index + m[0].length);
    if (open >= 0 && end > open) fns.push({ name: m[1], open, close: end });
  }

  const includes = [];
  for (const m of codeOnly.matchAll(/\binclude_(str|bytes)\s*!\s*\(/g)) {
    const open = m.index + m[0].length - 1;
    const close = matchBracket(codeOnly, open);
    includes.push({ kind: m[1], offset: m.index, line: lineOf(m.index), arg: noComments.slice(open + 1, close).trim() });
  }
  return { rel, src, noComments, codeOnly, lineOf, testRanges, testChildren, children, pathMods, notes, uses, defines, inlineMods, oddCfg, fns, includes };
}

/** The module a file IS, as a path segment names it: src/a/b.rs -> "b",
 *  src/a/mod.rs -> "a", lib.rs/main.rs -> "crate". */
function moduleName(rel) {
  const base = path.posix.basename(rel, ".rs");
  if (base === "lib" || base === "main") return "crate";
  if (base === "mod") return path.posix.basename(path.posix.dirname(rel));
  return base;
}

/**
 * The Rust source cargo compiles from Cargo.toml, outside src/: every `path = ".."`
 * (a [[bin]]/[lib] target, a path dependency, a [patch] entry) and `build = ".."`.
 * Comments (`#` to the end of the line, outside a string) are skipped.
 * Returns [{ target, line, how }].
 */
function cargoSources(root) {
  let text;
  try {
    text = fs.readFileSync(path.join(root, "Cargo.toml"), "utf8");
  } catch {
    return [];
  }
  const out = [];
  text.split(/\r?\n/).forEach((raw, i) => {
    // Cut a comment: the first `#` outside a basic string.
    let line = "";
    let inStr = false;
    for (let j = 0; j < raw.length; j++) {
      const c = raw[j];
      if (c === '"' && raw[j - 1] !== "\\") inStr = !inStr;
      if (c === "#" && !inStr) break;
      line += c;
    }
    for (const m of line.matchAll(/\b(path|build)\s*=\s*"([^"]+)"/g)) {
      const target = path.posix.normalize(m[2].replace(/\\/g, "/")).replace(/\/+$/, "");
      out.push({ target, line: i + 1, how: m[1] === "build" ? "a build script" : "a path in Cargo.toml (a target, a path dependency or a [patch] entry)" });
    }
  });
  return out;
}

/** The innermost fn whose body holds `offset`, or null. */
function enclosingFn(file, offset) {
  let best = null;
  for (const f of file.fns) if (f.open <= offset && offset <= f.close && (!best || f.open > best.open)) best = f;
  return best;
}

/** Resolve an include's argument to { target (repo-relative), how } or { problem }. */
function resolveTarget(rel, arg) {
  const compact = arg.replace(/\s+/g, "");
  const lits = stringLiterals(arg);
  if (/^"(?:[^"\\]|\\.)*"$/.test(compact)) {
    return { target: path.posix.normalize(`${path.posix.dirname(rel)}/${lits[0]}`), how: "a path relative to the file" };
  }
  if (/^concat!\(env!\("CARGO_MANIFEST_DIR"\),("(?:[^"\\]|\\.)*",?)+\)$/.test(compact) && lits.length >= 2) {
    return { target: path.posix.normalize(lits.slice(1).join("").replace(/^\/+/, "")), how: "CARGO_MANIFEST_DIR" };
  }
  if (/^concat!\(env!\("OUT_DIR"\),("(?:[^"\\]|\\.)*",?)+\)$/.test(compact) && lits.length >= 2) {
    return { target: `OUT_DIR/${lits.slice(1).join("").replace(/^\/+/, "")}`, how: "OUT_DIR" };
  }
  return { problem: `cannot resolve the include argument ${JSON.stringify(arg)} (teach resolveTarget the form)` };
}

/** Every .rs file under src/, repo-relative with '/' separators. */
function rustFiles(root) {
  const out = [];
  const walk = (rel) => {
    for (const e of fs.readdirSync(path.join(root, rel), { withFileTypes: true })) {
      const child = `${rel}/${e.name}`;
      if (e.isDirectory()) walk(child);
      else if (e.isFile() && e.name.endsWith(".rs")) out.push(child);
    }
  };
  walk("src");
  return out.sort();
}

const ED = "src/embedded_data.rs";

/**
 * The whole analysis. Returns {
 *   inputs, targets: [{ target, class, reason, reads, test_sites }],
 *   problems: [string]   (unresolvable includes, unknown test cfgs, stale list entries,
 *                         re-exports, unreviewed run-time-chosen embedded reads, reviewed
 *                         fallbacks that never say so at run time: each a failure)
 * }.
 */
function analyse(root = REPO, opts = {}) {
  const allow = opts.allowlist || ALLOWLIST;
  const fallbackSites = opts.fallbackSites || FALLBACK_SITES;
  const inputs = opts.inputs || FP.readInputs(root);
  const files = new Map(rustFiles(root).map((rel) => [rel, parseFile(root, rel)]));
  const problems = [];

  // ── Test-only files: declared under a test attribute, and their own children ──
  const testFiles = new Set();
  const queue = [];
  for (const f of files.values()) {
    for (const alts of f.testChildren) {
      const hit = alts.find((p) => files.has(p));
      if (hit) queue.push(hit);
    }
    for (const o of f.oddCfg) problems.push(`${f.rel}:${o.line}: a test cfg this check does not understand: ${o.text}`);
  }
  while (queue.length) {
    const rel = queue.pop();
    if (testFiles.has(rel)) continue;
    testFiles.add(rel);
    for (const alts of files.get(rel).children) {
      const hit = alts.find((p) => files.has(p));
      if (hit) queue.push(hit);
    }
  }
  const isTest = (f, offset) => testFiles.has(f.rel) || f.testRanges.some(([s, e]) => s <= offset && offset <= e);

  // Which files define each const/static name: a bare name a file defines itself is
  // its own, and a name defined in several files needs its qualifier checked.
  const definers = new Map();
  for (const f of files.values()) {
    for (const n of f.defines) {
      if (!definers.has(n)) definers.set(n, new Set());
      definers.get(n).add(f.rel);
    }
  }

  // The note_builtin_copy calls that belong to fn `fn` itself (not to a fn nested in it).
  const notesIn = (f, fn) => f.notes.filter((n) => n.offset > fn.open && n.offset < fn.close && enclosingFn(f, n.offset) === fn);

  // FALLBACK_SITES entries some read actually relied on (the rest are stale).
  const reviewUsed = new Set();

  // A read of embedded bytes at (file, offset), of `target` (null: chosen at run time).
  // Disk first = a disk read BEFORE it in the same fn, and a note naming the file.
  const readAt = (f, offset, via, target) => {
    const fn = enclosingFn(f, offset);
    let disk = null;
    let noted = false;
    const want = target && target.startsWith("data/") ? dataRel(target) : null;
    if (fn) {
      const at = f.codeOnly.slice(fn.open, offset).search(DISK_READ);
      if (at >= 0) disk = f.lineOf(fn.open + at);
      noted = !!want && notesIn(f, fn).some((n) => n.rel === want);
    }
    const key = `${f.rel}::${fn ? fn.name : "(module level)"}`;
    const why = [];
    if (!fn) why.push("module-level code reads it, so nothing reads the disk first");
    else {
      if (!disk) why.push(`fn ${fn.name} has no disk read before it`);
      if (want && !noted) why.push(`fn ${fn.name} never calls ${NOTE_FN}("${want}", ..) to say so at run time`);
    }
    return {
      file: f.rel,
      line: f.lineOf(offset),
      fn: fn ? fn.name : null,
      via,
      disk_read_line: disk,
      noted,
      disk_first: !!disk && noted,
      reviewed: fallbackSites[key] || null,
      key,
      why: why.join("; "),
    };
  };

  // ── Includes: resolve, and find who reads each one ─────────────────────────
  const targets = new Map(); // target -> { reads: [], test_sites: [] }
  const entry = (t) => {
    if (!targets.has(t)) targets.set(t, { reads: [], test_sites: [] });
    return targets.get(t);
  };
  const constDefs = []; // { name, file, target, offset }
  for (const f of files.values()) {
    for (const inc of f.includes) {
      const r = resolveTarget(f.rel, inc.arg);
      if (r.problem) {
        problems.push(`${f.rel}:${inc.line}: ${r.problem}`);
        continue;
      }
      const e = entry(r.target);
      if (isTest(f, inc.offset)) {
        e.test_sites.push(`${f.rel}:${inc.line}`);
        continue;
      }
      if (r.target.startsWith("OUT_DIR/")) continue; // the stamp: classified by name below
      const fn = enclosingFn(f, inc.offset);
      const before = f.noComments.slice(Math.max(0, inc.offset - 240), inc.offset);
      const cm = before.match(/\b(?:const|static)\s+([A-Z_][A-Z0-9_]*)\s*:[^;={}]*=\s*$/);
      if (!fn && cm) constDefs.push({ name: cm[1], file: f.rel, target: r.target, offset: inc.offset });
      else e.reads.push(readAt(f, inc.offset, `include_${inc.kind}! in ${fn ? `fn ${fn.name}` : "module-level code"}`, r.target));
    }
  }

  // ── embedded_data.rs: the get_embedded table, and is its reader disk-first? ──
  const ed = files.get(ED);
  const tableKey = new Map(); // "items.csv" -> NAME
  let diskFirstTable = false;
  if (ed) {
    const ge = ed.fns.find((f) => f.name === "get_embedded");
    if (ge) for (const m of ed.noComments.slice(ge.open, ge.close).matchAll(/"([^"]+)"\s*=>\s*Some\(\s*([A-Z0-9_]+)\s*\)/g)) tableKey.set(m[1], m[2]);
    const rd = ed.fns.find((f) => f.name === "read_data_or_embedded");
    if (rd) {
      const body = ed.codeOnly.slice(rd.open, rd.close);
      const disk = body.search(DISK_READ);
      const emb = body.search(/\bget_embedded\s*\(/);
      const ordered = disk >= 0 && emb > disk;
      const noted = notesIn(ed, rd).length > 0;
      diskFirstTable = ordered && noted;
      if (!ordered) {
        problems.push(`${ED}: read_data_or_embedded does not read the disk before calling get_embedded, so the embedded table cannot count as a disk-first fallback`);
      }
      if (!noted) {
        problems.push(
          `${ED}: read_data_or_embedded never calls ${NOTE_FN} when it serves the built-in copy, so a run that used one ` +
            "cannot be caught (the rigs refuse a run whose log has the note)"
        );
      }
    }
  }

  // ── Readers of each module-level const: every reference outside test code ──
  for (const c of constDefs) {
    const e = entry(c.target);
    const def = files.get(c.file);
    const rel = c.target.startsWith("data/") ? c.target.slice(5) : null;
    if (c.file === ED && rel && tableKey.get(rel) === c.name) {
      e.reads.push({
        file: ED,
        line: def.lineOf(c.offset),
        fn: "read_data_or_embedded",
        via: diskFirstTable
          ? `embedded_data::${c.name}, served by get_embedded("${rel}") after a disk miss (read_data_or_embedded notes it)`
          : `embedded_data::${c.name} through get_embedded("${rel}"), whose reader does not try the disk first and say so`,
        disk_read_line: diskFirstTable ? true : null,
        noted: diskFirstTable,
        disk_first: diskFirstTable,
        reviewed: null,
        key: `${ED}::get_embedded`,
        why: diskFirstTable ? "" : "read_data_or_embedded is not disk-first with a note (see PROBLEMS)",
      });
    }
    // The module names the const is reached through: its file's, and any inline
    // `mod x { .. }` around the definition (billboard_bake.rs's leaf_shape).
    const modNames = new Set([
      moduleName(c.file),
      ...def.inlineMods.filter((m) => m.open < c.offset && c.offset < m.close).map((m) => m.name),
    ]);
    const multi = (definers.get(c.name) || new Set()).size > 1;
    // A qualified `q::NAME` is ours unless another file defines a NAME too and q
    // names some other module (self/super/crate are taken as ours: fail closed).
    const qualOk = (q) => !multi || modNames.has(q) || q === "self" || q === "super" || q === "crate";
    for (const f of files.values()) {
      const names = new Set([c.name]);
      if (f.rel !== c.file) {
        for (const u of f.uses) {
          if (isTest(f, u.start)) continue;
          const named = new RegExp(`\\b${c.name}\\b(?!\\s*::)`).test(u.text);
          if (!named) continue;
          const alias = u.text.match(new RegExp(`\\b${c.name}\\s+as\\s+([A-Za-z_][A-Za-z0-9_]*)`));
          if (alias) names.add(alias[1]);
          if (u.pub) {
            problems.push(
              `${f.rel}:${f.lineOf(u.start)}: \`${u.text.replace(/\s+/g, " ")}\` is a re-export of ${c.name} (${c.target}). ` +
                "The analysis does not follow a re-export to its readers: read the const where it is defined, or teach scripts/lib/compiled-in.js"
            );
          }
        }
      }
      for (const name of names) {
        for (const m of f.codeOnly.matchAll(new RegExp(`\\b${name}\\b`, "g"))) {
          if (f.rel === c.file && m.index <= c.offset && m.index >= c.offset - 240) continue; // the definition
          if (f.uses.some((u) => u.start <= m.index && m.index < u.end)) continue; // an import, not a read
          const q = f.codeOnly.slice(Math.max(0, m.index - 80), m.index).match(/([A-Za-z_][A-Za-z0-9_]*)\s*::\s*$/);
          if (q) {
            if (!qualOk(q[1])) continue; // another module's same-named item
          } else if (f.rel !== c.file && f.defines.has(name)) {
            continue; // this file's own item of that name
          }
          if (f.rel === ED && enclosingFn(f, m.index) && enclosingFn(f, m.index).name === "get_embedded") continue; // the table, handled above
          if (isTest(f, m.index)) continue;
          e.reads.push(readAt(f, m.index, `${q ? `${q[1]}::` : ""}${name}${name !== c.name ? ` (an alias of ${c.name})` : ""}`, c.target));
        }
      }
    }
  }

  // Direct get_embedded("...") calls, and the run-time-chosen ones.
  const nameTarget = new Map(constDefs.filter((c) => c.file === ED).map((c) => [c.name, c.target]));
  for (const f of files.values()) {
    if (f.rel === ED) continue;
    for (const m of f.codeOnly.matchAll(/\bget_embedded\s*\(/g)) {
      if (isTest(f, m.index)) continue;
      const open = m.index + m[0].length - 1;
      const arg = f.noComments.slice(open + 1, matchBracket(f.codeOnly, open)).trim();
      const lit = arg.match(/^"([^"]+)"$/);
      if (lit) {
        const t = nameTarget.get(tableKey.get(lit[1]));
        if (t) entry(t).reads.push(readAt(f, m.index, `get_embedded(${arg})`, t));
        continue;
      }
      const r = readAt(f, m.index, `get_embedded(${arg})`, null);
      const fn = enclosingFn(f, m.index);
      const sameNote = !!fn && notesIn(f, fn).some((n) => n.first === arg);
      if (r.disk_read_line && sameNote) continue; // disk first, and says so with the same path
      if (r.reviewed) {
        reviewUsed.add(r.key);
        continue;
      }
      problems.push(
        `${f.rel}:${r.line} (fn ${r.fn}): get_embedded(${arg}) reads an embedded copy chosen at run time, and its fn does not ` +
          `both read the disk before it and call ${NOTE_FN}(${arg}, ..). Make it do both, or review it and add "${r.key}" to ` +
          "FALLBACK_SITES with the reason."
      );
    }
  }

  // ── Classify ──────────────────────────────────────────────────────────────
  const underInputs = (t) => inputs.find((inp) => t === inp || t.startsWith(`${inp}/`));
  const out = [];
  for (const [target, e] of [...targets].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))) {
    // One read per place (a same-named const in two files can reach one site twice).
    const seen = new Set();
    e.reads = e.reads.filter((r) => {
      const k = `${r.file}:${r.line}:${r.via}`;
      return seen.has(k) ? false : seen.add(k);
    });
    let cls;
    let reason;
    const embeddedOnly = e.reads.filter((r) => !r.disk_first && !r.reviewed);
    for (const r of e.reads) if (r.reviewed && !r.disk_first) reviewUsed.add(r.key);
    if (target.startsWith("OUT_DIR/")) {
      cls = target === "OUT_DIR/hos_src_stamp.txt" ? "stamp" : "UNCLASSIFIED";
      reason = cls === "stamp" ? "the source stamp itself, written by build.rs from the fingerprinted inputs" : "an OUT_DIR file that is not the stamp";
    } else if (underInputs(target)) {
      cls = "fingerprinted";
      reason = `under ${underInputs(target)}`;
    } else if (!e.reads.length) {
      cls = "test-only";
      reason = e.test_sites.length
        ? `read only in test code (${e.test_sites.join(", ")})`
        : "compiled in as a const that no shipped code reads";
    } else if (allow[target]) {
      cls = "allowlisted";
      reason = allow[target];
    } else if (target.startsWith("data/") && !embeddedOnly.length) {
      // Every read is disk-first, or a reviewed fallback (FALLBACK_SITES), whose
      // review is the claim that the disk was tried first by its caller.
      cls = "data-disk-first";
      reason = e.reads
        .map((r) =>
          r.reviewed && !r.disk_first
            ? `${r.file}:${r.line} fn ${r.fn} (reviewed fallback: ${r.reviewed})`
            : r.disk_read_line === true
              ? r.via
              : `${r.file}:${r.line} fn ${r.fn} reads the disk first (line ${r.disk_read_line}) and notes the fallback`
        )
        .join("; ");
    } else {
      cls = "UNCLASSIFIED";
      reason =
        (embeddedOnly.length
          ? `embedded-only read${embeddedOnly.length > 1 ? "s" : ""}: ${embeddedOnly
              .map((r) => `${r.file}:${r.line}${r.fn ? ` fn ${r.fn}` : ""} (${r.via}${r.why ? `: ${r.why}` : ""})`)
              .join("; ")}`
          : "no read of it tries the disk first") +
        `. Add it to FINGERPRINT_INPUTS in build.rs, make ${embeddedOnly.length > 1 ? "those reads" : "the read"} disk-first ` +
        `(a disk read first, and embedded_data::${NOTE_FN}("<the file>", why) where the built-in copy is served), ` +
        "or put it on ALLOWLIST in scripts/lib/compiled-in.js with the reason.";
    }
    out.push({ target, class: cls, reason, reads: e.reads, test_sites: e.test_sites });
  }

  // ── Rust source outside src/: Cargo.toml paths and #[path] modules ─────────
  const have = new Set(out.map((t) => t.target));
  const addSource = (target, test, where) => {
    if (have.has(target)) return;
    have.add(target);
    const under = underInputs(target);
    out.push({
      target,
      class: under ? "fingerprinted" : test ? "test-only" : "UNCLASSIFIED",
      reason: under
        ? `under ${under}`
        : test
          ? `Rust source compiled only into tests (${where})`
          : `compiled into the binary as Rust source (${where}) but not under FINGERPRINT_INPUTS, so editing it after a build ` +
            "leaves the stamp saying current. Add it to FINGERPRINT_INPUTS in build.rs.",
      reads: [],
      test_sites: [],
    });
  };
  for (const s of cargoSources(root)) addSource(s.target, false, `Cargo.toml line ${s.line}, ${s.how}`);
  for (const f of files.values()) {
    for (const pm of f.pathMods) addSource(pm.target, isTest(f, pm.offset), `#[path] module at ${f.rel}:${pm.line}`);
  }
  out.sort((a, b) => (a.target < b.target ? -1 : a.target > b.target ? 1 : 0));

  // Stale list entries are failures too: a list nobody prunes stops meaning anything.
  for (const k of Object.keys(allow)) {
    const t = out.find((x) => x.target === k);
    if (!t) problems.push(`ALLOWLIST names ${k}, which nothing in src/ includes any more: remove it`);
    else if (t.class !== "allowlisted") problems.push(`ALLOWLIST names ${k}, which is ${t.class} without it: remove it`);
  }
  for (const k of Object.keys(fallbackSites)) {
    if (!reviewUsed.has(k)) {
      problems.push(`FALLBACK_SITES names ${k}, which no read relies on any more (gone, or disk-first on its own): remove it`);
      continue;
    }
    // A reviewed fallback serves the built-in copy: it must say so at run time,
    // or a rig that ran on it could not tell.
    const [file, fnName] = k.split("::");
    const f = files.get(file);
    const fns = f ? f.fns.filter((x) => x.name === fnName) : [];
    if (!fns.some((fn) => notesIn(f, fn).length)) {
      problems.push(
        `FALLBACK_SITES: ${k} serves the built-in copy but never calls ${NOTE_FN}, so a run that used it cannot be caught. ` +
          `Call embedded_data::${NOTE_FN}(<the file>, why) in it.`
      );
    }
  }
  return { inputs, targets: out, problems, test_files: [...testFiles].sort() };
}

/** The table, as lines. */
function report(a) {
  const lines = [];
  const counts = {};
  for (const t of a.targets) counts[t.class] = (counts[t.class] || 0) + 1;
  lines.push(`compiled-in files: ${a.targets.length}  (${Object.entries(counts).map(([k, v]) => `${k} ${v}`).join(", ")})`);
  lines.push(`FINGERPRINT_INPUTS: ${a.inputs.join(", ")}`);
  lines.push("");
  for (const t of a.targets) {
    lines.push(`${t.class.padEnd(16)} ${t.target}`);
    if (t.class !== "fingerprinted") lines.push(`${" ".repeat(19)}${t.reason}`);
  }
  if (a.problems.length) {
    lines.push("");
    lines.push("PROBLEMS:");
    for (const p of a.problems) lines.push(`  ${p}`);
  }
  return lines;
}

module.exports = { ALLOWLIST, FALLBACK_SITES, DISK_READ, NOTE_FN, lexRust, parseFile, resolveTarget, cargoSources, moduleName, analyse, report };

if (require.main === module) {
  const a = analyse(REPO);
  if (process.argv.includes("--json")) process.stdout.write(JSON.stringify(a, null, 2) + "\n");
  else for (const l of report(a)) console.log(l);
  process.exit(a.targets.some((t) => t.class === "UNCLASSIFIED") || a.problems.length ? 1 : 0);
}
