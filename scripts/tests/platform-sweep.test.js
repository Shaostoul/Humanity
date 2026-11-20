// No dev script asks the OS for processes or the exe's name by itself: they ask scripts/lib/platform.js.
//
// THE FAILURE CLASS (2026-11-16, docs/design/cross-platform-dev.md). A contributor on macOS ran the
// rigs and every one of them failed or did nothing, silently: they joined "HumanityOS.exe" into
// paths (the binary is "HumanityOS" there), listed processes with tasklist or PowerShell's
// Get-CimInstance (which answered "nothing running", so the one-GPU guard saw no other game), and
// ended their games with taskkill (which did nothing, so a rig's game outlived the rig). 26 scripts,
// 147 mentions. 2026-11-20 swept them onto platform.js; this test keeps them there, because the
// next rig is written by copying an old one, and the old ones were all Windows.
//
// What it flags, in code (comments are skipped, and so are the words a person reads, such as
// "taskkill //PID 123" with doubled slashes in a refusal message: those come from platform.js's
// killCommand where a script was swept, and a stray one misleads, it does not break):
//   - "HumanityOS.exe" joined into a path: use platform.js EXE_NAME or builtExe(repo)
//   - taskkill / tasklist / Get-CimInstance / Get-Process / Stop-Process run as a command: use
//     platform.js listProcesses, childProcesses, killTree or killByExe
//   - mklink / fsutil (2026-11-20: probe-sweep's junction failed five rig tests on Linux CI): use
//     platform.js linkDir and isLink
// A file that is about Windows itself is listed below with its reason. Adding a file there is the
// last resort, for a script that only means anything on Windows.
//
// Seen red 2026-11-20: over the tree before the sweep it finds 127 lines in 22 files; after it, with
// one `taskkill /PID` put back into verify-falls.js on purpose:
//   "scripts/verify-falls.js:646 runs a Windows process command itself"
// and with "HumanityOS.exe" put back into boot-timing.js's rig path:
//   "scripts/boot-timing.js:131 joins the Windows file name HumanityOS.exe into a path",
// and (2026-11-20, after Linux CI) with boot-timing.js's `cmd /c mklink /J` put back:
//   "scripts/boot-timing.js:126 runs a Windows process command itself".

"use strict";

const test = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const path = require("path");

const ROOT = path.resolve(__dirname, "..", "..");
const SCRIPTS = path.join(ROOT, "scripts");

/** Files whose whole point is Windows, each with the reason it may speak Windows directly. */
const WINDOWS_BY_DESIGN = {
  "scripts/lib/platform.js": "the one place that asks the OS: Windows, Linux and macOS each in its own branch",
  "scripts/lib/machine-guard.js": "its Windows process query (psQuery) and the BUG-161 priority hold are PowerShell by design; off Windows it lists through platform.js and holds nothing",
  "scripts/archive-build.js": "the operator's Windows delivery: v<version>_HumanityOS.exe archives and C:\\Humanity\\HumanityOS.exe",
  "scripts/check-delivery.js": "checks that same Windows delivery",
  "scripts/dxc-beside-exe.js": "DirectX 12's shader compiler beside the Windows exe; macOS and Linux render with Metal or Vulkan",
  "scripts/lib/window-state.js": "reads a Windows window's state through window-state.ps1",
};

/** Every .js under scripts/ except the tests and their fixtures (a test may be Windows-only on purpose, with a skip). */
function scriptFiles(dir = SCRIPTS, out = []) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) {
      if (["tests", "node_modules", "fixtures"].includes(e.name)) continue;
      scriptFiles(p, out);
    } else if (e.name.endsWith(".js")) out.push(p);
  }
  return out;
}

const JOINS_EXE = /path\.(join|resolve)\([^)]*["'`]HumanityOS\.exe["'`]/;
const RUNS_COMMAND = /["']taskkill["']|taskkill \/PID|["']tasklist["']|["'`]tasklist |tasklist \/FO|Get-CimInstance|\bGet-Process\b|\bStop-Process\b|\bmklink\b|\bfsutil\b/;

/** The code lines of a file that break the rule: [{ line, text, why }]. PURE (text in). */
function findings(text) {
  const out = [];
  let inBlock = false;
  String(text)
    .split(/\r?\n/)
    .forEach((raw, i) => {
      let l = raw;
      if (inBlock) {
        const end = l.indexOf("*/");
        if (end < 0) return;
        l = l.slice(end + 2);
        inBlock = false;
      }
      const t = l.trim();
      if (t.startsWith("//") || t.startsWith("*")) return;
      if (t.startsWith("/*")) {
        if (!t.includes("*/")) inBlock = true;
        return;
      }
      // A trailing // comment is not code (a URL's // sits inside quotes: only cut at " //").
      const code = l.replace(/\s\/\/\s.*$/, "");
      if (JOINS_EXE.test(code)) out.push({ line: i + 1, text: t, why: "joins the Windows file name HumanityOS.exe into a path (use platform.js EXE_NAME or builtExe(repo))" });
      else if (RUNS_COMMAND.test(code)) out.push({ line: i + 1, text: t, why: "runs a Windows process command itself (use platform.js listProcesses, childProcesses, killTree, killByExe, or linkDir and isLink for folder links)" });
    });
  return out;
}

test("the finder catches the old forms and passes the swept ones", () => {
  const bad = [
    'const EXE = path.resolve(opt("--exe", path.join(REPO, "target", "release", "HumanityOS.exe")));',
    'fs.copyFileSync(EXE, path.join(RIG, "HumanityOS.exe"));',
    'execSync(`taskkill /PID ${pid} /T /F`, { stdio: "ignore" });',
    'execFileSync("taskkill", ["/PID", String(pid)], { stdio: "ignore" });',
    "const out = execSync('tasklist /FO CSV /NH');",
    '`powershell -NoProfile -Command "Get-Process HumanityOS | Stop-Process -Force"`',
    "const ps = `Get-CimInstance Win32_Process`;",
    'execSync(`cmd /c mklink /J "${link}" "${target}"`, { stdio: "ignore" });',
    'execSync(`fsutil reparsepoint query "${link}"`, { stdio: "ignore" });',
  ];
  for (const b of bad) assert.strictEqual(findings(b).length, 1, `not caught: ${b}`);
  const good = [
    "const EXE = path.resolve(opt(\"--exe\", PLAT.builtExe(REPO)));",
    "fs.copyFileSync(EXE, path.join(RIG, PLAT.EXE_NAME));",
    "PLAT.killTree(pid, { force: true });",
    "// execSync(`taskkill /PID ${pid} /T /F`) used to be here",
    " * taskkill /PID in a doc comment",
    'log(`left running: ${PLAT.killCommand(pid)} to close it`);',
    '"If it is a leftover rig of ours: taskkill //PID <pid> //F",',
    'log("boot 2: closing into the tray, then taskkill without /F");',
    'log(`taskkill while hidden: ${graceful ? "closed" : "FORCED"}`);',
    "  tasklist: () => `<rect/>`,",
  ];
  for (const g of good) assert.deepStrictEqual(findings(g), [], `wrongly caught: ${g}`);
  assert.deepStrictEqual(findings("/* a block\n taskkill /PID 5\n*/ PLAT.killTree(5);").length, 0, "inside a block comment");
});

test("no dev script asks the OS for processes or the exe's name by itself", () => {
  const problems = [];
  const files = scriptFiles();
  assert.ok(files.length > 100, `only ${files.length} scripts found: the walk is broken`);
  for (const f of files) {
    const rel = path.relative(ROOT, f).replace(/\\/g, "/");
    if (WINDOWS_BY_DESIGN[rel]) continue;
    for (const x of findings(fs.readFileSync(f, "utf8"))) problems.push(`${rel}:${x.line} ${x.why}\n    ${x.text.slice(0, 160)}`);
  }
  assert.deepStrictEqual(
    problems,
    [],
    "\n" +
      problems.join("\n") +
      "\n\nThese scripts would fail or do nothing off Windows (docs/design/cross-platform-dev.md). Use scripts/lib/platform.js." +
      "\nA script that only means anything on Windows goes in WINDOWS_BY_DESIGN in this test, with its reason.\n",
  );
});

test("every Windows-by-design entry still exists and still needs to be there", () => {
  for (const [rel, why] of Object.entries(WINDOWS_BY_DESIGN)) {
    const p = path.join(ROOT, rel);
    assert.ok(fs.existsSync(p), `${rel} is listed (${why}) but is gone: take it off the list`);
    assert.ok(why.length > 20, `${rel} needs a reason`);
    if (rel === "scripts/lib/window-state.js") continue; // speaks Windows through its .ps1, not a pattern here
    assert.ok(findings(fs.readFileSync(p, "utf8")).length > 0, `${rel} no longer speaks Windows directly: take it off WINDOWS_BY_DESIGN so the rule covers it`);
  }
});

