# Night loop plan (2026-07-01, ~8 hours unattended)

> **Read this file FIRST at the start of every wake-up iteration tonight.** The
> conversation may be summarized/compacted between wake-ups; this file is the
> durable source of truth for the mission, the safety rules, and the backlog.
> Also read `docs/PRIORITIES.md` (Active Focus) and
> `data/coordination/orchestrator_state.json` (recent_decisions) for the
> latest state -- update all three every cycle.

## Mission

Operator is asleep for ~8 hours and asked for autonomous feature-completion
work with zero interactive checkpoints. Priority order:

1. **Chat feature completeness** (explicit priority #1, backlog below).
2. **Livestreaming feature verification** (explicit priority #2 -- audit for
   real functional gaps, not just missing code; the code looks structurally
   complete, the ask was to confirm it actually WORKS end to end).
3. Broader stub-completion sweep across the rest of the codebase (backlog
   below) once 1 and 2 are solid.
4. Docs (FEATURES/STATUS/BUGS/ROADMAP/PAGES) kept in sync EVERY cycle, not
   batched to the end.

## Non-negotiable safety rules (read every time, these are why this is safe
## to run unattended)

1. **NEVER use Write on an existing file, ever, for any reason.** Read it,
   then Edit with a targeted diff. Write is ONLY for genuinely new files.
   This is the operator's single biggest fear tonight: a large pre-existing
   `.rs` file getting silently replaced/truncated by a well-meaning rewrite.
   If you catch yourself thinking "it'd be cleaner to just rewrite this
   file," stop -- edit it instead.
2. **Commit after every small verified increment**, not once at the end of
   a big feature. Frequent small commits are the actual safety net -- if
   something goes wrong, git history has fine-grained recovery points. Use
   `git commit -F <tmpfile>` (never bare `-m` for anything beyond one line
   -- see CLAUDE.md's PowerShell quoting gotcha).
3. **Never launch a built exe from a worktree path.** Only ever launch from
   the MAIN repo's own build output (`C:\Humanity\target\release\HumanityOS.exe`
   or the archived `C:\Humanity\vX.Y.Z_HumanityOS.exe`). The operator's
   network-permission rule (only the main exe has it) is about outbound
   connections to the real internet (production relay, STUN/TURN) -- a
   worktree-built exe reaching out to `united-humanity.us` could trigger a
   fresh firewall/permission prompt with nobody there to click it. Building
   in worktrees for code-editing isolation is fine; RUNNING a built exe for
   verification is main-repo-path-only, always.
4. **Never point any test instance at the real production relay
   (`united-humanity.us`) or its STUN/TURN.** Use the local loopback harness
   below instead -- zero internet, zero firewall risk, and it doesn't
   pollute the real `#general` channel / real user accounts with test
   traffic. `HUMANITY_DATA_DIR=<scratch dir>` gives a launched instance its
   own throwaway identity, completely isolated from the operator's real
   `%APPDATA%\HumanityOS\config.json` (their real identity/vault) -- NEVER
   touch that file.
5. **Never run `just clean-worktrees` tonight**, in any form, even the
   hardened version. There is nobody to review "safe to remove" judgment
   calls if something surprising happens. Leave worktrees lying around;
   the operator can clean up in the morning. (The hardened script protects
   against destruction, but "safe" still means "I trust my own judgment
   about what's disposable" -- don't exercise that judgment unsupervised
   for 8 hours.)
6. **Never use AskUserQuestion, or any tool that blocks on interactive
   approval.** Nobody is there to answer. If a planned action would need
   one (e.g. a destructive git operation, a risky Bash pattern the
   permission classifier might gate), don't attempt it -- route around it,
   log why in the journal, and move to the next backlog item instead of
   stalling the whole loop on one blocked action.
7. **No native-GUI interactive testing (clicking buttons in the running
   3D/egui window).** There is no computer-use/input-injection capability
   available for the native window, and it would be inappropriate to use
   even if available while the operator is away from their machine. Verify
   native GUI code via: (a) unit tests, (b) passive screenshot checks (the
   `debug/screenshot_request.json` protocol -- confirms rendering/no crash,
   never confirms interactive behavior), (c) protocol-level tests against
   the local headless relay (see harness below) that don't need the native
   GUI at all. Web pages (`web/chat/`, `web/pages/`) CAN be driven
   interactively via the Claude Preview browser tools against the local
   static preview server (`scripts/preview-server.js`) -- that's fine, it's
   real browser automation, not native-window input injection.
8. **Verify every change with the same discipline as today's recovery
   work**: `cargo check --features native` AND `cargo check --features
   relay --no-default-features` both clean, `cargo test --features native
   --lib` (watch the pass count go up, never down), the 5 standalone lints
   (`emdash_lint theme_token_lint theme_editor_coverage icon_glyph_lint
   engine_wiring_lint`), and `node scripts/check-doc-links.js` after any
   doc edit. Don't skip steps to save time -- the operator explicitly said
   "fast means zero artificial stops, not skipping verification."
9. **If genuinely blocked** (a real ambiguous product decision, not a
   mechanical one) -- pick the most conservative, most reversible option,
   log the fork and your reasoning in the journal under
   `open_questions_for_human`, and move on. Do not stall.

## Local loopback test harness (build this FIRST if it doesn't already exist)

Goal: verify chat/livestream protocol behavior end-to-end with zero
internet, zero firewall risk, zero pollution of the real server.

1. **Local relay**: `PORT=<test port> ./target/release/HumanityOS.exe
   --headless` binds to `0.0.0.0:<port>` but only ever gets hit via
   `127.0.0.1` in this harness -- loopback traffic doesn't cross the actual
   network interface, so no firewall prompt regardless of exe path. Use a
   throwaway SQLite DB (run from a scratch directory, or delete
   `data/relay.db` after -- never touch the real `data/relay.db` if one is
   tracked, check `.gitignore` first).
2. **Lightweight protocol test client -- BUILT, use it**:
   `scripts/ws-test-client.js` (added cycle 2, v0.641.0). Node's built-in
   `WebSocket` (no `ws` package needed, Node 22+), authenticates via the
   `bot_` + `bot_secret`/`API_SECRET` fastpath (`src/relay/relay.rs`
   ~2542) so it never needs the full Dilithium identify handshake.
   Usage: start the relay with `API_SECRET=<anything>` set, then
   `API_SECRET=<same> node scripts/ws-test-client.js ws://127.0.0.1:<port>
   bot_<name> '<json message>' '<json message>' ...` -- prints every
   message received. Already proven for the notification-prefs round trip
   (get defaults -> update -> get again, confirmed persisted). Reuse this
   for: a chat message send + broadcast confirm, join/leave a group
   channel, DM send/receive, the moderation-action messages
   (kick/ban/mute/mod/unmod), and the stream signaling messages (start/end
   stream, viewer join, stream chat). This is the PRIMARY verification
   method for chat backend logic -- fully automatable, no GUI, no button
   clicks, safe to run in a loop all night.
3. **Native GUI passive checks** (when a visual confirmation is actually
   needed, e.g. "does the chat page render correctly with a placed spot
   light" from tonight's earlier work, or "does the livestream scene
   picker UI look right"): launch `C:\Humanity\target\release\HumanityOS.exe`
   (rebuilt from main after merging any worktree work) with
   `HUMANITY_DATA_DIR=<scratch dir>` pointed at the local relay (pre-seed
   that scratch dir's `config.json` with `"server_url":
   "http://127.0.0.1:<port>"` before first launch to skip onboarding
   friction), drop `debug/screenshot_request.json`, read the PNG, kill the
   process. Never leave it running unattended between cycles.
4. **Web pages**: `scripts/preview-server.js` serves static files but does
   NOT proxy `/api` or `/ws` (by design, see its own header comment). For
   pages that need live data (chat, streaming), either (a) accept
   layout-only verification via the static preview + Claude Preview browser
   tools, or (b) temporarily extend a LOCAL COPY of the proxy behavior (do
   not permanently change `scripts/preview-server.js`'s no-proxy design
   without a clear reason -- it's intentional for fast static-page
   iteration) to forward `/api` and `/ws` to the local headless relay
   during a test session only.

## Backlog: chat completeness (priority #1)

Concrete, file:line-referenced gaps found by a repo-wide TODO/FIXME scan
(2026-07-01). Each one needs: read the surrounding code to understand the
real requirement, implement for real (not another stub), verify via the
loopback harness, commit small.

1. **DONE (v0.641.0)** ~~`src/gui/pages/chat.rs:705` -- DM notification
   toggle is a `// TODO: toggle DM notifications` no-op.~~ The relay +
   web client already fully supported this (`notification_prefs` table,
   `get`/`update_notification_prefs` WS messages); the native client just
   never sent/received them. `GuiState` gained
   `notif_dm_enabled`/`notif_mentions_enabled`/`notif_tasks_enabled`/
   `notif_dnd_start`/`notif_dnd_end`/`notif_prefs_loaded`; the popup now
   fetches on first open and the button is a real toggle that round-trips
   to the server. Verified with a REAL protocol test against a local relay
   using the new `scripts/ws-test-client.js` bot-auth harness (see below) --
   confirmed defaults, update, and persisted re-fetch all correct.
   **Follow-up left open**: mentions/tasks/DND are fetched and preserved
   (so the DM toggle never clobbers them) but have no native UI control
   yet -- a later increment should add a proper Settings-page section
   mirroring `web/pages/settings-app.js`'s full toggle set, for dual-UI
   parity. Logged in `docs/FEATURES.md`.
2. **DONE (v0.641.0, BUG-041)** ~~`src/gui/pages/chat.rs:1249` -- `let
   is_group_admin = true; // TODO: per-group role once server reports
   it`.~~ The server already reported this (`GroupData::role`,
   `"admin"`/`"member"` per `src/relay/storage/social.rs::create_group`)
   via `group_list` -- the client `ChatGroup` struct just had no field to
   receive it. Added `role: String`, wired the handler, extracted a
   testable `is_group_admin(role: &str) -> bool` helper with 3 unit tests.
3. `src/gui/pages/chat.rs:1346` -- `// TODO: wire group voice join/leave
   through the relay`. Compare against how the 1:1 voice call path already
   works (`chat-voice-calls.js`/`chat-voice-webrtc.js` on the web side,
   `src/net/webrtc.rs` on the native side) and extend the same signaling
   pattern to group voice rooms (`chat-voice-rooms.js` already exists on
   the web side -- check whether IT already does this correctly and native
   is just behind, i.e. a dual-UI-parity gap rather than a from-scratch
   build).
4. `src/gui/pages/chat.rs:1588` -- `// TODO: implement mute`. Backing store
   doesn't exist yet either (see #6 below) -- these two are one feature.
5. `src/gui/pages/main_menu.rs:135` -- the onboarding "Connect" button is
   `// TODO: actually connect via WebSocket` and just sets
   `state.server_connected = true` without connecting anything. Real
   connection already happens automatically at app init elsewhere
   (confirmed: the boot log always shows `WsClient: connecting to
   wss://...` regardless of this button). Figure out whether this button
   is: (a) vestigial dead UI that should be wired to reflect the REAL
   connection state instead of faking it, or (b) meant to gate a manual
   retry/reconnect action. Read the surrounding onboarding flow
   (`onboarding_step`) to determine intent before fixing -- don't guess.
6. `src/relay/handlers/msg_handlers.rs:1280-1281` -- ban has no real
   backing table (`ban == kick` today, so a "banned" user can just
   reconnect and rejoin); mute has no backing table or column at all. Add
   a `banned_keys` table (or a `server_members.banned` column) and a mute
   mechanism (`server_members.muted` column + a check at message-send
   time), matching the existing `role` column's pattern in
   `server_members`. This is the backend half of items #2/#4 above.

## Backlog: livestreaming verification (priority #2)

The code (`streams.rs` storage, `chat-voice-streaming.js`, the relay
handlers) looks structurally complete on inspection -- no TODO/stub markers
found. The ask is to CONFIRM it actually works, not to find missing pieces.
Use the protocol test harness to verify, end to end:
- Starting a stream creates a real `streams` row and broadcasts the right
  signaling message to viewers.
- Viewer join/leave updates `viewer_peak` correctly.
- Stream chat messages are stored via `store_stream_chat` and delivered to
  viewers in real time.
- Ending a stream sets `ended_at` and finalizes `viewer_peak`.
- The WebRTC signaling path for the actual video/audio stream (not just
  the chat/metadata layer) -- trace `chat-voice-streaming.js`'s signaling
  calls through to `src/relay/handlers/federation.rs`/`msg_handlers.rs`
  and confirm the offer/answer/ICE-candidate relay actually round-trips.
- Scene management UI (`chat-voice-streaming.js`'s scene picker) -- confirm
  it's wired to something real, not just a UI shell.
If a real gap is found, fix it with the same rigor as the chat backlog
above. If everything checks out, document that verification in
`docs/BUGS.md` or `docs/STATUS.md` (whichever fits) so nobody re-audits
this from scratch later, and move to the broader sweep.

## Backlog: broader stub sweep (priority #3, if time remains)

Found via the same repo-wide TODO scan -- these are scaffolded-but-empty
system modules (a struct/module with a comment describing intended fields,
no real implementation yet). Pick the ones with the clearest, most
self-contained scope first; skip ones that need a larger design decision
(log those as `open_questions_for_human` instead of guessing):
- `src/systems/food.rs:42,526` -- food spoilage: needs a data model + tick
  logic, self-contained.
- `src/systems/economy/mod.rs:86` -- passive income credit application,
  self-contained if the wallet/credit system already exists (check first).
- `src/systems/skills/learning.rs:29` -- learning-curve CSV threshold
  check, self-contained if the CSV schema already has the needed columns.
- `src/systems/navigation/orbital.rs:27` -- Kepler's equation solve, a
  well-defined self-contained math problem (real orbital mechanics --
  don't approximate, there's real reference material for this).
- `src/renderer/sky.rs:63` -- wire sun direction to `renderer.set_sun_light()`
  -- likely small given `set_sun_light` already exists and is used
  elsewhere (mothership home lighting from tonight's spotlight-cone work).
- Larger/riskier, defer or log as a question rather than guessing at scope:
  `src/physics/fluid.rs`, `src/systems/ai/autonomy.rs`,
  `src/systems/construction/{blueprint,csg}.rs`, `src/systems/logistics/mod.rs`,
  `src/systems/navigation/mod.rs`, `src/systems/psychology.rs`,
  `src/input/bindings.rs`, `src/input/mod.rs` -- these read like they need
  real design decisions (data model shape, what "done" means) rather than
  a mechanical fill-in. Don't invent scope for these unsupervised; note
  them for the operator instead.

## Per-cycle checklist (repeat this loop)

1. Read this file + `docs/PRIORITIES.md` Active Focus +
   `orchestrator_state.json` recent_decisions (in case a prior cycle this
   session updated them).
2. Pick the next unaddressed backlog item, in priority order.
3. Read the real surrounding code before writing anything -- confirm the
   actual requirement, don't guess from the TODO comment alone.
4. Implement for real (Edit, never Write over an existing file).
5. Verify: both cargo checks, lib tests (count must not drop), 5 lints,
   loopback-harness protocol test and/or a passive screenshot check as
   appropriate to what changed, doc-links check if docs changed.
6. Commit small (`git commit -F <tmpfile>`), bump version if Rust changed
   (`node scripts/bump-version.js minor`), push to both `origin` and
   `forge`.
7. Update `docs/FEATURES.md`/`docs/STATUS.md`/`docs/BUGS.md` if this
   closes or changes something they claim; update `docs/ROADMAP.md` +
   regenerate `data/roadmap.json` (`node scripts/roadmap-to-json.js`) if
   scope changed; update `docs/PRIORITIES.md` Active Focus with what
   shipped.
8. Append a `recent_decisions` entry to `orchestrator_state.json` (what,
   why, files).
9. `ScheduleWakeup` (or the loop mechanism already in flight) to continue.
   Self-pace: don't wake up faster than the cache-friendly window
   described in the ScheduleWakeup tool's own guidance, but also don't
   sit idle -- a real build+verify cycle naturally takes several minutes.
10. Stop condition: ~8 hours elapsed since 2026-07-01 (session start time
    the operator went to sleep), OR the backlog above is genuinely
    exhausted (unlikely), OR repeatedly blocked with nothing safe left to
    do. On stopping: write a clear `docs/history/2026-07-01-night-loop-
    results.md` summary (what shipped, what's verified, what's still
    open, any `open_questions_for_human`) so the operator wakes up to a
    clear account, not just a git log to reconstruct.
