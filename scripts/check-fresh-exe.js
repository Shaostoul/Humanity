#!/usr/bin/env node
// Freshness guard for the release exe. Answers ONE question before anything
// boots the app: "is this binary actually the build I am about to make claims
// about?"
//
// Why this exists as a mechanical gate rather than a rule in someone's head:
// on 2026-07-30 `just launch-bg` shipped booting the newest v*_HumanityOS.exe
// ARCHIVE instead of the build that was just compiled, so an agent verifying a
// renderer change saw a clean boot from a binary that predated the change. That
// is the same failure class as v0.782-784 and v0.1029-1038, where releases
// shipped broken because the verification never exercised the real binary.
//
// Checks, in order:
//   1. the exe exists at all
//   2. it is not OLDER than the newest v*_HumanityOS.exe archive in the repo
//      root (equal mtime + identical bytes counts as "it IS that build")
//   3. nothing under src/, assets/shaders/, Cargo.toml or build.rs is newer
//      than the exe (that means somebody edited source after this build; the
//      binary does not contain the change you are about to test)
//
// Usage:
//   node scripts/check-fresh-exe.js [--exe PATH] [--quiet]
// Exit 0 = safe to boot. Exit 1 = refuse, with the exact rebuild command.

const fs = require("fs");
const path = require("path");
const crypto = require("crypto");

const REPO = path.resolve(__dirname, "..");
const args = process.argv.slice(2);
const opt = (name, def) => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] ? args[i + 1] : def;
};
const QUIET = args.includes("--quiet");
const EXE = path.resolve(opt("--exe", path.join(REPO, "target", "release", "HumanityOS.exe")));

const ARCHIVE_PATTERN = /^v(\d+)\.(\d+)\.(\d+)_HumanityOS\.exe$/;
// Source that must be COMPILED IN to take effect. data/ is deliberately absent:
// the probe rig junctions data/ live, so a data edit needs no rebuild. Shaders
// are here because the first pipeline compile uses the include_str! embedded
// copies, so a shader edit before launch is invisible without a rebuild.
const SOURCE_ROOTS = ["src", path.join("assets", "shaders")];
const SOURCE_FILES = ["Cargo.toml", "build.rs"];

const REBUILD_HINT = [
  "Fix (pick one):",
  "  cargo build --features native --release    # rebuild target/release in place",
  "  just build-game                            # rebuild + bump version + archive",
].join("\n");

function say(msg) {
  if (!QUIET) console.log(`[fresh] ${msg}`);
}
function refuse(lines) {
  console.error("");
  for (const l of lines) console.error(l);
  console.error("");
  process.exit(1);
}
function stamp(ms) {
  const d = new Date(ms);
  const p = (n) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}
function sha256(file) {
  return crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");
}
function newestUnder(dir, acc) {
  let entries;
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch {
    return acc;
  }
  for (const e of entries) {
    const full = path.join(dir, e.name);
    if (e.isDirectory()) {
      acc = newestUnder(full, acc);
    } else {
      let m;
      try {
        m = fs.statSync(full).mtimeMs;
      } catch {
        continue;
      }
      if (m > acc.mtime) acc = { mtime: m, file: full };
    }
  }
  return acc;
}

// ── 1. exists ────────────────────────────────────────────────────────────────
if (!fs.existsSync(EXE)) {
  refuse([
    `NO BINARY: ${EXE} does not exist.`,
    "Nothing can be runtime-verified until there is a release build to boot.",
    REBUILD_HINT,
  ]);
}
const exeStat = fs.statSync(EXE);
say(`exe            ${path.relative(REPO, EXE)}  (${(exeStat.size / 1048576).toFixed(1)} MB, built ${stamp(exeStat.mtimeMs)})`);

// ── 2. not older than the newest archived build ──────────────────────────────
const archives = fs
  .readdirSync(REPO)
  .filter((f) => ARCHIVE_PATTERN.test(f))
  .map((f) => ({ name: f, mtime: fs.statSync(path.join(REPO, f)).mtimeMs, size: fs.statSync(path.join(REPO, f)).size }))
  .sort((a, b) => b.mtime - a.mtime);

if (!archives.length) {
  say("newest archive none in the repo root - skipping the stale-vs-archive check");
} else {
  const newest = archives[0];
  const newestPath = path.join(REPO, newest.name);
  say(`newest archive ${newest.name}  (built ${stamp(newest.mtime)})`);
  if (path.resolve(newestPath) === EXE) {
    say("ok: the exe under test IS the newest archived build");
  } else if (exeStat.mtimeMs > newest.mtime) {
    say("ok: the exe is newer than the newest archive (a fresh local build)");
  } else if (exeStat.size === newest.size && sha256(EXE) === sha256(newestPath)) {
    // `just build-game` copies target/release -> the archive, and Windows
    // CopyFileEx preserves the source mtime, so the current build normally
    // lands here: same timestamp, same bytes, same build.
    say("ok: byte-identical to the newest archive (same build, just archived)");
  } else {
    refuse([
      `STALE BINARY: ${path.relative(REPO, EXE)} is older than the newest archived build and is not the same file.`,
      `  exe under test   ${stamp(exeStat.mtimeMs)}   ${(exeStat.size / 1048576).toFixed(1)} MB`,
      `  newest archive   ${stamp(newest.mtime)}   ${newest.name}`,
      "",
      "Booting this would verify a build that predates the newest one in the tree, which is",
      "exactly how ten releases shipped panicking on world entry while the checks stayed green",
      "(CLAUDE.md, v0.1029-1038). Refusing instead of reporting a meaningless pass.",
      REBUILD_HINT,
    ]);
  }
}

// ── 3. no compiled-in source newer than the exe ──────────────────────────────
let newestSrc = { mtime: 0, file: null };
for (const root of SOURCE_ROOTS) newestSrc = newestUnder(path.join(REPO, root), newestSrc);
for (const f of SOURCE_FILES) {
  const full = path.join(REPO, f);
  if (!fs.existsSync(full)) continue;
  const m = fs.statSync(full).mtimeMs;
  if (m > newestSrc.mtime) newestSrc = { mtime: m, file: full };
}
if (newestSrc.file && newestSrc.mtime > exeStat.mtimeMs) {
  refuse([
    `SOURCE NEWER THAN BINARY: ${path.relative(REPO, newestSrc.file)} was edited after this build.`,
    `  source edited    ${stamp(newestSrc.mtime)}   ${path.relative(REPO, newestSrc.file)}`,
    `  exe built        ${stamp(exeStat.mtimeMs)}   ${path.relative(REPO, EXE)}`,
    "",
    "The binary does not contain that change, so any result from booting it is about",
    "different code than the tree you are looking at. (Several Claude sessions share this",
    "checkout - if that file is not yours, another session is mid-edit; rebuild anyway or",
    "wait for them, but do not report a runtime pass from this binary.)",
    "",
    "A file can also be TOUCHED without its content changing (a git checkout, an editor",
    "re-save). Cargo treats that as dirty too and will relink, so the rebuild below is",
    "still the honest answer, and it is cheap when nothing really changed.",
    REBUILD_HINT,
  ]);
}
if (newestSrc.file) {
  say(`ok: no compiled-in source newer than the exe (newest ${path.relative(REPO, newestSrc.file)} ${stamp(newestSrc.mtime)})`);
}
say("PASS: the binary under test is the current build");
process.exit(0);
