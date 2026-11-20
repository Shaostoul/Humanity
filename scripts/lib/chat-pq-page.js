// chat-pq-page: the website's own post-quantum chat code in a node:vm page, for the
// cross-implementation fixtures (2026-11-19, the post-quantum review's plan item 1b,
// docs/reference/research/2026-11-16-pqc-review/).
//
// WHAT RUNS
// web/chat/pq.js (the ML-DSA-65 / ML-KEM-768 primitives: pqDeriveIdentity, pqDeriveKyber,
// pqSignMessage, pqVerifyMessage, pqDmSeal, pqDmOpen) and web/chat/crypto.js (attachPqIdentity,
// pqBuildDmPuts, pqOpenDmEnvelope), unchanged, with the vendored noble bundle the website serves
// (web/shared/vendor/noble-pq.bundle.js) and Node's WebCrypto for AES-256-GCM. Nothing is stubbed
// on the sealing or opening path: the only stand-in is the place a page gets a person's DM key
// from (getPeerEcdhPublic, which the page answers from a checked key card), given here as the
// recipient key the fixture names.
//
// pq.js loads the bundle with `import('/shared/vendor/noble-pq.bundle.js')`, a URL a vm page cannot
// resolve; its cache variable `_pqMod` is set to that same file, imported here, so its own
// `_pqLoad` returns it.
//
// Used by scripts/pq-cross-fixtures.mjs (writes the website's half of the fixtures) and
// scripts/tests/pq-cross.test.js (checks the Rust half with the website's code).
// HOS_WEB_DIR points it at another copy of web/ (to see a test red against a changed copy).

"use strict";

const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const REPO = path.join(__dirname, "..", "..");
const BUNDLE = path.join(REPO, "web", "shared", "vendor", "noble-pq.bundle.js");

let NOBLE = null;
/**
 * The vendored noble bundle, the very file the website serves. Imported from its bytes as a
 * `data:` module (as key-cards-chat-web.test.js does): the repository's package.json declares no
 * module type, and a file import of an ES-module `.js` there warns on node 22+ and fails on older
 * node.
 */
async function loadNoble() {
  if (!NOBLE) NOBLE = await import("data:text/javascript;base64," + fs.readFileSync(BUNDLE).toString("base64"));
  return NOBLE;
}

const enc = (s) => new TextEncoder().encode(s);

/**
 * The keys of the master seed `[byte; 32]` derived with noble directly (the derivation pq.js
 * uses, pinned to the Rust one by scripts/pq-kat.mjs): {dil, kyb, key (hex), kyber (base64)}.
 */
async function keysOf(byte) {
  const m = await loadNoble();
  const seed = new Uint8Array(32).fill(byte);
  const dil = m.ml_dsa65.keygen(m.blake3.create({ context: enc("hum/dilithium3/v1"), dkLen: 32 }).update(seed).digest());
  const kyb = m.ml_kem768.keygen(m.blake3.create({ context: enc("hum/kyber768/v1"), dkLen: 64 }).update(seed).digest());
  return { dil, kyb, key: Buffer.from(dil.publicKey).toString("hex"), kyber: Buffer.from(kyb.publicKey).toString("base64") };
}

/**
 * A chat page whose identity is the master seed `[seedByte; 32]`, its identity attached by the
 * page's own attachPqIdentity. `peerKyber` answers getPeerEcdhPublic (the key a DM is sealed to).
 * Returns {ctx, fn(name), me: {key, kyber}}.
 */
async function chatPage(seedByte, opts = {}) {
  const noble = await loadNoble();
  const webDir = opts.webDir || process.env.HOS_WEB_DIR || path.join(REPO, "web");
  const quiet = { log() {}, warn() {}, error() {}, info() {}, debug() {} };
  const ctx = {
    console: opts.console || quiet,
    crypto: globalThis.crypto,
    btoa: globalThis.btoa,
    atob: globalThis.atob,
    TextEncoder,
    TextDecoder,
    setTimeout,
    clearTimeout,
  };
  ctx.window = ctx;
  ctx.self = ctx;
  vm.createContext(ctx);
  const run = (rel) => vm.runInContext(fs.readFileSync(path.join(webDir, rel), "utf8"), ctx, { filename: rel });
  run("chat/pq.js");
  run("chat/crypto.js");
  vm.runInContext("(m) => { _pqMod = m; }", ctx)(noble);
  // app.js declares `let myIdentity`; this page holds only the seed (identitySeed reads it).
  vm.runInContext(`var myIdentity = { seed32: new Uint8Array(32).fill(${Number(seedByte)}) };`, ctx);
  if (opts.peerKyber !== undefined) ctx.getPeerEcdhPublic = () => opts.peerKyber;
  const fn = (name) => vm.runInContext(name, ctx);
  if (!(await fn("attachPqIdentity")())) throw new Error("the page's attachPqIdentity failed");
  return { ctx, fn, me: { key: fn("myDilithiumPublicHex"), kyber: fn("myKyberPublicBase64") } };
}

module.exports = { REPO, BUNDLE, loadNoble, keysOf, chatPage };
