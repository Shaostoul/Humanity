// A PASSPHRASE-PROTECTED IDENTITY ON THE WEBSITE IS NEVER REPLACED BY ITSELF (the web twin of
// BUG-213, 2026-10-24). web/chat/crypto.js asks for the passphrase of an identity it holds only
// wrapped (on a browser without Ed25519 in WebCrypto, that is every visit), and a cancelled prompt
// or a wrong passphrase used to make and save a new identity, after which the protected one was
// never asked for again. Now it asks until the identity opens, or until the person chooses a new
// one after being told what it costs (unlockProtectedIdentity).
//
// crypto.js runs in a VM context with a stand-in localStorage and scripted prompt, confirm and
// alert; there is no IndexedDB, so getOrCreateIdentity takes the seed-only path
// (seedOnlyFromStorage), the one those browsers take. The protected seed is wrapped with
// crypto.js's own deriveKeyFromPassphrase, so the test opens exactly what the page writes. Each test
// was seen failing once; what it saw red is at the test.
"use strict";

const test = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const path = require("path");
const vm = require("vm");

const WEB = path.join(__dirname, "..", "..", "web");
const PASS = "correct horse battery";

/** crypto.js loaded with a protected seed in its localStorage, and the person's answers scripted. */
async function page(answers) {
  const store = new Map();
  const asked = { prompts: [], confirms: [], alerts: [] };
  const next = (kind) => {
    if (!answers.length) throw new Error(`asked a ${kind} with no answer left: it keeps asking`);
    const a = answers.shift();
    if (a.kind !== kind) throw new Error(`asked a ${kind}, the next answer is for a ${a.kind}`);
    return a.value;
  };
  const ctx = {
    console: { log() {}, warn() {}, error() {} },
    localStorage: {
      getItem: (k) => (store.has(k) ? store.get(k) : null),
      setItem: (k, v) => store.set(k, String(v)),
      removeItem: (k) => store.delete(k),
    },
    navigator: {},
    prompt: (text) => { asked.prompts.push(text); return next("prompt"); },
    confirm: (text) => { asked.confirms.push(text); return next("confirm"); },
    alert: (text) => { asked.alerts.push(text); },
    crypto: globalThis.crypto,
    btoa: globalThis.btoa,
    atob: globalThis.atob,
    TextEncoder,
    TextDecoder,
    setTimeout: () => 0,
    clearTimeout: () => {},
  };
  ctx.window = ctx;
  ctx.self = ctx;
  vm.createContext(ctx);
  vm.runInContext(fs.readFileSync(path.join(WEB, "chat", "crypto.js"), "utf8"), ctx, { filename: "chat/crypto.js" });
  const fn = (name) => vm.runInContext(name, ctx);
  // The protected seed, wrapped the way wrapAndStoreKey wraps it; no plain backup beside it.
  const seed = new Uint8Array(32).map((_, i) => i + 1);
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const key = await fn("deriveKeyFromPassphrase")(PASS, salt);
  const ct = await crypto.subtle.encrypt({ name: "AES-GCM", iv }, key, fn("pkcs8FromSeed")(seed));
  const wrapped = JSON.stringify({ v: 1, publicKeyHex: "", salt: fn("bufToHex")(salt), iv: fn("bufToHex")(iv), ct: btoa(String.fromCharCode(...new Uint8Array(ct))) });
  store.set("humanity_key_wrapped", wrapped);
  return { fn, store, asked, seed, wrapped };
}

const prompt = (value) => ({ kind: "prompt", value });
const confirm = (value) => ({ kind: "confirm", value });

/** Seen red 2026-10-24 against the one-prompt version: "a cancelled prompt made a new identity". */
test("a cancelled prompt asks again, and the protected identity opens", async () => {
  const p = await page([prompt(null), confirm(true), prompt(PASS)]);
  const id = await p.fn("seedOnlyFromStorage")();
  assert.ok(!id.isNew, "a cancelled prompt made a new identity");
  assert.deepStrictEqual([...id.seed32], [...p.seed], "the protected identity is the one that opens");
  assert.strictEqual(p.asked.prompts.length, 2, "it asked for the passphrase again");
});

/** Seen red 2026-10-24 against the one-prompt version: "a wrong passphrase made a new identity". */
test("a wrong passphrase says so and asks again", async () => {
  const p = await page([prompt("wrong horse"), prompt(PASS)]);
  const id = await p.fn("seedOnlyFromStorage")();
  assert.ok(!id.isNew, "a wrong passphrase made a new identity");
  assert.deepStrictEqual([...id.seed32], [...p.seed], "the right passphrase then opens it");
  assert.strictEqual(p.asked.alerts.length, 1, "the wrong passphrase was told");
});

/** A new identity only by the person's choice, told what it costs, and the protected one is left
 *  where it was. Seen red 2026-10-24 with the choice's answer ignored: "asked a prompt with no
 *  answer left: it keeps asking". */
test("a new identity is made only when the person chooses it", async () => {
  const p = await page([prompt(null), confirm(false)]);
  const id = await p.fn("seedOnlyFromStorage")();
  assert.ok(id.isNew, "choosing a new identity makes one");
  assert.notDeepStrictEqual([...id.seed32], [...p.seed], "it is not the protected one");
  assert.strictEqual(p.asked.confirms.length, 1, "the choice was put to the person");
  assert.match(p.asked.confirms[0], /identity_locked_choice|recovery phrase/, "the choice says what it costs");
  assert.strictEqual(p.store.get("humanity_key_wrapped"), p.wrapped, "the protected identity is left where it was");
});
