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
 *   NOT BUILT      source is newer than target/release/HumanityOS.exe.
 *                  Fix: cargo build --features native --release
 *   NOT DELIVERED  target/release is newer than the taskbar copy, or the stamp
 *                  disagrees with Cargo.toml. The build exists and he cannot
 *                  reach it. Fix: node scripts/archive-build.js
 *   NO STAMP       an exe from before stamping, or delivered by hand.
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

const root = path.join(__dirname, "..");
const STABLE = path.join(root, "HumanityOS.exe");
const STAMP = STABLE + ".build.json";
const BUILT = path.join(root, "target", "release", "HumanityOS.exe");

const args = process.argv.slice(2);
const warnOnly = args.includes("--warn");
const quiet = args.includes("--quiet");
const doFix = args.includes("--fix");

function mtime(p) {
  try {
    return fs.statSync(p).mtimeMs;
  } catch (e) {
    return null;
  }
}

function cargoVersion() {
  const c = fs.readFileSync(path.join(root, "Cargo.toml"), "utf8");
  return (c.match(/^version\s*=\s*"(.+?)"/m) || [])[1] || "?";
}

// Newest mtime under the trees that change what the exe DOES.
//
// Shaders are NOT in this set, and the reason is worth knowing. They are
// include_str! embedded at compile time, but shader_loader prefers a
// assets/shaders/ directory found beside the exe or up its parent chain
// (shader_loader.rs, locate step). The taskbar copy lives at the repo root
// with assets/ right next to it, so it reads the working-tree shaders and a
// shader edit reaches the operator on his next launch with no rebuild at all.
// Listing them here would report a rebuild he does not need.
//
// The embedded copy still matters for the exe that ships to a user, who has
// no assets/ directory. That is a release concern, not a delivery one.
//
// Cargo.toml is deliberately NOT in this set even though a version bump
// touches it. Every `just ship` bumps the patch, so including it would print
// "rebuild needed" after every docs-only commit and the warning would stop
// meaning anything within a day. The version is still checked, separately and
// more precisely, by comparing the stamp against Cargo.toml below.
function newestSource() {
  const roots = ["src"];
  let newest = 0;
  let which = null;
  const walk = (p) => {
    let st;
    try {
      st = fs.statSync(p);
    } catch (e) {
      return;
    }
    if (st.isDirectory()) {
      for (const e of fs.readdirSync(p)) walk(path.join(p, e));
      return;
    }
    if (st.mtimeMs > newest) {
      newest = st.mtimeMs;
      which = path.relative(root, p);
    }
  };
  for (const r of roots) walk(path.join(root, r));
  return { mtime: newest, file: which };
}

const ver = cargoVersion();
const stableM = mtime(STABLE);
const builtM = mtime(BUILT);
const src = newestSource();

let stamp = null;
try {
  stamp = JSON.parse(fs.readFileSync(STAMP, "utf8"));
} catch (e) {
  /* absent or unreadable */
}

const problems = [];
if (stableM === null) {
  problems.push("NOT DELIVERED  HumanityOS.exe does not exist -- nothing is pinned to the taskbar");
} else {
  if (!stamp) {
    problems.push("NO STAMP       HumanityOS.exe has no build stamp; its version is unknown");
  } else if (stamp.version !== ver) {
    problems.push(
      `NOT DELIVERED  taskbar exe is v${stamp.version}, the tree is v${ver}`
    );
  }
  if (builtM !== null && builtM > stableM + 1000) {
    problems.push("NOT DELIVERED  target/release is newer than the taskbar exe -- a build was never archived");
  }
}
if (builtM === null) {
  problems.push("NOT BUILT      target/release/HumanityOS.exe does not exist");
} else if (src.mtime > builtM + 1000) {
  problems.push(`NOT BUILT      ${src.file} is newer than target/release -- rebuild before archiving`);
}

if (quiet) {
  if (!problems.length) {
    console.log(`taskbar exe: v${ver}, current`);
  } else {
    console.log(`taskbar exe: ${problems[0].replace(/\s{2,}/, " -- ")}`);
    for (const p of problems.slice(1)) console.log(`             ${p.replace(/\s{2,}/, " -- ")}`);
  }
  process.exit(0);
}

if (!problems.length) {
  console.log(`Delivery OK: HumanityOS.exe is v${ver} and newer than every source file.`);
  process.exit(0);
}

console.log("DELIVERY STALE -- the operator is not launching this work:");
for (const p of problems) console.log("  " + p);

// --fix only archives. It deliberately does NOT build: a rebuild is minutes
// long and would be a surprise inside `just ship`, and archiving a build that
// predates the source would deliver a stale exe while reporting success, which
// is the same class of lie this script exists to catch.
if (doFix) {
  const canArchive = !problems.some((p) => p.startsWith("NOT BUILT"));
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
