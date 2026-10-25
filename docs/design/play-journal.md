# The play journal: what happened while you played, readable by an AI

*Planned 2026-10-24 (PRIORITIES TIER 0, item 5). Permanent dev tooling once built: never trim it
as cleanup (CLAUDE.md, "Forever development").*

## Why

When the operator plays and something goes wrong (a fall through a floor, a frame that stalls, a
message that never arrives, a page that will not open), the report today is his words and a
screenshot, and the session that fixes it has to rebuild the moment from `run.log`. The game
already knows much of what happened: it logs warnings, it measures every frame, it sees every
network frame and every world event. The play journal keeps those, by kind, small and local, and
lets him press one key to say "this moment": the game writes a mark and takes a screenshot, and a
Claude session reads "everything of kind X since mark 12" through the dev MCP, with the picture,
without him describing it.

## What it is

- **Events by kind**, one small file each, in a `journal` folder beside the logs
  (`crate::storage::portable_logs_dir()`'s parent, so the operator's install and every rig's
  sandbox each have their own): `journal/<kind>.jsonl`, one JSON line per event:
  `{"t": <unix ms>, "frame": <n>, "what": "<short words>", ...fields}`.
- **Kinds** (each a row in `data/dev/journal_kinds.json`, infinite-of-x: a new kind is a data row
  and a call, never a new file format):

  | Kind | What goes in | From |
  |---|---|---|
  | `log` | every warning and error line (`log::warn!` and above), with its module | the logger |
  | `panic` | each panic's message and place (the crash handler's text) | the panic hook |
  | `frame` | a frame over its budget: total ms, the slowest pass and stage | the Performance page's store |
  | `net` | connect, disconnect, a refused sign-in, `chat_refused`, `reach_refused`, a send that failed | the frame handlers |
  | `world` | world entered, storey or room changed, a teleport, a fall, death, a save written or loaded | the engine frame |
  | `page` | a page opened or closed | the GUI router |
  | `mark` | each mark: its number, the screenshot's file, what the HUD names (room, storey) | the Mark key |

- **Never in it** (privacy, safe by default): the words of any message, a key, a token, a
  recovery phrase, a file's contents, anyone else's name. It is local to the device, never sent,
  and says so on the Dev page. `net` events name the server and the kind of frame, never what it
  carried.
- **Capped**: each kind's file holds at most 256 KB; past that the oldest half is dropped (the
  newest is what a report is about). The `mark` screenshots keep the last 30. No file grows
  without bound, so it can stay on for every session.
- **Mark this moment**: a key (`GameAction::MarkMoment`, default F8, free on 2026-10-24 (F9 is dev flight), rebindable in Settings >
  Keybinds like every action) and a button on the Dev page. It writes a `mark` event, saves a
  screenshot to `journal/marks/<n>.png` through the existing screenshot path, and shows a short
  note on screen ("Marked #12. A Claude session can read what happened around it."). Marks number
  up from 1 per journal folder.

## Reading it

- **The MCP tool `dev_play_journal`** (bridge-side, `scripts/mcp-server.js`, so it needs no game
  running and reads a closed game's journal too): `{kind?, since_mark?, last?, folder?}` returns
  the events of that kind (all kinds when left out) after mark N (or the last N events), oldest
  first, and the mark's screenshot path. `dev_play_marks` lists the marks with their time and
  place. The folder defaults to the operator's own install; a rig's sandbox can be named.
- **In the app (GUI-first)**: Dev page > Journal: each kind's count and latest event, the marks
  with their thumbnails, Mark this moment, Open the folder, and Clear (with a confirm). The same
  words a session reads.

## Cost

- Per frame: an event is pushed to an in-memory queue (a `Vec` push under a mutex, only when an
  event happens); nothing is written on the render thread. A writer thread appends the queue to the
  files once a second. A frame with no events costs nothing; the `frame` kind compares the frame's
  total against its budget, a subtraction the Performance store already does.
- Disk: at most 7 kinds x 256 KB + 30 screenshots.
- Network: none, ever.

## Rungs

1. BUILT 2026-10-24, and seen working end to end the same day: the release exe booted in its
   rig sandbox (verify-runtime --keep-open), `{"action": "mark"}` dropped in, and `marks/1.png` (the
   frame, Earth and the HUD, 0.8 MB), the mark's line, and 24 `log` events (the sandbox's
   deliberately unreachable server) read back through `dev_play_marks` and `dev_play_journal`.
   **The store and the Mark key**: `src/engine/play_journal.rs` (the queue, the writer thread, the
   caps, the kinds read from `data/dev/journal_kinds.json`), the `log` and `panic` kinds (the
   logger and the panic hook feed it), `GameAction::MarkMoment` with its screenshot, and the
   bridge's `dev_play_journal` / `dev_play_marks`. Tests: the cap drops the oldest half; a mark's
   number survives a restart; nothing on the never-in-it list can be written (the event type has
   no field for it); the bridge reads a fixture journal.
2. BUILT 2026-10-24: Dev > Journal (`src/gui/pages/dev_journal.rs`): each kind's count and newest
   event, the marks newest first with Open picture, Mark this moment, Open the folder, Clear the
   journal (asking first); the files read on a worker thread every two seconds while it is on
   screen. The website's Dev Workbench lists Journal as the desktop app's. **The Dev page's Journal panel** (and its web mirror only if the website gets a Dev page of its
   own: the journal is the desktop app's).
3. **`frame`, `net`, `world`, `page`**, each a call at the place that already knows the event.
4. **Rigs write marks too**: a rig's verdict names the marks its vantages made, so a failed rig run
   points at the exact moment with its picture.
