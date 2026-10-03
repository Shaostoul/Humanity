// The shared shader-compiler lookup (scripts/lib/dxc-dlls.js), fed made-up
// folders. Pure node, no game: runs in `just rig-tests`.
//
// Run:  just rig-tests
// Or:   node --test scripts/tests/dxc-dlls.test.js
//
// The defect this guards (found 2026-10-03): four rigs (verify-live-screen.js,
// verify-screens.js, probe-sweep.js, photograph-home.js) looked for
// dxcompiler.dll and dxil.dll ONLY beside the exe. Their default exe is
// target/release/HumanityOS.exe and target/release holds no dlls (the pair
// lives in the repo root), so whenever the rig folder held no pair left from
// an earlier copy (fresh checkout, cleaned .probe-rig, a new rig) the sandbox
// booted on the FXC fallback compiler, sat in shader compilation for minutes,
// and printed nothing about why. boot-timing.js had the same blind spot and
// refused to run; make-clips.js and verify-copresence.js could assemble a
// split pair. Every case below is one way that lookup can be wrong, and each
// was shown to FAIL with its part of the fix removed: the break and the
// failure text are written beside each test.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const os = require("os");
const path = require("path");
const DXC = require("../lib/dxc-dlls.js");

// A made-up layout: the repo root, target/release inside it (where the rigs'
// default exe lives), and a rig sandbox folder.
const REPO = path.resolve(os.tmpdir(), "dxc-test-repo");
const RELEASE = path.join(REPO, "target", "release");
const EXE = path.join(RELEASE, "HumanityOS.exe");
const RIG = path.join(REPO, ".probe-rig", "screens");

/** An `exists` that answers from a list of files, not the disk. */
function fakeDisk(files) {
  const set = new Set(files.map((f) => path.resolve(f).toLowerCase()));
  return (p) => set.has(path.resolve(p).toLowerCase());
}
const pair = (dir) => DXC.DXC_DLLS.map((d) => path.join(dir, d));

// RED CHECK: removed the repo-root entry from searchDirs (the line
// `if (repo && !sameDir(...)) dirs.push(...)` deleted), which is the old
// beside-the-exe-only lookup. Failed with:
//   AssertionError [ERR_ASSERTION]: Expected values to be strictly equal:
//   false !== true
test("the pair is found in the repo root when target/release has none (the 2026-10-03 case)", () => {
  const r = DXC.findDxcDlls({ exe: EXE, repo: REPO, exists: fakeDisk([EXE, ...pair(REPO)]) });
  assert.strictEqual(r.found, true);
  assert.strictEqual(r.where, "in the repo root");
  assert.strictEqual(r.paths["dxcompiler.dll"], path.join(REPO, "dxcompiler.dll"));
  assert.strictEqual(r.paths["dxil.dll"], path.join(REPO, "dxil.dll"));
  // And the line says so, naming the folder it passed over.
  const line = DXC.describeDxc(r);
  assert.match(line, /DXC/);
  assert.match(line, /in the repo root/);
  assert.match(line, /none beside the exe/);
});

// RED CHECK: swapped the search order in searchDirs (repo root pushed first,
// the exe's folder second). Failed with:
//   AssertionError [ERR_ASSERTION]: Expected values to be strictly equal:
//   + actual - expected
//   + 'in the repo root'
//   - 'beside the exe'
test("when both folders hold a pair, the one beside the exe wins", () => {
  const r = DXC.findDxcDlls({ exe: EXE, repo: REPO, exists: fakeDisk([...pair(RELEASE), ...pair(REPO)]) });
  assert.strictEqual(r.where, "beside the exe");
  assert.strictEqual(r.dir, RELEASE);
  assert.doesNotMatch(DXC.describeDxc(r), /none/);
});

// RED CHECK: in findDxcDlls, `d.has.length === DXC_DLLS.length` changed to
// `d.has.length > 0`, so a folder with half a pair counts. Failed with:
//   AssertionError [ERR_ASSERTION]: Expected values to be strictly equal:
//   true !== false
test("a split pair (one dll in each folder) is reported, never assembled", () => {
  const disk = fakeDisk([path.join(RELEASE, "dxcompiler.dll"), path.join(REPO, "dxil.dll")]);
  const copied = [];
  const lines = [];
  const r = DXC.copyDxcDlls({ exe: EXE, repo: REPO, dest: RIG, exists: disk, copy: (a, b) => copied.push([a, b]), log: (l) => lines.push(l) });
  assert.strictEqual(r.found, false);
  assert.deepStrictEqual(copied, [], "half a pair must not be copied: the game needs both or falls back to FXC anyway");
  assert.strictEqual(lines.length, 1);
  assert.match(lines[0], /only dxcompiler\.dll/);
  assert.match(lines[0], /only dxil\.dll/);
});

// RED CHECK: deleted the `log(describeDxc(result, { staleInRig }))` call in
// copyDxcDlls's not-found branch, so the rig stays silent, as the two old
// lookups did. Failed with:
//   AssertionError [ERR_ASSERTION]: Expected values to be strictly equal:
//   0 !== 1
test("neither folder: nothing is copied and the rig's output says neither, and FXC", () => {
  const copied = [];
  const lines = [];
  const r = DXC.copyDxcDlls({ exe: EXE, repo: REPO, dest: RIG, exists: fakeDisk([EXE]), copy: (a, b) => copied.push([a, b]), log: (l) => lines.push(l) });
  assert.strictEqual(r.found, false);
  assert.deepStrictEqual(copied, []);
  assert.strictEqual(lines.length, 1);
  assert.match(lines[0], /^WARNING/);
  assert.match(lines[0], /beside the exe, .*: neither/);
  assert.match(lines[0], /in the repo root, .*: neither/);
  assert.match(lines[0], /FXC/);
});

// RED CHECK: copyDxcDlls's not-found branch made to pass
// `{ staleInRig: false }` always. Failed with:
//   AssertionError [ERR_ASSERTION]: The input did not match the regular
//   expression /earlier run/.
test("a pair left in the rig by an earlier run is named, not called FXC", () => {
  const lines = [];
  DXC.copyDxcDlls({ exe: EXE, repo: REPO, dest: RIG, exists: fakeDisk([EXE, ...pair(RIG)]), copy: () => assert.fail("nothing to copy"), log: (l) => lines.push(l) });
  assert.match(lines[0], /earlier run/);
  assert.doesNotMatch(lines[0], /with FXC/);
});

// RED CHECK: the `!sameDir(repo, dirs[0].dir)` test removed from searchDirs,
// so the repo root is always added. Failed with:
//   AssertionError [ERR_ASSERTION]: Expected values to be strictly equal:
//   2 !== 1
test("an exe in the repo root searches that one folder once", () => {
  const archived = path.join(REPO, "v0.1441.0_HumanityOS.exe");
  const r = DXC.findDxcDlls({ exe: archived, repo: REPO, exists: fakeDisk(pair(REPO)) });
  assert.strictEqual(r.searched.length, 1);
  assert.strictEqual(r.where, "beside the exe");
  // Windows paths ignore case: "c:\humanity" and "C:\Humanity" are one folder.
  if (process.platform === "win32") {
    const r2 = DXC.findDxcDlls({ exe: archived, repo: REPO.toUpperCase(), exists: fakeDisk(pair(REPO)) });
    assert.strictEqual(r2.searched.length, 1);
  }
});

// The real file system, end to end: a temp repo root holding the pair, an
// empty target/release with the exe, and a rig folder. The bytes that land
// in the rig must be the repo root's.
// RED CHECK: the same break as the first test (repo-root entry removed from
// searchDirs). Failed with:
//   AssertionError [ERR_ASSERTION]: Expected values to be strictly equal:
//   false !== true
test("on a real disk, the repo root's pair is copied into the rig", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "dxc-dlls-"));
  try {
    const release = path.join(root, "target", "release");
    const rig = path.join(root, ".probe-rig", "screens");
    fs.mkdirSync(release, { recursive: true });
    fs.mkdirSync(rig, { recursive: true });
    const exe = path.join(release, "HumanityOS.exe");
    fs.writeFileSync(exe, "not a real exe");
    for (const dll of DXC.DXC_DLLS) fs.writeFileSync(path.join(root, dll), `repo-root ${dll}`);
    const lines = [];
    const r = DXC.copyDxcDlls({ exe, repo: root, dest: rig, log: (l) => lines.push(l) });
    assert.strictEqual(r.found, true);
    for (const dll of DXC.DXC_DLLS) {
      assert.strictEqual(fs.readFileSync(path.join(rig, dll), "utf8"), `repo-root ${dll}`);
    }
    assert.match(lines[0], /in the repo root/);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

// Every script that boots the game in a rig, and the name it gives the exe it
// copies into that rig (probe-sweep.js and boot-timing.js call it EXE_SRC).
const RIGS = {
  "verify-live-screen.js": "EXE",
  "verify-screens.js": "EXE",
  "verify-copresence.js": "EXE",
  "probe-sweep.js": "EXE_SRC",
  "photograph-home.js": "EXE",
  "boot-timing.js": "EXE_SRC",
  "make-clips.js": "EXE",
};
const SCRIPTS = path.join(__dirname, "..");

// The lookup lives in ONE place: each rig calls the shared helper and none
// keeps a dll list of its own (a private copy is how four of the seven
// drifted to beside-the-exe-only).
// RED CHECK (2026-10-03, on a scratch copy of scripts/): deleted the
// `DXC.copyDxcDlls({ ... });` line from scripts/verify-screens.js. Failed with:
//   AssertionError [ERR_ASSERTION]: verify-screens.js must call DXC.copyDxcDlls
// And for each of the four rigs moved over in the review follow-up, put back
// its HEAD version (its private lookup) one at a time. Each failed, e.g.:
//   AssertionError [ERR_ASSERTION]: probe-sweep.js must require scripts/lib/dxc-dlls.js
// (photograph-home.js, boot-timing.js and make-clips.js failed with the same
// message under their own names.)
test("every rig uses the shared lookup and carries no private one", () => {
  for (const [rig, exeVar] of Object.entries(RIGS)) {
    const src = fs.readFileSync(path.join(SCRIPTS, rig), "utf8");
    assert.match(src, /require\("\.\/lib\/dxc-dlls\.js"\)/, `${rig} must require scripts/lib/dxc-dlls.js`);
    const call = new RegExp(`DXC\\.copyDxcDlls\\(\\{ exe: ${exeVar}, repo: REPO, dest: RIG, log \\}\\)`);
    assert.match(src, call, `${rig} must call DXC.copyDxcDlls`);
    assert.doesNotMatch(src, /"dxcompiler\.dll",\s*"dxil\.dll"/, `${rig} keeps its own dll list`);
  }
});

// The list above is complete: any script in scripts/ that writes a rig's
// portable.txt boots the game in a sandbox, so it needs the pair, so it must
// be in RIGS (and so must pass the test above). Without this, a new rig could
// copy the exe, skip the dlls, and boot on FXC with no test noticing.
// RED CHECK (2026-10-03, on a scratch copy of scripts/): added a new
// scripts/new-rig.js that writes portable.txt and copies the dlls with its own
// beside-the-exe loop. Failed with:
//   AssertionError [ERR_ASSERTION]: these scripts boot a rig (they write
//   portable.txt) but are not in RIGS, so nothing checks their shader
//   compiler lookup: new-rig.js
// And removed "make-clips.js" from RIGS. Failed with the same message naming
// make-clips.js.
// And added a "gone-rig.js" entry to RIGS for a script that does not exist.
// Failed with:
//   AssertionError [ERR_ASSERTION]: gone-rig.js is in RIGS but no longer
//   boots a rig (or was renamed)
// (The test above fails on that too, with ENOENT reading gone-rig.js.)
test("every script that boots a rig is in the rig list", () => {
  const booting = fs
    .readdirSync(SCRIPTS)
    .filter((f) => f.endsWith(".js"))
    .filter((f) => /portable\.txt/.test(fs.readFileSync(path.join(SCRIPTS, f), "utf8")));
  const missing = booting.filter((f) => !(f in RIGS));
  assert.deepStrictEqual(
    missing,
    [],
    `these scripts boot a rig (they write portable.txt) but are not in RIGS, so nothing checks their shader compiler lookup: ${missing.join(", ")}`
  );
  // And the list holds no stale names: each entry is a rig that still exists.
  for (const rig of Object.keys(RIGS)) {
    assert.ok(booting.includes(rig), `${rig} is in RIGS but no longer boots a rig (or was renamed)`);
  }
});
