// A rig's game is a BRAND-NEW IDENTITY on every run (BUG-316, 2026-11-15): scripts/lib/rig-identity.js
// forgets the sandbox's config.json and the autopilot's seed, and verify-copresence does it at
// every run's setup, before the boot, and then REQUIRES the first-run privacy window to be asked.
//
// Red first, 2026-11-15, each against the code before the fix:
// - "a sandbox that played before boots as a first launch": with identityFiles leaving config.json
//   out, it failed ("Expected values to be strictly deep-equal": gone was only the seed), and so
//   did "the files it forgets are the ones the game keeps an identity in".
// - "verify-copresence forgets the identity at every run's setup, before the boot": against the rig
//   as it was that morning (no forgetSandboxIdentity call in setupRig), "setupRig forgets the
//   sandbox's identity" failed.
// - "the default run requires the first-run window": against the morning's step 5b
//   (clearFirstRunWindows() with no mustAsk), "step 5b must require the window" failed.

const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("fs");
const os = require("os");
const path = require("path");

const RID = require("../lib/rig-identity.js");
const REPO = path.resolve(__dirname, "..", "..");

/** The body of the top-level function that starts at `head` in `src` (to its closing brace at the
 *  start of a line; the working copy may be CRLF). */
function bodyOf(src, head) {
  const at = src.indexOf(head);
  if (at < 0) return "";
  const end = src.slice(at).search(/\r?\n\}\r?\n/);
  return end < 0 ? "" : src.slice(at, at + end);
}

function sandbox() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "rig-identity-"));
  fs.mkdirSync(path.join(dir, "debug"));
  return dir;
}

test("a sandbox that played before boots as a first launch", () => {
  const rig = sandbox();
  try {
    fs.writeFileSync(path.join(rig, "config.json"), JSON.stringify({ privacy_tier: "private", public_key_hex: "ab" }));
    fs.writeFileSync(path.join(rig, "debug", "autopilot_seed.bin"), Buffer.alloc(32, 7));
    fs.writeFileSync(path.join(rig, "debug", "other_done.json"), "{}");
    fs.mkdirSync(path.join(rig, "saves"));
    fs.writeFileSync(path.join(rig, "saves", "offline_home.json"), "{}");
    const r = RID.forgetIdentity(rig);
    assert.equal(r.error, null);
    assert.deepEqual(r.gone, ["config.json", "autopilot_seed.bin"]);
    assert.ok(!fs.existsSync(path.join(rig, "config.json")), "config.json is gone (the chosen privacy level with it)");
    assert.ok(!fs.existsSync(path.join(rig, "debug", "autopilot_seed.bin")), "the autopilot's seed is gone (a new key next boot)");
    assert.ok(fs.existsSync(path.join(rig, "debug", "other_done.json")), "nothing else in debug/ is touched");
    assert.ok(fs.existsSync(path.join(rig, "saves", "offline_home.json")), "the saves are not this helper's");
    // A sandbox that never ran: nothing to forget, and that is not an error.
    assert.deepEqual(RID.forgetIdentity(rig), { gone: [], error: null });
  } finally {
    fs.rmSync(rig, { recursive: true, force: true });
  }
});

test("the files it forgets are the ones the game keeps an identity in", () => {
  // src/engine/ipc.rs keeps the autopilot's seed at this path, relative to the game's folder.
  const ipc = fs.readFileSync(path.join(REPO, "src", "engine", "ipc.rs"), "utf8");
  assert.match(ipc, /const SEED_PATH: &str = "debug\/autopilot_seed\.bin";/, "the autopilot's seed path in src/engine/ipc.rs");
  const files = RID.identityFiles("R").map((f) => path.relative("R", f).replace(/\\/g, "/"));
  assert.deepEqual(files, ["config.json", "debug/autopilot_seed.bin"]);
});

test("verify-copresence forgets the identity at every run's setup, before the boot", () => {
  const src = fs.readFileSync(path.join(REPO, "scripts", "verify-copresence.js"), "utf8");
  const setup = bodyOf(src, "function setupRig() {");
  assert.ok(setup.length > 0, "setupRig is found");
  const copy = setup.indexOf("copyExeIntoRig(");
  const forget = setup.indexOf("forgetSandboxIdentity()");
  assert.ok(forget > 0, "setupRig forgets the sandbox's identity");
  assert.ok(copy > 0 && forget > copy, "after the copy, which stops a game left running from the sandbox");
  assert.match(src, /RID\.forgetIdentity\(RIG\)/, "through scripts/lib/rig-identity.js");
  // Every mode's run starts with setupRig (the --plots reboot leg, a second boot inside one run,
  // is the one place that keeps the identity, on purpose).
  const runs = src.match(/^async function (main|run\w+Once)\(/gm) || [];
  assert.ok(runs.length >= 5, `every mode's run is found (${runs.join(", ")})`);
  for (const r of runs) {
    const at = src.indexOf(r);
    assert.match(src.slice(at, at + 400), /\n  setupRig\(\);/, `${r} starts with setupRig`);
  }
});

test("the default run requires the first-run window, asked and closed", () => {
  const src = fs.readFileSync(path.join(REPO, "scripts", "verify-copresence.js"), "utf8");
  // The default run's own body (the --starts run asks the same, so the whole file would match it).
  const main = bodyOf(src, "async function main() {");
  assert.ok(main.length > 1000 && !main.includes("runStartsOnce"), "the default run's main() is found, and only it");
  assert.match(main, /manifest\.steps_ok\.view = await clearFirstRunWindows\(\{ mustAsk: true, out: OUT \}\);\s*\n\s*step\("view"/, "step 5b must require the window");
  // A brand-new identity not asked fails, as a click that leaves the window does.
  assert.match(src, /if \(mustAsk\) ok = false;/);
  assert.match(src, /ok = ok && gone;/);
});
