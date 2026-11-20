// BUG-328: the website pads a sealed DM to its size bucket in UTF-8 BYTES, as the desktop does
// (src/net/dm_pq.rs build_signed_inner_ext: `bare.len() + 12 <= bucket`, bytes), so a message's
// sealed size says nothing about how much of it is not ASCII (2026-11-20).
//
// Run: node --test scripts/tests/dm-padding-web.test.js   (discovered by `just rig-tests`)
//
// Until 2026-11-20 the website measured `JSON.stringify(inner).length`, UTF-16 code units, so a
// non-ASCII message overshot its bucket by the bytes beyond one per character: found by the
// sealed-sender envelope fixture (scripts/tests/fixtures/dm-envelope-v2.json), whose text is not
// ASCII. Three website sealers pad a DM, and each is run here:
//   - web/chat/crypto.js pqBuildDmPuts (Chat; the real page code with the vendored bundle,
//     scripts/lib/chat-pq-page.js),
//   - web/shared/mine-tasks.js sealSelfNote (notes to myself from the Tasks page),
//   - web/pages/orgs-app.js sealedPut (organisation changes to each member; a page script with no
//     exports, so its function is taken from the file and run with recorded signing and sealing).
// For each, an ASCII, a mixed and a CJK text of the same shape must seal to the same number of
// bytes, the size the desktop's rule gives.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { REPO, keysOf, chatPage } = require("../lib/chat-pq-page.js");

const WEB = process.env.HOS_WEB_DIR || path.join(REPO, "web");
const BUCKETS = [256, 1024, 4096, 16384, 32768]; // dm_pq.rs DM_PAD_BUCKETS (reach-web.test.js holds the copies to it)
const bytes = (s) => Buffer.byteLength(s, "utf8");

/** The desktop's rule (dm_pq.rs): the padded inner's byte length for a bare inner of `bare` bytes. */
function desktopLength(bare) {
  const bucket = BUCKETS.find((b) => bare + 12 <= b) ?? bare + 12;
  return bare + 9 + Math.max(0, bucket - bare - 12); // 9 = the bytes of `,"pad":""`
}

/** Three texts whose sealed size must not tell them apart. */
const TEXTS = [
  "cross-check: plain ASCII text here, nothing else at all",
  "cross-check: Grüße, café, 日本語, \u{1f30d} and more",
  "日".repeat(400),
];

/** The bare inner payload's byte length (without its pad), for the desktop's rule. */
function bareBytesOf(innerJson) {
  const inner = JSON.parse(innerJson);
  delete inner.pad;
  return bytes(JSON.stringify(inner));
}

function assertPadded(sealedPlaintexts, who) {
  for (const p of sealedPlaintexts) {
    assert.strictEqual(bytes(p), desktopLength(bareBytesOf(p)), `${who}: padded to the desktop's size in bytes (${bytes(p)} bytes for "${JSON.parse(p).text.slice(0, 24)}")`);
  }
  assert.strictEqual(new Set(sealedPlaintexts.map(bytes)).size, 1, `${who}: ASCII and non-ASCII texts seal to the same size`);
}

// Seen red 2026-11-20 before the fix: "Chat's pqBuildDmPuts: padded to the desktop's size in bytes
// (16392 bytes for ...)" failed (16,381 expected).
test("Chat (crypto.js pqBuildDmPuts) pads by UTF-8 bytes", async () => {
  const rec = await keysOf(22);
  const page = await chatPage(11, { peerKyber: rec.kyber });
  const sealed = [];
  for (const text of TEXTS) {
    const built = await page.fn("pqBuildDmPuts")(text, rec.key, 1763500000000);
    assert.ok(built, "built");
    sealed.push(JSON.stringify(built.inner)); // exactly the JSON pqBuildDmPuts sealed
    // And the ciphertext is that many bytes plus AES-GCM's 16-byte tag.
    assert.strictEqual(Buffer.from(JSON.parse(built.recipientPut.content).ct_b64, "base64").length, bytes(sealed[sealed.length - 1]) + 16);
  }
  assertPadded(sealed, "Chat's pqBuildDmPuts");
});

// Seen red 2026-11-20 before the fix: "the Tasks page's sealSelfNote: padded to the desktop's size
// in bytes" failed.
test("Tasks (mine-tasks.js sealSelfNote) pads by UTF-8 bytes", async () => {
  const M = require(path.join(WEB, "shared", "mine-tasks.js"));
  const sealed = [];
  const saved = { sign: globalThis.pqSignMessage, seal: globalThis.pqDmSeal };
  globalThis.pqSignMessage = async () => new Uint8Array(3309).fill(7);
  globalThis.pqDmSeal = async (_pub, plaintext) => { sealed.push(plaintext); return { ek_ct_b64: "AA==", nonce_b64: "AA==", ct_b64: "AA==" }; };
  try {
    const me = (await keysOf(11)).key;
    for (const text of TEXTS) assert.ok(await M.sealSelfNote(text, 1763500000000, { me, dilithiumSecret: new Uint8Array(1), kyberPublicB64: "AA==" }), "sealed");
  } finally {
    globalThis.pqSignMessage = saved.sign;
    globalThis.pqDmSeal = saved.seal;
  }
  assertPadded(sealed, "the Tasks page's sealSelfNote");
});

/** orgs-app.js's sealedPut, taken from the file as written (from its line to its closing brace). */
function orgsSealedPut(ctx) {
  const src = fs.readFileSync(path.join(WEB, "pages", "orgs-app.js"), "utf8").replace(/\r\n/g, "\n");
  const start = src.indexOf("  async function sealedPut(");
  assert.ok(start >= 0, "orgs-app.js has sealedPut");
  const end = src.indexOf("\n  }\n", start);
  assert.ok(end > start, "and its end");
  vm.createContext(ctx);
  return vm.runInContext(`(${src.slice(start, end + 4).trim()})`, ctx);
}

// Seen red 2026-11-20 before the fix: "the Organisations page's sealedPut: padded to the desktop's
// size in bytes" failed.
test("Organisations (orgs-app.js sealedPut) pads by UTF-8 bytes", async () => {
  const sealed = [];
  const ctx = {
    TextEncoder,
    btoa: globalThis.btoa,
    b64: (u8) => Buffer.from(u8).toString("base64"),
    window: {
      pqSignMessage: async () => new Uint8Array(3309).fill(7),
      pqDmSeal: async (_pub, plaintext) => { sealed.push(plaintext); return { ek_ct_b64: "AA==", nonce_b64: "AA==", ct_b64: "AA==" }; },
    },
  };
  ctx.Date = Date;
  ctx.JSON = JSON;
  const sealedPut = orgsSealedPut(ctx);
  const me = (await keysOf(11)).key;
  const to = (await keysOf(22)).key;
  for (const text of TEXTS) assert.ok(await sealedPut({ id: { dilithiumSecret: new Uint8Array(1) }, hex: me }, to, "AA==", text), "sealed");
  assertPadded(sealed, "the Organisations page's sealedPut");
});

// Seen red 2026-11-20 on the fixture written before the fix: "the website's envelope is the
// desktop's size for the same text" failed (16408 against 16397 ciphertext bytes).
test("the shared envelope fixture: the website's envelope is the desktop's size for the same text", () => {
  const fx = JSON.parse(fs.readFileSync(path.join(REPO, "scripts", "tests", "fixtures", "dm-envelope-v2.json"), "utf8"));
  assert.strictEqual(fx.web.text, fx.rust.text, "the same text");
  const ct = (env) => Buffer.from(JSON.parse(env).ct_b64, "base64").length;
  assert.strictEqual(ct(fx.web.envelope), ct(fx.rust.envelope), "the website's envelope is the desktop's size for the same text");
});
