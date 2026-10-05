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
// HOW IT BUILDS (ship homes increment 5, 2026-10-05: building only on your own
// plot). A line on its input asks it to build or take down a piece in the
// shared world, the way the desktop app does when a player presses E or F there
// (src/systems/construction/shared.rs, the contract both the relay and the game
// compile):
//   build wood_wall@plot:p1:46,0.2,36,1    game_build: the blueprint, its frame
//                                         (plot:<id> or zone:<id>), x,y,z metres
//                                         from the frame's corner, quarter turns
//   unbuild 12                            game_unbuild: take piece 12 down
//   permit p1 did:hum:... 30              mint a household permit for p1 (it must
//                                         be our plot) for that person, 30 days,
//                                         good on this relay only (its own did:hum,
//                                         from /api/server-info, is in the words)
//   ... permit {json}                     either of the first two, carrying a permit
// It sends the pose as the relay's checks want it: the turn exactly as the
// game's placement makes it (`quarterTurn`, placement.rs `quarter_turn`) and
// the size the blueprint's own, read from data/blueprints/basic.ron (the file
// the relay reads). What the relay answers is logged one line each
// (`logBuilds`), in the patterns this module exports, which
// scripts/verify-copresence.js --build reads.
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
// The relay checks every update against how far anyone can go in the time since
// (ship homes increment 4, src/relay/handlers/move_check.rs, with the rules in
// data/ship/shared_world.ron): a player may bank at most 46.9 m of allowance
// (25 m/s on foot, with a quarter's margin, kept 1.5 s) and it builds at
// 31.25 m/s; a move past that is CORRECTED, not taken. So no step is longer than
// this, even after a long pause (makeWalk), and none goes faster than
// MAX_SPEED_MPS. (Until increment 4 the rule was 100 m an update, and this was 90.)
const MAX_STEP_M = 30;
// The fastest the walker goes, m/s: under the relay's 25 m/s on foot.
const MAX_SPEED_MPS = 20;
// When the path starts somewhere other than where the relay put us, we get
// there in about this many seconds (never slower than walking speed, never
// faster than MAX_SPEED_MPS).
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
  --path circle|line|still  what to walk (default circle). A line goes back
                    and forth through the centre; still stands where the relay
                    put it (a builder, verify-copresence --build).
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
  --route "X,Y,Z;X,Y,Z;..."  walk THROUGH these points, in order, before the
                    path starts (a way out of your home, through its door and
                    corridor, into the Commons). Without it the walk goes
                    straight to the start of the path in about ${APPROACH_SECONDS} s.
  --route-speed M/S how fast the route is walked (default: --speed).
  --home-spawn X,Z  your home's door, in metres from your plot's corner (what
                    the desktop app names in its join): the relay spawns you
                    there on your plot instead of in its middle.
  --chat TEXT       say one line in #${CHAT_CHANNEL} after joining (signed like a
                    person's message). A brand-new name has to wait a minute
                    before posting in public; it waits and tries again.
  --forwarded-for IP  the address this connection says it comes from, in the
                    X-Forwarded-For header nginx writes in front of a live
                    relay. A relay signs up at most five NEW accounts an hour
                    from one address, and every local connection without the
                    header shares one; a rig filling a ship with households
                    (verify-copresence --plots --order guest) gives each its
                    own. Local relays only, like everything here.
  --allow-remote    allow a relay that is not on this computer. Do not point
                    this at united-humanity.us without the operator's say-so.

A line "stop" on its input stops it the way Ctrl+C does (a rig's way to end it
gracefully: Windows gives a child process no signal to catch).

Lines on its input also build in the shared world (ship homes increment 5):
  build <blueprint>@<frame>:<x>,<y>,<z>,<turns>   e.g. build wood_wall@plot:p1:46,0.2,36,1
                    (frame plot:<id> or zone:<id>; x,y,z metres from its corner;
                    turns 0 to 3 quarter turns about the vertical)
  unbuild <piece number>                          take a piece down
  permit <plot id> <grantee did:hum> <days>       mint a household permit (90 days at most),
                    good on this relay only: it names the relay's own did:hum
and a build or an unbuild may end with "permit <the JSON a permit line logged>".
What the relay answers is logged, one line each.
`;

/** Read the command line into a plain options object. Throws an Error with a
 *  readable message for anything wrong, before any connection is made. */
function parseOptions(argv) {
  const known = new Set([
    "--server", "--name", "--seed", "--path", "--axis", "--center", "--radius",
    "--speed", "--seconds", "--chat", "--route", "--route-speed", "--home-spawn",
    "--forwarded-for",
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
  if (pathKind !== "circle" && pathKind !== "line" && pathKind !== "still") throw new Error("--path must be circle, line or still");
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

  // The route: points walked through in order before the path (increment 2 of
  // docs/design/ship-homes-and-logistics.md: from your door, through your
  // corridor, into the Commons).
  let route = [];
  if (raw["--route"] !== undefined && String(raw["--route"]).trim() !== "") {
    route = String(raw["--route"]).split(";").map((pt) => pt.split(",").map((s) => Number(s.trim())));
    if (!route.every((p) => p.length === 3 && p.every(Number.isFinite))) {
      throw new Error('--route must be points like "54,1.7,40;66,1.7,40" (three numbers each, ; between points)');
    }
  }
  let homeSpawn = null;
  if (raw["--home-spawn"] !== undefined) {
    homeSpawn = String(raw["--home-spawn"]).split(",").map((s) => Number(s.trim()));
    if (homeSpawn.length !== 2 || !homeSpawn.every(Number.isFinite)) throw new Error("--home-spawn must be two numbers like 53.5,40.5");
  }
  let forwardedFor = null;
  if (raw["--forwarded-for"] !== undefined) {
    forwardedFor = String(raw["--forwarded-for"]).trim();
    // An IPv4 or IPv6 address and nothing else: it goes into a header as it stands.
    if (!/^[0-9A-Fa-f.:]{2,45}$/.test(forwardedFor)) throw new Error("--forwarded-for must be an address like 10.77.0.3");
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
    route,
    routeSpeed: num("--route-speed", null, (v) => v > 0 && v <= 100, "a number of metres per second above 0 and at most 100"),
    homeSpawn,
    forwardedFor,
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

/** The web address of a relay given by its socket address: ws://host:port/ws
 *  becomes http://host:port (wss becomes https). Pure. */
function httpBase(serverUrl) {
  const u = new URL(serverUrl);
  u.protocol = u.protocol === "wss:" ? "https:" : "http:";
  return u.origin;
}

/** The ship the relay holds plots on, { id, hash }, from its public
 *  /api/server-info (ship homes increment 1b). A join naming this ship holds a
 *  plot of it, as the desktop app's does; one naming none is a guest in the
 *  Commons. Null when the relay names no ship (its ship did not load, or it is
 *  older than 1b). Rejects when the relay cannot be asked. */
async function fetchShip(serverUrl, { timeoutMs = 5000 } = {}) {
  const res = await fetch(`${httpBase(serverUrl)}/api/server-info`, { signal: AbortSignal.timeout(timeoutMs) });
  if (!res.ok) throw new Error(`/api/server-info answered ${res.status}`);
  const info = await res.json();
  const ship = info && info.ship;
  return ship && typeof ship.hash === "string" && ship.hash ? { id: String(ship.id || ""), hash: ship.hash } : null;
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
 *  us in (its `peer_list`); rejects with the relay's own reason otherwise.
 *  `forwardedFor`: the address this connection says it comes from
 *  (X-Forwarded-For, the --forwarded-for option); Node's WebSocket takes
 *  headers in its second argument. */
function signIn(serverUrl, identity, name, { timeoutMs = 20000, forwardedFor = null } = {}) {
  if (typeof WebSocket === "undefined") {
    return Promise.reject(new Error("this Node has no built-in WebSocket; use Node 22 or newer"));
  }
  return new Promise((resolve, reject) => {
    const ws = forwardedFor ? new WebSocket(serverUrl, { headers: { "x-forwarded-for": forwardedFor } }) : new WebSocket(serverUrl);
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

/** The `game_join` this script sends: the desktop app's (src/lib.rs,
 *  "game_join"), naming the relay's ship when `ship` is given so the join holds
 *  a plot (engine/home_plot.rs `add_join_fields`). A scripted player draws no
 *  home, so by default it names no door and arrives in the middle of its plot;
 *  `homeSpawn` ([x, z], plot-local metres, `--home-spawn`) names one, as a
 *  desktop player with that home does, and the relay spawns it at that door
 *  (increment 2: the walker then walks out through its own corridor). Pure. */
function joinMessage(name, look, ship, homeSpawn = null) {
  const msg = { type: "game_join", player_name: name, character_mode: "local", appearance: look };
  if (ship && ship.hash) msg.ship_hash = ship.hash;
  if (ship && ship.hash && Array.isArray(homeSpawn)) msg.home_spawn = [homeSpawn[0], homeSpawn[1]];
  return msg;
}

/** Step into the shared world. Resolves with the relay's `game_welcome`
 *  (our entity id, and everyone already there). `ship`: the relay's ship
 *  (`fetchShip`), named in the join so it holds a plot. */
function joinWorld(client, name, { look = LOOK, timeoutMs = 15000, ship = null, homeSpawn = null } = {}) {
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
    client.send(joinMessage(name, look, ship, homeSpawn));
  });
}

/** Log the OTHER players the relay tells us about, for a rig to read
 *  (scripts/verify-copresence.js --plots, the step-out-and-back check):
 *    player joined: entity N "name" at (x, y, z)   where the relay spawned them
 *    player left: entity N
 *    saw entity N at (x, y, z)                     an update the relay passed on
 *  An update is logged when it is the first since that player joined, when it
 *  is a quarter metre or more from the last one logged, or two seconds after
 *  it: enough to see every move, never 15 lines a second. `me` is our own
 *  entity id (never logged). Returns the function that stops it. */
function logOthers(client, me, log, { minMoveM = 0.25, everyS = 2 } = {}) {
  const last = new Map(); // entity id -> { pos, at }
  return client.onGame((g) => {
    if (g.type === "game_player_joined" && g.player_id !== me) {
      last.delete(g.player_id);
      log(`player joined: entity ${g.player_id} "${g.name || ""}" at ${fmt(g.position || [0, 0, 0])}`);
    } else if (g.type === "game_player_left" && g.player_id !== me) {
      last.delete(g.player_id);
      log(`player left: entity ${g.player_id}`);
    } else if (g.type === "game_in_view" && g.entity && g.entity.entity_type === "player" && g.entity.entity_id !== me) {
      // Another player came into our view (increment 4: the relay sends moves only to the players
      // who have the mover in view, src/relay/handlers/game_interest.rs).
      last.delete(g.entity.entity_id);
      log(`in view: entity ${g.entity.entity_id} "${(g.entity.components && g.entity.components.name) || ""}" at ${fmt((g.entity.position || [0, 0, 0]).map(Number))}`);
    } else if (g.type === "game_out_of_view" && g.entity_id !== me) {
      last.delete(g.entity_id);
      log(`out of view: entity ${g.entity_id}`);
    } else if (g.type === "game_position_update" && g.player_id !== me && Array.isArray(g.position)) {
      const prev = last.get(g.player_id);
      const now = Date.now();
      const moved = prev ? Math.hypot(...g.position.map((v, i) => v - prev.pos[i])) : Infinity;
      if (!prev || moved >= minMoveM || now - prev.at >= everyS * 1000) {
        last.set(g.player_id, { pos: g.position.slice(0, 3), at: now });
        log(`saw entity ${g.player_id} at ${fmt(g.position)}`);
      }
    }
  });
}

// ── Paths and facing ─────────────────────────────────────────────────────────

/** The point `s` metres along the path. The path never ends: a circle goes
 *  round and round, a line goes back and forth.
 *  - circle: starts at centre + (radius, 0, 0), turning from +X towards +Z.
 *  - line: starts at centre - radius along the axis, walks to + radius, and back. */
function pathPoint(plan, s) {
  // Standing still (a builder): always the centre, which is where it stands.
  if (plan.path === "still") return plan.center.slice(0, 3);
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
  // A builder (ship homes increment 5) stands where the relay put it, never beside someone else.
  if (opts.path === "still") return { center: start.slice(), start, why: "standing where the relay put us" };
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

/** The line a walker logs when the relay corrects it (ship homes increment 4), and the pattern a
 *  rig finds it by: scripts/verify-copresence.js counts every walker's corrections with
 *  CORRECTED_RE, so the two are one place (the review of increment 4, R5: the rig matched its own
 *  /corrected to/, and rewording the line or dropping the handler left every test green).
 *  Groups: x, y, z, the reason, the correction's number. */
const CORRECTED_RE = /corrected to \(([-\d.]+), ([-\d.]+), ([-\d.]+)\) \(([a-z_]+), correction (\d+)\)/;
const correctedLine = (position, reason, seq) => `corrected to ${fmt(position.map(Number))} (${reason}, correction ${seq})`;

/** One game message for the walk: a correction (`game_position_correction`) is logged in the line
 *  the rig reads (`correctedLine`) and the walk stands where the relay holds it and walks on from
 *  there (`walk.corrected`). Returns true when the message was one. */
function onCorrection(g, walk, log) {
  if (!g || g.type !== "game_position_correction" || !Array.isArray(g.position)) return false;
  log(correctedLine(g.position, g.reason, g.seq));
  walk.corrected(Number(g.seq), g.position);
  return true;
}

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

/** The OTHER players already in the world when we joined, as the welcome's
 *  snapshot gives them, one line each for a rig to read: "player present:
 *  entity N "name" at (x, y, z)" (scripts/verify-copresence.js, the meeting in
 *  the Commons: a player standing still sends no updates, so the relay's word in
 *  the welcome is how the walker sees them). Pure. */
function presentLines(welcome) {
  const snap = Array.isArray(welcome && welcome.world_snapshot) ? welcome.world_snapshot : [];
  return snap
    .filter((e) => e.entity_type === "player" && e.entity_id !== welcome.player_id && Array.isArray(e.position))
    .map((e) => `player present: entity ${e.entity_id} "${(e.components && e.components.name) || ""}" at ${fmt(e.position.map(Number))}`);
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
  // The approach: through the route's points, in order (`plan.route`, increment 2: out of our
  // home through its door and corridor), then to the start of the path. With no route it is
  // the one straight leg it always was, walked in about APPROACH_SECONDS; a route is walked at
  // `plan.routeSpeed` (else the walking speed), a person's pace through doors and corridors.
  const route = Array.isArray(plan.route) ? plan.route : [];
  const legs = [...route.map((p) => p.slice()), target];
  let leg = 0;
  const approachM = legs.reduce((acc, p, i) => acc + Math.hypot(...p.map((v, k) => v - (i ? legs[i - 1] : start)[k])), 0);
  let onPath = route.length === 0 && gap < 0.01;
  const approachSpeed = Math.min(route.length ? plan.routeSpeed || plan.speed : Math.max(plan.speed, gap / APPROACH_SECONDS), MAX_SPEED_MPS);
  // The newest correction the relay sent and we stood at (increment 4): every update says it, so
  // the relay can tell the updates sent before it from those after.
  let applied = 0;

  const update = (velocity) => ({
    type: "game_position_update",
    position: pos.slice(),
    rotation: facing.slice(),
    velocity,
    timestamp: clockS, // the sender clock, SECONDS (THE TIMESTAMP RULE)
    correction: applied,
  });

  return {
    gap,
    approachM,
    target,
    onPath: () => onPath,
    position: () => pos.slice(),
    here: () => update([0, 0, 0]),
    /** The relay corrected us (`game_position_correction`, increment 4): stand where it holds
     *  us, say so in every update from now on, and walk back to where the path is now. */
    corrected(seq, at) {
      if (!(seq > applied) || !Array.isArray(at) || at.length !== 3) return false;
      applied = seq;
      pos = at.map(Number);
      if (onPath) {
        onPath = false;
        legs.length = 0;
        legs.push(pathPoint(plan, travelled));
        leg = 0;
      }
      return true;
    },
    next(nowS) {
      // The time step: how long since the last update, by the same clock the
      // stamps come from. (At least a millisecond, so a velocity never
      // divides by zero; the stamp then moves on by that same millisecond.)
      const dt = Math.max(0.001, nowS - clockS);
      clockS += dt;
      let next;
      let reachedPath = false;
      if (!onPath) {
        // This step's distance along the approach, through as many of its points as it reaches
        // (never more than MAX_STEP_M, the relay's rule, even after a long pause).
        let budget = Math.min(approachSpeed * dt, MAX_STEP_M);
        next = pos.slice();
        while (!onPath) {
          const t = legs[leg];
          const d = Math.hypot(t[0] - next[0], t[1] - next[1], t[2] - next[2]);
          if (d > budget) {
            next = next.map((v, i) => v + ((t[i] - v) * budget) / d);
            break;
          }
          next = t.slice();
          budget -= d;
          leg += 1;
          if (leg >= legs.length) {
            // At the start of the path: the rest of this step is not walked (it never was).
            onPath = true;
            reachedPath = true;
          }
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
  const route = Array.isArray(plan.route) ? plan.route : [];
  if (route.length) {
    // One line the rig reads (scripts/verify-copresence.js, the meeting in the Commons).
    log(`walking the route through ${route.length} points (${walk.approachM.toFixed(1)} m at ${(plan.routeSpeed || plan.speed).toFixed(2)} m/s) to the start of the path ${fmt(walk.target)}`);
  } else if (!walk.onPath()) log(`walking ${walk.gap.toFixed(1)} m to the start of the path ${fmt(walk.target)}`);
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
    corrected: (seq, at) => walk.corrected(seq, at),
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

// ── Building in the shared world (ship homes increment 5, 2026-10-05) ────────
//
// The messages, their fields and their rules are the contract's, src/systems/construction/
// shared.rs (`ToRelay`, `FromRelay`, `Permit`); the tests in scripts/tests/second-player.test.js
// read that file and hold the messages here to it.

/** The blueprints file the relay reads (its own copy of the tree's data/, scripts/lib/
 *  throwaway-relay.js), and so the one a scripted build takes its sizes from. */
const BLUEPRINTS_RON = path.join(REPO, "data", "blueprints", "basic.ron");
/** A blueprint id as the contract caps it (shared.rs MAX_BLUEPRINT_ID_LEN, 64 bytes). */
const BLUEPRINT_ID_RULE = /^[a-z0-9_]{1,64}$/;
/** A frame of the ship (src/ship/build_frames.rs): a plot or a shared space, by its id, at most
 *  shared.rs MAX_FRAME_LEN (64) bytes in all. A planet site (`site:`) is not built through here. */
const FRAME_RULE = /^(plot|zone):[A-Za-z0-9_-]{1,59}$/;
/** A plot's own id, as a permit names it (`p3`, never `plot:p3`; shared.rs `Permit::plot`). */
const PLOT_ID_RULE = /^[A-Za-z0-9_-]{1,64}$/;
/** The longest a household permit may run, days: shared.rs PERMIT_MAX_DAYS, the relay's
 *  pq_crypto.rs PLOT_PERMIT_MAX_DAYS (the operator, 2026-10-05: no endless permits). */
const PERMIT_MAX_DAYS = 90;
/** The permit's signature domain: pq_crypto.rs PLOT_PERMIT_DOMAIN. */
const PERMIT_DOMAIN = "hum/permit/v1";
/** f32's pi/2, the step `placement::quarter_turn` multiplies by (std::f32::consts::FRAC_PI_2). */
const F32_FRAC_PI_2 = Math.fround(Math.PI / 2);

/** Split `text` at `sep` where it stands outside every string and bracket (RON). */
function splitTopLevel(text, sep) {
  const out = [];
  let depth = 0;
  let inStr = false;
  let from = 0;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (inStr) {
      if (c === "\\") i++;
      else if (c === '"') inStr = false;
    } else if (c === '"') inStr = true;
    else if (c === "(" || c === "[" || c === "{") depth++;
    else if (c === ")" || c === "]" || c === "}") depth--;
    else if (c === sep && depth === 0) {
      out.push(text.slice(from, i));
      from = i + 1;
    }
  }
  out.push(text.slice(from));
  return out.map((s) => s.trim()).filter(Boolean);
}

/** Every blueprint in the text of a blueprints RON file (data/blueprints/basic.ron), by id: its
 *  name, category, size [x, y, z] metres, build time (s), materials [[item, count]] and whether
 *  it is `shared` (unset is false, as the game's serde default). Comments are skipped wherever
 *  they stand and whatever brackets they hold; the fields are read at each entry's top level, so
 *  a nested value never stands in for one. Pure. */
function readBlueprints(text) {
  // Line comments out, strings kept whole.
  const src = String(text);
  let clean = "";
  for (let i = 0, inStr = false; i < src.length; i++) {
    const c = src[i];
    if (inStr) {
      clean += c;
      if (c === "\\" && i + 1 < src.length) clean += src[++i];
      else if (c === '"') inStr = false;
    } else if (c === '"') {
      inStr = true;
      clean += c;
    } else if (c === "/" && src[i + 1] === "/") {
      while (i < src.length && src[i] !== "\n") i++;
      clean += "\n";
    } else {
      clean += c;
    }
  }
  const out = new Map();
  const str = (v) => (v && v.startsWith('"') ? JSON.parse(v) : null);
  const entry = (body) => {
    const f = {};
    for (const field of splitTopLevel(body, ",")) {
      const k = field.indexOf(":");
      if (k > 0) f[field.slice(0, k).trim()] = field.slice(k + 1).trim();
    }
    const id = str(f.id);
    if (!id) return;
    const tuple = (v) => (v && /^\(.*\)$/s.test(v) ? splitTopLevel(v.slice(1, -1), ",").map(Number) : null);
    out.set(id, {
      id,
      name: str(f.name) || id,
      category: str(f.category) || "",
      size: tuple(f.size),
      build_time: f.build_time === undefined ? null : Number(f.build_time),
      materials: [...String(f.materials || "").matchAll(/\(\s*"([^"]*)"\s*,\s*(\d+)\s*\)/g)].map((m) => [m[1], Number(m[2])]),
      shared: f.shared === "true",
    });
  };
  // Each "(" one bracket inside the file's outer "[" opens a blueprint.
  let depth = 0;
  let start = -1;
  for (let i = 0, inStr = false; i < clean.length; i++) {
    const c = clean[i];
    if (inStr) {
      if (c === "\\") i++;
      else if (c === '"') inStr = false;
    } else if (c === '"') inStr = true;
    else if (c === "(" || c === "[") {
      depth++;
      if (c === "(" && depth === 2) start = i;
    } else if (c === ")" || c === "]") {
      if (c === ")" && depth === 2 && start >= 0) {
        entry(clean.slice(start + 1, i));
        start = -1;
      }
      depth--;
    }
  }
  return out;
}

/** A whole quarter turn about the vertical as a quaternion [x, y, z, w], exactly as the game's
 *  placement makes it (src/systems/construction/placement.rs `quarter_turn`: glam's
 *  `Quat::from_rotation_y((t % 4) as f32 * FRAC_PI_2)`, which is (0, sin(a/2), 0, cos(a/2)),
 *  all in f32). The relay keeps exactly this turn for a build within half a degree of it. */
function quarterTurn(t) {
  const turns = ((Math.trunc(Number(t)) % 4) + 4) % 4;
  const a = Math.fround(Math.fround(turns) * F32_FRAC_PI_2);
  const half = Math.fround(a * 0.5);
  return [0, Math.fround(Math.sin(half)), 0, Math.fround(Math.cos(half))];
}

/** The whole quarter turn (0 to 3) nearest a turn about the vertical [x, y, z, w], either sign. */
function turnOf(q) {
  const yaw = 2 * Math.atan2(Number(q[1]), Number(q[3]));
  return ((Math.round(yaw / (Math.PI / 2)) % 4) + 4) % 4;
}

/** A permit as a build or an unbuild carries it: the JSON a `permit` line logged (shared.rs
 *  `Permit`: issuer, server, plot, grantee, expiry, sig). */
function parsePermitJson(text) {
  let p = null;
  try {
    p = JSON.parse(text);
  } catch {}
  const ok =
    p &&
    typeof p === "object" &&
    typeof p.issuer === "string" &&
    typeof p.server === "string" &&
    typeof p.plot === "string" &&
    typeof p.grantee === "string" &&
    Number.isInteger(p.expiry) &&
    p.expiry >= 0 &&
    typeof p.sig === "string";
  if (!ok) throw new Error(`a permit must be the JSON a "permit" line logged: {"issuer", "server", "plot", "grantee", "expiry", "sig"}, got ${text}`);
  return { issuer: p.issuer, server: p.server, plot: p.plot, grantee: p.grantee, expiry: p.expiry, sig: p.sig };
}

/** One line of the walker's input, as a command: { kind: "build", blueprint, frame, local:
 *  [x, y, z], turns, permit }, { kind: "unbuild", pieceId, permit }, { kind: "permit", plot,
 *  grantee, days }, { kind: "stop" }, or null for an empty line. Throws, saying what it wants, for
 *  anything else (see the HELP text). Pure. */
function parseCommand(line) {
  const text = String(line ?? "").trim();
  if (!text) return null;
  // A permit rides at the end of a build or an unbuild: "... permit {json}".
  let head = text;
  let permit = null;
  const at = text.indexOf(" permit {");
  if (at >= 0) {
    permit = parsePermitJson(text.slice(at + " permit ".length).trim());
    head = text.slice(0, at).trim();
  }
  const words = head.split(/\s+/);
  const verb = words[0];
  if (verb === "stop" && words.length === 1 && !permit) return { kind: "stop" };
  if (verb === "build") {
    const wants = `build wants "build <blueprint>@<frame>:<x>,<y>,<z>,<turns>" (e.g. build wood_wall@plot:p1:46,0.2,36,1), got "${head}"`;
    const m = words.length === 2 ? words[1].match(/^([^@]+)@(.+):([^:]+)$/) : null;
    if (!m || !BLUEPRINT_ID_RULE.test(m[1])) throw new Error(wants);
    const [, blueprint, frame, nums] = m;
    if (!FRAME_RULE.test(frame)) throw new Error(`the frame must be plot:<id> or zone:<id> (src/ship/build_frames.rs), got "${frame}"`);
    const v = nums.split(",").map((s) => s.trim());
    if (v.length !== 4 || !v.every((s) => s !== "" && Number.isFinite(Number(s)))) throw new Error(wants);
    const [x, y, z, t] = v.map(Number);
    if (!Number.isInteger(t) || t < 0 || t > 3) throw new Error(`turns must be 0, 1, 2 or 3 (quarter turns about the vertical), got ${v[3]}`);
    return { kind: "build", blueprint, frame, local: [x, y, z], turns: t, permit };
  }
  if (verb === "unbuild") {
    if (words.length !== 2 || !/^\d+$/.test(words[1]) || Number(words[1]) < 1 || !Number.isSafeInteger(Number(words[1]))) {
      throw new Error(`unbuild wants a piece number, e.g. "unbuild 12", got "${head}"`);
    }
    return { kind: "unbuild", pieceId: Number(words[1]), permit };
  }
  if (verb === "permit" && !permit) {
    const [, plot, grantee, daysText] = words;
    const days = Number(daysText);
    if (words.length !== 4 || !PLOT_ID_RULE.test(plot || "") || !grantee || !Number.isFinite(days)) {
      throw new Error(`permit wants "permit <plot id> <grantee did:hum> <days>" (the plot's own id, p1, not plot:p1), got "${head}"`);
    }
    if (!(days > 0 && days <= PERMIT_MAX_DAYS)) throw new Error(`a household permit lasts at most ${PERMIT_MAX_DAYS} days (and more than none), got ${daysText}`);
    return { kind: "permit", plot, grantee, days };
  }
  throw new Error(`unknown command "${head}" (build, unbuild, permit or stop)`);
}

/** The `game_build` a build command sends (shared.rs `ToRelay::Build`): the pose as given, the
 *  turn as the game's placement makes it, the size the blueprint's own from `blueprints`
 *  (`readBlueprints`). Throws for a blueprint the file does not have. Pure. */
function buildMessage(cmd, reqId, blueprints) {
  const bp = blueprints.get(cmd.blueprint);
  if (!bp || !Array.isArray(bp.size) || bp.size.length !== 3) throw new Error(`no blueprint "${cmd.blueprint}" in data/blueprints/basic.ron`);
  const msg = {
    type: "game_build",
    req_id: reqId,
    frame: cmd.frame,
    blueprint_id: cmd.blueprint,
    position: cmd.local.slice(0, 3),
    rotation: quarterTurn(cmd.turns),
    scale: bp.size.slice(),
  };
  if (cmd.permit) msg.permit = cmd.permit;
  return msg;
}

/** The `game_unbuild` an unbuild command sends (shared.rs `ToRelay::Unbuild`). Pure. */
function unbuildMessage(cmd, reqId) {
  const msg = { type: "game_unbuild", req_id: reqId, piece_id: cmd.pieceId };
  if (cmd.permit) msg.permit = cmd.permit;
  return msg;
}

/** What a household permit's issuer signs: the relay's own words (src/relay/core/pq_crypto.rs
 *  `plot_permit_preimage`, pinned by its test, which scripts/tests/second-player.test.js reads).
 *  `server` is the did:hum of the server the permit is given on (its /api/server-info
 *  `server_did`, `fetchServerDid`): every server hands out its plots in the ship file's order, so
 *  one person often holds the same plot id on several, and a permit is good on its own server
 *  only (Wave 0's 6e0174cd7). */
function permitPreimage(server, plot, grantee, expiry) {
  return `${PERMIT_DOMAIN}\n${server}\n${plot}\n${grantee}\n${expiry}`;
}

/** A household permit `identity` (`deriveIdentity`) signs for `grantee` (their did:hum, `didFor`)
 *  on `plot` (its own id, p1) of the server `server` (its did:hum, `fetchServerDid`) until `expiry`
 *  (Unix seconds): { issuer, server, plot, grantee, expiry, sig }, the contract's `Permit`, its
 *  signature base64. Whether `plot` is the issuer's to give is the relay's question, and it answers
 *  it. */
function mintPermit(identity, { server, plot, grantee, expiry }) {
  const fit = (s) => typeof s === "string" && s.length > 0 && !/[\r\n]/.test(s);
  if (!fit(server) || !PLOT_ID_RULE.test(String(plot)) || !fit(grantee) || !Number.isSafeInteger(expiry) || expiry < 1) {
    throw new Error(`a permit needs the server's did:hum, a plot id, a grantee and an end date in Unix seconds, got ${JSON.stringify({ server, plot, grantee, expiry })}`);
  }
  const sig = identity.sign(new TextEncoder().encode(permitPreimage(server, plot, grantee, expiry)));
  return { issuer: identity.publicKeyHex, server, plot, grantee, expiry, sig: Buffer.from(sig).toString("base64") };
}

/** The relay's own did:hum (its /api/server-info `server_did`, src/relay/api.rs), which a household
 *  permit names as the server it is good on. Null when the relay names none (one from before
 *  2026-09-06, when servers had no DID). Rejects when the relay cannot be asked. */
async function fetchServerDid(serverUrl, { timeoutMs = 5000 } = {}) {
  const res = await fetch(`${httpBase(serverUrl)}/api/server-info`, { signal: AbortSignal.timeout(timeoutMs) });
  if (!res.ok) throw new Error(`/api/server-info answered ${res.status}`);
  const info = await res.json();
  const did = info && typeof info.server_did === "string" ? info.server_did.trim() : "";
  return did.startsWith("did:hum:") ? did : null;
}

/** Base58 in Bitcoin's alphabet, as the relay's `bs58` writes it (each leading zero byte is a 1). */
const BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
function base58(bytes) {
  const b = Array.from(bytes);
  let n = 0n;
  for (const x of b) n = (n << 8n) | BigInt(x);
  let out = "";
  while (n > 0n) {
    out = BASE58_ALPHABET[Number(n % 58n)] + out;
    n /= 58n;
  }
  for (const x of b) {
    if (x !== 0) break;
    out = `1${out}`;
  }
  return out;
}

/** The did:hum a relay holds plots under for a Dilithium key (hex): src/relay/core/did.rs, base58
 *  of the first 16 bytes of BLAKE3 of the key. What a household permit names its grantee by. */
function didFor(noble, publicKeyHex) {
  return `did:hum:${base58(noble.blake3(new Uint8Array(Buffer.from(publicKeyHex, "hex"))).slice(0, 16))}`;
}

/** The lines `logBuilds` writes, and the patterns a rig finds them by (scripts/verify-copresence.js
 *  --build), one place for both, as CORRECTED_RE is. Positions are metres from the frame's corner,
 *  to the millimetre; `turn` is the whole quarter turn of the piece's rotation. */
// Our own build, confirmed: id, blueprint, frame, x, y, z, turn, req, seq, placed_at.
const BUILT_RE = /(?:^|: )built: piece (\d+) (\S+) on (\S+) at \(([-\d.]+), ([-\d.]+), ([-\d.]+)\) turn (\d) \(req (\d+), seq (\d+), placed_at ([\d.]+)\)/;
// Someone else's build: id, blueprint, frame, x, y, z, turn, seq.
const SAW_BUILT_RE = /saw built: piece (\d+) (\S+) on (\S+) at \(([-\d.]+), ([-\d.]+), ([-\d.]+)\) turn (\d) \(seq (\d+)\)/;
// Our own take-down, confirmed: id, frame, req, seq.
const TOOK_DOWN_RE = /took down: piece (\d+) on (\S+) \(req (\d+), seq (\d+)\)/;
// Someone else's take-down: id, frame, seq.
const SAW_UNBUILT_RE = /saw unbuilt: piece (\d+) on (\S+) \(seq (\d+)\)/;
// A refusal: action, reason, why (or none), req (or "-"), the relay's sentence.
const REFUSED_RE = /\b(build|unbuild|pieces) refused: ([a-z_]+)(?: \(([a-z_]+)\))? \(req (\d+|-)\): (.*)$/;
// One piece of a frame's list: id, blueprint, frame, x, y, z, turn, " (mine)" or nothing.
const SNAPSHOT_PIECE_RE = /snapshot piece: piece (\d+) (\S+) on (\S+) at \(([-\d.]+), ([-\d.]+), ([-\d.]+)\) turn (\d)( \(mine\))?/;
// A request sent: build or unbuild, req.
const SENT_RE = /sent (build|unbuild): req (\d+)\b/;
// The welcome's ranks, as JSON (or "missing" from a relay older than increment 5).
const RANKS_RE = /ranks (\{.*\}|missing)/;
// This walker's own did:hum (a permit's grantee is named by it).
const DID_RE = /identity: (did:hum:[1-9A-HJ-NP-Za-km-z]+)/;
// The relay's own did:hum (a permit names the server it is good on).
const SERVER_DID_RE = /server: (did:hum:[1-9A-HJ-NP-Za-km-z]+)/;
// A permit this walker minted, as the JSON a build or an unbuild carries.
const PERMIT_RE = /permit: (\{.*\})$/;

/** The welcome's ranks line (shared.rs `Ranks`: what this player may do beyond their own plot). */
function ranksLine(welcome) {
  if (!welcome || !welcome.ranks || typeof welcome.ranks !== "object") return "ranks missing (the welcome tells none: a relay older than increment 5)";
  return `ranks ${JSON.stringify(welcome.ranks)}`;
}

/** Log what the relay tells us about pieces, one line each, for a rig to read (the patterns
 *  above): our own builds and take-downs (they carry our req_id), everyone else's, refusals with
 *  their codes and sentence, each frame's list as it arrives, and a frame leaving our view.
 *  Returns the function that stops it. */
function logBuilds(client, log) {
  const at3 = (p) => `(${(Array.isArray(p) ? p : []).map((v) => Number(v).toFixed(3)).join(", ")})`;
  const pieceText = (frame, p) => `piece ${p.piece_id} ${p.blueprint_id} on ${frame} at ${at3(p.position)} turn ${turnOf(p.rotation || [0, 0, 0, 1])}`;
  const ours = (g) => g.req_id !== undefined && g.req_id !== null;
  return client.onGame((g) => {
    if (g.type === "game_built" && g.piece) {
      if (ours(g)) log(`built: ${pieceText(g.frame, g.piece)} (req ${g.req_id}, seq ${g.seq}, placed_at ${Number(g.piece.placed_at).toFixed(3)})`);
      else log(`saw built: ${pieceText(g.frame, g.piece)} (seq ${g.seq})`);
    } else if (g.type === "game_unbuilt") {
      if (ours(g)) log(`took down: piece ${g.piece_id} on ${g.frame} (req ${g.req_id}, seq ${g.seq})`);
      else log(`saw unbuilt: piece ${g.piece_id} on ${g.frame} (seq ${g.seq})`);
    } else if (g.type === "game_build_refused") {
      log(`${g.action} refused: ${g.reason}${g.why ? ` (${g.why})` : ""} (req ${ours(g) ? g.req_id : "-"}): ${g.message || ""}`);
    } else if (g.type === "game_pieces") {
      const pieces = Array.isArray(g.pieces) ? g.pieces : [];
      log(`snapshot: ${g.frame} seq ${g.seq} part ${g.part}/${g.parts}, ${pieces.length} piece(s)`);
      for (const p of pieces) log(`snapshot piece: ${pieceText(g.frame, p)}${p.mine ? " (mine)" : ""}`);
    } else if (g.type === "game_frame_out_of_view") {
      log(`frame out of view: ${g.frame}`);
    }
  });
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
  // Who a household permit names, for a rig that has one walker give another a permit (DID_RE).
  log(`identity: ${didFor(noble, identity.publicKeyHex)}`);
  if (!opts.name.startsWith(TEST_BOT_PREFIX)) {
    log(`note: "${opts.name}" does not start with ${TEST_BOT_PREFIX}, so the relay adds it to its member list for good`);
  }

  let client;
  try {
    client = await signIn(opts.server, identity, opts.name, { forwardedFor: opts.forwardedFor });
  } catch (e) {
    return fail(2, e.message);
  }
  log(`signed in: the relay accepted our proof of key${opts.forwardedFor ? ` (from ${opts.forwardedFor}, by X-Forwarded-For)` : ""}`);

  // The relay's ship, named in the join so we hold a plot of it like a desktop
  // player (since ship homes 1b a join naming no ship is a guest in the Commons).
  let ship = null;
  try {
    ship = await fetchShip(opts.server);
    log(ship ? `the relay's ship is ${ship.id} (${ship.hash})` : "the relay names no ship: joining as a guest");
  } catch (e) {
    log(`could not ask the relay which ship it has (${e.message}): joining as a guest`);
  }
  // The relay's own did:hum (ship homes increment 5): a household permit this walker mints names
  // the server it is good on, so it is fetched before any permit is minted (SERVER_DID_RE).
  let serverDid = null;
  try {
    serverDid = await fetchServerDid(opts.server);
    log(serverDid ? `server: ${serverDid}` : "the relay names no did:hum at /api/server-info: no household permit can be minted for it");
  } catch (e) {
    log(`could not ask the relay for its did:hum (${e.message}): no household permit can be minted for it`);
  }

  let welcome;
  try {
    welcome = await joinWorld(client, opts.name, { ship, homeSpawn: opts.homeSpawn });
  } catch (e) {
    client.close();
    return fail(2, e.message);
  }
  const { center, start, why } = chooseCenter(opts, welcome);
  log(`in the world as entity ${welcome.player_id}, starting at ${fmt(start)}`);
  log(homePlotLine(welcome));
  // What this player may do beyond its own plot (ship homes increment 5), as the welcome says.
  log(ranksLine(welcome));
  for (const line of presentLines(welcome)) log(line);
  const stopLogging = logOthers(client, welcome.player_id, log);
  // Every piece the relay tells us about (increment 5). Attached before anything else is awaited,
  // so the frame lists the relay sends right after the welcome are logged too.
  const stopBuilds = logBuilds(client, log);
  const shape =
    opts.path === "line"
      ? `back and forth along a ${2 * opts.radius} m line (${opts.axis} axis)`
      : opts.path === "still"
        ? "nowhere: standing still"
        : `a circle of radius ${opts.radius} m`;
  log(`walking ${shape} at ${opts.speed} m/s, centred on ${fmt(center)} ${why}`);

  const plan = { path: opts.path, axis: opts.axis, center, radius: opts.radius, speed: opts.speed, route: opts.route, routeSpeed: opts.routeSpeed };
  const walker = startWalking(client, plan, start, log);
  // A correction from the relay (increment 4): one line a rig reads (CORRECTED_RE; an honest
  // walk never draws one), then stand where the relay holds us and walk on from there.
  const stopCorrections = client.onGame((g) => onCorrection(g, walker, log));
  const stopChat = opts.chat ? sayInChat(client, identity, opts.name, opts.chat, log) : () => {};

  let stopping = false;
  let limitTimer = null;
  const finish = async (reason, code = 0) => {
    if (stopping) return;
    stopping = true;
    clearTimeout(limitTimer);
    stopChat();
    stopLogging();
    stopBuilds();
    stopCorrections();
    walker.stop();
    // Step out of the world on purpose (the desktop app sends the same),
    // then give the socket a moment to deliver both before closing it.
    client.send({ type: "game_leave" });
    await new Promise((r) => setTimeout(r, 300));
    client.close();
    // Stop listening for "stop", so Node can exit by itself.
    process.stdin.destroy();
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
  // The commands a rig writes on our input (ship homes increment 5, `parseCommand`): build and
  // take down pieces through the relay, mint a household permit. Each request gets the next
  // req_id, said in a "sent" line (SENT_RE) so a rig can match the relay's answer to it; a
  // command that cannot be sent says why in a "command refused" line and sends nothing.
  let blueprints = null;
  let nextReq = 1;
  const onCommand = (line) => {
    let cmd;
    try {
      cmd = parseCommand(line);
    } catch (e) {
      log(`command refused: ${e.message}`);
      return;
    }
    if (!cmd) return;
    if (cmd.kind === "stop") {
      finish("stopped (asked on its input)");
      return;
    }
    try {
      if (cmd.kind === "build") {
        if (!blueprints) blueprints = readBlueprints(fs.readFileSync(BLUEPRINTS_RON, "utf8"));
        const msg = buildMessage(cmd, nextReq, blueprints);
        client.send(msg);
        log(`sent build: req ${nextReq} ${cmd.blueprint} on ${cmd.frame} at (${cmd.local.map((v) => v.toFixed(3)).join(", ")}) turn ${cmd.turns}${cmd.permit ? " with a permit" : ""}`);
        nextReq += 1;
      } else if (cmd.kind === "unbuild") {
        client.send(unbuildMessage(cmd, nextReq));
        log(`sent unbuild: req ${nextReq} piece ${cmd.pieceId}${cmd.permit ? " with a permit" : ""}`);
        nextReq += 1;
      } else if (cmd.kind === "permit") {
        if (!serverDid) throw new Error("the relay named no did:hum at /api/server-info, so no permit can name the server it is good on");
        const expiry = Math.floor(Date.now() / 1000) + Math.round(cmd.days * 86400);
        log(`permit: ${JSON.stringify(mintPermit(identity, { server: serverDid, plot: cmd.plot, grantee: cmd.grantee, expiry }))}`);
      }
    } catch (e) {
      log(`command refused: ${e.message}`);
    }
  };
  // A line "stop" on our input does what Ctrl+C does. Windows gives a child process no signal
  // it can catch (a "kill" ends it at once, with no game_leave, and the relay then keeps its
  // figure for its 90 s grace), so this is how a rig ends a walker cleanly
  // (scripts/verify-copresence.js). An input that is not a pipe or a console just ends.
  process.stdin.setEncoding("utf8");
  let typed = "";
  process.stdin.on("data", (chunk) => {
    typed += chunk;
    for (let i; (i = typed.indexOf("\n")) >= 0; ) {
      const line = typed.slice(0, i).trim();
      typed = typed.slice(i + 1);
      if (!stopping) onCommand(line);
    }
  });
  process.stdin.on("error", () => {});
  client.ws.addEventListener("close", () => {
    if (stopping) return;
    stopping = true;
    walker.stop();
    clearTimeout(limitTimer);
    stopChat();
    stopBuilds();
    fail(2, `the relay closed the connection after ${walker.sent()} updates`);
    setTimeout(() => process.exit(2), 1000).unref();
  });
}

module.exports = {
  CORRECTED_RE,
  correctedLine,
  onCorrection,
  SEND_HZ,
  MAX_STEP_M,
  MAX_SPEED_MPS,
  DEFAULT_NAME,
  TEST_BOT_PREFIX,
  NAME_RULE,
  parseOptions,
  nameFor,
  makeWalk,
  senderClockS,
  normaliseServer,
  httpBase,
  fetchShip,
  joinMessage,
  logOthers,
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
  presentLines,
  // Building in the shared world (ship homes increment 5).
  BLUEPRINTS_RON,
  readBlueprints,
  quarterTurn,
  turnOf,
  parseCommand,
  buildMessage,
  unbuildMessage,
  permitPreimage,
  mintPermit,
  fetchServerDid,
  base58,
  didFor,
  logBuilds,
  ranksLine,
  BUILT_RE,
  SAW_BUILT_RE,
  TOOK_DOWN_RE,
  SAW_UNBUILT_RE,
  REFUSED_RE,
  SNAPSHOT_PIECE_RE,
  SENT_RE,
  RANKS_RE,
  DID_RE,
  SERVER_DID_RE,
  PERMIT_RE,
};

if (require.main === module) main();
