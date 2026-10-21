#!/usr/bin/env node
// Put the fast shader compiler (dxcompiler.dll + dxil.dll) beside target/release/HumanityOS.exe
// before `just launch-bg` boots it (2026-10-18).
//
// The game compiles its shaders with DXC only when both dlls sit beside its exe, and otherwise
// falls back to FXC, which spent minutes in the pipeline compile on 2026-10-18 (over 540 s of CPU
// before the first frame, the window "not responding" the whole time). target/release holds no
// pair once the disk guard or `cargo clean` has emptied it, and the seven verification rigs each
// copy the pair into their own sandbox through scripts/lib/dxc-dlls.js, but launch-bg, the boot
// check CLAUDE.md names for every renderer change, did not: so its boots, and every number read
// off them, were FXC's.
//
// Uses the one shared lookup (the exe's folder, then the repo root, then, in a git worktree, the
// main checkout's root, where the pair actually lives: it is not in git, so a worktree never holds
// one; both files from the same folder). With no pair anywhere it says so and exits 0: the game
// still boots, only slower.
"use strict";
const fs = require("fs");
const path = require("path");
const DXC = require("./lib/dxc-dlls.js");

const REPO = path.resolve(__dirname, "..");
const exe = path.join(REPO, "target", "release", "HumanityOS.exe");
const dest = path.dirname(exe);

const found = DXC.findDxcDlls({ exe, repo: REPO });
if (!found.found) {
  console.log(DXC.describeDxc(found) + "\n  (booting anyway: shaders compile with FXC, much slower)");
  process.exit(0);
}
for (const dll of DXC.DXC_DLLS) {
  const to = path.join(dest, dll);
  if (path.resolve(found.paths[dll]) !== path.resolve(to)) fs.copyFileSync(found.paths[dll], to);
}
if (!process.argv.includes("--quiet")) console.log(DXC.describeDxc(found));
