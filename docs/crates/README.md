# The crate cards

One page for each of the ten crates the crate split plans (`docs/design/crate-split.md`), so an
agent working inside one crate reads one page instead of catching up on the whole repo. The
operator, 2026-10-30: "Sounds like something we gotta do."

Five of them are Cargo packages of their own, members of the workspace the root `Cargo.toml`
declares: `crates/humanity-core/` and `crates/humanity-protocol/` since rung 1 of the split
(2026-11-01), `crates/humanity-ship/`, `crates/humanity-terrain/` and `crates/humanity-systems/`
since rung 2 (2026-11-02). The other five are still parts of the root package, `src/`, and
`scripts/crate-layers.json` says which crate each part will go to (a cut crate gives its `dir`
there instead). These cards are documents, and they live here, in `docs/crates/`, whatever
happens to the code.

## The cards

<!-- generated: crate-cards.js; `just code-map` and `just brief` rewrite this block from scripts/crate-layers.json and the code, so never edit it by hand -->
| Card | Lines | May use | What it is for | Requests |
|---|---:|---|---|---:|
| [humanity-core](humanity-core.md) | 17,891 | nothing | The bottom of the stack: the small pieces every other crate leans on. | 0 |
| [humanity-protocol](humanity-protocol.md) | 13,882 | core | What the app and the server must agree on, byte for byte. | 0 |
| [humanity-ship](humanity-ship.md) | 33,192 | core | The home and the ship as places a person stands in. | 0 |
| [humanity-terrain](humanity-terrain.md) | 25,607 | core | Planets as shapes and numbers. | 0 |
| [humanity-systems](humanity-systems.md) | 79,786 | core, ship, terrain | The simulation: what happens in the world, and why. | 0 |
| [humanity-music](humanity-music.md) | 7,020 | core | The game's own music, composed live, and the ship's hum (docs/design/music.md). | 0 |
| [humanity-animation](humanity-animation.md) | 2,238 | core | Character animation, as plain Rust (docs/design/character-animation.md; rungs 0 and 1 built 2026-11-08). | 0 |
| [humanity-relay](humanity-relay.md) | 119,755 | core, protocol, ship, systems | The server: the relay the VPS runs as `HumanityOS --headless`. | 0 |
| [humanity-net](humanity-net.md) | 31,633 | core, protocol | The app's side of the network: the WebSocket client to a server (`ws_client`, `client`), the shared world's state as it arrives (`sync`, `protocol`), sealed direct messages and their history on this device (`dm_pq`, `dm_store`), P2P groups (`group_e2ee`), voice and calls (`voice`, `webrtc`, `call_relay`), device-to-device files (`file_link`, `file_offer`, `file_transfer`), the live stream and the live weather, and the recovery-phrase identity (`identity`). | 0 |
| [humanity-renderer](humanity-renderer.md) | 66,867 | core, terrain | The GPU side: everything that uploads and draws. | 0 |
| [humanity-gui](humanity-gui.md) | 217,841 | core, protocol, ship, terrain, systems, relay, net, renderer, music | Everything a person sees and touches in the desktop app's pages: every page and widget (`gui/`, with `GuiState`, the state every page reads, in `gui/mod.rs`), the settings that save them (`config`, which also holds the identity vault), notifications, and the pages' backends: the Games and Apps readers (`games`, `software`), the media player (`media`), the readable web (`web_reader`) and the updater that Settings > Updates drives, with `release_update`, the binary's release-signing commands and the launcher's check of a local exe against its signature file (the pure signature check the updater verifies every download by moved down to the protocol's `release_signature` on 2026-11-02, so the server can check a signed release too: docs/design/front-door.md, 3.1). | 0 |
| [humanity-engine](humanity-engine.md) | 98,922 | core, protocol, ship, terrain, systems, relay, net, renderer, gui, music, animation | THE FRAME: the main loop, and everything that wires the other crates into one running app. | 0 |
<!-- end generated -->

Crates are named short inside the cards: `core` is `humanity-core`, `gui` is `humanity-gui`.
"Requests" counts the lines waiting in each card's mailbox.

## What a card holds, and why

Every card has the same six sections, in this order:

1. **What it is for**: two or three plain sentences a newcomer understands.
2. **At a glance (generated)**: the paths the crate owns and the gate each compiles under, its
   size, the command that runs exactly its lib tests, the crates it may use and the crates that
   use it, the references that still point the wrong way (in code and in tests), and its de facto
   interface: the items other crates actually reference, most first.
3. **Rules**: the rules that bind work in the crate, each with its source.
4. **How to test it**: the recipes, test files and rigs that cover it.
5. **Open work**: from `docs/PRIORITIES.md`, the open entries of `docs/BUGS.md` and the backlog in
   `docs/design/crate-split.md`, each with its source.
6. **Requests from other crates**: the crate's mailbox. A session in another crate that needs
   something here (an item made `pub`, a function moved down, a behaviour changed) adds one line;
   the session that does the work deletes it in the same commit.

## How the generated parts stay true

`node scripts/crate-cards.js` rewrites ONLY the blocks between the
`<!-- generated: crate-cards.js ... -->` and `<!-- end generated -->` markers, in each card and in
the table above, from `scripts/crate-layers.json` (the plan's one source) and the counting
`scripts/crate-graph.js` does for the ratchet (`node scripts/check-crate-layers.js`). Everything
outside the markers is hand-written and never touched. `just code-map` runs it, and so does
`just brief` at the start of every session, so a card is never more than a session old.

`node scripts/crate-cards.js --check` runs in `just lints`: every crate in the plan has a card, no
card names a crate the plan does not, each card has the six headings in order with words under
each, one generated block under "At a glance", and its requests in the mailbox's form. It never
compares the numbers, because every code edit moves them. When the plan gains a crate,
`node scripts/crate-cards.js --new humanity-<name>` writes a skeleton whose "(to write)" parts the
check refuses until they are written. `node scripts/crate-cards.js --interface <crate>` lists a
crate's whole interface, past the card's table.

How the counting works (`scripts/crate-graph.js`): every `crate::` and `super::` path, mapped to a
crate by the plan's most specific path; in the binary's root (main.rs), every `humanity_engine::`
path, read as the library's `crate::` from the engine. A `use crate::a::{x, y}` group counts once,
as `a` (so a count understates the names); a `use crate::{a, b}` group counts each part. A
path to one of the crate root's own items (a function, static or re-export of lib.rs, such as
`crate::data_dir()`) is a reference into the crate that owns lib.rs. A file named by a
`#[path = "..."]` belongs to the module that declares it. Test code is read the same way, apart
(the test files, and the test-gated items of the others), and the ratchet
(`node scripts/check-crate-layers.js`, in `just lints`) holds code and test code to the plan file
by file: crate-layers.json's `budgets` and `testBudgets` list the only files allowed any wrong-way
reference, with their counts. `node scripts/crate-graph.js --tests` lists the test code's, with lines.
