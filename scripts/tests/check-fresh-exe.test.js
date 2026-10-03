// The freshness gate (scripts/check-fresh-exe.js) and the source fingerprint
// it compares (scripts/lib/src-fingerprint.js, mirrored by build.rs). Pure
// node, no game, no cargo: runs in `just rig-tests`.
//
// BUG-133: the gate used to judge a binary by file DATES, so a build of a
// different tree (main tested in a worktree, or the reverse) passed whenever it
// happened to be newer. These tests build a small source tree in a temp folder,
// make fake "exes" that carry (or lack) a source stamp, and run the real gate
// CLI against them with --tree, so every verdict here is the one a rig gets.
//
// THE SPEC is written out below a second time, on purpose, independently of
// the lib: the two must agree on a tree with every awkward case in it (CRLF,
// a lone CR, CR CR LF, binary bytes, a non-ASCII name, paths that sort
// differently by segment than by byte), and the spec must agree with the
// number build.rs itself produced for that tree (PINNED_FIXTURE_FP). When a
// build script exists under target/, the last test runs it on the fixture too.

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const crypto = require("node:crypto");
const { spawnSync } = require("node:child_process");

const REPO = path.resolve(__dirname, "..", "..");
const GATE = path.join(REPO, "scripts", "check-fresh-exe.js");
// Loaded inside each test, so a missing or broken lib fails test by test.
const lib = () => require("../lib/src-fingerprint.js");

const INPUTS = ["src", "assets/shaders", "Cargo.toml", "Cargo.lock", "build.rs"];
const BUILD_RS =
  "// fixture build.rs\n" +
  "const FINGERPRINT_INPUTS: &[&str] = &[\"src\", \"assets/shaders\", \"Cargo.toml\", \"Cargo.lock\", \"build.rs\"];\n" +
  "fn main() {}\n";

// Every awkward case the two implementations could disagree on.
const FIXTURE = {
  "build.rs": BUILD_RS,
  "Cargo.toml": '[package]\r\nname = "fixture"\r\nversion = "0.0.0"\r\n',
  "Cargo.lock": "# a lock file\n",
  "src/main.rs": 'fn main() {\r\n    println!("hi");\r\n}\r\n',
  "src/a.rs": "// a\n",
  "src/a-b.rs": "// a-b\n", // '-' 0x2d < '.' 0x2e < '/' 0x2f: byte order, not segment order
  "src/a/b.rs": "// a/b\n",
  "src/lone_cr.rs": "x\ry\r\n", // a lone CR is NOT a line ending; only CRLF folds
  "src/cr_crlf.rs": "p\r\r\nq", // CR CR LF -> CR LF, once, not repeatedly
  "src/ünï.rs": "// a non-ASCII file name\n",
  "assets/shaders/x.wgsl": "fn f() {}\r\n",
  "assets/shaders/bin.dat": Buffer.from([0, 13, 10, 255, 13]),
  "assets/icon.ico": "outside the inputs: must not count",
  "data/items.csv": "outside the inputs: must not count\n",
};

// Produced by build.rs itself (its write_source_stamp) on FIXTURE, by running
// the compiled build script with CARGO_MANIFEST_DIR set to the fixture tree
// (2026-10-03, the BUG-133 build). If the spec or the lib drifts, this stops
// matching; if build.rs drifts, the last test in this file stops matching (and
// so does every rig's gate, on the first build after the change).
const PINNED_FIXTURE_FP = "9c47459e91db94c35788666f878612ec8ff1f4de6786e5eaa436ede851f89ddc";

// ── The spec, independent of the lib ─────────────────────────────────────────
const sha256 = (b) => crypto.createHash("sha256").update(b).digest("hex");
function specLf(buf) {
  const out = [];
  for (let i = 0; i < buf.length; i++) {
    if (buf[i] === 13 && buf[i + 1] === 10) continue;
    out.push(buf[i]);
  }
  return Buffer.from(out);
}
function specManifest(map) {
  const paths = Object.keys(map)
    .filter((p) => INPUTS.some((i) => p === i || p.startsWith(i + "/")))
    .sort((a, b) => Buffer.compare(Buffer.from(a, "utf8"), Buffer.from(b, "utf8")));
  let text = "hos-src-manifest v1\n";
  for (const p of paths) text += `${sha256(specLf(Buffer.from(map[p])))} ${p}\n`;
  return { text, fingerprint: sha256(Buffer.from(text, "utf8")), files: paths.length };
}
function specStamp(map, { profile = "release", features = "native,relay" } = {}) {
  const m = specManifest(map);
  return (
    `HOS-SRC-STAMP v1\nfingerprint ${m.fingerprint}\nfiles ${m.files}\nprofile ${profile}\n` +
    `features ${features}\n${m.text}HOS-SRC-STAMP END\n`
  );
}

// ── Fixture helpers ──────────────────────────────────────────────────────────
const tmpRoot = fs.mkdtempSync(path.join(os.tmpdir(), "hos-fresh-test-"));
process.on("exit", () => fs.rmSync(tmpRoot, { recursive: true, force: true }));
let n = 0;
function writeTree(map) {
  const dir = path.join(tmpRoot, `tree${++n}`);
  for (const [rel, content] of Object.entries(map)) {
    const full = path.join(dir, ...rel.split("/"));
    fs.mkdirSync(path.dirname(full), { recursive: true });
    fs.writeFileSync(full, content);
  }
  return dir;
}
/** An exe-shaped file: binary noise around the stamp text(s), like .rdata. */
function fakeExe(stamps) {
  const parts = [crypto.randomBytes(4096)];
  for (const s of stamps) parts.push(Buffer.from(s, "utf8"), crypto.randomBytes(512));
  const file = path.join(tmpRoot, `exe${++n}.exe`);
  fs.writeFileSync(file, Buffer.concat(parts));
  return file;
}
function gate(args) {
  const r = spawnSync(process.execPath, [GATE, ...args], { encoding: "utf8" });
  return { status: r.status, out: `${r.stdout}\n${r.stderr}` };
}
const jsonOut = () => path.join(tmpRoot, `result${++n}.json`);
const withChange = (rel, content) => ({ ...FIXTURE, [rel]: content });

// ── The fingerprint (lib vs the spec) ────────────────────────────────────────
test("the spec on the fixture is the fingerprint build.rs produced for it", () => {
  assert.strictEqual(specManifest(FIXTURE).fingerprint, PINNED_FIXTURE_FP);
});

test("the lib fingerprints a tree exactly as the spec does", () => {
  const dir = writeTree(FIXTURE);
  const t = lib().fingerprintTree(dir);
  const s = specManifest(FIXTURE);
  assert.strictEqual(t.text, s.text);
  assert.strictEqual(t.fingerprint, s.fingerprint);
  assert.strictEqual(t.files.size, s.files);
});

test("the input list is read out of build.rs, the one definition", () => {
  const dir = writeTree(FIXTURE);
  assert.deepStrictEqual(lib().readInputs(dir), INPUTS);
  // The real build.rs must parse too, and must cover the gate's historic set.
  const real = lib().readInputs(REPO);
  for (const must of ["src", "assets/shaders", "Cargo.toml", "build.rs"]) {
    assert.ok(real.includes(must), `build.rs FINGERPRINT_INPUTS lost ${must}: ${real.join(", ")}`);
  }
});

test("a build.rs with no FINGERPRINT_INPUTS list is an error, not an empty list", () => {
  const dir = writeTree(withChange("build.rs", "fn main() {}\n"));
  assert.throws(() => lib().readInputs(dir), /FINGERPRINT_INPUTS/);
});

test("CRLF-only differences do not change the fingerprint; a lone CR and one byte do", () => {
  const L = lib();
  const base = L.fingerprintTree(writeTree(FIXTURE)).fingerprint;
  const lfOnly = withChange("src/main.rs", 'fn main() {\n    println!("hi");\n}\n');
  assert.strictEqual(L.fingerprintTree(writeTree(lfOnly)).fingerprint, base, "CRLF -> LF changed the fingerprint");
  const crlfAll = withChange("src/a.rs", "// a\r\n");
  assert.strictEqual(L.fingerprintTree(writeTree(crlfAll)).fingerprint, base, "LF -> CRLF changed the fingerprint");
  assert.notStrictEqual(L.fingerprintTree(writeTree(withChange("src/a.rs", "// b\n"))).fingerprint, base);
  assert.notStrictEqual(L.fingerprintTree(writeTree(withChange("src/lone_cr.rs", "xy\r\n"))).fingerprint, base);
});

test("files outside the inputs do not count; a new file inside them does", () => {
  const L = lib();
  const base = L.fingerprintTree(writeTree(FIXTURE)).fingerprint;
  assert.strictEqual(L.fingerprintTree(writeTree(withChange("data/items.csv", "edited\n"))).fingerprint, base);
  assert.strictEqual(L.fingerprintTree(writeTree(withChange("assets/icon.ico", "edited"))).fingerprint, base);
  assert.notStrictEqual(L.fingerprintTree(writeTree(withChange("src/new.rs", ""))).fingerprint, base);
});

test("the stamp is found in an exe's bytes, and its absence is reported as absence", () => {
  const L = lib();
  const found = L.readExeStamp(fakeExe([specStamp(FIXTURE)]));
  assert.strictEqual(found.stamps.length, 1);
  assert.strictEqual(found.stamps[0].fingerprint, specManifest(FIXTURE).fingerprint);
  assert.strictEqual(found.stamps[0].profile, "release");
  assert.strictEqual(found.stamps[0].features, "native,relay");
  const none = L.readExeStamp(fakeExe([]));
  assert.strictEqual(none.stamps.length, 0);
  assert.strictEqual(none.malformed.length, 0);
});

// ── The gate's decision, through the real CLI ────────────────────────────────
test("GATE: an exe built from the same tree passes", () => {
  const tree = writeTree(FIXTURE);
  const r = gate(["--tree", tree, "--exe", fakeExe([specStamp(FIXTURE)])]);
  assert.strictEqual(r.status, 0, r.out);
  assert.match(r.out, /PASS/);
});

test("GATE: one changed source byte refuses, naming both fingerprints, the file and the rebuild", () => {
  const tree = writeTree(withChange("src/a.rs", "// A\n"));
  const r = gate(["--tree", tree, "--exe", fakeExe([specStamp(FIXTURE)])]);
  assert.strictEqual(r.status, 1, r.out);
  assert.ok(r.out.includes(specManifest(FIXTURE).fingerprint), "exe fingerprint not named");
  assert.ok(r.out.includes(specManifest(withChange("src/a.rs", "// A\n")).fingerprint), "tree fingerprint not named");
  assert.match(r.out, /src\/a\.rs/);
  assert.match(r.out, /cargo build --features native --release/);
});

test("GATE: a build of a different tree refuses even when it is NEWER than every source", () => {
  // BUG-133 itself. The exe is written after the tree, so every date says
  // "fresh"; the old date-only gate passed exactly this.
  const tree = writeTree(withChange("src/new_feature.rs", "pub fn f() {}\n"));
  const exe = fakeExe([specStamp(FIXTURE)]);
  const future = new Date(Date.now() + 3600 * 1000);
  fs.utimesSync(exe, future, future);
  const r = gate(["--tree", tree, "--exe", exe]);
  assert.strictEqual(r.status, 1, r.out);
  assert.match(r.out, /src\/new_feature\.rs/);
});

test("GATE: a CRLF-only difference between the build's tree and this one passes", () => {
  const tree = writeTree(withChange("src/main.rs", 'fn main() {\n    println!("hi");\n}\n'));
  const r = gate(["--tree", tree, "--exe", fakeExe([specStamp(FIXTURE)])]);
  assert.strictEqual(r.status, 0, r.out);
});

test("GATE: the exe's DATE no longer matters when the content matches", () => {
  // The old gate refused a build older than any source file, even one only
  // touched. The content is what the binary was built from; a date is not.
  const tree = writeTree(FIXTURE);
  const exe = fakeExe([specStamp(FIXTURE)]);
  const past = new Date(Date.now() - 30 * 24 * 3600 * 1000);
  fs.utimesSync(exe, past, past);
  const r = gate(["--tree", tree, "--exe", exe]);
  assert.strictEqual(r.status, 0, r.out);
});

test("GATE: an exe with no stamp (built before the stamp existed) refuses, and says so", () => {
  const tree = writeTree(FIXTURE);
  const r = gate(["--tree", tree, "--exe", fakeExe([])]);
  assert.strictEqual(r.status, 1, r.out);
  assert.match(r.out, /NO SOURCE STAMP/);
  assert.match(r.out, /--allow-other-build/);
});

test("GATE: a missing exe refuses", () => {
  const r = gate(["--tree", writeTree(FIXTURE), "--exe", path.join(tmpRoot, "nope.exe")]);
  assert.strictEqual(r.status, 1, r.out);
  assert.match(r.out, /NO BINARY/);
});

test("GATE: two different stamps in one exe refuse", () => {
  const tree = writeTree(FIXTURE);
  const exe = fakeExe([specStamp(FIXTURE), specStamp(withChange("src/a.rs", "// other\n"))]);
  const r = gate(["--tree", tree, "--exe", exe]);
  assert.strictEqual(r.status, 1, r.out);
  assert.match(r.out, /TWO STAMPS/);
});

test("GATE: a stamp whose manifest does not hash to its fingerprint refuses", () => {
  const tree = writeTree(FIXTURE);
  const good = specStamp(FIXTURE);
  // Flip one digest character inside the manifest, leaving the header alone.
  const i = good.indexOf(" src/a.rs\n") - 1;
  const bad = good.slice(0, i) + (good[i] === "0" ? "1" : "0") + good.slice(i + 1);
  const r = gate(["--tree", tree, "--exe", fakeExe([bad])]);
  assert.strictEqual(r.status, 1, r.out);
  assert.match(r.out, /BROKEN STAMP/);
});

test("GATE: --allow-other-build passes a different build LOUDLY and reports it", () => {
  const tree = writeTree(withChange("src/a.rs", "// A\n"));
  const out = jsonOut();
  const reason = "red check: the 1b rig against a 1a build";
  const r = gate(["--tree", tree, "--exe", fakeExe([specStamp(FIXTURE)]), "--allow-other-build", reason, "--json-out", out, "--quiet"]);
  assert.strictEqual(r.status, 0, r.out);
  assert.match(r.out, /OTHER BUILD/);
  assert.match(r.out, /NOT this tree/);
  assert.ok(r.out.includes(reason), "the reason is not printed");
  const j = JSON.parse(fs.readFileSync(out, "utf8"));
  assert.deepStrictEqual(j.other_build, {
    reason,
    exe_fingerprint: specManifest(FIXTURE).fingerprint,
    tree_fingerprint: specManifest(withChange("src/a.rs", "// A\n")).fingerprint,
  });
  assert.strictEqual(j.verdict, "other-build");
});

test("GATE: --allow-other-build runs an UNSTAMPED old build, recorded with no exe fingerprint", () => {
  const tree = writeTree(FIXTURE);
  const out = jsonOut();
  const r = gate(["--tree", tree, "--exe", fakeExe([]), "--allow-other-build", "red run vs v0.1444.0", "--json-out", out]);
  assert.strictEqual(r.status, 0, r.out);
  assert.match(r.out, /OTHER BUILD/);
  assert.match(r.out, /unstamped/);
  const j = JSON.parse(fs.readFileSync(out, "utf8"));
  assert.strictEqual(j.other_build.exe_fingerprint, null);
  assert.strictEqual(j.other_build.tree_fingerprint, specManifest(FIXTURE).fingerprint);
});

test("GATE: --allow-other-build with no reason refuses", () => {
  const tree = writeTree(withChange("src/a.rs", "// A\n"));
  const r = gate(["--tree", tree, "--exe", fakeExe([specStamp(FIXTURE)]), "--allow-other-build"]);
  assert.strictEqual(r.status, 1, r.out);
  assert.match(r.out, /reason/i);
});

test("GATE: --allow-other-build on the tree's OWN build records nothing", () => {
  const tree = writeTree(FIXTURE);
  const out = jsonOut();
  const r = gate(["--tree", tree, "--exe", fakeExe([specStamp(FIXTURE)]), "--allow-other-build", "x", "--json-out", out]);
  assert.strictEqual(r.status, 0, r.out);
  const j = JSON.parse(fs.readFileSync(out, "utf8"));
  assert.strictEqual(j.verdict, "current");
  assert.strictEqual(j.other_build, null);
});

test("GATE: a refusal is written to --json-out too", () => {
  const out = jsonOut();
  const r = gate(["--tree", writeTree(FIXTURE), "--exe", fakeExe([]), "--json-out", out]);
  assert.strictEqual(r.status, 1, r.out);
  const j = JSON.parse(fs.readFileSync(out, "utf8"));
  assert.strictEqual(j.ok, false);
  assert.strictEqual(j.verdict, "refused");
});

// ── The rigs pass the flag through ───────────────────────────────────────────
test("every script that runs the gate goes through runFreshGate (so --allow-other-build reaches it)", () => {
  const callers = [];
  const walk = (dir) => {
    for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, e.name);
      if (e.isDirectory()) walk(full);
      else if (e.name.endsWith(".js")) callers.push(full);
    }
  };
  walk(path.join(REPO, "scripts"));
  const exempt = new Set([GATE, path.join(REPO, "scripts", "lib", "src-fingerprint.js"), __filename]);
  const offenders = [];
  let users = 0;
  for (const f of callers) {
    if (exempt.has(f)) continue;
    const src = fs.readFileSync(f, "utf8");
    if (!/check-fresh-exe\.js["'`]/.test(src) && !src.includes("runFreshGate")) continue;
    if (src.includes("runFreshGate(")) users++;
    // Spawning the gate directly bypasses the flag pass-through and the record.
    if (/check-fresh-exe\.js["'`]/.test(src)) offenders.push(path.relative(REPO, f));
  }
  assert.deepStrictEqual(offenders, [], "spawn the gate through require('./lib/src-fingerprint.js').runFreshGate instead");
  assert.ok(users >= 5, `expected the 4 rigs and the relay test to use runFreshGate, found ${users}`);
});

test("runFreshGate passes --allow-other-build through and returns the record", () => {
  const L = lib();
  // A real tree is needed for --tree; runFreshGate forwards extra gate args.
  const tree = writeTree(withChange("src/a.rs", "// A\n"));
  const g = L.runFreshGate(fakeExe([specStamp(FIXTURE)]), ["--x", "--allow-other-build", "why not"], {
    stdio: "pipe",
    gateArgs: ["--tree", tree],
  });
  assert.strictEqual(g.status, 0, g.stdout + g.stderr);
  assert.strictEqual(g.other_build.reason, "why not");
  const plain = L.runFreshGate(fakeExe([specStamp(FIXTURE)]), ["--x"], { stdio: "pipe", gateArgs: ["--tree", tree] });
  assert.strictEqual(plain.status, 1);
  assert.strictEqual(plain.other_build, null);
});

// ── Rust and node agree, when a build script is on disk ──────────────────────
test("the compiled build.rs stamps the fixture exactly as the spec says (skips with no build)", (t) => {
  const builds = [];
  for (const profile of ["release", "debug"]) {
    const dir = path.join(REPO, "target", profile, "build");
    if (!fs.existsSync(dir)) continue;
    for (const d of fs.readdirSync(dir)) {
      if (!d.startsWith("humanity-engine-")) continue;
      const exe = path.join(dir, d, process.platform === "win32" ? "build-script-build.exe" : "build-script-build");
      if (fs.existsSync(exe)) builds.push({ exe, mtime: fs.statSync(exe).mtimeMs });
    }
  }
  if (!builds.length) return t.skip("no compiled build script under target/ (build once to run this check)");
  builds.sort((a, b) => b.mtime - a.mtime);
  const tree = writeTree(FIXTURE);
  const outDir = fs.mkdtempSync(path.join(tmpRoot, "out-"));
  // No inherited CARGO_FEATURE_* (a test run from inside cargo would carry
  // its own), so the features line is exactly the two set below.
  const env = Object.fromEntries(Object.entries(process.env).filter(([k]) => !k.startsWith("CARGO_FEATURE_")));
  spawnSync(builds[0].exe, [], {
    cwd: tree,
    encoding: "utf8",
    env: {
      ...env,
      CARGO_MANIFEST_DIR: tree,
      OUT_DIR: outDir,
      PROFILE: "release",
      CARGO_FEATURE_NATIVE: "1",
      CARGO_FEATURE_RELAY: "1",
      // winres runs after the stamp is written and wants these; a non-msvc
      // target env makes it return an error instead of looking for rc.exe.
      CARGO_PKG_NAME: "fixture",
      CARGO_PKG_VERSION: "0.0.0",
      CARGO_PKG_VERSION_MAJOR: "0",
      CARGO_PKG_VERSION_MINOR: "0",
      CARGO_PKG_VERSION_PATCH: "0",
      CARGO_CFG_TARGET_ENV: "none",
    },
  });
  const stampFile = path.join(outDir, "hos_src_stamp.txt");
  if (!fs.existsSync(stampFile)) return t.skip(`the newest build script (${builds[0].exe}) predates the source stamp`);
  assert.strictEqual(fs.readFileSync(stampFile, "utf8"), specStamp(FIXTURE));
});
