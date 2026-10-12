// The copy of a person's profile that other servers keep, the website's half (BUG-191, 2026-10-12,
// docs/design/bug-191-profile-copies-plan.md 3.1 to 3.4, 3.11 and Wave 1 lane 1D):
// web/shared/profile-copy.js (the public copy, the words signed, the limits, the time on a copy)
// and web/chat/chat-profile.js signProfileCopy (signs when the server asks, sends `profile_sign`).
//
// Run: node --test scripts/tests/profile-copy-web.test.js   (in `just rig-tests`)
//
// Pure node, nothing booted or connected. The relay's twin is src/relay/core/profile_copy.rs, and
// the shared fixture scripts/tests/fixtures/profile-copy-v1.json is made by that builder from the
// seed [7; 32] (ML-DSA signing in Rust is deterministic). The words and the limits must agree byte
// for byte, or the relay refuses every copy the website signs. So this test READS the Rust
// constants and the pinned words out of profile_copy.rs (the friend-pass.test.js way) instead of
// copying them, and checks the fixture's Rust signatures over the WEBSITE's words with the
// vendored noble bundle. Noble's signing is hedged (randomized), so nothing compares signature
// bytes: a web signature is checked by verifying it.
//
// The chat half runs in a small node:vm page: the real profile-changes.js, profile-copy.js and
// chat-profile.js, with `ws`, `document` and `localStorage` stood in, and `pqSignMessage` the real
// Dilithium3 of the vendored bundle under the fixture's key (derived from [7; 32] the way pq.js
// does). HOS_WEB_DIR points the test at another copy of web/ (used to see each test red).
//
// Each test names, in a comment above it, the deliberate break it was seen red with (2026-10-12)
// and what failed; each passed again once the break was taken out.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const REPO = path.join(__dirname, "..", "..");
const WEB = process.env.HOS_WEB_DIR || path.join(REPO, "web");
const PC = require(path.join(WEB, "shared", "profile-copy.js"));
const RUST = fs.readFileSync(path.join(REPO, "src", "relay", "core", "profile_copy.rs"), "utf8");
const PQ_RUST = fs.readFileSync(path.join(REPO, "src", "relay", "core", "pq_crypto.rs"), "utf8");
const FX = JSON.parse(fs.readFileSync(path.join(REPO, "scripts", "tests", "fixtures", "profile-copy-v1.json"), "utf8"));

const enc = (s) => new TextEncoder().encode(s);
const unhex = (h) => Uint8Array.from(Buffer.from(h, "hex"));
const clone = (x) => JSON.parse(JSON.stringify(x));
const unescape = (s) => s.replace(/\\(.)/g, (_, c) => ({ n: "\n", t: "\t", r: "\r", "\\": "\\", '"': '"', "0": "\0" })[c] ?? c);

// ── The vendored noble bundle (an ES module: imported as a data: URL, as the other web tests do) ──

let NOBLE = null;
async function noble() {
  if (NOBLE) return NOBLE;
  const src = fs.readFileSync(path.join(REPO, "web", "shared", "vendor", "noble-pq.bundle.js"));
  NOBLE = await import("data:text/javascript;base64," + src.toString("base64"));
  return NOBLE;
}

/** The fixture person's Dilithium3 keypair, derived from [signer_master_byte; 32] exactly as pq.js pqDeriveIdentity does. */
let SIGNER = null;
async function signer() {
  if (SIGNER) return SIGNER;
  const m = await noble();
  const seed = new Uint8Array(32).fill(FX.signer_master_byte);
  const dilSeed = m.blake3.create({ context: enc("hum/dilithium3/v1"), dkLen: 32 }).update(seed).digest();
  const kp = m.ml_dsa65.keygen(dilSeed);
  SIGNER = { pub: kp.publicKey, secret: kp.secretKey, key: Buffer.from(kp.publicKey).toString("hex") };
  return SIGNER;
}

async function verifies(key, text, sigHex) {
  const m = await noble();
  try { return !!m.ml_dsa65.verify(unhex(sigHex), enc(text), unhex(key)); } catch (e) { return false; }
}

// ── Reading profile_copy.rs ──

/** A `pub const NAME: ... = <value>;` from a Rust file, as written. */
function rustConst(src, name) {
  const m = src.match(new RegExp(`pub const ${name}: [^=]+= ([^;]+);`));
  assert.ok(m, `the Rust file defines ${name}`);
  return m[1].trim();
}
const strList = (v) => [...v.matchAll(/"([^"]*)"/g)].map((m) => m[1]);
/** A Rust constant's number: products of numbers and other constants (profile_copy.rs, then pq_crypto.rs). */
function rustNumber(name) {
  const src = new RegExp(`pub const ${name}:`).test(RUST) ? RUST : PQ_RUST;
  return rustConst(src, name).split("*").map((t) => t.trim()).reduce((acc, t) => {
    const n = /^[0-9_]+$/.test(t) ? Number(t.replace(/_/g, "")) : rustNumber(t);
    return acc * n;
  }, 1);
}

// Seen red with "website" dropped from the web FIELDS_FOR_FRIENDS: "the fields a person may keep
// for friends" failed ([ 'pronouns', 'location' ] against the Rust list). With BIO_MAX_CHARS at
// 300 on the web side, "BIO_MAX_CHARS" failed (300 against 280).
test("the fields kept for friends, the tag and every limit are the relay's", () => {
  assert.deepEqual(PC.FIELDS_FOR_FRIENDS, strList(rustConst(RUST, "FIELDS_FOR_FRIENDS")), "the fields a person may keep for friends");
  assert.equal(PC.COPY_TAG, unescape(rustConst(RUST, "COPY_TAG").slice(1, -1)), "the tag");
  for (const name of [
    "BIO_MAX_CHARS", "NAME_MAX_BYTES", "LINKS_MAX_BYTES", "PICTURE_LINK_MAX_BYTES", "WEBSITE_MAX_BYTES",
    "PRONOUNS_MAX_CHARS", "LOCATION_MAX_CHARS", "KEY_HEX_LEN", "SIGNATURE_HEX_LEN", "MAX_AHEAD_MS",
  ]) {
    assert.equal(PC[name], rustNumber(name), name);
  }
  assert.equal(PC.KEY_HEX_LEN, FX.public_key.length, "the fixture's key has the key's length");
});

// Seen red with location and website swapped in copyText: "copyText(\"ab12\", PINNED_COPY,
// 1760000000001)" failed, actual '...\nshe/her\nhttps://ann.example\nLisbon\n1760000000001'
// against the Rust literal '...\nshe/her\nLisbon\nhttps://ann.example\n1760000000001'.
test("the signed words are the relay's pinned words", () => {
  const copyM = RUST.match(/const PINNED_COPY: &str = r#"(.*?)"#;/s);
  assert.ok(copyM, "profile_copy.rs pins a copy (PINNED_COPY)");
  const copy = JSON.parse(copyM[1]);
  const pins = [...RUST.matchAll(/assert_eq!\(\s*copy_text\(\s*"((?:[^"\\]|\\.)*)"\s*,\s*&c\s*,\s*([0-9_]+)\s*\)\s*,\s*"((?:[^"\\]|\\.)*)"\s*\)/g)];
  assert.equal(pins.length, 1, "profile_copy.rs pins copy_text with one literal");
  for (const [, key, at, expected] of pins) {
    const words = PC.copyText(unescape(key), copy, Number(at.replace(/_/g, "")));
    assert.equal(words, unescape(expected), `copyText("${key}", PINNED_COPY, ${at.replace(/_/g, "")})`);
    assert.ok(words.includes("Γεια!\nline two"), "the bio's own line break and its Greek are kept");
  }
});

// Seen red with keptForFriends reading "friends" instead of "private": "the copy of case 1"
// failed (location 'Lisbon' against ''), the deepEqual reporting the location kept for friends in
// the copy for other servers.
test("the fixture's copies and words come out of the website's builder, and the Rust signatures verify over them", async () => {
  const me = await signer();
  assert.equal(me.key, FX.public_key, "pq.js's derivation from [7; 32] gives the fixture's key");
  assert.equal(FX.cases.length, 4);
  for (const [i, c] of FX.cases.entries()) {
    const n = i + 1;
    const copy = PC.publicCopy(c.frame.name, c.frame, c.frame.privacy);
    assert.deepEqual(copy, c.copy, `the copy of case ${n} (${c.label})`);
    assert.equal(PC.nextCopyTime(c.now_ms, c.frame.signed_at, 0), c.at, `the time of case ${n}`);
    const words = PC.copyText(FX.public_key, copy, c.at);
    assert.equal(words, c.text, `the words of case ${n}`);
    assert.ok(PC.withinLimits(FX.public_key, copy), `case ${n} is within the limits`);
    assert.ok(PC.isSignatureText(c.signature), `case ${n}'s signature has the shape of one`);
    assert.ok(await verifies(FX.public_key, words, c.signature), `the Rust signature of case ${n} verifies over the website's words`);
    const changed = words.replace(/\d$/, (d) => String((Number(d) + 1) % 10));
    assert.ok(!(await verifies(FX.public_key, changed, c.signature)), `case ${n}: one changed character and it does not`);
  }
  // The copies say what the privacy rule says, in plain view.
  assert.equal(FX.cases[0].copy.location, "", "case 1: the location kept for friends is blank");
  assert.equal(FX.cases[0].copy.pronouns, "she/her", "case 1: pronouns marked public stay");
  assert.ok(PC.isWithdrawal(FX.cases[1].copy) && PC.unlisted(FX.cases[1].frame.privacy), "case 2: unlisted is the withdrawal");
  assert.equal(FX.cases[2].copy.socials, "{}", "case 3: links stored blank read as {}");
});

// Seen red with `===` turned to `==` in keptForFriends: "a location whose value is a list is
// public" failed (a list holding "private" compares equal to the word under ==; Rust's as_str
// says it is not a word).
test("the privacy map is read one way: only the word \"private\" keeps a field, only \"unlisted\" withdraws", () => {
  const p = { bio: "hi", avatar_url: "https://ann.example/a.png", socials: '{"github":"ann"}', pronouns: "she/her", location: "Lisbon", website: "https://ann.example" };
  const copy = (privacy) => PC.publicCopy("Ann", p, privacy);
  assert.equal(copy('{"location":"private","since":2019}').location, "", "a map holding a number still keeps the location");
  assert.equal(copy('{"location":"friends"}').location, "Lisbon", "only the word \"private\" keeps a field");
  assert.equal(copy('{"location":["private"]}').location, "Lisbon", "a location whose value is a list is public");
  assert.equal(copy('{"location":"PRIVATE"}').location, "Lisbon", "the word in its own case only");
  assert.deepEqual(copy('{"pronouns":"private","location":"private","website":"private"}'),
    { name: "Ann", bio: "hi", avatar_url: "https://ann.example/a.png", banner_url: "", socials: '{"github":"ann"}', pronouns: "", location: "", website: "" },
    "all three kept for friends; name, bio, pictures and links always");
  assert.deepEqual(copy('{"directory":"unlisted","location":"public"}'), PC.withdrawal(), "unlisted is the withdrawal, whatever else the map says");
  assert.equal(copy('{"directory":"listed"}').name, "Ann", "any other directory word lists");
  assert.equal(copy('{"directory":["unlisted"]}').name, "Ann", "a directory that is not a word lists");
  for (const notAMap of ["[]", '["private"]', "null", "3", '"private"', "{", "", undefined, null]) {
    const c = copy(notAMap);
    assert.equal(c.location, "Lisbon", `a map that is not an object (${notAMap}) keeps nothing for friends`);
    assert.equal(c.name, "Ann", `and lists the person (${notAMap})`);
  }
  assert.ok(!PC.keptForFriends('{"constructor":"x"}', "constructor") && !PC.unlisted("{}"), "nothing inherited counts");
  // Nothing is trimmed or re-serialized: the copy is what the server stored.
  const raw = PC.publicCopy("Ann", { bio: " two  spaces ", socials: '{ "b":"1", "a":"2" }', location: " Lisbon " }, "{}");
  assert.equal(raw.bio, " two  spaces ");
  assert.equal(raw.socials, '{ "b":"1", "a":"2" }', "the links' own text and key order");
  assert.equal(raw.location, " Lisbon ");
  assert.deepEqual([raw.avatar_url, raw.banner_url, raw.pronouns, raw.website], ["", "", "", ""], "an absent field is blank");
  assert.equal(PC.publicCopy("Ann", { socials: "" }, "{}").socials, "{}", "links stored blank read as {}");
});

// Seen red with chars() counting JS .length: "a bio of 280 characters outside the BMP is within"
// failed (.length 560). With utf8Bytes() counting .length: "an avatar of 513 UTF-8 bytes is
// over" failed (its .length is 266). With the line-break rule taken out: "a location holding a
// line break is over" failed.
test("the limits are counted the way the relay counts them", () => {
  const key = "ab".repeat(PC.KEY_HEX_LEN / 2);
  const base = { name: "Ann", bio: "", avatar_url: "", banner_url: "", socials: "{}", pronouns: "", location: "", website: "" };
  const ok = (over) => PC.withinLimits(key, Object.assign({}, base, over));
  assert.ok(ok({}), "a plain copy is within");
  // Characters (code points): the bio, pronouns and location.
  const clef = "\u{1D11E}"; // one character, two JS code units, four UTF-8 bytes
  assert.ok(ok({ bio: clef.repeat(280) }), "a bio of 280 characters outside the BMP is within");
  assert.ok(!ok({ bio: clef.repeat(281) }), "281 is over");
  assert.ok(ok({ pronouns: "é".repeat(64) }) && !ok({ pronouns: "é".repeat(65) }), "pronouns: 64 characters");
  assert.ok(ok({ location: clef.repeat(128) }) && !ok({ location: clef.repeat(129) }), "location: 128 characters");
  assert.ok(!ok({ bio: "<3" }) && !ok({ bio: "a > b" }), "the bio holds no < or >");
  assert.ok(ok({ bio: "line one\nline two" }), "the bio may hold a line break");
  // UTF-8 bytes: the links, avatar, banner and website.
  const link = (bytes) => { const head = "https://x.example/"; const n = (bytes - head.length) / 2; return head + "é".repeat(n); };
  assert.ok(ok({ avatar_url: link(512) }), "an avatar of 512 UTF-8 bytes is within");
  assert.ok(!ok({ avatar_url: link(512) + "a" }), "an avatar of 513 UTF-8 bytes is over");
  assert.ok(ok({ banner_url: link(512) }) && !ok({ banner_url: link(512) + "a" }), "the banner too");
  assert.ok(ok({ website: link(256) }) && !ok({ website: link(256) + "a" }), "the website: 256 bytes");
  const links = (bytes) => { const head = '{"a":"'; const tail = '"}'; return head + "é".repeat((bytes - head.length - tail.length) / 2) + tail; };
  assert.ok(ok({ socials: links(1024) }) && !ok({ socials: links(1024).replace('"a"', '"ab"') }), "the links: 1,024 bytes");
  assert.ok(!ok({ socials: "[]" }) && !ok({ socials: "" }) && !ok({ socials: "{" }), "the links are a JSON object");
  assert.ok(!ok({ avatar_url: "http://ann.example/a.png" }) && !ok({ website: "ann.example" }), "links start with https://");
  // Line breaks: only "\n", and only outside the bio.
  assert.ok(!ok({ location: "Lis\nbon" }), "a location holding a line break is over");
  for (const f of ["pronouns", "avatar_url", "banner_url", "website"]) {
    const v = f === "pronouns" ? "she\nher" : "https://ann.example/\n";
    assert.ok(!ok({ [f]: v }), `a line break in ${f}`);
  }
  assert.ok(!ok({ socials: '{"a":\n"b"}' }), "a line break in the links' text");
  assert.ok(ok({ location: "Lis\rbon" }), "a carriage return alone is not a line break to the relay");
  // The name, and the withdrawal.
  assert.ok(ok({ name: "Ann-1_x" }) && ok({ name: "a".repeat(24) }), "a name of letters, digits, _ and -, up to 24");
  assert.ok(!ok({ name: "a".repeat(25) }) && !ok({ name: "Ann B" }) && !ok({ name: "Änn" }), "nothing else");
  assert.ok(PC.withinLimits(key, PC.withdrawal()), "the withdrawal is within");
  assert.ok(!ok({ name: "", bio: "left over" }) && !ok({ name: "", socials: "" }), "a withdrawal holds nothing else");
  // Keys and signatures: lowercase hex of their length.
  assert.ok(!PC.withinLimits(key.toUpperCase(), base) && !PC.withinLimits(key.slice(2), base), "the key: 3,904 lowercase hex");
  assert.ok(PC.isKeyText(FX.public_key) && !PC.isKeyText(FX.public_key.toUpperCase()));
  assert.ok(!PC.isSignatureText(FX.cases[0].signature.toUpperCase()) && !PC.isSignatureText(FX.cases[0].signature.slice(1)));
});

// Seen red with nextCopyTime returning nowMs alone: "row 2" failed (1000 against 5001).
test("each copy is dated after the clock, the copy held and the last one signed here", () => {
  assert.ok(FX.next_copy_time.length >= 7);
  for (const [i, r] of FX.next_copy_time.entries()) {
    assert.equal(PC.nextCopyTime(r.now_ms, r.signed_at, r.last_here), r.at, `row ${i + 1}: ${JSON.stringify(r)}`);
  }
  assert.equal(PC.nextCopyTime(1000, undefined, undefined), 1000, "nothing held and nothing signed here read as 0");
  assert.equal(PC.nextCopyTime(1000, null, "5000"), 1000, "and so does anything that is not a number");
});

// ── The chat page (node:vm) ──

/** A chat page with the three scripts, signed in as the fixture's person on an open socket. */
async function page() {
  const m = await noble();
  const me = await signer();
  const sent = [];
  const warned = [];
  const store = new Map();
  const ctx = {
    console: { log() {}, info() {}, warn: (...a) => warned.push(a.join(" ")), error: (...a) => warned.push(a.join(" ")) },
    document: { getElementById: () => null },
    localStorage: { getItem: (k) => (store.has(k) ? store.get(k) : null), setItem: (k, v) => store.set(k, String(v)), removeItem: (k) => store.delete(k) },
    WebSocket: { CONNECTING: 0, OPEN: 1, CLOSING: 2, CLOSED: 3 },
    TextEncoder,
    setTimeout,
    clearTimeout,
    addEventListener() {},
    myKey: me.key,
    myName: "Ann",
    myDilithiumSecret: me.secret,
    pqSignMessage: async (secret, bytes) => m.ml_dsa65.sign(bytes, secret),
  };
  ctx.window = ctx;
  ctx.ws = { readyState: 1, send: (s) => sent.push(JSON.parse(s)) };
  vm.createContext(ctx);
  for (const rel of ["shared/profile-changes.js", "shared/profile-copy.js", "chat/chat-profile.js"]) {
    vm.runInContext(fs.readFileSync(path.join(WEB, rel), "utf8"), ctx, { filename: rel });
  }
  return { ctx, sent, warned, me };
}

/** Let the page's signing (a promise chain) finish. */
async function settle() {
  for (let i = 0; i < 5; i++) await new Promise((r) => setImmediate(r));
}

/** A fixture case's own-copy frame, with the copy the server holds dated `aheadMs` past now. */
function frameOf(i, aheadMs) {
  const f = clone(FX.cases[i].frame);
  f.signed_at = Date.now() + aheadMs;
  return f;
}

// Seen red with chat-profile.js building the copy without the map (publicCopy(msg.name, msg,
// '{}')): "case 1: the signature covers the copy for other servers" failed (it covered the
// location, Lisbon, kept for friends). With the time taken from the clock alone
// (nextCopyTime(Date.now(), 0, ...)): "case 1: dated after the copy the server holds" failed.
test("an own copy that asks is signed once, dated after the copy the server holds, and the signature verifies", async () => {
  for (const i of FX.cases.keys()) {
    const n = i + 1;
    const { ctx, sent, me } = await page();
    const frame = frameOf(i, 60_000); // the copy held is a minute ahead of this device's clock
    ctx.ownProfileArrived(frame);
    await settle();
    assert.equal(sent.length, 1, `case ${n}: exactly one frame went`);
    const out = sent[0];
    assert.deepEqual(Object.keys(out).sort(), ["at", "signature", "type"], `case ${n}: the frame is {type, at, signature}`);
    assert.equal(out.type, "profile_sign");
    assert.equal(out.at, frame.signed_at + 1, `case ${n}: dated after the copy the server holds`);
    assert.ok(Number.isSafeInteger(out.at), `case ${n}: at is a whole number of milliseconds`);
    assert.ok(PC.isSignatureText(out.signature), `case ${n}: 6,618 lowercase hex characters`);
    const words = PC.copyText(me.key, FX.cases[i].copy, out.at);
    assert.ok(await verifies(me.key, words, out.signature), `case ${n}: the signature covers the copy for other servers`);
  }
});

// Seen red with the `msg.sign_now !== true` check taken out of signProfileCopy: "signed a copy
// nobody asked for (sign_now absent)" failed (one profile_sign went). With the privacy check taken
// out of signProfileCopy: "the public copy of a save is never signed" failed. With the target
// check taken out: "a copy routed to another of my devices" failed. With the socket check after
// the wait taken out: "the socket changed while signing" failed (it went on the new socket).
test("nothing is sent unless the server asks, for my own copy, on the socket it asked on", async () => {
  const cases = [
    ["sign_now absent", (f) => { delete f.sign_now; }],
    ["sign_now false", (f) => { f.sign_now = false; }],
    ["sign_now the text \"true\"", (f) => { f.sign_now = "true"; }],
    ["sign_now 1", (f) => { f.sign_now = 1; }],
  ];
  for (const [label, edit] of cases) {
    const { ctx, sent } = await page();
    const f = frameOf(0, 60_000);
    edit(f);
    ctx.ownProfileArrived(f);
    assert.equal(await ctx.signProfileCopy(f), false, `signed a copy nobody asked for (${label})`);
    await settle();
    assert.deepEqual(sent, [], `signed a copy nobody asked for (${label})`);
  }
  {
    // The public copy of a save names me too, but carries no privacy map: it is not mine to sign.
    const { ctx, sent } = await page();
    const f = frameOf(0, 60_000);
    delete f.privacy;
    ctx.ownProfileArrived(f);
    assert.equal(await ctx.signProfileCopy(f), false, "the public copy of a save is never signed");
    await settle();
    assert.deepEqual(sent, [], "the public copy of a save is never signed");
  }
  {
    const { ctx, sent } = await page();
    const f = frameOf(0, 60_000);
    f.target = "ab".repeat(PC.KEY_HEX_LEN / 2);
    ctx.ownProfileArrived(f);
    await settle();
    assert.deepEqual(sent, [], "a copy routed to another of my devices");
  }
  {
    const { ctx, sent } = await page();
    ctx.myDilithiumSecret = null;
    ctx.ownProfileArrived(frameOf(0, 60_000));
    await settle();
    assert.deepEqual(sent, [], "no identity to sign with");
  }
  {
    const { ctx, sent } = await page();
    ctx.ws.readyState = 3;
    ctx.ownProfileArrived(frameOf(0, 60_000));
    await settle();
    assert.deepEqual(sent, [], "the socket is closed");
  }
  {
    const { ctx, sent } = await page();
    const sign = ctx.pqSignMessage;
    const later = [];
    ctx.pqSignMessage = async (secret, bytes) => {
      ctx.ws = { readyState: 1, send: (s) => later.push(JSON.parse(s)) }; // a reconnect while signing
      return sign(secret, bytes);
    };
    ctx.ownProfileArrived(frameOf(0, 60_000));
    await settle();
    assert.deepEqual([sent, later], [[], []], "the socket changed while signing");
  }
  {
    const { ctx, sent } = await page();
    const f = frameOf(0, 60_000);
    f.location = "x".repeat(PC.LOCATION_MAX_CHARS + 1); // over the limits: the server never asks for this
    f.privacy = "{}";
    ctx.ownProfileArrived(f);
    await settle();
    assert.deepEqual(sent, [], "a copy over the limits is not signed");
  }
});

// Seen red with profileCopyLastAt not recorded: "the second ask on this server is dated after
// the first" failed (the second carried the first's time again).
test("a second ask is dated after the first on that server, and another server keeps its own time", async () => {
  const { ctx, sent, me } = await page();
  ctx.hosServerDid = "did:hum:serverA";
  const first = frameOf(0, 60_000);
  ctx.ownProfileArrived(first);
  await settle();
  const again = clone(first); // the server still holds the same copy (its check of ours raced a save)
  ctx.ownProfileArrived(again);
  await settle();
  assert.equal(sent.length, 2);
  assert.equal(sent[0].at, first.signed_at + 1, "the first is dated after the copy the server holds");
  assert.equal(sent[1].at, sent[0].at + 1, "the second ask on this server is dated after the first");
  const atB = Date.now();
  ctx.hosServerDid = "did:hum:serverB";
  const other = clone(first);
  other.signed_at = 0;
  ctx.ownProfileArrived(other);
  await settle();
  assert.equal(sent.length, 3);
  assert.ok(sent[2].at >= atB && sent[2].at < sent[0].at, "another server: dated by the clock, not by server A's copies");
  for (const s of sent) assert.ok(await verifies(me.key, PC.copyText(me.key, FX.cases[0].copy, s.at), s.signature), `the copy dated ${s.at} verifies`);
});

// Seen red with the profile-copy.js tag moved after chat-profile.js in index.html: "before
// chat-profile.js" failed.
test("the chat page loads the builder before chat-profile.js, and keeps no copy of the words of its own", () => {
  const index = fs.readFileSync(path.join(WEB, "chat", "index.html"), "utf8");
  const at = (s) => index.indexOf(s);
  assert.ok(at('src="/shared/profile-copy.js?v=') > 0, "index.html loads /shared/profile-copy.js");
  assert.ok(at('src="/shared/profile-copy.js?v=') > at('src="/shared/profile-changes.js?v='), "after profile-changes.js");
  assert.ok(at('src="/shared/profile-copy.js?v=') < at('src="/chat/chat-profile.js?v='), "before chat-profile.js");
  const chat = fs.readFileSync(path.join(WEB, "chat", "chat-profile.js"), "utf8");
  assert.ok(!chat.includes("profile_v1"), "chat-profile.js keeps no copy of the signed words");
  assert.ok(chat.includes("C.publicCopy(") && chat.includes("C.copyText(") && chat.includes("C.nextCopyTime("), "it uses the shared builder");
  assert.ok(/function ownProfileArrived\(msg\) \{\s*if \(!msg \|\| typeof msg\.privacy !== 'string'\) return;[\s\S]{0,200}signProfileCopy\(msg\)/.test(chat),
    "ownProfileArrived asks signProfileCopy right after its own-copy check");
});
