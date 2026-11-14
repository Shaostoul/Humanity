# Booting the game from a script

**Who reads this:** anyone about to boot HumanityOS: a rig, `just launch-bg`, a relay run by hand,
a screenshot of the running game. `CLAUDE.md` keeps these rules in brief ("The operator's
machine"); this page holds their history and the mechanics behind them. Which runtime check fits a
change: the `game-checks` skill (`.claude/skills/game-checks/SKILL.md`).

Moved from `CLAUDE.md` on 2026-10-23 with the wording unchanged.

## Never steal the operator's focus (MANDATORY)

The operator has ONE computer and ONE screen. Every time an agent boots HumanityOS
and the window takes focus, he is pulled out of whatever he was doing and has to
click back manually.

**As of v0.1081 the default is INVERTED: focus requires PROOF of a human launch.**
A script-launched instance (parent process is bash/node/powershell/cmd/cargo -- 
anything but explorer.exe) opens in the BACKGROUND automatically, no env var
needed. The three earlier layers (env var v0.828, create-visible-inactive v0.1069,
no_focus.txt marker v0.1079) all made background the special case someone had to
remember to request, and each left a hole (forgotten env vars, bare exes copied to
fresh dirs, worktree target/ dirs with no marker). Full decision order and the
FFI parent-process check: `src/engine/launch_focus.rs`.

**A background instance is also SILENT (2026-09-27).** The same decision
(`launch_in_background()`) keeps it from opening an audio device, because
agent boots used to sound over the operator's film or game (the workshop
screen's 440 Hz demo tone, on every boot). And every in-world video screen
now starts muted until the player presses Unmute, so the operator's own
boots are quiet too. A person who clicks into a script-launched window (a launcher, a
.bat, a terminal) turns its sound on: only an instance nobody touches, an
agent's rig, stays silent.

What still matters in practice:

```bash
just probe-sweep --only <vantage>    # verification: background (belt + braces: env + marker + inversion)
just launch-bg                       # plain boot, opens behind, no cursor grab
target/release/HumanityOS.exe        # even a bare hand-rolled boot is now background (but ungated: verify through launch-bg or a rig)
```

`just play` and `just launch` DO take focus: they set `HUMANITY_TAKE_FOCUS=1`, the
operator's explicit opt-in. They are not for agents -- and agents must NEVER set
`HUMANITY_TAKE_FOCUS` themselves; that env var is the operator's proof-of-human,
and a grep for it in scripts/agent prompts should only ever hit the two Justfile
recipes, the updater's restart script (which propagates the current instance's
state), and the docs that state the rule (CLAUDE.md and this page; `tests/focus_optin_lint.rs` pins the list). A user double-clicking the downloaded exe gets a
focused window because explorer.exe is the parent -- no setup needed.

## One GPU, one instance

**ONE GPU, same rule as one screen (operator, 2026-07-31): at most ONE booted
HumanityOS instance machine-wide during working hours.** Three agent rigs ran
concurrently and the operator's whole computer bogged down -- each instance is a
full GPU renderer even in the background. Before booting any rig, check
`tasklist //FI "IMAGENAME eq HumanityOS.exe"`; if an instance is already running,
CLOSE IT rather than booting beside it. **Operator, 2026-10-13: "assume any
HumanityOS window presently open on my PC you can close while you're
developing"**, his own game included. Close it gracefully first (`taskkill //PID
<pid>` without `//F`, so the game gets its close and saves) and force it only if
it is still running about 15 s later. The exception is this session's own rigs:
wait for those to finish. Orchestrators: do not dispatch multiple rig-booting
agents in parallel -- stagger them so their sweeps serialize. Read-only/code
work parallelizes fine; GPU boots do not. **That exact-name filter misses the
operator's own game when he starts it with `just launch` or `just play`:**
those boot the newest ARCHIVE, named `v<version>_HumanityOS.exe` (found
2026-10-04). Check with `tasklist | grep -i humanityos` instead, which matches
both.

*What the rigs wait for (2026-10-23):* other games and rigs, the operator's programs, the boot
lock and free physical memory, not builds (`docs/ai/builds.md`); `--wait-builds` for a run that
measures timings.

*Resolution (2026-10-23):* use `tasklist | grep -i humanityos`, the form at the end of that
paragraph. The exact-name `tasklist //FI` filter in its first lines misses the archive the
operator launches, so it is superseded.

## The operator's share of his PC

**The operator keeps a share of his PC (2026-10-22): "1 CPU core and 1 GPU core to whatever I
want to do on my PC ... the rest for you but, enough for me to still comfortably use my PC."**
`scripts/pc-share-guard.ps1` (`just share-guard`; `just install-share-guard` makes it start at
every logon) keeps every build, test program and rig copy of HumanityOS at below-normal
priority and off the last physical CPU core. The GPU cannot be split, so while one of his
GPU-heavy programs runs (`scripts/lib/operator-apps.json`, starting with Helldivers 2; add his
others there; his own HumanityOS counts too, any copy outside `.probe-rig`, a worktree, `target` or
temp, so a behaviour rig no longer needs to close his game) a rig whose checks are about behaviour
boots its game in LOW IMPACT beside it (`HUMANITY_LOW_IMPACT`, `src/engine/low_impact.rs`:
minimum graphics, a frame cap, a small window, never saved), and a rig that judges the picture or
times frames waits for it and refuses low impact (operator, 2026-10-22: "I'd rather development
testing continue if possible ... minimum graphics vs max"). Which rig is which:
`scripts/lib/rig-needs.js` (a new rig goes there or `just rig-tests` fails; `--low-impact` /
`--full-graphics` override). A low-impact `verify-runtime` PASS does not cover the passes it
turned off: before a release, run it at full graphics. Never start work that defeats any of this
(no above-normal priorities, no pinning a build to every core, never set `HUMANITY_LOW_IMPACT` by
hand for a picture rig).

## No Windows Firewall prompts from dev work

**No Windows Firewall prompts from dev work either (operator, 2026-10-03).** A
program that listens on every network address (0.0.0.0) gets a "Windows Defender
Firewall has blocked some features" prompt once per exe PATH, and dev work makes
new paths constantly (temp relay copies, worktree builds, test binaries): 274
HumanityOS rules piled up on his PC, each one an interruption. So every relay or
listener started for development binds loopback: the rigs' throwaway relays set
`BIND_ADDRESS=127.0.0.1` and refuse a relay that listens anywhere else, and **a
relay you run by hand (`HumanityOS --headless` from a scratch or temp folder)
needs `BIND_ADDRESS=127.0.0.1` in its environment**. Tests bind 127.0.0.1. The
relay's default stays 0.0.0.0 for the LAN-node and self-hosting setups. Never
"fix" a prompt by changing firewall settings: that is his machine's policy.

## How the engine keeps a boot in the background

The engine honours background in two places, and both are needed (v0.1069): the
window is created with `with_active(false)` AND created already-visible, because
the normal path's later `set_visible(true)` goes through `ShowWindow(SW_SHOW)` on
Windows, which activates the window and defeats `with_active(false)` a few lines
after it was set. That was a real bug: every probe-rig boot stole focus despite
asking not to.

## A script's boot is hidden from the screen (2026-11-14)

The operator, 2026-11-14: "It appears the game often loads up then cycles through a bunch of static
scenes then turns off. What's happening? Is a test being ran or something?" Those were agents'
rigs. A background boot never took the focus, but it was still a full-screen window at the bottom
of the Z order and a button on his taskbar, seen whenever he uncovered the desktop.

**Now a developer's quiet boot is invisible to him while it still draws every frame**
(`src/engine/rig_window.rs`). A boot that is in the background AND was asked to be quiet
(`HUMANITY_NO_FOCUS` or the `no_focus.txt` marker: every rig, because `spawnGame` in
`scripts/lib/game-launch.js` always sets it, and `just launch-bg`) is, before its first show:

1. **Cloaked** by the Desktop Window Manager. Microsoft's
   [DWMWINDOWATTRIBUTE](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute)
   reference (page dated 2024-01-21, read 2026-11-14), `DWMWA_CLOAK`: "Cloaks the window such that
   it is not visible to the user. The window is still composed by DWM."
2. **A tool window** (`WS_EX_TOOLWINDOW` set, `WS_EX_APPWINDOW`, which winit puts on every window,
   cleared), because a cloaked window KEEPS its taskbar button: measured on this PC (Windows 10
   19045) with a cloaked test window, which still added "<title> - 1 running window" to the
   taskbar, while cloaked plus tool window added none. Microsoft's
   [Extended Window Styles](https://learn.microsoft.com/en-us/windows/win32/winmsg/extended-window-styles)
   (dated 2025-07-14), `WS_EX_TOOLWINDOW`: "A tool window does not appear in the taskbar or in the
   dialog that appears when the user presses ALT+TAB." The style is set while the window is still
   hidden, as [The Taskbar](https://learn.microsoft.com/en-us/windows/win32/shell/taskbar) page asks:
   "you must hide the window first (by calling ShowWindow with SW_HIDE), change the window style,
   and then show the window."

The client keeps exactly the size a shown boot gets (the frame is measured with the ordinary style,
then the client is framed with the thinner tool frame), so picture rigs photograph the same pixels.

| Option | Frame still presents at full rate | Dev screenshots still come out | Taskbar button | In Alt+Tab | Verdict |
|---|---|---|---|---|---|
| Behind the other windows (v0.1095 until this change) | yes | yes | yes | yes | seen whenever the desktop is uncovered |
| Minimized | no: a zero-size surface, wgpu stops drawing (the old `src/lib.rs` note: minimizing "can suspend the swapchain and break the offscreen screenshot capture") | no | yes | yes | rejected |
| Off every monitor | not measured (Windows may treat it as occluded) | not measured | yes | yes | rejected: still on the taskbar and in Alt+Tab |
| `ITaskbarList::DeleteTab` (winit `set_skip_taskbar`) | yes (style untouched) | yes | no, until the shell adds it back at the next show | yes | rejected: half a fix |
| Layered window at alpha 0 | not measured (a layered window changes how a swapchain presents) | not measured | yes | yes | rejected |
| DWM cloak alone | yes ("still composed by DWM") | yes | **yes (measured)** | not measured | rejected: the button stays |
| **DWM cloak + tool window** | **yes (measured, below)** | **yes (measured, below)** | **no (measured)** | **no (documented)** | **chosen** |

**The proof (2026-11-14, one boot, `home-racks-night`, hidden / visible / hidden in the same run
through the dev request below, 15 one-second frame-cost samples per phase with
`HUMANITY_FRAME_COSTS=1`):**

| Phase | Window, as Windows reports it | Taskbar buttons naming it | Frame ms (median) | GPU ms (median) | CPU spans ms (median) |
|---|---|---|---|---|---|
| A1 hidden | cloaked=1, ex_style 0x00000190 (tool window), not foreground | 0 (of 2 window buttons seen) | 42.0 | 18.80 | 31.7 |
| B visible (behind, never activated) | cloaked=0, ex_style 0x00040110 (app window), not foreground | 1 (of 3) | 41.0 | 18.97 | 32.5 |
| A2 hidden | cloaked=1, ex_style 0x00000190, not foreground | 0 (of 2) | 38.5 | 18.82 | 32.9 |

The client was 2560x1387 in every phase, the same as a shown boot. Every frame-cost difference
between the phases is smaller than the gap between the two hidden phases, so it is noise. The
captures are NOT pixel-identical, but the scene itself changes between captures in one run: A1
against B differs by a mean of 0.60/255 (0.98% of pixels by more than 8), while A1 against A2,
both hidden, differs by 1.99/255 (4.4%). The window state adds no difference beyond the scene's own
drift, and the pictures look the same. The B phase's taskbar button (1 of 3) is the read's
in-place positive control: the same read that finds no button for a hidden boot finds the button
of a shown one.

**The dev request behind it:** drop `debug/window_request.json` as `{"visible": true}` or
`{"visible": false}` in a hidden boot's folder and the game shows its window the ordinary
background way (the bottom of the Z order, never activated, never on top) or hides it again,
answering in `debug/window_done.json` with what the window then reports (`src/engine/rig_window.rs`
`poll_request`). Only a hidden script boot answers it, so it can never show or move a person's own
window. Keep a visible phase as short as its measurement needs.

**Who is never hidden:** a human launch (`just play`, `just launch`, a double-click) is foreground,
and a background launch nobody asked to be quiet (a person's launcher, Steam, a terminal) stays
visible behind, as before: hiding it would leave a person with a game running and nothing to see.

**The escape hatch: `HUMANITY_RIG_VISIBLE=1`** in a rig's environment keeps its window visible
behind the others, for a rig that genuinely needs one on screen (one testing the taskbar button,
Alt+Tab, or the window frame a person sees). It never brings the window forward or gives it the
focus: that stays the operator's alone (`tests/focus_optin_lint.rs`; the focus decision never
reads `HUMANITY_RIG_VISIBLE`, pinned by a test in `rig_window.rs`).

**Checked on every probe-sweep boot:** once the world is up, `scripts/lib/window-state.js` asks the
window itself from outside the game (`DwmGetWindowAttribute` `DWMWA_CLOAKED`, `GWL_EXSTYLE`, the
client size, and the taskbar buttons that name it, through UI Automation, with a count of all
window buttons as the read's positive control) and the sweep exits 2 when a boot meant to be hidden
is not. `node scripts/lib/window-state.js <pid> --taskbar` asks it by hand. Experiments on window
behaviour create their windows already cloaked and as tool windows, and prove properties by
reading the window's state, never by showing a window on his screen.

**Every boot is counted (the boot ledger).** Each boot a developer's script starts appends one line
to `%LOCALAPPDATA%\HumanityOS\dev\boots.jsonl` (when, which checkout, which script and rig, what
for, how long, how it ended, hidden or not), written by the launcher (`spawnGame`; the dev MCP's
close finishes a `game_boot` line; the game itself for a quiet boot no launcher recorded, such as
`just launch-bg`). It is a log, never read back by the game. `node scripts/boot-ledger.js` prints
the last day per worktree and per rig, with the repeats.

## No tray icon from a script's boot (2026-10-31)

An instance a script started (`launch_focus::launch_in_background`) makes no
notification-area icon, never flashes its taskbar button for a notice, and never
answers a click on an operating-system notice (`src/engine/tray.rs` `allowed`):
a force-closed process leaves its tray icon behind until the pointer passes over
it, so agents' boots would pile icons into the operator's tray, and a rig must not
take a click on the operator's own notice. A rig that tests the tray or the flash
sets `HUMANITY_TRAY=1` for its game; that alone serves NO clicks on notices (the
Windows notice activator is one class id for every copy, so a rig serving it beside
the operator's own game could take his clicks): only a rig that also sets
`HUMANITY_TRAY_CLICKS=1` serves them, and it runs only when his game is not up
(src/engine/tray.rs `serves_clicks`; the dev request's `click` action needs neither,
it calls the same open path directly). `just verify-tray` is the committed rig for
the hidden-window chain (HUMANITY_TRAY=1, never HUMANITY_TRAY_CLICKS). A rig drives
the tray through `debug/tray_request.json` (engine/tray.rs: `close`, `voice_close`,
`menu`, `icon`, `click`, `notice`, `setting`, `state`; answered in
`debug/tray_done.json`), and closes it GRACEFULLY so its icon goes with it. Never set
a real voice room or call in a rig: `voice_close` sets stand-ins and closes in the
same step, so no frame ever starts a voice session (that would open the operator's
microphone). A close posted by another program (`taskkill` without `/F`) always
quits, hidden or shown, even with "closing keeps HumanityOS running in the tray" on,
so the graceful close still saves (`src/engine/tray_window.rs`).

## A quick manual boot

A quick boot after a release build, world entry, and where a boot failure is written:
`docs/ai/renderer.md`, "Verify by booting and entering the world".

## Seeing the live 3D game

**Seeing the live 3D game (not just egui pages).** `just snapshots`/`just snapshot <name>` above render 2D egui UI headlessly, useful for pages, but they can't show the actual running 3D scene (lighting, placed geometry, the mothership). For that, drop `debug/screenshot_request.json` (any content) into the repo root while `HumanityOS` is running; the render loop notices it within one frame (an EMPTY file a quarter second later, in case its text is still being written: `REQUEST_EMPTY_GRACE` in `src/engine/ipc.rs`, shared by every dev request), writes a real capture of the current viewport to `debug/screenshot_N.png` (N monotonic per session), deletes the request file, and writes `debug/screenshot_done.json` as `{"ok":true,"path":...}` or `{"ok":false,"error":...}` if the backend's swapchain doesn't support the capture. Read the PNG with the Read tool like any other image. (v0.639)

**Or drive it as tools (the dev MCP, 2026-10-22).** `scripts/mcp-server.js` gives an MCP client (Claude Code, through the project's `.mcp.json`) the same IPC as tools: `game_boot` boots a rig sandbox with the endpoint on, then `game_screenshot` returns the picture inline with its PNG path, and `game_player_state`, `game_camera`, `game_showcase`, `game_logs` and the rest read and move the game; each is a thin call into the same request funnel the file drops use. `just mcp-smoke --boot --close` proves it end to end. Guide and safety rules: `docs/ai/mcp.md`.
