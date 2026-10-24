# The dev MCP: drive the game and the dev tools as tools

*Built 2026-10-22. Permanent dev tooling: never trim it as cleanup.*

The running game serves the [Model Context Protocol](https://modelcontextprotocol.io) on
127.0.0.1, so an AI session can look at it, move in it and read it as tools, instead of
dropping JSON files into `debug/` and polling for answers. A small bridge,
`scripts/mcp-server.js`, is what Claude Code talks to: it forwards the game's tools to the
running game and adds the tools that need no game at all (the rigs, the lib tests, the
lints, the journal).

```
Claude Code ──stdio──> scripts/mcp-server.js ──HTTP, 127.0.0.1, token──> the game (src/engine/mcp)
                         │                                                  │
                         ├─ dev_* tools: rigs, lib tests, lints, journal    └─ each tool hands its request to the dev IPC's
                         └─ game_boot: the rigs' launcher (probe-sweep)        one request funnel, the same one a dropped
                                                                               file goes through (engine/ipc.rs)
```

## Starting it

**For Claude Code**, register the bridge in the project's `.mcp.json` (project-scoped MCP
configuration; Claude Code asks once before it uses a project server):

```json
{
  "mcpServers": {
    "humanityos": {
      "command": "node",
      "args": ["scripts/mcp-server.js"]
    }
  }
}
```

The tools then appear as `mcp__humanityos__game_screenshot` and so on. The bridge starts
with the session, needs no game, and never boots one by itself.

**Get a game up** with the `game_boot` tool. It boots this tree's release build
(`target/release/HumanityOS.exe`, so build first after a source change:
`cargo build --features native --release`) through the rigs' own launcher, probe-sweep:
the freshness gate, a wait for other games and rigs, the machine-wide boot lock, a portable
sandbox in `.probe-rig/mcp/`, background and silent, never taking focus. It enters the world,
parks at a vantage (default `home-overview-noon`) and leaves the game up. One to a few
minutes. `game_close` closes it; so does the bridge exiting, and so does 20 minutes with no
game tool call (`HUMANITY_MCP_IDLE_MIN`), so a forgotten game never holds the one GPU that
every other agent's rig waits for.

**By hand**, any rig boot with `HUMANITY_DEV_MCP=1` in its environment starts the endpoint
(for example `HUMANITY_DEV_MCP=1 node scripts/probe-sweep.js --only home-clock-noon
--keep-open --width 0`). The game writes `debug/mcp_endpoint.json` in its folder: the URL,
the port the system picked, the token, its pid and version.

**Prove it end to end**: `just mcp-smoke --boot --close` starts the bridge exactly as Claude
Code does, boots a game, takes a screenshot (printing the PNG's path), reads the player's
state, and closes the game.

## The tools

### The game's (served by the running game; listed from `src/engine/mcp/tools.json`)

| Tool | What it does | Through which dev IPC request |
|---|---|---|
| `game_screenshot` | The current frame, inline (JPEG, long edge 1568 by default) and the full PNG's path; `width`+`height` for a hi-res offscreen render | `screenshot_request` |
| `game_player_state` | Camera position and look, world loaded, aboard, the storey and the room the HUD names, health, food and water, what is carried, credits, which save; `full` for the whole reading | `remote_players_request` with `{"seconds":0}` |
| `game_camera` | Park the camera (over a body, aboard the home, facing a screen) and return where it really went | `camera_request` |
| `game_console` | One build-console line on the live home, this session only; returns what the console said | showcase `console` |
| `game_showcase` | Any showcase verbs and pins (time, weather, hud, fov, wind, walk, face, hold, ...) | `showcase_request` |
| `game_open_page` | Open a page by id (`homes`, `maps`, `dev`, `none`, ...) | showcase `page` |
| `game_map` | The Maps page's verbs, returning its report | showcase `map*`, `debug/map_done.json` |
| `game_home_state` | The home's power, water and air as the HUD shows them, the storey and elevators, the room GI table | the recorder's `home_systems`, `debug/room_gi.json` |
| `game_frame_costs` | Frame time, GPU ms per pass, CPU ms per stage, VRAM and RAM | the Performance page's store |
| `game_logs` | The tail of run.log (optionally filtered), or every PANIC block and the end of crash.log | the log files |
| `game_wait` | Wait for frames, seconds, the world being loaded, or a log line | frame counter, recorder, log |
| `game_request` | The other read-only dumps: remote_players recordings, door_points, cloudmap, cloud_profile_dump | those requests |

### The bridge's own

| Tool | What it does |
|---|---|
| `game_boot`, `game_status`, `game_close` | Start a game through the rigs' launcher, say what is running, close it |
| `game_showcase_reset` | Release every sticky pin, the same list the rigs release before each vantage (`scripts/lib/showcase-pins.js`) |
| `game_goto_vantage` | Replay a vantage of `tests/visual/vantages.json` (pins, park, settle, post and final showcase), then a screenshot. Without the sweep's judges: `dev_rig_run probe-sweep` gives a judged capture |
| `dev_rig_vantages`, `dev_rig_list` | The vantages, and the rigs (every `verify-*` rig and probe-sweep, read from the Justfile) |
| `dev_rig_run` | Run a rig to its end and return its verdict (0 passed, 1 refused, 2 failed). Refused while `game_boot`'s game is up |
| `dev_lib_test` | Lib tests by filter, through `scripts/lib-tests.js` (the Windows link that works) |
| `dev_lints` | `just lints` |
| `dev_journal`, `dev_priorities` | The journal's focus and last decisions; the top of TIER 0 |

### Adding a game tool

Two edits: its entry in `src/engine/mcp/tools.json` (name, title, description, input schema,
annotations) and its row in the `HANDLERS` registry in `src/engine/mcp/tools.rs` (the name and a
`fn(&dyn GameLink, &Value) -> Value`). The test `every_listed_tool_has_a_handler_and_back`
holds the two to each other, and the bridge lists the new tool from `tools.json` without a
change. Make it a thin call into the dev IPC (a request through `GameLink::submit`, its done
file read back), so the file IPC and the MCP keep one implementation; if the game has no IPC
request for what the tool reads, add the request (or a field of the recorder's done file) first,
as `home_systems` was. A tool bridge-side (composed from game tools, or needing no game) is a row
in `OWN` in `scripts/mcp-server.js`.

## How it works, and why the IPC and the MCP cannot diverge

Every game tool is a thin call into the dev IPC. The endpoint does not write request
files: it hands the request's text to the IPC's one request funnel,
`take_request_text` (engine/ipc.rs), through `engine/mcp::take_injected`, which the funnel
asks first. The poller takes it the next time it runs, exactly as it would take a dropped
`debug/<name>_request.json`, and the same handler answers with the same done file, which the
tool reads. There is no second implementation of any verb to drift: a fix to a handler fixes
the rigs and the MCP at once, and the rigs keep dropping files as before. The test
`the_tools_name_the_requests_the_game_polls` fails if a request the tools use is renamed in
the game.

A request handed in is taken in some frame; the tool returns once the next frame has begun,
so the handler has finished and its done file is written. Tool calls run one at a time.

## Safety

- **Loopback only.** The listener binds 127.0.0.1 and `bind_loopback` refuses any other
  address before a socket exists (so a dev listener never raises a Windows Firewall prompt
  either). Host and Origin must name this computer (the DNS rebinding guard the
  specification requires), and every request needs the token from the endpoint file, which
  a web page cannot read.
- **Off unless asked, and only in a sandbox.** The endpoint starts only when
  `HUMANITY_DEV_MCP=1` is set AND the game runs portable (`portable.txt` beside the exe: its
  own throwaway identity, config and saves, never the operator's) AND its server is on this
  computer (none, a throwaway relay, or the rigs' dead loopback port). A player never sets the
  variable and an installed game is not portable. With the variable set anywhere else, the
  game logs why it refused and writes the refusal to `debug/mcp_endpoint.json`. If the server
  ever changes to one off this computer, every call is refused.
- **No tool sends a message, signs, spends, changes the identity or reaches a server.** The
  showcase verbs that would are refused by name (`accept_request`, `choose_start`, `pairing`,
  `meal`, `water`, `place`, `take_down`, `drop_link`, `solo`); the rigs that need them run
  against a throwaway relay of their own. The `screen_request` and `ui_request` IPC, which
  click through the app as a person would, are not offered at all. The autopilot request is
  not offered: `game_boot` enters the world through the rig.
- **One game, the boot lock, background, no focus.** A game is only booted through the rigs'
  launcher, which waits its turn behind other games and rigs and takes the machine-wide boot
  lock. The bridge never adds a focus or launch switch of its own: it adds exactly
  `HUMANITY_DEV_MCP=1` and `HUMANITY_FRAME_COSTS=1` (a test holds it to that). A rig is
  refused while `game_boot`'s game is up, and the game is closed when the bridge exits or
  sits idle.

## The protocol, and why not an SDK

The specification has two eras, and both clients exist: **legacy** (2025-11-25 and earlier:
an `initialize` handshake, sessions) and **modern** (2026-07-28: no handshake, the version in
every request's `_meta`, `server/discover`, `resultType` on every result, and on HTTP the
`MCP-Protocol-Version`, `Mcp-Method` and `Mcp-Name` headers, which must agree with the body).
The game's endpoint (`src/engine/mcp/protocol.rs`) and the bridge
(`scripts/lib/mcp-protocol.js`) are both dual-era: each request is answered in the era it is
asked in, an unsupported modern version is refused with -32022 and the supported list, a
header that disagrees with the body with -32020.

The official Rust SDK, [`rmcp`](https://crates.io/crates/rmcp) (modelcontextprotocol/rust-sdk,
**Apache-2.0**, 3.7.0 on 2026-10-22), fits the protocol but not this build, so the small
subset is written here instead, std::net and serde_json only:

1. Its server and Streamable HTTP features add about fifteen crates to the native build
   (schemars 1, rmcp-macros, pastey, uuid, chrono, sse-stream, hyper-util, http-body-util,
   tokio-util, rand 0.10, ...), and the native lib test program is already at the Windows PDB
   link limit (BUG-174) with builds that run out of memory on a busy machine.
2. It is released several times a month across major versions (3.4.1 to 3.7.0 in a month),
   and its minimum Rust (1.88) is exactly this toolchain: churn a permanent dev tool does not
   need.
3. The subset needed is four methods and one transport, pinned by our own tests.
4. The side Claude Code talks to must be a node script anyway (above), so the SDK would only
   have covered the HTTP half.

**Why the bridge is node and not `HumanityOS --mcp`:** Claude Code keeps a stdio server
running for the whole session, and Windows will not let cargo replace an exe that is
running. A bridge that was the game's exe would make every release build of the session
fail.

## Tests

- **Rust** (`just test-lib engine::mcp`, also run by `just verify`): the legacy handshake and
  the modern discovery; the tool list equal to the handlers both ways; headers that disagree
  with the body; unknown tools and methods; round trips against a double of the game's IPC
  (screenshot inline and on disk, player state, console, logs, frame costs); the refused verbs
  sending nothing; every call refused once the server leaves this computer; the switch off
  starting nothing and a real install or a public server refused; a non-loopback bind refused;
  the HTTP gate; a real loopback round trip; the live link going through the real IPC funnel
  and waiting a frame; and the request names checked against the game's pollers.
- **Node** (`node --test scripts/tests/mcp-server.test.js`, in `just rig-tests`): the bridge
  over real stdio in both eras, its tool list, forwarding with the token and headers to a
  double of the endpoint, the pin reset's exact list, no game and a dead game, a refused
  endpoint, `game_boot`'s command and environment, and the dev tools reading the tree.

**Red first.** Each set was run against broken code and failed before it passed (2026-10-22):

- Node, against a bridge whose `callGame` dropped the token, whose `bootCommand` dropped
  `--keep-open`, and whose tool list left out the game's tools: 3 of 8 failed,
  `the fake game saw Authorization undefined`, `args lack --keep-open`, `missing
  game_screenshot, game_player_state, ...`.
- Rust (2026-10-23), against an endpoint with six faults put in on purpose (the protocol
  answering `{}` to everything, `decide` starting anywhere, `bind_loopback` skipping its check
  while still binding 127.0.0.1 only, the funnel's door shut, no verb refused, the
  server-off-this-computer refusal gone): 17 of 19 failed, each at its own assertion. The two
  left green, the HTTP gate and the request names, were then broken on their own (the token not
  checked; `door_points_request.json` renamed on the tools' side only) and failed too: 19 of 19
  red ("engine/ipc.rs no longer names debug/door_point_request.json"). With the faults switched
  off in the same binary all 19 passed but the renamed request's, and with them taken out, all.
- End to end (2026-10-24, `just mcp-smoke --boot --close`): the bridge on stdio listed 24 tools,
  `game_boot` brought the release build up in `.probe-rig/mcp` (endpoint on 127.0.0.1), then
  `game_screenshot` returned a 1568x850 inline JPEG and the 2560x1387 PNG's path, and
  `game_player_state` read the world loaded, the player aboard with health 100 and 56 items.
- And one real defect, caught before any fault was put in: the screenshot's path came back as
  `C:\...\debug/screenshot_7.png` (mixed separators); it is now joined piece by piece.

## What is not done yet

- **The admin console's action registry** (another lane is building list and run endpoints
  on the relay): when they land, the bridge should expose them as two tools that call those
  endpoints on a relay on this computer only, not a copy of the registry.
- **`game_goto_vantage` replays without the sweep's judges** (the station park check, the
  discarded first pass, the capture-time check). Extracting the vantage protocol into one
  module both use would close that.
- **The room GI table needs `HUMANITY_FRAME_COSTS=1` at boot** (game_boot sets it): the
  renderer writes it only then.
