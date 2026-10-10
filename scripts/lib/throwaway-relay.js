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
// WHY THAT FOLDER HOLDS THE TREE'S data/ (critic review, 2026-10-03). The relay
// READS data/ relative to where it runs too (npc/crew.ron, npc/chores.ron,
// ships/room_equipment.ron, market/categories.json, governance/...), as the
// VPS relay does from /opt/Humanity. A folder holding only server-config.json
// made every one of those reads miss: the relay served the copies built into
// the exe, or nothing at all for a file with no built-in copy (no crew chores,
// no market categories), while the freshness gate called the binary current.
// So mirrorData() puts the tree's data/ in the folder first: files up to 1 MiB
// are COPIED (a write by the relay stays in its copy), bigger ones (planet
// tiles, star catalogues, images: none of which the relay writes) are HARD
// LINKED, so a start does not copy 200 MB. Folders are real folders, never
// links, so a file the relay creates lands in its own folder. What the relay
// owns (its database, uploads/, backups/, keys, the claim code) and
// server-config.json (the caller's, or none) are never mirrored. The one way a
// relay could still write into the tree: an admin edit, through the Files API,
// of a mirrored file over 1 MiB (a hard link writes through). No rig or test
// makes one.
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
// NEVER PRODUCTION: its database is brand new every time, and it LISTENS ON
// 127.0.0.1 ONLY.
//
// LOOPBACK ONLY, AND CHECKED (2026-10-03). The relay listens on every network
// interface unless told otherwise (its BIND_ADDRESS setting, default 0.0.0.0,
// which self-hosted LAN nodes need). On Windows, a program that listens on
// 0.0.0.0 makes Windows Defender Firewall stop whoever is at the keyboard with
// "Windows Defender Firewall has blocked some features of this app", once for
// every exe PATH, and this file runs the relay from a NEW temp folder every
// time: 54 of the operator's firewall rules were for these copies, one prompt
// each, several a day, interrupting whatever he was doing. Nothing here needs
// the network (every client is on 127.0.0.1), so relayEnv() sets
// BIND_ADDRESS=127.0.0.1, and once /health answers, startRelay() asks the
// operating system which addresses the relay's PID is actually listening on
// (netstat on Windows, ss on Linux, lsof on macOS) and refuses, killing the
// relay, unless every one is loopback. That is what stops a quiet revert: an
// exe built before BIND_ADDRESS existed, or a relayEnv() that lost the line,
// fails loudly here. Not BEFORE any prompt, though: the check can only look
// once the relay is listening, and Windows asks the moment a program listens
// on the wildcard, so a revert still costs ONE prompt (for that run's temp
// path) and then stops the run, and every run after it, until it is fixed.
// The prevention is the BIND_ADDRESS line; this check is what makes a revert
// cost one prompt instead of one per run, forever, unnoticed. Do not "fix"
// that failure by removing the check; see docs/INCIDENT-PLAYBOOK.md.
// scripts/tests/throwaway-relay.test.js proves startRelay() runs it.

"use strict";

const { spawn, execSync } = require("node:child_process");
const fs = require("node:fs");
const crypto = require("node:crypto");
const os = require("node:os");
const net = require("node:net");
const http = require("node:http");
const path = require("node:path");

const EXE_NAME = process.platform === "win32" ? "HumanityOS.exe" : "HumanityOS";

/** The address every throwaway relay listens on (its BIND_ADDRESS). */
const LOOPBACK_BIND = "127.0.0.1";

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

/** What makes a relay a throwaway one on loopback: relayEnv sets these, and a
 *  caller's `env` may never (`checkExtraEnv`). */
const OWN_SETTINGS = ["PORT", "DATABASE_PATH", "BIND_ADDRESS", "HUMANITY_NO_FOCUS"];

/** Refuse a caller's `env` (startRelay's option) that names one of the
 *  settings that make the relay a throwaway loopback one (OWN_SETTINGS, in any
 *  case of letters), or a value that is not a string. Returns it as plain
 *  string pairs. */
function checkExtraEnv(extra) {
  if (extra === null || extra === undefined) return {};
  if (typeof extra !== "object" || Array.isArray(extra)) throw new Error(`startRelay's env must be an object of NAME: "value" pairs, got ${JSON.stringify(extra)}`);
  const out = {};
  for (const [k, v] of Object.entries(extra)) {
    if (OWN_SETTINGS.includes(k.toUpperCase())) {
      throw new Error(`startRelay's env may not set ${k}: a throwaway relay sets ${OWN_SETTINGS.join(", ")} itself (its own port and database, loopback only). Nothing was started.`);
    }
    if (typeof v !== "string") throw new Error(`startRelay's env value for ${k} must be a string, got ${JSON.stringify(v)}`);
    out[k] = v;
  }
  return out;
}

/** This shell's environment, minus anything that would make the throwaway
 *  relay act as a real one: every HUMANITY_* setting (focus, owner keys, data
 *  folders), the bot password, admins, an outside webhook, and the port,
 *  database and listen address it is about to be given. Then `extra`, the
 *  caller's own settings (startRelay's `env`, ship homes increment 5,
 *  2026-10-05): how a rig names an admin (ADMIN_KEYS) on purpose, where the
 *  shell's is always dropped. `extra` may not name OWN_SETTINGS. */
function relayEnv(port, dbPath, extra = null) {
  const own = checkExtraEnv(extra);
  const env = {};
  const drop = new Set(["API_SECRET", "ADMIN_KEYS", "WEBHOOK_URL", "WEBHOOK_TOKEN", "PORT", "DATABASE_PATH", "RUST_LOG", "BIND_ADDRESS", "TURN_BIND", "TURN_PORT"]);
  for (const [k, v] of Object.entries(process.env)) {
    const K = k.toUpperCase();
    if (K.startsWith("HUMANITY_") || drop.has(K)) continue;
    env[k] = v;
  }
  // The caller's own, before the throwaway's settings below (which it may not name anyway).
  Object.assign(env, own);
  // HUMANITY_NO_FOCUS: never the operator's focus, even though a headless
  // relay opens no window (the whole rig asks the same way).
  // BIND_ADDRESS: loopback only, so Windows never raises a firewall prompt
  // for this temp copy (see the top of this file). startRelay() checks it.
  // TURN_BIND / TURN_PORT: the relay's call forwarder (src/relay/call_forwarder.rs)
  // opens a UDP port of its own; keep it on loopback too, on a port the system
  // picks, so two rigs (or a rig beside the operator's own node) never share 3478.
  Object.assign(env, {
    PORT: String(port),
    DATABASE_PATH: dbPath,
    BIND_ADDRESS: LOOPBACK_BIND,
    TURN_BIND: LOOPBACK_BIND,
    TURN_PORT: "0",
    HUMANITY_NO_FOCUS: "1",
    RUST_LOG: "info",
  });
  return env;
}

// ── Which addresses a process is listening on, from the operating system ──

/** Split "host:port" as the OS tools print it ("127.0.0.1:3210",
 *  "[::1]:3210", "*:3210", Linux ss's "[::ffff:127.0.0.1]:3210") into
 *  { host, port }; host loses its brackets. */
function splitHostPort(s) {
  const i = s.lastIndexOf(":");
  if (i < 0) return null;
  let host = s.slice(0, i);
  const port = Number(s.slice(i + 1));
  if (host.startsWith("[") && host.endsWith("]")) host = host.slice(1, -1);
  // Linux prints an interface scope on link-local and some loopback rows.
  host = host.replace(/%.*$/, "");
  if (!Number.isInteger(port)) return null;
  return { host, port };
}

/** True for an address only this computer can reach: 127.0.0.0/8, ::1, and
 *  127.x written as an IPv4-mapped IPv6 address. Everything else, including
 *  every wildcard spelling (0.0.0.0, ::, *), is reachable from the network. */
function isLoopbackHost(host) {
  const h = String(host).toLowerCase();
  return /^127\.\d+\.\d+\.\d+$/.test(h) || h === "::1" || /^::ffff:127\.\d+\.\d+\.\d+$/.test(h);
}

/**
 * The TCP listening sockets of process `pid` in the text a platform's tool
 * printed. Pure, so the tests can feed it captured output.
 *
 *   win32   `netstat -ano`: "TCP  127.0.0.1:3210  0.0.0.0:0  LISTENING  1234".
 *           A listening row is matched by its foreign address (0.0.0.0:0 or
 *           [::]:0) as well as by the word LISTENING, which Windows
 *           translates on non-English systems.
 *   linux   `ss -Hltnp`: "LISTEN 0 1024 127.0.0.1:3210 0.0.0.0:* users:((...pid=1234...))".
 *   darwin  `lsof -nP -a -p PID -iTCP -sTCP:LISTEN -Fn`: one "n127.0.0.1:3210"
 *           line per socket (already filtered to the PID by lsof).
 *
 * Returns [{ host, port, line }].
 */
function parseListening(text, platform, pid) {
  const rows = [];
  for (const raw of String(text).split(/\r?\n/)) {
    const line = raw.trim();
    if (!line) continue;
    let local = null;
    if (platform === "win32") {
      const f = line.split(/\s+/);
      if (f[0] !== "TCP" || f.length < 5) continue;
      const listening = f[3] === "LISTENING" || f[2] === "0.0.0.0:0" || f[2] === "[::]:0";
      if (!listening || Number(f[f.length - 1]) !== pid) continue;
      local = f[1];
    } else if (platform === "linux") {
      const f = line.split(/\s+/);
      if (f[0] !== "LISTEN" || f.length < 5) continue;
      const m = line.match(/pid=(\d+)/g) || [];
      if (!m.some((p) => Number(p.slice(4)) === pid)) continue;
      local = f[3];
    } else if (platform === "darwin") {
      if (!line.startsWith("n")) continue;
      local = line.slice(1);
    } else {
      continue;
    }
    const hp = splitHostPort(local);
    if (hp) rows.push({ ...hp, line });
  }
  return rows;
}

/** Ask the operating system which TCP addresses `pid` is listening on. Throws
 *  when the tool cannot be run: an unverifiable relay is not a verified one. */
function listeningSockets(pid, platform = process.platform) {
  let cmd;
  if (platform === "win32") cmd = "netstat -ano";
  else if (platform === "linux") cmd = "ss -Hltnp";
  else if (platform === "darwin") cmd = `lsof -nP -a -p ${pid} -iTCP -sTCP:LISTEN -Fn`;
  else throw new Error(`cannot list listening sockets on ${platform}`);
  let text;
  try {
    text = execSync(cmd, { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"], windowsHide: true, timeout: 20000 });
  } catch (e) {
    // lsof exits 1 when it finds nothing; that is an answer, not a failure.
    if (platform === "darwin" && e.status === 1) text = String(e.stdout || "");
    else throw new Error(`could not run "${cmd}" to see what the relay listens on: ${e.message}`);
  }
  return parseListening(text, platform, pid);
}

/** Why `rows` (one process's listening sockets) are NOT a loopback-only
 *  listener on `port`, or null when they are. Pure. Nothing found is a
 *  failure too: a check that finds no sockets has verified nothing. */
function loopbackOnlyProblem(rows, port) {
  const fmt = (r) => (r.host.includes(":") ? `[${r.host}]` : r.host) + `:${r.port}`;
  if (!rows.length) return "the operating system shows no listening socket for it at all, so nothing was verified";
  if (!rows.some((r) => r.port === port)) {
    return `it is not listening on its port ${port} (it listens on ${rows.map(fmt).join(", ")})`;
  }
  const open = rows.filter((r) => !isLoopbackHost(r.host));
  if (open.length) return `it listens on ${open.map(fmt).join(", ")}, reachable from the network, not on loopback only`;
  return null;
}

/**
 * Throw unless process `pid` listens on loopback only (and on `port`).
 * Returns the rows it found, so a caller can show them.
 */
function assertLoopbackOnly(pid, port, platform = process.platform) {
  const rows = listeningSockets(pid, platform);
  const problem = loopbackOnlyProblem(rows, port);
  if (problem) {
    throw new Error(
      `the dev relay (pid ${pid}) is not a loopback-only listener: ${problem}.\n` +
        (rows.length ? `  what the OS shows:\n${rows.map((r) => `    ${r.line}`).join("\n")}\n` : "") +
        `  A dev relay must listen on 127.0.0.1 only (BIND_ADDRESS=${LOOPBACK_BIND}): on Windows a\n` +
        `  wildcard listener raises a firewall prompt for every new exe path, and the operator\n` +
        `  has to click it away. Check that relayEnv() still sets BIND_ADDRESS, and that the exe\n` +
        `  is new enough to read it (built after 2026-10-03). docs/INCIDENT-PLAYBOOK.md explains.`,
    );
  }
  return rows;
}

// ── The tree's data/, for a relay's own folder ────────────────────────────
const REPO_DATA = path.resolve(__dirname, "..", "..", "data");
// Top-level names under data/ that the relay writes or that a throwaway relay
// must own: never mirrored (a hard link would let a write reach the tree).
const RELAY_OWNED = /^(relay\.db.*|server-config\.json|uploads|backups|backup\.key|vapid_private\.key|owner-claim-code\.txt)$/i;
// Files above this are hard-linked instead of copied.
const LINK_OVER = 1 << 20;

/** Remove `p` without ever following a link: a junction or symlink is removed
 *  as a link, a folder is emptied entry by entry. (A tree of copies and hard
 *  links is what mirrorData makes; this never reaches into the tree's data/.) */
function removeNoFollow(p) {
  let st;
  try {
    st = fs.lstatSync(p);
  } catch {
    return;
  }
  if (st.isSymbolicLink()) {
    try {
      fs.unlinkSync(p);
    } catch {
      fs.rmdirSync(p); // a directory junction on Windows
    }
  } else if (st.isDirectory()) {
    for (const e of fs.readdirSync(p)) removeNoFollow(path.join(p, e));
    fs.rmdirSync(p);
  } else {
    fs.unlinkSync(p);
  }
}

/**
 * Put the tree's data/ into a relay's own folder (see "WHY THAT FOLDER HOLDS THE
 * TREE'S data/" at the top). `destData` is emptied first (without following a
 * link). Junctions and symlinks in the source are skipped, never followed.
 * Returns { files, copied, linked, skipped: [top-level names left out] }.
 */
function mirrorData(srcData, destData) {
  removeNoFollow(destData);
  fs.mkdirSync(destData, { recursive: true });
  const out = { files: 0, copied: 0, linked: 0, skipped: [] };
  const walk = (src, dest, top) => {
    for (const e of fs.readdirSync(src, { withFileTypes: true })) {
      if (top && RELAY_OWNED.test(e.name)) {
        out.skipped.push(e.name);
        continue;
      }
      const s = path.join(src, e.name);
      const d = path.join(dest, e.name);
      if (e.isSymbolicLink()) continue; // never follow a link out of the tree
      if (e.isDirectory()) {
        fs.mkdirSync(d);
        walk(s, d, false);
      } else if (e.isFile()) {
        out.files++;
        if (fs.statSync(s).size > LINK_OVER) {
          try {
            fs.linkSync(s, d);
            out.linked++;
            continue;
          } catch {
            /* another volume, or not allowed: copy */
          }
        }
        fs.copyFileSync(s, d);
        out.copied++;
      }
    }
  };
  walk(srcData, destData, true);
  return out;
}

/**
 * Start a throwaway relay and wait (bounded) for it to answer /health.
 *
 *   sourceExe  the build to copy and run (required).
 *   expectSha256  the SHA-256 of the exe the caller's freshness gate judged
 *                 (runFreshGate's result.exe_sha256), REQUIRED: the copy must be
 *                 those very bytes or startRelay rejects before starting it. A
 *                 build finishing between the gate and this copy would otherwise
 *                 run a relay nobody checked (BUG-133), and a caller whose gate
 *                 recorded no hash cannot pass one by accident (2026-10-03: a
 *                 null used to skip the check without a word).
 *   prefix     the temp folder's name prefix, e.g. "second-player-relay-test-".
 *   config     written as data/server-config.json in its folder (optional).
 *   dataFrom   the data/ folder to mirror into it (default: this tree's data/;
 *              see mirrorData). null: none (the relay then reads built-in
 *              copies only, which is not what a deployed relay does).
 *   healthTimeoutMs  how long to wait for /health (default 60 s).
 *   checkListening   (pid, port) => rows, throwing to refuse the relay.
 *                    Default assertLoopbackOnly; a test passes its own to
 *                    prove startRelay() calls it and stops a refused relay.
 *   spawnProcess     (exe, args, options) => ChildProcess. Default
 *                    child_process.spawn; a test passes one that runs a tiny
 *                    stand-in relay in node, so the check above can be seen
 *                    working without a release build.
 *   env        the relay's own settings, { NAME: "value" } (optional, ship homes
 *              increment 5): ADMIN_KEYS, for a rig that needs an admin
 *              (verify-copresence --build: its rank holder). The shell's
 *              ADMIN_KEYS is dropped either way. It may not name PORT,
 *              DATABASE_PATH, BIND_ADDRESS or HUMANITY_NO_FOCUS: refused, with
 *              nothing started.
 *
 * Resolves with a handle whether or not /health answered: `health` is the
 * parsed /health body, or null, and the caller decides what that means
 * (logText() says why). Rejects if the folder or the copy could not be made,
 * and, once /health has answered, if the relay is NOT listening on loopback
 * only (the relay is stopped first; see "LOOPBACK ONLY" at the top).
 *
 * The handle: { dir, exe, port, url (ws://.../ws), httpUrl, dbPath, logPath,
 * pid, proc, health, listening, exited(), logText(), kill(), removeDir(),
 * stop(), data }. `listening` is what the OS showed the relay listening on (rows
 * { host, port, line }), so a caller can print the evidence; `data` is what
 * mirrorData put in its folder ({ files, copied, linked, skipped }), or null.
 */
async function startRelay({
  sourceExe,
  prefix = "throwaway-relay-",
  config = null,
  expectSha256 = null,
  dataFrom = REPO_DATA,
  healthTimeoutMs = 60000,
  checkListening = assertLoopbackOnly,
  spawnProcess = spawn,
  env = null,
} = {}) {
  if (!sourceExe || !fs.existsSync(sourceExe)) throw new Error(`no relay exe to copy: ${sourceExe}`);
  // Before anything is made: an env naming the throwaway's own settings starts nothing.
  checkExtraEnv(env);
  if (typeof expectSha256 !== "string" || !/^[0-9a-f]{64}$/.test(expectSha256)) {
    throw new Error(
      `startRelay needs expectSha256, the SHA-256 the freshness gate recorded for the exe it judged ` +
        `(runFreshGate(...).result.exe_sha256); got ${JSON.stringify(expectSha256)}. Without it the relay copy ` +
        "could be a binary nobody checked (BUG-133). Nothing was started.",
    );
  }
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
    listening: [],
    data: null,
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

  // The tree's data/ first (the relay reads it from its working folder), then
  // the caller's config, which mirrorData never copies from the tree.
  if (dataFrom) h.data = mirrorData(dataFrom, path.join(dir, "data"));
  else fs.mkdirSync(path.join(dir, "data"));
  if (config) fs.writeFileSync(path.join(dir, "data", "server-config.json"), JSON.stringify(config));
  fs.copyFileSync(sourceExe, h.exe);
  const got = crypto.createHash("sha256").update(fs.readFileSync(h.exe)).digest("hex");
  if (got !== expectSha256) {
    await h.stop();
    throw new Error(
      `the relay copy is not the exe the freshness gate judged: the copy of ${sourceExe} hashes ${got}, ` +
        `the judged exe ${expectSha256} (it changed in between, most likely a build finishing). Nothing was started.`,
    );
  }

  h.port = await freePort();
  h.url = `ws://127.0.0.1:${h.port}/ws`;
  h.httpUrl = `http://127.0.0.1:${h.port}`;
  const log = fs.openSync(h.logPath, "w");
  proc = spawnProcess(h.exe, ["--headless"], { cwd: dir, env: relayEnv(h.port, h.dbPath, env), stdio: ["ignore", log, log], windowsHide: true });
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

  // Serving, so listening: now make the OS prove it is on loopback only. A
  // relay that is not gets killed and its folder removed BEFORE the error
  // goes up, so a refused relay is never left running for a caller to forget.
  // (A relay that never answered /health is the caller's to judge, as before.)
  if (h.health) {
    try {
      h.listening = checkListening(h.pid, h.port);
    } catch (e) {
      await h.stop();
      throw e;
    }
  }
  return h;
}

module.exports = {
  EXE_NAME,
  LOOPBACK_BIND,
  REPO_DATA,
  mirrorData,
  startRelay,
  freePort,
  getJson,
  relayEnv,
  parseListening,
  isLoopbackHost,
  listeningSockets,
  loopbackOnlyProblem,
  assertLoopbackOnly,
};
