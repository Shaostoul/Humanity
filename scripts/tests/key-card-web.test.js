// Key cards, the website's half of the card itself (2026-10-22, docs/design/key-cards.md):
// web/shared/key-card.js (the words signed, the one spelling, the check, the pin rule and the
// sentences), held to the relay's src/relay/core/key_card.rs.
//
// Run: node --test scripts/tests/key-card-web.test.js   (in `just rig-tests`)
//
// Pure node, nothing booted or connected. The shared fixture scripts/tests/fixtures/key-card-v1.json
// is made by the Rust builder from the seeds [7; 32] and [9; 32] (ML-DSA signing in Rust is
// deterministic): Rust and JavaScript must agree on the card's signed bytes, or no app could check
// the other's cards. So this test reads the pinned words out of key_card.rs (the friend-pass.test.js
// way), builds the fixture's words with the website's cardText, checks the Rust signature over them
// with the vendored noble bundle, and checks that a web-signed card (noble signs hedged, so the
// bytes differ) verifies under the same words. HOS_WEB_DIR points the test at another copy of web/
// (used to see each test red).

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const REPO = path.join(__dirname, "..", "..");
const WEB = process.env.HOS_WEB_DIR || path.join(REPO, "web");
const K = require(path.join(WEB, "shared", "key-card.js"));
const RUST = fs.readFileSync(path.join(REPO, "src", "relay", "core", "key_card.rs"), "utf8");
const FX = JSON.parse(fs.readFileSync(path.join(REPO, "scripts", "tests", "fixtures", "key-card-v1.json"), "utf8"));

const enc = (s) => new TextEncoder().encode(s);
const unescape = (s) => s.replace(/\\(.)/g, (_, c) => ({ n: "\n", t: "\t", r: "\r", "\\": "\\", '"': '"', "0": "\0" })[c] ?? c);

let NOBLE = null;
async function noble() {
  if (NOBLE) return NOBLE;
  const src = fs.readFileSync(path.join(REPO, "web", "shared", "vendor", "noble-pq.bundle.js"));
  NOBLE = await import("data:text/javascript;base64," + src.toString("base64"));
  return NOBLE;
}
const verify = async (pub, msg, sig) => {
  const m = await noble();
  try { return !!m.ml_dsa65.verify(sig, msg, pub); } catch (e) { return false; }
};

/** The keys of the seed [byte; 32], derived exactly as web/chat/pq.js does. */
async function keysOf(byte) {
  const m = await noble();
  const seed = new Uint8Array(32).fill(byte);
  const dil = m.ml_dsa65.keygen(m.blake3.create({ context: enc("hum/dilithium3/v1"), dkLen: 32 }).update(seed).digest());
  const kyb = m.ml_kem768.keygen(m.blake3.create({ context: enc("hum/kyber768/v1"), dkLen: 64 }).update(seed).digest());
  return {
    key: Buffer.from(dil.publicKey).toString("hex"),
    secret: dil.secretKey,
    kyber: Buffer.from(kyb.publicKey).toString("base64"),
  };
}

// Seen red 2026-10-22 with `key` and `kyber` swapped in cardText: "cardText(\"ab12\", \"S3k=\",
// 1760000000001)" failed against the Rust literal.
test("the signed words are the relay's pinned words, the tag and every length too", () => {
  const pins = [...RUST.matchAll(/assert_eq!\(\s*card_text\(\s*"((?:[^"\\]|\\.)*)"\s*,\s*"((?:[^"\\]|\\.)*)"\s*,\s*([0-9_]+)\s*\)\s*,\s*"((?:[^"\\]|\\.)*)"\s*\)/g)];
  assert.equal(pins.length, 1, "key_card.rs pins card_text with one literal");
  for (const [, key, kyber, at, expected] of pins) {
    assert.equal(K.cardText(unescape(key), unescape(kyber), Number(at.replace(/_/g, ""))), unescape(expected), `cardText("${key}", "${kyber}", ${at.replace(/_/g, "")})`);
  }
  const tag = RUST.match(/pub const CARD_TAG: &str = "([^"]*)";/);
  assert.ok(tag, "key_card.rs names its tag");
  assert.equal(K.CARD_TAG, tag[1], "the tag");
  const max = RUST.match(/pub const REQUEST_MAX: usize = (\d+);/);
  assert.equal(K.REQUEST_MAX, Number(max[1]), "the most cards one request asks for");
  assert.equal(K.KEY_HEX_LEN, FX.card.key.length, "a key's length");
  assert.equal(K.KYBER_B64_LEN, FX.card.kyber.length, "a Kyber key's length in base64");
  assert.equal(K.SIG_B64_LEN, FX.card.sig.length, "a signature's length in base64");
});

// Rust and JavaScript agree on the card's signed bytes (the shared fixture). Seen red 2026-10-22 with
// `key` and `kyber` swapped in cardText (the copy of web/ through HOS_WEB_DIR the first test was red
// with): "the fixture's words" failed.
test("the fixture's card: the website's words, the Rust signature checks out over them, and the seed gives its keys", async () => {
  assert.equal(K.cardText(FX.card.key, FX.card.kyber, FX.card.at), FX.text, "the fixture's words");
  assert.ok(K.shapeOk(FX.card), "the card has a card's shape");
  assert.ok(await K.verifyCard(FX.card, verify), "the Rust signature checks out over the website's words");
  assert.ok(await K.cardFor(FX.card, FX.card.key, verify), "and it is the signer's card");
  const me = await keysOf(FX.signer_master_byte);
  assert.equal(me.key, FX.card.key, "pq.js's derivation from [7; 32] gives the card's key");
  assert.equal(me.kyber, FX.card.kyber, "and its Kyber768 key");
});

// A server-swapped key is refused. Seen red 2026-10-22 with verifyCard checking only shapeOk: "a
// card with the server's own Kyber key put in checked out" failed.
test("a server-swapped key, the wrong signer and somebody else's card are refused", async () => {
  assert.ok(K.shapeOk(FX.swapped), "the swap keeps the shape, so only the signature catches it");
  assert.equal(await K.verifyCard(FX.swapped, verify), false, "a card with the server's own Kyber key put in checked out");
  assert.equal(await K.verifyCard(FX.wrong_signer, verify), false, "a card signed by somebody else's key checked out");
  assert.ok(await K.verifyCard(FX.other_card, verify), "the other seed's own card is good");
  assert.equal(await K.cardFor(FX.other_card, FX.card.key, verify), false, "and is no card for the signer");
  const moved = { ...FX.card, at: FX.card.at + 1 };
  assert.equal(await K.verifyCard(moved, verify), false, "a card whose time was changed checked out");
});

test("one spelling: nothing but a canonical card is a card", async () => {
  const c = FX.card;
  const bad = [
    { ...c, key: c.key.toUpperCase() },
    { ...c, kyber: " " + c.kyber.slice(1) },
    { ...c, kyber: c.kyber.replace(/=+$/, "") },
    { ...c, sig: c.sig.slice(0, -4) },
    { ...c, v: 2 },
    { ...c, at: 0 },
    { ...c, at: "1760000000000" },
  ];
  for (const [i, b] of bad.entries()) {
    assert.equal(K.shapeOk(b), false, `bad spelling ${i} passed`);
    assert.equal(await K.verifyCard(b, verify), false, `bad spelling ${i} checked out`);
  }
  assert.equal(K.parseCard({ ...c, name: "Ann" }), null, "an unsigned field rode along");
  assert.equal(K.parseCard({ v: 1, key: c.key, kyber: c.kyber, at: c.at }), null, "a card without its signature");
  assert.deepEqual(K.parseCard(c), c);
});

test("a card the website signs checks out under the same words (noble signs hedged)", async () => {
  const me = await keysOf(FX.signer_master_byte);
  const m = await noble();
  const card = await K.signCard(me.key, me.kyber, FX.card.at, (bytes) => m.ml_dsa65.sign(bytes, me.secret));
  assert.equal(K.cardText(card.key, card.kyber, card.at), FX.text, "the same words");
  assert.ok(await K.cardFor(card, me.key, verify), "the web-signed card checks out");
});

// The pin rule rows are the relay's (key_card.rs compare). Seen red 2026-10-22 with sealingKey not
// looking at `changes`: "nothing is sealed to her while the change waits" failed.
test("the pin rule is the relay's, row for row, and take() keeps the pins and changes", () => {
  for (const row of FX.compare) {
    assert.equal(K.compare(row.pin, row.card.kyber, row.card.at), row.seen, JSON.stringify(row));
  }
  const state = { pins: {}, changes: {} };
  const card = (kyber, at) => ({ v: 1, key: "ann", kyber, at, sig: "" });
  assert.deepEqual(K.take(state, card("K1", 100)), { seen: "first", dirty: true });
  assert.equal(K.sealingKey(state, "ann"), "K1");
  assert.deepEqual(K.take(state, card("K2", 200)), { seen: "changed", dirty: true });
  assert.equal(K.sealingKey(state, "ann"), null, "nothing is sealed to her while the change waits");
  assert.deepEqual(K.take(state, card("K0", 50)), { seen: "stale", dirty: false });
  assert.equal(K.accept(state, "ann"), "K2");
  assert.equal(K.sealingKey(state, "ann"), "K2", "the accepted key is sealed to");
  assert.deepEqual(K.take(state, card("K3", 300)), { seen: "changed", dirty: true });
  assert.deepEqual(K.take(state, card("K2", 400)), { seen: "same", dirty: true }, "the newest card brings the pin back");
  assert.equal(K.sealingKey(state, "ann"), "K2", "and the change waiting is dropped");
});

test("the sentences are the relay's, word for word", () => {
  const r = FX.refusals;
  assert.equal(K.refusalText("no_card", r.name), r.no_card);
  assert.equal(K.refusalText("fetching", r.name), r.fetching);
  assert.equal(K.refusalText("changed", r.name), r.changed);
  assert.equal(K.changeNotice(r.name), r.notice);
  assert.equal(K.ACCEPT_LABEL, r.accept);
  // The words rules (CLAUDE.md): the retired account words and the long dash, spelled so this
  // file does not carry them itself.
  const banned = [["seed", "phrase"].join(" "), ["no", "account"].join(" "), String.fromCharCode(0x2014)];
  for (const s of [r.no_card, r.fetching, r.changed, r.notice]) {
    assert.ok(!banned.some((b) => s.includes(b)), `the words rules: ${s}`);
  }
});
