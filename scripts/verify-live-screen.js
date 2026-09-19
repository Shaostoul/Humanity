#!/usr/bin/env node
// verify-live-screen: the gate for a `watch:` screen on an in-world wall.
//
// The operator photographed wall_screen_4 (the great room television since
// the 2026-09-19 home redesign; it was in the console room then) reading
// "Could not connect: URL error: No host name in the URL Trying again
// shortly." and asked whether the live feature could be pointed at an actual
// stream and tested. Two things had to exist for that to be answerable, and
// this gate proves both with nobody at the keyboard:
//
//   1. A PICTURE. There is no third-party stream this feature can watch: it
//      watches our own relay. So the rig starts a relay of its own on a spare
//      port with its own database, publishes a moving test pattern into it
//      (scripts/live-publish.js), points the game at THAT relay, and proves
//      the wall is live by taking two snapshots a second apart and comparing
//      them against a STATIC screen over the same second. A live picture
//      changes; a frozen one does not.
//
//   2. THREE HONEST MESSAGES. With the publisher stopped, the wall must say
//      the stream is not live, not show a parse error. With no server set at
//      all it must say so, name where a server is set, and NOT OPEN A SOCKET,
//      which is checked in the log rather than on the glass: the screen's own
//      "opening a viewer on ..." line must not appear once after the server
//      is cleared.
//
// NEVER PRODUCTION. The relay this rig talks to is one it started itself, on
// loopback, with a database it deleted first. live-publish.js refuses a
// non-loopback server unless told otherwise, and this rig never tells it
// otherwise. united-humanity.us is the operator's live service; its streaming
// switch is the operator's decision.
//
// Usage:
//   node scripts/verify-live-screen.js [--exe PATH] [--port N] [--timeout-min N] [--keep-open]
//   node scripts/verify-live-screen.js --dry-verdict <manifest.json>
// Exit 0 = every check passed. Exit 1 = refused to run. Exit 2 = failed.
//
// ONE GPU (CLAUDE.md): this refuses to boot while ANY HumanityOS.exe is
// running, the operator's game included, and never sets the take-focus env
// var (src/engine/launch_focus.rs).

const fs = require("fs");
const path = require("path");
const { spawn, spawnSync, execSync } = require("child_process");
const png = require("./lib/png.js");
const G = require("./rig-graphics.js");
const MG = require("./lib/machine-guard.js");

const REPO = path.resolve(__dirname, "..");
const args = process.argv.slice(2);
const opt = (name, def) => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] ? args[i + 1] : def;
};
const flag = (name) => args.includes(name);
const KEEP_OPEN = flag("--keep-open");
const EXE = path.resolve(opt("--exe", path.join(REPO, "target", "release", "HumanityOS.exe")));
const TIMEOUT_MS = Number(opt("--timeout-min", "12")) * 60 * 1000;
const PORT = Number(opt("--port", "3399"));
const DRY = opt("--dry-verdict", null);

// Its own rig folder, so it never fights verify-screens or verify-runtime
// over one exe copy or one set of debug/*.json files.
const RIG = path.join(REPO, ".probe-rig", "live-screen");
const DEBUG = path.join(RIG, "debug");
const LOG = path.join(RIG, "logs", "run.log");
const RELAY_DIR = path.join(RIG, "relay");
const RELAY_LOG = path.join(RIG, "logs", "relay.log");

// THE SCREENS, fixed on purpose: the placed instances in
// data/machines/home.ron. wall_screen_4's source is watch:shaostoul, so the
// publisher claims that name and the wall lights up with no data edit at all.
// wall_screen_2 (the tasks page) is the CONTROL: a screen that should not
// change over the same second the live one does.
const LIVE_SCREEN = "wall_screen_4";
const STATIC_SCREEN = "wall_screen_2";
const STREAM = "shaostoul";

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const pad = (s, n) => String(s).padEnd(n);
const rel = (p) => {
  const r = path.relative(REPO, p);
  return !r || r.startsWith("..") ? p : r;
};
function log(msg) {
  console.log(`[live-screen] ${msg}`);
}
function refuse(lines) {
  console.error("");
  for (const l of lines) console.error(l);
  console.error("");
  process.exit(1);
}

// ── The verdict: pure, over a manifest ───────────────────────────────────────
// Everything the live run learned is in the manifest (the screen answers
// verbatim, the PNG names beside it, the log tail counts), so --dry-verdict
// re-judges the exact logic the live run trusted.
function loadPng(dir, name) {
  if (!name) return { error: "no snapshot recorded" };
  const p = path.join(dir, name);
  if (!fs.existsSync(p)) return { error: `missing ${name}` };
  try {
    return { img: png.decode(fs.readFileSync(p)) };
  } catch (e) {
    return { error: `${name}: ${e.message}` };
  }
}

// A live picture must change a LOT over a second, and the floor it has to
// clear is measured, not guessed: the same second on a screen that is not
// playing anything.
const MOVING_MIN_FRACTION = 0.1; // at least a tenth of the wall changed
const MOVING_MIN_RATIO = 10; // and at least ten times the static screen's own churn

function judge(m, dir) {
  const checks = [];
  const add = (id, ok, detail) => checks.push({ id, ok: !!ok, detail });
  const live = m.live || {};
  const offline = m.offline || {};
  const noServer = m.no_server || {};

  add("relay_up", m.relay && m.relay.ok === true, m.relay ? m.relay.detail : "never started");
  add(
    "stream_published",
    m.publisher && m.publisher.listed === true,
    m.publisher ? m.publisher.detail || "" : "never started",
  );

  // The wall received frames from the relay.
  const conn = live.status || null;
  add(
    "screen_connected",
    conn && conn.ok === true && conn.connected === true && Number(conn.frames) > 0,
    conn ? conn.error || `connected=${conn.connected} frames=${conn.frames} heading=${JSON.stringify(conn.heading)}` : "never ran",
  );

  // The picture MOVES: two snapshots a second apart, against a static
  // screen's own churn over the same second.
  const a = loadPng(dir, live.before);
  const b = loadPng(dir, live.after);
  const sa = loadPng(dir, live.control_before);
  const sb = loadPng(dir, live.control_after);
  if (a.img && b.img && sa.img && sb.img) {
    const d = png.diffPixels(a.img, b.img);
    const c = png.diffPixels(sa.img, sb.img);
    const frac = d.total ? d.differing / d.total : 0;
    const cfrac = c.total ? c.differing / c.total : 0;
    const ok =
      !d.sizeMismatch &&
      !c.sizeMismatch &&
      frac >= MOVING_MIN_FRACTION &&
      d.differing > MOVING_MIN_RATIO * (c.differing + 1);
    add(
      "picture_moves",
      ok,
      `live ${d.differing}/${d.total} px changed (${(frac * 100).toFixed(1)}%), ` +
        `static control ${c.differing}/${c.total} (${(cfrac * 100).toFixed(2)}%) over the same second` +
        (ok ? "" : `; needs >= ${(MOVING_MIN_FRACTION * 100).toFixed(0)}% and > ${MOVING_MIN_RATIO}x the control`),
    );
  } else {
    add("picture_moves", false, a.error || b.error || sa.error || sb.error || "no pair of snapshots to compare");
  }

  // The publisher stopped: the wall says the stream is not live, and says it
  // about THIS stream.
  const off = offline.status || null;
  const offSaysNotLive = !!(
    off &&
    off.ok === true &&
    off.heading === "Stream offline" &&
    typeof off.detail === "string" &&
    off.detail.includes(`streaming as "${STREAM}"`)
  );
  add(
    "offline_message",
    offSaysNotLive,
    off ? off.error || `${JSON.stringify(off.heading)} / ${JSON.stringify(off.detail)}` : "never ran",
  );
  add(
    "offline_no_parse_error",
    off && typeof off.detail === "string" && !/host name|URL error/i.test(off.detail + " " + (off.heading || "")),
    off ? `detail=${JSON.stringify(off.detail)}` : "never ran",
  );
  const offFind = offline.find || null;
  add(
    "offline_drawn",
    offFind && offFind.ok === true && offFind.found === true,
    offFind ? offFind.error || `"${offFind.text}" drawn on the wall (${offFind.matches} match(es))` : "never ran",
  );

  // No server: the sentence, and NO socket.
  const ns = noServer.status || null;
  add(
    "no_server_message",
    ns && ns.ok === true && ns.heading === "No server set" && typeof ns.detail === "string" && ns.detail.includes("Server field"),
    ns ? ns.error || `${JSON.stringify(ns.heading)} / ${JSON.stringify(ns.detail)}` : "never ran",
  );
  const nsFind = noServer.find || null;
  add(
    "no_server_drawn",
    nsFind && nsFind.ok === true && nsFind.found === true,
    nsFind ? nsFind.error || `"${nsFind.text}" drawn on the wall (${nsFind.matches} match(es))` : "never ran",
  );
  // THE LOG CHECK. `may_connect:false` is the screen's own claim; this is the
  // evidence. Every socket the screen opens writes one "opening a viewer"
  // line, so zero of them in the log written AFTER the server was cleared
  // means no connection was attempted, and it is read from the tail so the
  // sockets opened earlier in this very run cannot mask it.
  const tail = noServer.log_tail || null;
  add(
    "no_connection_attempted",
    tail && tail.opened === 0 && tail.said_no_server > 0,
    tail
      ? `${tail.opened} "opening a viewer" line(s) and ${tail.said_no_server} "No server set" line(s) in the ${tail.bytes} bytes of run.log written after the server was cleared`
      : "never read",
  );

  const panics = Number(m.panics || 0);
  add("no_panics", panics === 0, `${panics} PANIC line(s) in run.log`);
  return { checks, pass: checks.every((c) => c.ok) };
}

function printVerdict(prefix, m, dir) {
  const { checks, pass } = judge(m, dir);
  console.log("");
  console.log("-".repeat(72));
  for (const c of checks) console.log(`${c.ok ? "PASS" : "FAIL"}  ${pad(c.id, 24)} ${c.detail}`);
  console.log("-".repeat(72));
  if (pass) {
    console.log(`${prefix}PASS  ${checks.length}/${checks.length} live-screen checks passed`);
  } else {
    const failed = checks.filter((c) => !c.ok).map((c) => c.id);
    console.log(`${prefix}FAIL  ${checks.length - failed.length}/${checks.length} passed; failed: ${failed.join(", ")}`);
  }
  console.log(`        evidence ${rel(dir)}`);
  console.log("-".repeat(72));
  console.log("");
  return pass;
}

if (DRY) {
  const manifest = path.resolve(DRY);
  if (!fs.existsSync(manifest)) refuse([`--dry-verdict: no such manifest: ${manifest}`]);
  const m = JSON.parse(fs.readFileSync(manifest, "utf8"));
  console.log(`[dry] verdict only, nothing booted: ${rel(manifest)}`);
  const pass = printVerdict("DRY VERDICT (nothing was booted): ", m, path.dirname(manifest));
  process.exit(pass ? 0 : 2);
}

// ── Preconditions ────────────────────────────────────────────────────────────
console.log("");
console.log("verify-live-screen  a real relay, a real stream, a real wall screen");
console.log("");

const instances = MG.listInstances();
if (instances.length) {
  refuse([
    `REFUSED: HumanityOS.exe is already running (pid ${instances.map((p) => `${p.pid} ${p.exe}`).join("; ")}).`,
    "One GPU, one instance (CLAUDE.md). This rig also starts a headless relay from the same",
    "binary, so a leftover of either would be counted twice. Wait for it to exit, then run again.",
    "If it is a leftover rig of ours: taskkill //PID <pid> //F",
  ]);
}

const fresh = spawnSync(process.execPath, [path.join(__dirname, "check-fresh-exe.js"), "--exe", EXE], {
  cwd: REPO,
  stdio: "inherit",
});
if (fresh.status !== 0) {
  console.error("verify-live-screen: REFUSED - see the freshness failure above. Nothing was booted.");
  process.exit(1);
}

// ── Rig setup ────────────────────────────────────────────────────────────────
function ensureJunction(link, target) {
  try {
    const st = fs.lstatSync(link);
    if (st.isSymbolicLink() || st.isDirectory()) {
      try {
        if (fs.realpathSync(link).toLowerCase() === fs.realpathSync(target).toLowerCase()) return;
      } catch {}
      fs.rmSync(link, { recursive: true, force: true });
    }
  } catch {}
  fs.symlinkSync(target, link, "junction");
}
function killRigProcesses() {
  const rigExe = path.join(RIG, "HumanityOS.exe").replace(/'/g, "''");
  try {
    execSync(
      `powershell -NoProfile -Command "Get-Process HumanityOS -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq '${rigExe}' } | Stop-Process -Force"`,
      { stdio: "ignore" },
    );
  } catch {}
}
function setupRig() {
  fs.mkdirSync(RIG, { recursive: true });
  fs.mkdirSync(DEBUG, { recursive: true });
  fs.mkdirSync(path.join(RIG, "logs"), { recursive: true });
  fs.mkdirSync(RELAY_DIR, { recursive: true });
  // portable.txt: identity/config/saves stay inside the rig, and the dev
  // autopilot runs (it refuses against a real installed identity).
  fs.writeFileSync(path.join(RIG, "portable.txt"), "verify-live-screen rig\n");
  // no_focus.txt: the engine boots background even if the env var is lost.
  fs.writeFileSync(path.join(RIG, "no_focus.txt"), "engine marker: boot background, never steal focus.\n");
  ensureJunction(path.join(RIG, "data"), path.join(REPO, "data"));
  ensureJunction(path.join(RIG, "assets"), path.join(REPO, "assets"));
  if (!fs.existsSync(EXE)) {
    refuse([`ERROR: exe not found: ${EXE}`, "  build one first: cargo build --features native --release"]);
  }
  killRigProcesses();
  try {
    fs.copyFileSync(EXE, path.join(RIG, "HumanityOS.exe"));
  } catch (e) {
    if (e.code !== "EBUSY") throw e;
    execSync("ping -n 3 127.0.0.1 >nul", { shell: "cmd.exe" });
    fs.copyFileSync(EXE, path.join(RIG, "HumanityOS.exe"));
  }
  // DXC dlls beside the exe drop boot from ~25 s to ~5 s.
  for (const dll of ["dxcompiler.dll", "dxil.dll"]) {
    const s = path.join(path.dirname(EXE), dll);
    if (fs.existsSync(s)) fs.copyFileSync(s, path.join(RIG, dll));
  }
  for (const f of fs.readdirSync(DEBUG)) {
    if (/\.png$/.test(f) || /_done\.json$/.test(f) || /_request\.json$/.test(f)) fs.unlinkSync(path.join(DEBUG, f));
  }
  if (fs.existsSync(LOG)) fs.truncateSync(LOG, 0);
  // A FRESH relay database every run: the name the publisher claims, the
  // identities, the stream, none of it should carry over.
  for (const f of fs.readdirSync(RELAY_DIR)) {
    if (/^relay\.db/.test(f)) fs.unlinkSync(path.join(RELAY_DIR, f));
  }
}

// ── IPC helpers (the same file-drop protocol verify-screens uses) ────────────
function clearDone(...names) {
  for (const n of names) {
    const p = path.join(DEBUG, n);
    if (fs.existsSync(p)) fs.unlinkSync(p);
  }
}
async function waitFile(name, timeoutMs, pollMs = 300) {
  const p = path.join(DEBUG, name);
  const t0 = Date.now();
  while (Date.now() - t0 < timeoutMs) {
    if (fs.existsSync(p)) {
      try {
        return JSON.parse(fs.readFileSync(p, "utf8"));
      } catch {
        // half-written; retry
      }
    }
    await sleep(pollMs);
  }
  return null;
}
function req(name, body) {
  fs.writeFileSync(path.join(DEBUG, name), JSON.stringify(body));
}
async function waitBoot(timeoutMs) {
  const t0 = Date.now();
  while (Date.now() - t0 < timeoutMs) {
    if (fs.existsSync(LOG)) {
      const txt = fs.readFileSync(LOG, "utf8");
      if (/PANIC/.test(txt)) throw new Error("PANIC during boot (see run.log)");
      if (/Cloud noise volumes generated/.test(txt)) return true;
    }
    await sleep(1000);
  }
  throw new Error("the exe did not finish booting in time");
}
async function screen(body, timeoutMs = 30000, placingMs = 60000) {
  const t0 = Date.now();
  for (;;) {
    clearDone("screen_done.json");
    req("screen_request.json", body);
    const d = await waitFile("screen_done.json", timeoutMs);
    if (!d) return { ok: false, error: `no screen_done.json within ${timeoutMs} ms for ${JSON.stringify(body)}` };
    if (d.ok === false && /no screen named/.test(d.error || "") && Date.now() - t0 < placingMs) {
      await sleep(1000);
      continue;
    }
    return d;
  }
}
function panicCount() {
  if (!fs.existsSync(LOG)) return 0;
  return (fs.readFileSync(LOG, "utf8").match(/PANIC/g) || []).length;
}
function logSize() {
  try {
    return fs.statSync(LOG).size;
  } catch {
    return 0;
  }
}
/** The bytes of run.log written after `from`. Reading a TAIL is the point:
 *  the sockets this run legitimately opened earlier must not be able to mask
 *  the one it must not open now. */
function logTail(from) {
  if (!fs.existsSync(LOG)) return "";
  const size = fs.statSync(LOG).size;
  if (size <= from) return "";
  const fd = fs.openSync(LOG, "r");
  try {
    const buf = Buffer.alloc(size - from);
    fs.readSync(fd, buf, 0, buf.length, from);
    return buf.toString("utf8");
  } finally {
    fs.closeSync(fd);
  }
}

async function httpJson(url, timeoutMs = 3000) {
  try {
    const r = await fetch(url, { signal: AbortSignal.timeout(timeoutMs) });
    if (!r.ok) return null;
    return await r.json();
  } catch {
    return null;
  }
}

// ── The live run ─────────────────────────────────────────────────────────────
const stamp = new Date().toISOString().replace(/[-:]/g, "").replace(/\..+/, "").replace("T", "-");
const OUT = path.join(RIG, "runs", stamp);
const SERVER_URL = `http://127.0.0.1:${PORT}`;

async function main() {
  setupRig();
  fs.mkdirSync(OUT, { recursive: true });
  const manifest = {
    kind: "verify-live-screen",
    stamp,
    exe: EXE,
    rig: RIG,
    server: SERVER_URL,
    live_screen: LIVE_SCREEN,
    static_screen: STATIC_SCREEN,
    stream: STREAM,
    steps: [],
    relay: null,
    publisher: null,
    live: {},
    offline: {},
    no_server: {},
    panics: 0,
  };
  const save = () => fs.writeFileSync(path.join(OUT, "manifest.json"), JSON.stringify(manifest, null, 2));
  const step = (id, d) => {
    manifest.steps.push({ id, ...(d && typeof d === "object" ? d : { value: d }) });
    save();
    log(`${d && d.ok !== false ? "ok  " : "FAIL"} ${pad(id, 20)} ${summarize(d)}`);
    return d;
  };
  const keepPng = (d, name) => {
    if (!d || !d.png) return null;
    const src = path.join(RIG, d.png);
    if (!fs.existsSync(src)) return null;
    fs.copyFileSync(src, path.join(OUT, name));
    return name;
  };

  // Everything we start, so nothing is left running whatever happens.
  let relayProc = null;
  let publisher = null;
  let gamePid = null;
  let killed = false;
  const killAll = () => {
    if (killed) return;
    killed = true;
    for (const p of [publisher, relayProc]) {
      if (!p || p.killed) continue;
      try {
        execSync(`taskkill /PID ${p.pid} /T /F`, { stdio: "ignore" });
      } catch {}
    }
    if (gamePid) {
      try {
        execSync(`taskkill /PID ${gamePid} /T /F`, { stdio: "ignore" });
      } catch {}
    }
    killRigProcesses();
  };
  process.on("exit", () => {
    if (!KEEP_OPEN) killAll();
  });
  const watchdog = setTimeout(() => {
    log(`TIMEOUT after ${TIMEOUT_MS / 60000} min; killing everything`);
    manifest.steps.push({ id: "timeout", ok: false, error: `run exceeded ${TIMEOUT_MS / 60000} min` });
    manifest.panics = panicCount();
    save();
    killAll();
    printVerdict("RESULT: ", manifest, OUT);
    process.exit(2);
  }, TIMEOUT_MS);

  try {
    // ── 1. A relay of our own, on loopback, with a database we just deleted.
    log(`starting a local relay on ${SERVER_URL} (db ${rel(path.join(RELAY_DIR, "relay.db"))})`);
    const relayOut = fs.openSync(RELAY_LOG, "w");
    // cwd is the relay's OWN folder, not the rig root: the relay resolves a
    // handful of paths relative to its working directory (data/uploads,
    // data/server-config.json), and the rig root's `data` is a junction to
    // the repository's real data directory. Running it here means it can
    // only ever write inside its own scratch folder, and it reads no server
    // config, so every feature is at its default (on), which is what a
    // fresh self-hosted node gets.
    relayProc = spawn(path.join(RIG, "HumanityOS.exe"), ["--headless"], {
      cwd: RELAY_DIR,
      stdio: ["ignore", relayOut, relayOut],
      env: {
        ...process.env,
        PORT: String(PORT),
        DATABASE_PATH: path.join(RELAY_DIR, "relay.db"),
        // Never the operator's focus, even though a headless relay opens no
        // window: the marker files and this var are how the whole rig asks.
        HUMANITY_NO_FOCUS: "1",
      },
    });
    let health = null;
    for (let t0 = Date.now(); Date.now() - t0 < 60000; ) {
      health = await httpJson(`${SERVER_URL}/health`);
      if (health && health.status === "ok") break;
      await sleep(500);
    }
    manifest.relay = health
      ? { ok: true, detail: `${SERVER_URL} answered /health (pid ${relayProc.pid})` }
      : { ok: false, detail: `${SERVER_URL} never answered /health; see ${rel(RELAY_LOG)}` };
    step("relay", manifest.relay);
    if (!health) throw new Error("the local relay did not come up");

    // ── 2. The test pattern. It claims the stream name first (the relay
    // resolves a stream id from the publisher's REGISTERED name), then
    // publishes until we stop it.
    log(`publishing a test pattern as "${STREAM}"`);
    publisher = spawn(
      process.execPath,
      [
        path.join(__dirname, "live-publish.js"),
        "--server", SERVER_URL,
        "--name", STREAM,
        "--register",
        "--size", "640x360",
        "--fps", "8",
        "--seconds", "0",
        "--quiet",
      ],
      { cwd: REPO, stdio: ["ignore", "pipe", "pipe"] },
    );
    const pubOut = [];
    publisher.stdout.on("data", (b) => pubOut.push(String(b)));
    publisher.stderr.on("data", (b) => pubOut.push(String(b)));
    let listed = null;
    for (let t0 = Date.now(); Date.now() - t0 < 60000; ) {
      const dir = await httpJson(`${SERVER_URL}/api/live`);
      const row = dir && Array.isArray(dir.streams) ? dir.streams.find((s) => s.id === STREAM) : null;
      if (row) {
        listed = row;
        break;
      }
      await sleep(500);
    }
    manifest.publisher = listed
      ? { ok: true, listed: true, detail: `/api/live lists "${listed.id}" (${listed.frames} frames so far)`, row: listed }
      : { ok: false, listed: false, detail: `/api/live never listed "${STREAM}": ${pubOut.join("").trim() || "(no output)"}` };
    step("publisher", manifest.publisher);
    if (!listed) throw new Error("the test publisher never went live");

    // ── 3. The game, pointed at OUR relay.
    log(`launching ${path.basename(EXE)} in ${rel(RIG)}`);
    const child = spawn(path.join(RIG, "HumanityOS.exe"), [], {
      cwd: RIG,
      detached: true,
      stdio: "ignore",
      env: { ...process.env, HUMANITY_NO_FOCUS: "1" },
    });
    gamePid = child.pid;
    fs.writeFileSync(path.join(RIG, "probe_pid.txt"), String(gamePid));
    child.unref();

    log("waiting for boot...");
    await waitBoot(180000);
    log("entering world (autopilot), server = the local relay...");
    clearDone("autopilot_done.json");
    req("autopilot_request.json", { server_url: SERVER_URL, user_name: "LiveRig", character_name: "LiveRig" });
    const ap = await waitFile("autopilot_done.json", 180000);
    if (!ap || ap.ok !== true) throw new Error(`autopilot failed: ${JSON.stringify(ap)}`);
    step("autopilot", ap);

    // Park facing the live wall. The home is built a few frames after world
    // entry, so retry while the camera request answers "no screen named".
    log(`parking in front of ${LIVE_SCREEN}...`);
    let cam = null;
    for (let t0 = Date.now(); Date.now() - t0 < 90000; ) {
      clearDone("camera_done.json");
      req("camera_request.json", { station: "home", screen: LIVE_SCREEN, distance_m: 2.2 });
      cam = await waitFile("camera_done.json", 30000);
      if (cam && cam.ok === true) break;
      if (cam && /no screen named/.test(cam.error || "")) {
        await sleep(1000);
        continue;
      }
      break;
    }
    if (!cam || cam.ok !== true) throw new Error(`camera park failed: ${JSON.stringify(cam)}`);
    step("camera", cam);
    await sleep(3000);

    // ── 4. THE PICTURE. Wait for frames, then two snapshots a second apart,
    // with a static screen measured over the same second as the floor.
    let st = null;
    for (let t0 = Date.now(); Date.now() - t0 < 90000; ) {
      st = await screen({ screen: LIVE_SCREEN, action: "status" });
      if (st.ok === true && st.connected === true && Number(st.frames) > 0) break;
      await sleep(1000);
    }
    manifest.live.status = st;
    step("live_status", st);

    const ca = await screen({ screen: STATIC_SCREEN, action: "snapshot" });
    const la = await screen({ screen: LIVE_SCREEN, action: "snapshot" });
    manifest.live.control_before = keepPng(ca, "control_before.png");
    manifest.live.before = keepPng(la, "live_before.png");
    step("snapshot_a", la);
    await sleep(1000);
    const lb = await screen({ screen: LIVE_SCREEN, action: "snapshot" });
    const cb = await screen({ screen: STATIC_SCREEN, action: "snapshot" });
    manifest.live.after = keepPng(lb, "live_after.png");
    manifest.live.control_after = keepPng(cb, "control_after.png");
    step("snapshot_b", lb);
    manifest.live.status_after = await screen({ screen: LIVE_SCREEN, action: "status" });
    save();

    // ── 5. THE STREAM STOPS. The wall must say so, about this stream, and
    // must not show a parse error. The provider waits out its own retry
    // (screens/live.rs RETRY_AFTER, 15 s) before the relay can answer "not
    // live", so this is patient.
    log("stopping the publisher...");
    try {
      execSync(`taskkill /PID ${publisher.pid} /T /F`, { stdio: "ignore" });
    } catch {}
    publisher = null;
    let off = null;
    for (let t0 = Date.now(); Date.now() - t0 < 120000; ) {
      off = await screen({ screen: LIVE_SCREEN, action: "status" });
      if (off.ok === true && off.heading === "Stream offline" && String(off.detail || "").includes("streaming as")) break;
      await sleep(2000);
    }
    manifest.offline.status = off;
    step("offline_status", off);
    // And the words are really ON THE GLASS, not just in a status field.
    manifest.offline.find = await screen({ screen: LIVE_SCREEN, find: { text: "Stream offline" } });
    step("offline_find", manifest.offline.find);
    manifest.offline.png = keepPng(await screen({ screen: LIVE_SCREEN, action: "snapshot" }), "offline.png");
    save();

    // ── 6. NO SERVER. Clear it the same way it was set, then prove both the
    // sentence and the silence: zero sockets opened after this mark.
    const mark = logSize();
    log("clearing the server...");
    clearDone("autopilot_done.json");
    req("autopilot_request.json", { server_url: "" });
    const ap2 = await waitFile("autopilot_done.json", 60000);
    step("clear_server", ap2 || { ok: false, error: "no autopilot_done.json" });
    let ns = null;
    for (let t0 = Date.now(); Date.now() - t0 < 60000; ) {
      ns = await screen({ screen: LIVE_SCREEN, action: "status" });
      if (ns.ok === true && ns.heading === "No server set") break;
      await sleep(1000);
    }
    manifest.no_server.status = ns;
    step("no_server_status", ns);
    manifest.no_server.find = await screen({ screen: LIVE_SCREEN, find: { text: "No server set" } });
    step("no_server_find", manifest.no_server.find);
    manifest.no_server.png = keepPng(await screen({ screen: LIVE_SCREEN, action: "snapshot" }), "no_server.png");
    // Give the screen a few seconds of framed ticks: if it were going to
    // retry, a 15 s RETRY_AFTER has already elapsed since the last viewer
    // ended, so a retry would fire on the very next tick.
    for (let i = 0; i < 6; i++) {
      await screen({ screen: LIVE_SCREEN, action: "status" });
      await sleep(500);
    }
    const tailText = logTail(mark);
    const mine = new RegExp(`\\[Screens\\] watch:${STREAM}: opening a viewer`, "g");
    manifest.no_server.log_tail = {
      from: mark,
      bytes: tailText.length,
      opened: (tailText.match(mine) || []).length,
      said_no_server: (tailText.match(/No server set/g) || []).length,
    };
    step("no_connection_attempted", { ok: manifest.no_server.log_tail.opened === 0, ...manifest.no_server.log_tail });
    save();

    // A viewport capture for the human reading the evidence folder.
    clearDone("screenshot_done.json");
    req("screenshot_request.json", { note: "verify-live-screen live wall" });
    const shot = await waitFile("screenshot_done.json", 30000);
    if (shot && shot.ok && shot.path) {
      const src = path.isAbsolute(shot.path) ? shot.path : path.join(RIG, shot.path);
      if (fs.existsSync(src)) {
        fs.copyFileSync(src, path.join(OUT, "viewport.png"));
        manifest.screenshot = "viewport.png";
      }
    }
  } catch (e) {
    manifest.steps.push({ id: "abort", ok: false, error: String(e.message || e) });
    log(`ABORT ${e.message || e}`);
  }
  clearTimeout(watchdog);
  manifest.panics = panicCount();
  try {
    fs.copyFileSync(LOG, path.join(OUT, "run.log"));
    manifest.log = "run.log";
  } catch {}
  try {
    fs.copyFileSync(RELAY_LOG, path.join(OUT, "relay.log"));
  } catch {}
  save();
  if (!KEEP_OPEN) killAll();
  else log(`--keep-open: the rig is still running (game pid ${gamePid}); taskkill //PID ${gamePid} //T //F`);
  const pass = printVerdict("RESULT: ", manifest, OUT);
  if (!pass) {
    console.log("What to do:");
    console.log(`  1. open the PNGs in ${rel(OUT)} (live before/after, the control pair, the two message pages)`);
    console.log(`  2. read ${rel(path.join(OUT, "run.log"))} for [Screens] watch: lines (and any PANIC)`);
    console.log(`  3. read ${rel(path.join(OUT, "relay.log"))} for what the relay thought of the publisher`);
    console.log(`  4. re-judge without booting: node scripts/verify-live-screen.js --dry-verdict ${rel(path.join(OUT, "manifest.json"))}`);
    console.log("");
  }
  process.exit(pass ? 0 : 2);
}

function summarize(d) {
  if (!d || typeof d !== "object") return String(d);
  const parts = [];
  if (d.detail && !d.heading) parts.push(String(d.detail));
  if (d.heading) parts.push(`${JSON.stringify(d.heading)} / ${JSON.stringify(d.detail)}`);
  if (d.connected !== undefined) parts.push(`connected=${d.connected}`);
  if (d.frames !== undefined) parts.push(`frames=${d.frames}`);
  if (d.may_connect !== undefined) parts.push(`may_connect=${d.may_connect}`);
  if (d.found !== undefined) parts.push(`found=${d.found}`);
  if (d.opened !== undefined) parts.push(`opened=${d.opened} said_no_server=${d.said_no_server}`);
  if (d.png) parts.push(d.png);
  if (d.error) parts.push(d.error);
  return parts.join(" ");
}

main().catch((e) => {
  console.error(e);
  process.exit(2);
});
