// Every rig boots the game as a Dev sandbox on this computer (scripts/lib/rig-gameplay.js),
// 2026-10-04 and 2026-10-05.
//
// GAMEPLAY: fresh installs start in Normal mode with progress kept between launches (the
// operator's decision of 2026-10-04; src/config.rs PlayMode's default and
// fresh_world_each_launch's). A rig's sandbox IS a fresh install (portable.txt, its own
// config.json), so without a pin every rig would have flipped to Normal with that change: the
// probe vantages' garden is planted only while free resources are on (src/engine/ipc.rs
// showcase_gate, "a rig is a Dev-mode sandbox"), and each run is meant to start from the tree's
// default home, not from what an earlier run left in its save.
//
// NO LIVE SERVER: the game dials every server in its config's saved_servers at boot
// (src/engine/bg_connections.rs). Four rig sandboxes held united-humanity.us there, and a
// verify-screens run on 2026-10-05 identified on the LIVE server with a rig identity, read the
// live #general and its 56 users, and tried to join the live shared world
// (.probe-rig/screens/logs/run.log). The pin clears every saved server, points the rig at its
// own throwaway relay or at a dead loopback port, and spawnGame refuses a config that still
// names a server off this computer.
//
// The pin lives in spawnGame (scripts/lib/game-launch.js), the one way a rig starts the game
// (scripts/tests/rig-boot.test.js holds every rig to that), so no rig can boot without it.
//
// Pure node, no game, no GPU: runs in `just rig-tests`. The "game" in the spawnGame tests is a
// copy of node in a temporary sandbox, which reads the config it was started with and reports it.
//
// RED FIRST, this file against the scripts as they were:
//   2026-10-04, before rig-gameplay.js existed: every test failed with
//     "Error: Cannot find module '../lib/rig-gameplay.js'"
//   with the helper in place but spawnGame not calling it, the boot test failed with
//     AssertionError: the game started in the Dev mode   + 'Normal' - 'Dev'
//   and with fresh_world_each_launch still unclassified in rig-graphics.js (so "visual"), the
//   mirror test failed with
//     AssertionError: mirroring the operator's graphics never copies Start every session from
//     the default home   + false - true
//   2026-10-05, with the pin's server half removed (only the play mode and the fresh home set):
//     AssertionError: a rig's config names no server off this computer after the pin
//     + [ { path: 'saved_servers[0].name', value: 'united-humanity.us' }, ... ]
//   and with spawnGame's refusal removed, the refusal test failed with
//     AssertionError: a config naming the live server in a field the pin does not know is
//     refused (exit 1); status 0

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const REPO = path.resolve(__dirname, "..", "..");
const RG = require("../lib/rig-gameplay.js");
const GL = require("../lib/game-launch.js");
const G = require("../rig-graphics.js");

const tmpRoot = fs.mkdtempSync(path.join(os.tmpdir(), "hos-rig-gameplay-test-"));
process.on("exit", () => fs.rmSync(tmpRoot, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 }));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let n = 0;
const dir = (name) => {
  const d = path.join(tmpRoot, `${++n}-${name}`);
  fs.mkdirSync(d, { recursive: true });
  return d;
};
// An environment with no HUMANITY_DATA_DIR, so a test says which config the game reads.
const envWithout = () => {
  const e = { ...process.env };
  delete e.HUMANITY_DATA_DIR;
  return e;
};
const KEY = "a6af7846001bb9dbeb45b7ba7d2d93c7";
// A rig sandbox's config as the probe sweep's, verify-screens', photograph-home's and
// make-clips' were on 2026-10-05: the live server saved (and so dialed at every boot), with the
// memories a connection there leaves behind, next to a loopback relay's plot that must survive.
const reachedLive = () => ({
  play_mode: "Normal",
  fresh_world_each_launch: false,
  server_url: "",
  saved_servers: [
    { name: "united-humanity.us", url: "https://united-humanity.us" },
    { name: "", url: "http://127.0.0.1:57265" },
  ],
  home_plots: {
    [`${KEY} https://united-humanity.us`]: { ship_hash: "aa", plot: "p2" },
    [`${KEY} http://127.0.0.1:62050`]: { ship_hash: "bb", plot: "p1" },
  },
  account_erased_on: { [`${KEY} https://united-humanity.us`]: "Done" },
  collapsed_servers: ["https://united-humanity.us", "http://127.0.0.1:62050"],
  last_world: "server:srv_https://united-humanity.us",
  fov: 77,
});

test("a rig's config comes out a Dev sandbox with the default home every launch, from nothing", () => {
  const { next, changed } = RG.pinGameplay({});
  assert.strictEqual(next.play_mode, "Dev");
  assert.strictEqual(next.fresh_world_each_launch, true);
  assert.strictEqual(next.server_url, RG.RIG_NO_SERVER, "never empty: an empty server_url is the game's built-in one, the live server");
  assert.deepStrictEqual(next.saved_servers, []);
  assert.deepStrictEqual(Object.keys(changed).sort(), ["fresh_world_each_launch", "play_mode", "saved_servers", "server_url"]);
  assert.deepStrictEqual(RG.gameplayProblems(next), []);
  assert.ok(RG.gameplayProblems({}).length > 0, "an empty config is the fresh-install default: Normal, on the live server");
});

test("a rig config holding Normal with progress kept is pinned back, and nothing else in it changes", () => {
  const before = {
    play_mode: "Normal",
    fresh_world_each_launch: false,
    fov: 80,
    readable_web: true,
    home_plots: { [`${KEY} http://127.0.0.1:3399`]: { ship_hash: "abc", plot: "p2" } },
  };
  const { next, changed } = RG.pinGameplay(before);
  assert.strictEqual(next.play_mode, "Dev");
  assert.strictEqual(next.fresh_world_each_launch, true);
  assert.strictEqual(next.fov, 80);
  assert.strictEqual(next.readable_web, true);
  assert.deepStrictEqual(next.home_plots, before.home_plots, "a loopback relay's remembered plot stays");
  assert.deepStrictEqual(changed.play_mode, { was: "Normal", now: "Dev" });
  assert.strictEqual(before.play_mode, "Normal", "pinGameplay does not change what it was given");
  // Already pinned: nothing to change.
  assert.deepStrictEqual(RG.pinGameplay(next).changed, {});
});

test("a rig may say it keeps progress between launches, never that it is not a Dev sandbox", () => {
  const { next } = RG.pinGameplay({ play_mode: "Creative" }, { fresh_world_each_launch: false, play_mode: "Normal" });
  assert.strictEqual(next.play_mode, "Dev", "the play mode is not a rig's to choose");
  assert.strictEqual(next.fresh_world_each_launch, false, "a rig that tests kept progress can ask for it");
});

test("a rig sandbox that once reached the live server is taken off it: no saved server, no memory of it", () => {
  const { next } = RG.pinGameplay(reachedLive());
  assert.deepStrictEqual(RG.publicServers(next), [], "a rig's config names no server off this computer after the pin");
  assert.deepStrictEqual(next.saved_servers, [], "nothing is left for the game to dial in the background");
  assert.strictEqual(next.server_url, RG.RIG_NO_SERVER);
  assert.deepStrictEqual(Object.keys(next.home_plots), [`${KEY} http://127.0.0.1:62050`], "the loopback relay's plot stays");
  assert.deepStrictEqual(next.account_erased_on, {});
  assert.deepStrictEqual(next.collapsed_servers, ["http://127.0.0.1:62050"]);
  assert.strictEqual(next.last_world, "", "a pairing with the live server is forgotten");
  assert.strictEqual(next.fov, 77);
  assert.deepStrictEqual(RG.gameplayProblems(next), []);
});

test("a rig with a throwaway relay is pointed at it, and a server off this computer is refused", () => {
  const relay = "http://127.0.0.1:43210";
  const { next } = RG.pinGameplay(reachedLive(), { server_url: relay });
  assert.strictEqual(next.server_url, relay);
  assert.deepStrictEqual(next.saved_servers, [], "the relay is the autopilot's server, not a saved one dialed beside it");
  assert.deepStrictEqual(RG.gameplayProblems(next, { server_url: relay }), []);
  for (const bad of ["https://united-humanity.us", "wss://united-humanity.us/ws", "http://192.168.1.5:3210", "http://example.org", "united-humanity.us"]) {
    assert.throws(() => RG.pinGameplay({}, { server_url: bad }), /throwaway relay on this computer/, `refused: ${bad}`);
  }
});

test("publicServers finds a server off this computer anywhere in a config, by its host or by the live domain", () => {
  const found = (cfg) => RG.publicServers(cfg).map((p) => p.path);
  assert.deepStrictEqual(found({ a: { b: ["wss://example.org/ws"] } }), ["a.b[0]"]);
  assert.deepStrictEqual(found({ m: { [`${KEY} https://united-humanity.us`]: 1 } }), ["m{key}"]);
  assert.deepStrictEqual(found({ name: "united-humanity.us" }), ["name"], "the live domain named bare");
  assert.deepStrictEqual(found({ id: "srv_https://example.org" }), ["id"], "a URL inside an id");
  for (const ok of ["http://127.0.0.1:3210", "ws://localhost:3210/ws", "ws://[::1]:3210", "C:\\Videos\\clip.mkv", "file:///C:/x", "Dev"]) {
    assert.deepStrictEqual(found({ v: ok }), [], `on this computer, or no server at all: ${ok}`);
  }
});

test("the sandbox config is the one the game reads: HUMANITY_DATA_DIR, then portable.txt beside the exe, never the player's own", () => {
  const sandbox = dir("sandbox");
  const exe = path.join(sandbox, "HumanityOS.exe");
  // No portable.txt and no HUMANITY_DATA_DIR: the game would read the player's own config,
  // which no rig writes.
  assert.strictEqual(RG.sandboxConfigPath(exe, envWithout()), null);
  fs.writeFileSync(path.join(sandbox, "portable.txt"), "test rig\n");
  assert.strictEqual(RG.sandboxConfigPath(exe, envWithout()), path.join(sandbox, "config.json"));
  const dataDir = dir("data-dir");
  assert.strictEqual(RG.sandboxConfigPath(exe, { ...envWithout(), HUMANITY_DATA_DIR: ` ${dataDir} ` }), path.join(dataDir, "config.json"));
});

test("pinSandboxGameplay writes the pin into the sandbox's config, and keeps one it could not read", () => {
  const sandbox = dir("pin");
  const exe = path.join(sandbox, "HumanityOS.exe");
  fs.writeFileSync(path.join(sandbox, "portable.txt"), "test rig\n");
  const r = RG.pinSandboxGameplay(exe, envWithout(), {}, () => {});
  assert.strictEqual(r.path, path.join(sandbox, "config.json"));
  assert.deepStrictEqual(r.public_servers, []);
  const written = JSON.parse(fs.readFileSync(r.path, "utf8"));
  assert.deepStrictEqual(RG.gameplayProblems(written), []);
  // A config the game could not parse would boot on the fresh-install defaults (Normal, the
  // live server): the pin starts over from nothing, and the unreadable file is kept beside it.
  fs.writeFileSync(r.path, "{ not json");
  const again = RG.pinSandboxGameplay(exe, envWithout(), {}, () => {});
  assert.deepStrictEqual(RG.gameplayProblems(JSON.parse(fs.readFileSync(again.path, "utf8"))), []);
  assert.ok(fs.readdirSync(sandbox).some((f) => f.startsWith("config.json.unreadable")), "the unreadable config is kept");
  // Not a sandbox: nothing is written anywhere.
  const bare = dir("bare");
  assert.strictEqual(RG.pinSandboxGameplay(path.join(bare, "HumanityOS.exe"), envWithout(), {}, () => {}), null);
  assert.deepStrictEqual(fs.readdirSync(bare), []);
});

/** A sandbox with a stand-in game (node under the game's name, beside portable.txt) and `cfg`
 *  as its config.json. */
function sandboxWith(name, cfg) {
  const sandbox = dir(name);
  fs.writeFileSync(path.join(sandbox, "portable.txt"), "test rig\n");
  fs.writeFileSync(path.join(sandbox, "config.json"), JSON.stringify(cfg, null, 2));
  const exe = path.join(sandbox, "HumanityOS.exe");
  fs.copyFileSync(process.execPath, exe);
  return { sandbox, exe, fresh: { result: { exe, exe_sha256: GL.fileSha256(exe), exe_fingerprint: null, verdict: "current" } } };
}

test("spawnGame pins the sandbox before the game starts: the game reads the Dev mode, on this computer", async () => {
  const { sandbox, exe, fresh } = sandboxWith("boot", reachedLive());
  const seen = path.join(tmpRoot, "seen-config.json");
  const read = `require('fs').writeFileSync(process.argv[1], require('fs').readFileSync(require('path').join(require('path').dirname(process.execPath), 'config.json')))`;
  // A refusal in spawnGame is process.exit(1) (lib/game-launch.js), which here would end the whole
  // run before it reported anything, the other tests' messages included (found 2026-10-05 by
  // breaking the pin's server half on purpose: the file failed with only "test: REFUSED"). So a
  // refusal is this test's own failure, and the run goes on.
  const exit = process.exit;
  process.exit = (code) => {
    throw new Error(`spawnGame refused to boot (exit ${code}): the pin left the rig's config naming a server off this computer`);
  };
  let w;
  try {
    w = GL.spawnGame(exe, ["-e", read, seen], { fresh, rigName: "test", env: envWithout(), log: () => {} });
  } finally {
    process.exit = exit;
  }
  const t0 = Date.now();
  while (!w.exited() && Date.now() - t0 < 20000) await sleep(100);
  assert.strictEqual(w.code, 0, `the stand-in game ran: ${w.exited()}`);
  const cfg = JSON.parse(fs.readFileSync(seen, "utf8"));
  assert.strictEqual(cfg.play_mode, "Dev", "the game started in the Dev mode");
  assert.strictEqual(cfg.fresh_world_each_launch, true, "the game started from the default home");
  assert.deepStrictEqual(RG.publicServers(cfg), [], "the game started with no server off this computer to dial");
  assert.deepStrictEqual(cfg.saved_servers, []);
  assert.strictEqual(cfg.fov, 77, "the rest of the rig's config is left as it was");
  assert.deepStrictEqual(w.gameplay && w.gameplay.path, path.join(sandbox, "config.json"), "the watch says what it pinned");
});

test("spawnGame refuses a rig config still naming a server off this computer, and starts nothing", () => {
  // A field the pin does not know, naming the live server: the boot is refused, not dialed.
  const { exe, fresh } = sandboxWith("refuse", { some_new_field: "https://united-humanity.us/ws" });
  const marker = path.join(tmpRoot, "started-refuse.txt");
  const script = `
    const GL = require(${JSON.stringify(path.join(REPO, "scripts", "lib", "game-launch.js"))});
    const env = { ...process.env }; delete env.HUMANITY_DATA_DIR;
    const w = GL.spawnGame(${JSON.stringify(exe)}, ["-e", "require('fs').writeFileSync(process.argv[1], 'x')", ${JSON.stringify(marker)}], {
      fresh: ${JSON.stringify(fresh)}, rigName: "the-test-rig", env, log: () => {},
    });
    w.child.on("exit", () => process.exit(0));
  `;
  const r = spawnSync(process.execPath, ["-e", script], { encoding: "utf8", timeout: 30000 });
  const out = `${r.stdout}\n${r.stderr}`;
  assert.strictEqual(r.status, 1, `a config naming the live server in a field the pin does not know is refused (exit 1); status ${r.status}\n${out}`);
  assert.match(out, /RIG CONFIG NAMES A SERVER OFF THIS COMPUTER/);
  assert.match(out, /some_new_field: https:\/\/united-humanity\.us\/ws/);
  assert.match(out, /the-test-rig: REFUSED - nothing was booted/);
  assert.ok(!fs.existsSync(marker), "nothing was started");
});

// The rigs themselves: every script that starts the game through spawnGame does so from a sandbox
// (portable.txt beside its copy of the exe), so the config spawnGame pins is the one the game
// reads (rig-boot.test.js holds every game-booting script to spawnGame); a rig that runs a relay
// of its own names it as the server; and none asks for another play mode.
test("every rig starts the game from a sandbox spawnGame pins, a rig with a relay is pointed at it, and none asks for another play mode", () => {
  const rigs = [];
  const walk = (d) => {
    for (const e of fs.readdirSync(d, { withFileTypes: true })) {
      const full = path.join(d, e.name);
      if (e.isDirectory()) {
        if (e.name !== "tests" && e.name !== "node_modules") walk(full);
      } else if (e.name.endsWith(".js")) {
        const src = fs.readFileSync(full, "utf8");
        if (/\bspawnGame\s*\(/.test(src) && !full.endsWith(path.join("lib", "game-launch.js"))) rigs.push([full, src]);
      }
    }
  };
  walk(path.join(REPO, "scripts"));
  assert.ok(rigs.length >= 7, `found only ${rigs.length} rigs that call spawnGame: the scan broke`);
  const problems = [];
  for (const [file, src] of rigs) {
    const rel = path.relative(REPO, file).replace(/\\/g, "/");
    if (!src.includes("portable.txt")) problems.push(`${rel}: writes no portable.txt, so its game reads the player's own config`);
    if (/gameplay\s*:\s*\{[^}]*play_mode/.test(src)) problems.push(`${rel}: asks spawnGame for a play mode`);
    const runsRelay = /\bstartRelay\s*\(/.test(src) || /["']--headless["']/.test(src);
    const calls = [...src.matchAll(/\bspawnGame\s*\(/g)].map((m) => src.slice(m.index, m.index + 900));
    if (runsRelay) {
      calls.forEach((c, i) => {
        if (!/gameplay\s*:\s*\{\s*server_url\s*:/.test(c)) problems.push(`${rel}: runs a relay but spawnGame call ${i + 1} does not name it as the server`);
      });
    }
  }
  assert.deepStrictEqual(problems, []);
  // And spawnGame pins, then checks, before it spawns (the boot tests above run it for real).
  const gl = fs.readFileSync(path.join(REPO, "scripts", "lib", "game-launch.js"), "utf8");
  const pin = gl.indexOf("pinSandboxGameplay(");
  const refuse = gl.indexOf("RIG CONFIG NAMES A SERVER OFF THIS COMPUTER");
  const spawnAt = gl.indexOf("child = spawn(");
  assert.ok(pin > 0 && refuse > pin && spawnAt > refuse, "spawnGame pins and checks the sandbox config before it spawns");
});

test("mirroring the operator's graphics into a rig never copies the play mode or the fresh-home switch", () => {
  const rig = RG.pinGameplay({ ssao_strength: 0.55 }).next;
  const operator = { play_mode: "Dev", fresh_world_each_launch: false, ssao_strength: 0.9594 };
  const m = G.mirrorOperatorGraphics(rig, { ...operator, play_mode: "Normal" });
  assert.strictEqual(m.next.play_mode, "Dev", "mirroring the operator's graphics never copies the play mode");
  assert.strictEqual(
    m.next.fresh_world_each_launch,
    true,
    "mirroring the operator's graphics never copies Start every session from the default home",
  );
  assert.strictEqual(m.next.ssao_strength, 0.9594, "the graphics settings are still mirrored");
  assert.strictEqual(G.classifyKey("fresh_world_each_launch"), "record_only");
  assert.strictEqual(G.classifyKey("play_mode"), "record_only");
  // Servers are private keys: never mirrored from the operator into a rig.
  assert.strictEqual(G.classifyKey("saved_servers"), "private");
  assert.strictEqual(G.classifyKey("server_url"), "private");
});

// THE AUTOPILOT REQUEST CANNOT UNDO THE PIN (2026-10-05, BUG-160). The game applies the
// request's server_url over the pinned config (src/engine/ipc.rs poll_autopilot_request), and an
// empty one does not mean no server: the chat page fills an empty address with the live
// server's (src/gui/pages/chat/left_panel.rs) and the auto-connect then dials it. Five rigs sent
// { server_url: "" }, and verify-screens' v0.1462.0 run identified on the live server, read its
// chat and was refused its ship. So a rig's autopilot request names no server (the pin stands)
// or its own relay through a variable; the one deliberate clear, verify-live-screen's "No server
// set" step, carries the marker `rig-clears-server:`.
//
// RED FIRST: against the scripts as they were (git show HEAD:scripts/<name>), this check listed
// boot-timing.js, make-clips.js, photograph-home.js, probe-sweep.js, verify-live-screen.js and
// verify-screens.js.
function autopilotServerProblems(name, text) {
  const problems = [];
  text.split(/\r?\n/).forEach((line, i) => {
    if (!line.includes("autopilot_request.json") || line.includes("rig-clears-server:")) return;
    const m = line.match(/server_url\s*:\s*(?:"([^"]*)"|'([^']*)')/);
    if (!m) return; // no server_url (the pin stands), or the rig's own relay through a variable
    const v = m[1] !== undefined ? m[1] : m[2];
    if (v === "" || RG.namesPublicServer(v)) problems.push(`${name}:${i + 1}: ${line.trim()}`);
  });
  return problems;
}

test("no rig's autopilot request sends an empty or public server address", () => {
  const dir = path.join(REPO, "scripts");
  const problems = [];
  for (const f of fs.readdirSync(dir).filter((f) => f.endsWith(".js"))) {
    problems.push(...autopilotServerProblems(f, fs.readFileSync(path.join(dir, f), "utf8")));
  }
  assert.deepStrictEqual(problems, []);
  // The check itself catches the two shapes the rigs used.
  assert.strictEqual(autopilotServerProblems("x", `req("autopilot_request.json", { server_url: "" });`).length, 1);
  assert.strictEqual(autopilotServerProblems("x", `JSON.stringify({ server_url: "https://united-humanity.us" }) // autopilot_request.json`).length, 1);
  assert.strictEqual(autopilotServerProblems("x", `req("autopilot_request.json", { server_url: relay.httpUrl });`).length, 0);
});
