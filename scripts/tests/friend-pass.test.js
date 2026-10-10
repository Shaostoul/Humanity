// Friendship passes v2 (2026-10-09, docs/design/blocking-and-safe-mode.md 10b): the web
// builder says exactly what the relay checks.
//
// Run: node --test scripts/tests/friend-pass.test.js   (in `just rig-tests`)
//
// Pure node, nothing booted or connected, well under a second.
//
// Why it matters: a pass is signed by one client and checked by the relay, so the words the web
// client signs (web/shared/friend-pass.js, used by web/chat/crypto.js) must be byte for byte the
// relay's (src/relay/core/pq_crypto.rs `friend_cert_preimage`). Until v2 the web side built its
// string inline in crypto.js and nothing compared the two. This test reads the Rust test's PINNED
// preimage literal out of pq_crypto.rs and the protocol constants beside it, and holds the web
// builder to them; a change on either side fails here until the other follows. The signature path
// (a pass minted by Rust, verified with the vendored noble bundle over these words) is
// scripts/pq-kat.mjs (`just pq-kat`).
//
// Red first, 2026-10-09: with the server left out of friendPassPreimage's words, "the pinned
// preimage" failed with actual 'hum/friend/v2\nAABB\nCCDD\n0011..\nmessage,trade' against the
// Rust literal 'hum/friend/v2\ndid:hum:srv\nAABB\nCCDD\n0011..\nmessage,trade'. With "call"
// dropped from the web FRIEND_PASS_KINDS, "the protocol words are the relay's" failed. Restored,
// both pass.

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const REPO = path.join(__dirname, "..", "..");
const fp = require(path.join(REPO, "web", "shared", "friend-pass.js"));
const RUST = fs.readFileSync(path.join(REPO, "src", "relay", "core", "pq_crypto.rs"), "utf8");

const unescape = (s) => s.replace(/\\(.)/g, (_, c) => ({ n: "\n", t: "\t", r: "\r", "\\": "\\", '"': '"' })[c] ?? c);

/** A `pub const NAME: ... = <value>;` from pq_crypto.rs, as written. */
function rustConst(name) {
  const m = RUST.match(new RegExp(`pub const ${name}: [^=]+= ([^;]+);`));
  assert.ok(m, `pq_crypto.rs defines ${name}`);
  return m[1].trim();
}
const strList = (v) => [...v.matchAll(/"([^"]*)"/g)].map((m) => m[1]);
const num = (v) => Number(v.replace(/_/g, "").split("*").map((x) => x.trim()).reduce((a, b) => a * Number(b), 1));

test("the pinned preimage", () => {
  const pins = [...RUST.matchAll(/assert_eq!\(\s*friend_cert_preimage\(([^)]*)\)\s*,\s*"((?:[^"\\]|\\.)*)"\s*\)/g)];
  assert.ok(pins.length >= 1, "pq_crypto.rs pins friend_cert_preimage with a literal");
  for (const [, argText, expected] of pins) {
    // String literals only (a `may` holds commas, so the arguments are read as literals, not split).
    const args = [...argText.matchAll(/"((?:[^"\\]|\\.)*)"/g)].map((m) => unescape(m[1]));
    assert.equal(args.length, 5, `the pinned call's five arguments are literals: ${argText}`);
    assert.equal(fp.friendPassPreimage(...args), unescape(expected), `friendPassPreimage(${argText})`);
  }
});

test("the protocol words are the relay's", () => {
  assert.equal(fp.FRIEND_PASS_DOMAIN, unescape(rustConst("FRIEND_CERT_DOMAIN").slice(1, -1)));
  assert.equal(fp.FRIEND_PASS_VERSION, Number(rustConst("FRIEND_CERT_VERSION")));
  assert.deepEqual(fp.FRIEND_PASS_KINDS, strList(rustConst("FRIEND_PASS_KINDS")));
  assert.deepEqual(fp.FRIEND_PASS_DEFAULT_MAY, strList(rustConst("FRIEND_PASS_DEFAULT_MAY")));
  assert.equal(fp.FRIEND_PASS_SERIAL_BYTES, num(rustConst("FRIEND_CERT_SERIAL_BYTES")));
  assert.equal(fp.FRIEND_PASS_MAX_LEN, num(rustConst("FRIEND_CERT_MAX_LEN")));
  assert.ok(!fp.FRIEND_PASS_DEFAULT_MAY.includes("call"), "new friends cannot call until chosen to");
});

test("may has one spelling, as the relay reads it", () => {
  assert.equal(fp.friendPassMay(["trade", "message", "trade"]), "message,trade", "sorted, each kind once");
  assert.equal(fp.friendPassMay([]), null, "nothing allowed is no pass");
  assert.equal(fp.friendPassMay(["message", "shout"]), null, "a kind nobody knows");
  assert.equal(fp.friendPassMay(fp.FRIEND_PASS_DEFAULT_MAY), "invite,message,trade,voice_message");
});

test("the parser refuses what the relay refuses, and reads a pass the relay minted", () => {
  const fixture = JSON.parse(fs.readFileSync(path.join(REPO, "src", "relay", "core", "pq_kat_friend_pass.json"), "utf8"));
  const pass = fp.friendPassParse(fixture.cert);
  assert.ok(pass, "the Rust-minted pass parses");
  assert.equal(pass.serial, fixture.serial);
  assert.equal(pass.may, fixture.may);
  const v = JSON.parse(fixture.cert);
  const with_ = (k, val) => fp.friendPassParse(JSON.stringify({ ...v, [k]: val }));
  assert.ok(with_("serial", "ffeeddccbbaa99887766554433221100"), "any lowercase-hex serial of the right length");
  assert.equal(with_("serial", v.serial.toUpperCase()), null, "upper case serial");
  assert.equal(with_("serial", "0011"), null, "short serial");
  assert.equal(with_("may", "message,invite"), null, "out of order");
  assert.equal(with_("may", ""), null, "nothing allowed");
  assert.equal(with_("v", 1), null, "v1");
  assert.equal(fp.friendPassParse(v.sig), null, "a bare v1 signature");
  assert.equal(fp.friendPassParse("x".repeat(fp.FRIEND_PASS_MAX_LEN + 1)), null, "too long to read");
  // What the web side sends parses back to itself (key order does not matter to either side).
  assert.deepEqual(fp.friendPassParse(fp.friendPassJson(v.serial, v.may, v.sig)), { serial: v.serial, may: v.may, sig: v.sig });
  assert.ok(fp.friendPassFieldOk("did:hum:x") && !fp.friendPassFieldOk("") && !fp.friendPassFieldOk("a\nb"));
});

test("the chat client builds its words with the shared builder, loaded first", () => {
  const crypto = fs.readFileSync(path.join(REPO, "web", "chat", "crypto.js"), "utf8");
  assert.ok(!crypto.includes("hum/friend/"), "crypto.js keeps no copy of the pass words of its own");
  assert.ok(crypto.includes("friendPassPreimage(") && crypto.includes("friendPassParse("), "it uses the shared builder");
  const index = fs.readFileSync(path.join(REPO, "web", "chat", "index.html"), "utf8");
  const at = (s) => index.indexOf(s);
  assert.ok(at("/shared/friend-pass.js") > 0, "index.html loads friend-pass.js");
  assert.ok(at("/shared/friend-pass.js") < at("/chat/crypto.js"), "before crypto.js");
});
