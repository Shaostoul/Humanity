// Device-to-device big files, the website's shared half (2026-10-11, docs/design/media-showcase.md,
// "Device-to-device big files: the shapes, exact", sections 1, 3 and 5): web/shared/file-transfer.js
// (the pass, the notes, the channel protocol, the Sender and the Receiver) and web/shared/blake3.js
// (the BLAKE3 both ends hash with). The chat page's use of them (the attach step, the notes sent and
// taken, the question, the rows, the connection) is in file-transfer-chat-web.test.js.
//
// Run: node --test scripts/tests/file-transfer-web.test.js   (in `just rig-tests`)
//
// The two ends run against each other over a stand-in data channel: messages are delivered one per
// turn, in order, and its buffered amount counts what was sent and not delivered yet, as a browser's
// does. Hashes are real BLAKE3, held to the vendored noble bundle (itself held to the Rust crate by
// scripts/pq-kat.mjs); the pass is signed and checked with real Dilithium3 from that bundle.
//
// What it proves:
//  1. web/shared/blake3.js is BLAKE3: the same as the noble bundle at every length around the block,
//     chunk and tree boundaries and for every split of the input across update calls, and the
//     published hash of no input.
//  2. The pass's words and JSON and the three notes' text are the spec's, byte for byte: the spec's
//     own code blocks, their placeholders filled in, read from the design doc (and the shared
//     fixture, scripts/tests/fixtures/file-offer-v1.json, when it is in this tree). A pass signed
//     over the words verifies with real Dilithium3, and not for another file or the other way round.
//  3. An offer is read only when every field is fit: the piece size is 256 KiB, the piece count is
//     ceil(size / piece), the pass names the same transfer; a zero-byte file is never offered.
//  4. The hash list: the receiver checks BLAKE3 of the piece hashes against the offer's `hashes`
//     before asking for a single piece; a list that does not match ends it with the spec's words
//     and nothing is kept.
//  5. Piece checking: a piece that arrives damaged is asked for again and the file still arrives
//     whole; the third failure of one piece ends it, with nothing after that piece kept.
//  6. Resume: after a drop, and after a reload (a new Receiver over the same kept pieces), the
//     receiver asks again from the first piece it lacks, and the file arrives whole.
//  7. Pause: from the receiver, nothing more is sent until it resumes; from the sender, the
//     receiver asks for nothing until the sender resumes; then it goes on from the first missing.
//  8. Cancel from either side ends both ends.
//  9. The saved file only after the whole file's BLAKE3 matches: pieces that each match the list,
//     under an offer whose whole-file hash is another file's, are never handed over to be saved.
// 10. Nothing near the file's size is held on the sending side: hashing and sending a 40 MB file
//     read it only a piece at a time (the whole file is never read), every message is at most
//     64 KiB, and the channel never holds much more than 1 MB.
//
// HOS_WEB_DIR points the test at another copy of web/. Red first: see the end of this file.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const ROOT = path.join(__dirname, "..", "..");
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, "web");
const FT = require(path.join(WEB, "shared", "file-transfer.js"));
const B3 = require(path.join(WEB, "shared", "blake3.js"));

// Line endings undone (the working copy may have CRLF), so the spec's blocks read as written.
const SPEC = fs.readFileSync(path.join(ROOT, "docs", "design", "media-showcase.md"), "utf8").replace(/\r\n/g, "\n");
// The shapes section alone, up to the next heading (never the As built notes after it, which quote it).
const SHAPES_AT = SPEC.indexOf("## Device-to-device big files: the shapes, exact");
const SHAPES = SPEC.slice(SHAPES_AT, SPEC.indexOf("\n## ", SHAPES_AT + 10));
const FIXTURE = path.join(__dirname, "fixtures", "file-offer-v1.json");

const hex = (u8) => Buffer.from(u8).toString("hex");
const SERVER = "did:hum:4dQe1bVHyiHm1Vh8rWbx2F";

let NOBLE = null;
async function noble() {
  if (NOBLE) return NOBLE;
  const src = fs.readFileSync(path.join(WEB, "shared", "vendor", "noble-pq.bundle.js"));
  NOBLE = await import("data:text/javascript;base64," + Buffer.from(src).toString("base64"));
  return NOBLE;
}

// ── Stand-ins ────────────────────────────────────────────────────────────

/** Deterministic bytes for a file of `size`: byte i is a mix of i and the seed. */
function byteAt(i, seed) { return (Math.imul(i, 2654435761) >>> 13 ^ seed) & 0xff; }
function fileBytes(size, seed = 7) {
  const b = new Uint8Array(size);
  for (let i = 0; i < size; i++) b[i] = byteAt(i, seed);
  return b;
}

/**
 * A file that is never in memory: a Blob-like whose slices make their bytes when read. It keeps
 * every slice asked for and refuses to be read whole (arrayBuffer, text and stream on the file).
 */
function virtualFile(size, seed = 7) {
  const reads = [];
  const refuse = () => { throw new Error("the whole file was read"); };
  const slice = (a, b) => {
    const start = Math.max(0, a | 0);
    const end = Math.min(size, b === undefined ? size : b | 0);
    return {
      size: end - start,
      async arrayBuffer() {
        reads.push(end - start);
        const out = new Uint8Array(end - start);
        for (let i = start; i < end; i++) out[i - start] = byteAt(i, seed);
        return out.buffer;
      },
      slice: refuse,
    };
  };
  return { size, name: "big.bin", type: "application/octet-stream", reads, slice, arrayBuffer: refuse, text: refuse, stream: refuse };
}

/** Kept pieces in memory, as the page keeps them in IndexedDB. */
function memoryStore() {
  const pieces = new Map();
  return {
    pieces,
    async firstMissing() { let n = 0; while (pieces.has(n)) n++; return n; },
    async put(i, bytes) { pieces.set(i, new Uint8Array(bytes)); },
    async get(i) { return pieces.get(i) || null; },
    assembled() {
      const n = pieces.size;
      const parts = [];
      for (let i = 0; i < n; i++) parts.push(pieces.get(i));
      return Buffer.concat(parts.map((p) => Buffer.from(p)));
    },
  };
}

/**
 * A data channel between a Sender and a Receiver: one message delivered per turn, in order, each
 * way. `bufferedAmount` on the sending side counts the bytes sent and not delivered yet, and
 * drained() settles when it is at or under 256 KiB, as a browser's bufferedamountlow does.
 * `tamper(bytes, n)` may change a data message (the n-th) on its way to the receiver.
 */
function channel(opts = {}) {
  const st = { toReceiver: [], toSender: [], buffered: 0, maxBuffered: 0, maxMessage: 0, waiters: [], open: true, sent: [], said: [], deliveredData: 0 };
  let sender = null;
  let receiver = null;
  let n = 0;
  const wake = () => {
    if (st.buffered > 262144) return;
    const w = st.waiters.splice(0);
    for (const fn of w) fn();
  };
  const tick = () => {
    if (!st.open) return;
    if (st.toReceiver.length) {
      let m = st.toReceiver.shift();
      st.buffered -= typeof m === "string" ? m.length : m.length;
      if (typeof m !== "string") {
        st.deliveredData++;
        if (opts.tamper) m = opts.tamper(m, n++) || m;
      }
      if (receiver) receiver.receive(m);
      wake();
    }
    if (st.toSender.length) {
      const m = st.toSender.shift();
      if (sender) sender.receive(m);
    }
    if (st.toReceiver.length || st.toSender.length) setImmediate(tick);
    else st.idle = true;
  };
  const kick = () => { if (st.idle !== false) { st.idle = false; setImmediate(tick); } };
  const senderIo = {
    send(data) {
      if (!st.open) throw new Error("closed");
      const size = typeof data === "string" ? data.length : data.length;
      if (typeof data !== "string") st.maxMessage = Math.max(st.maxMessage, size);
      st.sent.push(typeof data === "string" ? data : "bin:" + size);
      st.buffered += size;
      st.maxBuffered = Math.max(st.maxBuffered, st.buffered);
      st.toReceiver.push(typeof data === "string" ? data : new Uint8Array(data));
      kick();
    },
    buffered: () => st.buffered,
    drained: () => new Promise((r) => { st.waiters.push(r); kick(); }),
  };
  const receiverIo = {
    send(data) {
      if (!st.open) throw new Error("closed");
      st.said.push(data);
      st.toSender.push(data);
      kick();
    },
    buffered: () => 0,
    drained: () => Promise.resolve(),
  };
  return {
    st, senderIo, receiverIo,
    bind(s, r) { sender = s; receiver = r; },
    /** The connection drops: nothing more is delivered either way. */
    drop() { st.open = false; st.toReceiver.length = 0; st.toSender.length = 0; st.buffered = 0; wake(); },
  };
}

/** Wait (by turns) until `pred` holds; fail naming `what` if it never does. */
async function waitFor(pred, what, turns = 2000000) {
  for (let i = 0; i < turns; i++) {
    if (pred()) return;
    await new Promise((r) => setImmediate(r));
  }
  assert.fail("never happened: " + what);
}
/** A few quiet turns. */
async function turns(n = 50) { for (let i = 0; i < n; i++) await new Promise((r) => setImmediate(r)); }

/** An offer for `file` with its hashes made the way the sender's page makes them. */
async function offerFor(file, over = {}) {
  const h = await FT.hashFile(file, { blake3: B3 });
  const id = over.id || "0123456789abcdef0123456789abcdef";
  return {
    hashed: h,
    offer: Object.assign({
      id, name: file.name || "clip.bin", size: file.size, type: file.type || "", blake3: h.blake3,
      piece: FT.PIECE_BYTES, pieces: h.pieces, hashes: h.hashes,
      pass: FT.filePassObject(id, 1760000000 + 3600, "c2ln"),
    }, over),
  };
}

/** A sender and a receiver over one channel; events kept on each. */
function connect(offer, file, pieceHashes, store, opts = {}) {
  const ch = channel(opts);
  const ev = { sender: [], receiver: [], sentAt: {} };
  // ev.sentAt[type]: how many messages the sender had sent when it raised that event.
  const sender = new FT.Sender({ offer, file, pieceHashes, io: ch.senderIo, onEvent: (t, d) => { ev.sender.push([t, d]); ev.sentAt[t] = ch.st.sent.length; } });
  const receiver = opts.receiver || new FT.Receiver({ offer, store, blake3: B3, onEvent: (t, d) => ev.receiver.push([t, d]) });
  if (opts.receiver) opts.receiver.onEvent = (t, d) => ev.receiver.push([t, d]);
  ch.bind(sender, receiver);
  return { ch, ev, sender, receiver, start: () => receiver.attach(ch.receiverIo) };
}
const ended = (list) => (list.find(([t]) => t === "end") || [])[1];
const has = (list, type) => list.some(([t]) => t === type);
const gets = (st) => st.said.filter((s) => typeof s === "string" && s.includes('"t":"get"')).map((s) => JSON.parse(s));

// ── 1. BLAKE3 ──────────────────────────────────────────────────────────────

test("1. blake3.js is BLAKE3: the noble bundle's hash at every boundary and for every split", async () => {
  const m = await noble();
  assert.equal(hex(B3.hash(new Uint8Array(0))), "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262", "the published hash of no input");
  const lengths = [0, 1, 2, 63, 64, 65, 127, 128, 129, 1023, 1024, 1025, 2047, 2048, 2049, 3072, 3073, 4095, 4096, 4097,
    5120, 5121, 6144, 6145, 7168, 7169, 8192, 8193, 16384, 16385, 31744, 65536, 102400, 262143, 262144, 262145, 1048576 + 77];
  const splits = [[1], [63, 1, 64], [65, 1023, 7], [1024], [999, 25, 3000], [262144]];
  for (const n of lengths) {
    const b = new Uint8Array(n);
    for (let i = 0; i < n; i++) b[i] = i % 251; // the official test vectors' input
    const want = hex(m.blake3.create({ dkLen: 32 }).update(b).digest());
    assert.equal(hex(B3.hash(b)), want, `one update of ${n} bytes`);
    for (const steps of splits) {
      const h = B3.create();
      let p = 0;
      let s = 0;
      while (p < n) { const t = Math.min(steps[s++ % steps.length], n - p); h.update(b.subarray(p, p + t)); p += t; }
      assert.equal(hex(h.digest()), want, `${n} bytes in updates of ${steps.join(",")}`);
    }
  }
});

// ── 2. The words and the notes, byte for byte ─────────────────────────────

/** The spec's code block that starts with `start`, its lines joined the way the JSON is written (no line breaks). */
function specBlock(start) {
  const at = SHAPES.indexOf("```text\n" + start);
  assert.ok(at >= 0, "the spec's block starting " + start);
  const body = SHAPES.slice(at + "```text\n".length, SHAPES.indexOf("```", at + 8));
  return body;
}

test("2. the pass's words and JSON and the three notes are the spec's, byte for byte", async () => {
  const id = "00112233445566778899aabbccddeeff";
  const issuer = "a1".repeat(8);
  const grantee = "b2".repeat(8);
  const expiry = 1760604800;
  // The pass's words: the spec's block with each {field} filled in and each \n a line break.
  const wordsBlock = specBlock("hum/file-pass/v1").trim();
  assert.equal(wordsBlock, "hum/file-pass/v1\\n{server}\\n{issuer}\\n{grantee}\\n{id}\\n{expiry}", "the spec's block, as written");
  const filled = wordsBlock.replace(/\\n/g, "\n").replace("{server}", SERVER).replace("{issuer}", issuer)
    .replace("{grantee}", grantee).replace("{id}", id).replace("{expiry}", String(expiry));
  assert.equal(FT.filePassPreimage(SERVER, issuer, grantee, id, expiry), filled, "the words a pass signs");
  // The pass as it travels.
  assert.ok(SHAPES.includes('`{"v":1,"id":"<id>","expiry":<n>,"sig":"<base64>"}`'), "the spec's pass shape");
  assert.equal(JSON.stringify(FT.filePassObject(id, expiry, "QUJD")), `{"v":1,"id":"${id}","expiry":${expiry},"sig":"QUJD"}`);

  // The three notes: the spec's block, its wrapped lines joined, its placeholders filled in order.
  const notes = specBlock("[[hum:offer:v1]]").split(/\n(?=\[\[)/).map((s) => s.replace(/\n\s*/g, "").trim());
  assert.equal(notes.length, 3, "three notes in the spec's block");
  const whole = "ab".repeat(32);
  const list = "cd".repeat(32);
  const sig = "c2lnbmF0dXJl";
  const offer = {
    id, name: "clip.mp4", size: 2468000000, type: "video/mp4", blake3: whole, piece: 262144, pieces: 9415, hashes: list,
    pass: FT.filePassObject(id, expiry, sig),
  };
  const expectOffer = notes[0]
    .replace('"<32 hex>"', `"${id}"`)
    .replace(/"<64 hex, the whole file>"/, `"${whole}"`)
    .replace(/"<64 hex, BLAKE3 of the piece hashes[^"]*>"/, `"${list}"`)
    .replace('"<same id>"', `"${id}"`)
    .replace("<n>", String(expiry))
    .replace('"<base64>"', `"${sig}"`);
  assert.ok(!/<[^>]*>/.test(expectOffer), "every placeholder filled: " + expectOffer);
  assert.equal(FT.offerNoteText(offer), expectOffer, "the offer note");
  assert.equal(FT.piecesFor(2468000000, 262144), 9415, "the spec's piece count is ceil(size / piece)");
  assert.equal(FT.answerNoteText(id, true), notes[1].replace('"<32 hex>"', `"${id}"`).replace("true|false", "true"), "the answer, yes");
  assert.equal(FT.answerNoteText(id, false), notes[1].replace('"<32 hex>"', `"${id}"`).replace("true|false", "false"), "the answer, no");
  assert.equal(FT.cancelNoteText(id), notes[2].replace('"<32 hex>"', `"${id}"`), "the cancel");
  // Read back to the same fields.
  assert.deepEqual(FT.offerNoteParse(FT.offerNoteText(offer)), offer);
  assert.deepEqual(FT.answerNoteParse(FT.answerNoteText(id, false)), { id, accept: false });
  assert.deepEqual(FT.cancelNoteParse(FT.cancelNoteText(id)), { id });
  assert.equal(FT.channelLabel(id), "file:" + id, "the channel's label");
  assert.equal(FT.channelLabelId("file:" + id), id);
  assert.equal(FT.channelLabelId("dm"), null);

  // The shared fixture, minted by the relay's Rust (scripts/tests/fixtures/file-offer-v1.json; the
  // desktop app's tests read the same one): the website must agree with it byte for byte, the
  // pass's words, its signature, the notes and the file's hashes. Until 2026-10-12 this block
  // looked for field names the fixture does not have, and checked nothing.
  const fx = JSON.parse(fs.readFileSync(FIXTURE, "utf8"));
  assert.equal(FT.filePassPreimage(fx.server, fx.issuer, fx.grantee, fx.id, fx.expiry), fx.preimage, "the pass's words, as the relay writes them");
  const nm = await noble();
  assert.ok(
    nm.ml_dsa65.verify(Buffer.from(fx.sig, "base64"), new TextEncoder().encode(fx.preimage), Buffer.from(fx.issuer, "hex")),
    "the relay's signature verifies here",
  );
  assert.deepEqual(FT.filePassParse(fx.pass), FT.filePassParse(JSON.parse(fx.pass_text)), "the pass object and its text agree");
  const parsedOffer = FT.offerNoteParse(fx.offer_note);
  assert.ok(parsedOffer, "the fixture's offer reads");
  assert.equal(FT.offerNoteText(parsedOffer), fx.offer_note, "and is written back byte for byte");
  assert.equal(FT.answerNoteText(fx.id, true), fx.accept_note, "the yes");
  assert.equal(FT.answerNoteText(fx.id, false), fx.decline_note, "the no");
  assert.equal(FT.cancelNoteText(fx.id), fx.cancel_note, "the cancel");
  // The fixture's file is defined by a rule (byte i is i mod 251); its hashes, as the web makes them.
  const fileU8 = new Uint8Array(fx.file.size);
  for (let i = 0; i < fileU8.length; i++) fileU8[i] = i % 251;
  const B3 = require(path.join(WEB, "shared", "blake3.js"));
  const got = await FT.hashFile(new Blob([fileU8]), { blake3: B3 });
  assert.equal(got.pieces, fx.file.pieces, "the piece count");
  assert.deepEqual(
    Array.from({ length: got.pieces }, (_, i) => hex(got.pieceHashes.subarray(i * 32, i * 32 + 32))),
    fx.file.piece_hashes,
    "each piece's hash",
  );
  assert.equal(got.hashes, fx.file.hashes, "the hash of the piece hashes");
  assert.equal(got.blake3, parsedOffer.blake3, "the whole file's hash, as in the offer");

  // A real signature over the words: it verifies for this transfer and these two, and nothing else.
  const m = await noble();
  const kp = m.ml_dsa65.keygen(new Uint8Array(32).fill(9));
  const issuerKey = hex(kp.publicKey);
  const words = new TextEncoder().encode(FT.filePassPreimage(SERVER, issuerKey, grantee, id, expiry));
  const signature = m.ml_dsa65.sign(words, kp.secretKey);
  assert.ok(m.ml_dsa65.verify(signature, words, kp.publicKey), "the pass verifies");
  const other = new TextEncoder().encode(FT.filePassPreimage(SERVER, issuerKey, grantee, "ff".repeat(16), expiry));
  assert.ok(!m.ml_dsa65.verify(signature, other, kp.publicKey), "not for another file");
  const reversed = new TextEncoder().encode(FT.filePassPreimage(SERVER, grantee, issuerKey, id, expiry));
  assert.ok(!m.ml_dsa65.verify(signature, reversed, kp.publicKey), "nor the other way round");
});

// ── 3. Only a fit offer is read ───────────────────────────────────────────

test("3. an offer is read only when every field is fit, and a pass only for its own transfer", () => {
  const id = "00112233445566778899aabbccddeeff";
  const base = {
    id, name: "clip.mp4", size: 600000, type: "video/mp4", blake3: "ab".repeat(32), piece: 262144, pieces: 3,
    hashes: "cd".repeat(32), pass: FT.filePassObject(id, 1760604800, "c2ln"),
  };
  assert.ok(FT.offerNoteParse(FT.CTL_OFFER + JSON.stringify(base)), "the fit offer");
  const bad = (change, what) => {
    const v = Object.assign({}, base, change);
    assert.equal(FT.offerNoteParse(FT.CTL_OFFER + JSON.stringify(v)), null, what);
    assert.equal(FT.offerNoteText(v), null, what + " (and it is never written)");
  };
  bad({ piece: 131072, pieces: 5 }, "a piece size other than 256 KiB");
  bad({ pieces: 4 }, "a piece count that is not ceil(size / piece)");
  bad({ size: 0, pieces: 0 }, "a zero-byte file");
  bad({ id: "00112233445566778899AABBCCDDEEFF" }, "an id in capitals");
  bad({ blake3: "ab".repeat(31) }, "a short hash");
  bad({ name: "two\nlines" }, "a name with a line break");
  bad({ pass: FT.filePassObject("ff".repeat(16), 1760604800, "c2ln") }, "a pass for another transfer");
  bad({ pass: { v: 2, id, expiry: 1760604800, sig: "c2ln" } }, "a pass of another version");
  assert.equal(FT.offerNoteParse("[[hum:offer:v1]]not json"), null);
  assert.equal(FT.answerNoteParse(FT.CTL_OFFER_ANSWER + '{"id":"' + id + '","accept":"yes"}'), null, "an answer is true or false");
  assert.ok(FT.isFileNoteText(FT.cancelNoteText(id)) && FT.isFileNoteText(FT.answerNoteText(id, true)) && FT.isFileNoteText(FT.offerNoteText(base)));
  assert.ok(!FT.isFileNoteText("[[hum:file:v1]]abc"), "a file marker is not one of these notes");
  const now = 1760000000;
  assert.ok(FT.filePassTimeOk({ expiry: FT.filePassExpiry(now) }, now), "a pass for 7 days is good");
  assert.equal(FT.filePassExpiry(now) - now, 7 * 24 * 3600, "and that is the most it is made for");
  assert.ok(!FT.filePassTimeOk({ expiry: now - 1 }, now), "one that ended is not");
  assert.ok(!FT.filePassTimeOk({ expiry: now + 9 * 24 * 3600 }, now), "nor one too far ahead");
});

// ── 4. The hash list ──────────────────────────────────────────────────────

test("4. the receiver checks the piece hashes against the offer before asking for a piece", async () => {
  const bytes = fileBytes(3 * FT.PIECE_BYTES + 1234);
  const file = new Blob([bytes]);
  const { offer, hashed } = await offerFor(file);
  // The sender's list is another file's: each entry changed.
  const wrong = hashed.pieceHashes.slice();
  wrong[5] ^= 1;
  const store = memoryStore();
  const c = connect(offer, file, wrong, store);
  await c.start();
  await waitFor(() => ended(c.ev.receiver), "the receiver ends");
  assert.equal(ended(c.ev.receiver).reason, "mismatch", "the list does not match its description");
  assert.equal(gets(c.ch.st).length, 0, "not one piece was asked for");
  assert.equal(store.pieces.size, 0, "nothing kept");
  assert.equal(FT.WORDS.mismatch("Ann"), "The file Ann offered does not match its description", "the spec's words");
  assert.ok(SHAPES.replace(/\s+/g, " ").includes('"The file Ann offered does not match its description"'), "the spec's own words");
  // A list that is too long ends it the same way.
  const long = new Uint8Array(hashed.pieceHashes.length + 32);
  long.set(hashed.pieceHashes);
  const c2 = connect(offer, file, long, memoryStore());
  await c2.start();
  await waitFor(() => ended(c2.ev.receiver), "the receiver ends");
  assert.equal(ended(c2.ev.receiver).reason, "mismatch");
});

// ── 5. Piece checking ─────────────────────────────────────────────────────

test("5. a damaged piece is asked for again; the third failure of one piece ends it", async () => {
  const bytes = fileBytes(5 * FT.PIECE_BYTES + 777, 3);
  const file = new Blob([bytes]);
  const { offer, hashed } = await offerFor(file);
  // Piece 2 arrives damaged once: asked for again, and the file arrives whole.
  let damaged = 0;
  const once = (m) => {
    const r = FT.readPieceMessage(m);
    if (r.piece === 2 && r.offset === 65528 && damaged === 0) { damaged++; const x = m.slice(); x[100] ^= 0xff; return x; }
    return m;
  };
  const store = memoryStore();
  const c = connect(offer, file, hashed.pieceHashes, store, { tamper: once });
  await c.start();
  await waitFor(() => has(c.ev.receiver, "complete") || ended(c.ev.receiver), "the transfer ends");
  assert.ok(has(c.ev.receiver, "complete"), "it completes: " + JSON.stringify(ended(c.ev.receiver)));
  assert.deepEqual(c.ev.receiver.filter(([t]) => t === "piece-failed").map(([, d]) => d), [{ piece: 2, tries: 1 }], "one failure, of piece 2");
  const asks = gets(c.ch.st);
  assert.ok(asks.some((g, i) => i > 0 && g.from === 2), "piece 2 asked for again: " + JSON.stringify(asks));
  assert.ok(Buffer.from(bytes).equals(store.assembled()), "the kept pieces are the file");
  await waitFor(() => ended(c.ev.sender), "the sender hears done");
  assert.equal(ended(c.ev.sender).reason, "done");

  // Piece 3 arrives damaged every time: three tries, then it ends, with nothing after piece 3 kept.
  const always = (m) => {
    const r = FT.readPieceMessage(m);
    if (r.piece === 3 && r.offset === 0) { const x = m.slice(); x[20] ^= 0x55; return x; }
    return m;
  };
  const store2 = memoryStore();
  const c2 = connect(offer, file, hashed.pieceHashes, store2, { tamper: always });
  await c2.start();
  await waitFor(() => ended(c2.ev.receiver), "the receiver ends");
  assert.equal(ended(c2.ev.receiver).reason, "damaged", "three failures of one piece end it");
  assert.deepEqual(c2.ev.receiver.filter(([t]) => t === "piece-failed").map(([, d]) => d.tries), [1, 2, 3], "after exactly three tries");
  assert.deepEqual([...store2.pieces.keys()].sort(), [0, 1, 2], "nothing after piece 3 kept");
  assert.ok(!has(c2.ev.receiver, "complete"));
  assert.ok(SHAPES.replace(/\s+/g, " ").includes("three failures of one piece end the transfer with words"), "the spec's rule");
});

// ── 6. Resume ─────────────────────────────────────────────────────────────

test("6. after a drop, and after a reload, it asks again from the first piece it lacks", async () => {
  const bytes = fileBytes(40 * FT.PIECE_BYTES + 99, 11);
  const file = new Blob([bytes]);
  const { offer, hashed } = await offerFor(file);
  const store = memoryStore();
  const c = connect(offer, file, hashed.pieceHashes, store);
  await c.start();
  await waitFor(() => store.pieces.size >= 7, "seven pieces kept");
  c.ch.drop();
  c.sender.close();
  c.receiver.detach();
  await c.receiver.queue;
  await turns(20);
  const kept = await store.firstMissing();
  assert.ok(kept >= 7 && kept < offer.pieces, "part of the file kept: " + kept);
  // The same Receiver on a new connection (the page still open).
  const c2 = connect(offer, file, hashed.pieceHashes, store, { receiver: c.receiver });
  await c2.start();
  await waitFor(() => gets(c2.ch.st).length > 0, "it asks");
  assert.equal(gets(c2.ch.st)[0].from, kept, "from the first piece it lacks");
  assert.ok(!c2.ch.st.said.includes('{"t":"hashes"}'), "it already holds the checked list");
  await waitFor(() => store.pieces.size >= 20, "more pieces kept");
  c2.ch.drop();
  c2.sender.close();
  c2.receiver.detach();
  await c2.receiver.queue;
  await turns(20);

  // A reload: a new Receiver over the same kept pieces.
  const kept2 = await store.firstMissing();
  const c3 = connect(offer, file, hashed.pieceHashes, store);
  await c3.start();
  await waitFor(() => has(c3.ev.receiver, "complete") || ended(c3.ev.receiver), "the transfer ends");
  assert.ok(has(c3.ev.receiver, "complete"), "it completes");
  assert.ok(c3.ch.st.said[0] === '{"t":"hashes"}', "a new page asks for the list again first");
  assert.equal(gets(c3.ch.st)[0].from, kept2, "then from the first piece it lacks: " + kept2);
  assert.ok(Buffer.from(bytes).equals(store.assembled()), "the file arrived whole, the pieces kept before the reload checked in the whole-file hash");
  for (const g of [...gets(c.ch.st), ...gets(c2.ch.st), ...gets(c3.ch.st)]) assert.ok(g.count >= 1 && g.count <= 16, "each window at most 16 pieces");
});

// ── 7. Pause ──────────────────────────────────────────────────────────────

test("7. a pause from either side holds the transfer until the same side resumes", async () => {
  const bytes = fileBytes(40 * FT.PIECE_BYTES, 5);
  const file = new Blob([bytes]);
  const { offer, hashed } = await offerFor(file);
  const store = memoryStore();
  const c = connect(offer, file, hashed.pieceHashes, store);
  await c.start();
  await waitFor(() => store.pieces.size >= 3, "three pieces kept");
  c.receiver.pause();
  await waitFor(() => has(c.ev.sender, "paused-there"), "the sender hears");
  await turns(400);
  const atPause = store.pieces.size;
  assert.ok(c.ch.st.sent.length - c.ev.sentAt["paused-there"] <= 1, "the sender stops once it hears the pause (the rest of its window is not sent): "
    + (c.ch.st.sent.length - c.ev.sentAt["paused-there"]) + " more");
  const sentAtPause = c.ch.st.sent.length;
  await turns(400);
  assert.equal(c.ch.st.sent.length, sentAtPause, "the sender sends nothing more while the receiver is paused");
  assert.equal(store.pieces.size, atPause, "and nothing more is kept");
  c.receiver.resume();
  await waitFor(() => store.pieces.size > atPause + 2, "it goes on");
  const lastAsk = gets(c.ch.st).pop();
  assert.ok(lastAsk.from <= atPause + 16, "from the first piece it lacked");

  // The sender pauses: the receiver asks for nothing until the sender resumes.
  c.sender.pause();
  await waitFor(() => has(c.ev.receiver, "paused-there"), "the receiver hears");
  await turns(400);
  const asksAtPause = gets(c.ch.st).length;
  const keptAtPause = store.pieces.size;
  await turns(400);
  assert.equal(gets(c.ch.st).length, asksAtPause, "no asks while the sender is paused");
  c.receiver.resume(); // the receiver's own resume does not undo the sender's pause
  await turns(200);
  assert.equal(store.pieces.size, keptAtPause, "nothing kept while the sender is paused");
  c.sender.resume();
  await waitFor(() => has(c.ev.receiver, "complete") || ended(c.ev.receiver), "the transfer ends");
  assert.ok(has(c.ev.receiver, "complete"), "it completes after the sender resumes");
  assert.equal(gets(c.ch.st)[asksAtPause].from, keptAtPause, "asking from the first piece it lacked");
  assert.ok(Buffer.from(bytes).equals(store.assembled()));
});

// ── 8. Cancel ─────────────────────────────────────────────────────────────

test("8. cancel from either side ends both ends", async () => {
  const bytes = fileBytes(30 * FT.PIECE_BYTES, 2);
  const file = new Blob([bytes]);
  const { offer, hashed } = await offerFor(file);
  for (const side of ["receiver", "sender"]) {
    const store = memoryStore();
    const c = connect(offer, file, hashed.pieceHashes, store);
    await c.start();
    await waitFor(() => store.pieces.size >= 2, "two pieces kept");
    c[side].cancel();
    const other = side === "receiver" ? "sender" : "receiver";
    await waitFor(() => ended(c.ev[other]), `the ${other} hears`);
    assert.equal(ended(c.ev[side]).reason, "cancelled-here", `the ${side} cancelled`);
    assert.equal(ended(c.ev[other]).reason, "cancelled-there", `the ${other} was told`);
    const sentAt = c.ch.st.sent.length;
    await turns(300);
    assert.ok(c.ch.st.sent.length - sentAt <= 1, "nothing more is sent");
    assert.ok(!has(c.ev.receiver, "complete"));
  }
});

// ── 9. The whole file's BLAKE3 ────────────────────────────────────────────

test("9. nothing is handed over to be saved unless the whole file's BLAKE3 matches the offer", async () => {
  const bytes = fileBytes(4 * FT.PIECE_BYTES + 5, 9);
  const file = new Blob([bytes]);
  const { offer, hashed } = await offerFor(file);
  // Every piece matches the list, and the list matches the offer: only the whole-file hash differs.
  const lying = Object.assign({}, offer, { blake3: hex(B3.hash(fileBytes(10, 1))) });
  const store = memoryStore();
  const c = connect(lying, file, hashed.pieceHashes, store);
  await c.start();
  await waitFor(() => ended(c.ev.receiver), "the receiver ends");
  assert.equal(ended(c.ev.receiver).reason, "mismatch-file", "the whole file is not the one described");
  assert.ok(has(c.ev.receiver, "checking"), "it was checked");
  assert.ok(!has(c.ev.receiver, "complete"), "never handed over to be saved");
  assert.ok(!c.ch.st.said.includes('{"t":"done"}'), "and the sender is not told it is done");
  // The honest offer completes, and says done only after the check.
  const c2 = connect(offer, file, hashed.pieceHashes, memoryStore());
  await c2.start();
  await waitFor(() => has(c2.ev.receiver, "complete"), "it completes");
  const order = c2.ev.receiver.map(([t]) => t).filter((t) => t === "checking" || t === "complete");
  assert.deepEqual(order, ["checking", "complete"]);
  assert.equal(c2.ch.st.said[c2.ch.st.said.length - 1], '{"t":"done"}', "done is the last word, after the check");
});

// ── 10. The sending side holds nothing near the file's size ───────────────

test("10. hashing and sending a 40 MB file read it a piece at a time, in messages of 64 KiB, the channel held near 1 MB", async () => {
  const size = 40 * 1024 * 1024 + 4321;
  const file = virtualFile(size, 13);
  const hashed = await FT.hashFile(file, { blake3: B3 });
  assert.equal(hashed.pieces, Math.ceil(size / FT.PIECE_BYTES));
  assert.ok(Math.max(...file.reads) <= FT.PIECE_BYTES, "hashing read at most one piece at a time: " + Math.max(...file.reads));
  assert.equal(file.reads.reduce((a, b) => a + b, 0), size, "and each byte once");
  file.reads.length = 0;
  const id = "fe".repeat(16);
  const offer = {
    id, name: "big.bin", size, type: "", blake3: hashed.blake3, piece: FT.PIECE_BYTES, pieces: hashed.pieces, hashes: hashed.hashes,
    pass: FT.filePassObject(id, 1760604800, "c2ln"),
  };
  const store = memoryStore();
  const c = connect(offer, file, hashed.pieceHashes, store);
  await c.start();
  await waitFor(() => has(c.ev.receiver, "complete") || ended(c.ev.receiver), "the transfer ends");
  assert.ok(has(c.ev.receiver, "complete"), "it completes: " + JSON.stringify(ended(c.ev.receiver)));
  assert.ok(Math.max(...file.reads) <= FT.PIECE_BYTES, "sending read at most one piece at a time");
  assert.ok(c.ch.st.maxMessage <= 65536, "every message at most 64 KiB: " + c.ch.st.maxMessage);
  assert.ok(c.ch.st.maxBuffered <= FT.BUFFER_HIGH + 65536, "the channel never held more than 1 MB and one message: " + c.ch.st.maxBuffered);
  assert.ok(c.ch.st.maxBuffered > FT.BUFFER_HIGH / 2, "while keeping it busy: " + c.ch.st.maxBuffered);
  let ok = true;
  for (let i = 0; i < hashed.pieces && ok; i++) {
    const p = store.pieces.get(i);
    for (let k = 0; k < p.length; k += 4099) if (p[k] !== byteAt(i * FT.PIECE_BYTES + k, 13)) { ok = false; break; }
  }
  assert.ok(ok, "the pieces kept are the file's");
});

// ── Red first ────────────────────────────────────────────────────────────
// 2026-10-11: each break made in a copy of web/ and this file run against it with HOS_WEB_DIR,
// seen failing with the assertion named; every other test passed.
//  E1  blake3.js, the first G's second rotation 11 instead of 12: test 1 "the published hash of no
//      input".
//  E2  file-transfer.js, filePassPreimage with issuer and grantee swapped: test 2 "the words a pass
//      signs".
//  E3  file-transfer.js, offerChecked writing `name` before `id`: test 2 "the offer note".
//  E4  file-transfer.js, the Receiver not checking the piece hashes against `hashes`: test 4 "the
//      list does not match its description".
//  E5  file-transfer.js, the Receiver not checking a piece against its hash: test 5 "it completes"
//      (it ended "mismatch-file" instead, at the whole-file check).
//  E6  file-transfer.js, PIECE_TRIES 4: test 5 "after exactly three tries".
//  E7  file-transfer.js, Receiver.load ignoring the kept pieces (always from 0): test 6 "then from
//      the first piece it lacks".
//  E8  file-transfer.js, the Sender hearing a pause but sending the rest of its window: test 7 "the
//      sender stops once it hears the pause" (46 more). The first version of this test measured
//      only after the window had run out, and passed against this break; it was tightened.
//  E9  file-transfer.js, Receiver._finish without the whole-file check: test 9 "the receiver ends"
//      (it completed and asked for the file to be saved).
//  E10 file-transfer.js, readPiece reading the whole file and slicing it: test 10 (the stand-in file
//      refuses to be read whole).
//  E11 file-transfer.js, the Sender not holding back over 1 MB: test 10 "the channel never held
//      more than 1 MB and one message" (4,194,944).
//  E12 file-transfer.js, the Sender ignoring {"t":"cancel"}: test 8 "the sender hears".
//  E13 file-transfer.js, CHUNK_BYTES 64 KiB without room for the header: test 10 "every message at
//      most 64 KiB" (65,544), and test 5, whose damaged message no longer started where it did.
//  E14 file-transfer.js, offerChecked taking any piece size: test 3 "a piece size other than
//      256 KiB".
