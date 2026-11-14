// The machine-wide boot ledger (2026-11-14): one line per game boot a developer's script started,
// so the orchestrator can see repeated or wasted boots across every worktree.
//
// WHY. The operator, 2026-11-14: "It appears the game often loads up then cycles through a bunch of
// static scenes then turns off. What's happening? Is a test being ran or something? Do all the test
// that are being ran need to be run?" Nobody could answer how many boots there had been, from which
// worktree, or for what. Now every boot says.
//
// THE FILE: %LOCALAPPDATA%\HumanityOS\dev\boots.jsonl, outside every repository (several worktrees
// boot on one machine, and the count is the machine's). A LOG: appended, never read back by the game,
// so it is not a cache. HUMANITY_BOOT_LEDGER_FILE points a test at its own file.
//
// WHO WRITES IT (the launcher, never each rig):
//   - spawnGame, in lib/game-launch.js, for every rig's boot: one line when the game exits or the rig's
//     process ends, whichever comes first. It sets HUMANITY_BOOT_LEDGER=spawnGame for the game.
//   - a boot spawnGame leaves running (probe-sweep --keep-open, which the dev MCP's game_boot uses)
//     is written as PENDING in the rig folder's debug/boot_ledger_pending.json; whoever closes it
//     (the dev MCP's game_close, its idle close, its session end) finishes the line, and a pending
//     record nobody finished is written by the next spawnGame in that folder ("end not seen").
//   - the game itself (src/engine/boot_ledger.rs), for a quiet boot no launcher recorded
//     (`just launch-bg`): its line at exit.
//
// One line's fields (src/engine/boot_ledger.rs writes the same names; the test pins them):
//   t, end            start and end, UTC ISO 8601
//   duration_s        seconds the game ran (as far as the writer saw)
//   checkout          the checkout the launching script belongs to
//   worktree          its short name (the folder under .claude\worktrees, else the checkout's name)
//   script            the script that started the boot (probe-sweep.js, verify-screens.js, ...)
//   rig               the rig's name as the launcher knows it
//   via               the gate or tool that ran that script (verify-runtime, the dev MCP), if known
//   purpose           what the boot was for (the vantages, a game_boot), if known
//   pid, exe          the game process
//   hidden            whether the boot was meant to be hidden from the screen (src/engine/rig_window.rs)
//   low_impact        the low-impact value it ran with, or null
//   ended             how it ended ("exited with code 0", "stopped by the rig", ...)
//   writer            spawnGame | mcp-server | engine

const fs = require("fs");
const os = require("os");
const path = require("path");

const FIELDS = ["t", "end", "duration_s", "checkout", "worktree", "script", "rig", "via", "purpose", "pid", "exe", "hidden", "low_impact", "ended", "writer"];
/** The variable a launcher sets to say it writes this boot's line (the game then writes none). */
const OWNER_ENV = "HUMANITY_BOOT_LEDGER";
const FILE_ENV = "HUMANITY_BOOT_LEDGER_FILE";
const PURPOSE_ENV = "HUMANITY_BOOT_PURPOSE";
const VIA_ENV = "HUMANITY_BOOT_VIA";
/** The rig folder's record of a boot left running (in its debug/). */
const PENDING_FILE = "boot_ledger_pending.json";
/** The escape hatch that keeps a rig's window visible (src/engine/rig_window.rs RIG_VISIBLE_ENV). */
const RIG_VISIBLE_ENV = "HUMANITY_RIG_VISIBLE";

/** %LOCALAPPDATA%\HumanityOS\dev\boots.jsonl, or HUMANITY_BOOT_LEDGER_FILE. */
function ledgerPath(env = process.env) {
  if (env[FILE_ENV]) return env[FILE_ENV];
  if (process.env[FILE_ENV]) return process.env[FILE_ENV];
  const local = env.LOCALAPPDATA || path.join(os.homedir(), "AppData", "Local");
  return path.join(local, "HumanityOS", "dev", "boots.jsonl");
}

/** The short name the report groups by (src/engine/boot_ledger.rs worktree_name, the same rule). */
function worktreeName(checkout) {
  if (!checkout) return null;
  const m = /[\\/]\.claude[\\/]worktrees[\\/]([^\\/]+)/i.exec(checkout);
  return m ? m[1] : path.basename(path.resolve(checkout));
}

/** Whether a boot with this environment is meant to be hidden (the engine decides; this mirrors it for the record). */
function meantHidden(env) {
  const v = env[RIG_VISIBLE_ENV];
  return !(v && v !== "0");
}

/** A record's start half, with every field present (null when unknown). */
function startRecord(o) {
  const r = {};
  for (const f of FIELDS) r[f] = null;
  Object.assign(r, {
    // Milliseconds kept, so a duration is exact (the game's half writes whole seconds; both parse).
    t: new Date(o.startMs != null ? o.startMs : Date.now()).toISOString(),
    checkout: o.checkout || null,
    worktree: worktreeName(o.checkout),
    script: o.script || null,
    rig: o.rig || null,
    via: o.via || null,
    purpose: o.purpose || null,
    pid: o.pid != null ? o.pid : null,
    exe: o.exe || null,
    hidden: o.hidden != null ? !!o.hidden : null,
    low_impact: o.lowImpact || null,
    writer: o.writer || null,
  });
  return r;
}

/** The record finished: its end, duration and how it ended. */
function finishRecord(rec, ended, endMs = Date.now()) {
  const startMs = Date.parse(rec.t);
  return {
    ...rec,
    end: new Date(endMs).toISOString(),
    duration_s: Number.isFinite(startMs) ? Math.round((endMs - startMs) / 100) / 10 : null,
    ended,
  };
}

/** Append one record. Never throws: a ledger that cannot be written must not fail a rig. Inside
 *  `node --test` (NODE_TEST_CONTEXT) nothing is written unless a test named its own file: the
 *  rig tests start stand-in games, and those are not boots. */
function append(rec, env = process.env) {
  const named = env[FILE_ENV] || process.env[FILE_ENV];
  if (!named && (env.NODE_TEST_CONTEXT || process.env.NODE_TEST_CONTEXT)) return false;
  const file = named || ledgerPath(env);
  try {
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.appendFileSync(file, JSON.stringify(rec) + "\n");
    return true;
  } catch (e) {
    try {
      process.stderr.write(`boot ledger: could not write ${file}: ${e.message}\n`);
    } catch {}
    return false;
  }
}

/** Every record since `sinceMs` (a line that does not parse is skipped). */
function readBoots(file, sinceMs = 0) {
  let text = "";
  try {
    text = fs.readFileSync(file, "utf8");
  } catch {
    return [];
  }
  const out = [];
  for (const line of text.split(/\r?\n/)) {
    if (!line.trim()) continue;
    try {
      const r = JSON.parse(line);
      const t = Date.parse(r.t);
      if (!Number.isFinite(t) || t >= sinceMs) out.push(r);
    } catch {}
  }
  return out;
}

// ── A boot left running (probe-sweep --keep-open) ───────────────────────────────────────────────

function pendingPath(dir) {
  return path.join(dir, PENDING_FILE);
}

/** Note a boot left running, for whoever closes it. */
function writePending(dir, rec) {
  try {
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(pendingPath(dir), JSON.stringify(rec));
    return true;
  } catch {
    return false;
  }
}

/** Finish the pending boot in `dir` (only the one with `pid`, when given). Returns the record written, or null. */
function finishPending(dir, ended, { pid = null, env = process.env, endMs = Date.now() } = {}) {
  let rec;
  try {
    rec = JSON.parse(fs.readFileSync(pendingPath(dir), "utf8"));
  } catch {
    return null;
  }
  if (pid != null && rec.pid != null && Number(rec.pid) !== Number(pid)) return null;
  try {
    fs.rmSync(pendingPath(dir), { force: true });
  } catch {}
  const done = finishRecord(rec, ended, endMs);
  append(done, env);
  return done;
}

/** A pending boot nobody finished (found at the next boot in the same folder): written with its end unknown. */
function flushStalePending(dir, env = process.env) {
  let rec;
  try {
    rec = JSON.parse(fs.readFileSync(pendingPath(dir), "utf8"));
  } catch {
    return null;
  }
  try {
    fs.rmSync(pendingPath(dir), { force: true });
  } catch {}
  const done = { ...rec, end: null, duration_s: null, ended: "kept open; its end was not seen (found at the next boot in this folder)" };
  append(done, env);
  return done;
}

// ── The report ───────────────────────────────────────────────────────────────────────────────────

/** Boots grouped per worktree and per rig, and the repeats: the same worktree, rig and purpose more than once. */
function summarize(records) {
  const by = (key) => {
    const m = new Map();
    for (const r of records) {
      const k = key(r);
      const g = m.get(k) || { key: k, boots: 0, seconds: 0, hidden: 0, visible: 0, unknown_end: 0, endings: {} };
      g.boots += 1;
      if (typeof r.duration_s === "number") g.seconds += r.duration_s;
      else g.unknown_end += 1;
      if (r.hidden === true) g.hidden += 1;
      else if (r.hidden === false) g.visible += 1;
      const e = shortEnding(r.ended);
      g.endings[e] = (g.endings[e] || 0) + 1;
      m.set(k, g);
    }
    return [...m.values()].sort((a, b) => b.boots - a.boots || String(a.key).localeCompare(String(b.key)));
  };
  const worktrees = by((r) => r.worktree || "(unknown)");
  const rigs = by((r) => r.rig || r.script || `(${r.writer || "unknown"})`);
  const repeats = by((r) => `${r.worktree || "?"} | ${r.rig || r.script || "?"} | ${r.purpose || "(no purpose given)"}`).filter((g) => g.boots > 1);
  const seconds = records.reduce((s, r) => s + (typeof r.duration_s === "number" ? r.duration_s : 0), 0);
  return { boots: records.length, seconds, worktrees, rigs, repeats };
}

/** "exited with code 0" stays; anything long is cut to its first clause, for the report's columns. */
function shortEnding(e) {
  if (!e) return "(still running or unknown)";
  return String(e).split(/[;(]/)[0].trim();
}

module.exports = {
  FIELDS,
  OWNER_ENV,
  FILE_ENV,
  PURPOSE_ENV,
  VIA_ENV,
  PENDING_FILE,
  RIG_VISIBLE_ENV,
  ledgerPath,
  worktreeName,
  meantHidden,
  startRecord,
  finishRecord,
  append,
  readBoots,
  writePending,
  finishPending,
  flushStalePending,
  summarize,
  shortEnding,
};
