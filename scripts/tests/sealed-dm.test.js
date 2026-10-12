// The rigs' sealed-sender DM (scripts/lib/sealed-dm.js) against the desktop app's (src/net/dm_pq.rs).
//
// Run: node --test scripts/tests/sealed-dm.test.js   (pure node, well under a second of work per
// test; nothing is booted or connected to)
//
// What this proves, in plain terms:
//  1. The domains, the padding buckets and the control markers are dm_pq.rs's own, read from its
//     source, so a change there fails here until the rigs follow.
//  2. A DM sealed here opens here, and only for its recipient; and its AES key is BLAKE3
//     derive_key under the domain READ FROM dm_pq.rs, checked by decrypting by hand. (A round trip
//     alone cannot catch a misspelt domain: both ends would share the mistake.)
//  3. The signed inner is padded, ordered and signed as dm_pq.rs `build_signed_inner_ext` does it.
//  4. A DM the game itself sealed (scripts/tests/fixtures/permit-note-v1.json, written by an
//     ignored Rust test of ship homes 5b's lane 1A) opens here, its signature checks out, and the
//     inner this file would write for the same message is the same bytes. Until that fixture is
//     merged, this test SKIPS and says so.

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const path = require("path");
const nodeCrypto = require("crypto");

const DM = require("../lib/sealed-dm.js");
const sp = require("../second-player.js");

const REPO = path.resolve(__dirname, "..", "..");
const DM_PQ_RS = fs.readFileSync(path.join(REPO, "src", "net", "dm_pq.rs"), "utf8");
// The game's own sealed permit note (lane 1A). SEALED_DM_FIXTURE points the test at another file
// of the same shape (how this lane saw test 4 red and green before 1A merged; see its comment).
const FIXTURE = process.env.SEALED_DM_FIXTURE || path.join(__dirname, "fixtures", "permit-note-v1.json");

/** A `const NAME: &str = "...";` (pub or not) out of the Rust source. */
const rustStr = (name) => {
  const m = DM_PQ_RS.match(new RegExp(`const ${name}: &str = "((?:[^"\\\\]|\\\\.)*)";`));
  assert.ok(m, `dm_pq.rs declares ${name}`);
  return m[1].replace(/\\(.)/g, "$1");
};

const master = (b, len = 32) => new Uint8Array(len).fill(b);

// Red check run 2026-10-12, restored afterwards: DM_PAD_BUCKETS in sealed-dm.js changed to
// [256, 1024, 4096, 8192]. FAILED at "the padding buckets are dm_pq.rs's DM_PAD_BUCKETS":
//   actual: [ 256, 1024, 4096, 8192 ], expected: [ 256, 1024, 4096, 16384 ]
test("the domains, the buckets and the markers are dm_pq.rs's, read from its source", () => {
  assert.equal(DM.DM_AES_DOMAIN, rustStr("DM_AES_DOMAIN"), "the AES key's domain is dm_pq.rs's DM_AES_DOMAIN");
  assert.equal(DM.DM_SIG_DOMAIN, rustStr("DM_SIG_DOMAIN"), "the signature's domain is dm_pq.rs's DM_SIG_DOMAIN");
  const buckets = DM_PQ_RS.match(/pub const DM_PAD_BUCKETS: \[usize; \d+\] = \[([^\]]*)\];/);
  assert.ok(buckets, "dm_pq.rs declares DM_PAD_BUCKETS");
  assert.deepEqual([...DM.DM_PAD_BUCKETS], buckets[1].split(",").map((s) => Number(s.trim())), "the padding buckets are dm_pq.rs's DM_PAD_BUCKETS");
  const overhead = DM_PQ_RS.match(/\.find\(\|b\| bare\.len\(\) \+ (\d+) <= \*b\)/);
  assert.ok(overhead, "dm_pq.rs picks a bucket with `bare.len() + N <= *b`");
  assert.equal(DM.DM_PAD_OVERHEAD, Number(overhead[1]), "the room left for the pad field is dm_pq.rs's");
  for (const name of ["CTL_FOLLOW", "CTL_UNFOLLOW", "CTL_FRIEND_CERT", "CTL_PERMIT"]) {
    assert.equal(DM[name], rustStr(name), `${name} is dm_pq.rs's`);
  }
});

// Red check run 2026-10-12, restored afterwards: DM_AES_DOMAIN in sealed-dm.js misspelt
// "hum/dm-aes/v2". The round trip still passed (both ends shared the mistake), and the test FAILED
// at the hand decryption under dm_pq.rs's own domain:
//   Error: Unsupported state or unable to authenticate data
test("a DM sealed here opens only for its recipient, under dm_pq.rs's own AES domain", async () => {
  const noble = await sp.loadNoble();
  const ada = sp.deriveIdentity(noble, master(11));
  const bo = sp.deriveIdentity(noble, master(22));
  const eve = sp.deriveIdentity(noble, master(99));
  const text = "hello from the walker, Gärtner \"quoted\"\nnew line";
  const env = DM.sealV2(noble, bo.kyberPublicB64, text);
  const j = JSON.parse(env);
  assert.deepEqual(Object.keys(j), ["ct_b64", "ek_ct_b64", "nonce_b64", "v"], "the wire envelope in serde_json's order, and no sender in it");
  assert.equal(j.v, 2);
  assert.equal(DM.openV2(noble, bo.kyberSecretKey, env), text, "its recipient opens it");
  assert.throws(() => DM.openV2(noble, eve.kyberSecretKey, env), /does not open/, "nobody else does");
  assert.throws(() => DM.openV2(noble, ada.kyberSecretKey, env), /does not open/, "not even its sender (who sealed it to Bo)");
  const body = Buffer.from(j.ct_b64, "base64");
  body[0] ^= 0xff;
  assert.throws(() => DM.open(noble, bo.kyberSecretKey, j.ek_ct_b64, j.nonce_b64, body.toString("base64")), /does not open/, "a changed body fails its tag");
  assert.throws(() => DM.openV2(noble, bo.kyberSecretKey, JSON.stringify({ v: 1, r: {}, s: {} })), /not a v2/, "the old v1 shape is refused");
  assert.throws(() => DM.seal(noble, Buffer.alloc(32).toString("base64"), "x"), /must be 1184 bytes/, "never sealed to a key of the wrong size");

  // By hand: ML-KEM-768 decapsulation, BLAKE3 derive_key under the domain dm_pq.rs declares,
  // AES-256-GCM with the tag at the end. Nothing from sealed-dm.js but the envelope.
  const ss = noble.ml_kem768.decapsulate(new Uint8Array(Buffer.from(j.ek_ct_b64, "base64")), bo.kyberSecretKey);
  const key = noble.blake3.create({ context: new TextEncoder().encode(rustStr("DM_AES_DOMAIN")), dkLen: 32 }).update(ss).digest();
  const sealed = Buffer.from(j.ct_b64, "base64");
  const d = nodeCrypto.createDecipheriv("aes-256-gcm", key, Buffer.from(j.nonce_b64, "base64"));
  d.setAuthTag(sealed.subarray(sealed.length - 16));
  const plain = Buffer.concat([d.update(sealed.subarray(0, sealed.length - 16)), d.final()]).toString("utf8");
  assert.equal(plain, text, "decrypted by hand under dm_pq.rs's DM_AES_DOMAIN");
  assert.equal(Buffer.from(j.nonce_b64, "base64").length, 12, "a 12-byte nonce");
  assert.notEqual(JSON.parse(DM.sealV2(noble, bo.kyberPublicB64, text)).ek_ct_b64, j.ek_ct_b64, "every seal encapsulates afresh");
});

// Red check run 2026-10-12, restored afterwards: DM_PAD_OVERHEAD in sealed-dm.js set to 9. FAILED at
// "the 16,384 bucket, less the 3 bytes the overhead allows over the pad field":
//   actual: 16384, expected: 16381
// (the walker padded 3 bytes further than dm_pq.rs does). The same break failed test 4 against the
// Rust stand-in (a padded [[hum:follow]] inner): "signedInner writes the game's inner byte for byte".
test("the signed inner is padded, ordered and signed as dm_pq.rs build_signed_inner_ext", async () => {
  const noble = await sp.loadNoble();
  const ada = sp.deriveIdentity(noble, master(11));
  const bo = sp.deriveIdentity(noble, master(22));
  const eve = sp.deriveIdentity(noble, master(66));
  const short = DM.signedInner(ada, { to: bo.publicKeyHex, ts: 1, text: "ok" });
  const medium = DM.signedInner(ada, { to: bo.publicKeyHex, ts: 2, text: "a considerably longer message with several clauses in it, the kind a length observer would love to distinguish" });
  assert.equal(Buffer.byteLength(short), Buffer.byteLength(medium), "short and medium land in the same bucket (dm_pq.rs v2_padding_hides_message_length)");
  assert.equal(Buffer.byteLength(short), 16384 - 3, "the 16,384 bucket, less the 3 bytes the overhead allows over the pad field");
  assert.deepEqual(Object.keys(JSON.parse(short)), ["from", "pad", "sig", "text", "to", "ts", "v"], "serde_json's sorted keys");
  const withCert = DM.signedInner(ada, { to: bo.publicKeyHex, ts: 3, text: DM.CTL_FRIEND_CERT, cert: '{"v":2}' });
  assert.deepEqual(Object.keys(JSON.parse(withCert)), ["cert", "from", "pad", "sig", "text", "to", "ts", "v"], "a pass rides in cert, sorted first");

  // Checked against the key it says it is from.
  const got = DM.verifyInner(noble, short);
  assert.deepEqual([got.from, got.to, got.ts, got.text, got.cert], [ada.publicKeyHex, bo.publicKeyHex, 1, "ok", null]);
  assert.equal(DM.verifyInner(noble, withCert).cert, '{"v":2}');
  const changed = (f) => {
    const v = JSON.parse(short);
    f(v);
    return JSON.stringify(v);
  };
  assert.throws(() => DM.verifyInner(noble, changed((v) => (v.text = "swapped"))), /does not check out/, "a changed text");
  assert.throws(() => DM.verifyInner(noble, changed((v) => (v.ts = 2))), /does not check out/, "a changed time");
  assert.throws(() => DM.verifyInner(noble, changed((v) => (v.to = eve.publicKeyHex))), /does not check out/, "a changed recipient");
  assert.throws(() => DM.verifyInner(noble, changed((v) => (v.v = 1))), /not a v2/);
  assert.throws(() => DM.verifyInner(noble, changed((v) => delete v.sig)), /missing/);
  const forged = { publicKeyHex: ada.publicKeyHex, sign: eve.sign };
  assert.throws(() => DM.verifyInner(noble, DM.signedInner(forged, { to: bo.publicKeyHex, ts: 4, text: "pretending to be Ada" })), /does not check out/, "Eve signing as Ada (dm_pq.rs v2_spoofed_sender_rejected)");

  // Every bucket, with a made-up signer whose signature length sets the bare inner's size. The
  // rule, from dm_pq.rs: the first bucket the bare inner fits with 12 bytes to spare, padded up to
  // it (so the whole is the bucket less 3); none past the last bucket.
  const sized = (sigBytes, text = "x") => {
    const id = { publicKeyHex: "ab", sign: () => new Uint8Array(sigBytes) };
    const inner = DM.signedInner(id, { to: "cd", ts: 5, text });
    const v = JSON.parse(inner);
    const bare = Buffer.byteLength(JSON.stringify({ from: v.from, sig: v.sig, text: v.text, to: v.to, ts: v.ts, v: v.v }));
    return { inner, bare, len: Buffer.byteLength(inner), pad: v.pad.length };
  };
  const seen = new Set();
  for (let n = 0; n < 13000; n += 13) {
    const { bare, len, pad } = sized(n);
    const bucket = DM.DM_PAD_BUCKETS.find((b) => bare + 12 <= b);
    if (bucket) {
      assert.equal(len, bucket - 3, `a bare inner of ${bare} bytes pads to the ${bucket} bucket`);
      seen.add(bucket);
    } else {
      assert.equal(pad, 0, `a bare inner of ${bare} bytes is past every bucket: no padding`);
      assert.equal(len, bare + 9, "only the empty pad field added");
      seen.add("none");
    }
  }
  assert.deepEqual([...seen].sort(), [1024, 16384, 256, 4096, "none"].sort(), "every bucket was reached, and past the last");
  // The edges, exactly: a bare inner of 244 bytes fits 256 with nothing to pad; 245 goes to 1,024.
  // (base64 grows in steps of 4, so the text's length makes up the rest.)
  let edge = null;
  for (let t = 1; t <= 8 && !edge; t++) for (let n = 0; n < 200 && !edge; n++) if (sized(n, "x".repeat(t)).bare === 244) edge = { n, text: "x".repeat(t) };
  assert.ok(edge, "a made-up inner of exactly 244 bare bytes");
  assert.equal(sized(edge.n, edge.text).len, 253, "a bare inner of 244 bytes fits the 256 bucket with an empty pad");
  assert.equal(sized(edge.n, `${edge.text}y`).len, 1021, "one more byte and it goes to the 1,024 bucket");
  // Bytes, not characters: dm_pq.rs measures `bare.len()`, UTF-8 bytes. One "ä" is one character
  // and two bytes, so this inner is 244 characters long and 245 bytes: the 1,024 bucket.
  const wide = sized(edge.n, `${edge.text.slice(1)}ä`);
  assert.equal(wide.bare, 245, "245 bytes bare (244 characters)");
  assert.equal(wide.len, 1021, "a text with a non-ASCII letter is measured in bytes, as Rust's len() does");
});

/** The recipient's master seed, the envelope, the note and the permit, from a sealed-note fixture.
 *  The names the game's fixture may use for each are tried in turn; one it does not hold fails the
 *  test (a fixture that cannot be read is a broken fixture, never a pass). */
function readSealedFixture(fx) {
  const pick = (...names) => names.map((n) => fx[n]).find((v) => v !== undefined && v !== null);
  const masters = [];
  const hexSeed = pick("recipient_master_hex", "recipient_seed_hex", "grantee_master_hex", "grantee_seed_hex");
  if (typeof hexSeed === "string") masters.push(new Uint8Array(Buffer.from(hexSeed, "hex")));
  const byte = pick("recipient_master_byte", "recipient_seed_byte", "grantee_master_byte", "grantee_seed_byte");
  if (Number.isInteger(byte)) masters.push(master(byte, 32), master(byte, 64));
  const env = pick("envelope", "sealed", "envelope_json", "sealed_envelope");
  const note = pick("note", "note_text", "text");
  let permit = pick("permit");
  if (typeof permit === "string") permit = JSON.parse(permit);
  assert.ok(masters.length, `the fixture names the recipient's seed (recipient_master_hex or recipient_master_byte); it holds: ${Object.keys(fx).join(", ")}`);
  assert.ok(env !== undefined, `the fixture holds the sealed envelope (envelope); it holds: ${Object.keys(fx).join(", ")}`);
  return { masters, envelope: typeof env === "string" ? env : JSON.stringify(env), note, permit };
}

// Red check run 2026-10-12 BEFORE lane 1A's fixture existed, against a stand-in of the same shape
// (SEALED_DM_FIXTURE): a permit note sealed by a scratch Rust program whose seal, padding and
// signing were copied from dm_pq.rs and pq_crypto.rs line for line (built with the crate versions
// in Cargo.lock), restored afterwards: DM_AES_DOMAIN in sealed-dm.js misspelt "hum/dm-aes/v2".
// FAILED at "the game's sealed note opens with the recipient's key": "no recipient seed in the
// fixture opens its envelope". Green with the domain restored: opened, signature checked, and
// signedInner rebuilt the 20,834-byte inner byte for byte (a short [[hum:follow]] inner, padded to
// 16,381, too). The same stand-in showed the other way: an inner the walker sealed opened and
// verified in that Rust program (`open_v2`, `parse_verify_inner`).
test("a DM the game sealed opens here, and its inner is the walker's byte for byte", async (t) => {
  if (!fs.existsSync(FIXTURE)) {
    t.skip(`${path.relative(REPO, FIXTURE)} is not here yet: lane 1A of ship homes 5b writes it (an ignored Rust test, from fixed seeds). Until it merges, the game-sealed DM is not checked by this file.`);
    return;
  }
  const noble = await sp.loadNoble();
  const fx = readSealedFixture(JSON.parse(fs.readFileSync(FIXTURE, "utf8")));
  let opened = null;
  let me = null;
  for (const m of fx.masters) {
    const id = sp.deriveIdentity(noble, m);
    try {
      opened = DM.openV2(noble, id.kyberSecretKey, fx.envelope);
      me = id;
      break;
    } catch {}
  }
  assert.ok(opened !== null, "the game's sealed note opens with the recipient's key: no recipient seed in the fixture opens its envelope");
  const inner = DM.verifyInner(noble, opened);
  assert.equal(inner.to, me.publicKeyHex, "it is addressed to the recipient");
  if (fx.note !== undefined) assert.equal(inner.text, fx.note, "it carries the fixture's note");
  if (DM.CTL_PERMIT && inner.text.startsWith(DM.CTL_PERMIT)) {
    const permit = sp.parsePermitNote(inner.text);
    assert.ok(permit, "the note reads as a permit");
    assert.equal(permit.issuer, inner.from, "given by its sender");
    if (fx.permit) assert.deepEqual(permit, sp.parsePermitJson(JSON.stringify(fx.permit)), "the fixture's permit");
  }
  // The same message written here, with the game's own signature: the same bytes.
  const sigOf = { publicKeyHex: inner.from, sign: () => new Uint8Array(Buffer.from(inner.sig, "base64")) };
  assert.equal(DM.signedInner(sigOf, { to: inner.to, ts: inner.ts, text: inner.text, cert: inner.cert }), opened, "signedInner writes the game's inner byte for byte");
});
