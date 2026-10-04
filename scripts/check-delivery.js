#!/usr/bin/env node
/**
 * Is the exe the OPERATOR launches actually the work we just did?
 *
 * This exists because every other check in the repo answers a different
 * question. `just brief` compares Cargo.toml to the newest GitHub release, CI
 * compares the push to the VPS, the tag compares to itself. All three can read
 * "in sync at v0.1331" while the HumanityOS.exe pinned to the taskbar is still
 * v0.1326, because nothing on the git side ever looks at the binary.
 *
 * That is not hypothetical: five releases (v0.1327 through v0.1331) were built,
 * verified, tagged and pushed while the operator kept launching the build from
 * before all of them, and reported the bugs those releases had already fixed.
 * The archive step is a separate command, it is easy to forget, and forgetting
 * it is invisible from every angle except his.
 *
 * Three things can be wrong, and they need different fixes:
 *
 *   NOT BUILT      the code in this tree is not in target/release either.
 *                  Fix: cargo build --features native --release
 *   NOT DELIVERED  target/release holds this tree's code and the taskbar copy
 *                  does not. The build exists and he cannot reach it.
 *                  Fix: node scripts/archive-build.js
 *   NO STAMP       an exe from before stamping, or delivered by hand.
 *
 * "Holds this tree's code" is decided by CONTENT since v0.1446.0 (BUG-133):
 * every build carries a fingerprint of its compiled-in sources (build.rs, read
 * back by scripts/lib/src-fingerprint.js), and this compares the taskbar exe's
 * per-file list with the tree's. File dates used to decide it, and dates
 * cannot tell a touched file from a changed one, or this tree from another.
 * Two kinds of file are left out of the comparison on purpose, for the same
 * reasons the date check left them out (see DELIVERY_IGNORES).
 *
 * Usage:
 *   node scripts/check-delivery.js           report; exit 1 if stale
 *   node scripts/check-delivery.js --warn    report; always exit 0
 *   node scripts/check-delivery.js --fix     archive if a fresh build is waiting
 *   node scripts/check-delivery.js --quiet   one line, for `just brief`
 */
const fs = require("fs");
const path = require("path");
const { execSync } = require("child_process");
const FP = require("./lib/src-fingerprint.js");

const root = path.join(__dirname, "..");
const STABLE = path.join(root, "HumanityOS.exe");
const STAMP = STABLE + ".build.json";
const BUILT = path.join(root, "target", "release", "HumanityOS.exe");

const args = process.argv.slice(2);
const warnOnly = args.includes("--warn");
const quiet = args.includes("--quiet");
const doFix = args.includes("--fix");

function cargoVersion() {
  const c = fs.readFileSync(path.join(root, "Cargo.toml"), "utf8");
  return (c.match(/^version\s*=\s*"(.+?)"/m) || [])[1] || "?";
}

// Which differing files do NOT mean the operator is missing code.
//
// SHADERS SPLIT IN TWO, and getting this wrong shipped a stale binary once.
//
// assets/shaders/pbr/ is LIVE. Those parts are include_str! embedded, but
// shader_loader prefers a assets/shaders/ directory found beside the exe or up
// its parent chain, and the taskbar copy sits at the repo root with assets/
// right next to it. So a PBR shader edit reaches the operator on his next
// launch with no rebuild, and counting those would demand a rebuild he
// does not need.
//
// EVERY OTHER SHADER IS COMPILED IN. cloud_resolve.wgsl, cloud_composite.wgsl
// and the rest are include_str! with NO disk path at all, so an edit to one of
// them reaches nothing until the crate is rebuilt. On 2026-09-22 this check
// called a binary current after a cloud_resolve edit and an archive went out
// carrying an experimental gate that had already been reverted in source. It
// was caught before it reached him, but only by hand.
//
// If a shader ever gains or loses its disk path, this split has to move with
// it; the authority is the locate step in shader_loader.rs.
//
// Cargo.toml (and Cargo.lock, which carries the same version line) are
// deliberately ignored even though a version bump touches them. Every `just ship` bumps the patch, so including it would print
// "rebuild needed" after every docs-only commit and the warning would stop
// meaning anything within a day. The version is still checked, separately and
// more precisely, by comparing the stamp against Cargo.toml below.
const DELIVERY_IGNORES = [
  (rel) => rel.startsWith("assets/shaders/pbr/"), // read from disk at runtime
  (rel) => rel === "Cargo.toml" || rel === "Cargo.lock", // version bumps (see above)
];
const ignored = (rel) => DELIVERY_IGNORES.some((f) => f(rel));

/** The files that differ between a build's stamp and the tree, minus the
 *  ignored ones. null when the build has no usable stamp. */
function codeDiff(stamp, tree) {
  if (!stamp) return null;
  const d = FP.diffManifests(stamp.files, tree.files);
  const keep = (list) => list.filter((rel) => !ignored(rel));
  const out = { changed: keep(d.changed), onlyTree: keep(d.onlyTree), onlyExe: keep(d.onlyExe) };
  out.total = out.changed.length + out.onlyTree.length + out.onlyExe.length;
  out.first = out.changed[0] || out.onlyTree[0] || out.onlyExe[0] || null;
  return out;
}

/** The one stamp an exe carries, or null (missing file, no stamp, two stamps,
 *  or a broken one: none of those can vouch for its code). */
function stampOf(exePath) {
  if (!fs.existsSync(exePath)) return null;
  const info = FP.readExeStamp(exePath);
  return info.stamps.length === 1 && !info.malformed.length ? info.stamps[0] : null;
}

/**
 * The delivery verdict. Pure: no I/O.
 *   stable / built  { exists: bool, stamp: parsed stamp or null }
 *   tree            fingerprintTree() output
 * Returns { problems: [string], canArchive: bool }. The taskbar exe is current
 * when its code matches the tree's (ignoring DELIVERY_IGNORES), whatever
 * target/release holds; otherwise the fix depends on whether target/release
 * already has the tree's code (archive it) or not (build first).
 */
function judgeDelivery({ stable, built, tree }) {
  const problems = [];
  const builtDiff = built.exists ? codeDiff(built.stamp, tree) : null;
  const builtCurrent = !!(builtDiff && builtDiff.total === 0);
  if (!stable.exists) {
    problems.push("NOT DELIVERED  HumanityOS.exe does not exist -- nothing is pinned to the taskbar");
  } else if (!stable.stamp) {
    problems.push(
      "NO STAMP       HumanityOS.exe carries no source fingerprint (built before v0.1446.0, or not by build.rs), so what code it holds is unknown"
    );
  } else {
    const d = codeDiff(stable.stamp, tree);
    if (d.total === 0) return { problems, canArchive: false };
    if (!builtCurrent) {
      problems.push(
        `NOT BUILT      ${d.total} compiled-in file(s) differ from what the taskbar exe was built from (first: ${d.first}), and target/release does not hold them either -- rebuild`
      );
      return { problems, canArchive: false };
    }
    problems.push(
      `NOT DELIVERED  target/release holds this tree's code and the taskbar exe does not (${d.total} file(s) differ, first: ${d.first}) -- a build was never archived`
    );
    return { problems, canArchive: true };
  }
  if (!built.exists) problems.push("NOT BUILT      target/release/HumanityOS.exe does not exist");
  else if (!builtCurrent)
    problems.push("NOT BUILT      target/release does not hold this tree's code either (no stamp, or files differ) -- rebuild");
  return { problems, canArchive: builtCurrent };
}

// Required as a module (the tests), export the pure parts and stop here.
if (require.main !== module) {
  module.exports = { judgeDelivery, codeDiff, DELIVERY_IGNORES };
  return;
}

const ver = cargoVersion();
const tree = FP.fingerprintTree(root);
const stable = { exists: fs.existsSync(STABLE), stamp: stampOf(STABLE) };
const built = { exists: fs.existsSync(BUILT), stamp: stampOf(BUILT) };
const verdict = judgeDelivery({ stable, built, tree });

let stamp = null;
try {
  stamp = JSON.parse(fs.readFileSync(STAMP, "utf8"));
} catch (e) {
  /* absent or unreadable */
}

const problems = verdict.problems;
const notes = [];
if (stable.exists) {
  if (!stamp) {
    problems.push("NO STAMP       HumanityOS.exe has no build stamp; its version is unknown");
  } else if (stamp.version !== ver) {
    // A VERSION difference on its own is NOT a delivery failure, and calling it
    // one would be the same cry-wolf mistake the rebuild check avoids. Every
    // `just ship` bumps the patch, so a docs-only release leaves the taskbar exe
    // one patch behind with byte-identical code in it. He is missing nothing.
    //
    // What he IS missing, if anything, is decided entirely by judgeDelivery:
    // does the taskbar exe's source fingerprint match the tree's code. That
    // answers "does the exe contain the current code". The version
    // string only decides what the title bar reads, which is how he identifies
    // a build, so it is worth SAYING and not worth failing on.
    notes.push(
      `title bar will read v${stamp.version}, the tree is v${ver} (code is current; ` +
        `rebuild only if the number itself matters)`
    );
  }
}

if (quiet) {
  if (!problems.length) {
    // The brief has one line for this, so say the short version there: which
    // build he is on, and that its code is current even if the number trails.
    const v = stamp ? stamp.version : ver;
    console.log(
      notes.length
        ? `v${v}, code current (tree is v${ver}; title bar trails by a docs-only bump)`
        : `v${ver}, current`
    );
  } else {
    console.log(`taskbar exe: ${problems[0].replace(/\s{2,}/, " -- ")}`);
    for (const p of problems.slice(1)) console.log(`             ${p.replace(/\s{2,}/, " -- ")}`);
  }
  process.exit(0);
}

if (!problems.length) {
  const v = stamp ? stamp.version : ver;
  console.log(`Delivery OK: HumanityOS.exe is v${v} and holds exactly this tree's code (its source fingerprint matches).`);
  for (const n of notes) console.log("  note: " + n);
  process.exit(0);
}

console.log("DELIVERY STALE -- the operator is not launching this work:");
for (const p of problems) console.log("  " + p);

// --fix only archives. It deliberately does NOT build: a rebuild is minutes
// long and would be a surprise inside `just ship`, and archiving a build that
// predates the source would deliver a stale exe while reporting success, which
// is the same class of lie this script exists to catch.
if (doFix) {
  const canArchive = verdict.canArchive;
  if (!canArchive) {
    console.log("");
    console.log("  Not archiving: the build itself is stale. Run `just deliver` (build + archive).");
    process.exit(warnOnly ? 0 : 1);
  }
  console.log("");
  console.log("  Archiving the waiting build...");
  execSync("node scripts/archive-build.js", { cwd: root, stdio: "inherit" });
  process.exit(0);
}

console.log("");
console.log("  Fix: `just deliver`   (build + archive + refresh the taskbar copy)");
process.exit(warnOnly ? 0 : 1);
