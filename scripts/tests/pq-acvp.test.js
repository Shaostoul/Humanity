// NIST's official test vectors against the website's post-quantum library (2026-11-19, the
// post-quantum review's plan item 1b, docs/reference/research/2026-11-16-pqc-review/).
//
// Run: node --test scripts/tests/pq-acvp.test.js   (discovered by `just rig-tests`; in CI)
//
// The vendored noble bundle the website serves (web/shared/vendor/noble-pq.bundle.js,
// @noble/post-quantum 0.6.1) against NIST's ACVP vectors (FIPS 203 and FIPS 204, the "internal
// projection" files, which carry the expected answers), cut down to the parameter sets HumanityOS
// uses, ML-KEM-768 and ML-DSA-65, by scripts/acvp-extract.mjs into scripts/tests/fixtures/acvp/
// (each fixture names its ACVP-Server commit and the source file's SHA-256; NIST's notice is
// NOTICE.md beside them). The pinned RustCrypto crates meet the same vectors in
// crates/humanity-protocol/src/pq_acvp_tests.rs.
//
// Every group in a fixture runs: a group this file cannot place fails instead of being skipped.
// HashML-DSA (pre-hash) groups are not in the fixtures; each fixture's `left_out` says why.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const { REPO, loadNoble } = require("../lib/chat-pq-page.js");

const DIR = process.env.HOS_ACVP_DIR || path.join(REPO, "scripts", "tests", "fixtures", "acvp");
const hex = (b) => Buffer.from(b).toString("hex");
const unhex = (s) => new Uint8Array(Buffer.from(s, "hex"));

function vectors(name) {
  const v = JSON.parse(fs.readFileSync(path.join(DIR, name), "utf8"));
  assert.strictEqual(v.source.repository, "https://github.com/usnistgov/ACVP-Server", `${name}: from NIST's ACVP-Server`);
  assert.match(v.source.commit, /^[0-9a-f]{40}$/, `${name}: names its commit`);
  assert.match(v.source.sha256, /^[0-9a-f]{64}$/, `${name}: names its source file's SHA-256`);
  assert.ok(v.testGroups.length > 0, `${name}: has groups`);
  for (const g of v.testGroups) assert.strictEqual(g.parameterSet, v.parameterSet, `${name}: only ${v.parameterSet}`);
  return v;
}

/** Count of tests run per kind, compared at the end so nothing is silently skipped. */
function tally() {
  const ran = {};
  return { add: (k) => { ran[k] = (ran[k] || 0) + 1; }, ran };
}

// Seen red 2026-11-19 with one hex digit of the first test's `ek` changed in the fixture (a copy
// under HOS_ACVP_DIR): "tcId 26: the encapsulation key" failed.
test("ML-KEM-768 key generation (FIPS 203 KeyGen_internal from d || z)", async () => {
  const m = await loadNoble();
  const v = vectors("ML-KEM-768-keyGen.json");
  let n = 0;
  for (const g of v.testGroups) {
    assert.strictEqual(g.testType, "AFT");
    for (const t of g.tests) {
      const { publicKey, secretKey } = m.ml_kem768.keygen(unhex(t.d + t.z));
      assert.strictEqual(hex(publicKey), t.ek.toLowerCase(), `tcId ${t.tcId}: the encapsulation key`);
      assert.strictEqual(hex(secretKey), t.dk.toLowerCase(), `tcId ${t.tcId}: the decapsulation key`);
      n++;
    }
  }
  assert.strictEqual(n, 25, "every key-generation vector ran");
});

// Seen red 2026-11-19 with one hex digit of the first decapsulation test's `k` changed in the
// fixture (a copy under HOS_ACVP_DIR): "tcId 86 (valid decapsulation): the shared secret" failed.
test("ML-KEM-768 encapsulation, decapsulation (with modified ciphertexts) and the key checks", async () => {
  const m = await loadNoble();
  const v = vectors("ML-KEM-768-encapDecap.json");
  const { add, ran } = tally();
  const refuses = (fn) => { try { fn(); return false; } catch (e) { return true; } };
  for (const g of v.testGroups) {
    for (const t of g.tests) {
      const id = `tcId ${t.tcId} (${t.reason || "AFT"})`;
      switch (g.function) {
        case "encapsulation": {
          const { cipherText, sharedSecret } = m.ml_kem768.encapsulate(unhex(t.ek), unhex(t.m));
          assert.strictEqual(hex(cipherText), t.c.toLowerCase(), `${id}: the ciphertext`);
          assert.strictEqual(hex(sharedSecret), t.k.toLowerCase(), `${id}: the shared secret`);
          break;
        }
        case "decapsulation":
          assert.strictEqual(hex(m.ml_kem768.decapsulate(unhex(t.c), unhex(t.dk))), t.k.toLowerCase(), `${id}: the shared secret`);
          break;
        case "encapsulationKeyCheck":
          // noble checks the key's coefficients when it encapsulates ("wrong publicKey modulus").
          assert.strictEqual(!refuses(() => m.ml_kem768.encapsulate(unhex(t.ek))), t.testPassed, `${id}: the encapsulation key check`);
          break;
        case "decapsulationKeyCheck":
          // noble checks the stored hash of the encapsulation key when it decapsulates
          // ("invalid secretKey: hash check failed"); any ciphertext of the right length will do.
          assert.strictEqual(!refuses(() => m.ml_kem768.decapsulate(new Uint8Array(1088), unhex(t.dk))), t.testPassed, `${id}: the decapsulation key check`);
          break;
        default:
          assert.fail(`an encapDecap group this test cannot place: function ${g.function}`);
      }
      add(g.function);
    }
  }
  assert.deepStrictEqual(ran, { encapsulation: 25, decapsulation: 10, encapsulationKeyCheck: 10, decapsulationKeyCheck: 10 }, "every vector ran");
});

// Seen red 2026-11-19 with one hex digit of the first test's `pk` changed in the fixture (a copy
// under HOS_ACVP_DIR): "tcId 26: the public key" failed.
test("ML-DSA-65 key generation (FIPS 204 KeyGen_internal from the seed)", async () => {
  const m = await loadNoble();
  const v = vectors("ML-DSA-65-keyGen.json");
  let n = 0;
  for (const g of v.testGroups) {
    for (const t of g.tests) {
      const { publicKey, secretKey } = m.ml_dsa65.keygen(unhex(t.seed));
      assert.strictEqual(hex(publicKey), t.pk.toLowerCase(), `tcId ${t.tcId}: the public key`);
      assert.strictEqual(hex(secretKey), t.sk.toLowerCase(), `tcId ${t.tcId}: the secret key`);
      n++;
    }
  }
  assert.strictEqual(n, 25, "every key-generation vector ran");
});

// Seen red 2026-11-19 with one hex digit of a hedged external test's `signature` changed in the
// fixture (a copy under HOS_ACVP_DIR): "external hedged tcId 211: the signature" failed.
test("ML-DSA-65 signing: external (with context), internal and precomputed mu, deterministic and hedged", async () => {
  const m = await loadNoble();
  const v = vectors("ML-DSA-65-sigGen.json");
  const { add, ran } = tally();
  for (const g of v.testGroups) {
    assert.strictEqual(g.preHash, g.signatureInterface === "external" ? "pure" : "none", "pre-hash groups are left out of the fixture");
    const kind = `${g.signatureInterface}${g.externalMu ? " mu" : ""} ${g.deterministic ? "deterministic" : "hedged"}`;
    for (const t of g.tests) {
      const extraEntropy = g.deterministic ? false : unhex(t.rnd);
      let sig;
      if (g.signatureInterface === "external" && !g.externalMu) {
        sig = m.ml_dsa65.sign(unhex(t.message), unhex(t.sk), { context: unhex(t.context), extraEntropy });
      } else if (g.signatureInterface === "internal") {
        sig = m.ml_dsa65.internal.sign(unhex(g.externalMu ? t.mu : t.message), unhex(t.sk), { externalMu: !!g.externalMu, extraEntropy });
      } else {
        assert.fail(`a sigGen group this test cannot place: ${kind}`);
      }
      assert.strictEqual(hex(sig), t.signature.toLowerCase(), `${kind} tcId ${t.tcId}: the signature`);
      add(kind);
    }
  }
  const want = Object.fromEntries(["external deterministic", "external hedged", "internal deterministic", "internal hedged", "internal mu deterministic", "internal mu hedged"].map((k) => [k, 15]));
  assert.deepStrictEqual(ran, want, "every signing vector ran");
});

// Seen red 2026-11-19 with the first test's `testPassed` flipped in the fixture (a copy under
// HOS_ACVP_DIR): "tcId 31 (modified signature - z): the verdict" failed.
test("ML-DSA-65 verification: valid signatures verify, modified messages and signatures do not", async () => {
  const m = await loadNoble();
  const v = vectors("ML-DSA-65-sigVer.json");
  let ran = 0;
  let refused = 0;
  for (const g of v.testGroups) {
    for (const t of g.tests) {
      const id = `tcId ${t.tcId} (${t.reason})`;
      let got;
      if (g.signatureInterface === "external" && !g.externalMu) {
        got = m.ml_dsa65.verify(unhex(t.signature), unhex(t.message), unhex(t.pk), { context: unhex(t.context) });
      } else if (g.signatureInterface === "internal") {
        got = m.ml_dsa65.internal.verify(unhex(t.signature), unhex(g.externalMu ? t.mu : t.message), unhex(t.pk), { externalMu: !!g.externalMu });
      } else {
        assert.fail(`a sigVer group this test cannot place: ${g.signatureInterface}`);
      }
      assert.strictEqual(got, t.testPassed, `${id}: the verdict`);
      if (!t.testPassed) refused++;
      ran++;
    }
  }
  assert.strictEqual(ran, 45, "every verification vector ran");
  assert.ok(refused >= 30, `the modified signatures and messages are among them: ${refused}`);
});
