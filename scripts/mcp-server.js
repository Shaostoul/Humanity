#!/usr/bin/env node
// THE DEV MCP BRIDGE (2026-10-22): HumanityOS's development tools for an AI session, as Model
// Context Protocol tools over stdio. Claude Code starts it from the project's .mcp.json; any
// MCP client can (`node scripts/mcp-server.js`). docs/ai/mcp.md is the guide.
//
// Two kinds of tool:
//
//   THE GAME's (game_*): served by the running game itself on 127.0.0.1 (src/engine/mcp), each a
//   thin call into the dev IPC the rigs drive. This bridge forwards them, with the endpoint's
//   token, to the game it finds through debug/mcp_endpoint.json. Their list is the tree's
//   src/engine/mcp/tools.json, the same file the game compiles in, so they are offered before a
//   game is up. Two more are composed here from the rigs' own data: game_showcase_reset (the
//   sticky pins scripts/lib/showcase-pins.js releases before every vantage) and game_goto_vantage
//   (a vantage of tests/visual/vantages.json replayed). game_boot / game_status / game_close
//   start and stop the game.
//
//   THE TREE's (dev_*), needing no game: the rig vantages and rigs, running a rig for its verdict,
//   lib tests by filter, the lints, the journal and the top of PRIORITIES.
//
// WHY A NODE BRIDGE, NOT `HumanityOS --mcp`: Claude Code keeps a stdio server running for the
// whole session, and Windows will not let cargo replace an exe that is running, so a bridge that
// WAS the game's exe would make every `cargo build --release` of the session fail. And the dev
// tools here (rigs, lib tests) are node and cargo anyway.
//
// SAFETY (docs/ai/mcp.md): the game is booted only through the rigs' launcher (probe-sweep: the
// freshness gate, the machine guard, the machine-wide boot lock, a portable sandbox whose server
// is on this computer, background and silent, never taking focus); it waits its turn behind other
// games and rigs; at most one game, and a rig is refused while this bridge's game is up. A game
// this bridge booted is closed when the bridge exits and after HUMANITY_MCP_IDLE_MIN minutes
// (default 20) with no game tool call, so it never sits on the GPU. No tool sends a message,
// signs, spends, changes the identity or reaches a server (the game refuses those verbs itself).
//
// stdout carries only protocol messages, one per line; everything else goes to stderr.

"use strict";

const fs = require("fs");
const path = require("path");
const readline = require("readline");
const { spawn, execFileSync } = require("child_process");
const P = require("./lib/mcp-protocol.js");

const REPO = path.resolve(__dirname, "..");
const VERSION = (() => {
  try {
    return fs.readFileSync(path.join(REPO, "Cargo.toml"), "utf8").match(/^version\s*=\s*"(.+?)"/m)[1];
  } catch {
    return "0";
  }
})();
const SERVER_INFO = { name: "humanityos-dev", title: "HumanityOS development tools", version: VERSION };
const GAME_TOOLS_FILE = path.join(REPO, "src", "engine", "mcp", "tools.json");
const VANTAGES_FILE = path.join(REPO, "tests", "visual", "vantages.json");
/** The rig folder game_boot boots in: a folder of its own, so an ordinary sweep's setup never stops it. */
const RIG = path.resolve(process.env.HUMANITY_MCP_RIG || path.join(REPO, ".probe-rig", "mcp"));
const IDLE_MIN = Number(process.env.HUMANITY_MCP_IDLE_MIN || 20);
const DEFAULT_VANTAGE = "home-overview-noon";
const MODERN = P.MODERN_VERSIONS[0];

const log = (...a) => console.error("[mcp-server]", ...a);

// ── Where the game is ─────────────────────────────────────────────────────────────────────────

/** The endpoint files to look in, first match wins. */
function endpointCandidates() {
  if (process.env.HUMANITY_MCP_ENDPOINT) return [path.resolve(process.env.HUMANITY_MCP_ENDPOINT)];
  return [
    path.join(RIG, "debug", "mcp_endpoint.json"),
    path.join(REPO, ".probe-rig", "debug", "mcp_endpoint.json"),
    path.join(REPO, "debug", "mcp_endpoint.json"),
  ];
}

function pidAlive(pid) {
  if (!Number.isInteger(pid) || pid <= 0) return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch (e) {
    return e.code === "EPERM";
  }
}

/** The live endpoint: { ok: true, ...file, file } | { ok: false, refused, file } | null (no game). */
function findEndpoint() {
  for (const file of endpointCandidates()) {
    let e;
    try {
      e = JSON.parse(fs.readFileSync(file, "utf8"));
    } catch {
      continue;
    }
    if (!pidAlive(e.pid)) continue; // a game that has ended left its file behind
    return { ...e, file };
  }
  return null;
}

/** Is this endpoint a game this bridge's rig booted (one it may close for idleness)? */
const isOurs = (e) => !!e && !!e.game_dir && path.resolve(e.game_dir).toLowerCase() === RIG.toLowerCase();

let lastGameCall = Date.now();
/** The game this bridge process booted (game_boot), closed when the bridge exits. */
let bootedPid = null;

/** Call one of the game's tools over its endpoint. Resolves to a CallToolResult. */
async function callGame(name, args, { signal } = {}) {
  const e = findEndpoint();
  if (!e) {
    return P.toolError(
      `No game is running with the dev MCP endpoint on. Start one with game_boot (it boots the release build in a rig ` +
        `sandbox, in the background, through the rigs' launcher). Looked in: ${endpointCandidates().join(", ")}`
    );
  }
  if (e.ok !== true) return P.toolError(`The game (pid ${e.pid}) did not start its MCP endpoint: ${e.refused || "no reason given"}`);
  lastGameCall = Date.now();
  const timeoutS = Number(args && args.timeout_s) > 0 ? Number(args.timeout_s) : 0;
  const body = {
    jsonrpc: "2.0",
    id: Date.now(),
    method: "tools/call",
    params: {
      name,
      arguments: args || {},
      _meta: {
        [P.META_VERSION]: MODERN,
        "io.modelcontextprotocol/clientCapabilities": {},
        "io.modelcontextprotocol/clientInfo": SERVER_INFO,
      },
    },
  };
  const timeout = AbortSignal.timeout(Math.max(180, timeoutS + 60) * 1000);
  let res;
  try {
    res = await fetch(e.url, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json, text/event-stream",
        Authorization: `Bearer ${e.token}`,
        "MCP-Protocol-Version": MODERN,
        "Mcp-Method": "tools/call",
        "Mcp-Name": P.headerValue(name),
      },
      body: JSON.stringify(body),
      signal: signal ? AbortSignal.any([signal, timeout]) : timeout,
    });
  } catch (err) {
    return P.toolError(`Could not reach the game at ${e.url} (pid ${e.pid}): ${err.message}. It may have closed or stalled; game_status says which.`);
  }
  let reply;
  try {
    reply = await res.json();
  } catch {
    return P.toolError(`The game answered HTTP ${res.status} with no JSON.`);
  }
  if (reply.error) {
    if (reply.error.code === P.ERR.INVALID_PARAMS && /Unknown tool/.test(reply.error.message || "")) {
      return P.toolError(
        `The running game (v${e.version}) does not know ${name}: its build predates the tool. Rebuild ` +
          `(cargo build --features native --release), game_close, then game_boot.`
      );
    }
    return P.toolError(`The game refused the call (HTTP ${res.status}, ${reply.error.code}): ${reply.error.message}`);
  }
  const r = { ...reply.result };
  delete r.resultType; // this bridge's own protocol layer adds it for a modern client
  delete r._meta;
  return r;
}

// ── Running a script, with progress and cancellation ───────────────────────────────────────────

/** Run a command to the end. Resolves { code, out } (stdout and stderr together, the tail kept). */
function run(cmd, args, { env, timeoutMs, signal, notify, label } = {}) {
  return new Promise((resolve) => {
    const child = spawn(cmd, args, { cwd: REPO, env: env || process.env, windowsHide: true, stdio: ["ignore", "pipe", "pipe"] });
    let out = "";
    const keep = (d) => {
      out += d.toString();
      if (out.length > 400000) out = out.slice(-300000);
    };
    child.stdout.on("data", keep);
    child.stderr.on("data", keep);
    const started = Date.now();
    const tick = setInterval(() => {
      if (!notify) return;
      const last = out.trimEnd().split(/\r?\n/).pop() || "";
      notify(`${label || cmd}: ${Math.round((Date.now() - started) / 1000)} s. ${last.slice(0, 200)}`);
    }, 15000);
    const kill = () => {
      try {
        execFileSync("taskkill", ["/PID", String(child.pid), "/T", "/F"], { stdio: "ignore" });
      } catch {
        try {
          child.kill();
        } catch {}
      }
    };
    const timer = timeoutMs
      ? setTimeout(() => {
          out += `\n[mcp-server] stopped after ${Math.round(timeoutMs / 60000)} min\n`;
          kill();
        }, timeoutMs)
      : null;
    if (signal) signal.addEventListener("abort", kill, { once: true });
    child.on("error", (e) => {
      clearInterval(tick);
      if (timer) clearTimeout(timer);
      resolve({ code: -1, out: `${out}\ncould not start ${cmd}: ${e.message}` });
    });
    child.on("close", (code) => {
      clearInterval(tick);
      if (timer) clearTimeout(timer);
      resolve({ code: code === null ? -1 : code, out });
    });
  });
}

const tail = (text, n) => String(text).trimEnd().split(/\r?\n/).slice(-n).join("\n");

// ── The game's lifecycle ──────────────────────────────────────────────────────────────────────

/** The probe-sweep command game_boot runs, and the environment it runs with (pure, tested). */
function bootCommand({ vantage = DEFAULT_VANTAGE, waitMin = 10, rig = RIG, env = process.env } = {}) {
  const args = [
    path.join(REPO, "scripts", "probe-sweep.js"),
    "--rig", rig,
    "--only", vantage,
    "--keep-open", // the game stays up for the tools when the sweep's own vantage is done
    "--width", "0", // a boot, not a measurement: any window size will do
    "--game-only", // wait for other games, not for builds (agents compile all day)
    "--wait-min", String(waitMin),
  ];
  // The two switches the game reads: the endpoint (src/engine/mcp) and the frame-cost and room GI
  // drops game_frame_costs and game_home_state read. Nothing else is added: probe-sweep's launcher
  // makes it background, silent and sandboxed.
  return { cmd: process.execPath, args, env: { ...env, HUMANITY_DEV_MCP: "1", HUMANITY_FRAME_COSTS: "1" } };
}

async function gameBoot(args, ctx) {
  const live = findEndpoint();
  if (live && live.ok === true) {
    return P.textResult(`A game is already up: pid ${live.pid}, v${live.version}, ${live.url}. game_close first to boot another.`, live);
  }
  const vantage = String(args.vantage || DEFAULT_VANTAGE);
  if (!/^[a-z0-9-]+$/.test(vantage)) return P.toolError(`vantage ${JSON.stringify(vantage)} is not a vantage id (dev_rig_vantages lists them)`);
  const waitMin = Number(args.wait_min) > 0 ? Number(args.wait_min) : 10;
  // A previous boot's file must not be mistaken for this one's.
  try {
    fs.unlinkSync(path.join(RIG, "debug", "mcp_endpoint.json"));
  } catch {}
  const { cmd, args: argv, env } = bootCommand({ vantage, waitMin });
  log(`game_boot: ${cmd} ${argv.join(" ")}`);
  const r = await run(cmd, argv, { env, timeoutMs: (waitMin + 15) * 60000, signal: ctx.signal, notify: ctx.notify, label: "game_boot" });
  const e = findEndpoint();
  if (e && e.ok === true) {
    bootedPid = e.pid;
    lastGameCall = Date.now();
    const meaning = r.code === 0 ? "its vantage captured" : `its vantage reported exit ${r.code}; the game is up regardless`;
    return P.textResult(
      `The game is up: pid ${e.pid}, v${e.version}, endpoint ${e.url}, folder ${e.game_dir} (${meaning}). ` +
        `It closes when this bridge exits or after ${IDLE_MIN} min with no game tool call; game_close closes it now.\n\n` +
        tail(r.out, 25),
      { pid: e.pid, version: e.version, url: e.url, game_dir: e.game_dir, sweep_exit: r.code }
    );
  }
  if (e && e.ok === false) return P.toolError(`The game started but refused to open its endpoint: ${e.refused}\n\n${tail(r.out, 25)}`);
  const why =
    r.code === 1
      ? "the rig REFUSED before booting (the freshness gate, or the machine never went quiet: another game or rig held it)"
      : `the sweep ended with exit ${r.code} and no endpoint file appeared`;
  return P.toolError(`No game came up: ${why}. The rig's last lines:\n\n${tail(r.out, 40)}`);
}

function listGames() {
  try {
    const MG = require("./lib/machine-guard.js");
    return MG.listInstances();
  } catch {
    return [];
  }
}

function gameStatus() {
  const e = findEndpoint();
  const others = listGames().filter((p) => !e || p.pid !== e.pid);
  const st = {
    endpoint: e ? { ok: e.ok, pid: e.pid, version: e.version, url: e.url, game_dir: e.game_dir, refused: e.refused, file: e.file } : null,
    ours: isOurs(e),
    idle_close_min: IDLE_MIN,
    idle_for_s: Math.round((Date.now() - lastGameCall) / 1000),
    other_instances: others.map((p) => ({ pid: p.pid, name: p.name, exe: p.exe })),
    tree_version: VERSION,
  };
  const head = e
    ? e.ok
      ? `Game up: pid ${e.pid}, v${e.version}${e.version !== VERSION ? ` (the tree is v${VERSION})` : ""}, ${e.url}.`
      : `A game (pid ${e.pid}) is running but refused its endpoint: ${e.refused}`
    : "No game with the dev MCP endpoint is running.";
  const more = others.length ? ` Other HumanityOS processes: ${others.map((p) => `${p.name} pid ${p.pid}`).join(", ")}.` : "";
  return P.textResult(head + more, st);
}

/** Close a game by pid: a graceful close (it saves), forced after 15 s. */
async function closePid(pid) {
  try {
    execFileSync("taskkill", ["/PID", String(pid)], { stdio: "ignore" });
  } catch {}
  for (let i = 0; i < 30 && pidAlive(pid); i++) await new Promise((r) => setTimeout(r, 500));
  if (pidAlive(pid)) {
    try {
      execFileSync("taskkill", ["/PID", String(pid), "/T", "/F"], { stdio: "ignore" });
    } catch {}
    return "forced after 15 s";
  }
  return "closed";
}

async function gameClose() {
  const e = findEndpoint();
  if (!e) return P.textResult("No game with the dev MCP endpoint is running: nothing to close.");
  const how = await closePid(e.pid);
  try {
    fs.unlinkSync(e.file);
  } catch {}
  return P.textResult(`Game pid ${e.pid} ${how}.`, { pid: e.pid, how });
}

// Close a game this bridge's rig booted once nobody has used it for IDLE_MIN minutes: a game
// left up holds the one GPU, and every other agent's rig waits behind it. Started by main().
function startIdleClose() {
  const t = setInterval(async () => {
    const e = findEndpoint();
    if (!isOurs(e) || !(IDLE_MIN > 0)) return;
    if (Date.now() - lastGameCall < IDLE_MIN * 60000) return;
    log(`closing the game (pid ${e.pid}): no game tool call for ${IDLE_MIN} min`);
    await closePid(e.pid);
  }, 60000);
  t.unref();
}

// ── Composed game tools ──────────────────────────────────────────────────────────────────────

async function showcaseReset(args, ctx) {
  const { STICKY_PIN_RESETS, DIAG_RESETS } = require("./lib/showcase-pins.js");
  return callGame("game_showcase", { verbs: { ...DIAG_RESETS, ...STICKY_PIN_RESETS } }, ctx);
}

function readVantages() {
  const v = JSON.parse(fs.readFileSync(VANTAGES_FILE, "utf8"));
  return Array.isArray(v) ? v : v.vantages || [];
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** Replay a vantage the way probe-sweep's vantage loop does (pins, park, clock held, settle,
 *  post_showcase, re-park, final_showcase), without its judges: rig_run for a judged capture. */
async function gotoVantage(args, ctx) {
  const v = readVantages().find((x) => x.id === args.id);
  if (!v) return P.toolError(`No vantage ${JSON.stringify(args.id)} in tests/visual/vantages.json (dev_rig_vantages lists them).`);
  const { STICKY_PIN_RESETS, DIAG_RESETS } = require("./lib/showcase-pins.js");
  const steps = [];
  const fail = (r, what) => P.toolError(`${what}: ${(r.content || []).map((c) => c.text).join(" ")}\nDone so far: ${steps.join("; ")}`);
  let r = await callGame("game_showcase", { verbs: { ...DIAG_RESETS, ...STICKY_PIN_RESETS, ...(v.showcase || {}) } }, ctx);
  if (r.isError) return fail(r, "showcase");
  steps.push("pins and showcase");
  await sleep(v.showcase ? 3500 : 1500);
  r = await callGame("game_camera", { request: v.camera }, ctx);
  if (r.isError) return fail(r, "park");
  steps.push("parked");
  await callGame("game_showcase", { verbs: { time_scale: "0" } }, ctx);
  await sleep((v.settle_s ?? 8) * 1000);
  if (v.post_showcase) {
    r = await callGame("game_showcase", { verbs: v.post_showcase }, ctx);
    if (r.isError) return fail(r, "post_showcase");
    steps.push("post_showcase");
    await sleep((v.post_settle_s ?? 3) * 1000);
  }
  const park = await callGame("game_camera", { request: v.camera }, ctx);
  if (park.isError) return fail(park, "re-park");
  steps.push("re-parked");
  await sleep(v.hold_altitude ? 900 : 6000);
  for (const step of v.final_showcase ? (Array.isArray(v.final_showcase) ? v.final_showcase : [v.final_showcase]) : []) {
    r = await callGame("game_showcase", { verbs: step }, ctx);
    if (r.isError) return fail(r, "final_showcase");
    steps.push("final_showcase");
    await sleep((v.final_settle_s ?? 4) * 1000);
  }
  const head = { type: "text", text: `At vantage ${v.id}: ${v.desc}\nSteps: ${steps.join("; ")}.\n(Replayed without the sweep's judges; dev_rig_run probe-sweep judges a capture.)` };
  if (args.screenshot === false) return { content: [head, ...(park.content || [])], structuredContent: park.structuredContent, isError: false };
  const shot = await callGame("game_screenshot", {}, ctx);
  return { content: [head, ...(shot.content || [])], structuredContent: { camera_done: park.structuredContent, screenshot: shot.structuredContent }, isError: !!shot.isError };
}

// ── The tree's tools (no game needed) ───────────────────────────────────────────────────────────

/** The rigs the Justfile runs as `<name> *ARGS: node scripts/<file>.js [fixed flags] "$@"`: every
 *  verify-* rig and probe-sweep, read from the Justfile so a new rig is offered without a change here. */
function listRigs() {
  const text = fs.readFileSync(path.join(REPO, "Justfile"), "utf8").replace(/\r\n/g, "\n");
  const lines = text.split("\n");
  const rigs = [];
  for (let i = 0; i < lines.length - 1; i++) {
    const m = lines[i].match(/^((?:verify-[a-z0-9-]+)|probe-sweep) \*ARGS:$/);
    const n = lines[i + 1].match(/^\s+node scripts\/([\w.-]+\.js)((?: --[\w-]+)*) "\$@"$/);
    if (!m || !n) continue;
    let j = i - 1;
    while (j >= 0 && /^\[.*\]$/.test(lines[j])) j--; // [positional-arguments]
    const about = j >= 0 && lines[j].startsWith("#") ? lines[j].replace(/^#\s?/, "") : "";
    rigs.push({ name: m[1], script: n[1], flags: n[2].trim() ? n[2].trim().split(/\s+/) : [], about });
  }
  return rigs;
}

async function rigRun(args, ctx) {
  const rig = listRigs().find((r) => r.name === args.rig);
  if (!rig) return P.toolError(`No rig ${JSON.stringify(args.rig)}. dev_rig_list names them: ${listRigs().map((r) => r.name).join(", ")}`);
  const up = findEndpoint();
  if (up) {
    return P.toolError(
      `Refused: a game is up (pid ${up.pid}), and a rig waits for every running game (one GPU). game_close it first, then run the rig.`
    );
  }
  const extra = Array.isArray(args.args) ? args.args.map(String) : [];
  if (rig.name === "probe-sweep" && !extra.includes("--only")) {
    if (!args.vantage) return P.toolError("probe-sweep needs a vantage (or --only in args): a whole sweep is hundreds of vantages and hours");
    extra.push("--only", String(args.vantage));
  }
  const minutes = Number(args.timeout_min) > 0 ? Number(args.timeout_min) : 90;
  const argv = [path.join(REPO, "scripts", rig.script), ...rig.flags, ...extra];
  const r = await run(process.execPath, argv, { timeoutMs: minutes * 60000, signal: ctx.signal, notify: ctx.notify, label: rig.name });
  // The rigs share one exit convention: 0 passed, 1 refused before anything was judged, 2 failed.
  const meaning = { 0: "PASSED", 1: "REFUSED (nothing was judged)", 2: "FAILED" }[r.code] || `ended with exit ${r.code}`;
  const verdictLines = r.out.split(/\r?\n/).filter((l) => /\b(PASS|FAIL|REFUSED|VERDICT|ok:|manifest)\b/i.test(l)).slice(-25);
  return {
    content: [{ type: "text", text: `${rig.name} ${meaning}.\n\nVerdict lines:\n${verdictLines.join("\n") || "(none)"}\n\nLast lines:\n${tail(r.out, 40)}` }],
    structuredContent: { rig: rig.name, exit: r.code, meaning },
    isError: r.code !== 0,
  };
}

async function libTest(args, ctx) {
  const filters = (Array.isArray(args.filters) ? args.filters : [args.filters]).filter(Boolean).map(String);
  if (!filters.length) return P.toolError("give at least one filter (a module path or test name piece): the whole suite takes minutes; use `just verify` for that");
  if (filters.some((f) => f.startsWith("-") && f !== "--nocapture")) return P.toolError("filters are test names; the only flag taken is --nocapture");
  const minutes = Number(args.timeout_min) > 0 ? Number(args.timeout_min) : 40;
  const env = { ...process.env, CARGO_BUILD_JOBS: process.env.CARGO_BUILD_JOBS || "4" };
  const r = await run(process.execPath, [path.join(REPO, "scripts", "lib-tests.js"), ...filters], { env, timeoutMs: minutes * 60000, signal: ctx.signal, notify: ctx.notify, label: "lib tests" });
  const result = (r.out.match(/test result: .*/g) || []).pop() || "(no test result line: did it compile? see the last lines)";
  const failed = [...r.out.matchAll(/^test (\S+) \.\.\. FAILED$/gm)].map((m) => m[1]);
  const oom = /E0786|os error 1455|STATUS_STACK|out of memory/i.test(r.out) ? "\nThis looks like the machine ran out of memory while compiling (E0786, os error 1455): wait and run again." : "";
  return {
    content: [{ type: "text", text: `${result}${failed.length ? `\nFailed: ${failed.join(", ")}` : ""}${oom}\n\nLast lines:\n${tail(r.out, 60)}` }],
    structuredContent: { exit: r.code, result, failed },
    isError: r.code !== 0,
  };
}

async function lints(args, ctx) {
  const r = await run("just", ["lints"], { timeoutMs: 15 * 60000, signal: ctx.signal, notify: ctx.notify, label: "lints" });
  const failedLine = r.out.split(/\r?\n/).find((l) => l.startsWith("LINT FAILED"));
  return {
    content: [{ type: "text", text: `${r.code === 0 ? "All lints pass." : failedLine || `just lints ended with exit ${r.code}`}\n\n${tail(r.out, r.code === 0 ? 25 : 80)}` }],
    structuredContent: { exit: r.code },
    isError: r.code !== 0,
  };
}

function journal(args) {
  const j = JSON.parse(fs.readFileSync(path.join(REPO, "data", "coordination", "orchestrator_state.json"), "utf8"));
  const n = Math.min(Math.max(Number(args.entries) || 5, 1), 40);
  const recent = (j.recent_decisions || []).slice(-n);
  const text = [
    `Focus: ${typeof j.current_focus === "string" ? j.current_focus.slice(0, 1500) : JSON.stringify(j.current_focus).slice(0, 1500)}`,
    "",
    `The last ${recent.length} decisions (newest last):`,
    ...recent.map((d) => `\n${d.date}: ${d.decision}\n  Why: ${d.why || ""}`),
  ].join("\n");
  return P.textResult(text, { recent_decisions: recent });
}

function priorities(args) {
  const text = fs.readFileSync(path.join(REPO, "docs", "PRIORITIES.md"), "utf8").replace(/\r\n/g, "\n");
  const at = text.indexOf("## TIER 0");
  const n = Math.min(Math.max(Number(args.lines) || 80, 10), 400);
  const lines = text.slice(at >= 0 ? at : 0).split("\n").slice(0, n);
  return P.textResult(lines.join("\n"));
}

function vantages(args) {
  const all = readVantages();
  const f = String(args.filter || "").toLowerCase();
  const hits = all.filter((v) => !f || v.id.toLowerCase().includes(f) || String(v.desc || "").toLowerCase().includes(f));
  const n = Math.min(Math.max(Number(args.limit) || 100, 1), 600);
  const shown = hits.slice(0, n);
  const text = `${hits.length} of ${all.length} vantages${f ? ` match ${JSON.stringify(f)}` : ""}${hits.length > n ? ` (first ${n} shown)` : ""}:\n` +
    shown.map((v) => `${v.id}: ${String(v.desc || "").slice(0, 160)}`).join("\n");
  return P.textResult(text, { count: hits.length, ids: shown.map((v) => v.id) });
}

// ── The tool table ───────────────────────────────────────────────────────────────────────────

const obj = (properties = {}, required = []) => ({ type: "object", properties, ...(required.length ? { required } : {}), additionalProperties: false });
const RO = { readOnlyHint: true, destructiveHint: false, idempotentHint: true, openWorldHint: false };
const ACT = { readOnlyHint: false, destructiveHint: false, idempotentHint: false, openWorldHint: false };

/** The bridge's own tools: [definition, handler(args, ctx)]. */
const OWN = [
  [{ name: "game_boot", title: "Boot the game for the tools", annotations: ACT,
     description: "Boot this tree's release build (target/release/HumanityOS.exe) in a rig sandbox with the dev MCP endpoint on, through the rigs' launcher: the freshness gate, waiting its turn behind other games and rigs (wait_min), the boot lock, a portable sandbox whose server is on this computer, background and silent, never taking focus. Enters the world and parks at a vantage (default home-overview-noon), then leaves the game up for the game_* tools. Takes one to a few minutes. Build first if the source changed (cargo build --features native --release).",
     inputSchema: obj({ vantage: { type: "string", description: "Where to start, a vantage id (dev_rig_vantages)." }, wait_min: { type: "number", minimum: 1, maximum: 120, description: "How long to wait for the machine to be free, default 10." } }) }, gameBoot],
  [{ name: "game_status", title: "Is a game up?", annotations: RO,
     description: "Whether a game with the dev MCP endpoint is running (its pid, version, endpoint), whether this bridge booted it, how long since a game tool was used, and any other HumanityOS processes on the machine.",
     inputSchema: obj() }, gameStatus],
  [{ name: "game_close", title: "Close the game", annotations: { ...ACT, destructiveHint: true },
     description: "Close the game the dev MCP endpoint belongs to: a normal close first (it saves), forced after 15 s. Only that game; never another HumanityOS window.",
     inputSchema: obj() }, gameClose],
  [{ name: "game_showcase_reset", title: "Release the sticky showcase pins", annotations: ACT,
     description: "Release every sticky showcase pin and diagnostic channel (wind, anim_clock, fov, hud, hull, aurora, room_gi, sun shadows, ...) to its normal value: the same list the rigs release before every vantage (scripts/lib/showcase-pins.js).",
     inputSchema: obj() }, showcaseReset],
  [{ name: "game_goto_vantage", title: "Go to a rig vantage", annotations: ACT,
     description: "Replay a vantage of tests/visual/vantages.json in the running game the way probe-sweep does (pins released, its showcase, the park, the clock held, the settle, post and final showcase), then take a screenshot unless screenshot is false. Without the sweep's judges: dev_rig_run probe-sweep gives a judged capture.",
     inputSchema: obj({ id: { type: "string", description: "The vantage id." }, screenshot: { type: "boolean", description: "Take a screenshot at the end, default true." } }, ["id"]) }, gotoVantage],
  [{ name: "dev_rig_vantages", title: "List the rig vantages", annotations: RO,
     description: "The vantages of tests/visual/vantages.json (where the rigs park the camera, with what pins), by id and description, optionally only those whose id or description contains filter.",
     inputSchema: obj({ filter: { type: "string" }, limit: { type: "integer", minimum: 1, maximum: 600, description: "Default 100." } }) }, vantages],
  [{ name: "dev_rig_list", title: "List the rigs", annotations: RO,
     description: "The rigs dev_rig_run can run (every verify-* rig and probe-sweep in the Justfile), with what each checks.",
     inputSchema: obj() }, () => P.textResult(listRigs().map((r) => `${r.name} (scripts/${r.script}${r.flags.length ? " " + r.flags.join(" ") : ""}): ${r.about}`).join("\n"), { rigs: listRigs() })],
  [{ name: "dev_rig_run", title: "Run a rig and return its verdict", annotations: ACT,
     description: "Run a named rig (dev_rig_list) to its end and return its verdict: exit 0 passed, 1 refused before judging, 2 failed, with the verdict lines and the last lines. Rigs boot the game themselves, wait their turn, and take minutes; refused while game_boot's game is up (close it first). probe-sweep needs a vantage.",
     inputSchema: obj({ rig: { type: "string" }, vantage: { type: "string", description: "probe-sweep's --only." }, args: { type: "array", items: { type: "string" }, description: "More arguments for the rig." }, timeout_min: { type: "number", minimum: 1, maximum: 240, description: "Default 90." } }, ["rig"]) }, rigRun],
  [{ name: "dev_lib_test", title: "Run library tests by filter", annotations: ACT,
     description: "Run the library's unit tests whose names contain any of the filters (node scripts/lib-tests.js, which links the Windows test program the way that works), and return the result line, the failed tests and the last lines. The first run after a change compiles for minutes.",
     inputSchema: obj({ filters: { type: "array", items: { type: "string" }, minItems: 1, description: "e.g. [\"engine::mcp\"] or [\"registry\", \"parses\"]; \"--nocapture\" shows output." }, timeout_min: { type: "number", minimum: 1, maximum: 120, description: "Default 40." } }, ["filters"]) }, libTest],
  [{ name: "dev_lints", title: "Run the lints", annotations: RO,
     description: "Run `just lints`: every std-only lint (em dashes, theme tokens, glyphs, page parity, settings persistence, file sizes, schema order, bind groups, ...) and the encoding check, in about a minute. Says which one failed and why.",
     inputSchema: obj() }, lints],
  [{ name: "dev_journal", title: "Read the journal", annotations: RO,
     description: "The orchestrator journal (data/coordination/orchestrator_state.json): the current focus and the last few decisions with why they were made.",
     inputSchema: obj({ entries: { type: "integer", minimum: 1, maximum: 40, description: "How many decisions, default 5." } }) }, journal],
  [{ name: "dev_priorities", title: "Read the top of PRIORITIES", annotations: RO,
     description: "The top of TIER 0 in docs/PRIORITIES.md: what gets worked on next.",
     inputSchema: obj({ lines: { type: "integer", minimum: 10, maximum: 400, description: "Default 80." } }) }, priorities],
];

function gameToolDefs() {
  try {
    return JSON.parse(fs.readFileSync(GAME_TOOLS_FILE, "utf8")).tools || [];
  } catch (e) {
    log(`could not read ${GAME_TOOLS_FILE}: ${e.message}`);
    return [];
  }
}

function makeHost() {
  const gameDefs = gameToolDefs();
  const own = new Map(OWN.map(([def, fn]) => [def.name, fn]));
  const forwarded = new Set(gameDefs.map((d) => d.name));
  return {
    serverInfo: SERVER_INFO,
    instructions:
      "HumanityOS development tools. game_boot starts the game in a rig sandbox (background, silent, never taking " +
      "focus); then game_screenshot, game_player_state, game_camera, game_showcase and the other game_* tools drive " +
      "and inspect it through its dev IPC; game_close when done. dev_* tools need no game: rigs, lib tests, lints, " +
      "journal, priorities. docs/ai/mcp.md.",
    tools: () => [...OWN.map(([d]) => d), ...gameDefs],
    call: async (name, args, ctx) => {
      if (own.has(name)) return own.get(name)(args, ctx);
      if (forwarded.has(name)) return callGame(name, args, ctx);
      return null;
    },
  };
}

// ── stdio ────────────────────────────────────────────────────────────────────────────────────

function main() {
  const host = makeHost();
  startIdleClose();
  const inflight = new Map(); // request id -> AbortController
  const send = (m) => process.stdout.write(JSON.stringify(m) + "\n");
  const rl = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
  rl.on("line", async (line) => {
    if (!line.trim()) return;
    let msg;
    try {
      msg = JSON.parse(line);
    } catch (e) {
      send({ jsonrpc: "2.0", id: null, error: { code: P.ERR.PARSE_ERROR, message: `not JSON: ${e.message}` } });
      return;
    }
    if (msg && msg.method === "notifications/cancelled" && msg.params) {
      const c = inflight.get(msg.params.requestId);
      if (c) c.abort();
      return;
    }
    const ac = new AbortController();
    if (msg && msg.id !== undefined) inflight.set(msg.id, ac);
    const token = msg && msg.params && msg.params._meta ? msg.params._meta.progressToken : undefined;
    let n = 0;
    const notify =
      token === undefined
        ? null
        : (message) => send({ jsonrpc: "2.0", method: "notifications/progress", params: { progressToken: token, progress: ++n, message } });
    const reply = await P.handle(msg, host, { signal: ac.signal, notify });
    if (msg && msg.id !== undefined) inflight.delete(msg.id);
    if (reply && !ac.signal.aborted) send(reply);
  });
  rl.on("close", async () => {
    // The client is gone: close the game this bridge booted, so it does not sit on the GPU. A game
    // another bridge booted in the same rig (two sessions) is left to that bridge and the idle close.
    if (bootedPid && pidAlive(bootedPid)) {
      log(`closing the game (pid ${bootedPid}): the client closed the session`);
      await closePid(bootedPid);
    }
    process.exit(0);
  });
  log(`ready: ${OWN.length} bridge tools and ${gameToolDefs().length} game tools (tree v${VERSION})`);
}

if (require.main === module) main();

module.exports = { bootCommand, listRigs, makeHost, findEndpoint, endpointCandidates, RIG, DEFAULT_VANTAGE };
