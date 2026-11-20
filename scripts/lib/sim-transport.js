// The headless transport's client (the headless sim runner, increment 5, 2026-11-20;
// docs/design/headless-sim-runner.md, section 2): starts `HumanityOS --sim verbs -` and speaks its
// JSON lines, one request out and one answer back (src/engine/sim_verbs.rs says what each line
// does). A rig that walks the game with verbs drives this the way it drives a windowed game's
// `debug/` file drop, so one route walks either:
//
//   const sim = await ST.start({ exe, dataDir, cwd, logFile });   // in the world when it resolves
//   await sim.verbs({ walk: "1", cam: "x,y,z,yaw,pitch" });        // the showcase verbs
//   await sim.step(1.5);                                           // 1.5 s of the world
//   await sim.recordBegin(2.2); await sim.verbs({ hold: "forward", hold_s: "1.2" });
//   const done = await sim.recordEnd();                            // the recorder's done JSON
//   await sim.quit();
//
// No window and no GPU: the machine guard does not count a `--sim` run as a game (machine-guard.js
// isSimRun), and the run is put below normal priority, inside the share of the PC the operator
// keeps (CLAUDE.md). The game's log (stderr) goes to `logFile`, the run's run.log.

"use strict";

const fs = require("fs");
const { spawn } = require("child_process");
const PLAT = require("./platform.js");

/** How long the boot and the homestead's entry may take, ms (1.3 to 1.5 s measured, 2026-11-15). */
const BOOT_TIMEOUT_MS = 120000;
/** How long one answer may take, ms (an hour of the world stepped is about 90 s). */
const LINE_TIMEOUT_MS = 300000;

/**
 * Start the transport. Resolves with the client once the game says it is in the world (its first
 * line, `hello`), rejects when it exits or says nothing in time.
 * opts: { exe, dataDir, cwd, logFile, args: [] }.
 */
function start(opts) {
  const args = ["--sim", "verbs", "-", ...(opts.dataDir ? ["--data", opts.dataDir] : []), ...(opts.args || [])];
  const log = opts.logFile ? fs.openSync(opts.logFile, "w") : "ignore";
  const child = spawn(opts.exe, args, { cwd: opts.cwd || process.cwd(), stdio: ["pipe", "pipe", log], windowsHide: true });
  if (child.pid) PLAT.lowerPriority(child.pid);
  let buf = "";
  const waiting = [];
  let exited = null;
  const fail = (e) => {
    while (waiting.length) waiting.shift().reject(e);
  };
  child.stdout.setEncoding("utf8");
  child.stdout.on("data", (chunk) => {
    buf += chunk;
    let nl;
    while ((nl = buf.indexOf("\n")) >= 0) {
      const line = buf.slice(0, nl).trim();
      buf = buf.slice(nl + 1);
      if (!line) continue;
      const w = waiting.shift();
      if (!w) continue;
      try {
        w.resolve(JSON.parse(line));
      } catch (e) {
        w.reject(new Error(`the transport answered a line that is not JSON: ${line.slice(0, 200)}`));
      }
    }
  });
  child.on("exit", (code) => {
    exited = code;
    fail(new Error(`the headless game exited (code ${code})${opts.logFile ? `: see ${opts.logFile}` : ""}`));
    if (typeof log === "number") {
      try {
        fs.closeSync(log);
      } catch {}
    }
  });
  child.on("error", (e) => fail(e));

  /** The next answer, or a rejection after `ms`. */
  const next = (ms, what) =>
    new Promise((resolve, reject) => {
      if (exited !== null) return reject(new Error(`the headless game has exited (code ${exited})`));
      const t = setTimeout(() => reject(new Error(`${what}: no answer in ${ms / 1000} s`)), ms);
      waiting.push({
        resolve: (v) => {
          clearTimeout(t);
          resolve(v);
        },
        reject: (e) => {
          clearTimeout(t);
          reject(e);
        },
      });
    });
  /** Send one request line and wait for its answer; an answer with ok false throws. */
  const send = async (req) => {
    const answer = next(LINE_TIMEOUT_MS, JSON.stringify(req));
    child.stdin.write(`${JSON.stringify(req)}\n`);
    const a = await answer;
    if (!a || a.ok !== true) throw new Error(`the headless game refused ${JSON.stringify(req)}: ${JSON.stringify(a)}`);
    return a;
  };
  const client = {
    child,
    pid: child.pid,
    hello: null,
    verbs: (verbs) => send({ do: "verbs", verbs }),
    step: (s) => send({ do: "step", s }),
    recordBegin: (s) => send({ do: "record_begin", s }),
    recordEnd: () => send({ do: "record_end" }),
    record: (s) => send({ do: "record", s }),
    quit: async () => {
      if (exited !== null) return;
      try {
        await send({ do: "quit" });
      } catch {}
      try {
        child.stdin.end();
      } catch {}
    },
    kill: () => {
      try {
        PLAT.killTree(child.pid, { force: true });
      } catch {}
    },
  };
  return next(BOOT_TIMEOUT_MS, "the headless boot").then((hello) => {
    if (!hello || hello.ok !== true) throw new Error(`the headless game did not enter the world: ${JSON.stringify(hello)}`);
    client.hello = hello;
    return client;
  });
}

module.exports = { start, BOOT_TIMEOUT_MS, LINE_TIMEOUT_MS };
