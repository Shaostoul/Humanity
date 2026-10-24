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

## A quick manual boot

A quick boot after a release build, world entry, and where a boot failure is written:
`docs/ai/renderer.md`, "Verify by booting and entering the world".

## Seeing the live 3D game

**Seeing the live 3D game (not just egui pages).** `just snapshots`/`just snapshot <name>` above render 2D egui UI headlessly, useful for pages, but they can't show the actual running 3D scene (lighting, placed geometry, the mothership). For that, drop `debug/screenshot_request.json` (any content) into the repo root while `HumanityOS` is running; the render loop notices it within one frame (an EMPTY file a quarter second later, in case its text is still being written: `REQUEST_EMPTY_GRACE` in `src/engine/ipc.rs`, shared by every dev request), writes a real capture of the current viewport to `debug/screenshot_N.png` (N monotonic per session), deletes the request file, and writes `debug/screenshot_done.json` as `{"ok":true,"path":...}` or `{"ok":false,"error":...}` if the backend's swapchain doesn't support the capture. Read the PNG with the Read tool like any other image. (v0.639)

**Or drive it as tools (the dev MCP, 2026-10-22).** `scripts/mcp-server.js` gives an MCP client (Claude Code, through the project's `.mcp.json`) the same IPC as tools: `game_boot` boots a rig sandbox with the endpoint on, then `game_screenshot` returns the picture inline with its PNG path, and `game_player_state`, `game_camera`, `game_showcase`, `game_logs` and the rest read and move the game; each is a thin call into the same request funnel the file drops use. `just mcp-smoke --boot --close` proves it end to end. Guide and safety rules: `docs/ai/mcp.md`.
