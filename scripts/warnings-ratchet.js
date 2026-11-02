#!/usr/bin/env node
// The Rust warnings ratchet (2026-11-01, PRIORITIES TIER 0 item 14: "Rust warnings to zero, then
// held there"). `just verify` and CI run our crate's checks through this, and a change that adds a
// warning fails them.
//
// Usage: node scripts/warnings-ratchet.js native   cargo check --workspace --features native --lib --bins --tests
//        node scripts/warnings-ratchet.js relay    cargo check --workspace --features relay --no-default-features --lib --bins --tests
//        node scripts/warnings-ratchet.js --count <cargo-json-file> [native|relay]
//                                                   count a saved `--message-format=json` stream (no cargo)
//
// WHY: until 2026-11-01 this PC's Windows user environment set RUSTFLAGS=-Awarnings, which hid
// every rustc warning in every checkout, so about 1,070 native and 262 relay warning lines piled
// up while every agent reported "0 warnings" (docs/ai/builds.md, "The warnings ratchet"). Two
// rules follow from that, and this script is both of them:
//
//   1. COUNT, from the normal build. It runs the check `just verify` always ran, with
//      `--message-format=json` added (the message format is not part of cargo's fingerprint, so
//      nothing rebuilds; a different RUSTFLAGS value WOULD rebuild everything, which is why this
//      never adds `-D warnings` itself). Cargo replays a cached crate's warnings, so a warm run
//      counts the same warnings a cold one does. It counts the warnings in OUR crates only (every
//      member of the workspace: the root package and each crate the root Cargo.toml's [workspace]
//      lists, since the crate split's rung 1, 2026-11-01; `--workspace` checks every member's own
//      targets, its tests included), each one once: the lib and its test build report the same
//      warning twice. The count is compared with scripts/warnings-budget.json
//      and the run fails when it is higher.
//
//   2. NEVER PASS BECAUSE THE WARNINGS ARE HIDDEN. Before cargo runs, it reads every place cargo
//      takes rustc flags from (RUSTFLAGS, CARGO_ENCODED_RUSTFLAGS, CARGO_BUILD_RUSTFLAGS, any
//      CARGO_TARGET_<triple>_RUSTFLAGS, and `rustflags` in every .cargo/config(.toml) from here up
//      and in CARGO_HOME), and every member's crate roots and Cargo.toml for a crate-wide allow. Any lint
//      allow (`-A...`, `--allow`) or `--cap-lints` there FAILS the run loudly, saying the count
//      cannot be read and how to fix it, instead of reporting a 0 that proves nothing.
//
// Exit codes: 0 within budget; 1 over budget, or warnings hidden; cargo's own code when cargo
// fails (a compile error). The rendered warnings and errors are printed to stderr as cargo would.

"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawn } = require("child_process");

const ROOT = path.resolve(__dirname, "..");
const BUDGET_FILE = path.join(__dirname, "warnings-budget.json");

/** The two checks, exactly as `just verify` runs them (plus the JSON message format). Every member
 * of the workspace (rung 1 of the crate split): a member's own test targets are not checked by a
 * command that names only the root package. */
const CHECKS = {
  native: ["check", "--workspace", "--features", "native", "--lib", "--bins", "--tests"],
  relay: ["check", "--workspace", "--features", "relay", "--no-default-features", "--lib", "--bins", "--tests"],
};

/** The workspace's member folders, from the root Cargo.toml's text (pure, tested): ["."] when it has
 * no [workspace], else its `members` (the root package is "."). */
function workspaceMembers(toml) {
  const lines = String(toml || "").replace(/\r\n/g, "\n").split("\n");
  let table = "";
  for (let i = 0; i < lines.length; i++) {
    const h = /^\s*\[([^\]]+)\]\s*(?:#.*)?$/.exec(lines[i]);
    if (h) {
      table = h[1].trim();
      continue;
    }
    const m = table === "workspace" && /^\s*members\s*=\s*\[(.*)$/.exec(lines[i]);
    if (m) {
      let body = m[1];
      while (!body.includes("]") && i + 1 < lines.length) body += lines[++i];
      return [...body.slice(0, body.indexOf("]")).matchAll(/"([^"]+)"/g)].map((x) => x[1]);
    }
  }
  return ["."];
}

/** Every member's manifest path under `root`. */
function memberManifests(root = ROOT) {
  return workspaceMembers(readOrNull(path.join(root, "Cargo.toml"))).map((m) => path.join(root, m, "Cargo.toml"));
}

// ---------------------------------------------------------------------------------------------
// Hidden warnings.

/** Split a RUSTFLAGS-style value (space separated) into flags. */
function splitSpaced(v) {
  return String(v || "").trim().split(/\s+/).filter(Boolean);
}

/** The flags in `flags` that hide warnings: a lint allow of any kind, or a lint cap. */
function hidingFlags(flags) {
  const bad = [];
  for (let i = 0; i < flags.length; i++) {
    const f = flags[i];
    if (f === "-A" || f === "--allow" || f === "--cap-lints") {
      bad.push(`${f} ${flags[i + 1] || ""}`.trim());
      i++;
    } else if (/^-A./.test(f) || /^--allow=/.test(f) || /^--cap-lints=/.test(f)) {
      bad.push(f);
    }
  }
  return bad;
}

/** Every `rustflags = ...` value in a cargo config file's text, as flag lists. */
function configRustflags(text) {
  const out = [];
  // Comments off (a `#` that starts a line or follows whitespace; config strings rarely hold one).
  const body = String(text)
    .split(/\r?\n/)
    .map((l) => l.replace(/(^|\s)#.*$/, ""))
    .join("\n");
  const re = /(?:^|[\s.{,])rustflags\s*=\s*(\[[\s\S]*?\]|"[^"\n]*"|'[^'\n]*')/g;
  let m;
  while ((m = re.exec(body))) {
    const v = m[1];
    if (v.startsWith("[")) {
      const items = [];
      const s = /"([^"]*)"|'([^']*)'/g;
      let q;
      while ((q = s.exec(v))) items.push(q[1] !== undefined ? q[1] : q[2]);
      out.push(items);
    } else {
      out.push(splitSpaced(v.slice(1, -1)));
    }
  }
  return out;
}

/** The cargo config files cargo would read from `cwd` (its folder and every parent, then CARGO_HOME). */
function configFiles(cwd, env) {
  const files = [];
  let dir = path.resolve(cwd);
  for (;;) {
    for (const n of ["config.toml", "config"]) files.push(path.join(dir, ".cargo", n));
    const up = path.dirname(dir);
    if (up === dir) break;
    dir = up;
  }
  const home = env.CARGO_HOME || path.join(os.homedir(), ".cargo");
  for (const n of ["config.toml", "config"]) files.push(path.join(home, n));
  return [...new Set(files)];
}

/**
 * Why the warnings would be hidden, as a list of "<where>: <flag>" strings (empty when nothing
 * hides them). `readFile(path)` returns the text or null; injectable for the tests.
 */
function hiddenBy(env, { cwd = ROOT, root = ROOT, readFile = readOrNull } = {}) {
  const found = [];
  const note = (where, flags) => {
    for (const f of hidingFlags(flags)) found.push(`${where}: ${f}`);
  };
  if (env.CARGO_ENCODED_RUSTFLAGS) note("CARGO_ENCODED_RUSTFLAGS", env.CARGO_ENCODED_RUSTFLAGS.split("\x1f"));
  for (const k of Object.keys(env)) {
    if (k === "RUSTFLAGS" || k === "CARGO_BUILD_RUSTFLAGS" || /^CARGO_TARGET_.+_RUSTFLAGS$/.test(k)) note(k, splitSpaced(env[k]));
  }
  for (const f of configFiles(cwd, env)) {
    const text = readFile(f);
    if (text == null) continue;
    for (const flags of configRustflags(text)) note(f, flags);
  }
  // A crate-wide allow in the code or the manifest hides them just as well: every member's.
  for (const member of workspaceMembers(readFile(path.join(root, "Cargo.toml")))) {
    const base = member === "." ? "" : `${member.replace(/\\/g, "/").replace(/\/$/, "")}/`;
    for (const rel of [`${base}src/lib.rs`, `${base}src/main.rs`]) {
      const text = readFile(path.join(root, rel));
      if (text != null && /#!\[\s*allow\s*\([^)]*\bwarnings\b/.test(text)) found.push(`${rel}: #![allow(warnings)]`);
    }
    const manifest = readFile(path.join(root, base, "Cargo.toml"));
    if (manifest != null) {
      const lints = manifest.match(/^\[(?:workspace\.)?lints\.rust\][\s\S]*?(?=^\[|(?![\s\S]))/m);
      if (lints && /^\s*warnings\s*=\s*(?:"allow"|\{[^}]*level\s*=\s*"allow")/m.test(lints[0])) {
        found.push(`${base}Cargo.toml: [lints.rust] warnings = "allow"`);
      }
    }
  }
  return found;
}

function readOrNull(f) {
  try {
    return fs.readFileSync(f, "utf8");
  } catch {
    return null;
  }
}

// ---------------------------------------------------------------------------------------------
// Counting.

/** Is this cargo message about one of OUR crates (a package whose manifest is one of `manifests`,
 * or, with no manifest path, one named in `pkgNames`)? */
function isOurs(msg, manifests, pkgNames) {
  if (msg.manifest_path) {
    const m = path.resolve(msg.manifest_path).toLowerCase();
    return manifests.some((x) => path.resolve(x).toLowerCase() === m);
  }
  const id = String(msg.package_id || "");
  return pkgNames.some((n) => id.startsWith(`${n} `) || id.includes(`#${n}@`) || new RegExp(`[/\\\\]${n}#\\d`).test(id));
}

/** rustc's own "N warnings emitted" summary, which is not a warning. */
function isSummary(m) {
  return (!m.spans || m.spans.length === 0) && /^\d+ warnings? emitted/.test(m.message || "");
}

/** One warning's identity: its lint, its words and where it points. The lib and its tests repeat it. */
function warningKey(m) {
  const p = (m.spans || []).find((s) => s.is_primary) || (m.spans || [])[0] || {};
  return `${(m.code && m.code.code) || ""}|${m.message}|${p.file_name || ""}:${p.line_start || 0}:${p.column_start || 0}`;
}

/**
 * Count the warnings in a cargo `--message-format=json` stream (an array of lines).
 * Returns { ours: Map(key -> message), others: Map, rendered: [text] } where `rendered` is every
 * distinct diagnostic's text (warnings and errors, ours and other local packages'), in order.
 */
function countLines(lines, { manifests = memberManifests(), pkgNames = packageNames() } = {}) {
  const ours = new Map();
  const others = new Map();
  const seenText = new Set();
  const rendered = [];
  for (const line of lines) {
    if (!line || line[0] !== "{") continue;
    let msg;
    try {
      msg = JSON.parse(line);
    } catch {
      continue;
    }
    if (msg.reason !== "compiler-message" || !msg.message) continue;
    const m = msg.message;
    if (m.level === "warning" && isSummary(m)) continue;
    const text = m.rendered || `${m.level}: ${m.message}\n`;
    if (!seenText.has(text)) {
      seenText.add(text);
      rendered.push(text);
    }
    if (m.level !== "warning") continue;
    const into = isOurs(msg, manifests, pkgNames) ? ours : others;
    const key = warningKey(m);
    if (!into.has(key)) into.set(key, m);
  }
  return { ours, others, rendered };
}

/** Our crates' names, from each member's Cargo.toml [package] table. */
function packageNames(root = ROOT) {
  const out = [];
  for (const manifest of memberManifests(root)) {
    const text = readOrNull(manifest) || "";
    const m = text.match(/^\[package\][\s\S]*?^name\s*=\s*"([^"]+)"/m);
    if (m) out.push(m[1]);
  }
  return out.length ? out : ["humanity-engine"];
}

/** A short "kind: count" table of the warnings, most first (by lint code, else by message). */
function byKind(ours) {
  const kinds = new Map();
  for (const m of ours.values()) {
    const k = (m.code && m.code.code) || m.message.replace(/`[^`]*`/g, "`_`");
    kinds.set(k, (kinds.get(k) || 0) + 1);
  }
  return [...kinds.entries()].sort((a, b) => b[1] - a[1]);
}

function readBudget() {
  const text = readOrNull(BUDGET_FILE);
  if (text == null) throw new Error(`no budget file at ${BUDGET_FILE}`);
  return JSON.parse(text);
}

/** Judge a count against the budget: { ok, lines } (the lines to print). */
function judge(which, ours, others, budget) {
  const lines = [];
  const limit = budget[which];
  if (typeof limit !== "number") {
    return { ok: false, lines: [`FAILED: scripts/warnings-budget.json has no number for "${which}"`] };
  }
  const n = ours.size;
  if (others.size) lines.push(`   (${others.size} warning(s) in other local packages, e.g. vendor/: printed above, not counted)`);
  if (n > limit) {
    lines.push(`FAILED: ${n} Rust warning(s) in the ${which} build, over its budget of ${limit} (scripts/warnings-budget.json).`);
    for (const [k, c] of byKind(ours).slice(0, 12)) lines.push(`     ${String(c).padStart(5)}  ${k}`);
    lines.push("   Fix the warnings printed above (each is in one of our crates). The budget is a ratchet: it only goes down.");
    return { ok: false, lines };
  }
  lines.push(`>> warnings ok (${which}): ${n}, budget ${limit}`);
  if (n < limit) lines.push(`   The count is below the budget: lower "${which}" in scripts/warnings-budget.json to ${n}, so it holds there.`);
  return { ok: true, lines };
}

function hiddenMessage(found) {
  return [
    "FAILED: the Rust warnings cannot be counted, because something hides them:",
    ...found.map((f) => `     ${f}`),
    "   A count read through that would be 0 and prove nothing (until 2026-11-01 this PC's user",
    "   environment had RUSTFLAGS=-Awarnings; docs/ai/builds.md, \"The warnings ratchet\").",
    "   Fix: remove the flag. For one command in bash: `RUSTFLAGS= just verify`. For good on Windows:",
    "   Settings > System > About > Advanced system settings > Environment Variables, delete it, then",
    "   restart the programs that run cargo. A config file's rustflags: take the allow out of it.",
  ];
}

// ---------------------------------------------------------------------------------------------

function runCargo(which) {
  return new Promise((resolve) => {
    const args = [...CHECKS[which], "--message-format=json"];
    console.error(`-- cargo ${CHECKS[which].join(" ")} (warnings counted by scripts/warnings-ratchet.js) --`);
    const c = spawn("cargo", args, { cwd: ROOT, stdio: ["ignore", "pipe", "inherit"] });
    const lines = [];
    let buf = "";
    c.stdout.setEncoding("utf8");
    c.stdout.on("data", (d) => {
      buf += d;
      let i;
      while ((i = buf.indexOf("\n")) >= 0) {
        const line = buf.slice(0, i);
        buf = buf.slice(i + 1);
        lines.push(line);
        // Errors as they come, so a compile error is not held back until the end.
        if (line.includes('"level":"error"')) {
          try {
            const m = JSON.parse(line).message;
            if (m && m.rendered) process.stderr.write(m.rendered);
          } catch {}
        }
      }
    });
    c.on("error", (e) => resolve({ code: 127, lines, error: e }));
    c.on("close", (code) => {
      if (buf) lines.push(buf);
      resolve({ code: code == null ? 1 : code, lines });
    });
  });
}

async function main(argv) {
  const countAt = argv.indexOf("--count");
  if (countAt >= 0) {
    const file = argv[countAt + 1];
    const which = argv.find((a, i) => i !== countAt + 1 && CHECKS[a]) || "native";
    const lines = fs.readFileSync(file, "utf8").split(/\r?\n/);
    const { ours, others } = countLines(lines);
    for (const m of ours.values()) process.stderr.write(m.rendered || `warning: ${m.message}\n`);
    const r = judge(which, ours, others, readBudget());
    for (const l of r.lines) console.log(l);
    return r.ok ? 0 : 1;
  }
  const which = argv[0];
  if (!CHECKS[which]) {
    console.error("usage: node scripts/warnings-ratchet.js native|relay   (or --count <cargo-json-file> [native|relay])");
    return 2;
  }
  const found = hiddenBy(process.env);
  if (found.length) {
    for (const l of hiddenMessage(found)) console.error(l);
    return 1;
  }
  const res = await runCargo(which);
  if (res.error) {
    console.error(`FAILED: could not run cargo: ${res.error.message}`);
    return 127;
  }
  const { ours, others, rendered } = countLines(res.lines);
  for (const t of rendered) if (!/^error/.test(t)) process.stderr.write(t);
  if (res.code !== 0) {
    console.error(`FAILED: cargo ${CHECKS[which].join(" ")} exited with ${res.code}`);
    return res.code;
  }
  const r = judge(which, ours, others, readBudget());
  for (const l of r.lines) console.log(l);
  return r.ok ? 0 : 1;
}

module.exports = { hidingFlags, configRustflags, configFiles, hiddenBy, countLines, judge, byKind, warningKey, workspaceMembers, CHECKS };

if (require.main === module) {
  main(process.argv.slice(2)).then(
    (code) => process.exit(code),
    (e) => {
      console.error(`FAILED: warnings-ratchet: ${e && e.stack ? e.stack : e}`);
      process.exit(1);
    }
  );
}
