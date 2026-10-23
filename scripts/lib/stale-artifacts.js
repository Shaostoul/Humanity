// Old builds of OUR crate inside a cargo target folder (2026-10-23, the build-speed review).
//
// Cargo puts the package version into every artifact's hash (humanity_engine-<16 hex>.rlib,
// the test program humanity_engine-<hash>.exe and its .pdb, HumanityOS-<hash>.exe, the
// .fingerprint/humanity-engine-<hash>/ folder), and this project bumps its version several times
// a day. So every release, every feature set and every toolchain leaves one more full set
// behind, and cargo never deletes one: on 2026-10-22 the main checkout's target\debug held
// about 146 GB of them, 3,606 humanity_engine files and 1,449 fingerprint folders.
//
// This finds them. A GROUP is one crate's files sharing one hash in deps/, where the bytes are
// (the .fingerprint/ and build/ folders are kilobytes and are left alone). Only the workspace's
// own crates are touched (the library, the binary, the integration tests and examples), never a
// dependency: a dependency's old copy is small, and its "newest" may legitimately be weeks old.
// A group is STALE when nothing in it was written for `staleDays` (cargo rewrites a group each
// time it rebuilds it, and our crate is rebuilt on every edit) AND newer groups of the same
// crate exist: the newest `keep` groups of a crate are always kept, however old (six: one crate
// has several live builds at once, check and test and build, native and relay), so a target
// folder nobody built in for a month keeps them. Incremental folders
// (incremental/<crate>-<id>/) are judged the same way by their newest session.
//
// planPrune is pure (a list of entries in, a list of deletions out), so the rule is tested on
// made-up folders; scanProfile reads a real one.

"use strict";

const fs = require("fs");
const path = require("path");

const DAY = 24 * 3600 * 1000;

/** `libhumanity_engine-0123456789abcdef.rlib` -> { crate: "humanity_engine", hash } (null when not an artifact). */
function parseDepName(file) {
  const m = String(file).match(/^(?:lib)?([A-Za-z0-9_]+)-([0-9a-f]{16})(?:\.[A-Za-z0-9.]+)?$/);
  return m ? { crate: m[1], hash: m[2] } : null;
}
/** `.fingerprint/humanity-engine-0123456789abcdef` (or build/) -> { crate: "humanity_engine", hash }. */
function parseUnitDir(name) {
  const m = String(name).match(/^([A-Za-z0-9_-]+)-([0-9a-f]{16})$/);
  return m ? { crate: m[1].replace(/-/g, "_"), hash: m[2] } : null;
}
/** `incremental/humanity_engine-1abc2def3gh4i` -> { crate } (the id is not the deps hash). */
function parseIncrementalDir(name) {
  const m = String(name).match(/^([A-Za-z0-9_]+)-([0-9a-z]+)$/);
  return m ? { crate: m[1], id: m[2] } : null;
}

/**
 * Plan what to delete. Pure.
 *   entries   [{ kind: "deps"|"incremental", path, crate, key, mtimeMs, bytes }]
 *             key: the hash, or the incremental folder's id
 *   crates    the workspace's crate names (the only ones touched)
 *   now, staleDays, keep
 * Returns { remove: [entries], kept: count, groups: [{ crate, key, newest, bytes, stale }] }.
 */
function planPrune(entries, { crates, now = Date.now(), staleDays = 3, keep = 6 }) {
  const ours = new Set(crates);
  const groups = new Map(); // "family:crate:key" -> group
  for (const e of entries) {
    if (!ours.has(e.crate)) continue;
    const family = e.kind === "incremental" ? "incremental" : "build";
    const id = `${family}:${e.crate}:${e.key}`;
    let g = groups.get(id);
    if (!g) groups.set(id, (g = { family, crate: e.crate, key: e.key, newest: 0, bytes: 0, entries: [] }));
    g.newest = Math.max(g.newest, e.mtimeMs);
    g.bytes += e.bytes || 0;
    g.entries.push(e);
  }
  const byCrate = new Map();
  for (const g of groups.values()) {
    const k = `${g.family}:${g.crate}`;
    if (!byCrate.has(k)) byCrate.set(k, []);
    byCrate.get(k).push(g);
  }
  const remove = [];
  for (const list of byCrate.values()) {
    list.sort((a, b) => b.newest - a.newest);
    list.forEach((g, i) => {
      g.stale = i >= keep && now - g.newest > staleDays * DAY;
      if (g.stale) remove.push(...g.entries);
    });
  }
  return { remove, groups: [...groups.values()] };
}

function statOr(p) {
  try {
    return fs.statSync(p);
  } catch {
    return null;
  }
}
/** Newest mtime and total size under a folder (one level of sessions is enough for incremental). */
function dirStats(dir, depth = 2) {
  let newest = 0;
  let bytes = 0;
  const walk = (d, left) => {
    let list = [];
    try {
      list = fs.readdirSync(d, { withFileTypes: true });
    } catch {
      return;
    }
    for (const e of list) {
      const p = path.join(d, e.name);
      const st = statOr(p);
      if (!st) continue;
      newest = Math.max(newest, st.mtimeMs);
      if (e.isDirectory()) {
        if (left > 0) walk(p, left - 1);
      } else bytes += st.size;
    }
  };
  const st = statOr(dir);
  if (st) newest = st.mtimeMs;
  walk(dir, depth);
  return { newest, bytes };
}

/** Every entry planPrune needs from one profile folder (target/debug, target/release). */
function scanProfile(profileDir) {
  const out = [];
  const deps = path.join(profileDir, "deps");
  let files = [];
  try {
    files = fs.readdirSync(deps);
  } catch {}
  for (const f of files) {
    const p = parseDepName(f);
    if (!p) continue;
    const st = statOr(path.join(deps, f));
    if (st && st.isFile()) out.push({ kind: "deps", path: path.join(deps, f), crate: p.crate, key: p.hash, mtimeMs: st.mtimeMs, bytes: st.size });
  }
  let inc = [];
  try {
    inc = fs.readdirSync(path.join(profileDir, "incremental"));
  } catch {}
  for (const d of inc) {
    const p = parseIncrementalDir(d);
    if (!p) continue;
    const full = path.join(profileDir, "incremental", d);
    const s = dirStats(full, 2);
    out.push({ kind: "incremental", path: full, crate: p.crate, key: p.id, mtimeMs: s.newest, bytes: s.bytes, dir: true });
  }
  return out;
}

/** The workspace's crate names in a checkout: the package's lib and bin, its tests/ and examples/. */
function workspaceCrates(root) {
  const names = new Set(["humanity_engine", "HumanityOS", "build_script_build"]);
  for (const sub of ["tests", "examples"]) {
    let list = [];
    try {
      list = fs.readdirSync(path.join(root, sub));
    } catch {}
    for (const f of list) if (f.endsWith(".rs")) names.add(f.slice(0, -3));
  }
  return [...names];
}

module.exports = { parseDepName, parseUnitDir, parseIncrementalDir, planPrune, scanProfile, workspaceCrates, DAY };
