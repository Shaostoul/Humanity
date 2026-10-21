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
//   2. the repo root, then
//   3. the MAIN checkout's root, when the repo is a git worktree,
//
// taking the first folder that holds BOTH files, and saying which it used, or
// that it found neither, in the rig's output.
//
// The third folder since 2026-10-21. The pair is not in git (it is Microsoft's
// binary, placed by hand in C:\Humanity), so a worktree never holds one, and a
// rig run from a worktree's target/release booted on FXC: its ~170 s pipeline
// build nearly overran the 180 s autopilot wait (the room-GI lane), and
// BUG-140's fixer copied the pair in by hand to get on. The main checkout is
// found the way `git rev-parse --git-common-dir` finds it (mainCheckoutRoot).
//
// And a rig that boots on FXC anyway now says so in its verdict, not only in a
// setup line that has scrolled away by then: lib/game-launch.js spawnGame
// checks the pair beside the exe it starts (the game's own test) and, when it
// is missing, prints fxcBanner() as the rig's last lines and tells a parent
// gate (verify-runtime), which prints it in its own verdict.
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
 * The main checkout's root when `repo` is a git worktree: the parent of the
 * folder `git rev-parse --git-common-dir` names. null for the main checkout
 * itself, or for a folder that is not a git checkout at all.
 *
 * Read from the files git keeps rather than by running git (no git on PATH
 * needed, nothing to time out): a worktree's `.git` is a FILE, "gitdir: <its
 * folder under the main .git/worktrees/>", and that folder's `commondir` file
 * names the shared .git folder relative to it (git-worktree(1), "DETAILS").
 * scripts/tests/dxc-dlls.test.js checks the answer against git's own.
 */
function mainCheckoutRoot(repo, { readFile = (p) => fs.readFileSync(p, "utf8"), isDir = (p) => fs.statSync(p).isDirectory() } = {}) {
  if (!repo) return null;
  try {
    const dotgit = path.join(path.resolve(repo), ".git");
    if (isDir(dotgit)) return null; // the main checkout itself
    const m = readFile(dotgit).match(/^gitdir:\s*(.+?)\s*$/m);
    if (!m) return null;
    const gitdir = path.resolve(repo, m[1]);
    const common = readFile(path.join(gitdir, "commondir")).trim();
    return path.dirname(path.resolve(gitdir, common));
  } catch {
    return null;
  }
}

/**
 * The folders to search, in order, without repeats. An exe that already sits
 * in the repo root (an archived v*_HumanityOS.exe) gives one folder, not the
 * same folder twice under two names. `mainRoot` is the main checkout's root
 * when `repo` is a worktree (mainCheckoutRoot), searched last.
 */
function searchDirs(exe, repo, mainRoot = null) {
  const dirs = [];
  const add = (dir, where) => {
    if (dir && !dirs.some((d) => sameDir(d.dir, dir))) dirs.push({ dir: path.resolve(dir), where });
  };
  add(path.dirname(path.resolve(exe)), "beside the exe");
  add(repo, "in the repo root");
  add(mainRoot, "in the main checkout's root");
  return dirs;
}

/**
 * Find the pair. Pure when `mainRoot` is given (null for none): `exists` is
 * then the only contact with the disk. Left out, the main checkout is found
 * from `repo` (mainCheckoutRoot).
 *
 * Returns { found, dir, where, paths, searched }:
 *   found    true when some folder holds both files
 *   dir      that folder (null when none does)
 *   where    "beside the exe", "in the repo root" or "in the main checkout's
 *            root" (null when none does)
 *   paths    { "dxcompiler.dll": full path, "dxil.dll": full path } (null when none does)
 *   searched every folder looked at, with the dlls it does hold, so a report
 *            can name a half pair rather than just say "missing"
 */
function findDxcDlls({ exe, repo, mainRoot, exists = fs.existsSync }) {
  const main = mainRoot === undefined ? mainCheckoutRoot(repo) : mainRoot;
  const searched = searchDirs(exe, repo, main).map((d) => ({
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
  // The folder to fix is the last one searched: the main checkout's root when the run is
  // in a worktree (every worktree looks there), else the repo root.
  const home = result.searched[result.searched.length - 1];
  return (
    `${head} The game will compile its shaders with FXC, which takes minutes on a first boot. ` +
    `Put both dlls ${home && home.where !== "beside the exe" ? `${home.where}, ${home.dir}` : "in the repo root"} ` +
    `(from the Windows SDK bin folder or a DirectXShaderCompiler release).`
  );
}

/**
 * Which compiler the game at `exe` will use, by the game's own test
 * (src/renderer/mod.rs dxc_dlls_beside_exe: BOTH dlls in the exe's folder):
 * "DXC" or "FXC". null off Windows, where there is no DX12.
 */
function compilerFor(exe, { exists = fs.existsSync, platform = process.platform } = {}) {
  if (platform !== "win32") return null;
  const dir = path.dirname(path.resolve(exe));
  return DXC_DLLS.every((dll) => exists(path.join(dir, dll))) ? "DXC" : "FXC";
}

/**
 * The lines a rig ends with when its game compiled its shaders with FXC: loud,
 * because every timing and every timeout in such a run is FXC's before it is
 * the build's (2026-10-21: ~170 s of pipeline build against a 180 s autopilot
 * wait). `where` is the folder to put the pair in (the main checkout's root
 * from a worktree, else the repo root).
 */
function fxcBanner({ exe, pid = null, rig = "rig", where = null } = {}) {
  return [
    "!! ".padEnd(72, "!"),
    `!! SHADER COMPILER: FXC. The game ${rig} started${pid ? ` (pid ${pid})` : ""} compiled its shaders with`,
    `!! FXC, the slow fallback: no ${DXC_DLLS.join(" + ")} pair beside ${exe || "its exe"}.`,
    "!! FXC's pipeline build takes minutes (about 170 s on 2026-10-21, against a 180 s autopilot",
    "!! wait): a timeout, a slow boot or a frame time from this run may be FXC's, not the build's.",
    `!! Fix: put both dlls in ${where || "the repo root"} (from the Windows SDK bin folder or a`,
    "!! DirectXShaderCompiler release), then run again. Every rig looks there, worktrees included.",
    "!! ".padEnd(72, "!"),
  ];
}

/**
 * Find the pair and copy it into the rig's sandbox folder `dest`, logging the
 * one line describeDxc writes. Copies nothing when no folder holds both, and
 * never copies half a pair. Returns the findDxcDlls result.
 *
 * `exists` and `copy` default to the real file system; tests may pass fakes.
 */
function copyDxcDlls({ exe, repo, mainRoot, dest, log = console.log, exists = fs.existsSync, copy = fs.copyFileSync }) {
  const result = findDxcDlls({ exe, repo, mainRoot, exists });
  if (result.found) {
    for (const dll of DXC_DLLS) copy(result.paths[dll], path.join(dest, dll));
    log(describeDxc(result));
  } else {
    const staleInRig = DXC_DLLS.every((dll) => exists(path.join(dest, dll)));
    log(describeDxc(result, { staleInRig }));
  }
  return result;
}

/** Where a person should put the pair, for a message: the main checkout's root
 *  when `repo` is a worktree, else the repo root. */
function pairHome(repo) {
  return mainCheckoutRoot(repo) || (repo ? path.resolve(repo) : null);
}

module.exports = { DXC_DLLS, findDxcDlls, describeDxc, copyDxcDlls, searchDirs, mainCheckoutRoot, compilerFor, fxcBanner, pairHome };
