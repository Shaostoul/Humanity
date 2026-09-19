#!/usr/bin/env node
// live-publish: a TEST PUBLISHER for the live video path, so the whole chain
// (publish -> relay fanout -> a `watch:` screen on an in-world wall) can be
// exercised with nobody broadcasting.
//
// The live feature does not watch a third-party stream; it watches OUR OWN
// relay (src/relay/live.rs). So until somebody goes live from Studio there is
// nothing for a wall screen to show, and "does the screen work" cannot be
// answered. This script is the missing half: it authenticates as a publisher
// exactly the way the native publisher does (src/net/live.rs), then pushes a
// generated test pattern in the documented envelope.
//
// What it sends, per frame, one binary WebSocket message:
//
//     [1 byte tag][8 bytes PTS micros, big-endian][JPEG bytes]
//
// tag 1 = keyframe. v1 of the wire format is MJPEG, so every frame is a
// self-contained keyframe and no codec-config frame (tag 0) is ever sent.
// This is the envelope documented at the top of src/relay/live.rs and built
// by `envelope()` in src/net/live.rs.
//
// AUTH (src/relay/live.rs::authenticate). The first message on the publisher
// socket is a JSON auth frame:
//
//     {"key": <dilithium3 public key hex>, "timestamp": <ms>,
//      "sig": <hex of Dilithium3 over "live_publish\n<timestamp>">,
//      "title": ..., "chat": ...}
//
// The relay verifies the signature, checks freshness (5 min) and replay, and
// then resolves THE STREAM ID SERVER-SIDE from the key's registered name. You
// cannot publish under a name you have not claimed, which is why --register
// exists: it claims the name over the ordinary chat socket first, using the
// same two-phase Dilithium identify challenge every client uses.
//
// The signing is the repo's own: the vendored noble bundle the chat client
// ships (web/shared/vendor/noble-pq.bundle.js), whose ML-DSA-65 keygen is
// pinned to the Rust implementation by scripts/pq-kat.mjs. No second crypto
// implementation is introduced here.
//
// LOCAL RELAYS ONLY. This refuses any server that is not loopback unless you
// pass --allow-remote. united-humanity.us is the operator's live service; its
// streaming switch and its stream names are the operator's decision, not a
// test script's.
//
// Usage:
//   # a local relay on port 3399, claim the name first, then publish 60 s
//   node scripts/live-publish.js --server http://127.0.0.1:3399 \
//       --name shaostoul --register --size 640x360 --fps 8 --seconds 60
//
//   node scripts/live-publish.js --help     full option list
//
// Exit 0 = the run finished (or was stopped with Ctrl-C after publishing).
// Exit 1 = refused before connecting. Exit 2 = the relay refused us, or the
// socket failed.

const fs = require("fs");
const path = require("path");
const { pathToFileURL } = require("url");

const REPO = path.resolve(__dirname, "..");
const BUNDLE = path.join(REPO, "web", "shared", "vendor", "noble-pq.bundle.js");

// The vendored PQ bundle is a `.js` file written as an ES module, and this
// script is CommonJS, so importing it makes Node print a four-line
// MODULE_TYPELESS_PACKAGE_JSON warning before anything else. Adding
// "type": "module" to package.json would be the fix and would break every
// other CommonJS script in scripts/, so this drops that ONE warning and
// leaves every other warning printing exactly as Node would.
process.removeAllListeners("warning");
process.on("warning", (w) => {
  if (w && w.name === "MODULE_TYPELESS_PACKAGE_JSON") return;
  console.error(w && w.stack ? w.stack : String(w));
});

// ── Options ──────────────────────────────────────────────────────────────────
const argv = process.argv.slice(2);
const has = (name) => argv.includes(name);
const opt = (name, def) => {
  const i = argv.indexOf(name);
  return i >= 0 && argv[i + 1] !== undefined ? argv[i + 1] : def;
};

if (has("--help") || has("-h")) {
  console.log(`
live-publish: publish a generated test pattern to a LOCAL HumanityOS relay.

  --server URL       relay base, e.g. http://127.0.0.1:3399 (required)
  --name NAME        the registered name to publish under (default: shaostoul)
                     The relay resolves the stream id from this name, so a
                     wall screen whose source is watch:<name> shows this.
  --register         claim NAME on the relay first (chat identify handshake).
                     Needed once per relay database.
  --seed HEX         32-byte master seed, hex. Default: derived from NAME, so
                     the same name always gets the same throwaway identity.
  --size WxH         frame size (default 640x360)
  --fps N            frames per second (default 8)
  --seconds N        how long to publish; 0 means until killed (default 30)
  --quality N        JPEG quality 1-100 (default 70)
  --title TEXT       stream title shown in /api/live (default: a test title)
  --chat ROOM        bound chat room (default: the relay's #live-<name>)
  --allow-remote     permit a non-loopback server. Do not point this at
                     united-humanity.us.
  --quiet            only print errors and the final summary
`);
  process.exit(0);
}

const SERVER = String(opt("--server", "")).trim();
const NAME = String(opt("--name", "shaostoul")).trim().toLowerCase();
const REGISTER = has("--register");
const SIZE = String(opt("--size", "640x360"));
const FPS = Math.max(1, Math.min(60, Number(opt("--fps", "8")) || 8));
const SECONDS = Math.max(0, Number(opt("--seconds", "30")) || 0);
const QUALITY = Math.max(1, Math.min(100, Number(opt("--quality", "70")) || 70));
const TITLE = String(opt("--title", `Test pattern (${NAME})`));
const CHAT = String(opt("--chat", ""));
const ALLOW_REMOTE = has("--allow-remote");
const QUIET = has("--quiet");

function die(code, ...lines) {
  for (const l of lines) console.error(l);
  process.exit(code);
}
function log(...a) {
  if (!QUIET) console.log("[live-publish]", ...a);
}

if (!SERVER) die(1, "live-publish: --server is required (e.g. --server http://127.0.0.1:3399)");

// A name must survive the relay's own format check (relay.rs: letters,
// digits, underscore, dash, at most 24), or --register is refused there with
// a message that arrives a whole handshake later.
if (!/^[a-z0-9_-]{1,24}$/.test(NAME)) {
  die(1, `live-publish: --name ${JSON.stringify(NAME)} is not a valid relay name (letters, digits, _ and -, max 24).`);
}

const m = /^(\d+)\s*[xX]\s*(\d+)$/.exec(SIZE.trim());
if (!m) die(1, `live-publish: --size ${JSON.stringify(SIZE)} is not WIDTHxHEIGHT (e.g. 640x360)`);
// Even dimensions, the same rule the native publisher applies, and a floor
// that still leaves room to draw the counter.
const W = Math.max(64, Number(m[1])) & ~1;
const H = Math.max(36, Number(m[2])) & ~1;

// ── The loopback guard ───────────────────────────────────────────────────────
// A test tool that can reach production is a test tool that eventually does.
function hostOf(url) {
  try {
    return new URL(url.replace(/^ws:/, "http:").replace(/^wss:/, "https:")).hostname;
  } catch {
    return "";
  }
}
const HOST = hostOf(SERVER);
const LOOPBACK = HOST === "127.0.0.1" || HOST === "localhost" || HOST === "::1" || HOST === "[::1]";
if (!HOST) die(1, `live-publish: --server ${JSON.stringify(SERVER)} does not name a host.`);
if (!LOOPBACK && !ALLOW_REMOTE) {
  die(
    1,
    `live-publish: REFUSED to publish to ${HOST}.`,
    "This is a test publisher; it only talks to a relay on this machine by default.",
    "Start a local one:",
    "  PORT=3399 DATABASE_PATH=<scratch>/relay.db target/release/HumanityOS.exe --headless",
    "If you really mean a remote relay, pass --allow-remote (never united-humanity.us:",
    "that server is the operator's live service and its streaming switch is their call).",
  );
}

// The WebSocket base: ws:// for http, wss:// for https.
const WS_BASE = SERVER.replace(/\/+$/, "").replace(/^https:/, "wss:").replace(/^http:/, "ws:");

// ── Identity: the repo's own Dilithium3 derivation ───────────────────────────
// Same chain as src/net/identity.rs::derive_pq_identity and
// web/shared/pq-identity.js: BLAKE3 with the context "hum/dilithium3/v1" over
// the 32-byte master seed gives the ML-DSA-65 keygen seed. scripts/pq-kat.mjs
// pins this to the Rust side; if it ever drifts, that KAT fails first.
const DIL_CONTEXT = "hum/dilithium3/v1";

async function loadNoble() {
  if (!fs.existsSync(BUNDLE)) {
    die(1, `live-publish: vendored PQ bundle missing: ${BUNDLE}`, "  run `just pq-vendor` to fetch it");
  }
  const n = await import(pathToFileURL(BUNDLE).href);
  if (!n.ml_dsa65 || !n.blake3) die(1, "live-publish: the vendored bundle has no ml_dsa65 / blake3 export");
  return n;
}

const hex = (b) => Buffer.from(b).toString("hex");

/** The 32-byte master seed: --seed, or one derived from the name so a rerun
 *  keeps the same throwaway identity (and therefore the same registered
 *  name) instead of burning a fresh one on every launch. */
function masterSeed(noble) {
  const given = String(opt("--seed", "")).trim();
  if (given) {
    if (!/^[0-9a-fA-F]{64}$/.test(given)) die(1, "live-publish: --seed must be 64 hex characters (32 bytes)");
    return new Uint8Array(Buffer.from(given, "hex"));
  }
  const ctx = new TextEncoder().encode("hum/live-publish/test-identity/v1");
  return noble.blake3
    .create({ context: ctx, dkLen: 32 })
    .update(new TextEncoder().encode(NAME))
    .digest();
}

function deriveIdentity(noble, master) {
  const ctx = new TextEncoder().encode(DIL_CONTEXT);
  const seed = noble.blake3.create({ context: ctx, dkLen: 32 }).update(master).digest();
  const kp = noble.ml_dsa65.keygen(seed);
  return {
    publicKeyHex: hex(kp.publicKey),
    // noble's argument order is sign(message, secretKey).
    sign: (msg) => noble.ml_dsa65.sign(msg, kp.secretKey),
  };
}

// ── The test pattern ─────────────────────────────────────────────────────────
// A 5x7 bitmap per digit, one bit per pixel, most significant bit at the
// left. Drawn by hand because a frame counter that cannot be read is not a
// frame counter, and pulling in a font renderer for ten glyphs is silly.
const DIGITS = {
  "0": [0x0e, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0e],
  "1": [0x04, 0x0c, 0x04, 0x04, 0x04, 0x04, 0x0e],
  "2": [0x0e, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1f],
  "3": [0x1f, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0e],
  "4": [0x02, 0x06, 0x0a, 0x12, 0x1f, 0x02, 0x02],
  "5": [0x1f, 0x10, 0x1e, 0x01, 0x01, 0x11, 0x0e],
  "6": [0x06, 0x08, 0x10, 0x1e, 0x11, 0x11, 0x0e],
  "7": [0x1f, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
  "8": [0x0e, 0x11, 0x11, 0x0e, 0x11, 0x11, 0x0e],
  "9": [0x0e, 0x11, 0x11, 0x0f, 0x01, 0x02, 0x0c],
};

/** Draw `text` (digits only) into an RGB buffer at (x0, y0), `scale` pixels
 *  per font pixel, in white on a dark halo so it reads over any background. */
function drawDigits(rgb, w, h, x0, y0, scale, text) {
  let cx = x0;
  for (const ch of text) {
    const glyph = DIGITS[ch];
    if (glyph) {
      for (let row = 0; row < 7; row++) {
        for (let col = 0; col < 5; col++) {
          const on = (glyph[row] >> (4 - col)) & 1;
          if (!on) continue;
          for (let sy = 0; sy < scale; sy++) {
            for (let sx = 0; sx < scale; sx++) {
              const px = cx + col * scale + sx;
              const py = y0 + row * scale + sy;
              if (px < 0 || py < 0 || px >= w || py >= h) continue;
              const i = (py * w + px) * 3;
              rgb[i] = 255;
              rgb[i + 1] = 255;
              rgb[i + 2] = 255;
            }
          }
        }
      }
    }
    cx += 6 * scale;
  }
}

/** One frame of the test pattern, as tightly-packed RGB8.
 *
 *  Three moving things, because the point of this pattern is that a rig can
 *  prove a wall screen is LIVE by diffing two snapshots a second apart:
 *    - a diagonal stripe field that slides (most pixels change every frame),
 *    - a bar sweeping left to right,
 *    - a disc orbiting the centre,
 *  plus the frame number in readable digits, for a human watching the wall. */
function renderFrame(w, h, n, fps) {
  const rgb = Buffer.alloc(w * h * 3);
  const t = n / fps; // seconds since the first frame
  const slide = t * 90; // stripe travel, pixels per second
  const cx = w / 2;
  const cy = h / 2;
  const orbitR = Math.min(w, h) * 0.3;
  const ox = cx + Math.cos(t * 1.3) * orbitR;
  const oy = cy + Math.sin(t * 1.3) * orbitR;
  const discR = Math.max(6, Math.min(w, h) * 0.08);
  const barX = (t * w * 0.45) % w;
  const barW = Math.max(4, w / 80);

  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const i = (y * w + x) * 3;
      // Sliding diagonal stripes over a vertical gradient.
      const band = (((x + y * 0.5 + slide) % 72) + 72) % 72;
      const lit = band < 36;
      const grad = y / h;
      let r = lit ? 30 + grad * 40 : 12 + grad * 18;
      let g = lit ? 70 + grad * 90 : 24 + grad * 34;
      let b = lit ? 120 + grad * 110 : 46 + grad * 54;
      // The sweeping bar.
      const dx = Math.abs(x - barX);
      if (dx < barW || dx > w - barW) {
        r = 250;
        g = 210;
        b = 90;
      }
      // The orbiting disc.
      const ddx = x - ox;
      const ddy = y - oy;
      if (ddx * ddx + ddy * ddy < discR * discR) {
        r = 240;
        g = 245;
        b = 255;
      }
      rgb[i] = r | 0;
      rgb[i + 1] = g | 0;
      rgb[i + 2] = b | 0;
    }
  }
  const scale = Math.max(2, Math.round(h / 48));
  drawDigits(rgb, w, h, scale * 2, scale * 2, scale, String(n));
  return rgb;
}

// ── The two sockets ──────────────────────────────────────────────────────────
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** Claim NAME on the relay over the ordinary chat socket, using the same
 *  two-phase Dilithium identify challenge every client uses (relay.rs:
 *  Identify -> IdentifyChallenge{nonce} -> IdentifyResponse{sig_b64}). The
 *  relay registers the name on a successful bind, and the publisher path
 *  then resolves the stream id from it. Resolves true if the name is ours. */
async function registerName(id) {
  const url = `${WS_BASE}/ws`;
  log(`claiming the name "${NAME}" on ${url}`);
  const ws = new WebSocket(url);
  return await new Promise((resolve) => {
    let settled = false;
    const done = (ok, why) => {
      if (settled) return;
      settled = true;
      try {
        ws.close();
      } catch {}
      if (!ok) console.error(`live-publish: could not claim "${NAME}": ${why}`);
      else log(`name claimed: ${why}`);
      resolve(ok);
    };
    const timer = setTimeout(() => done(false, "no answer within 20 s"), 20000);
    ws.addEventListener("open", () => {
      ws.send(JSON.stringify({ type: "identify", public_key: id.publicKeyHex, display_name: NAME }));
    });
    ws.addEventListener("error", (e) => {
      clearTimeout(timer);
      done(false, `socket error (${(e && e.message) || "no detail"}). Is the relay running on ${WS_BASE}?`);
    });
    ws.addEventListener("close", () => {
      clearTimeout(timer);
      done(false, "the relay closed the socket before binding");
    });
    ws.addEventListener("message", (ev) => {
      let msg;
      try {
        msg = JSON.parse(typeof ev.data === "string" ? ev.data : Buffer.from(ev.data).toString("utf8"));
      } catch {
        return;
      }
      if (msg.type === "identify_challenge") {
        // The exact preimage relay.rs builds before verifying.
        const preimage = new TextEncoder().encode(`hum/identify/v1\n${msg.nonce}\n${id.publicKeyHex}`);
        const sig = id.sign(preimage);
        ws.send(JSON.stringify({ type: "identify_response", sig_b64: Buffer.from(sig).toString("base64") }));
        return;
      }
      // A peer list is what the relay sends once the socket is bound: by then
      // the name is registered to this key.
      if (msg.type === "peer_list") {
        clearTimeout(timer);
        done(true, `bound as "${NAME}"`);
        return;
      }
      if (msg.type === "name_taken") {
        clearTimeout(timer);
        done(false, msg.message || "name taken");
      }
    });
  });
}

/** Open the publisher socket, authenticate, and return it once the relay has
 *  accepted the stream. Rejects with the relay's own reason. */
async function openPublisher(id) {
  const url = `${WS_BASE}/ws/live/pub`;
  log(`publishing to ${url}`);
  const ws = new WebSocket(url);
  ws.binaryType = "arraybuffer";
  const streamId = await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error("the relay did not answer the auth frame within 20 s")), 20000);
    ws.addEventListener("open", () => {
      const timestamp = Date.now();
      // The preimage the relay verifies: "live_publish\n<timestamp>", signed
      // with Dilithium3 and sent as HEX (src/relay/live.rs::authenticate ->
      // verify_dilithium_signature).
      const sig = id.sign(new TextEncoder().encode(`live_publish\n${timestamp}`));
      const frame = { key: id.publicKeyHex, timestamp, sig: hex(sig), title: TITLE, chat: CHAT };
      ws.send(JSON.stringify(frame));
    });
    ws.addEventListener("error", (e) => {
      clearTimeout(timer);
      reject(new Error(`socket error (${(e && e.message) || "no detail"}). Is the relay running on ${WS_BASE}?`));
    });
    ws.addEventListener("close", () => {
      clearTimeout(timer);
      reject(new Error("the relay closed the publisher socket"));
    });
    ws.addEventListener("message", (ev) => {
      let msg;
      try {
        msg = JSON.parse(typeof ev.data === "string" ? ev.data : Buffer.from(ev.data).toString("utf8"));
      } catch {
        return;
      }
      clearTimeout(timer);
      if (msg.ok === true) resolve(String(msg.stream || ""));
      else {
        reject(
          new Error(
            `the relay refused the stream: ${msg.error || "unknown"}` +
              (msg.error === "unauthorized"
                ? `\n  the key has no registered name on this relay. Run again with --register.`
                : ""),
          ),
        );
      }
    });
  });
  return { ws, streamId };
}

/** Wrap a JPEG in the wire envelope: [1B tag][8B PTS micros BE][payload]. */
function envelope(tag, ptsMicros, payload) {
  const head = Buffer.alloc(9);
  head[0] = tag;
  head.writeBigUInt64BE(BigInt(ptsMicros), 1);
  return Buffer.concat([head, payload]);
}
const TAG_KEYFRAME = 1;

async function main() {
  let sharp;
  try {
    sharp = require("sharp");
  } catch {
    die(
      1,
      "live-publish: sharp is not installed, and the frames have to become JPEGs somehow.",
      "  npm install        (sharp is already this repo's only devDependency)",
    );
  }

  const noble = await loadNoble();
  const id = deriveIdentity(noble, masterSeed(noble));
  log(`identity ${id.publicKeyHex.slice(0, 16)}... publishing as "${NAME}"`);

  if (REGISTER) {
    const ok = await registerName(id);
    if (!ok) process.exit(2);
    // The relay writes the registration inside the identify handler; give the
    // socket a moment to finish closing before the publisher asks for it.
    await sleep(300);
  }

  let pub;
  try {
    pub = await openPublisher(id);
  } catch (e) {
    console.error(`live-publish: ${e.message}`);
    process.exit(2);
  }
  const { ws, streamId } = pub;
  log(`live as "${streamId}": a wall screen whose source is watch:${streamId} will show this`);
  log(`${W}x${H} at ${FPS} fps, quality ${QUALITY}${SECONDS ? `, for ${SECONDS} s` : ", until stopped"}`);

  let sent = 0;
  let dropped = 0;
  let bytes = 0;
  let stop = false;
  const t0 = Date.now();
  const onSignal = () => {
    stop = true;
  };
  process.on("SIGINT", onSignal);
  process.on("SIGTERM", onSignal);
  ws.addEventListener("close", () => {
    stop = true;
  });

  const interval = 1000 / FPS;
  let n = 0;
  let nextAt = Date.now();
  while (!stop) {
    if (SECONDS && (Date.now() - t0) / 1000 >= SECONDS) break;
    // Same drop-to-present rule as the native publisher: if the socket is
    // already holding more than a couple of frames, skip this one rather than
    // build a queue of stale pictures.
    if (ws.bufferedAmount > 512 * 1024) {
      dropped++;
    } else {
      const rgb = renderFrame(W, H, n, FPS);
      const jpeg = await sharp(rgb, { raw: { width: W, height: H, channels: 3 } })
        .jpeg({ quality: QUALITY })
        .toBuffer();
      const msg = envelope(TAG_KEYFRAME, (Date.now() - t0) * 1000, jpeg);
      try {
        ws.send(msg);
      } catch (e) {
        console.error(`live-publish: send failed after ${sent} frame(s): ${e.message}`);
        break;
      }
      sent++;
      bytes += msg.length;
      if (!QUIET && (sent === 1 || sent % (FPS * 5) === 0)) {
        log(`${sent} frames, ${(bytes / 1024).toFixed(0)} KB, ${((Date.now() - t0) / 1000).toFixed(0)} s`);
      }
    }
    n++;
    nextAt += interval;
    const wait = nextAt - Date.now();
    // A slow encoder must not make the stream drift forever behind the clock.
    if (wait < -interval * 4) nextAt = Date.now();
    if (wait > 0) await sleep(wait);
  }

  try {
    ws.close();
  } catch {}
  const secs = (Date.now() - t0) / 1000;
  console.log(
    `live-publish: ${sent} frames in ${secs.toFixed(1)} s ` +
      `(${(sent / Math.max(secs, 0.001)).toFixed(1)} fps, ${((bytes * 8) / secs / 1000).toFixed(0)} kbps, ${dropped} dropped)`,
  );
  // Give the close handshake a moment so the relay sees a clean shutdown and
  // its viewers get a Close rather than a reset.
  await sleep(200);
  process.exit(0);
}

main().catch((e) => {
  console.error(`live-publish: ${(e && e.stack) || e}`);
  process.exit(2);
});
