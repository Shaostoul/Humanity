# Developing on macOS and Linux: what the dev tooling needs (feasibility, 2026-11-16)

A contributor on macOS could not run the project (2026-11-16). Fixed the same day: the Justfile's
shell and PATH (1dcd0e70b), five hardcoded personal paths (0ea86a5ad), and a backslash-path test.
The operator then asked whether the rig and build-slot tooling can work there too: "Let's find out if
we can do this". This is the answer, from reading the code. Steps 1, 2, 3 (Linux), the message
of step 4 and step 5 were built on 2026-11-20: see "Built" at the end.

## What happens off Windows today

| Piece | Windows-only part | Off Windows today | Effect |
|---|---|---|---|
| Build slot (`scripts/build-slot/hos-build-slot.rs`, 2,091 lines) | Free memory (`GlobalMemoryStatusEx`), job objects to measure a compile's process tree, process start times | Already compiles: `#[cfg(not(windows))]` stubs report endless memory and no start times | Compiles never wait for memory and the tree is not measured: it works, without protection |
| Machine guard (`scripts/lib/machine-guard.js`) | The process list through PowerShell `Get-CimInstance Win32_Process` | Degrades to "nothing running" | Rigs run, but do not see other games or builds |
| Game launch (`scripts/lib/game-launch.js`) | Child processes through `Get-CimInstance`; `taskkill /T /F` to end a tree | The lookups fail; the kill does nothing | A rig's game may outlive the rig |
| Rig scripts (26 files) | 147 mentions of `HumanityOS.exe`, many in comments and messages, many in paths such as `target/release/HumanityOS.exe` | The binary is `HumanityOS` there | Rigs look for a file that does not exist |
| Process tools (23 scripts) | `tasklist`, `taskkill`, `Get-CimInstance` | Fail | As above |
| Rig windows (`src/engine/rig_window.rs`) | Cloaking a test window (DWM) so it never covers the person's screen | `#[cfg(target_os = "windows")]` only | A rig's window shows normally |
| Share guard (`scripts/pc-share-guard.ps1`) | PowerShell priorities and cores | Not run | No priority lowering |
| DXC shader DLLs (`scripts/lib/dxc-dlls.js`) | DirectX 12's compiler | Not needed: macOS and Linux render with Metal or Vulkan | Nothing to do |

## The plan (one agent's job)

1. **`scripts/lib/platform.js`**: the executable's name (`HumanityOS.exe` or `HumanityOS`), the
   process list (`ps -axo pid,ppid,etime,comm,args` on macOS and Linux, the CIM query on Windows), ending
   a process tree (a process group, or walking `ps` children, then `SIGTERM` and `SIGKILL`), and lower
   priority (`nice`/`renice`). Each with a test that runs on the platform it is on.
2. **Sweep the 26 rig scripts** onto it: paths and lookups use the helper, words that name the file stay
   as they are where they mean Windows.
3. **The build slot on Linux**: free memory from `/proc/meminfo` (`MemAvailable`), start times from
   `/proc/<pid>/stat`, and a compile's tree measured by summing the resident sizes of its descendants
   from `/proc`. macOS second: `vm_stat` and `sysctl` for memory, `ps` for the tree; until then it keeps
   today's no-wait stub, said in its output.
4. **Rig windows**: off Windows a test window cannot be cloaked the same way; the rig says so before it
   boots, and where it can, opens the window minimised. The headless runner (`HumanityOS --sim soak`,
   `--sim walk`, built 2026-11-15) needs no window at all and already runs anywhere; increment 5
   (headless storeys and falls) removes most of the need.
5. **A Linux job in CI** running the node rig tests and the headless `--sim` runs, so a change that
   breaks the tooling off Windows is caught on the push.

## Built (2026-11-20)

| Step | What exists | Its check |
|---|---|---|
| 1 | `scripts/lib/platform.js`: `EXE_NAME`, `builtExe`, `listProcesses` (Windows: the CIM query; Linux: /proc, no child process; macOS: `ps`), `childProcesses`, `killTree` (taskkill `/T /F` on Windows, exactly as before; elsewhere the tree listed by parent id, then SIGTERM or SIGKILL), `killByExe`, `linkDir` / `isLink` (a rig's data/ and assets/ links: a junction on Windows, a symlink elsewhere), `startMs`, `lowerPriority` (Node's `os.setPriority`, never raising; one-way off Windows, where raising needs root), `killCommand` / `listCommand` for refusal messages | `scripts/tests/platform.test.js`: live on its own platform (the list holds the test's own process; a killed tree's detached grandchild is gone; the priority read back) plus the /proc, `ps` and CIM parsers on captured text |
| 2 | Every rig script, the machine guard (its Linux list comes from platform.js; the build names are `cargo`, `rustc`, `cc`, `ld` there; the BUG-161 priority hold says plainly it holds nothing off Windows, where a lowered priority cannot be put back), game-launch (hand-off search, stops), the boot lock's start time, mcp-server, browser-webrtc, clean-test-temp, pc-disk-guard, the build A/B tools | `scripts/tests/platform-sweep.test.js` fails on a script under `scripts/` that joins `HumanityOS.exe` into a path or runs taskkill, tasklist, Get-CimInstance, Get-Process or Stop-Process itself; six files that are about Windows itself are listed with their reasons (127 lines in 22 files before the sweep) |
| 3 | The build slot on Linux: MemAvailable for physical memory; commit counted only when the kernel enforces it (`vm.overcommit_memory` 2), else MemAvailable plus free swap; start times from `/proc/<pid>/stat`; a thread walks /proc for a compile's tree every 200 ms (each process's VmHWM, the largest sum of private memory at one look), getrusage for its CPU seconds. macOS keeps the no-wait stub and says so on every compile and in `--status` | The wrapper's own tests (captured text and a fake /proc tree, run on every platform; a live /proc test on Linux); `build-slot.test.js`'s queue, line and retry tests now run on Linux too |
| 4 | Only the message: off Windows, `spawnGame` says before a windowed boot that the window cannot be hidden there. Off Windows it also refuses a file that is not a program before the spawn, in the words Windows' own refusal gets (Linux starts a process and fails it a moment later). Opening it minimised is not built (it is engine work in `src/engine/rig_window.rs`) | none (a log line) |
| 5 | `.github/workflows/verify.yml`: `tooling-linux` runs every node rig test on Linux; `native-linux` builds the native app (debug) and runs `--sim soak --days 1` and `--sim walk` over the storeys route | the jobs themselves |

Still Windows-only, and why: the hand-off search (only Windows keeps an orphan's parent id, and
the hand-off itself is a Windows delivery's), the build slot's job-object tests (a tiny link can
end between two /proc looks), the share guard (`scripts/pc-share-guard.ps1`), the DXC pair, the
operator's delivery scripts (`archive-build.js`, `check-delivery.js`).

## Not changed by this

The operator's machine rules (CLAUDE.md: one game at a time, his share of the computer, no focus taken)
stay as they are on Windows; this only stops the same tools from failing or doing nothing elsewhere.
