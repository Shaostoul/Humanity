// The scripted second player (scripts/second-player.js), run for real against
// a throwaway relay (week plan Day 3, 2026-10-02).
//
// Run: just verify-second-player
//   (= node --test scripts/tests/second-player-relay.test.js; about 20 s)
//
// WHAT IT BOOTS, AND WHY THIS IS NOT IN `just verify`
// It starts a relay: a COPY of target/release/HumanityOS.exe, run with
// --headless (no window, no GPU) from a new temp folder, on a free port, with
// its database and working folder in that temp folder, so it cannot touch
// %APPDATA%\HumanityOS or the repository's data/. Why a copy: while a program
// runs, Windows locks its file, and target/release/HumanityOS.exe is the very
// file another session's release build or `just deliver` writes, so running it
// in place could make their link fail. A leftover copy is also easy to spot by
// its path (a temp folder named second-player-relay-test-...).
// Booting anything does not belong in the static `just verify` (the
// machine guard also counts a running HumanityOS.exe as a competing instance),
// so these tests have their own recipe; the parts of the script that need no
// relay are tested in scripts/tests/second-player.test.js, in `just rig-tests`.
//
// The relay is started by scripts/lib/throwaway-relay.js (shared with
// scripts/verify-copresence.js since 2026-10-03), which kills it BY PID and
// removes its temp folder when the tests finish, when this process exits for
// any other reason, and on Ctrl+C, Ctrl+Break or a closed console. How that
// was proven, and why the handlers matter even on Windows, is written at the
// top of that file.
//
// WHICH BUILD
// Before booting, scripts/check-fresh-exe.js is run on the exe and its verdict
// printed. If the build is not of this tree's sources (somebody edited src/
// after it was built, or it was built in another checkout; BUG-133), the tests
// SKIP and say so, rather than passing or failing on code that is not the code
// in the tree. SECOND_PLAYER_RELAY_EXE=<path> tests another build of THIS tree
// instead, for example a relay-only debug build (same sources, same stamp):
//   cargo build --features relay --no-default-features --target-dir <somewhere>
//   SECOND_PLAYER_RELAY_EXE=<somewhere>/debug/HumanityOS.exe just verify-second-player
// SECOND_PLAYER_RELAY_ALLOW_OTHER_BUILD="<reason>" runs a build of ANOTHER tree
// on purpose (a red check); the gate prints that loudly and so does this test.
//
// What this proves, in plain terms:
//  1. The scripted player gets in the way a PERSON does: a real Dilithium3
//     identity that answers the relay's sign-in challenge. (Before it,
//     nothing could drive a second player: scripts/ai-sample-client.js signs
//     in the old way and the relay drops it.)
//  2. Somebody else in the world SEES it walk: a second client, signed in by
//     this test with the same sign-in code, joins the world and receives the
//     scripted player's position updates, which follow the requested circle,
//     carry a velocity that matches the movement over the timestamps (THE
//     TIMESTAMP RULE, top of second-player.js: the sender's clock in seconds),
//     face the way it walks, and end standing still.
//  3. When it stops it LEAVES the world on purpose and the watcher sees that at
//     once, while the relay runs with a reconnect grace of 60 s: a connection
//     that merely dropped would keep its figure standing for those 60 s.
//  4. Its one line of chat is accepted, which the relay only does for a
//     correctly signed message.

const { describe, test, before, after } = require("node:test");
const assert = require("node:assert");
const { spawn, spawnSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

const sp = require("../second-player.js");
const TR = require("../lib/throwaway-relay.js");

const REPO = path.resolve(__dirname, "..", "..");
const SCRIPT = path.join(REPO, "scripts", "second-player.js");
// The freshness gate (scripts/check-fresh-exe.js), through its one runner.
const { runFreshGate } = require("../lib/src-fingerprint.js");
const SOURCE_EXE = process.env.SECOND_PLAYER_RELAY_EXE
  ? path.resolve(process.env.SECOND_PLAYER_RELAY_EXE)
  : path.join(REPO, "target", "release", TR.EXE_NAME);
// The reconnect grace the throwaway relay runs with. Above zero on purpose:
// with zero, a dropped connection also despawns at once, and the test could
// not tell a deliberate leave from a socket that simply closed.
const GRACE_SECS = 60;

/** Should the relay tests run? false when yes, or the reason they skip. The
 *  freshness verdict is printed either way, so a skip says exactly why. */
function skipReason() {
  if (!fs.existsSync(SOURCE_EXE)) {
    return `no relay to test against: ${SOURCE_EXE} is missing (build it with cargo build --features native --release, or set SECOND_PLAYER_RELAY_EXE)`;
  }
  // SECOND_PLAYER_RELAY_ALLOW_OTHER_BUILD="<reason>" runs a build of another
  // tree on purpose (a red check); the gate says so loudly and this test has no
  // manifest, so the record is printed with the verdict instead.
  const allow = process.env.SECOND_PLAYER_RELAY_ALLOW_OTHER_BUILD;
  const r = runFreshGate(SOURCE_EXE, allow !== undefined ? ["--allow-other-build", allow] : [], { stdio: "pipe" });
  const verdict = `${r.stdout}${r.stderr}`.trim();
  console.log(`second-player-relay.test: freshness of ${SOURCE_EXE} (scripts/check-fresh-exe.js):`);
  for (const line of verdict.split(/\r?\n/)) console.log(`  ${line}`);
  if (r.other_build) console.log(`second-player-relay.test: other_build ${JSON.stringify(r.other_build)}`);
  if (r.status !== 0) {
    return `${SOURCE_EXE} is not this tree's build (scripts/check-fresh-exe.js refused it, see above), so testing it would test other relay code. Rebuild it, or point SECOND_PLAYER_RELAY_EXE at a relay build of this tree`;
  }
  return false;
}
const SKIP = skipReason();
if (SKIP) console.log(`second-player-relay.test: SKIPPING the relay tests: ${SKIP}`);

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
// A deadline that does not keep Node running once the tests are done (a
// plain 30 s timer left over from a race held the whole run open for 30 s).
const deadline = (ms, value) => new Promise((r) => setTimeout(() => r(value), ms).unref());

// The throwaway relay (scripts/lib/throwaway-relay.js) while the tests run.
let relay = null;

async function waitFor(check, timeoutMs, stepMs = 100) {
  const t0 = Date.now();
  while (Date.now() - t0 < timeoutMs) {
    if (check()) return true;
    await sleep(stepMs);
  }
  return check();
}

/** Run second-player.js with these arguments, keeping every line it prints. */
function runPlayer(args) {
  const proc = spawn(process.execPath, [SCRIPT, ...args], { cwd: REPO, stdio: ["ignore", "pipe", "pipe"] });
  const out = [];
  const take = (b) => out.push(...String(b).split(/\r?\n/).filter(Boolean));
  proc.stdout.on("data", take);
  proc.stderr.on("data", take);
  const exited = new Promise((resolve) => proc.on("exit", (code) => resolve(code)));
  return {
    proc,
    exited,
    text: () => out.join("\n"),
    /** Wait for a printed line matching re; the match, or null if the
     *  player exited or the time ran out first. */
    async waitLine(re, timeoutMs) {
      let done = false;
      exited.then(() => (done = true));
      const found = () => out.map((l) => l.match(re)).find(Boolean);
      await waitFor(() => found() || done, timeoutMs);
      return found() || null;
    },
  };
}

describe("a scripted second player on a throwaway relay", { skip: SKIP }, () => {
  let relayLog;
  let url;
  let dbPath;
  const players = [];
  const sockets = [];

  before(async () => {
    // A copy of the build, --headless, in a new temp folder, on a free port,
    // with its own database (see scripts/lib/throwaway-relay.js). The relay
    // reads data/server-config.json from its working folder.
    relay = await TR.startRelay({
      sourceExe: SOURCE_EXE,
      prefix: "second-player-relay-test-",
      config: { server_name: "second-player test relay", reconnect_grace_secs: GRACE_SECS },
    });
    url = relay.url;
    dbPath = relay.dbPath;
    relayLog = relay.logPath;
    console.log(`second-player-relay.test: relay pid ${relay.pid}, running ${relay.exe}`);
    const logText = () => fs.readFileSync(relayLog, "utf8");
    assert.ok(relay.health && relay.health.status === "ok", `the throwaway relay never answered /health; its log:\n${logText().slice(-2000)}`);
    // The relay prints its limits at start; make sure the grace really is on,
    // or the leave test below proves nothing.
    assert.match(logText(), new RegExp(`reconnect_grace_secs=${GRACE_SECS}\\b`), `the relay runs with a ${GRACE_SECS} s reconnect grace (its log should say so)`);
  });

  after(async () => {
    for (const s of sockets) s.close();
    for (const p of players) if (p.proc.exitCode === null) p.proc.kill();
    if (relay) await relay.stop();
  });

  // Red checks run 2026-10-02/03 against a fresh relay-only debug build of
  // the tree (SECOND_PLAYER_RELAY_EXE), each restored byte for byte after:
  //  - second-player.js no longer sending `game_leave` when it stops (it just
  //    closes its socket): FAILED at "the watcher saw the walker leave the
  //    world at once, not after the 60 s reconnect grace", the relay's game
  //    log ending "Game: player a4a353a19144... lost their link; holding their
  //    place for 60s". With the grace at 0, as this test ran before, that
  //    change passed unnoticed (the critic's finding, 2026-10-02).
  //  - the relay's handle_game_leave put back to calling
  //    handle_game_disconnect (the dropped-socket path), relay rebuilt:
  //    FAILED at the same line.
  //  - makeWalk's stamp sent in milliseconds (`clockS * 1000`): FAILED at
  //    "timestamps are the sender clock in seconds (step 1: 77.5 between two
  //    updates)".
  // From the first version of this test (2026-10-02, then in
  // second-player.test.js):
  //  - identifyPreimage signing "hum/identify/v2\n..." (the wrong text): FAILED
  //    at "the scripted player never got into the world", its output showing
  //    "the relay refused the sign-in: Identify challenge verification failed.
  //    Reconnect and re-identify."
  //  - every velocity sent as [0, 0, 0]: FAILED at "velocity matches the
  //    movement (step 1: sent 0.000,0.000,0.000, moved -0.029,0.000,1.500)".
  //  - stop() no longer sending the standing-still update: FAILED at "its last
  //    update stands still, so nobody's screen slides it on".
  test("signs in as a real identity, joins, is seen walking the circle, and leaves at once", async () => {
    const CENTER = [6, 1.7, -3];
    const RADIUS = 3;
    const SPEED = 1.5;
    const NAME = "TestBotWalker";
    const player = runPlayer([
      "--server", url, "--name", NAME, "--seed", "second-player-test-walker",
      "--path", "circle", "--center", CENTER.join(","), "--radius", String(RADIUS),
      "--speed", String(SPEED), "--seconds", "10",
    ]);
    players.push(player);

    // 1. It gets in. Checked first and on its own, so a refused sign-in reads
    //    as exactly that.
    const inWorld = await player.waitLine(/in the world as entity (\d+)/, 30000);
    assert.ok(inWorld, `the scripted player never got into the world. Its output:\n${player.text()}`);
    const walkerId = Number(inWorld[1]);

    // 2. Somebody else joins and watches.
    const noble = await sp.loadNoble();
    const watcherId = sp.deriveIdentity(noble, sp.masterSeedFrom(noble, "second-player-test-watcher", "TestBotWatcher"));
    const watcher = await sp.signIn(url, watcherId, "TestBotWatcher");
    sockets.push(watcher);
    const updates = [];
    let leftAt = null;
    watcher.onGame((g) => {
      if (g.type === "game_position_update" && g.player_id === walkerId) updates.push(g);
      if (g.type === "game_player_left" && g.player_id === walkerId && leftAt === null) leftAt = Date.now();
    });
    const welcome = await sp.joinWorld(watcher, "TestBotWatcher");
    const seen = (welcome.world_snapshot || []).find((e) => e.entity_id === walkerId);
    assert.ok(seen, "the walker is in the world the watcher joined");
    assert.equal(seen.entity_type, "player");
    assert.equal(seen.components && seen.components.name, NAME, "under its own name");

    // 3. It finishes on its own when its time is up, and leaves the world on
    //    purpose: the watcher hears of it within a few seconds, far inside
    //    the relay's reconnect grace.
    const code = await Promise.race([player.exited, deadline(30000, "still running")]);
    const exitedAt = Date.now();
    assert.equal(code, 0, `the scripted player should stop by itself and exit 0. Its output:\n${player.text()}`);
    // The relay's own "Game:" log lines say what it did with the walker
    // (despawned it, or held its place), so a failure explains itself.
    const gameLog = () =>
      fs.readFileSync(relayLog, "utf8").split(/\r?\n/).filter((l) => l.includes("Game:")).slice(-6)
        // Without the terminal colour codes, and with long keys shortened.
        .map((l) => l.replace(/\x1b\[[0-9;]*m/g, "").replace(/[0-9a-f]{64,}/g, (k) => `${k.slice(0, 12)}...`))
        .join("\n");
    assert.ok(
      await waitFor(() => leftAt !== null, 5000),
      `the watcher saw the walker leave the world at once, not after the ${GRACE_SECS} s reconnect grace. The relay's game log:\n${gameLog()}`,
    );
    assert.ok(leftAt - exitedAt < 5000, "within a few seconds of it stopping");

    // 4. What the watcher saw.
    assert.ok(updates.length >= 60, `the watcher received only ${updates.length} position updates from the walker`);
    const last = updates[updates.length - 1];
    assert.deepEqual(last.velocity, [0, 0, 0], "its last update stands still, so nobody's screen slides it on");
    assert.ok(updates.every((u) => typeof u.timestamp === "number" && u.timestamp > 0), "every update carries a sender clock (0 would mean none)");

    const on = (u) =>
      Math.abs(Math.hypot(u.position[0] - CENTER[0], u.position[2] - CENTER[2]) - RADIUS) < 0.02 &&
      Math.abs(u.position[1] - CENTER[1]) < 0.01;
    // The stretch after it reached the path: everything after the last update
    // that was off the circle (on the way from the spawn point), up to the
    // final standing-still one.
    const moving = updates.slice(0, -1);
    let lastOff = -1;
    moving.forEach((u, i) => {
      if (!on(u)) lastOff = i;
    });
    const walking = moving.slice(lastOff + 1);
    assert.ok(walking.length >= 50, `only the last ${walking.length} of ${updates.length} updates were on the circle`);

    let turned = 0;
    for (let i = 1; i < walking.length; i++) {
      const a = walking[i - 1];
      const b = walking[i];
      // THE TIMESTAMP RULE: seconds on the sender's clock. Updates go out
      // fifteen times a second, so two in a row are a fraction of a second
      // apart (a millisecond stamp would put about 67 here).
      const dt = b.timestamp - a.timestamp;
      assert.ok(dt > 0 && dt < 1, `timestamps are the sender clock in seconds (step ${i}: ${dt.toFixed(1)} between two updates)`);
      // The angle round the centre keeps growing: it walks the circle one way.
      const ang = (u) => Math.atan2(u.position[2] - CENTER[2], u.position[0] - CENTER[0]);
      let d = ang(b) - ang(a);
      if (d < -Math.PI) d += 2 * Math.PI;
      if (d > Math.PI) d -= 2 * Math.PI;
      assert.ok(d > 0, `the walker went backwards round the circle (step ${i})`);
      turned += d;
      // The velocity is the real one: the movement since the last update over
      // the difference of their stamps. (Two updates the watcher received
      // back to back are also two the walker sent back to back.)
      const moved = [0, 1, 2].map((k) => (b.position[k] - a.position[k]) / dt);
      const err = Math.hypot(...[0, 1, 2].map((k) => b.velocity[k] - moved[k]));
      assert.ok(err <= 0.05 * Math.hypot(...moved) + 0.01, `velocity matches the movement (step ${i}: sent ${b.velocity.map((v) => v.toFixed(3))}, moved ${moved.map((v) => v.toFixed(3))})`);
      const speed = Math.hypot(...b.velocity);
      assert.ok(speed > 0.8 * SPEED && speed < 1.2 * SPEED, `walking speed ${speed.toFixed(2)} m/s, asked for ${SPEED}`);
      // It faces the way it walks, in the desktop app's own encoding.
      const yaw = 2 * Math.atan2(b.rotation[1], b.rotation[3]);
      const facing = [Math.sin(yaw), -Math.cos(yaw)];
      const cos = (facing[0] * b.velocity[0] + facing[1] * b.velocity[2]) / Math.hypot(b.velocity[0], b.velocity[2]);
      assert.ok(cos > 0.99, `it faces the way it walks (step ${i}: cos ${cos.toFixed(3)})`);
    }
    // 1.5 m/s on a 3 m circle for several seconds is well over one radian.
    assert.ok(turned > 1, `the walker only went ${turned.toFixed(2)} radians round the circle`);
  });

  // Red check run 2026-10-02 (first version of this test): chatPreimage
  // returning `${content}` without the "\n" + timestamp the relay signs over.
  // FAILED at "the watcher received the walker's line", the walker printing
  // "chat refused: Message rejected: your post-quantum signature failed to
  // verify".
  test("says one line in chat, signed the way the relay requires", async () => {
    const TEXT = "Hello from the scripted second player";
    const NAME = "TestBotTalker";
    const noble = await sp.loadNoble();
    const talker = sp.deriveIdentity(noble, sp.masterSeedFrom(noble, "second-player-test-talker", NAME));

    // The relay makes a brand-new name wait 60 s before posting in public
    // (relay.rs NEW_IDENTITY_GRACE_SECS). So, like a person who signed up
    // earlier: sign in once to register the name, then move that registration
    // two minutes into the past in the throwaway database. The relay reads
    // the time from the database on every message, so it counts at once.
    const first = await sp.signIn(url, talker, NAME);
    first.close();
    // node:sqlite is built in (no dependency) but announces itself as
    // experimental on first use; that one line is noise here, so it is dropped.
    const printWarning = process.listeners("warning");
    process.removeAllListeners("warning");
    process.on("warning", (w) => {
      if (w && w.name === "ExperimentalWarning" && /SQLite/.test(w.message)) return;
      for (const fn of printWarning) fn(w);
    });
    const { DatabaseSync } = require("node:sqlite");
    const db = new DatabaseSync(dbPath);
    try {
      db.exec("PRAGMA busy_timeout = 5000");
      const r = db.prepare("UPDATE registered_names SET registered_at = registered_at - 120000 WHERE public_key = ?").run(talker.publicKeyHex);
      assert.equal(Number(r.changes), 1, "the talker's name was registered by its first sign-in");
    } finally {
      db.close();
    }

    const watcherId = sp.deriveIdentity(noble, sp.masterSeedFrom(noble, "second-player-test-watcher", "TestBotWatcher"));
    const watcher = await sp.signIn(url, watcherId, "TestBotWatcher");
    sockets.push(watcher);
    const heard = [];
    watcher.on((m) => {
      if (m.type === "chat" && m.from === talker.publicKeyHex) heard.push(m);
    });

    const player = runPlayer([
      "--server", url, "--name", NAME, "--seed", "second-player-test-talker",
      "--center", "0,1.7,0", "--radius", "2", "--seconds", "4", "--chat", TEXT,
    ]);
    players.push(player);
    const code = await Promise.race([player.exited, deadline(30000, "still running")]);
    assert.equal(code, 0, `the talker should stop by itself and exit 0. Its output:\n${player.text()}`);
    assert.ok(
      await waitFor(() => heard.some((m) => m.content === TEXT), 3000),
      `the watcher received the walker's line. Its output:\n${player.text()}`,
    );
    assert.match(player.text(), /said in #general/, "and the walker saw its own line come back");
  });
});
