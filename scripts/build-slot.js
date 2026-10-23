#!/usr/bin/env node
// The build slot's installer and health check (2026-10-23). The slot itself is
// scripts/build-slot/hos-build-slot.rs: read its header for what it does and why.
//
//   node scripts/build-slot.js install     just build-slot
//       Builds the wrapper from the MAIN checkout's source with plain rustc (std only, like the
//       lints) into <main checkout>\.cargo\hos-build-slot.exe, and writes
//       <main checkout>\.cargo\config.toml, which turns it on for every cargo command in the
//       main checkout and in every worktree under it (cargo searches up the folders for
//       .cargo\config.toml). Both files are machine-local and gitignored: a checked-in relative
//       path would point every worktree at a wrapper of its own, and Linux CI and the servers
//       have none. Run from a worktree it still installs the main checkout's version, so two
//       checkouts can never take turns replacing each other's wrapper.
//   node scripts/build-slot.js check       just build-slot-check
//       Exit 0 when the wrapper is installed and built from the current source; otherwise say
//       exactly what is wrong and the command that fixes it, exit 1. "Not installed" is exit 0
//       with a note (the slot is opt-in per machine).
//   node scripts/build-slot.js ensure      (lib-tests.js and `just verify` run this first)
//       When the slot is installed but its exe is missing or older than its source, rebuild it;
//       otherwise do nothing. Quiet when there is nothing to do.
//   node scripts/build-slot.js status      just build-slot-status
//       Who is compiling, who waits, the memory, the learned peaks (the exe's --status).
//   node scripts/build-slot.js build --out <exe>
//       Build THIS checkout's source to <exe> (a test, or trying a changed wrapper with
//       CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER=<exe> before it is merged).
//   node scripts/build-slot.js uninstall
//       Remove the config this script wrote (the exe stays). Every checkout's next build of our
//       crate is then a full rebuild: cargo hashes the wrapper into our crate's artifacts.
//
// Why a missing wrapper is a real failure: with the config in place cargo runs the wrapper for
// every compile of our crate, and a missing exe stops every build on the machine with "could
// not execute process". So the exe lives in .cargo\, which no cleaner touches (the disk guard
// and `cargo clean` only delete target\), `ensure` rebuilds it from the scripts that build, and
// the config's comment names the fix.

"use strict";

const fs = require("fs");
const path = require("path");
const crypto = require("crypto");
const { spawnSync } = require("child_process");
const { mainCheckoutRoot } = require("./lib/dxc-dlls.js");

const HERE_ROOT = path.resolve(__dirname, "..");
const SRC_REL = path.join("scripts", "build-slot", "hos-build-slot.rs");
const SLOT_EXE = "hos-build-slot.exe";
const MARKER = "# managed by scripts/build-slot.js";

/** The checkout whose .cargo\ holds the machine's slot: the main one. */
function mainRoot(from = HERE_ROOT) {
  return mainCheckoutRoot(from) || from;
}

function paths(root) {
  const dir = path.join(root, ".cargo");
  return {
    root,
    dir,
    src: path.join(root, SRC_REL),
    exe: path.join(dir, SLOT_EXE),
    info: path.join(dir, "hos-build-slot.json"),
    config: path.join(dir, "config.toml"),
  };
}

/** SHA-256 of the source with CRLF read as LF, so a CRLF checkout and an LF one agree. */
function sourceHash(file) {
  const text = fs.readFileSync(file, "utf8").replace(/\r\n/g, "\n");
  return crypto.createHash("sha256").update(text).digest("hex");
}

function configText() {
  return [
    `${MARKER} (\`just build-slot\`); it rewrites this file, so change that script, not this.`,
    "# Machine-local, gitignored.",
    "#",
    "# Every cargo command in this checkout and in every worktree under it (they find this file by",
    "# cargo's search up the folders) runs the workspace's own compiles through the build slot: one",
    "# memory budget for the machine, so several sessions' compiles of our 600k-line crate take",
    "# turns instead of paging the 32 GB PC into the ground (scripts/build-slot/hos-build-slot.rs).",
    "# The dependency crates are not gated.",
    "#",
    "#   HUMANITY_BUILD_SLOT=off cargo ...   compile without waiting, this once",
    "#   just build-slot-status              who is compiling, who waits, the memory",
    "#   just build-slot                     rebuild the wrapper (after its source changed)",
    "#",
    "# If cargo says it \"could not execute process\" ...hos-build-slot.exe: the wrapper is missing.",
    "# Run `just build-slot` in the main checkout. Do not delete this file to get around it: the",
    "# wrapper is part of the hash cargo gives our crate, so every checkout would rebuild it whole.",
    "[build]",
    'rustc-workspace-wrapper = ".cargo/hos-build-slot.exe"',
    "# Four compiles at a time per cargo, not one per logical processor (twelve here): the dependency",
    "# crates are not gated by the slot, and on 2026-10-23 eighty cargo and rustc processes left 690 MB",
    "# of 32 GB free. CARGO_BUILD_JOBS overrides it for one command.",
    "jobs = 4",
    "",
  ].join("\n");
}

/** Compile `src` to `out` with plain rustc, the source hash compiled in. Throws with rustc's words. */
function compile(src, out, cwd) {
  const sha = sourceHash(src);
  fs.mkdirSync(path.dirname(out), { recursive: true });
  const tmp = `${out}.${process.pid}.tmp.exe`;
  const r = spawnSync("rustc", ["--edition", "2021", "-O", "-A", "warnings", "-C", "debuginfo=0", src, "-o", tmp], {
    cwd,
    env: { ...process.env, HOS_BUILD_SLOT_SRC_SHA256: sha },
    encoding: "utf8",
    windowsHide: true,
  });
  for (const ext of [".pdb"]) fs.rmSync(tmp.replace(/\.exe$/, ext), { force: true });
  if (r.status !== 0) {
    fs.rmSync(tmp, { force: true });
    throw new Error(`rustc could not build ${src}:\n${r.stderr || r.error}`);
  }
  // Replace the exe even while compiles are running it: Windows lets a running exe be
  // RENAMED (not deleted or overwritten), so the old one moves aside and the new one takes
  // its name. Old copies are deleted once nothing runs them.
  if (fs.existsSync(out)) {
    const aside = `${out.replace(/\.exe$/, "")}.old-${Date.now()}.exe`;
    fs.renameSync(out, aside);
  }
  fs.renameSync(tmp, out);
  for (const f of fs.readdirSync(path.dirname(out))) {
    if (/^hos-build-slot\.old-\d+\.exe$/.test(f)) {
      try {
        fs.unlinkSync(path.join(path.dirname(out), f));
      } catch {
        /* still running: next time */
      }
    }
  }
  return sha;
}

function rustcV(cwd) {
  const r = spawnSync("rustc", ["-V"], { cwd, encoding: "utf8", windowsHide: true });
  return (r.stdout || "").trim();
}

/** What is installed and whether it is current. Pure but for reads. */
function inspect(root = mainRoot()) {
  const p = paths(root);
  const has = (f) => fs.existsSync(f);
  const config = has(p.config) ? fs.readFileSync(p.config, "utf8") : null;
  const managed = !!config && config.includes(MARKER);
  const wired = !!config && /rustc-workspace-wrapper\s*=\s*"\.cargo\/hos-build-slot\.exe"/.test(config);
  let info = null;
  try {
    info = JSON.parse(fs.readFileSync(p.info, "utf8"));
  } catch {}
  const srcHash = has(p.src) ? sourceHash(p.src) : null;
  let exeHash = null;
  if (has(p.exe)) {
    const r = spawnSync(p.exe, ["--source"], { encoding: "utf8", windowsHide: true, timeout: 20000 });
    exeHash = r.status === 0 ? (r.stdout || "").trim() : null;
  }
  return { p, config, managed, wired, info, srcHash, exeHash, exe: has(p.exe) };
}

/** The verdict on an inspect() result: { state, lines }. state: ok | not-installed | missing | stale | foreign-config | no-source */
function verdict(s) {
  const fix = `cd "${s.p.root}" && just build-slot`;
  if (!s.config) return { state: "not-installed", lines: [`build slot: not installed on this machine (no ${s.p.config}). Optional; to turn it on: ${fix}`] };
  if (!s.wired) {
    return {
      state: "foreign-config",
      lines: [
        `build slot: ${s.p.config} exists but does not run the slot (it was not written by scripts/build-slot.js).`,
        `Add these two lines to it, or move it aside and run: ${fix}`,
        "  [build]",
        '  rustc-workspace-wrapper = ".cargo/hos-build-slot.exe"',
      ],
    };
  }
  if (!s.srcHash) {
    return { state: "no-source", lines: [`build slot: the config is in place but ${s.p.src} is missing (a checkout from before the slot?). Run: ${fix} once it is there, or uninstall: node scripts/build-slot.js uninstall`] };
  }
  if (!s.exe) {
    return {
      state: "missing",
      lines: [
        `BUILD SLOT MISSING: ${s.p.config} runs every compile of our crate through ${s.p.exe}, and that file is gone,`,
        "so every cargo build on this machine fails with \"could not execute process\". Fix:",
        `  ${fix}`,
      ],
    };
  }
  if (s.exeHash !== s.srcHash) {
    return {
      state: "stale",
      lines: [
        `BUILD SLOT STALE: ${s.p.exe} was built from source ${s.exeHash ? s.exeHash.slice(0, 16) : "(unreadable)"}, the source is now ${s.srcHash.slice(0, 16)}.`,
        "It still works (the old rules); rebuild it so the new ones apply:",
        `  ${fix}`,
      ],
    };
  }
  return { state: "ok", lines: [`build slot: installed and current (${s.p.exe}, source ${s.srcHash.slice(0, 16)}${s.info && s.info.rustc ? `, built with ${s.info.rustc}` : ""})`] };
}

function install(root = mainRoot(), { quiet = false } = {}) {
  const p = paths(root);
  if (!fs.existsSync(p.src)) throw new Error(`${p.src} does not exist: the main checkout has no build slot source yet (merge it first).`);
  if (fs.existsSync(p.config) && !fs.readFileSync(p.config, "utf8").includes(MARKER)) {
    const v = verdict(inspect(root));
    if (v.state === "foreign-config") {
      for (const l of v.lines) console.error(l);
      throw new Error("refusing to overwrite a .cargo/config.toml this script did not write");
    }
  }
  const sha = compile(p.src, p.exe, root);
  fs.writeFileSync(p.info, JSON.stringify({ src_sha256: sha, rustc: rustcV(root), built_at: new Date().toISOString(), source: p.src }, null, 2) + "\n");
  if (!fs.existsSync(p.config) || fs.readFileSync(p.config, "utf8") !== configText()) fs.writeFileSync(p.config, configText());
  if (!quiet) {
    console.log(`build slot: built ${p.exe} (source ${sha.slice(0, 16)}) and wrote ${p.config}.`);
    console.log("Every cargo build under this checkout now takes its turn for memory. Status: just build-slot-status");
  }
  return sha;
}

function main() {
  const [cmd, ...rest] = process.argv.slice(2);
  switch (cmd) {
    case "install":
      install();
      return 0;
    case "check": {
      const v = verdict(inspect());
      for (const l of v.lines) (v.state === "ok" || v.state === "not-installed" ? console.log : console.error)(l);
      return v.state === "ok" || v.state === "not-installed" ? 0 : 1;
    }
    case "ensure": {
      const s = inspect();
      const v = verdict(s);
      if (v.state === "missing" || v.state === "stale") {
        console.log(`build slot: ${v.state === "missing" ? "the wrapper was missing" : "the wrapper was older than its source"}; rebuilding ${s.p.exe}`);
        install(s.p.root, { quiet: true });
      } else if (v.state === "foreign-config" || v.state === "no-source") {
        for (const l of v.lines) console.error(l);
      }
      return 0;
    }
    case "status": {
      const s = inspect();
      const v = verdict(s);
      for (const l of v.lines) console.log(l);
      if (!s.exe) return v.state === "not-installed" ? 0 : 1;
      return spawnSync(s.p.exe, ["--status"], { stdio: "inherit" }).status || 0;
    }
    case "build": {
      const at = rest.indexOf("--out");
      if (at < 0 || !rest[at + 1]) {
        console.error("build-slot: build needs --out <exe>");
        return 2;
      }
      const out = path.resolve(rest[at + 1]);
      const sha = compile(path.join(HERE_ROOT, SRC_REL), out, HERE_ROOT);
      console.log(`build slot: built ${out} from ${path.join(HERE_ROOT, SRC_REL)} (source ${sha.slice(0, 16)})`);
      return 0;
    }
    case "uninstall": {
      const p = paths(mainRoot());
      if (fs.existsSync(p.config) && fs.readFileSync(p.config, "utf8").includes(MARKER)) {
        fs.rmSync(p.config);
        console.log(`build slot: removed ${p.config} (the exe stays). Every checkout's next build of our crate is a full rebuild.`);
      } else console.log("build slot: no config of ours to remove.");
      return 0;
    }
    default:
      console.error("usage: node scripts/build-slot.js install | check | ensure | status | build --out <exe> | uninstall");
      return 2;
  }
}

if (require.main === module) {
  try {
    process.exit(main());
  } catch (e) {
    console.error(`build-slot: ${e.message}`);
    process.exit(1);
  }
}

module.exports = { mainRoot, paths, sourceHash, configText, compile, inspect, verdict, install, MARKER };
