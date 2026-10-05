// Every file the binary carries inside it is either seen by the source stamp or
// is out of it for a stated reason (BUG-133, the third gap). Pure node, no game,
// no cargo: runs in `just rig-tests`.
//
// The freshness gate (scripts/check-fresh-exe.js) can only refuse a stale binary
// for files the stamp covers: build.rs's FINGERPRINT_INPUTS. A file pulled in with
// include_str!/include_bytes! from anywhere else changes the binary without
// changing the stamp. scripts/lib/compiled-in.js finds every such include and
// classifies its target; the first test here fails on anything it cannot place.
//
// RED FIRST (2026-10-03, before build.rs gained the compiled-in-only files and
// before the stray embedded-only reads were made disk-first), the first test
// failed with:
//   compiled-in files: 172  (stamp 1, UNCLASSIFIED 26, fingerprinted 30,
//   data-disk-first 110, test-only 5)
//   UNCLASSIFIED     assets/icon.png
//                    embedded-only read: src/lib.rs:961 fn resumed (include_bytes! in fn resumed). ...
//   UNCLASSIFIED     data/blueprints/structure_types.ron
//                    embedded-only read: src/ship/structure.rs:89 fn structure_types ...
//   UNCLASSIFIED     data/items.csv
//                    embedded-only read: src/gui/pages/inventory.rs:396 fn lookup_item_details (::ITEMS_CSV) ...
//   ... (26 in all), and PROBLEMS:
//   src/assets/mod.rs:655 (fn parse_embedded_csv): get_embedded(path) reads an embedded copy
//   chosen at run time, with no disk read in the same fn. ...
// The fixture tests below pin each rule on a small tree, including the cases that
// must FAIL (an embedded-only read, an unresolvable include), so the real-repo
// test is known to be able to fail.

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const REPO = path.resolve(__dirname, "..", "..");
const CI = () => require("../lib/compiled-in.js");
const FP = () => require("../lib/src-fingerprint.js");

const tmpRoot = fs.mkdtempSync(path.join(os.tmpdir(), "hos-compiled-in-test-"));
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

// ── The real tree ────────────────────────────────────────────────────────────
test("every compiled-in file in src/ is fingerprinted, disk-first data, test-only, or allowlisted", () => {
  const a = CI().analyse(REPO);
  const bad = a.targets.filter((t) => t.class === "UNCLASSIFIED");
  const msg = CI().report(a).join("\n");
  assert.deepStrictEqual(a.problems, [], msg);
  assert.deepStrictEqual(bad.map((t) => t.target), [], msg);
  // The analysis must actually have looked: a regex gone wrong finds nothing
  // and passes everything. src/ has well over a hundred includes.
  assert.ok(a.targets.length > 100, `only ${a.targets.length} compiled-in files found:\n${msg}`);
  for (const cls of ["fingerprinted", "data-disk-first", "test-only", "stamp"]) {
    assert.ok(a.targets.some((t) => t.class === cls), `no ${cls} file at all:\n${msg}`);
  }
});

test("every FINGERPRINT_INPUTS entry in build.rs exists (build.rs skips a missing one silently)", () => {
  const inputs = FP().readInputs(REPO);
  const missing = inputs.filter((p) => !fs.existsSync(path.join(REPO, p)));
  assert.deepStrictEqual(missing, [], "a typo here fingerprints nothing and passes every edit to the file it meant");
});

// ── The rules, on a small tree ───────────────────────────────────────────────
const BUILD_RS = 'const FINGERPRINT_INPUTS: &[&str] = &["src", "Cargo.toml", "assets/icon.png"];\nfn main() {}\n';
const EMBEDDED = `
pub const A_CSV: &str = include_str!("../data/a.csv");
pub const B_RON: &str = include_str!("../data/b.ron");
pub fn get_embedded(path: &str) -> Option<&'static str> {
    match path {
        "a.csv" => Some(A_CSV),
        "b.ron" => Some(B_RON),
        _ => None,
    }
}
pub fn read_data_or_embedded(data_dir: &std::path::Path, rel: &str) -> Option<String> {
    match std::fs::read_to_string(data_dir.join(rel)) {
        Ok(s) => Some(s),
        Err(e) => {
            let s = get_embedded(rel)?;
            note_builtin_copy(rel, e);
            Some(s.to_string())
        }
    }
}
pub fn note_builtin_copy(rel: &str, why: impl std::fmt::Display) { let _ = (rel, why.to_string()); }
`;
const LIB = `
mod embedded_data;
fn icon() -> &'static [u8] { include_bytes!("../assets/icon.png") }
// include_str!("../data/in_a_comment.ron") is not an include
const NOT: &str = "include_str!(\\"../data/in_a_string.ron\\")";
static STAMP: &str = include_str!(concat!(env!("OUT_DIR"), "/hos_src_stamp.txt"));

/// Disk first: this fn reads the file, the include is its fallback.
pub fn load_c() -> String {
    match std::fs::read_to_string("data/c.ron") {
        Ok(s) => s,
        Err(e) => {
            crate::embedded_data::note_builtin_copy("c.ron", e);
            include_str!("../data/c.ron").to_string()
        }
    }
}
/// A module-level const read only by a disk-first loader.
const D_RON: &str = include_str!("../data/d.ron");
pub fn load_d() -> String {
    std::fs::read_to_string("data/d.ron").unwrap_or_else(|e| {
        crate::embedded_data::note_builtin_copy("data/d.ron", e);
        D_RON.to_string()
    })
}
/// Compiled-in only: nothing here ever looks at the disk.
pub fn table_e() -> &'static str { include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/e.ron")) }
/// The embedded table's own const, read directly with no disk read.
pub fn b_now() -> &'static str {
    crate::embedded_data::note_builtin_copy("b.ron", "always");
    crate::embedded_data::B_RON
}

#[cfg(test)]
#[path = "fixture_tests.rs"]
mod tests;

#[cfg(all(test, feature = "native"))]
mod more {
    #[test]
    fn t() { let _ = include_str!("../tests/fixtures/only_tests.txt"); }
}
`;
const FIXTURE_TESTS = 'const F: &str = include_str!("../data/f_only_tests.ron");\n';

function analyse(extra = {}, opts = {}) {
  const dir = writeTree({
    "build.rs": BUILD_RS,
    "Cargo.toml": "[package]\n",
    "src/lib.rs": LIB,
    "src/embedded_data.rs": EMBEDDED,
    "src/fixture_tests.rs": FIXTURE_TESTS,
    "assets/icon.png": "png",
    ...extra,
  });
  return CI().analyse(dir, opts);
}
const cls = (a, t) => (a.targets.find((x) => x.target === t) || { class: "ABSENT" }).class;

test("RULES: each kind of include lands in its class", () => {
  const a = analyse();
  assert.strictEqual(cls(a, "assets/icon.png"), "fingerprinted"); // a FILE entry in FINGERPRINT_INPUTS
  assert.strictEqual(cls(a, "OUT_DIR/hos_src_stamp.txt"), "stamp");
  assert.strictEqual(cls(a, "data/a.csv"), "data-disk-first"); // only through get_embedded
  assert.strictEqual(cls(a, "data/c.ron"), "data-disk-first"); // in a fn that reads the disk
  assert.strictEqual(cls(a, "data/d.ron"), "data-disk-first"); // a const read by a disk-first fn
  assert.strictEqual(cls(a, "data/f_only_tests.ron"), "test-only"); // a #[cfg(test)] #[path] file
  assert.strictEqual(cls(a, "tests/fixtures/only_tests.txt"), "test-only"); // cfg(all(test, ..))
  assert.strictEqual(cls(a, "data/in_a_comment.ron"), "ABSENT");
  assert.strictEqual(cls(a, "data/in_a_string.ron"), "ABSENT");
});

test("RULES: an embedded-only read is UNCLASSIFIED, so the real-tree test can fail", () => {
  const a = analyse();
  assert.strictEqual(cls(a, "data/e.ron"), "UNCLASSIFIED"); // compiled-in only
  // b.ron IS served disk-first through the table, but b_now() reads the const
  // with no disk read: one embedded-only read is enough to fail.
  assert.strictEqual(cls(a, "data/b.ron"), "UNCLASSIFIED");
  assert.match(a.targets.find((t) => t.target === "data/b.ron").reason, /src\/lib\.rs:\d+ fn b_now/);
});

test("RULES: ALLOWLIST and FALLBACK_SITES clear a file, and a stale entry is itself a failure", () => {
  const a = analyse({}, { allowlist: { "data/e.ron": "fixture reason" }, fallbackSites: { "src/lib.rs::b_now": "fixture reason" } });
  assert.strictEqual(cls(a, "data/e.ron"), "allowlisted");
  assert.strictEqual(cls(a, "data/b.ron"), "data-disk-first");
  assert.deepStrictEqual(a.problems, []);
  const stale = analyse({}, { allowlist: { "data/gone.ron": "x" }, fallbackSites: { "src/lib.rs::load_c": "x" } });
  assert.ok(stale.problems.some((p) => /ALLOWLIST names data\/gone\.ron/.test(p)), stale.problems.join("\n"));
  assert.ok(stale.problems.some((p) => /FALLBACK_SITES names src\/lib\.rs::load_c/.test(p)), stale.problems.join("\n"));
});

test("RULES: an include whose path cannot be worked out is a problem, never skipped", () => {
  const a = analyse({ "src/odd.rs": 'const X: &str = include_str!(some_macro!());\n' });
  assert.ok(a.problems.some((p) => /src\/odd\.rs:1: cannot resolve/.test(p)), a.problems.join("\n"));
});

test("RULES: a get_embedded(variable) with no disk read in its fn is a problem until reviewed", () => {
  const extra = {
    "src/helper.rs":
      'pub fn any(p: &str) -> Option<&\'static str> { crate::embedded_data::note_builtin_copy(p, "fallback"); crate::embedded_data::get_embedded(p) }\n',
  };
  const a = analyse(extra);
  assert.ok(a.problems.some((p) => /src\/helper\.rs:1 \(fn any\): get_embedded\(p\)/.test(p)), a.problems.join("\n"));
  const b = analyse(extra, { fallbackSites: { "src/helper.rs::any": "fixture: only called after a disk miss", "src/lib.rs::b_now": "x" } });
  assert.ok(!b.problems.some((p) => /helper/.test(p)), b.problems.join("\n"));
});

test("RULES: read_data_or_embedded that consults the table BEFORE the disk voids the disk-first class", () => {
  const backwards = EMBEDDED.replace(
    /match std::fs::read_to_string[\s\S]*?\n    }\n/,
    "get_embedded(rel).map(|s| s.to_string()).or_else(|| std::fs::read_to_string(data_dir.join(rel)).ok())\n"
  );
  assert.ok(backwards.includes("or_else(|| std::fs"), "the fixture rewrite took");
  const a = analyse({ "src/embedded_data.rs": backwards });
  assert.ok(a.problems.some((p) => /read_data_or_embedded does not read the disk before/.test(p)), a.problems.join("\n"));
  assert.strictEqual(cls(a, "data/a.csv"), "UNCLASSIFIED");
});

// BUG-163 (2026-10-05): load_data_or_embedded also serves the table, when the disk copy
// cannot be read in full. Every fn in embedded_data.rs that consults the table is held to
// the same rule as read_data_or_embedded. RED FIRST, against the check that looked only at
// read_data_or_embedded: both tests that must find a problem failed, as no problem named
// load_data_or_embedded (a reader that consulted the table first, or never said so, passed).
const SECOND_READER = `
pub fn load_data_or_embedded<T>(data_dir: &std::path::Path, rel: &str, build: impl Fn(&[u8]) -> Result<T, String>) -> Result<T, String> {
    let why = match std::fs::read(data_dir.join(rel)) {
        Ok(bytes) => match build(&bytes) {
            Ok(v) => return Ok(v),
            Err(e) => e,
        },
        Err(e) => e.to_string(),
    };
    let built_in = get_embedded(rel).ok_or_else(|| why.clone())?;
    note_builtin_copy(rel, &why);
    build(built_in.as_bytes())
}
`;

test("RULES: a second reader of the table that reads the disk first and says so keeps the class", () => {
  const a = analyse({ "src/embedded_data.rs": EMBEDDED + SECOND_READER });
  assert.strictEqual(cls(a, "data/a.csv"), "data-disk-first");
  assert.ok(!a.problems.some((p) => /load_data_or_embedded/.test(p)), a.problems.join("\n"));
});

test("RULES: a second reader of the table that consults it BEFORE the disk voids the disk-first class", () => {
  const backwards = SECOND_READER.replace(
    "let why = match std::fs::read(data_dir.join(rel)) {",
    "let _first = get_embedded(rel);\n    let why = match std::fs::read(data_dir.join(rel)) {"
  );
  assert.ok(backwards.includes("let _first = get_embedded(rel);"), "the fixture rewrite took");
  const a = analyse({ "src/embedded_data.rs": EMBEDDED + backwards });
  assert.ok(a.problems.some((p) => /load_data_or_embedded does not read the disk before/.test(p)), a.problems.join("\n"));
  assert.strictEqual(cls(a, "data/a.csv"), "UNCLASSIFIED");
});

test("RULES: a second reader of the table that never says it served the built-in copy is a problem", () => {
  const silent = SECOND_READER.replace("note_builtin_copy(rel, &why);", "let _ = &why;");
  assert.ok(silent.includes("let _ = &why;"), "the fixture rewrite took");
  const a = analyse({ "src/embedded_data.rs": EMBEDDED + silent });
  assert.ok(a.problems.some((p) => /load_data_or_embedded never calls note_builtin_copy/.test(p)), a.problems.join("\n"));
  assert.strictEqual(cls(a, "data/a.csv"), "UNCLASSIFIED");
});

test("RULES: a test cfg shape the check does not know is reported, not guessed", () => {
  const a = analyse({ "src/odd_cfg.rs": '#[cfg(any(test, doc))]\nconst X: &str = include_str!("../data/x.ron");\n' });
  assert.ok(a.problems.some((p) => /odd_cfg\.rs:1: a test cfg this check does not understand/.test(p)), a.problems.join("\n"));
});

// ── Critic review 2 (2026-10-03): shapes the first version could not see ──────
//
// RED FIRST (2026-10-03, these tests against the compiled-in.js of c15a4be8b):
//   ✖ RULES: a const read through a GROUPED import is a read
//       data/g1.ron: 'data-disk-first' !== 'UNCLASSIFIED'
//   ✖ RULES: a const read through a GLOB import is a read
//       data/g2.ron: 'test-only' !== 'UNCLASSIFIED'
//   ✖ RULES: an aliased import and a re-export are followed or refused, never missed
//   ✖ RULES: an unrelated disk read (Path::exists on another file) is not "disk first"
//       data/g3.ron: 'data-disk-first' !== 'UNCLASSIFIED'
//   ✖ RULES: a disk read AFTER the built-in read is not "disk first"
//   ✖ RULES: a fallback that never says so at run time is not "disk first"
//   ✖ RULES: the note must name the file it serves
//   ✖ RULES: read_data_or_embedded that never says it served the built-in copy is a problem
//   ✖ RULES: a reviewed fallback that never says so at run time is a problem
//   ✖ RULES: a Cargo path dependency outside FINGERPRINT_INPUTS is UNCLASSIFIED
//       vendor/foo: 'ABSENT' !== 'UNCLASSIFIED' (Cargo.toml was never read)
//   ✖ RULES: a #[path] module outside the fingerprinted tree is UNCLASSIFIED
// and the real-tree test, with the new rules and before the Rust side caught up
// (compiled-in files: 187, UNCLASSIFIED 20, and 14 PROBLEMS), for example:
//   UNCLASSIFIED     vendor/rav1d
//                    compiled into the binary as Rust source (Cargo.toml line 260, a path in
//                    Cargo.toml (a target, a path dependency or a [patch] entry)) but not under
//                    FINGERPRINT_INPUTS ...
//   UNCLASSIFIED     data/ground/materials.ron
//                    ... fn material_table never calls note_builtin_copy("ground/materials.ron", ..)
//                    to say so at run time ...   (the loader the critic broke on purpose)
//   PROBLEMS: src/ship/hull.rs:167 (fn load): get_embedded(HULL_PROFILE_REL) reads an embedded
//   copy chosen at run time, and its fn does not both read the disk before it and call
//   note_builtin_copy(HULL_PROFILE_REL, ..) ...; FALLBACK_SITES: src/assets/mod.rs::parse_embedded_csv
//   serves the built-in copy but never calls note_builtin_copy ...
const DEFS = `
pub const G1_RON: &str = include_str!("../data/g1.ron");
pub const G2_RON: &str = include_str!("../data/g2.ron");
pub struct Other;
/// The disk-first loader of g1: a perfectly good one.
pub fn load_g1() -> String {
    std::fs::read_to_string("data/g1.ron").unwrap_or_else(|e| {
        crate::embedded_data::note_builtin_copy("g1.ron", e);
        G1_RON.to_string()
    })
}
`;
const reasonOf = (a, t) => (a.targets.find((x) => x.target === t) || { reason: "" }).reason;

test("RULES: a const read through a GROUPED import is a read", () => {
  const a = analyse({
    "src/defs.rs": DEFS,
    "src/user.rs": "use crate::defs::{Other, G1_RON};\npub fn now() -> &'static str { let _ = Other; G1_RON }\n",
  });
  assert.strictEqual(cls(a, "data/g1.ron"), "UNCLASSIFIED", "data/g1.ron");
  assert.match(reasonOf(a, "data/g1.ron"), /src\/user\.rs:2 fn now/);
});

test("RULES: a const read through a GLOB import is a read", () => {
  const a = analyse({ "src/defs.rs": DEFS, "src/user.rs": "use crate::defs::*;\npub fn now() -> &'static str { G2_RON }\n" });
  assert.strictEqual(cls(a, "data/g2.ron"), "UNCLASSIFIED", "data/g2.ron");
  assert.match(reasonOf(a, "data/g2.ron"), /src\/user\.rs:2 fn now/);
});

test("RULES: an aliased import and a re-export are followed or refused, never missed", () => {
  const aliased = analyse({ "src/defs.rs": DEFS, "src/user.rs": "use crate::defs::G1_RON as TABLE;\npub fn now() -> &'static str { TABLE }\n" });
  assert.strictEqual(cls(aliased, "data/g1.ron"), "UNCLASSIFIED", "a read through an alias");
  const reexport = analyse({ "src/defs.rs": DEFS, "src/user.rs": "pub use crate::defs::G1_RON;\n" });
  assert.ok(reexport.problems.some((p) => /src\/user\.rs:1: .*re-export/.test(p)), reexport.problems.join("\n"));
});

test("RULES: a same-named const of another module is not ours", () => {
  // user.rs defines its own G1_RON: its bare G1_RON is that one, not defs.rs's.
  const a = analyse({ "src/defs.rs": DEFS, "src/user.rs": "const G1_RON: &str = \"local\";\npub fn now() -> &'static str { G1_RON }\n" });
  assert.strictEqual(cls(a, "data/g1.ron"), "data-disk-first");
});

// Red first (2026-10-03), against the first version of the qualifier check
// (the file's module name only): 'data-disk-first' !== 'UNCLASSIFIED'. The
// real case: billboard_bake.rs reads `super::leaf_shape::EMBEDDED_TREES`, a
// const of its own inline `mod leaf_shape`, while tree_mesh.rs defines an
// EMBEDDED_TREES too.
test("RULES: a qualified read through an inline module of the defining file is a read", () => {
  const a = analyse({
    "src/inl.rs": `pub mod shapes {
    pub(super) const IN_RON: &str = include_str!("../data/in.ron");
    pub fn load() -> String {
        std::fs::read_to_string("data/in.ron").unwrap_or_else(|e| {
            crate::embedded_data::note_builtin_copy("in.ron", e);
            IN_RON.to_string()
        })
    }
}
pub fn now() -> &'static str { self::shapes::IN_RON }
`,
    "src/other_in.rs": 'const IN_RON: &str = "a same-named const elsewhere";\npub fn x() -> &\'static str { IN_RON }\n',
  });
  assert.strictEqual(cls(a, "data/in.ron"), "UNCLASSIFIED", "data/in.ron");
  assert.match(reasonOf(a, "data/in.ron"), /src\/inl\.rs:\d+ fn now/);
});

test('RULES: an unrelated disk read (Path::exists on another file) is not "disk first"', () => {
  const a = analyse({
    "src/g3.rs": `pub fn table() -> &'static str {
    if std::path::Path::new("portable.txt").exists() { let _ = 1; }
    include_str!("../data/g3.ron")
}
`,
  });
  assert.strictEqual(cls(a, "data/g3.ron"), "UNCLASSIFIED", "data/g3.ron");
});

test('RULES: a disk read AFTER the built-in read is not "disk first"', () => {
  const a = analyse({
    "src/g4.rs": `pub fn table() -> String {
    let built_in = include_str!("../data/g4.ron");
    crate::embedded_data::note_builtin_copy("g4.ron", "always");
    let _later = std::fs::read_to_string("data/g4.ron");
    built_in.to_string()
}
`,
  });
  assert.strictEqual(cls(a, "data/g4.ron"), "UNCLASSIFIED");
  assert.match(reasonOf(a, "data/g4.ron"), /no disk read before it/);
});

test('RULES: a fallback that never says so at run time is not "disk first"', () => {
  // The shape of every masked edit: the disk copy fails to parse, the built-in
  // copy is served, and nothing in run.log that a rig could catch says so.
  const a = analyse({
    "src/g5.rs": `pub fn table() -> String {
    match std::fs::read_to_string("data/g5.ron") {
        Ok(s) => s,
        Err(_) => include_str!("../data/g5.ron").to_string(),
    }
}
`,
  });
  assert.strictEqual(cls(a, "data/g5.ron"), "UNCLASSIFIED");
  assert.match(reasonOf(a, "data/g5.ron"), /note_builtin_copy\("g5\.ron"/);
});

test("RULES: the note must name the file it serves", () => {
  const a = analyse({
    "src/g6.rs": `pub fn table() -> String {
    match std::fs::read_to_string("data/g6.ron") {
        Ok(s) => s,
        Err(e) => { crate::embedded_data::note_builtin_copy("something_else.ron", e); include_str!("../data/g6.ron").to_string() }
    }
}
`,
  });
  assert.strictEqual(cls(a, "data/g6.ron"), "UNCLASSIFIED");
});

test("RULES: read_data_or_embedded that never says it served the built-in copy is a problem", () => {
  const silent = EMBEDDED.replace(
    /let s = get_embedded\(rel\)\?;\n\s*note_builtin_copy\(rel, e\);\n\s*Some\(s\.to_string\(\)\)/,
    "get_embedded(rel).map(|s| { let _ = e; s.to_string() })"
  );
  assert.ok(silent.includes("let _ = e;"), "the fixture rewrite took");
  const a = analyse({ "src/embedded_data.rs": silent });
  assert.ok(a.problems.some((p) => /read_data_or_embedded never calls note_builtin_copy/.test(p)), a.problems.join("\n"));
  assert.strictEqual(cls(a, "data/a.csv"), "UNCLASSIFIED");
});

test("RULES: a reviewed fallback that never says so at run time is a problem", () => {
  const extra = { "src/helper.rs": "pub fn any(p: &str) -> Option<&'static str> { crate::embedded_data::get_embedded(p) }\n" };
  const a = analyse(extra, { fallbackSites: { "src/helper.rs::any": "fixture", "src/lib.rs::b_now": "x" } });
  assert.ok(a.problems.some((p) => /src\/helper\.rs::any .*note_builtin_copy/.test(p)), a.problems.join("\n"));
});

test("RULES: a Cargo path dependency outside FINGERPRINT_INPUTS is UNCLASSIFIED", () => {
  const cargo =
    '[package]\nname = "x"\n[[bin]]\nname = "x"\npath = "src/main.rs" # under src\n# path = "commented/out"\n' +
    '[patch.crates-io]\nfoo = { path = "vendor/foo" }\n';
  const a = analyse({ "Cargo.toml": cargo, "vendor/foo/src/lib.rs": "pub fn f() {}\n" });
  assert.strictEqual(cls(a, "vendor/foo"), "UNCLASSIFIED", "vendor/foo");
  assert.match(reasonOf(a, "vendor/foo"), /Cargo\.toml/);
  assert.strictEqual(cls(a, "commented/out"), "ABSENT", "a commented-out line is not a dependency");
  assert.strictEqual(cls(a, "src/main.rs"), "fingerprinted");
  const covered = analyse({
    "Cargo.toml": cargo,
    "vendor/foo/src/lib.rs": "pub fn f() {}\n",
    "build.rs": BUILD_RS.replace('"assets/icon.png"', '"assets/icon.png", "vendor"'),
  });
  assert.strictEqual(cls(covered, "vendor/foo"), "fingerprinted");
});

test("RULES: a #[path] module outside the fingerprinted tree is UNCLASSIFIED", () => {
  const a = analyse({
    "src/odd_path.rs": '#[path = "../tests/shared_helpers.rs"]\nmod helpers;\n',
    "tests/shared_helpers.rs": "pub fn h() {}\n",
  });
  assert.strictEqual(cls(a, "tests/shared_helpers.rs"), "UNCLASSIFIED");
});

test("the lexer keeps offsets and blanks comments and string contents only", () => {
  const src = 'let a = "x // y"; // c\n/* b /* nested */ */ let q = \'"\'; let r = r#"in "raw""#; \'a: loop {}';
  const { noComments, codeOnly } = CI().lexRust(src);
  assert.strictEqual(noComments.length, src.length);
  assert.strictEqual(codeOnly.length, src.length);
  assert.ok(noComments.includes('"x // y"'));
  assert.ok(!noComments.includes("// c"));
  assert.ok(!noComments.includes("nested"));
  assert.ok(!codeOnly.includes("x // y"));
  assert.ok(!codeOnly.includes("in "));
  assert.ok(codeOnly.includes("'a: loop {}"), "a loop label is code, not a char literal");
});
