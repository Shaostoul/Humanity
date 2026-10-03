// A THROWAWAY RELAY for tests and rigs: a copy of the release exe, run
// --headless from a new temp folder, on a free port, with its own database,
// and killed by PID however the calling process ends.
//
// Moved here on 2026-10-03 from scripts/tests/second-player-relay.test.js so
// that test and scripts/verify-copresence.js start their relays the same way.
// Used by both; keep it std-only (node built-ins), like the rest of scripts/lib.
//
// WHY A COPY: while a program runs, Windows locks its file, and
// target/release/HumanityOS.exe is the very file another session's release
// build or `just deliver` writes, so running it in place could make their link
// fail. A leftover copy is also easy to spot by its path (a temp folder whose
// name starts with the prefix the caller gave).
//
// WHY ITS OWN FOLDER AS THE WORKING FOLDER: the relay resolves data/uploads and
// data/server-config.json relative to where it runs, so it can only ever write
// inside its temp folder, never in %APPDATA%\HumanityOS or the repository's
// data/.
//
// HOW IT IS NEVER LEFT RUNNING: every relay started here is killed BY PID (and
// its temp folder removed) by stop(), when this process exits for any other
// reason, and on Ctrl+C, Ctrl+Break, a closed console or a polite kill. The
// release exe is a Windows GUI program with no console, so it never receives a
// Ctrl+C itself.
// On Windows there is a second net under this one: Node puts every child it
// starts (unless `detached`) in a Windows job that is killed when Node exits,
// however Node exits. Seen 2026-10-03 (in the relay test, before the move):
// with the "exit" handler deleted and the process made to exit mid-test
// (node -e "setTimeout(() => process.exit(1), 9000); require('./scripts/tests/second-player-relay.test.js')"),
// the relay was gone anyway, but its temp folder, holding the 37 MB exe copy,
// was left behind. So on Windows the handlers are what tidy up; on Linux and
// macOS, which have no such job, they are also what stop the relay. Checked
// the same way with the handlers in place: the relay gone, the folder gone,
// and with process.emit("SIGINT") in place of the exit, exit code 130.
//
// NEVER PRODUCTION: the relay listens on 127.0.0.1 only as far as anyone here
// uses it, and its database is brand new every time.

"use strict";

const { spawn, execSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const net = require("node:net");
const http = require("node:http");
const path = require("node:path");

const EXE_NAME = process.platform === "win32" ? "HumanityOS.exe" : "HumanityOS";

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// Every relay started and not yet stopped, so the exit and signal handlers
// can reach them.
const live = new Set();
let hooked = false;

/** Kill every live relay and remove its folder. Synchronous on purpose, so it
 *  also works inside an "exit" handler, where nothing asynchronous runs any
 *  more. */
function cleanupAllSync() {
  for (const h of [...live]) {
    h.kill();
    h.removeDir();
    live.delete(h);
  }
}

/** Install the exit and signal handlers once, on the first relay started. A
 *  signal ends the process the way an interrupted program ends (130 for
 *  Ctrl+C), and process.exit runs every other "exit" handler on the way out,
 *  so a caller's own cleanup still happens. */
function hookProcessExit() {
  if (hooked) return;
  hooked = true;
  process.on("exit", cleanupAllSync);
  for (const [sig, code] of [["SIGINT", 130], ["SIGTERM", 143], ["SIGBREAK", 149], ["SIGHUP", 129]]) {
    process.on(sig, () => {
      cleanupAllSync();
      process.exit(code);
    });
  }
}

/** A port nobody is listening on right now. */
function freePort() {
  return new Promise((resolve, reject) => {
    const srv = net.createServer();
    srv.once("error", reject);
    srv.listen(0, "127.0.0.1", () => {
      const { port } = srv.address();
      srv.close(() => resolve(port));
    });
  });
}

/** GET a URL and parse its JSON body; null on any failure. */
function getJson(url, timeoutMs = 2000) {
  return new Promise((resolve) => {
    const req = http.get(url, { timeout: timeoutMs }, (res) => {
      let body = "";
      res.on("data", (c) => (body += c));
      res.on("end", () => {
        try {
          resolve(JSON.parse(body));
        } catch {
          resolve(null);
        }
      });
    });
    req.on("error", () => resolve(null));
    req.on("timeout", () => {
      req.destroy();
      resolve(null);
    });
  });
}

/** This shell's environment, minus anything that would make the throwaway
 *  relay act as a real one: every HUMANITY_* setting (focus, owner keys, data
 *  folders), the bot password, admins, an outside webhook, and the port and
 *  database it is about to be given. */
function relayEnv(port, dbPath) {
  const env = {};
  const drop = new Set(["API_SECRET", "ADMIN_KEYS", "WEBHOOK_URL", "WEBHOOK_TOKEN", "PORT", "DATABASE_PATH", "RUST_LOG"]);
  for (const [k, v] of Object.entries(process.env)) {
    const K = k.toUpperCase();
    if (K.startsWith("HUMANITY_") || drop.has(K)) continue;
    env[k] = v;
  }
  // HUMANITY_NO_FOCUS: never the operator's focus, even though a headless
  // relay opens no window (the whole rig asks the same way).
  Object.assign(env, { PORT: String(port), DATABASE_PATH: dbPath, HUMANITY_NO_FOCUS: "1", RUST_LOG: "info" });
  return env;
}

/**
 * Start a throwaway relay and wait (bounded) for it to answer /health.
 *
 *   sourceExe  the build to copy and run (required).
 *   prefix     the temp folder's name prefix, e.g. "second-player-relay-test-".
 *   config     written as data/server-config.json in its folder (optional).
 *   healthTimeoutMs  how long to wait for /health (default 60 s).
 *
 * Resolves with a handle whether or not /health answered: `health` is the
 * parsed /health body, or null, and the caller decides what that means
 * (logText() says why). Rejects only if the folder or the copy could not be
 * made.
 *
 * The handle: { dir, exe, port, url (ws://.../ws), httpUrl, dbPath, logPath,
 * pid, proc, health, exited(), logText(), kill(), removeDir(), stop() }.
 */
async function startRelay({ sourceExe, prefix = "throwaway-relay-", config = null, healthTimeoutMs = 60000 } = {}) {
  if (!sourceExe || !fs.existsSync(sourceExe)) throw new Error(`no relay exe to copy: ${sourceExe}`);
  hookProcessExit();
  let dir = fs.mkdtempSync(path.join(os.tmpdir(), prefix));
  let proc = null;
  let exited = false;
  const h = {
    dir,
    exe: path.join(dir, EXE_NAME),
    port: 0,
    url: "",
    httpUrl: "",
    dbPath: path.join(dir, "data", "relay.db"),
    logPath: path.join(dir, "relay.log"),
    pid: 0,
    proc: null,
    health: null,
    exited: () => exited,
    logText() {
      try {
        return fs.readFileSync(h.logPath, "utf8");
      } catch {
        return "";
      }
    },
    /** Kill the relay by its PID. Synchronous. Never called on a relay
     *  already known to have exited, so a reused PID is never hit. */
    kill() {
      if (!proc || exited) return;
      try {
        if (process.platform === "win32") execSync(`taskkill /PID ${proc.pid} /T /F`, { stdio: "ignore" });
        else process.kill(proc.pid, "SIGKILL");
      } catch {}
      exited = true;
    },
    /** Remove the temp folder (best effort: a just-killed exe can hold its
     *  file for a moment, hence the retries). */
    removeDir() {
      if (!dir) return;
      try {
        fs.rmSync(dir, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 });
      } catch {}
      dir = null;
    },
    /** Kill it, wait (at most 5 s) for it to be gone, remove its folder. */
    async stop() {
      if (proc && !exited) {
        const gone = new Promise((r) => (proc.exitCode !== null ? r() : proc.once("exit", r)));
        h.kill();
        await Promise.race([gone, new Promise((r) => setTimeout(r, 5000).unref())]);
      }
      h.removeDir();
      live.delete(h);
    },
  };
  live.add(h);

  fs.mkdirSync(path.join(dir, "data"));
  if (config) fs.writeFileSync(path.join(dir, "data", "server-config.json"), JSON.stringify(config));
  fs.copyFileSync(sourceExe, h.exe);

  h.port = await freePort();
  h.url = `ws://127.0.0.1:${h.port}/ws`;
  h.httpUrl = `http://127.0.0.1:${h.port}`;
  const log = fs.openSync(h.logPath, "w");
  proc = spawn(h.exe, ["--headless"], { cwd: dir, env: relayEnv(h.port, h.dbPath), stdio: ["ignore", log, log], windowsHide: true });
  fs.closeSync(log);
  h.proc = proc;
  h.pid = proc.pid;
  proc.once("exit", () => (exited = true));

  for (const t0 = Date.now(); Date.now() - t0 < healthTimeoutMs && !exited; ) {
    const health = await getJson(`${h.httpUrl}/health`);
    if (health && health.status === "ok") {
      h.health = health;
      break;
    }
    await sleep(300);
  }
  return h;
}

module.exports = { EXE_NAME, startRelay, freePort, getJson, relayEnv };
