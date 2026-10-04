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
// HOW IT ANSWERS (since BUG-133, 2026-10-03): by CONTENT, not by date.
// build.rs hashes every compiled-in source into the binary (the source stamp;
// the list is FINGERPRINT_INPUTS in build.rs, the definition is at the top of
// scripts/lib/src-fingerprint.js). This gate hashes the same files in the tree
// it is run from and compares:
//   1. the exe exists
//   2. it carries exactly one well-formed stamp
//   3. the stamp's fingerprint equals this tree's
// Anything else refuses, naming both fingerprints and which files differ.
//
// The DATE checks it used to make are gone, on purpose, because the content
// check answers both of their questions exactly and they only ever answered
// them approximately:
//   - "no compiled-in source is newer than the exe": a source EDITED after the
//     build now changes the tree's fingerprint, so it is caught by content; one
//     only TOUCHED (a git checkout, an editor re-save) no longer refuses a build
//     that does contain it. And the date check could not see what BUG-133 was:
//     a build of ANOTHER tree that happened to be newer passed as current.
//   - "not older than the newest v*_HumanityOS.exe archive": an older binary
//     whose stamp matches this tree IS this tree's build; an archive built from
//     the same sources has the same fingerprint, and one built from different
//     sources is a different tree, which (3) refuses whatever the dates say.
//
// Usage:
//   node scripts/check-fresh-exe.js [--exe PATH] [--tree DIR] [--quiet]
//        [--allow-other-build "<reason>"] [--json-out FILE]
//   node scripts/check-fresh-exe.js --print-manifest tree|exe [--exe PATH] [--tree DIR]
//
//   --tree DIR     compare against another checkout (default: the one this script is in),
//                  e.g. a worktree's build against the main checkout: --tree C:\Humanity
//   --allow-other-build "<reason>"
//                  run a DIFFERENT build on purpose (a red check against an old build).
//                  Passes, but prints loudly that the binary is not this tree; the rigs
//                  pass it through and record other_build in their manifest. Needs a reason.
//   --json-out F   write the verdict as JSON (what the rigs read; see runFreshGate)
//   --print-manifest tree|exe
//                  print the per-file manifest (sha256 + path), to diff two trees or a
//                  tree against a build by hand
// Exit 0 = safe to boot (this tree's build, or another build allowed on purpose).
// Exit 1 = refuse, with the exact rebuild command.

const fs = require("fs");
const path = require("path");
const FP = require("./lib/src-fingerprint.js");

const REPO = path.resolve(__dirname, "..");
const args = process.argv.slice(2);
const opt = (name, def) => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] !== undefined && !args[i + 1].startsWith("--") ? args[i + 1] : def;
};
const QUIET = args.includes("--quiet");
const EXE = path.resolve(opt("--exe", path.join(REPO, "target", "release", "HumanityOS.exe")));
const TREE = path.resolve(opt("--tree", REPO));
const JSON_OUT = opt("--json-out", null);
const PRINT = opt("--print-manifest", null);
const allowRaw = FP.allowOtherFrom(args);
const ALLOW = allowRaw === undefined ? null : allowRaw.trim();

function say(msg) {
  if (!QUIET) console.log(`[fresh] ${msg}`);
}
function rel(p) {
  const r = path.relative(REPO, p);
  return !r || r.startsWith("..") ? p : r;
}
function stamp(ms) {
  const d = new Date(ms);
  const p = (n) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}
function writeJson(obj) {
  if (!JSON_OUT) return;
  fs.writeFileSync(JSON_OUT, JSON.stringify({ exe: EXE, tree_root: TREE, ...obj }, null, 2));
}
function refuse(lines, extra = {}) {
  writeJson({ ok: false, verdict: "refused", exe_fingerprint: null, tree_fingerprint: null, other_build: null, exe_sha256: null, ...extra });
  console.error("");
  for (const l of lines) console.error(l);
  console.error("");
  process.exit(1);
}

if (args.includes("--print-manifest") && PRINT !== "tree" && PRINT !== "exe") {
  refuse(["USAGE: --print-manifest tree   (this tree's per-file manifest)  or  --print-manifest exe   (the build's)"]);
}
if (ALLOW === "") {
  refuse([
    "USAGE: --allow-other-build needs a reason, in quotes, saying why this run is about another build:",
    '  --allow-other-build "red check: the 1b rig against the v0.1444.0 build"',
    "The reason goes into the rig's manifest, so whoever reads the result later knows the binary",
    "was not this tree's code and why that was the point.",
  ]);
}

// ── The tree's fingerprint ───────────────────────────────────────────────────
let tree;
try {
  tree = FP.fingerprintTree(TREE);
} catch (e) {
  refuse([`CANNOT FINGERPRINT THE TREE: ${e.message}`, "Nothing can be judged without it."]);
}

if (PRINT === "tree") {
  process.stdout.write(tree.text);
  process.exit(0);
}

// ── 1. exists ────────────────────────────────────────────────────────────────
if (!fs.existsSync(EXE)) {
  refuse(
    [
      `NO BINARY: ${EXE} does not exist.`,
      "Nothing can be runtime-verified until there is a release build to boot.",
      "Fix (pick one):",
      "  cargo build --features native --release    # rebuild target/release from this tree",
      "  just build-game                            # rebuild + bump version + archive",
    ],
    { tree_fingerprint: tree.fingerprint }
  );
}
const exeStat = fs.statSync(EXE);
const exeInfo = FP.readExeStamp(EXE);

if (PRINT === "exe") {
  if (exeInfo.stamps.length !== 1) refuse([`--print-manifest exe: ${EXE} carries ${exeInfo.stamps.length} valid stamps, not 1.`]);
  const s = exeInfo.stamps[0];
  process.stdout.write(FP.MANIFEST_HEADER + [...s.files].map(([p, h]) => `${h} ${p}\n`).join(""));
  process.exit(0);
}

const s0 = exeInfo.stamps[0];
say(
  `exe            ${rel(EXE)}  (${(exeStat.size / 1048576).toFixed(1)} MB, written ${stamp(exeStat.mtimeMs)}` +
    (s0 ? `, ${s0.profile}, features ${s0.features})` : ")")
);
say(`tree           ${TREE}  (${tree.files.size} compiled-in files under ${tree.inputs.join(", ")})`);

// ── 2 + 3. the stamp, and whether it is this tree ────────────────────────────
const v = FP.decide({ exeLabel: rel(EXE), exeInfo, tree, allowOther: ALLOW });
const record = {
  ok: v.ok,
  verdict: v.verdict,
  exe_fingerprint: v.exe_fingerprint,
  tree_fingerprint: v.tree_fingerprint,
  other_build: v.other_build,
  // The hash of the exact bytes judged. A rig copies the exe into its own
  // folder AFTER this check; it compares the copy with this before booting it
  // (src-fingerprint.js checkBootCopy), so a build finishing in between cannot
  // put an unjudged binary in the rig.
  exe_sha256: exeInfo.sha256,
};

if (v.verdict === "refused") {
  refuse([v.headline, ...v.lines], record);
}
writeJson(record);
if (v.verdict === "other-build") {
  // Never --quiet: this is the line that must not be missed.
  console.error("");
  for (const l of v.lines) console.error(l);
  console.error("");
  process.exit(0);
}
say(`fingerprint    ${v.tree_fingerprint}  (the exe and the tree agree)`);
for (const l of v.lines) say(l);
say(v.headline);
process.exit(0);
