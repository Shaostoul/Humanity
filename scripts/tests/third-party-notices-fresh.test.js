// THIRD-PARTY-NOTICES.txt is GENERATED (scripts/gen-third-party-notices.js, 2026-11-20): the copyright
// and licence notices of every Rust package the programs link and of the website's vendored code,
// shipped beside the program in every download. This holds it to its inputs the way
// scripts/check-library-fresh.js holds the Library to its sources. Pure node, no cargo and no
// registry, so it runs everywhere (`just rig-tests`, CI): it recomputes the fingerprint the file's
// header carries (Cargo.lock's packages, the workspace members, the generator, the standard texts,
// the vendored code's licence files) and the checksum of its body.
//
// Run:  node --test scripts/tests/third-party-notices-fresh.test.js   (discovered by `just rig-tests`)
//
// SEEN RED 2026-11-20, for real: with `version = "1.0.229"` changed to "1.0.230" under serde in
// Cargo.lock (and the committed file left alone), the first test failed with "THIRD-PARTY-NOTICES.txt
// is stale against its inputs (Cargo.lock, ...): run node scripts/gen-third-party-notices.js" and the
// third with "serde 1.0.229 is named in the notices, but Cargo.lock has serde 1.0.230"; with one word
// of a licence text changed by hand ("hereby" to "herby"), the second failed with "the body no longer
// matches its checksum; it was edited by hand". Both restored, all six pass. The last two tests keep
// that red reachable for good.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const path = require("path");
const gen = require("../gen-third-party-notices.js");
const release = require("../lib/release-files.js");

const ROOT = path.resolve(__dirname, "..", "..");
const DOC = fs.readFileSync(path.join(ROOT, gen.OUT), "utf8");
const LOCK = fs.readFileSync(path.join(ROOT, "Cargo.lock"), "utf8");

/** name -> the versions Cargo.lock holds. */
function lockVersions(lockText) {
  const out = new Map();
  for (const m of lockText.replace(/\r/g, "").matchAll(/^\[\[package\]\]\nname = "([^"]+)"\nversion = "([^"]+)"/gm)) {
    out.set(m[1], [...(out.get(m[1]) || []), m[2]]);
  }
  return out;
}

test("the committed notices match Cargo.lock and their other inputs (the fingerprint)", () => {
  const h = gen.readHeader(DOC);
  assert.ok(h.fingerprint, `${gen.OUT} has no Fingerprint line: run node scripts/gen-third-party-notices.js`);
  assert.strictEqual(
    h.fingerprint,
    gen.fingerprint(ROOT),
    `${gen.OUT} is stale against its inputs (Cargo.lock, the workspace members, the generator, scripts/lib/licence-texts/, the vendored code's licence files): run node scripts/gen-third-party-notices.js`,
  );
});

test("nobody edited the notices by hand (the body checksum)", () => {
  const h = gen.readHeader(DOC);
  assert.ok(h.checksum, `${gen.OUT} has no Body checksum line`);
  assert.ok(h.bodyMatches, `${gen.OUT}: the body no longer matches its checksum; it was edited by hand. Regenerate it: node scripts/gen-third-party-notices.js`);
});

test("every Rust package the notices name is in Cargo.lock at that version", () => {
  const lock = lockVersions(LOCK);
  const rust = gen.entriesOf(DOC).filter((e) => e.part === 1);
  assert.ok(rust.length > 300, `the notices name ${rust.length} Rust packages; the programs link hundreds`);
  for (const e of rust) {
    const have = lock.get(e.name) || [];
    assert.ok(have.includes(e.version), `${e.name} ${e.version} is named in the notices, but Cargo.lock has ${e.name} ${have.join(", ") || "(none)"}`);
  }
  const web = gen.entriesOf(DOC).filter((e) => e.part === 2);
  assert.ok(web.some((e) => e.name === "@noble/post-quantum"), "Part 2 names the vendored post-quantum library");
});

// The licences docs/reference/findings/2026-11-20-dependency-licences-and-agpl.md reviewed against
// distributing an AGPL-3.0 program: the notice-only permissive ones, and the four families it
// quotes (MPL-2.0, Unicode-3.0, CDLA-Permissive-2.0, the two font licences). A package that can
// only be taken under something else (a GPL-2.0-only crate, an unknown or proprietary licence)
// fails here until someone researches it into a dated finding and adds it.
const REVIEWED = new Set([
  "MIT", "MIT-0", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Zlib", "0BSD", "Unlicense", "CC0-1.0", "BSL-1.0",
  "MPL-2.0", "Unicode-3.0", "CDLA-Permissive-2.0", "OFL-1.1", "Ubuntu-font-1.0",
]);

/** The linked packages whose licence expression cannot be met from REVIEWED alone. */
function unreviewed(doc) {
  const t = doc.replace(/\r\n?/g, "\n");
  const part1 = t.slice(t.indexOf("\nPART 1."), t.indexOf("\nPART 2."));
  const out = [];
  for (const m of part1.matchAll(/^(\S+) (\S+)\n {4}Licence: (.*)$/gm)) {
    let ok = false;
    try {
      ok = gen.satisfiable(gen.parseSpdx(m[3]), (id) => REVIEWED.has(id));
    } catch (e) {
      ok = false;
    }
    if (!ok) out.push(`${m[1]} ${m[2]} (${m[3]})`);
  }
  return out;
}

test("every linked package can be taken under a licence the dependency finding reviewed", () => {
  const bad = unreviewed(DOC);
  assert.deepStrictEqual(bad, [], `linked under a licence no dated finding has reviewed against the AGPL: ${bad.join("; ")}. Research it (docs/reference/findings/README.md) and add it to REVIEWED with the finding`);
});

// Seen red 2026-11-20 on the real file with "MPL-2.0" taken out of REVIEWED: "linked under a licence
// no dated finding has reviewed against the AGPL: symphonia 0.6.1 (MPL-2.0); symphonia-bundle-flac
// 0.6.1 (MPL-2.0); ...". Below, kept for good: a GPL-2.0-only package and a licence-file-only one
// are refused, while a dual licence with one reviewed side passes.
test("the licence gate can fail: GPL-2.0-only and unknown licences are refused", () => {
  const sample = "\nPART 1. X\n\nfoo 1.0.0\n    Licence: GPL-2.0-only\n\nbar 2.0.0\n    Licence: see LICENSE.txt\n\nbaz 3.0.0\n    Licence: GPL-3.0-or-later OR MIT\n\nPART 2. Y\n";
  assert.deepStrictEqual(unreviewed(sample), ["foo 1.0.0 (GPL-2.0-only)", "bar 2.0.0 (see LICENSE.txt)"]);
});

test("the gate can fail: a version changed in Cargo.lock changes the fingerprint", () => {
  const lf = LOCK.replace(/\r\n/g, "\n");
  const changed = lf.replace(/(\nname = "serde"\nversion = ")([^"]+)"/, '$1999.0.0"');
  assert.notStrictEqual(changed, lf, "the test changed serde's version");
  assert.notStrictEqual(gen.fingerprint(ROOT, changed), gen.fingerprint(ROOT, LOCK), "a changed version must make the committed notices stale");
  // The project's own version, bumped with every release, is NOT an input: a release must not
  // need the notices regenerated.
  const bumped = lf.replace(/(\nname = "humanity-engine"\nversion = ")([^"]+)"/, '$1999.0.0"');
  assert.notStrictEqual(bumped, lf, "the test changed humanity-engine's version");
  assert.strictEqual(gen.fingerprint(ROOT, bumped), gen.fingerprint(ROOT, LOCK), "a version bump of our own crate leaves the notices current");
});

test("the gate can fail: a hand edit no longer matches the checksum", () => {
  const edited = DOC.replace("Permission is hereby granted", "Permission is herby granted");
  assert.notStrictEqual(edited, DOC, "the test edited a licence text");
  assert.strictEqual(gen.readHeader(edited).bodyMatches, false);
});

test("the notices ship beside the program in every download", () => {
  assert.ok(release.FILES.includes(gen.OUT), `scripts/lib/release-files.js's FILES carries ${gen.OUT}`);
  assert.ok(release.releaseFiles(ROOT).includes(gen.OUT), `the release list carries ${gen.OUT}`);
});
