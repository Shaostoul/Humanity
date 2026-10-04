// The delivery check (scripts/check-delivery.js): does the exe pinned to the
// operator's taskbar hold this tree's code? Pure node, no game, no cargo: runs
// in `just rig-tests`.
//
// Since v0.1446.0 the answer comes from the source fingerprint every build
// carries (BUG-133), not from file dates. These tests feed judgeDelivery
// synthetic stamps (per-file hash lists) so each verdict is checked without a
// real exe. Mutation checks, run 2026-10-03 when this was written: dropping
// the assets/shaders/pbr/ ignore failed "a live PBR shader edit..."; dropping
// the Cargo.toml/Cargo.lock ignore failed "a version bump alone..."; making
// codeDiff report nothing failed the three "differs" tests.

const { test } = require("node:test");
const assert = require("node:assert");

const { judgeDelivery } = require("../check-delivery.js");

// A stamp or a tree is, for this purpose, a Map of path -> content hash.
const files = (obj) => new Map(Object.entries(obj));
const TREE = {
  "src/main.rs": "aaa",
  "src/lib.rs": "bbb",
  "assets/shaders/pbr/90-fragment-main.wgsl": "ccc",
  "assets/shaders/cloud_resolve.wgsl": "ddd",
  "Cargo.toml": "v1446",
  "Cargo.lock": "lock1446",
  "build.rs": "eee",
};
const tree = { files: files(TREE) };
const stampOf = (over = {}) => ({ files: files({ ...TREE, ...over }) });
const exe = (stamp) => ({ exists: true, stamp });
const NONE = { exists: false, stamp: null };

test("the taskbar exe built from this tree is current, whatever target/release holds", () => {
  const r = judgeDelivery({ stable: exe(stampOf()), built: NONE, tree });
  assert.deepStrictEqual(r.problems, []);
});

test("a source change that is built but not archived is NOT DELIVERED, and can be archived", () => {
  const r = judgeDelivery({ stable: exe(stampOf({ "src/lib.rs": "old" })), built: exe(stampOf()), tree });
  assert.strictEqual(r.problems.length, 1, JSON.stringify(r));
  assert.match(r.problems[0], /^NOT DELIVERED/);
  assert.match(r.problems[0], /src\/lib\.rs/);
  assert.strictEqual(r.canArchive, true);
});

test("a source change that is not built anywhere is NOT BUILT, and must not be archived", () => {
  const old = stampOf({ "src/lib.rs": "old" });
  const r = judgeDelivery({ stable: exe(old), built: exe(old), tree });
  assert.match(r.problems[0], /^NOT BUILT/);
  assert.strictEqual(r.canArchive, false);
});

test("a build of ANOTHER tree is not current, however new it is (the BUG-133 case)", () => {
  const other = stampOf({ "src/main.rs": "other-branch" });
  const r = judgeDelivery({ stable: exe(other), built: exe(other), tree });
  assert.match(r.problems[0], /^NOT BUILT/);
});

test("a version bump alone (Cargo.toml, Cargo.lock) does not call the exe stale", () => {
  const r = judgeDelivery({ stable: exe(stampOf({ "Cargo.toml": "v1445", "Cargo.lock": "lock1445" })), built: NONE, tree });
  assert.deepStrictEqual(r.problems, [], "a docs-only release bumps the version; the code is the same");
});

test("a live PBR shader edit needs no rebuild (read from disk beside the exe)", () => {
  const r = judgeDelivery({ stable: exe(stampOf({ "assets/shaders/pbr/90-fragment-main.wgsl": "old" })), built: NONE, tree });
  assert.deepStrictEqual(r.problems, []);
});

test("a compiled-in shader edit (outside pbr/) DOES need a rebuild", () => {
  const r = judgeDelivery({ stable: exe(stampOf({ "assets/shaders/cloud_resolve.wgsl": "old" })), built: NONE, tree });
  assert.match(r.problems[0], /^NOT BUILT/);
  assert.match(r.problems[0], /cloud_resolve/);
});

test("a file only in the tree, or only in the build, counts as a difference", () => {
  const t2 = { files: files({ ...TREE, "src/new.rs": "fff" }) };
  const a = judgeDelivery({ stable: exe(stampOf()), built: NONE, tree: t2 });
  assert.match(a.problems[0], /src\/new\.rs/);
  const b = judgeDelivery({ stable: exe(stampOf({ "src/gone.rs": "ggg" })), built: NONE, tree });
  assert.match(b.problems[0], /src\/gone\.rs/);
});

test("an exe with no source fingerprint cannot vouch for its code", () => {
  const r = judgeDelivery({ stable: { exists: true, stamp: null }, built: exe(stampOf()), tree });
  assert.match(r.problems[0], /^NO STAMP/);
  assert.strictEqual(r.canArchive, true, "target/release holds this tree's code, so archiving fixes it");
});

test("no taskbar exe at all is NOT DELIVERED", () => {
  const r = judgeDelivery({ stable: NONE, built: exe(stampOf()), tree });
  assert.match(r.problems[0], /^NOT DELIVERED/);
  assert.strictEqual(r.canArchive, true);
});
