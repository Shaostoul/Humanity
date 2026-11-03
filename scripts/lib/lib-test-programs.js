// The workspace's test programs, for scripts/lib-tests.js (the crate split's rung 1, 2026-11-01):
// which members there are, which test program cargo built for which member, and one summary line
// for several programs' runs. Pure functions, tested in scripts/tests/lib-tests-policy.test.js.
//
// Since rung 1 each member of the workspace has a test program of its own (humanity_core-<hash>.exe,
// humanity_protocol-<hash>.exe, humanity_engine-<hash>.exe). Until then there was one, and
// lib-tests.js took "the" test artifact cargo printed: kept that way it would run a third of the
// suite and call it green. So every member's program is found by its package, and a member without
// one fails the build step.

"use strict";

/** The workspace's members that have a library, as package names, in `cargo metadata`'s order
 * (its `workspace_members`), from its JSON. */
function membersOf(meta) {
  const byId = new Map(meta.packages.map((p) => [p.id, p]));
  return meta.workspace_members
    .map((id) => byId.get(id))
    .filter(Boolean)
    .filter((p) => p.targets.some((t) => t.kind.includes("lib")))
    .map((p) => p.name);
}

/** The folder of each member that has a library (Map: package name -> folder), from `cargo
 * metadata`'s JSON: where `cargo test` starts that member's test program, and the
 * CARGO_MANIFEST_DIR it gives it. */
function memberDirs(meta) {
  const path = require("path");
  const byId = new Map(meta.packages.map((p) => [p.id, p]));
  return new Map(
    meta.workspace_members
      .map((id) => byId.get(id))
      .filter(Boolean)
      .filter((p) => p.targets.some((t) => t.kind.includes("lib")) && p.manifest_path)
      .map((p) => [p.name, path.dirname(p.manifest_path)]),
  );
}

/** A package's name from a cargo package id, in any of the forms cargo has used:
 * `path+file:///C:/x/crates/humanity-core#0.1.0` (the folder is the name), `...#humanity-core@0.1.0`,
 * or the old `humanity-core 0.1.0 (path+file:///...)`. */
function packageName(id) {
  const s = String(id);
  let m = /^([A-Za-z0-9_-]+) \d/.exec(s);
  if (m) return m[1];
  m = /#([A-Za-z0-9_-]+)@/.exec(s);
  if (m) return m[1];
  m = /[/\\]([A-Za-z0-9_-]+)#\d/.exec(s);
  return m ? m[1] : s;
}

/** The test program of each package in a cargo `--message-format=json` stream (lines): Map(package
 * name -> executable), from the artifacts of a library's test build. Lines that are not JSON are
 * returned in `other` (the build slot's own lines), to be shown. */
function testPrograms(lines) {
  const exes = new Map();
  const other = [];
  for (const line of lines) {
    if (!line.startsWith("{")) {
      if (line.trim()) other.push(line);
      continue;
    }
    try {
      const m = JSON.parse(line);
      if (m.reason === "compiler-artifact" && m.executable && m.profile && m.profile.test && m.target && m.target.kind.includes("lib")) {
        exes.set(packageName(m.package_id), m.executable);
      }
    } catch {
      /* not a cargo message */
    }
  }
  return { exes, other };
}

/** Several programs' "test result:" lines as one, the counts added up and each program's passed
 * count after it: "test result: ok. 5584 passed; 0 failed; ... (humanity-core 1201, ...)". One
 * program's line is returned as it is. `runs`: [{ name, summary }] (summary: its last result line). */
function combinedSummary(runs) {
  if (runs.length === 1) return runs[0].summary;
  const keys = ["passed", "failed", "ignored", "measured", "filtered out"];
  const total = Object.fromEntries(keys.map((k) => [k, 0]));
  const parts = [];
  let ok = true;
  for (const r of runs) {
    const s = r.summary;
    if (!s) {
      ok = false;
      parts.push(`${r.name}: no result line`);
      continue;
    }
    if (!/^test result: ok\./.test(s)) ok = false;
    for (const k of keys) {
      const m = new RegExp(`(\\d+) ${k}`).exec(s);
      if (m) total[k] += Number(m[1]);
    }
    parts.push(`${r.name} ${(/(\d+) passed/.exec(s) || [])[1] || 0}`);
  }
  return `test result: ${ok ? "ok" : "FAILED"}. ${keys.map((k) => `${total[k]} ${k}`).join("; ")} (${parts.join(", ")})`;
}

module.exports = { membersOf, memberDirs, packageName, testPrograms, combinedSummary };
