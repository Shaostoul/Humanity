// Compile a standalone std-only Rust program with plain rustc, once per content (2026-10-22).
//
// WHY: `just lints` compiled its 19 `rustc --test` programs on every run (about 42 s measured
// in the build-speed review) although they change a few times a week. The build slot's wrapper
// (scripts/build-slot/) and its test are built the same way. This keeps each program in a
// cache folder under a name made from everything that decides what rustc produces:
//
//   - the source file's bytes (SHA-256)
//   - `rustc -V` (a new toolchain makes a different program; rust-toolchain.toml pins it)
//   - every argument given to rustc
//   - every environment variable the program reads AT COMPILE TIME (the lints read
//     CARGO_MANIFEST_DIR through env!, so the checkout's path is part of the key)
//
// A program whose source pulls in other files at compile time (include_str!, include_bytes!,
// a `mod x;` declared without a body, #[path]) is NOT cached: its key would miss those files.
// It is compiled fresh each time, and the result says why (`uncacheable`).
//
// Writes are atomic: rustc builds into a folder of this process's own, then the program is
// renamed into the cache (the folder is inside the cache, so the rename never crosses a drive). Two runs that race both build it, and one rename wins; neither ever
// runs a half-written program. Old entries of the same name are pruned (the newest few stay).

"use strict";

const fs = require("fs");
const path = require("path");
const crypto = require("crypto");
const { spawnSync, spawn } = require("child_process");

const KEEP_PER_NAME = 3;

let rustcVersionMemo = null;
/** `rustc -V` as the toolchain in force for `cwd` reports it (rustup reads rust-toolchain.toml). */
function rustcVersion(cwd) {
  if (rustcVersionMemo && rustcVersionMemo.cwd === cwd) return rustcVersionMemo.v;
  const r = spawnSync("rustc", ["-V"], { cwd, encoding: "utf8", windowsHide: true });
  if (r.status !== 0) throw new Error(`rustc -V failed in ${cwd}: ${(r.stderr || r.error || "").toString().trim()}`);
  rustcVersionMemo = { cwd, v: r.stdout.trim() };
  return rustcVersionMemo.v;
}

/** Why this source cannot be cached by its own bytes, or null when it can. */
function uncacheableReason(text) {
  if (/\binclude_(str|bytes)!\s*\(/.test(text)) return "it uses include_str!/include_bytes!";
  if (/#\s*\[\s*path\s*=/.test(text)) return "it uses #[path]";
  if (/^\s*(pub(\([^)]*\))?\s+)?mod\s+\w+\s*;/m.test(text)) return "it declares a `mod x;` read from another file";
  return null;
}

/** The cache key for one compile. Pure. */
function cacheKey({ srcBytes, rustcV, args, env }) {
  const h = crypto.createHash("sha256");
  h.update("hos-rustc-cache v1\n");
  h.update(rustcV + "\n");
  h.update(JSON.stringify(args) + "\n");
  const keys = Object.keys(env || {}).sort();
  h.update(JSON.stringify(keys.map((k) => [k, env[k]])) + "\n");
  h.update(crypto.createHash("sha256").update(srcBytes).digest("hex"));
  return h.digest("hex");
}

/** A folder of this run's own INSIDE the cache folder, so the final rename never crosses drives. */
function workDir(p) {
  fs.mkdirSync(p.cacheDir, { recursive: true });
  return fs.mkdtempSync(path.join(p.cacheDir, `.work-${p.name}-`));
}

function exeName(name, key) {
  return `${name}-${key.slice(0, 16)}${process.platform === "win32" ? ".exe" : ""}`;
}

function prune(cacheDir, name, keep) {
  let entries = [];
  try {
    entries = fs
      .readdirSync(cacheDir)
      .filter((f) => f.startsWith(name + "-") && /-[0-9a-f]{16}(\.exe)?$/.test(f))
      .map((f) => ({ f, t: fs.statSync(path.join(cacheDir, f)).mtimeMs }))
      .sort((a, b) => b.t - a.t);
  } catch {
    return;
  }
  for (const e of entries.slice(keep)) {
    try {
      fs.unlinkSync(path.join(cacheDir, e.f));
    } catch {
      /* in use (running): next time */
    }
  }
  // Work folders a killed run left behind (a live one is minutes old at most).
  try {
    for (const f of fs.readdirSync(cacheDir)) {
      if (!f.startsWith(".work-")) continue;
      const p = path.join(cacheDir, f);
      if (Date.now() - fs.statSync(p).mtimeMs > 3600 * 1000) fs.rmSync(p, { recursive: true, force: true });
    }
  } catch {
    /* nothing to tidy */
  }
}

/** The plan for one compile: where it lives, whether it is there already. Pure but for stat. */
function plan({ src, name, cacheDir, args = [], env = {}, cwd }) {
  const srcBytes = fs.readFileSync(src);
  const rustcV = rustcVersion(cwd);
  const reason = uncacheableReason(srcBytes.toString("utf8"));
  const key = cacheKey({ srcBytes, rustcV, args, env });
  const exe = path.join(cacheDir, exeName(name, key));
  return { src, name, cacheDir, args, env, cwd, key, exe, uncacheable: reason, cached: !reason && fs.existsSync(exe) };
}

/** Build one planned program (synchronously). Returns { ok, exe, cached, ms, stderr }. */
function buildSync(p) {
  if (p.cached) return { ok: true, exe: p.exe, cached: true, ms: 0, stderr: "" };
  const t0 = Date.now();
  const work = workDir(p);
  const out = path.join(work, exeName(p.name, p.key));
  const r = spawnSync("rustc", [...p.args, p.src, "-o", out], {
    cwd: p.cwd,
    env: { ...process.env, ...p.env },
    encoding: "utf8",
    windowsHide: true,
  });
  return finish(p, work, out, r.status, r.stderr || (r.error ? String(r.error) : ""), t0);
}

/** Build one planned program (asynchronously). Resolves to what buildSync returns. */
function buildAsync(p) {
  if (p.cached) return Promise.resolve({ ok: true, exe: p.exe, cached: true, ms: 0, stderr: "" });
  const t0 = Date.now();
  const work = workDir(p);
  const out = path.join(work, exeName(p.name, p.key));
  return new Promise((resolve) => {
    const c = spawn("rustc", [...p.args, p.src, "-o", out], { cwd: p.cwd, env: { ...process.env, ...p.env }, windowsHide: true });
    let err = "";
    c.stderr.on("data", (d) => (err += d));
    c.on("error", (e) => resolve(finish(p, work, out, 1, String(e), t0)));
    c.on("close", (code) => resolve(finish(p, work, out, code, err, t0)));
  });
}

function finish(p, work, out, status, stderr, t0) {
  const ms = Date.now() - t0;
  if (status !== 0 || !fs.existsSync(out)) {
    fs.rmSync(work, { recursive: true, force: true });
    return { ok: false, exe: null, cached: false, ms, stderr };
  }
  if (p.uncacheable) {
    // Not cached: keep it in its own folder for this run; the caller may delete it.
    return { ok: true, exe: out, cached: false, ms, stderr, scratch: work };
  }
  fs.mkdirSync(p.cacheDir, { recursive: true });
  try {
    fs.renameSync(out, p.exe);
  } catch (e) {
    // Another run put the same program there first (same key, same bytes): use theirs.
    if (!fs.existsSync(p.exe)) {
      fs.rmSync(work, { recursive: true, force: true });
      return { ok: false, exe: null, cached: false, ms, stderr: `could not move the program into ${p.cacheDir}: ${e.message}` };
    }
  }
  fs.rmSync(work, { recursive: true, force: true });
  prune(p.cacheDir, p.name, KEEP_PER_NAME);
  return { ok: true, exe: p.exe, cached: false, ms, stderr };
}

module.exports = { rustcVersion, uncacheableReason, cacheKey, exeName, plan, buildSync, buildAsync, prune };
