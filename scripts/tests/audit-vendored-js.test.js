// scripts/audit-vendored-js.mjs reads the right versions to audit (2026-11-19): the pins in
// scripts/build-noble-bundle.mjs, held to the vendored bundle's own header, so the advisory job
// (.github/workflows/advisories.yml) audits exactly what the website ships. Offline: the npm part
// runs only in CI.
//
// Run: node --test scripts/tests/audit-vendored-js.test.js   (discovered by `just rig-tests`)

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const { pathToFileURL } = require("node:url");

const REPO = path.join(__dirname, "..", "..");
const load = () => import(pathToFileURL(path.join(REPO, "scripts", "audit-vendored-js.mjs")).href);
const BUILDER = fs.readFileSync(path.join(REPO, "scripts", "build-noble-bundle.mjs"), "utf8");
const BUNDLE = fs.readFileSync(path.join(REPO, "web", "shared", "vendor", "noble-pq.bundle.js"), "utf8");

// Seen red 2026-11-19, for real: the header was first read with /Source: (.+)\n/, and in a Windows
// working copy (CRLF line endings) `.` stops at the \r, so it failed with 'the vendored bundle's
// header names "", but scripts/build-noble-bundle.mjs pins ...'.
test("the versions audited are the builder's exact pins, the ones the bundle names", async () => {
  const { vendoredPins } = await load();
  const pins = vendoredPins();
  assert.deepStrictEqual(Object.keys(pins).sort(), ["@noble/hashes", "@noble/post-quantum"], "both packages");
  for (const v of Object.values(pins)) assert.match(v, /^\d+\.\d+\.\d+$/, "exact versions, no ranges");
  assert.ok(BUNDLE.slice(0, 600).includes(`Source: @noble/post-quantum@${pins["@noble/post-quantum"]} + @noble/hashes@${pins["@noble/hashes"]}`), "the bundle names them");
  assert.deepStrictEqual(vendoredPins(BUILDER, BUNDLE), pins, "the builder's exact pins");
});

test("a pin changed without rebuilding the bundle, or a range, is refused", async () => {
  const { vendoredPins } = await load();
  const changed = BUILDER.replace(/const PQ_VER = '[^']+'/, "const PQ_VER = '9.9.9'");
  assert.notStrictEqual(changed, BUILDER, "the test changed the pin");
  assert.throws(() => vendoredPins(changed, BUNDLE), /rebuild the bundle/, "a pin the bundle was not built from");
  assert.throws(() => vendoredPins(BUILDER.replace(/const HASHES_VER = '[^']+'/, "const HASHES_VER = '^2.2.0'"), BUNDLE), /no exact HASHES_VER pin/, "a range");
});
