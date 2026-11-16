# Developing on macOS and Linux: what the dev tooling needs (feasibility, 2026-11-16)

A contributor on macOS could not run the project (2026-11-16). Fixed the same day: the Justfile's
shell and PATH (1dcd0e70b), five hardcoded personal paths (0ea86a5ad), and a backslash-path test.
The operator then asked whether the rig and build-slot tooling can work there too: "Let's find out if
we can do this". This is the answer, from reading the code; nothing here is built yet.

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

## Not changed by this

The operator's machine rules (CLAUDE.md: one game at a time, his share of the computer, no focus taken)
stay as they are on Windows; this only stops the same tools from failing or doing nothing elsewhere.
