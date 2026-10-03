#!/usr/bin/env node
// second-player: a scripted second player for the shared game world.
//
// WHY THIS EXISTS
// One person testing multiplayer needs to see ANOTHER player walking around,
// and until now that took a second human at a second computer. This script is
// that second person: it signs in to a relay exactly the way a real person's
// client does, steps into the shared world, and walks a path you choose
// (a circle or a back-and-forth line) at walking speed, sending its position
// fifteen times a second like the desktop app does. Open the game, enter the
// world, run this, and you should see someone walking.
//
// HOW IT SIGNS IN (the same way a person does)
// It is NOT a bot. Bots use a `bot_` key and a shared server password; this
// uses a real identity: a 32-byte master seed, from which it derives a
// Dilithium3 (ML-DSA-65) key pair exactly as the web client does
// (web/chat/pq.js: BLAKE3 derive_key("hum/dilithium3/v1", seed), then
// ML-DSA-65 keygen). Since v0.274.0 the relay asks every non-bot client to
// PROVE it holds its key (src/relay/relay.rs, the Identify arm):
//
//   client -> {"type":"identify", "public_key": <Dilithium3 public key hex>, ...}
//   relay  -> {"type":"identify_challenge", "nonce": <64 hex chars>}
//   client -> {"type":"identify_response", "sig_b64": <base64 signature over
//              the exact text "hum/identify/v1\n" + nonce + "\n" + public_key>}
//   relay  -> {"type":"peer_list", ...}   (you are in)
//
// The signing library is the one the chat client already ships, the
// vendored noble bundle web/shared/vendor/noble-pq.bundle.js. Its output is
// pinned to the Rust relay's by scripts/pq-kat.mjs, so no second crypto
// implementation is introduced here and nothing is installed.
//
// HOW IT MOVES (the same messages the desktop app sends)
// After signing in it sends `game_join` (src/lib.rs sends the same), waits
// for `game_welcome`, and then sends `game_position_update`
// fifteen times a second (lib.rs: `game_pos_timer >= 1.0 / 15.0`). Like the
// desktop app (src/engine/net_route.rs send_game_position, which sends its
// real velocity since 2026-10-02), it sends a REAL velocity: metres per
// second, worked out from its last step. Other clients use it to keep a
// figure moving smoothly between updates (src/net/sync.rs), so a real one
// matters.
//
// THE TIMESTAMP RULE (shared with the desktop app and every receiver): the
// `timestamp` of a game_position_update is the SENDER's own steady clock in
// SECONDS (a fractional number), counted from any start the sender likes;
// only the difference between two of one sender's stamps means anything. It
// advances by exactly the time step the movement used, so velocity =
// distance moved / stamp difference holds for every pair of updates. 0, or no
// field, means "no clock" (an older sender) and receivers then use their own
// arrival times. The relay passes the field on untouched. This script's clock
// is performance.now() in seconds, which never jumps when the wall clock is
// changed.
//
// The relay refuses any single update that moves more than 100 m
// (src/relay/handlers/msg_handlers.rs handle_game_position_update, "reject
// teleportation"), and every update after a refused one is measured from
// the old place, so they would all be refused until the path came back near
// it. So no step is ever longer than 90 m: not on the way from where the
// relay put us to the start of the path (which may be far), and not on the
// path itself after a pause (a slow computer, a console window held by a
// text selection), when a whole pause's worth of walking would otherwise
// arrive in one step.
//
// LOCAL RELAYS ONLY unless you pass --allow-remote. united-humanity.us is the
// operator's live service; putting a scripted figure in front of real people
// there is his decision, not a test script's.
//
// Usage:
//   node scripts/second-player.js                      walk a circle on ws://localhost:3210/ws
//   node scripts/second-player.js --path line --radius 6 --axis z
//   node scripts/second-player.js --center 10,1.7,-4 --seconds 60 --chat "hello"
//   node scripts/second-player.js --help               every option
//
// Stop it with Ctrl+C: it stands still, steps out of the world, and closes
// the connection. Exit 0 = finished or stopped. Exit 1 = bad options or
// refused before connecting. Exit 2 = the relay refused us or the connection
// failed.
//
// The same file is a small library: scripts/tests/second-player.test.js
// checks its paths, steps, clock and names without a relay, and
// scripts/tests/second-player-relay.test.js signs its own listening client
// in with the same code.

"use strict";

const fs = require("fs");
const path = require("path");
const { pathToFileURL } = require("url");
const { performance } = require("perf_hooks");

const REPO = path.resolve(__dirname, "..");
const BUNDLE = path.join(REPO, "web", "shared", "vendor", "noble-pq.bundle.js");

// ── Constants, each one copied from the place that decides it ───────────────

const DEFAULT_SERVER = "ws://localhost:3210/ws";
// The desktop app sends its position when `game_pos_timer >= 1.0 / 15.0`
// (src/lib.rs), so fifteen times a second.
const SEND_HZ = 15;
// The relay drops any update that moves more than 100 m in one go
// (handle_game_position_update). No step is longer than this, on the way to
// the path or on it (makeWalk), so we stay safely under it.
const MAX_STEP_M = 90;
// When the path starts somewhere other than where the relay put us, we get
// there in about this many seconds (never slower than walking speed).
const APPROACH_SECONDS = 4;
// A plain walking pace for a person, metres per second.
const DEFAULT_SPEED = 1.4;
const DEFAULT_RADIUS = 4;
// Names the relay accepts (relay.rs: letters, numbers, underscore, dash, at
// most 24). Checked here so a bad name fails before connecting.
const NAME_RULE = /^[A-Za-z0-9_-]{1,24}$/;
// The relay adds everyone who signs in to the server's member list for good,
// EXCEPT names starting AISampleBot, TestBot or SampleBot (relay.rs, the
// Identify arm, "is_test_bot_name"), which also get purged from the name
// register at every relay start (storage/members.rs purge_test_bot_members).
// That exemption exists because test clients kept coming back after the
// operator kicked them, so the default name carries the prefix.
const TEST_BOT_PREFIX = "TestBot";
const DEFAULT_NAME = `${TEST_BOT_PREFIX}Walker`;
// The domain strings: identical to web/chat/pq.js and src/relay/core/pq_crypto.rs.
const DIL_CONTEXT = "hum/dilithium3/v1";
const KYBER_CONTEXT = "hum/kyber768/v1";
// Ours alone: turns a --seed word (or the name) into a 32-byte master seed.
const SEED_CONTEXT = "hum/second-player/seed/v1";
// How the scripted player looks to others (src/player_look.rs): skin, hair,
// height. Fixed, so a person learns to recognise it.
const LOOK = { skin: [0.72, 0.53, 0.42], hair: [0.12, 0.08, 0.05], height: 1.0 };
const CHAT_CHANNEL = "general";

// ── Options ──────────────────────────────────────────────────────────────────

const HELP = `
second-player: a scripted second player that walks around the shared world.

  --server URL      relay to join (default ${DEFAULT_SERVER}).
                    http://host:port is accepted and turned into ws://host:port/ws.
  --name NAME       the name it signs in and appears under (default
                    ${DEFAULT_NAME}). Letters, numbers, _ and -, at most 24.
                    A name starting ${TEST_BOT_PREFIX} is kept off the server's
                    member list; any other name joins it for good.
  --seed TEXT       which identity to be. Any word or phrase gives the same
                    identity every run, so you can recognise it; 64 hex
                    characters are used as the raw 32-byte master seed;
                    "random" gives a brand-new identity (and, unless --name is
                    given, a name ending in a few characters of its key, since
                    a name already taken by another identity is refused).
                    Default: made from the name, so the same name is always
                    the same identity.
  --path circle|line  what to walk (default circle). A line goes back and
                    forth through the centre.
  --axis x|z        which way the line runs (default x).
  --center X,Y,Z    the middle of the path, in world metres. "auto" (the
                    default): when the relay gave us a plot of our own (it
                    hands every player one since increment 1b), the path
                    STARTS at our spawn on it, so we walk in our own home;
                    as a guest (the ship is full) it centres on the first
                    other player already in the world, or on our own spawn
                    point when nobody is there.
                    Y is eye height, like the desktop app sends.
  --radius M        circle radius, or half the line's length (default ${DEFAULT_RADIUS}).
  --speed M/S       walking speed (default ${DEFAULT_SPEED}).
  --seconds N       stop after N seconds of walking (default: until Ctrl+C).
  --chat TEXT       say one line in #${CHAT_CHANNEL} after joining (signed like a
                    person's message). A brand-new name has to wait a minute
                    before posting in public; it waits and tries again.
  --allow-remote    allow a relay that is not on this computer. Do not point
                    this at united-humanity.us without the operator's say-so.
`;

/** Read the command line into a plain options object. Throws an Error with a
 *  readable message for anything wrong, before any connection is made. */
function parseOptions(argv) {
  const known = new Set([
    "--server", "--name", "--seed", "--path", "--axis", "--center", "--radius",
    "--speed", "--seconds", "--chat",
  ]);
  const flags = new Set(["--allow-remote", "--help", "-h"]);
  const raw = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (flags.has(a)) {
      raw[a] = true;
    } else if (known.has(a)) {
      if (argv[i + 1] === undefined) throw new Error(`${a} needs a value`);
      raw[a] = argv[++i];
    } else {
      throw new Error(`unknown option "${a}" (try --help)`);
    }
  }
  if (raw["--help"] || raw["-h"]) return { help: true };

  const num = (flag, def, ok, rule) => {
    if (raw[flag] === undefined) return def;
    const v = Number(raw[flag]);
    if (!Number.isFinite(v) || !ok(v)) throw new Error(`${flag} must be ${rule}`);
    return v;
  };

  const name = String(raw["--name"] ?? DEFAULT_NAME).trim();
  if (!NAME_RULE.test(name)) {
    throw new Error("--name may only hold letters, numbers, _ and -, at most 24 characters");
  }
  const pathKind = String(raw["--path"] ?? "circle").toLowerCase();
  if (pathKind !== "circle" && pathKind !== "line") throw new Error("--path must be circle or line");
  const axis = String(raw["--axis"] ?? "x").toLowerCase();
  if (axis !== "x" && axis !== "z") throw new Error("--axis must be x or z");

  let center = "auto";
  if (raw["--center"] !== undefined && String(raw["--center"]).toLowerCase() !== "auto") {
    const parts = String(raw["--center"]).split(",").map((s) => Number(s.trim()));
    if (parts.length !== 3 || !parts.every(Number.isFinite)) {
      throw new Error('--center must be "auto" or three numbers like 10,1.7,-4');
    }
    center = parts;
  }

  return {
    help: false,
    server: normaliseServer(String(raw["--server"] ?? DEFAULT_SERVER)),
    name,
    nameGiven: raw["--name"] !== undefined,
    seed: raw["--seed"] === undefined ? null : String(raw["--seed"]),
    path: pathKind,
    axis,
    center,
    radius: num("--radius", DEFAULT_RADIUS, (v) => v > 0 && v <= 10000, "a number of metres above 0"),
    speed: num("--speed", DEFAULT_SPEED, (v) => v > 0 && v <= 100, "a number of metres per second above 0 and at most 100"),
    seconds: num("--seconds", 0, (v) => v >= 0, "0 or more"),
    chat: raw["--chat"] === undefined ? null : String(raw["--chat"]),
    allowRemote: !!raw["--allow-remote"],
  };
}

/** Accept ws://, wss://, http:// and https://, and add /ws when no path is
 *  given, so the same address works for this and for live-publish.js. */
function normaliseServer(s) {
  let u;
  try {
    u = new URL(s.trim());
  } catch {
    throw new Error(`--server "${s}" is not an address`);
  }
  if (u.protocol === "http:") u.protocol = "ws:";
  if (u.protocol === "https:") u.protocol = "wss:";
  if (u.protocol !== "ws:" && u.protocol !== "wss:") throw new Error(`--server must be ws:// or http://, got ${u.protocol}`);
  if (u.pathname === "/" || u.pathname === "") u.pathname = "/ws";
  return u.toString();
}

/** True for this computer: localhost, 127.x.x.x and ::1. */
function isLoopback(serverUrl) {
  const h = new URL(serverUrl).hostname.replace(/^\[|\]$/g, "").toLowerCase();
  return h === "localhost" || h === "::1" || /^127\./.test(h);
}

// ── Identity ─────────────────────────────────────────────────────────────────

let nobleModule = null;

/** Load the vendored post-quantum bundle the chat client ships. */
async function loadNoble() {
  if (nobleModule) return nobleModule;
  if (!fs.existsSync(BUNDLE)) {
    throw new Error(`the vendored post-quantum bundle is missing: ${BUNDLE} (run \`just pq-vendor\`)`);
  }
  // The bundle is an ES module in a .js file and this script is CommonJS, so
  // Node prints a four-line MODULE_TYPELESS_PACKAGE_JSON warning on import.
  // Drop that ONE warning (live-publish.js does the same); every other
  // warning still prints.
  // (Node 24 puts the name in `code`, older ones in `name`; check both.)
  process.removeAllListeners("warning");
  process.on("warning", (w) => {
    if (w && (w.code === "MODULE_TYPELESS_PACKAGE_JSON" || w.name === "MODULE_TYPELESS_PACKAGE_JSON")) return;
    // Printed the way Node prints a warning by default.
    console.error(`(node:${process.pid}) ${w && w.name ? `${w.name}: ${w.message}` : String(w)}`);
  });
  const n = await import(pathToFileURL(BUNDLE).href);
  if (!n.ml_dsa65 || !n.blake3 || !n.ml_kem768) throw new Error("the vendored bundle lacks ml_dsa65 / ml_kem768 / blake3");
  nobleModule = n;
  return n;
}

/** The 32-byte master seed (the thing a person's recovery phrase holds).
 *  - 64 hex characters: used as is.
 *  - "random": a fresh one, a new identity every run.
 *  - any other text, or nothing (then the name is used): hashed into one, so
 *    the same text always gives the same identity. */
function masterSeedFrom(noble, seedText, name) {
  if (seedText && /^[0-9a-fA-F]{64}$/.test(seedText.trim())) {
    return new Uint8Array(Buffer.from(seedText.trim(), "hex"));
  }
  if (seedText && seedText.trim().toLowerCase() === "random") {
    return new Uint8Array(require("crypto").randomBytes(32));
  }
  const text = seedText && seedText.trim() ? seedText.trim() : `name:${name}`;
  const ctx = new TextEncoder().encode(SEED_CONTEXT);
  return noble.blake3.create({ context: ctx, dkLen: 32 }).update(new TextEncoder().encode(text)).digest();
}

/** Derive the identity a person's client derives from the same seed:
 *  the Dilithium3 key (who you are, and what you sign with) and the Kyber768
 *  key (what others use to send you private messages). Same steps as
 *  web/chat/pq.js pqDeriveIdentity and pqDeriveKyber. */
function deriveIdentity(noble, master) {
  const enc = new TextEncoder();
  const dilSeed = noble.blake3.create({ context: enc.encode(DIL_CONTEXT), dkLen: 32 }).update(master).digest();
  const dil = noble.ml_dsa65.keygen(dilSeed);
  const kyberSeed = noble.blake3.create({ context: enc.encode(KYBER_CONTEXT), dkLen: 64 }).update(master).digest();
  const kyber = noble.ml_kem768.keygen(kyberSeed);
  return {
    publicKeyHex: Buffer.from(dil.publicKey).toString("hex"),
    kyberPublicB64: Buffer.from(kyber.publicKey).toString("base64"),
    // noble's argument order is sign(message, secretKey).
    sign: (bytes) => noble.ml_dsa65.sign(bytes, dil.secretKey),
  };
}

/** The name to sign in under. Normally the one asked for (or the default).
 *  With `--seed random` and no `--name`, a dash and the first six characters
 *  of the new key are added (TestBotWalker-3fa9c1): the relay ties a name to
 *  the first identity that used it ("already registered to another
 *  identity"), so a second random run under the plain default name would be
 *  refused. Stays inside the relay's 24-character limit. */
function nameFor(opts, publicKeyHex) {
  const isRandom = !!opts.seed && opts.seed.trim().toLowerCase() === "random";
  if (!isRandom || opts.nameGiven) return opts.name;
  const suffix = `-${publicKeyHex.slice(0, 6)}`;
  return `${opts.name.slice(0, 24 - suffix.length)}${suffix}`;
}

/** The exact text the relay checks the sign-in signature against
 *  (relay.rs: format!("hum/identify/v1\n{}\n{}", nonce, public_key)). */
function identifyPreimage(nonce, publicKeyHex) {
  return `hum/identify/v1\n${nonce}\n${publicKeyHex}`;
}

/** The exact text the relay checks a chat signature against
 *  (broadcast.rs verify_dilithium_signature: "{content}\n{timestamp}"),
 *  the same as web/chat/crypto.js pqSignChatMessage. */
function chatPreimage(content, timestamp) {
  return `${content}\n${timestamp}`;
}

/** A chat message the relay will accept: signed with Dilithium3, the
 *  signature sent as hex in `pq_signature` (as the web client does). */
function buildChatMessage(identity, name, content, timestamp, channel = CHAT_CHANNEL) {
  const sig = identity.sign(new TextEncoder().encode(chatPreimage(content, timestamp)));
  return {
    type: "chat",
    from: identity.publicKeyHex,
    from_name: name,
    content,
    timestamp,
    channel,
    pq_signature: Buffer.from(sig).toString("hex"),
  };
}

// ── The connection ───────────────────────────────────────────────────────────

/** Wrap a WebSocket: JSON in and out, and game messages unwrapped. The relay
 *  delivers game messages as {"type":"system","message":"__game__:{...}"}
 *  (a private one is turned into the same shape before it is sent). */
function wrapSocket(ws) {
  const listeners = new Set();
  const client = {
    ws,
    /** Send one JSON message (dropped quietly if the socket has closed). */
    send(obj) {
      if (ws.readyState === 1) ws.send(JSON.stringify(obj));
    },
    /** Call fn(msg, game) for every message; game is the unwrapped game
     *  payload or null. Returns a function that removes the listener. */
    on(fn) {
      listeners.add(fn);
      return () => listeners.delete(fn);
    },
    /** Call fn(game) for every game message only. */
    onGame(fn) {
      return client.on((_m, g) => g && fn(g));
    },
    close() {
      try {
        ws.close();
      } catch {}
    },
  };
  ws.addEventListener("message", (ev) => {
    let msg;
    try {
      msg = JSON.parse(typeof ev.data === "string" ? ev.data : Buffer.from(ev.data).toString("utf8"));
    } catch {
      return;
    }
    let game = null;
    if (msg && msg.type === "system" && typeof msg.message === "string" && msg.message.startsWith("__game__:")) {
      try {
        game = JSON.parse(msg.message.slice("__game__:".length));
      } catch {}
    }
    for (const fn of [...listeners]) fn(msg, game);
  });
  return client;
}

/** Open a socket and sign in as `identity` under `name`, answering the
 *  relay's challenge. Resolves with the wrapped client once the relay lets
 *  us in (its `peer_list`); rejects with the relay's own reason otherwise. */
function signIn(serverUrl, identity, name, { timeoutMs = 20000 } = {}) {
  if (typeof WebSocket === "undefined") {
    return Promise.reject(new Error("this Node has no built-in WebSocket; use Node 22 or newer"));
  }
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(serverUrl);
    const client = wrapSocket(ws);
    let settled = false;
    const finish = (err) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      off();
      if (err) {
        client.close();
        reject(err);
      } else {
        resolve(client);
      }
    };
    const timer = setTimeout(() => finish(new Error(`the relay did not let us in within ${timeoutMs / 1000} s`)), timeoutMs);

    ws.addEventListener("open", () => {
      client.send({
        type: "identify",
        public_key: identity.publicKeyHex,
        display_name: name,
        kyber_public: identity.kyberPublicB64,
      });
    });
    ws.addEventListener("error", (e) => {
      finish(new Error(`could not reach ${serverUrl} (${(e && e.message) || "connection failed"}). Is a relay running there?`));
    });
    ws.addEventListener("close", () => finish(new Error("the relay closed the connection before letting us in")));

    const off = client.on((msg) => {
      if (msg.type === "identify_challenge" && typeof msg.nonce === "string") {
        // Prove we hold the key: sign the exact text the relay will check.
        const sig = identity.sign(new TextEncoder().encode(identifyPreimage(msg.nonce, identity.publicKeyHex)));
        client.send({ type: "identify_response", sig_b64: Buffer.from(sig).toString("base64") });
      } else if (msg.type === "peer_list") {
        // The relay sends the peer list once the socket is bound: we are in.
        finish(null);
      } else if (msg.type === "name_taken") {
        finish(new Error(`the relay refused the name: ${msg.message || "taken"}`));
      } else if (msg.type === "system" && typeof msg.message === "string") {
        // The relay's refusals during sign-in arrive as plain system text.
        if (/verification failed|Too many|authentication failed|banned/i.test(msg.message)) {
          finish(new Error(`the relay refused the sign-in: ${msg.message}`));
        }
      }
    });
  });
}

/** Step into the shared world. Resolves with the relay's `game_welcome`
 *  (our entity id, and everyone already there). */
function joinWorld(client, name, { look = LOOK, timeoutMs = 15000 } = {}) {
  return new Promise((resolve, reject) => {
    let off = () => {};
    const timer = setTimeout(() => {
      off();
      reject(new Error(`no game_welcome within ${timeoutMs / 1000} s`));
    }, timeoutMs);
    off = client.on((msg, game) => {
      const done = (err, value) => {
        clearTimeout(timer);
        off();
        if (err) reject(err);
        else resolve(value);
      };
      if (game && game.type === "game_welcome") done(null, game);
      else if (game && game.type === "game_join_denied") done(new Error(`the relay refused to let us into the world: ${game.message || game.reason}`));
      else if (msg.type === "system" && /has 'game' disabled/.test(msg.message || "")) done(new Error(msg.message));
    });
    // The same message the desktop app sends (src/lib.rs, "game_join").
    client.send({ type: "game_join", player_name: name, character_mode: "local", appearance: look });
  });
}

// ── Paths and facing ─────────────────────────────────────────────────────────

/** The point `s` metres along the path. The path never ends: a circle goes
 *  round and round, a line goes back and forth.
 *  - circle: starts at centre + (radius, 0, 0), turning from +X towards +Z.
 *  - line: starts at centre - radius along the axis, walks to + radius, and back. */
function pathPoint(plan, s) {
  const [cx, cy, cz] = plan.center;
  const r = plan.radius;
  if (plan.path === "line") {
    const u = ((s % (4 * r)) + 4 * r) % (4 * r);
    const offset = u <= 2 * r ? u - r : 3 * r - u;
    return plan.axis === "z" ? [cx, cy, cz + offset] : [cx + offset, cy, cz];
  }
  const a = s / r;
  return [cx + r * Math.cos(a), cy, cz + r * Math.sin(a)];
}

/** The rotation that faces a direction of travel on the ground, in the
 *  desktop app's own encoding (net_route.rs send_game_position): a turn
 *  about the vertical axis by the camera's yaw, [0, sin(yaw/2), 0, cos(yaw/2)],
 *  where a camera at that yaw looks along (sin yaw, 0, -cos yaw)
 *  (renderer/camera.rs forward). So a desktop player walking the same way
 *  would send this same rotation. */
function facingQuat(vx, vz) {
  const yaw = Math.atan2(vx, -vz);
  return [0, Math.sin(yaw / 2), 0, Math.cos(yaw / 2)];
}

/** Where to centre the path: the --center given; or (auto) with a plot of our
 *  own, the centre that makes the path START at our spawn on it (a line runs
 *  on from there along its axis, a circle starts at centre + (radius, 0, 0));
 *  or, as a guest, the first other player already in the world, or our own
 *  spawn point. Starting at the spawn means no approach walk at all, and a
 *  walk that stays in our own home: another player's spot is inside THEIR
 *  home since increment 1b. */
function chooseCenter(opts, welcome) {
  const snap = Array.isArray(welcome.world_snapshot) ? welcome.world_snapshot : [];
  const me = snap.find((e) => e.entity_id === welcome.player_id);
  const start = me && Array.isArray(me.position) ? me.position.slice(0, 3) : [0, 1, 0];
  if (opts.center !== "auto") return { center: opts.center, start, why: "as asked" };
  if (welcome.home_plot) {
    const r = opts.radius ?? DEFAULT_RADIUS;
    let center;
    if (opts.path === "line") center = opts.axis === "z" ? [start[0], start[1], start[2] + r] : [start[0] + r, start[1], start[2]];
    else center = [start[0] - r, start[1], start[2]];
    return { center, start, why: `starting at our own spawn on plot ${welcome.home_plot.id}` };
  }
  const other = snap.find((e) => e.entity_type === "player" && e.entity_id !== welcome.player_id && Array.isArray(e.position));
  if (other) {
    const who = (other.components && other.components.name) || `player ${other.entity_id}`;
    return { center: other.position.slice(0, 3), start, why: `around ${who}` };
  }
  return { center: start, start, why: "around our own spawn point (nobody else is in the world)" };
}

const fmt = (p) => `(${p.map((v) => v.toFixed(2)).join(", ")})`;

/** The plot the relay gave us, as the one line the log carries (and
 *  scripts/verify-copresence.js reads): "home_plot {json}" with the welcome's
 *  {id, kind, origin, size}, or "home_plot null" for a guest (the ship is
 *  full), or "home_plot missing" when the welcome has no such field (a relay
 *  from before increment 1b). */
function homePlotLine(welcome) {
  if (!welcome || !("home_plot" in welcome)) return "home_plot missing (this relay hands out no plots)";
  if (welcome.home_plot === null) return "home_plot null (the ship is full: a guest in the Commons)";
  return `home_plot ${JSON.stringify(welcome.home_plot)}`;
}

/** The sender's own steady clock, in seconds (THE TIMESTAMP RULE, top of
 *  this file). performance.now() counts from this process's start and never
 *  jumps when the wall clock is changed. Kept above zero, since a stamp of 0
 *  means "no clock". */
const senderClockS = () => Math.max(performance.now() / 1000, 0.001);

/** The walk itself, with no timers and no socket, so a test can drive it
 *  with a made-up clock. `startS` is the sender's clock (seconds) at the
 *  first update. Every update it hands back is a complete
 *  game_position_update message.
 *
 *  - here(): standing still where we are, stamped with the current clock.
 *    Sent once before the first step.
 *  - next(nowS): move on by the time since the last update and return
 *    { msg, reachedPath }. The clock advances by exactly the time step the
 *    movement used, so msg.velocity = distance moved / stamp difference.
 *  - stopped(nowS): one last update standing still (velocity zero), so
 *    nobody's screen keeps sliding the figure on along its last velocity. */
function makeWalk(plan, start, startS) {
  let pos = start.slice();
  let facing = [0, 0, 0, 1];
  let travelled = 0; // metres along the path, once on it
  let clockS = startS; // the timestamp of the last update handed out
  const target = pathPoint(plan, 0);
  const gap = Math.hypot(target[0] - pos[0], target[1] - pos[1], target[2] - pos[2]);
  let onPath = gap < 0.01;
  const approachSpeed = Math.max(plan.speed, gap / APPROACH_SECONDS);

  const update = (velocity) => ({
    type: "game_position_update",
    position: pos.slice(),
    rotation: facing.slice(),
    velocity,
    timestamp: clockS, // the sender clock, SECONDS (THE TIMESTAMP RULE)
  });

  return {
    gap,
    target,
    onPath: () => onPath,
    position: () => pos.slice(),
    here: () => update([0, 0, 0]),
    next(nowS) {
      // The time step: how long since the last update, by the same clock the
      // stamps come from. (At least a millisecond, so a velocity never
      // divides by zero; the stamp then moves on by that same millisecond.)
      const dt = Math.max(0.001, nowS - clockS);
      clockS += dt;
      let next;
      let reachedPath = false;
      if (!onPath) {
        const d = Math.hypot(target[0] - pos[0], target[1] - pos[1], target[2] - pos[2]);
        const step = Math.min(approachSpeed * dt, MAX_STEP_M);
        if (d <= step) {
          next = target.slice();
          onPath = true;
          reachedPath = true;
        } else {
          next = pos.map((v, i) => v + ((target[i] - v) * step) / d);
        }
      } else {
        // At most MAX_STEP_M along the path per update. Along a circle or a
        // line, the straight distance between two points is never more than
        // the distance walked between them, so the step is at most 90 m too,
        // under the relay's 100 m limit even after a long pause.
        travelled += Math.min(plan.speed * dt, MAX_STEP_M);
        next = pathPoint(plan, travelled);
      }
      // The velocity is the real one: how far this step went, over the same
      // time step the stamp moved on by.
      const velocity = next.map((v, i) => (v - pos[i]) / dt);
      pos = next;
      if (Math.hypot(velocity[0], velocity[2]) > 1e-3) facing = facingQuat(velocity[0], velocity[2]);
      return { msg: update(velocity), reachedPath };
    },
    stopped(nowS) {
      // Stamped later than the last update, never at the same instant.
      clockS = Math.max(nowS, clockS + 0.001);
      return update([0, 0, 0]);
    },
  };
}

/** Walk `plan` from `start`, sending game_position_update SEND_HZ times a
 *  second. Returns { stop(), position(), sent() }. stop() sends one last
 *  update standing still (makeWalk's stopped()). */
function startWalking(client, plan, start, log) {
  let sent = 0;
  const walk = makeWalk(plan, start, senderClockS());
  if (!walk.onPath()) log(`walking ${walk.gap.toFixed(1)} m to the start of the path ${fmt(walk.target)}`);
  else log(`on the path at ${fmt(start)}`);

  const send = (msg) => {
    client.send(msg);
    sent++;
  };

  // "Here I am", standing still, before the first step.
  send(walk.here());

  const tick = () => {
    const { msg, reachedPath } = walk.next(senderClockS());
    if (reachedPath) log(`on the path at ${fmt(msg.position)}`);
    send(msg);
  };

  const timer = setInterval(tick, 1000 / SEND_HZ);
  const status = setInterval(() => log(`walking: ${sent} updates sent, now at ${fmt(walk.position())}`), 15000);

  return {
    position: () => walk.position(),
    sent: () => sent,
    stop() {
      clearInterval(timer);
      clearInterval(status);
      send(walk.stopped(senderClockS()));
    },
  };
}

// ── Saying one line in chat ──────────────────────────────────────────────────

/** Post `text` in #general, signed. A brand-new name has to wait before
 *  posting in public (relay.rs NEW_IDENTITY_GRACE_SECS); when the relay says
 *  so, wait the time it names and try once more. */
function sayInChat(client, identity, name, text, log) {
  let retried = false;
  let retryTimer = null;
  const attempt = () => client.send(buildChatMessage(identity, name, text, Date.now()));
  const off = client.on((msg) => {
    if (msg.type === "chat" && msg.from === identity.publicKeyHex && msg.content === text) {
      log(`said in #${CHAT_CHANNEL}: "${text}"`);
      off();
    } else if (msg.type === "system" && typeof msg.message === "string") {
      const wait = msg.message.match(/New accounts wait .*Try again in (\d+)s/);
      if (wait && !retried) {
        retried = true;
        const secs = Number(wait[1]) + 1;
        log(`the relay makes a new name wait before posting in public; trying again in ${secs} s`);
        retryTimer = setTimeout(attempt, secs * 1000);
      } else if (/^Message rejected/.test(msg.message)) {
        log(`chat refused: ${msg.message}`);
        off();
      }
    }
  });
  attempt();
  return () => {
    off();
    clearTimeout(retryTimer);
  };
}

// ── The program ──────────────────────────────────────────────────────────────

async function main() {
  const log = (s) => console.log(`second-player: ${s}`);
  const fail = (code, s) => {
    console.error(`second-player: ${s}`);
    process.exitCode = code;
  };

  let opts;
  try {
    opts = parseOptions(process.argv.slice(2));
  } catch (e) {
    return fail(1, `${e.message}`);
  }
  if (opts.help) {
    console.log(HELP);
    return;
  }
  if (!opts.allowRemote && !isLoopback(opts.server)) {
    return fail(1, `${opts.server} is not on this computer. Pass --allow-remote if you really mean it.`);
  }

  let noble;
  try {
    noble = await loadNoble();
  } catch (e) {
    return fail(1, e.message);
  }
  const identity = deriveIdentity(noble, masterSeedFrom(noble, opts.seed, opts.name));
  // From here on opts.name is the name actually used (see nameFor).
  opts.name = nameFor(opts, identity.publicKeyHex);
  log(`connecting to ${opts.server} as "${opts.name}" (key ${identity.publicKeyHex.slice(0, 16)}...)`);
  if (!opts.name.startsWith(TEST_BOT_PREFIX)) {
    log(`note: "${opts.name}" does not start with ${TEST_BOT_PREFIX}, so the relay adds it to its member list for good`);
  }

  let client;
  try {
    client = await signIn(opts.server, identity, opts.name);
  } catch (e) {
    return fail(2, e.message);
  }
  log("signed in: the relay accepted our proof of key");

  let welcome;
  try {
    welcome = await joinWorld(client, opts.name);
  } catch (e) {
    client.close();
    return fail(2, e.message);
  }
  const { center, start, why } = chooseCenter(opts, welcome);
  log(`in the world as entity ${welcome.player_id}, starting at ${fmt(start)}`);
  log(homePlotLine(welcome));
  const shape = opts.path === "line"
    ? `back and forth along a ${2 * opts.radius} m line (${opts.axis} axis)`
    : `a circle of radius ${opts.radius} m`;
  log(`walking ${shape} at ${opts.speed} m/s, centred on ${fmt(center)} ${why}`);

  const plan = { path: opts.path, axis: opts.axis, center, radius: opts.radius, speed: opts.speed };
  const walker = startWalking(client, plan, start, log);
  const stopChat = opts.chat ? sayInChat(client, identity, opts.name, opts.chat, log) : () => {};

  let stopping = false;
  let limitTimer = null;
  const finish = async (reason, code = 0) => {
    if (stopping) return;
    stopping = true;
    clearTimeout(limitTimer);
    stopChat();
    walker.stop();
    // Step out of the world on purpose (the desktop app sends the same),
    // then give the socket a moment to deliver both before closing it.
    client.send({ type: "game_leave" });
    await new Promise((r) => setTimeout(r, 300));
    client.close();
    log(`${reason}: stood still and left the world after ${walker.sent()} updates`);
    process.exitCode = code;
    // Everything is closed, so Node exits by itself; this is only a backstop.
    setTimeout(() => process.exit(code), 3000).unref();
  };

  if (opts.seconds > 0) limitTimer = setTimeout(() => finish(`${opts.seconds} s are up`), opts.seconds * 1000);
  process.on("SIGINT", () => {
    if (stopping) process.exit(130); // a second Ctrl+C: stop at once
    finish("stopped (Ctrl+C)");
  });
  client.ws.addEventListener("close", () => {
    if (stopping) return;
    stopping = true;
    walker.stop();
    clearTimeout(limitTimer);
    stopChat();
    fail(2, `the relay closed the connection after ${walker.sent()} updates`);
    setTimeout(() => process.exit(2), 1000).unref();
  });
}

module.exports = {
  SEND_HZ,
  MAX_STEP_M,
  DEFAULT_NAME,
  TEST_BOT_PREFIX,
  NAME_RULE,
  parseOptions,
  nameFor,
  makeWalk,
  senderClockS,
  normaliseServer,
  isLoopback,
  loadNoble,
  masterSeedFrom,
  deriveIdentity,
  identifyPreimage,
  chatPreimage,
  buildChatMessage,
  signIn,
  joinWorld,
  pathPoint,
  facingQuat,
  chooseCenter,
  homePlotLine,
};

if (require.main === module) main();
