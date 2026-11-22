// Key cards, the website's half of the card itself (2026-10-22, docs/design/key-cards.md):
// web/shared/key-card.js (the words signed, the one spelling, the check, the pin rule and the
// sentences), held to the relay's crates/humanity-protocol/src/key_card.rs.
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
const RUST = fs.readFileSync(path.join(REPO, "crates", "humanity-protocol", "src", "key_card.rs"), "utf8");
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

// THE FINGERPRINT IS THE RELAY'S (the owner branch's second review, W1): the website's
// `fingerprint` gives the known answer pinned in key_card.rs's `the_fingerprint_is_pinned` (read
// out of that file), six groups of four hex digits. Seen red 2026-11-20 with the domain written
// "hum/key-fingerprint/v2": the known answer differed.
test("the fingerprint matches the relay's known answer", async () => {
  const m = await noble();
  const derive = (ctx, b) => m.blake3.create({ context: enc(ctx), dkLen: 32 }).update(b).digest();
  const kat = /assert_eq!\(fingerprint\(&key\), "([0-9a-f ]+)"\)/.exec(RUST);
  assert.ok(kat, "key_card.rs pins the known answer");
  assert.strictEqual(K.fingerprint("ab".repeat(1952), derive), kat[1]);
  assert.strictEqual(K.fingerprint("ab".repeat(1952), derive).split(" ").length, 6);
  assert.strictEqual(K.fingerprint("not hex", derive), "");
});

// FOUR GROUPS AT RANDOM POSITIONS CONFIRM IT (the third review's M2; four since the fourth review's
// finding 5, 2026-11-21): what the owner types for every handover and naming is the four groups at
// the positions the page drew for this ask (key_card.rs `groups_at`, its known answer read out of
// that file), in order, any case and spacing. The draw covers every set of four of the six groups
// (15) alike, and a key ground to match THREE groups (about 10^5 core-hours at 1 to 2 microseconds
// a candidate) passes no ask at all; one ground on four (2^64) passes exactly one set in 15.
// Seen red 2026-11-21 with groupsAt reading the last two groups whatever was asked: "the groups
// key_card.rs pins" got "c757 ed6b". Round 5, seen red 2026-11-21 with groupsMatchAt
// comparing only the first two positions asked (the round-4 two-group rule in effect): "a key ground
// on three groups passes no ask" failed at the set 0,1,2,3.
test("four groups at random positions confirm the fingerprint; a key ground on three groups passes no ask", async () => {
  const m = await noble();
  const derive = (ctx, b) => m.blake3.create({ context: enc(ctx), dkLen: 32 }).update(b).digest();
  const kat = /assert_eq!\(groups_at\(&fingerprint\(&key\), \[1, 2, 4, 5\]\), "([0-9a-f ]+)"\)/.exec(RUST);
  assert.ok(kat, "key_card.rs pins the groups at positions 1, 2, 4 and 5");
  const key = "ab".repeat(1952);
  const at = [1, 2, 4, 5];
  assert.strictEqual(K.CONFIRM_GROUPS, 4);
  assert.ok(/pub const CONFIRM_GROUPS: usize = 4;/.test(RUST), "the same count as key_card.rs");
  assert.strictEqual(K.groupsAt(K.fingerprint(key, derive), at), kat[1], "the groups key_card.rs pins");
  assert.ok(K.fingerprintConfirmedAt(key, at, "  " + kat[1].toUpperCase() + " ", derive), "in any case, spaces around");
  assert.ok(K.fingerprintConfirmedAt(key, at, kat[1].replace(/ /g, ""), derive), "or none between");
  assert.ok(!K.fingerprintConfirmedAt(key, at, kat[1].split(" ").slice(2).join(" "), derive), "two groups do not confirm four asked");
  assert.ok(!K.fingerprintConfirmedAt(key, at, "d2cc 223c c757 ed6b", derive), "nor the last four when others were asked");
  assert.ok(!K.fingerprintConfirmedAt(key, at, kat[1].split(" ").reverse().join(" "), derive), "nor out of order");
  assert.ok(!K.fingerprintConfirmedAt("not hex", [0, 1, 2, 3], "", derive) && !K.fingerprintConfirmedAt(key, [1, 2, 4, 6], kat[1], derive));
  assert.strictEqual(K.groupsAt(K.fingerprint(key, derive), [1, 1, 4, 5]), "", "a position named twice is no ask");

  // The draw (the fifth review's H1): uniform over all 15 sets, every ask, whatever was asked before
  // (key_card.rs `the_draw_is_uniform_over_all_fifteen_sets`). Each of the first 15 values lands on
  // a different set, a value past the rejection zone is drawn again, a generator that repeats gives
  // the same set again, and 30,000 draws land on each set 1,700 to 2,300 times (2,000 expected).
  // Seen red 2026-11-21 with round 5's rule put back (the set drawn before left out of the next
  // draw, remembered between calls; scratchpad ownfix8-red-web.log): "each of the first 15 values
  // lands on a different set" got 14.
  assert.strictEqual(K.confirmSets().length, 15);
  const byValue = new Set();
  for (let r = 0; r < 15; r++) byValue.add(K.confirmPositions(() => r).join(","));
  assert.strictEqual(byValue.size, 15, "each of the first 15 values lands on a different set");
  const zone = 0x100000000 - (0x100000000 % 15);
  const values = [zone, 3];
  assert.deepStrictEqual(K.confirmPositions(() => values.shift()), K.confirmSets()[3], "a value past the zone is drawn again");
  assert.deepStrictEqual(K.confirmPositions(() => 7), K.confirmPositions(() => 7), "a generator that repeats draws the same set again");
  const counts = new Map();
  for (let i = 0; i < 30000; i++) {
    const p = K.confirmPositions();
    assert.ok(p.length === 4 && p.every((x, j) => j === 0 || p[j - 1] < x) && p[0] >= 0 && p[3] < 6, "four different groups, ascending: " + p);
    counts.set(p.join(","), (counts.get(p.join(",")) || 0) + 1);
  }
  assert.strictEqual(counts.size, 15, "every set of four of the six groups is drawn");
  for (const [set, n] of counts) assert.ok(n >= 1700 && n <= 2300, set + " drawn " + n + " times of 30,000");

  // A wrong set is not an empty one (key_card.rs `a_wrong_set_is_not_an_empty_one`): only sixteen
  // characters that differ are a mismatch.
  // Seen red 2026-11-21 with judgeAt answering 'short' for any set that does not match (round 5's
  // behaviour; scratchpad ownfix8-red-web.log): "a wrong set of four is a mismatch" got 'short'.
  assert.strictEqual(K.judgeAt(key, at, "", derive), "empty");
  assert.strictEqual(K.judgeAt(key, at, "  ", derive), "empty");
  assert.strictEqual(K.judgeAt(key, at, "e5d0 d2cc", derive), "short", "two groups are not yet a judgement");
  assert.strictEqual(K.judgeAt(key, at, kat[1].toUpperCase(), derive), "matched");
  assert.strictEqual(K.judgeAt(key, at, "0000 0000 0000 0000", derive), "mismatched", "a wrong set of four is a mismatch");
  assert.strictEqual(K.judgeAt(key, at, kat[1] + " 0", derive), "short", "seventeen characters are not four groups");

  // Ground keys: R's fingerprint equals Bea's at three groups, nowhere else. The owner types Bea's
  // real groups at the asked positions (what she reads out); no set of four fits R.
  const bea = K.fingerprint(key, derive).split(" ");
  const groundOn = (on) => bea.map((g, i) => (on.includes(i) ? g : "0000")).join(" ");
  for (const three of [[0, 1, 2], [1, 3, 5], [3, 4, 5]]) {
    for (const set of K.confirmSets()) {
      assert.ok(!K.groupsMatchAt(groundOn(three), set, K.groupsAt(bea.join(" "), set)), "a key ground on three groups passes no ask: " + three + " passed the set " + set);
    }
  }
  const passing = K.confirmSets().filter((set) => K.groupsMatchAt(groundOn([0, 2, 3, 5]), set, K.groupsAt(bea.join(" "), set)));
  assert.deepStrictEqual(passing, [[0, 2, 3, 5]], "a key ground on four groups passes one set in 15");
});

// THE NAME RULE IS THE HONEST RELAY'S (the third review's M2; key_card.rs `honest_name`, relay.rs
// identify): 1 to 24 of [A-Za-z0-9_-]. A name with a fingerprint written into it is refused.
// Seen red 2026-11-21 with honestName allowing spaces and brackets: "Bea (3fa2 ...)" passed.
test("only a name an honest relay could list passes", () => {
  for (const ok of ["Bea", "a", "Cal_2", "x-y", "z".repeat(24)]) assert.ok(K.honestName(ok), ok);
  for (const bad of ["", "Bea (3fa2 91c0 77de 0b4e 5a19 c2d0)", "Bea ", "B\u00e9", "z".repeat(25), "a\u202eb", "a.b", null, 7]) {
    assert.ok(!K.honestName(bad), JSON.stringify(bad));
  }
  assert.strictEqual(K.NAME_MAX, 24);
  assert.ok(/pub const NAME_MAX: usize = 24;/.test(RUST), "the same limit as key_card.rs");
});

// THE APP'S AND THE SERVER'S NAMES ARE NOBODY'S (the owner branch's seventh review, H1 (b)): the
// same list as key_card.rs `RESERVED_NAMES`, folded the same way, so "System" and "Sy5tem" are
// refused names on both, and a name merely containing one is not.
// Seen red 2026-11-21 before reservedName existed: honestName("System") was true.
test("the reserved names are key_card.rs's, folded the same way", () => {
  const m = RUST.match(/pub const RESERVED_NAMES: &\[&str\] = &\[([^\]]*)\];/);
  assert.ok(m, "key_card.rs lists RESERVED_NAMES");
  assert.deepStrictEqual(K.RESERVED_NAMES, m[1].split(',').map((s) => s.trim().replace(/^"|"$/g, '')).filter(Boolean));
  for (const bad of ["System", "SYSTEM", "Sys_tem", "Sy5tem", "5y5t3m", "humanity-os", "Hurnanity0S", "From_this_server", "Fr0mThi5Server", "4pp"]) {
    assert.ok(K.reservedName(bad) && !K.honestName(bad), bad);
  }
  for (const ok of ["Systems", "Ada", "Serverly", "Apple", "Humanist", "SysAdmin"]) assert.ok(!K.reservedName(ok) && K.honestName(ok), ok);
});
