// Where a rig finds the fast shader compiler, in ONE place for every rig.
//
// The game compiles its shaders with DXC when dxcompiler.dll and dxil.dll sit
// beside the exe, and falls back to FXC when they do not (src/renderer/mod.rs,
// the `backend_options` block: it needs BOTH files, from the exe's own
// folder). FXC is far slower: about 25 s of boot instead of 5 on a warm cache,
// and on 2026-10-03 a rig's sandbox sat in FXC's pipeline compile for over
// three minutes, long past the autopilot's wait, which reads as "the rig is
// broken" when it is merely slow.
//
// Every rig copies the exe into a sandbox folder of its own and boots it
// there, so the rig has to copy the pair in too. The default exe is
// target/release/HumanityOS.exe, and target/release holds no dlls: the pair
// lives in the repo root, beside the archived v*_HumanityOS.exe the taskbar
// launches (scripts/archive-build.js refreshes that copy from target/release
// only when target/release holds one).
//
// Until 2026-10-03 each of the seven rigs carried its own copy of the lookup:
//   - verify-live-screen.js, verify-screens.js, probe-sweep.js and
//     photograph-home.js looked ONLY beside the exe. From target/release they
//     booted on FXC, saying nothing, whenever the rig folder held no pair left
//     from an earlier copy (fresh checkout, cleaned .probe-rig, a new rig).
//     The older rig folders on the operator's machine did hold such a pair,
//     which is how the gap went unseen: the last run logs of the live-screen
//     rig (2026-09-19) and the screens rig (2026-09-25) both name DXC.
//   - boot-timing.js also looked only beside the exe, but refused to run, with
//     a message telling you to copy the pair "from target/release", which
//     holds none.
//   - make-clips.js and verify-copresence.js searched both folders, but took
//     each file from wherever it turned up first, so they could pair two
//     different releases (see below).
//
// This module is now the one lookup all seven use, and
// scripts/tests/dxc-dlls.test.js fails if any of them, or any new script
// that boots a rig, carries a lookup of its own:
//
//   1. the exe's own folder, then
//   2. the repo root,
//
// taking the first folder that holds BOTH files, and saying which it used, or
// that it found neither, in the rig's output.
//
// Why a folder must hold both rather than taking each file from wherever it
// turns up first: dxil.dll validates and signs what dxcompiler.dll produces,
// and the two ship as a pair from one release. A dxcompiler.dll from one folder
// beside a dxil.dll from another can be two different versions, so a split
// pair is reported, not assembled.
//
// The search is pure (it takes an `exists` function), so
// scripts/tests/dxc-dlls.test.js can check every case without a game.

const fs = require("fs");
const path = require("path");

/** The two files, in the order the game's own check names them. */
const DXC_DLLS = ["dxcompiler.dll", "dxil.dll"];

/** Compare two folder paths the way the file system does: Windows ignores case. */
function sameDir(a, b) {
  const ra = path.resolve(a);
  const rb = path.resolve(b);
  return process.platform === "win32" ? ra.toLowerCase() === rb.toLowerCase() : ra === rb;
}

/**
 * The folders to search, in order, without repeats. An exe that already sits
 * in the repo root (an archived v*_HumanityOS.exe) gives one folder, not the
 * same folder twice under two names.
 */
function searchDirs(exe, repo) {
  const dirs = [{ dir: path.dirname(path.resolve(exe)), where: "beside the exe" }];
  if (repo && !sameDir(repo, dirs[0].dir)) dirs.push({ dir: path.resolve(repo), where: "in the repo root" });
  return dirs;
}

/**
 * Find the pair. Pure: `exists` is the only contact with the disk.
 *
 * Returns { found, dir, where, paths, searched }:
 *   found    true when some folder holds both files
 *   dir      that folder (null when none does)
 *   where    "beside the exe" or "in the repo root" (null when none does)
 *   paths    { "dxcompiler.dll": full path, "dxil.dll": full path } (null when none does)
 *   searched every folder looked at, with the dlls it does hold, so a report
 *            can name a half pair rather than just say "missing"
 */
function findDxcDlls({ exe, repo, exists = fs.existsSync }) {
  const searched = searchDirs(exe, repo).map((d) => ({
    ...d,
    has: DXC_DLLS.filter((dll) => exists(path.join(d.dir, dll))),
  }));
  const hit = searched.find((d) => d.has.length === DXC_DLLS.length);
  if (!hit) return { found: false, dir: null, where: null, paths: null, searched };
  const paths = {};
  for (const dll of DXC_DLLS) paths[dll] = path.join(hit.dir, dll);
  return { found: true, dir: hit.dir, where: hit.where, paths, searched };
}

/**
 * One line for the rig's output: which folder the pair came from, or that no
 * folder had it and what that costs. `staleInRig` is true when the sandbox
 * still holds a pair from an earlier run, which the game WILL use, so the line
 * must not claim FXC in that case.
 */
function describeDxc(result, { staleInRig = false } = {}) {
  if (result.found) {
    // The folders searched BEFORE the one that had the pair, named so the
    // line says "repo root (none beside the exe)": that is the case the
    // old lookup missed, and the line should show it was looked at.
    const hitAt = result.searched.findIndex((d) => d.dir === result.dir);
    const passed = result.searched.slice(0, hitAt);
    const note = passed.length ? ` (none ${passed.map((d) => d.where).join(" or ")})` : "";
    return `shader compiler: DXC, ${DXC_DLLS.join(" + ")} found ${result.where}, ${result.dir}${note}`;
  }
  const looked = result.searched
    .map((d) => `${d.where}, ${d.dir}: ${d.has.length ? `only ${d.has.join(", ")}` : "neither"}`)
    .join("; ");
  const head = `WARNING: shader compiler: no folder searched holds both ${DXC_DLLS.join(" and ")} (${looked}).`;
  if (staleInRig) {
    return `${head} The rig folder still holds a pair copied by an earlier run, so the game will use that (DXC).`;
  }
  return (
    `${head} The game will compile its shaders with FXC, which takes minutes on a first boot. ` +
    `Put both dlls in the repo root (from the Windows SDK bin folder or a DirectXShaderCompiler release).`
  );
}

/**
 * Find the pair and copy it into the rig's sandbox folder `dest`, logging the
 * one line describeDxc writes. Copies nothing when no folder holds both, and
 * never copies half a pair. Returns the findDxcDlls result.
 *
 * `exists` and `copy` default to the real file system; tests may pass fakes.
 */
function copyDxcDlls({ exe, repo, dest, log = console.log, exists = fs.existsSync, copy = fs.copyFileSync }) {
  const result = findDxcDlls({ exe, repo, exists });
  if (result.found) {
    for (const dll of DXC_DLLS) copy(result.paths[dll], path.join(dest, dll));
    log(describeDxc(result));
  } else {
    const staleInRig = DXC_DLLS.every((dll) => exists(path.join(dest, dll)));
    log(describeDxc(result, { staleInRig }));
  }
  return result;
}

module.exports = { DXC_DLLS, findDxcDlls, describeDxc, copyDxcDlls, searchDirs };
