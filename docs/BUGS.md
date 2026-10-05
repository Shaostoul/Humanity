# Bug Tracker

All known bugs and their resolution status. Check here BEFORE fixing any bug to avoid duplicate work.

## Resolved Bugs

### BUG-046: v0.675.0 relay crashed at startup on the LIVE database -- new index in the schema batch referenced a column only added later by the ALTER migration block
- **Status**: Fixed
- **Version Fixed**: v0.676.0 (v0.675.0 was the broken deploy; ~25 min relay downtime until the hotfix deploy went green)
- **Reported**: caught by watching the Deploy-to-VPS run for v0.675.0 (build succeeded on the VPS, then "activating" -> "Process exited with status 3"), not operator-reported.
- **Root cause**: the shared-file library added `CREATE INDEX idx_user_uploads_shared ON user_uploads(shared, id)` inside the main schema `execute_batch` in `src/relay/storage/mod.rs`. That batch runs BEFORE the ALTER-TABLE migration block that adds the `shared` column to pre-existing tables. On a FRESH database (every unit test + the pre-release local smoke test) `CREATE TABLE IF NOT EXISTS` brings the column with it, so everything passed. On the LIVE database the table already existed without the column, the index statement errored ("no such column"), the `?` on the batch propagated, `Storage::open` returned Err, and the relay exited with status 3 on every systemd restart attempt.
- **Fix**: the index is created in its own statement AFTER the ALTER block, where the column exists on both fresh and migrated databases. Regression test `opens_a_pre_v0675_database_and_migrates_it` (`src/relay/storage/uploads.rs`) builds the OLD table shape with a seeded row and requires `Storage::open` to succeed + the migrated row to behave -- the exact production sequence.
- **Lesson (applies to ALL future schema work)**: any index (or trigger/view) over a column added via the ALTER migration block must be created after that block, never in the main schema batch. Fresh-DB tests and fresh-DB smoke tests structurally CANNOT catch live-DB migration ordering -- if a change touches an existing table's shape, add a pre-migration-shape `Storage::open` test like the one above.

### BUG-045: Cloned/mirrored homes in a residential zone rendered walls only -- no floor, ceiling, or trim
- **Status**: Fixed
- **Version Fixed**: v0.654.0
- **Reported**: operator, in-game screenshot ("Looks like the floors for the mirrored homes aren't rendering and some of the other stuff in the home").
- **Root cause**: `ClonableHomeDesign::bake_local_groups` (`src/ship/home_structure.rs`), which bakes the geometry `tile_home_clones` stamps into every residential-zone slot, extracted ONLY `HomesteadMeshes::material_walls` from the generated mesh -- `floors`, `ceilings`, and `trim` (separate fields on the same struct) were never pulled in, so every home clone besides the one the player is actively editing rendered walls with nothing else. The function's own doc comment already described the intent ("an opaque roof reads better en masse"), but the actual ceiling/floor extraction was simply never written.
- **Fix**: `bake_local_groups` now also folds in floors (opaque only, alpha/material_type dropped -- the cloned-home colour bucket has no per-group material slot, the same simplification `material_walls` already accepted) and an always-opaque ceiling + trim (fixed colours matching `src/lib.rs`'s non-glass ceiling/trim materials, since a clone has no independent "is my roof glass" state). Windows and mirrors remain excluded (semi-transparent geometry needs an alpha-aware colour bucket the current flat-RGB scheme doesn't have; a real gap but not what "the floor is missing" was about) and logged as a known follow-up. New test `cloned_home_design_includes_floor_and_ceiling_not_just_walls`, confirmed via revert-and-retest (fails against the reverted code -- only the wall colour bucket present, no ceiling). Files: `src/ship/home_structure.rs`.

### BUG-044: Spoiled food had zero gameplay consequence -- tracked but never checked when eaten
- **Status**: Fixed
- **Version Fixed**: v0.646.0 (pending release)
- **Reported**: found during the 2026-07-01 overnight autonomous-loop broader stub-completion sweep (repo-wide TODO scan), not operator-reported.
- **Root cause**: `src/systems/food.rs`'s spoilage pass (§3 of `FoodSystem::tick`) correctly ages every food item in every inventory and flips a per-slot `spoiled: bool` once `spoilage_timer >= max_freshness` -- but the EAT handler (§1, drains `consume_request`) resolved nutrition purely from the item's static `NutritionProfile` (by item_id) and never consulted the spoilage side-table at all. A player could eat a fully-spoiled item with full nutrition and zero risk, forever, as long as the item_id's own `raw_consumption_risk` was 0 (true for all cooked/canned/preserved food). The `TODO: Replace item with "spoiled_food" variant or reduce nutrition value` comment right at the spoiled-flip site documented the gap but nothing implemented it.
- **Fix**: the EAT handler now looks up the eaten item's inventory slot, checks `self.spoilage.get(&(entity_bits, slot_idx))` for `spoiled`, and if true applies a `nutrition_mult` of `0.25` to both satiation and hydration gain AND guarantees `food_poisoning` regardless of the profile's own `raw_consumption_risk`. Fresh food is unaffected (`nutrition_mult = 1.0`, existing risk-roll logic unchanged). New test `eating_spoiled_food_poisons_and_reduces_nutrition` (`src/systems/food.rs::nutrition_tests`), confirmed to actually catch the bug via a temporary revert-and-retest (fails against the reverted code with the exact expected wrong behavior -- no poisoning, full nutrition). Files: `src/systems/food.rs`.
- **Follow-up fix (same night, adversarial review caught it before the operator woke up)**: the initial fix found the eaten item's slot via `inv.slots.iter().position(...)` (first matching slot, forward order), but `Inventory::remove_item` (the fn that ACTUALLY consumes the item, `src/systems/inventory/mod.rs`) removes from the LAST matching slot backward (`.iter_mut().rev()`, to preserve earlier stacks) -- a real, reachable mismatch whenever the same item_id occupies two separate slots (a normal outcome of `add_item` splitting a stack once the first slot fills). A fresh stack in an earlier slot + a spoiled stack in a later slot meant the spoilage check inspected the fresh slot while `remove_item` actually consumed from the spoiled one -- full nutrition, no poisoning, the exact bug this fix was written to prevent (and the reverse also occurred: an unwarranted penalty on food that wasn't the one eaten). Fixed by making the slot search match `remove_item`'s own order (`.iter().enumerate().rev().find(...)`). New regression test `spoilage_check_matches_the_slot_remove_item_actually_consumes`, confirmed via revert-and-retest (fails against the forward-search version with the exact expected wrong outcome). Caught by an independent adversarial-review agent pass over the night's full diff before any code shipped further -- see `docs/history/2026-07-01-night-loop-plan.md` cycle 12.

### BUG-043: Livestream "peak viewer count" was recorded wrong -- fed the live count at the wrong moment, not the actual peak
- **Status**: Fixed
- **Version Fixed**: v0.645.0
- **Reported**: found during the 2026-07-01 overnight autonomous-loop livestreaming end-to-end verification sweep, not operator-reported.
- **Root cause**: `handle_stream_viewer_leave` and `handle_stream_stop` (`src/relay/handlers/msg_handlers.rs`) both persisted `stream.viewer_keys.len()` (the LIVE viewer count) as the stream's `viewer_peak`. That count is only ever highest right at the moment of a join and monotonically decreases from there -- `handle_stream_viewer_join` never wrote to `viewer_peak` at all. By the time a stream ends (viewers usually trickle out before the streamer stops), the persisted peak was frequently 0 or far below the real maximum. Proved live: 2 viewers joined a test stream (true peak 2), both left, the stream stopped -- the OLD code would have recorded `viewer_peak: 0`.
- **Fix**: `ActiveStream` (`src/relay/relay.rs`) gained a `peak_viewers: usize` high-water mark, updated via `.max()` on every `handle_stream_viewer_join` (the only place the true peak is ever observable). Both the leave and stop handlers now persist `stream.peak_viewers` instead of the live `viewer_keys.len()`. Verified live against a real relay (2 joins -> both leave -> stop -> DB row correctly shows `viewer_peak: 2`) and with 4 unit tests in `src/relay/handlers/msg_handlers.rs::stream_tests`, confirmed to actually catch the bug via a temporary revert-and-retest (both regression tests failed against the old code, recording 1 and 0 instead of 2 and 1). Files: `src/relay/relay.rs`, `src/relay/handlers/msg_handlers.rs`.

### BUG-042: Onboarding "Connect" button always said "Connected!" regardless of whether the server was reachable
- **Status**: Fixed
- **Version Fixed**: v0.644.0
- **Reported**: found during the 2026-07-01 overnight autonomous-loop chat-completeness sweep (repo-wide TODO scan), not operator-reported.
- **Root cause**: `src/gui/pages/main_menu.rs`'s first-run onboarding wizard, step 1 (server URL), had `// TODO: actually connect via WebSocket` and unconditionally set `state.server_connected = true` on click, regardless of whether the typed URL pointed at anything real. Investigation found the app's REAL auto-connect mechanism (`src/lib.rs`) is intentionally gated on `onboarding_complete` and a live identity (created at step 2, one step later) -- so a full WS identify handshake genuinely can't happen yet at step 1. The honest fix isn't the full handshake; it's a real reachability check.
- **Fix**: the button now spawns a background thread (mirrors `src/updater.rs`'s existing `check_now` mpsc pattern, so the UI thread never blocks) that does a lightweight `GET <server_url>/health` (the same endpoint every relay instance already exposes). `server_connected` now reflects the real outcome; a failure shows the actual error message instead of a silent success, and "Continue" only appears once the check genuinely succeeds ("Skip (stay offline)" remains available regardless). Extracted `derive_health_url` and `poll_server_check` as small testable functions (7 unit tests, including the fail-safe cases: a still-checking receiver, a dropped sender, a failed check must never fabricate `server_connected = true`). Verified live: hit a real local relay's `/health` endpoint (success) and a genuinely closed port (failure) to confirm both paths behave correctly. Files: `src/gui/mod.rs`, `src/gui/pages/main_menu.rs`.

### BUG-041: Every group chat member saw themselves as group admin
- **Status**: Fixed
- **Version Fixed**: v0.641.0
- **Reported**: found during the 2026-07-01 overnight autonomous-loop chat-completeness sweep (repo-wide TODO scan), not operator-reported.
- **Root cause**: `src/gui/pages/chat.rs`'s group-channel-row rendering had `let is_group_admin = true; // TODO: per-group role once server reports it` -- every member of every group saw the admin-only channel-edit gear icon as clickable, regardless of real role. The server was NOT actually missing this: `GroupData::role` (`src/relay/relay.rs`) already carries `"admin"` (the group's creator, per `src/relay/storage/social.rs::create_group`) or `"member"` for every entry in the `group_list` WS message -- the client's `ChatGroup` struct (`src/gui/mod.rs`) just had no field to receive it, so the `group_list` handler (`src/lib.rs`) silently discarded the role on the way in.
- **Fix**: `ChatGroup` gained a `role: String` field (defaults to `"member"` if a payload is malformed/legacy -- fail closed, not open); the `group_list` handler now reads `role` from the JSON payload; `chat.rs` gained a small testable `is_group_admin(role: &str) -> bool` helper (`role == "admin"`, case-sensitive, no silent upgrades) with 3 unit tests covering the admin/member/malformed-default cases. Files: `src/gui/mod.rs`, `src/lib.rs`, `src/gui/pages/chat.rs`.

### BUG-040: Star skybox (stars + constellations) entirely invisible in first person
- **Status**: Fixed
- **Version Fixed**: v0.446.0
- **Reported**: 2026-06-14 (operator: showroom is a "black void"; with the homestead roof removed, still no stars/orbits/constellations from inside the home)
- **Root cause**: The star shader (`assets/shaders/stars.wgsl`) places stars + the constellation figures at `direction * 5000.0`, but `StarRenderer::update_camera` built the star view-projection from the GAMEPLAY camera's `projection_matrix()`, whose far plane is `render_distance` (default 500 m). Every star at 5000 sat beyond the far plane and was clipped, so the entire skybox drew nothing. Latent forever; the always-roofed home hid it until the showroom + roof-off (v0.445) exposed it. Not a showroom-state leak (the operator's guess); the showroom merely revealed it.
- **Fix**: `update_camera` now builds a DEDICATED projection for the star pass: gameplay fov/aspect but `Mat4::perspective_rh(fov, aspect, 1.0, 100_000.0)` (far = 100k). The star pass is depthless, so the standard non-reverse-Z convention is safe; x/y matches the gameplay camera. Stars + constellations now render. File: `src/renderer/stars.rs`.
- **Still open (separate, bigger)**: the planet (Earth at GEO ~42,000 km), solar-system bodies (millions of km), and orbit rings (AU-scale) are ALSO clipped by the 500 m gameplay far plane and need a dedicated far/celestial render pass (interior-scale + solar-scale depth-range problem). Tracked as the "celestial far pass" follow-up.

### BUG-039: Cannot sprint; Shift floats down, Space floats up (free-fly noclip)
- **Status**: Fixed
- **Version Fixed**: v0.438.0
- **Reported**: 2026-06-13 (operator: "When I press shift to sprint I instead float down like I have noclip on. When I press space I float up. I can't sprint.")
- **Root cause**: `update_first_person` (`src/renderer/camera.rs`) was free-fly, not grounded. Shift (`descend`) applied a 0.4x crouch-slow AND `position.y -= speed*dt` (float down); Space (`ascend`) applied `position.y += speed*dt` (float up); gravity was commented out with the note "Gravity disabled for space station (no ground reference)" so the jump impulse did nothing. There was no sprint and no real jump.
- **Fix**: Grounded first-person movement. Shift = SPRINT (1.9x, no vertical), Space = JUMP (real impulse), gravity (GRAVITY 12 m/s^2) integrates height with a floor clamp at `ground_y`. The main loop sets `ground_y` each frame via `CameraController::set_ground_floor(floor_y)` from the AABB of the room the player stands in (home room floors are coplanar at y=0); falls back to the last floor when outside every room. ThirdPerson/Orbit vertical fly left unchanged. Files: `src/renderer/camera.rs`, `src/lib.rs`.

### BUG-038: Saved mouse sensitivity ignored on boot (camera too fast, slider showed ~0)
- **Status**: Fixed
- **Version Fixed**: v0.435.0
- **Reported**: 2026-06-13 (operator: "doesn't seem to be saving what I set it to. When I first spawn in my sensitivity is super high until I adjust the value. On first boot it shows 0.0.")
- **Root cause**: TWO separate issues, neither was a save bug (config.json correctly held the saved value, e.g. 0.10948). (1) The camera controller booted at `CameraController::new(5.0, 3.0)`'s hardcoded sensitivity and only synced from `gui_state.settings` inside the `if settings_dirty` block (`src/lib.rs` ~line 4386), which fires ONLY when a slider moves. So on every launch the camera used 3.0 (the old default, ~12x the operator's 0.109) until the slider was nudged. FOV + render-distance had the same latent boot bug. (2) The "shows 0.0" was a DISPLAY artifact: the Mouse Sensitivity slider's range max was 10.0, which selects `labeled_slider`'s 1-decimal format (`{:.1}`), so 0.109 rendered as "0.1" with no precision to tune the low end the operator actually uses.
- **Fix**: (a) Set `gui_state.settings_dirty = true` right after `config.apply_to_gui_state` at startup so the existing apply block pushes loaded fov + sensitivity + fullscreen + render distance into the engine on frame 1. (b) Retune the default 3.0 -> 0.25 in all three spots (config serde default, SettingsState default, controller constructor). (c) Slider range 0.01..=10.0 -> 0.02..=1.0 (max <= 1.0 selects the 2-decimal display + confines the slider to the usable band). (d) Guard `apply_to_gui_state` against a non-positive saved value (a 0.0 would freeze the look) by falling back to the default. Files: `src/lib.rs`, `src/config.rs`, `src/gui/mod.rs`, `src/gui/pages/settings.rs`.

### BUG-037: Chat message duplicates in-memory after a delay, clears on app restart
- **Status**: Fixed
- **Version Fixed**: v0.284.0
- **Reported**: 2026-05-20 (operator saw a #general reply duplicate "after some random amount of time"; closing + reopening the app cleared it)
- **Root cause**: The native client deduped its own sent messages via `chat_sent_timestamps`, but that list is ONE-SHOT, the live-broadcast handler removes the timestamp on the first echo (`src/lib.rs` ~line 1536). On a WS reconnect, `history_fetched` resets (~line 2704) and the client re-fetches the last 50 messages from `/api/messages`. The history-fetch dedup only checked `chat_sent_timestamps` (already consumed) and never checked whether the message was ALREADY in `chat_messages`, so it re-appended copies already on screen. In-memory only (the relay always had exactly one copy), which is why a restart → fresh fetch showed the correct single copy.
- **Fix**: Added a robust content-based dedup, skip the append if `chat_messages` already holds a message with the same `(sender_key, timestamp_ms)`, to BOTH the live-broadcast handler and the history-fetch loop. `(sender_key, timestamp_ms)` uniquely identifies a message (ms precision, per-sender). The `chat_sent_timestamps` fast-path is kept as an optimization; the content dedup is the order-independent backstop that survives reconnect replays + duplicate broadcasts.

### BUG-001: Backup button on settings page broken
- **Status**: Fixed
- **Version Fixed**: v0.15.1
- **Fix**: Fixed event handler binding

### BUG-002: Desktop fetch interceptor failing
- **Status**: Fixed
- **Version Fixed**: v0.16.0
- **Fix**: Corrected Tauri IPC fetch proxy

### BUG-003: Desktop app CSP blocking resources
- **Status**: Fixed
- **Version Fixed**: v0.17.1
- **Fix**: Updated Content-Security-Policy headers

### BUG-004: Blank page on desktop launch
- **Status**: Fixed
- **Version Fixed**: v0.18.1
- **Fix**: Added Tauri IPC guard for window ready state

### BUG-005: Tasks/roadmap API proxy fallback missing
- **Status**: Fixed
- **Version Fixed**: v0.18.2
- **Fix**: Added api_proxy fallback for desktop context

### BUG-006: CORS rejecting Tauri origins
- **Status**: Fixed
- **Version Fixed**: v0.19.0
- **Fix**: Added tauri.localhost to CORS allowed origins

### BUG-007: WebSocket 403 from Tauri
- **Status**: Fixed
- **Version Fixed**: v0.19.1
- **Fix**: Added Tauri-specific WebSocket origin handling

### BUG-008: Service worker breaking desktop app
- **Status**: Fixed
- **Version Fixed**: v0.19.2
- **Fix**: Skip SW registration in Tauri context

### BUG-009: Passphrase modal not showing/hiding
- **Status**: Fixed
- **Version Fixed**: v0.21.0
- **Fix**: Fixed modal show/hide toggle logic

### BUG-010: Download page direct download broken
- **Status**: Fixed
- **Version Fixed**: v0.22.0
- **Fix**: Updated download URL construction

### BUG-011: External links not opening in browser
- **Status**: Fixed
- **Version Fixed**: v0.24.0
- **Fix**: Added target="_blank" and Tauri shell open

### BUG-012: Download page icons missing/broken
- **Status**: Fixed
- **Version Fixed**: v0.24.1
- **Fix**: Added platform brand SVGs

### BUG-013: Game launch button goes to 404
- **Status**: Fixed
- **Version Fixed**: v0.35.1
- **Fix**: Redirected to download page (game is native-only)

### BUG-014: /groups command spamming chat
- **Status**: Fixed
- **Version Fixed**: v0.38.1
- **Fix**: Suppressed unknown command output for /groups

### BUG-015: Upload errors not showing file size limit
- **Status**: Fixed
- **Version Fixed**: v0.38.1
- **Fix**: Added descriptive error messages with size limit info

### BUG-016: Sidebar badges not showing in right panel
- **Status**: Fixed
- **Version Fixed**: v0.38.1
- **Fix**: Added roleBadge() and streamingBadge() to userRow() in chat-voice.js

### BUG-017: Ops nav icon not showing
- **Status**: Fixed
- **Version Fixed**: v0.38.2
- **Fix**: Changed icon key from 'server' to 'ops'

### BUG-018: Ops page not getting active underline
- **Status**: Fixed
- **Version Fixed**: v0.38.3
- **Fix**: Fixed URL detection for /ops path

### BUG-019: Context toggle only clickable on text
- **Status**: Fixed
- **Version Fixed**: v0.38.4
- **Fix**: Made entire pill container the click target

### BUG-020: Green box-shadow on all nav tabs
- **Status**: Fixed
- **Version Fixed**: v0.38.4
- **Fix**: Removed blanket box-shadow, color comes from ::before underline only

### BUG-021: Civilization page blank (JS path wrong)
- **Status**: Fixed
- **Version Fixed**: v0.39.0
- **Fix**: Changed relative script src to absolute /pages/civilization-app.js

### BUG-022: Color underlines blending with border
- **Status**: Fixed
- **Version Fixed**: v0.38.4
- **Fix**: Made underlines 3px thick, offset 2px from bottom, opacity-based

### BUG-023: WASD not mapping to cardinal directions in gardening
- **Status**: Won't Fix
- **Version Found**: v0.24.0
- **Notes**: Superseded by native 3D engine. 2D canvas game is deprecated.

### BUG-024: Desktop app crash on launch (Vulkan overlay segfault)
- **Status**: Fixed
- **Version Found**: v0.88.0
- **Version Fixed**: v0.89.0
- **Description**: App segfaults before main() runs. Steam overlay DLLs hook into vulkan-1.dll loading during wgpu instance creation, corrupting function pointers. Log shows `wgpu_hal::vulkan::conv` warnings then crash.
- **Fix**: Set `Backends::DX12` only on Windows in `src/renderer/mod.rs`. Note: wgpu still compiles+loads Vulkan (hardcoded in wgpu-core's Cargo.toml), but DX12 backend selection avoids the crash path on most systems. Full fix requires disabling vulkan cargo feature (blocked by cargo feature unification).

### BUG-025: Empty config values overwrite GUI defaults
- **Status**: Fixed
- **Version Found**: v0.88.0
- **Version Fixed**: v0.89.0
- **Description**: Fresh `config.json` had empty `server_url` and `user_name` strings. `apply_to_gui_state()` overwrote the hardcoded defaults ("https://united-humanity.us", "Player") with empty strings, preventing auto-connect.
- **Fix**: Guard with `if !self.server_url.is_empty()` before overwriting in `src/config.rs`.

### BUG-026: Passphrase modal blocks startup
- **Status**: Fixed
- **Version Found**: v0.88.0
- **Version Fixed**: v0.89.0
- **Description**: `needs_passphrase()` returned true on every launch if an encrypted key existed, forcing a modal dialog before the user could do anything. Zero-knowledge users had no idea what to do.
- **Fix**: Default to limited mode on startup. Users unlock via Settings > Security when needed. `passphrase_needed` stays false until explicitly triggered.

### BUG-027: Chat message text overlapping header
- **Status**: Fixed
- **Version Found**: v0.88.0
- **Version Fixed**: v0.89.0
- **Description**: `row.rs` tried to render content text beside the header using complex glyph-count-to-byte-offset splitting. Miscalculated byte boundaries caused text to overflow and overlap.
- **Fix**: Complete rewrite of `row.rs`. Content now renders full-width below the header line. No splitting logic needed.

### BUG-028: Wrong binary name in deploy workflow
- **Status**: Fixed
- **Version Found**: v0.89.0
- **Version Fixed**: v0.89.0
- **Description**: `cargo build` produces `target/release/HumanityOS.exe` (per `[[bin]]` in Cargo.toml), but deploy scripts copied `humanity-engine.exe` (the package name). A stale `humanity-engine.exe` from an old build existed in target/, so the copy succeeded silently but deployed an ancient binary that crashed.
- **Fix**: Always copy `target/release/HumanityOS.exe`. Added to SOP.md. Ran `cargo clean` to remove stale artifacts.

### BUG-029: White window flash on startup
- **Status**: Partially Fixed
- **Version Found**: v0.88.0
- **Version Fixed**: v0.89.0
- **Description**: Windows OS paints new windows white before the first GPU frame renders. Briefly visible as a white flash before the chat UI appears.
- **Fix**: Window starts hidden (`with_visible(false)`), renderer initializes, then `set_visible(true)`. Most heavy init is deferred (3D world loads lazily). A brief dark flash may still occur between window show and first egui frame on some systems.

### BUG-030: name_taken error on reconnect
- **Status**: Fixed
- **Version Found**: v0.90.3
- **Version Fixed**: v0.90.5
- **Description**: When the WebSocket connection dropped and the client reconnected, the server rejected the identify message with `name_taken` because the old session was still registered. Users had to restart the app to reconnect.
- **Fix**: Server now properly cleans up stale sessions on disconnect, and the client handles `name_taken` by retrying with the existing identity.

### BUG-031: Native DM encryption not matching web client
- **Status**: Fixed
- **Version Found**: v0.90.3
- **Version Fixed**: v0.90.5
- **Description**: Native desktop client could not decrypt DMs sent from the web client. The ECDH P-256 key exchange and AES-256-GCM encryption in the native binary did not match the web client's crypto.js implementation.
- **Fix**: Implemented matching ECDH P-256 keypair generation, storage, and announcement in the native identify flow (v0.90.4). Added ECDH key import from web client in Settings > Account (v0.90.5).

### BUG-032: Cross-platform build failure (dirs:: crate)
- **Status**: Fixed
- **Version Found**: v0.90.5
- **Version Fixed**: v0.90.6
- **Description**: Build failed on some platforms because the `dirs::` crate could not determine the config directory. The crate has platform-specific behavior that does not work consistently across all environments.
- **Fix**: Replaced all `dirs::config_dir()` calls with `std::env::var("APPDATA")` (Windows) and equivalent env vars on other platforms. Zero external dependency for path resolution.

### BUG-033: Worktree context rot corrupting AI agent edits
- **Status**: Fixed (process fix)
- **Version Found**: v0.90.0
- **Version Fixed**: v0.90.2
- **Description**: Stale git worktrees from previous AI agent sessions contained old file paths (e.g., `native/src/`, `server/src/`) that no longer exist after the v0.90.0 unified binary restructure. Agents working in stale worktrees would write edits to nonexistent paths, losing all work.
- **Fix**: Added `just clean-worktrees` recipe that removes all worktrees except main and current. Added to CLAUDE.md mandatory session start checklist. Automated hygiene prevents context rot.

### BUG-035: Native chat reply disappears after a brief WebSocket reconnect
- **Status**: Fixed
- **Version Found**: long-standing (since the chat page existed)
- **Version Fixed**: v0.125.0
- **Description**: User sends a message in #general; text appears in their chat (local echo). WebSocket has a transient drop/reconnect. After reconnect, the user's message is gone from their own view. The server *did* receive and store the message, on a later session it shows up in history. Net effect: user thinks their message was lost, sends it again, ends up double-posting.
- **Root cause(s)**: Two bugs compounded:
  1. **Same-channel-click clears chat_messages**, Every click on a channel/DM/group/scratchpad row in the sidebar called `chat_messages.clear()` and `history_fetched = false` unconditionally, even if the click was on the *active* row. After a connection blip the user often clicks the channel they're already in (to "refresh"), which nuked any local-echoed unsent text.
  2. **HTTP history fetch on reconnect doesn't dedup** against `chat_sent_timestamps`. The WS broadcast handler at `lib.rs:1139` already dedups server echoes of locally-sent messages by matching `(sender_key == my_key) && timestamp ∈ chat_sent_timestamps`. The HTTP `/api/messages` history fetch in the same file (`lib.rs:1830`) ran no such check, so the user's own message reappeared as a duplicate when it came back from history, and since the local echo was likely cleared by (1), the only visible copy was the server's at the bottom of a freshly-fetched 50-message window.
- **Fix**: `src/gui/pages/chat.rs`, every channel-switch site now no-ops when the click target equals `state.chat_active_channel`. `src/lib.rs`, the HTTP history-fetch loop dedups against `chat_sent_timestamps` mirroring the WS broadcast dedup logic.

### BUG-036: Deleted system channels resurrect on every relay restart
- **Status**: Fixed
- **Version Found**: long-standing (since the seed list landed)
- **Version Fixed**: v0.125.0
- **Description**: An admin opens the cog menu on a system channel (welcome/announcements/rules/stream/dev), confirms delete. The channel disappears for the rest of that session. After the next relay restart, which happens automatically on every git push to main via the deploy CI, the deleted channel is back.
- **Root cause**: `src/relay/mod.rs:170-175` re-ran `create_channel("welcome", ...)` etc. on every boot. `INSERT OR IGNORE` only suppresses on conflict; once a channel was deleted the row was gone, so the next restart's INSERT succeeded and resurrected it. The 6 system channels (welcome, announcements, rules, general, stream, dev) were re-seeded every restart with `created_by = "system"`.
- **Fix**: The seed list now runs **once on first boot** and is gated by a `default_channels_seeded` row in the existing `server_state` key/value table. Subsequent boots skip the seed. The catch-all `general` channel is still always ensured (it's protected from deletion server-side anyway). For pre-v0.125.0 deployments, a one-shot migration sets the seeded flag if the messages table already has rows, so existing operators inherit their current channel set rather than re-seeding deleted channels one last time. To deliberately re-seed (e.g. after wiping the database), delete the `default_channels_seeded` row from `server_state`.

### BUG-034: In-app updater corrupted the local exe ("Unsupported 16-Bit Application")
- **Status**: Fixed
- **Version Found**: v0.122.0 (long-standing, every release of build-desktop.yml since the bundle change)
- **Version Fixed**: v0.124.0
- **Description**: The Build Desktop App workflow only published a single asset per platform, `HumanityOS-<platform>.tar.gz` containing the binary plus `data/` and `assets/`. The in-app updater downloaded that asset, wrote the bytes straight to disk, and renamed it to the exe path. The result was a gzipped tar archive masquerading as `HumanityOS.exe`. Windows refused to load it with `Unsupported 16-Bit Application` because the gzip magic bytes look nothing like a PE header.
- **Fix**: Two changes:
  1. `.github/workflows/build-desktop.yml` now also publishes the raw binary (`HumanityOS-windows-x64.exe`, `HumanityOS-linux-x64`, `HumanityOS-macos-arm64`, `HumanityOS-macos-x64`) alongside the existing `.tar.gz` bundle. Bundles still ship for fresh installs that need the data/assets too.
  2. `src/updater.rs::find_platform_asset` now prefers a raw binary asset and **refuses** archive-only releases instead of silently corrupting the install. Pre-v0.124.0 releases will surface "No binary for this platform", operators must wait for the next tag (which will ship with raw binaries).

### BUG-047: Sky dome vanished (stars at noon) when the planet-detail cap was set low
- **Status**: Fixed
- **Version Found**: v0.913.1 era (latent since the shells were added; surfaced 2026-07-21 in the probe rig)
- **Version Fixed**: v0.918.0
- **Description**: The atmosphere and cloud shell meshes were built at `5.min(planet_max_subdiv)` icosphere subdivisions, sharing the cap that exists to bound the heavy planet-body meshes (levels 8-9, hundreds of MB). An icosphere below level ~3 has its face planes well inside the sphere (a level-0 icosahedron's inradius is 0.79R against the 0.97R planet surface), so with Settings > Planet detail at 0-2 the entire sky shell sat underground: no daytime sky, full starfield at noon, no limb glow from orbit, no disc haze. Every graphics version of the exe showed it identically because the trigger was the CONFIG value, not code drift - which made it masquerade as a renderer regression during bisection.
- **Fix**: Both shell meshes use a fixed level 5 (20,480 tris - trivial on any GPU), independent of `planet_max_subdiv` ([lib.rs](../src/lib.rs), the two `let shell_level = 5;` sites). The cap still bounds the body meshes it was written for.
- **Lesson**: The probe rig's `config.json` accumulates experiment state across sessions. Before attributing a visual bug to code, dump the rig's graphics toggles against `src/config.rs` defaults (`planet_max_subdiv` 6, `planet_lod_px` 10, `planet_clouds` true) - a five-minute check that would have saved an hour of exe bisection.

### BUG-048: Cloud deck invisible from the ground (cloud shadows under a clear sky)
- **Status**: Fixed
- **Version Found**: v0.958.0 (latent since that release; caught 2026-07-26 while hunting the separate underside-banding polish item)
- **Version Fixed**: v0.974.0
- **Description**: The v0.958 low-camera haze fade (`cloud_low_cam_haze` in the megashader) removed the ocean-vantage horizon slab artifact by fading deck fragments on ABSOLUTE slant distance, 30 km to 80 km, tuned from a comment assuming a 2 km deck height. The drawn cloud shell actually sits at `CLOUD_SHELL_SCALE` 1.008, which is 51 km altitude, so from the ground even the zenith fragment sat at 51 km slant (40 percent faded) and every fragment below roughly 50 degrees elevation exceeded 80 km slant and vanished entirely. Net effect for a player standing on the planet: cloud ground shadows sweeping the terrain under a visually clear sky, at three probed locations including one with an active Rain HUD. From orbit everything looked normal (the fade only engages when the camera is inside the shell), which is why 16 releases of from-space captures never caught it.
- **Fix**: Fade on the grazing RATIO instead: slant divided by the camera's radial gap to the shell, which is ~1/sin(elevation) regardless of shell height. Full deck above ~10 degrees elevation (ratio 6), dissolved below ~4 degrees (ratio 14). The horizon slabs sat at ratio 15+ and stay dead. Verified live via shader hot-reload in the probe rig: deck visible overhead from under the Congo canopy, ocean-grazing horizon still clean, from-orbit disc unchanged ([40-clouds.wgsl](../assets/shaders/pbr/40-clouds.wgsl) `cloud_low_cam_haze`).
- **Lesson**: A fade tuned in absolute units silently breaks when the geometry it assumed changes (or was never measured). Dimensionless ratios survive retunes. Also: a fix verified only at the artifact site (the ocean vantage) can delete far more than the artifact; the verify sweep for any "fade X out" change must include a vantage where X should still be VISIBLE.

## Open Bugs

None listed here. BUG-052, the one entry this list carried, was closed on
2026-09-28 (fixed in v0.1315.0 as BUG-077). Entries marked OPEN, PARTIALLY or
REOPENED in their headings further down (BUG-079) are the live ones; BUG-092's
last item was closed by the one clock (v0.1395.0).

Report bugs at https://github.com/Shaostoul/Humanity/issues

## BUG-049: Storm weather rendered as screen-filling rings/lattice (v0.1069.0-v0.1069.1)

**Found**: 2026-07-31, by the operator LIVE (users were on the signed v0.1069.1). Weather
panel -> Storm: giant concentric rings around a disc, wind-scaled. **Fixed**: v0.1070.0
(revert of the clouds tonal-range merge).

**Root cause**: the v0.1069.0 clouds change made the Medium-quality cloud path consume
the params.w slab-bounds ratio for the first time. Under Storm the deck family sits low,
a ground camera is INSIDE the slab, shell_ratio flips to the fly-through branch, and the
march ran on collapsed bounds. Wind scales storm intensity, hence "affected by wind".

**Why verification missed it**: the workflow verified three vantages, ALL clear-weather,
ALL camera-below-slab. The failing combination (weather storm + in-slab camera + Medium
quality) had zero coverage. **Countermeasure**: permanent vantage ground-storm-inslab in
tests/visual/vantages.json carries the regression line; any storm re-land must pass it.

**Lesson**: a verify set that only samples the default environment cannot catch a
regression gated on environment state. Weather conditions are part of the render state
space and the vantage set must sample them.

## BUG-050: GPU-path precipitation rendered as giant colored spheres/blobs (v0.1068.0-v0.1070.2)

**Found**: 2026-07-31 by the operator (experimental GPU particles + heavy snow: a
screen-filling blue sphere, cyan flake-blobs, magenta clusters). **Fixed**: v0.1071.0.

**Root cause**: particle_sim.wgsl declared the shared vertex buffer as a WGSL struct
annotated "matches ParticleVertexData byte for byte". It cannot: vec3 in a WGSL
storage buffer aligns to 16 bytes, so the sim wrote 64-byte records into the 52-byte
packed stream the vertex path reads; every instance drifted 12 bytes further out of
phase (sizes read world coordinates, colors read neighbouring floats).

**Why nothing caught it**: default-off setting, so the rig never exercised the path
(portable sandbox boots default config), and the byte-parity claim lived in a comment
with no check that could fail.

**Countermeasures**: sim now packs 13 floats by hand into array<f32> (keeps the tight
52-byte layout the CPU path was optimized to); showcase IPC gained a gpu_precip flip
key; permanent vantage ground-snow-gpu exercises the experimental path with the bug
signature in its regressions. **Open gap**: no mechanical Rust-vs-WGSL layout check
exists; any shared-buffer struct change still relies on eyes. Toolsmith candidate.

## BUG-051: Snow followed the camera into orbit, second recurrence (fixed v0.1073.0)

**Found**: 2026-07-31 by the operator ("The snow is in space again"), FTL orbit view
with Snow active. **Class history**: v0.1064 fixed rain persisting to space in low
flight with a surface_mode + altitude < 4000 m gate.

**Root cause of the recurrence**: the gate read surface_altitude_m.unwrap_or(0.0).
The altitude readout is None whenever no surface is frame-locked (reset every frame
on the FTL/fly branch), so "no reading" was treated as "0 m, standing on the
ground" and the gate passed in deep space. Fixed to map_or(false): no reading = no
precipitation. Single choke point, covers both CPU and GPU paths.

**Countermeasure**: permanent vantage orbit-snow-gate (blue-marble camera + Snow +
gpu_precip) expects ZERO flakes in frame; ground-snow-gpu proves the fix does not
kill legitimate snowfall. Both captured clean on v0.1073.0.

**Lesson**: unwrap_or on an Option encodes a default-state ASSUMPTION. For gates,
absence of data must fail SAFE (here: not-near-surface), never default to the
permissive branch.

## BUG-052: Settings VSync OFF panics the app at boot (FIXED v0.1315.0, as BUG-077; closed 2026-09-28)

**Closed 2026-09-28.** This is the crash BUG-077 found again on 2026-09-18
and fixed in v0.1315.0: the present-mode change is now recorded by
`Renderer::set_vsync` and applied at the START of the next frame, before the
surface texture is acquired (`renderer/surface.rs`,
`apply_pending_surface_config`), which is the "defer the reconfigure" fix
this entry proposed. The entry stayed OPEN, and the only item in the Open Bugs
list, for ten days after the fix and two months after it was filed. Re-proved on v0.1403.0: a probe-rig boot with `vsync: false` in the rig's config.json logged `[Surface] present mode AutoVsync -> AutoNoVsync (applied before the frame)`, entered the world and captured blue-marble-12000km with panics=0 and no "window is in use" line in run.log.

The original report follows.

**Found**: 2026-07-31, by the clouds domain pass's perf agent while trying to lift the
present-pacing cap off frame-time measurement. Reproduced DETERMINISTICALLY twice on
v0.1073.1. NOT operator-reported yet, and NOT a clouds bug - filed separately because it
is shipped and user-facing (any user who turns VSync off in Settings > Graphics).

**Symptom**: with `vsync: false` in config.json the app dies during world entry. Every
subsequent IPC request times out. Log sequence:

```
ERROR wgpu_hal::dx12  ResizeBuffers failed: The application made a call that is invalid... (0x887A0001)
ERROR wgpu_core::device::global  surface configuration failed: window is in use
PANIC ... In Surface::configure / Invalid surface
```

**Mechanism (unconfirmed, this is the reading of the log, not a diagnosed fix)**: the
boot-frame settings-apply calls `Renderer::set_vsync` (`src/renderer/mod.rs:1305`); the
requested present mode differs from the current one, so `surface.configure` runs - and it
appears to run while a swapchain image is still acquired. With `vsync: true` the mode
matches, no reconfigure happens, and the same rig boots and runs normally. A second,
NON-fatal instance of the same "window is in use" configure shows up occasionally at boot
even with vsync on, which is what suggests a race rather than something specific to the
present mode. Likely shape of the fix: defer the reconfigure to the top of the next
frame, before the surface texture is acquired.

**Why it also matters to engineering**: with vsync off, `frame_ms` becomes a continuous
measurement instead of one bounded by the refresh interval. `blue-marble-12000km` reads
exactly 16.1 ms in every configuration because it is present-capped, so it can never show
a per-feature delta. Fixing this unblocks the cleanest perf-measurement path we have.

**Acceptance**: toggle VSync off in Settings > Graphics on a normal boot, and again under
`HUMANITY_NO_FOCUS=1`; expect 0 PANIC in run.log and a frame rate that rises above the
refresh interval.

## BUG-053: Disabling rain froze it mid-air instead of clearing (fixed v0.1076.0)

**Found**: 2026-07-31 by the operator ("it just freezes in place like time
stopped"), GPU particle path. **Root cause**: leaving rain/snow skips the whole
GPU block, so simulate() stops dispatching, but the pool's live count and vertex
buffer keep their last state and the draw renders the stale verts every frame.
**Fix**: deactivate_gpu_particles() (live = 0) runs every frame the path is
inactive; draw skips at live == 0, the recycling sim re-seeds on reactivation.
Verified with an in-session rain-to-Clear transition at the rig: zero residual
streaks.

**Class note**: third GPU-pool defect in two days (BUG-050 stride, BUG-051 gate,
BUG-053 lifecycle). The pattern: state the CPU path managed implicitly (emitter
lists rebuilt per frame) that the GPU pool must manage explicitly.

## BUG-054: Terrain vanished and reappeared on a ~6 s cycle while standing still (fixed v0.1077.0)

**Found**: 2026-07-31 by the operator; root-caused from his own run.log (78 of 880
diag ticks collapsed, 20 of them to a single 389-million-px triangle at 11 m).

**Root cause**: patches the selector DEPENDS ON but never draws (split parents,
provably-invisible drops) were invisible to both LRU eviction guards. At maxed
terrain sliders the patch cache genuinely reaches its 1536 MiB cap, eviction
engages, and the oldest entries are exactly these load-bearing never-drawn
patches; evicting one stalls restricted descent and the subtree collapses to one
giant leaf until the rebuild, 120 frames later, forever. Standing still made it
worse BY CONSTRUCTION: a frozen draw set leaves only these as eviction victims.

**Why no rig run ever saw it**: fresh rig configs use the default patch budget
(3072), where the cache never approaches the cap and eviction never fires. The
enabling condition only exists at slider max (12288 + split_px 2).

**Fix**: `Selection::required` reports every built node the walk depended on
without drawing; the LRU stamps draws AND required each frame. Guarded by
`required_patches_are_reported_and_losing_one_collapses_the_cover` (structural:
descendants of an evicted required node vanish and an ancestor takes over as one
leaf). Rig re-run at the operator's exact settings shows healthy ramps and zero
collapse ticks; the conclusive validation is the next long parked session.

**Still open from the same investigation** (parked in PRIORITIES): the root-cause
alternative (do not let a provably-invisible child block its parent at all), and
a real secondary find: the v0.1062 arena rebalance over-corrected, vertex arena
now binds first (130k "vertex arena full" warnings, 1.2-2.4k classic fallbacks
per frame vs the commit's claimed 0-374).

## BUG-055: Every "quiet" background boot could steal focus when nobody was typing (fixed v0.1081.0)

Agent-booted HumanityOS instances kept yanking the operator out of games/videos
despite THREE prior countermeasures (env var v0.828, create-visible-inactive
v0.1069, no_focus.txt marker v0.1079). Verbose foreground tracing on 2026-07-31
proved the v0.1069 mechanism NEVER worked: a window created VISIBLE is activated
by the system whenever no foreground input lock is held, i.e. exactly when the
operator is watching rather than typing. Every earlier "verified quiet"
measurement had passed only because active typing held the input lock. The trace:
`background=true` logged correctly, window foreground from sample 0 anyway.

Fix (two layers, `src/engine/launch_focus.rs`):
1. POLICY INVERSION: focus requires proof of a human launch -- explorer.exe as
   the parent process (real double-click; toolhelp32 FFI) or HUMANITY_TAKE_FOCUS
   (set only by `just play` / `just launch`; updater restart scripts propagate).
   Scripts get background BY DEFAULT; a DEAD parent also means background (only
   script launchers exit instantly -- the first hostile test caught this fallback
   pointing the wrong way and stealing).
2. MECHANISM: all windows now create HIDDEN (hidden windows cannot activate);
   background instances are shown via raw ShowWindow(SW_SHOWNOACTIVATE) +
   SetWindowPos(HWND_BOTTOM), which never activates regardless of input-lock
   state. winit's set_visible(true) always activates on Windows -- never use it
   for a background window.

Guard: `tests/focus_optin_lint.rs` pins HUMANITY_TAKE_FOCUS to an allowlist so
no script or agent definition can ever set it. Verified: hostile dead-parent
boot with the operator's browser foregrounded stayed focus-clean across a
24-sample trace; probe rig still enters the world and captures.

LESSON for any future focus work: a focus test is only valid when NO input lock
is held (nobody typing). Test with a detached spawn whose parent exits, sampling
GetForegroundWindow -- not by watching whether a window "seems" to come up behind.

## BUG-056: Tree canopies shaded as bark -- transmission and flutter dead on 5 of 8 species (fixed v0.1081.0)

Backlit crowns on sakura/momiji/oak/birch/acacia read as black shards (canopy/sky
luma ratio 0.026 measured; real foliage is ~0.5). Root cause: tree_mesh::blade()
emitted foliage through PlantMeshBuilder::tri2, which never sets the organ tag,
so ORGAN_BIT_LEAF (bit 19) stayed clear, `is_leaf` was false in
90-fragment-main.wgsl, and every leaf took the BARK shading branch. The
subsurface transmission shipped in v0.1078 and the leaf flutter shipped in
v0.1080 never executed on any of those species (palm alone used b.leaf()).
This was also the mechanism behind the operator's "textures look like one big
chlorophyll sheet with leaf cutouts" report.

Fix: plant_mesh gains pub(crate) set_organ(); blade() tags Organ::Leaf around
its tri2 calls. After: ratio 0.448, dark fraction 22.8%, independently
reproduced by an adversarial reviewer with its own PNG classifier.

Guards: tree_mesh unit test asserts procedural species emit leaf-tagged
geometry (pre-fix count was exactly 0); fuji-forest-ground vantage carries a
quantified NO-black-backlit-canopy regression with its classifier spelled out.

LESSON: when a "missing feature" is reported (no transmission, no flutter),
check whether the feature is GATED ON A TAG the geometry never sets before
building more feature. Two shipped features were dead for 3 releases because
the gate bit was never written.

## BUG-057: The night side glowed -- four unlit-light leaks (fixed v0.1083.0)

Operator: "check all the shaders/textures to make sure they're not slipping in
emissiveness anywhere... I think the beach water might be as well." A three-way
audit (shader terms, asset inventory, night captures) found and MEASURED four
defects, each pixel-predicted before fixing:

1. **Trees sunlit at midnight.** The celestial pass stamped sun intensity as a
   constant 2.5 day and night; only terrain (type 12) has a per-fragment
   terminator gate, so every tree/prop rendered warm-lit against black ground
   (trunks 19.7 mean luma vs terrain 0.0). Fix: renderer.celestial_sun_day,
   camera-local day factor scaling the stamped intensity (lib.rs computes it
   beside the sky's day term; 1.0 off-planet).
2. **Leaf transmission un-gated.** The subsurface term was not multiplied by any
   light amount; an up-facing leaf at midnight scored backlit ~1.0 against the
   below-horizon sun. Now scaled by the day factor (shader reads
   sun_direction.w * 0.4).
3. **Beach/underwater in-scatter was a constant.** vec3(0.008,0.030,0.055)
   added un-multiplied: the through-water half of a coastal frame was
   BIT-IDENTICAL at noon and midnight (measured 8,41,63 both). This was the
   operator's suspected beach glow. Now scaled by daylight; the noon frame is
   unchanged, the night frame goes dark.
4. **Night fog rendered at daytime brightness.** The weather-fog sky tint used
   lum.max(0.25), resurrecting a light-independent floor after the sky was
   correctly day-scaled to zero (measured 139/143/147 vs predicted 139/143/146
   -- exact). Floor removed; fog scatters the light that exists.

Also closed from the audit: home MIRRORS kept 1.6 emissive (same class as the
v0.780 window fix, missed); space_dust was the only alpha-blended emitter with
nonzero emissive (0.6 -> 0.0); the MaterialUniforms doc comment claimed "z/w
unused" when w is emissive AND repurposed as a data channel by types 12/15/18
(comment now warns -- that lie is how the next glow bug gets written).

Verified: rebuilt, re-ran the audit's own six night scenarios -- Fuji forest at
local midnight is black with stars through the canopy (and rolled fog stayed
dark, covering #4); beach noon vs night now differ (bright turquoise vs
near-black). Captures in the session scratchpad night-out/.

LESSON: additive light terms must name what LIGHT they scatter. Any term added
to final color carrying only albedo/geometry factors is a night-glow bug by
construction. And the emissive slot doubling as a type-specific data channel
means "grep for emissive" is not an audit -- walk every `+` in the color path.

## BUG-058: Fir and pine rendered NOTHING in a shipped build (fixed v0.1086.0)

Release bundles carry no assets/models/ (build-desktop.yml ships data/ +
icons + shaders only), and the near-tree loader had no fallback for
model-backed species: the glTF parse failed, a sentinel was cached, the atlas
tile stayed empty, and the card discarded on alpha - so the only two conifers
in the game were invisible AT EVERY DISTANCE for anyone who downloaded it.
The dev checkout masked it for months because the models exist there; every
vegetation fidelity number to date blended 62 non-shipping photoscans.

Fix: on parse failure the loader now builds the species PROCEDURALLY (fir
and pine carry form:"conifer" in trees.ron), at the SPECIES height, cached
under the proc key and fed to the card baker; the draw site derives use_proc
from the sentinel and switches stem/scale/suffixes together. The scale had
to branch BEFORE the TREE_MODEL_H divisor - the naive fallback (build proc,
keep the model-scale math) draws a 381 m fir, because TREE_MODEL_H are
~1.3-unit sapling scans (critic catch, journaled before implementation).

Guard: fuji-forest-ground carries a "FIR AND PINE MUST BE VISIBLE IN A
SHIPPED BUILD" regression naming all three legs of the hole.

LESSON: the dev checkout is a strictly RICHER environment than the shipped
product. Any feature keyed on an asset's presence needs a fallback tested
with the asset ABSENT - and the rig junctions the full repo, so rig green
does not cover it.

## BUG-059: The cluster-sprite bake ran every frame (fixed v0.1088.3)

The v0.1088.0 card wiring called bake_cluster_sprites unconditionally inside
the near-tree block, which is PER-FRAME - the moved>12m hysteresis closes
ABOVE the call, not around it. Every consumer below is guarded by cache
checks, so from frame 2 the ~90 ms blocking bake (device.poll Wait) was
computed and discarded: 1,671-4,576 [Cluster] lines per session on three
independent rigs, eating the entire frame budget (fuji 8.5 fps). Found by a
domain-pass challenger agent measuring a different thing entirely.

Fix: bake only when a clustered species lacks its card cache entry. After:
6 [Cluster] lines per session, 24.5 fps / 40.7 ms at the same vantage.

LESSON: "runs once" must be enforced by a guard you can point at, not by
assumption about the enclosing block - the near-tree block LOOKS like a
once-per-arrival block and is not. And a frame-time regression right after
a wiring change is the wiring until proven otherwise - I attributed the
drop to card draw cost without measuring.

## BUG-060: Leaf/grass transmission was sign-inverted, unphysical, and unshadowed (fixed v0.1095.0)

The foliage transmission lobe computed dot(V, L - N*d) - which peaks when the
sun is IN FRONT of the leaf, the exact opposite of the standard backlit form -
at coefficient 1.05, which exceeds a leaf's own maximum diffuse response by
1.32x, and it bypassed the shadow map entirely. Measured: the term supplied
73% of all grass luminance; removing it made grass:terrain mean luminance
exactly 1.00. This was the operator's "grass glows while the land is dark"
dawn report, and the same block exists byte-identical in the tree canopy path.
Diagnosed by A/B shader hot-patching with per-pixel frame differencing.

Fix (both type-20 and type-23 blocks): correct lobe sign, coefficient 1.05 ->
0.15 (+0.35 -> 0.06 on the backlit floor), multiplied by sun_shadow.

LESSON: "reads as ambient bounce" comments hide magnitude bugs - any additive
light term needs its coefficient justified against the surface's own diffuse
peak, and NO sun-derived term may skip the shadow map.

## BUG-061: Strip-light endpoints tracked the player (fixed v0.1095.0)

LINE lights pack endpoint B in the `dir` field (cos_outer <= -1.5 sentinel).
The orbital-station translation offset `pos` and never `dir`, so all 10 home
strip lights became segments stretching from the station down to the RENDER
ORIGIN - which in surface mode is rigidly welded to the player (frozen
camera.position; walking moves the frame anchor instead). The shader's
closest-point-on-segment math then pooled faint cool-white light at the
player's feet, tracking them through jumps - the operator's report and their
player-space-vs-world-space hypothesis, exactly. Bonus damage: the light
tiler binned those segments as planet-sized spheres, flooding all 144 tiles
and silently evicting real lights at TILE_CAP=64. Invisible to every probe
rig because dev teleports reset camera.position ~52 m up, parking the stray
endpoints underground - only a player who walks out of the home could see it.

Fix: line-sentinel lights offset dir too; overlay_objects (the one list the
station translation missed) now offsets as well.

LESSON: a position living in a field named `dir` is invisible to every
translation site. Fields that change meaning per-variant need a translate()
method that knows, not a convention. And probe rigs share the dev-teleport
blind spot - operator-path reproductions matter.

## BUG-062: SSAO estimator painted an aura around every tree (fixed v0.1100.0)

The operator: "trees seem to have a kind of aura around them that's altering
the color of the grass behind them." Measured at their settings: a symmetric
6.6-9.5% darkening hugging every trunk silhouette, decaying by ~70-80 px,
gone with ssao_strength=0. Aerial haze, god rays, and foliage mip-bleed all
refuted by A/B measurement.

Root cause (assets/shaders/ssao.wgsl, v0.901 estimator): depth-only occlusion
with a 0.4-1.6 m "full occluder" window and a 48 px screen-disc cap that
binds for everything nearer than ~29 m. Ground behind a trunk is exactly
1.6 m-class nearer, so every trunk shaded ground it never touched. No normal
meant grazing ground planes also self-occluded (broad ground darkening).
There was no blur pass to blame - the estimator itself was the halo.

Fix (v0.1100 rebuild): reconstruct view-space positions from depth (true
focal length in pixels replaces the px-per-radian approximation), build a
surface normal from neighbor depths choosing the smaller-delta side per axis
(edges don't smear), cosine-weighted occlusion above the tangent plane, hard
range falloff at 2x a 0.4 m radius (was 1.6 m), screen disc capped 16 px
(was 48). Foreground objects now fail the range falloff; on-plane taps have
~zero cosine. Deferred (fenced, not half-done): applying AO to the ambient
term inside the PBR shader instead of multiplying the tone-mapped frame
needs a depth prepass - tracked in PRIORITIES.

LESSON x2: (1) a missing blur was the WRONG hypothesis - "add bilateral
blur" would have blurred a fundamentally wrong signal; diagnose the
estimator before the filter. (2) The probe rig's default settings produced a
CLEAN FALSE NEGATIVE on this bug (ssao 0.55 vs operator 0.96, dense-grass
filter broken by veg_density mismatch); the same measurement at operator
settings separated 0.905 vs 1.018. A rig verdict is a verdict about the
rig's settings - probe-sweep now records graphics settings in its manifest
and offers --operator-config.

## BUG-063: Sapling photoscans stretched 19x into metre-wide leaves (fixed v0.1101.0)

The operator's v0.1100 captures showed large pale blades lying flat across the
grass, roughly a metre long, plus black slivers of the same shape and
near-black fronds against the sky.

All one asset: the fir and pine PHOTOSCANS. They are scans of ~1 m saplings,
and trees.ron gives those species 22 m and 16 m, so the draw site's uniform
scale (species_height / model_height) ran 13.7-19.2x and multiplied EVERY
triangle - each 3-7 cm needle spray became a 0.5-1.9 m sheet. Measured from
the capture by calibrating against the authored grass height band: blades
0.72-1.74 m where co-located grass tufts read 0.30-0.54 m.

The pale/black split was a SECOND bug in the same asset: material type 19 had
no transmission term at all, while the sun term is shadow-gated, the fill is
N.L-gated and the ambient floor is 0.005. So a face pointing away from the sun
rendered at 1/255. The pixel population was bimodal - thousands of blown-out
and thousands of exactly (1,1,0), almost no mid-tones - which is the signature
of a missing transmission term, not of a shading gradient.

Fix: the near-tree loader computes each scan's AABB height and REJECTS one
whose species would stretch it past MAX_MODEL_STRETCH (3.0), falling back to
the existing BUG-058 procedural path. Type 19 gained the type-20 leaf
transmission, gated on params.w so furniture and machines sharing the type do
not transmit.

LESSON: this was invisible to every release-path check because the SHIPPED
build has no assets/models/ and has always taken the procedural fallback. A
dev-checkout-only artifact survives exactly as long as verification only ever
runs the shipped path. Also: no scale factor turns a sapling into a mature
tree - the branching architecture differs, not just the size - so "we have a
scan of that species" is not the same as "we can use it at that height".

## BUG-064: BUG-060's shadow gate reached two of three foliage branches (fixed v0.1101.0)

BUG-060 (v0.1095) established that no sun-derived term may skip the shadow map
- a leaf in shadow receives no sun to transmit. The fix was applied to the
type-20 procedural leaf and type-23 grass branches. The type-21 CLUSTER CARD
branch has the same backlit term and did not get it, so cards standing inside
another tree's shadow kept emitting at full strength and shaded crowns glowed.

Found by an adversarial review of an unrelated diagnosis, not by any gate.

Fix: type 21's backlit term now multiplies by sun_shadow, same as its twins.

LESSON: a fix applied by search-and-edit stops at the occurrences you happened
to search for. When a rule is "every X must do Y", enumerate every X - and
prefer a shared helper the branches call over three copies of a formula.

## BUG-065: Cluster cards never used the mip chain built for them (fixed v0.1101.0)

v0.1090 gave foliage cluster cards a full alpha-coverage-preserving mip chain,
a trilinear sampler and anisotropy_clamp 8, specifically to stop them crawling
at distance, and recorded that as fixed. The shader fetch was
textureSampleLevel(..., 0.0) - an explicit LOD 0, which bypasses both the mip
chain and the anisotropy. Type 19 had the same forced-LOD-0 fetch against
1024x1024 photoscan atlases.

Fix: both branches sample with textureSampleGrad using the gradients already
computed in uniform control flow at the top of the fragment shader.

LESSON: the evidence for "cards now carry their full mip chain" was the UPLOAD
code. Nothing verified that the shader asked for a mip. When a fix spans a
CPU-side resource and a GPU-side fetch, the claim is only true if BOTH ends
were checked - and the checkable end is the rendered result, not the setup.

## BUG-066: A timer named cpu.patch_build charged 2,030 lines (fixed v0.1102.0)

A canopy increment appeared to cost 35 ms per frame (`cpu.patch_build` 3.84 ->
38.56 ms, 30 fps -> 11 fps). A release was held for it. There was no
regression: on the vantage the repo designates for frame-time work the new
build was FASTER (-3.0 ms GPU, -0.7 ms CPU, -81 MB VRAM).

Three independent errors produced the phantom, and all three are worth knowing:

1. THE TIMER. The `cpu.patch_build` RAII guard lived until the whole
   `if chunked_on` block closed - about 2,030 lines - so it also charged patch
   draw batching, the near-tree loader, twelve glTF parses, procedural
   fallbacks, the cluster-sprite bake, grass, the far-tree card sheet and the
   water shell. A ONE-TIME ~2.4 s world-entry bake landed in a per-frame stage
   and the frame EMA smeared it across ~20 frames.
2. THE CONTROL. `.probe-rig/data` is a SYMLINK to the repo's `data/`, and
   serde ignores unknown fields - so the "before" exe read the NEW trees.ron
   and already had every cluster card the change added. The A/B had no control
   at all; the thing under test was in both arms.
3. THE VANTAGE. `fuji-forest-ground` says in its own `_perf_floor_note` that it
   must NOT be used for A/B frame-time work (it keeps getting heavier for ~40 s;
   a prior audit measured 2.15x across byte-identical runs). Use
   `ground-storm-inslab`.

Fix: the stage ends where patch build ends (2.51 ms measured), the remainder is
charged to a bucket that names its own contents and the split that finishes the
job.

LESSON: a measurement is a claim about a stage, a build, and a scene, and it is
only as true as the weakest of the three. Before trusting a delta, check that
the stage measures what its NAME says, that the control arm genuinely lacks the
thing under test, and that the scene is one that repeats. Two of these three
were documented in the repo already and I did not read them.

## BUG-067: Every card gate silently exempted the species that needed it (found v0.1102)

Three gates - `cluster_sprite_geometry_fits_its_card`,
`near_blades_stay_inside_the_card_shell`, and
`cluster_cards_reach_target_lai_and_fit_the_budget` - all begin by skipping any
species with no `clusters` block. Fir, pine, acacia and palm have none, so four
of eight species sit outside EVERY blade and card gate. Their own stdout is the
proof: every reported line names only sakura, momiji, oak and birch.

Those four are exactly the species whose crowns are raw blade triangles with no
card mass to hide inside, which is what the operator sees as scattered darts on
a conifer. The gate skipped precisely the case it exists to catch.

Contrast `crown_depth_is_a_real_live_crown_ratio`, which filters to broadleaf
and then asserts `seen > 0`. That non-vacuity guard is the whole difference.

LESSON: a filter at the top of a gate is a silent exemption list. Any gate that
skips rows must assert how many rows it actually examined - and when the skipped
set is non-empty, that set belongs in a DATED allowlist, not in an `if` nobody
can see the effect of.

## BUG-068: The card-hide radius promised models that never drew - three proxies in three releases (fixed v0.1110.2)

The renderer is told a radius inside which every terrain tree CARD must discard,
because "a real 3D model stands here". That radius is a PROMISE, and three
successive rules each computed a PROXY for it instead of the thing itself. Each
proxy broke exactly where it parted from the promise.

- **v0.995, the view-culled draw count.** "Fewer than 64 trees drew, so the set
  is sparse, so hide cards across the whole window." With half the set always
  behind the camera that misfired constantly, hiding cards over ground the
  64-tree budget never covered. Symptom: a bare ring riding with the player.
- **v0.1107, the budget-th tree.** The nearest-sorted set's budget-th tree was
  taken to mark where models end. Measured, 45.2% of frames had hide radius >
  the model distance, so a 34-52 m treeless ring rode with the player.
- **v0.1110.1, the farthest tree that DREW.** Correct only if the draw order is
  perfectly nearest-first, and it is not: the harvest re-sorts every 12 m of
  walking while the camera keeps moving, so trees ahead of the player overtake
  the ranking. Measured over a 40 m walk at Fuji, forest density 0.6: up to 32
  orphaned trees per frame at the shipped budget (48 at 400 m), in an 11.5 m
  band, **100% of them ahead of the direction of travel**. The count is zero
  right after a re-harvest and worst just before the next, so the hole PULSES at
  the 12 m walk period. Operator: "the billboards for the lower LOD trees just
  kind of phase out of existence instead of actually shifting to a higher LOD."

Compounding it, a hardcoded `600` was passed as the harvest's `max_n` while the
draw budget was 1024. The harvest walks, gates, ground-samples and SORTS the
whole disc before truncating, so that cap bought nothing except destroying the
information the draw loop needs - which is why raising the draw budget from 1024
to 4096 changed literally nothing.

**Fix (v0.1110.2)**: `terrain::near_trees::ModelCoverage` derives the radius from
the NEAREST TREE THAT GOT NO MODEL - budget exhausted, or mesh not yet streamed,
a case every earlier rule was blind to. The harvest cap is now `budget + 256`, so
the budget is always the binding constraint, because the budget is the only cap
the draw loop can observe. It fails safe: worst case a card shows inside a model,
where the model hides it.

**LESSON**: when a value is a PROMISE to another subsystem, compute the promise,
not a correlate of it. Write the promise down as a sentence first ("every card
inside this radius has a model in it") and check the expression against the
sentence. Also: a single-frame test passes on all three broken rules - only a
MOVING camera exposes any of them, so the gate walks 40 m and asserts the fixture
saturates the cap, or it would quietly stop exercising the bug.

## BUG-069: Forest density was a process-global that two streams read separately (fixed v0.1111.0)

Two independent code paths decide which trees exist: the card bake inside a
terrain patch, and the near 3D-model harvest. `near_trees.rs` states the contract
in its own comment - they "MUST stay byte-identical" - because a card with no
model behind it discards in the colour pass while STILL CASTING SHADE (the shadow
pass deliberately does not mirror the card-distance discards). Both read the
density from `TREE_DENSITY_BITS`, a process-global atomic, for themselves.

They could disagree two ways.

- **Rounding.** The bake rounded twice (`round(round(800 * d) * cos(lat))`), the
  harvest once. `count` is not a loop bound - it is the right-hand side of the
  survival gate `item >= count * vw` - so it decides WHICH items live. Measured
  over all 43,478 northern cells: **32.51% disagree at density 0.6294**, 26.69%
  at 0.6295. Not clean even at the shipped default: exactly one cell row splits
  at 0.6, iy 10727 at 21.2 degrees north, so a real latitude band through Mexico,
  India and Vietnam shipped with cards that had no models.
- **Split-brain.** A slider move changed what the next patch bake emitted at
  once, but the harvest only re-ran every 12 m of walking.

It also caused a ~50% test flake, because one test wrote the atomic mid-run and a
sibling in the same binary read it.

**Fix (v0.1111.0)**: density is an explicit argument of both streams, sourced
once per frame, and the per-cell count comes from ONE function. The atomic is
DELETED rather than kept as a bridge; callers that want ground geometry and do
not care what grows on it take a fixed `AGNOSTIC_TREE_DENSITY` pinned to the
shipped default by a test. The invariant is now stated and gated: *every tree
card that can be on screen has a 3D model standing in it*, which holds because
the enumeration is monotone in density (each item's randoms depend only on its
index, and both the loop bound and the survival threshold rise together, so a
higher density yields a strict superset).

**LESSON**: a value two subsystems must agree on exactly is not a global, it is
an ARGUMENT. A global makes the agreement a matter of timing; an argument makes
it a matter of type-checking. The same reasoning kills the test flake for free -
there is no shared mutable state left to fight over. And when the rounding of a
shared quantity is duplicated, the duplicate IS the bug: one function, two
callers.

## BUG-070: Chat's "optimistic" connected flag froze the whole app during server outages (fixed v0.1122.0)

**Report:** "the app froze while I was sitting inside of it watching the chat.
I had to Alt+F4" (operator, 2026-08-13, during the Namecheap maintenance
outage).

**Root cause, one word wide:** `WsClient` initialized `connected: true` with
the comment "optimistic; we'll detect disconnection on poll". That one lie
had two independent consequences whenever the relay was unreachable:

1. The reconnect backoff (5s doubling to 60s) NEVER engaged: the
   reset-on-success block gated on `is_connected()`, which was true the
   instant each attempt spawned, so attempts reset to zero every cycle and
   the log showed "attempt 1" forever, every ~26 s.
2. The channel-history fetch gated on the same lie and fired every cycle:
   an inline `ureq::get().call()` with NO timeout ON THE RENDER THREAD.
   With the host null-routed, each call blocked ~21 s on the OS connect
   timeout (error 10060). Twenty-one-second freezes with five-second gaps
   reads as "the app is frozen"; run.log stops mid-cycle at the moment of
   the Alt+F4.

**Fix (three layers, v0.1122.0):**
- `LinkState { Connecting, Connected, Dropped }` replaces the bool: only
  the network thread's `__CONNECTED__` sentinel proves Connected, only
  failure/close proves Dropped, and the teardown path gates on
  `is_dropped()` so a still-handshaking client is not ripped down early.
- The history fetch moved to a background thread with real timeouts
  (4 s connect / 8 s total), result drained non-blocking via a channel
  (`net_route::chat_history_pump`). A 21 s render stall is now impossible
  by construction, even against a slow-but-alive server.
- The WebRTC bind log no longer dumps the full ~5 KB identity hex every
  reconnect cycle (it was most of a 318 KB run.log by itself).

**Falsifiable tests:** `ws_client::tests::fresh_spawn_is_connecting_not_connected`
(a never-accepting listener holds the client in Connecting; the old code
fails this instantly by claiming Connected) and
`refused_connect_becomes_dropped_never_connected`.

**Lesson:** an "optimistic" status flag is a check that cannot fail wearing
a friendly name. Status booleans must be earned by the event they claim,
never pre-granted; and any network call reachable from the render thread
must carry an explicit timeout, because the OS default is 21 seconds of
frozen UI.

## BUG-071: Federation was seven independent defects deep, each alone fatal (fixed v0.1123.0)

Zero peers ever federated (live DB: 0 federation rows, ever). The 2026-08-12
investigation found three defects; the repair found four more. The full set,
each independently fatal to the handshake or the data flow:

1. **The identify gate ate every inbound hello.** The pre-bind socket loop
   accepted only Identify/IdentifyResponse; a peer relay's FederationHello
   hit `_ => continue` and vanished. Fixed: a dedicated pre-bind arm hands
   the socket to `federation::run_inbound_peer`, which authenticates by
   server key + operator trust tier instead of the user challenge.
2. **The welcome reply went to local chat clients.** `handle_federation_hello`
   pushed FederationWelcome into `broadcast_tx` (the fan-out to signed-in
   users) instead of the peer's socket. Fixed: the handler returns the
   welcome; the peer loop sends it on the socket it owns.
3. **Hello signature preimage mismatch.** Sender signed `"{ts}"`; verifier
   checked `"{ts}\n{ts}"`. Fixed: canonical `"fed_hello\n{server_id}\n{ts}"`
   both sides.
4. **Fed-chat signature preimage mismatch.** Sender signed
   `"{content}\n{ts}\n{channel}"`; verifier expected
   `"fed_chat\n{from}\n{channel}\n{content}\n{ts}"`. Fixed: sender now
   signs the verifier's canonical form.
5. **A URL-added peer could never match a key-identified hello.**
   /server-add files peers under their URL; a hello self-identifies by
   public key; the lookup compared only id/url. Fixed: match by the pinned
   public key too (pinned at add time by the server-info discovery fetch,
   which channel-binds it to the URL the operator chose to trust).
6. **Verify/persist split-brain.** The verifying chat handler only
   broadcast (never persisted); the persisting path (outbound pump) never
   verified. Fixed: one shared `handle_peer_message` for both directions,
   calling the verifying handler, which now persists before broadcasting.
7. **History erased federated lines on restart.** `load_recent_messages`
   filtered `msg_type='chat'`, so the rows store_federated_message wrote
   "to survive restarts" were never loaded again. Fixed: loader includes
   federated_chat.

Also closed in the same arc: empty-signature profile gossip was accepted
(an empty string was a skeleton key over every cached profile; now no
signature, no cache write) and user sockets could inject federation
messages (hello/chat/gossip arms removed from the bound-user loop; those
are server-to-server messages and arrive only on authenticated peer
sockets).

**Proof:** tests/federation_two_relays.rs boots two complete relays in one
process on real localhost sockets and asserts the handshake completes
through the identify gate, a signed chat line replicates and persists
across servers, a Dilithium3-signed profile gossips across, and unsigned
gossip is refused. The test's own development caught defects 6 and 7 live
(green handshake, silent chat leg) plus a test-side check-that-cannot-fail
(set_channel_federated returns Ok(false) on a missing row and .expect()
swallowed it).

**Lesson:** nothing about this code path had ever run end to end, and every
layer had been "finished" separately. A feature whose halves are each
tested but whose WHOLE has never executed once is not dormant, it is
unbuilt; the two-relay test is now the definition of federation working.

**Defect 8, found live after all seven (fixed v0.1127.0):** the first
production deployment handshook both directions but dropped every message
arriving on an OUTBOUND socket. The dialer registered its connection under
the URL it dialed while messages self-identify their origin by public key,
so the source-identity check ("peer sent chat claiming server_id X,
dropped") rejected everything the peer relayed back. The two-relay test
had asymmetric coverage: its chat leg only exercised dialer-to-listener,
the exact direction that happened to work. Fixed: both directions now
register peers under the pinned public key (the dialer resolves it from
the trust row at connect time), and the test gained the reverse chat leg,
which reproduced the live drop red before the fix and is green after.
Live proof 2026-08-14: bot probes crossed BOTH ways between public.guide
and united-humanity.us, persisting as msg_type='federated_chat' with the
correct origin_server key on the receiving side.

## BUG-072: The HUD clock ran exactly lon/15 hours ahead of the sky (fixed v0.1164.0)

**Symptom:** at Silverdale (lon -122.7) the HUD read "20:04" while the sun
stood at 42 degrees due south - local NOON. The realism audit initially
read it as a ~3-hour ephemeris drift; the investigation found no drift at
all.

**Root cause:** the game clock is LON-0 mean solar time BY CONSTRUCTION
(dev_travel::planet_spin_from_time ties the planet spin to it, documented
there since v0.878), but every local-facing surface (the HUD clock, the
showcase "time" pin, and human reasoning about captures) treated it as
LOCAL time at the camera. The disagreement is exactly lon/15 = 8.18 h at
Silverdale. Smoking gun: every lit Silverdale vantage pinned time 20.2
(= local noon + 8.18) and the night vantage 8.2 (= midnight + 8.18) - the
authors had been hand-compensating without knowing it, and the warm-up
vantage at lon 13 (where the offset is ~50 min) never surfaced it.

**Fix:** one global clock stays authoritative (multiplayer needs
observer-independent planet orientation); the local-facing surfaces
convert: the HUD shows local solar time from the frame-lock anchor's
longitude (GuiGameTime.local_hour), and the camera request's "time" is
now the LOCAL hour at the vantage's lat/lon, converted in ipc.rs. The 7
hand-compensated vantage pins migrated to their true local values.

**Lock:** dev_travel::local_solar_time_puts_the_sun_at_the_astronomical_elevation
- the full chain vs the astronomical formula at 4 latitudes x 4 local
hours; the old behavior fails the Silverdale-noon cell by ~62 degrees.

**Deliberately unfixed here:** the model has no axial tilt (eternal
equinox; Season is decorative). Obliquity is its own future increment.

## BUG-073: The flown-camera starburst - a rig that teleports cannot see what a player who flies sees (fixed v0.1239.0)

**Symptom (operator, across v0.1235-v0.1237):** a cardinal-locked
starburst/balloon converging at the feet, visible from inside the cloud
layer and from space, clouds only, "cuts through clouds making them look
flat." Three successive fixes (epipole motion gate, motion floor,
unbiased supersampler) each dissolved a rig-visible artifact and left the
operator's untouched: "Doesn't appear like you actually affected the
thing at all."

**Why the rig was blind:** the rig TELEPORTS - camera.position stays
~30 m and the planet distance rides in the f64 ship_world_pos. The
operator FLIES, and with no floating-origin rebase the whole ~36,000 km
journey accumulates in the f32 camera.position (ulp 4 m). The two states
render differently, and every fix was verified on the wrong one.

**The repro:** ipc camera_request knob `far_frame_km` re-splits the same
absolute pose (camera.position takes the distance, ship_world_pos gives
it back; the frame lock preserves the split). Standing vantage
`starburst-far` = starburst-repro + far_frame_km 36000. Red first try:
murky whole-frame veil + cardinal speckle cross at the nadir.

**Root causes (three, in dominance order):**
1. `dist = render_off.length()` in the celestial draw loop - planet
   distance measured from the SHIP-FRAME ORIGIN, not the camera.
   [CloudRegime] read px=280 mix=0.00 at 4.3 km: the ORBITAL far-map
   cloud regime + starved planet LOD rendered from inside the cloud
   layer. Fixed: camera-relative in f64.
2. The cloud motion delta computed in f32 at planet magnitudes -
   centimeter true motion snapped to an axis-aligned lattice, feeding
   map reprojection, resolve prev_dpos, and the motion gates (a parked
   camera read phantom motion, so temporal history never converged).
   Fixed: f64 end to end, only the final small delta cast down.
3. cloud_resolve.wgsl reprojected via (cam + rd*t) - (cam + dpos),
   rounding two small quantities through the huge camera position.
   Fixed: small form normalize(rd*t_w - prev_dpos).

**Lesson (the general rule):** verify on the STATE THE PLAYER ACTUALLY
REACHES, not the state the rig constructs. Any dev shortcut that places
the camera differently from how a player arrives there (teleport vs
flight, fresh boot vs long session) can hide an entire defect class.
starburst-far is the standing regression; the architectural cure for
the class is a floating-origin rebase (PRIORITIES follow-up).

## BUG-074: The nadir rosette was the cloud family read ninety degrees around the planet (fixed v0.1282.0)

**Symptom (operator, v0.1232.5 through v0.1281.1, reported in nearly every
session for a week):** looking straight down from inside or just above the
cloud layer, a starburst of radial slivers, wedges and bright lines
converging exactly at the feet, fading toward the horizon. Present at
Medium, High and Ultra, absent at Low, and untouched by every F10 toggle:
temporal, dither, shape frame, uniform step, sample-anchored march, the
world-anchored shape, height-varying walls, the thin deck, the wide edge,
every density-component bisect, the in-cloud light, the field warp.

**Cause:** `cloud_march_core` picks ONE cloud regime (family: band base and
top, cover bias, extinction, tint) per ray from the type field at the
midpoint of the ray's segment. The v0.1232.5 horizon-seam fix changed that
midpoint from the CLIPPED segment to the UNCLIPPED top-shell chord so the
family could not flip across the base-shell tangent. For a camera inside
or above the slab looking down, that chord runs through the planet and its
midpoint sits about ninety degrees around the globe, in the pixel's own
screen azimuth. Screen azimuth therefore mapped to far-hemisphere azimuth
and screen radius to angular distance: the 8-19 degree type-noise cells
printed as wedges radiating from the nadir, sectors whose band base lay
above the camera rendered as see-through slivers of ocean, and the pattern
was correct only toward the horizon where the lookup became local again.
Low has no regime, which is why it alone was clean.

**Why fifteen increments missed it:** the CPU twin pins the type, so it
could not see a per-ray lookup; the synthetic checker replaced the density
AFTER the regime gate, so it proved the projection right and was read as
proving the artifact unavoidable; and every A/B toggled a march, dither,
shape or lighting term while the regime line sat outside all of them.

**Fix:** the lookup point is the segment midpoint but never farther than four
slab thicknesses down the ray (`reg_reach`). Down-looks are local; at the
base-shell tangent both neighbouring rays are far longer than the cap, so
the v0.1233 continuity holds (worst jump under the cap, about 0.1 degree
on the sphere, far inside one type cell).

**Proof:** operator-bm12 (lat -11.09, lon -153.95, 1.63 km, straight down,
clock pinned, in-cloud light on), same build, one line changed: inner-bin
radial coherence +0.91 / +0.81 / +0.96 to 0.00 / 0.00 / 0.00, slivers 2 to
0, grain 1.03 to 0.23; the frame turned from the starburst into plain fog,
the honest view from inside the local deck. Found by a fresh read-only panel
(five lenses, two refuters per candidate) whose march-lens reader reproduced
the predicted far-hemisphere pattern offline against the on-disk capture (r
0.66) before anything was rebuilt.

## BUG-075: Clicking a section title did nothing, only its triangle toggled (fixed v0.1314.0)

**Symptom:** on every page that uses `widgets::section_disclosure` (the
inventory's Status and Equipment sections among seven callers), a click on
the section's TITLE text showed the text-selection cursor and left the
section as it was; only the small triangle toggled it. The runtime verifier
saw it on the inventory wall on 2026-09-16 (a `Text` cursor over "Status",
no change), and the first live run of `just verify-screens` hit it when it
tried to collapse Status to bring the Home container into view.

**Cause:** the widget made the whole header row clickable but drew the title
with `ui.label`, and egui labels are selectable by default; a selectable
label takes the press for a text-selection drag, so the row's click never
fired when the press landed on the text. The same defect had just been fixed
on the inventory's container header labels (rung 6 of the screens ladder),
which is how the pattern was recognised.

**Fix:** the title is `egui::Label::new(..).selectable(false)`: a section
title is a control, not prose. Pinned by
`gui::screen_surface::tests::a_section_title_click_collapses_it_and_brings_home_into_view_on_a_wall`,
which clicks the Status title on a wall-sized inventory through the same
event API the look ray uses and asserts the section's Weight card is gone;
with the fix reverted it fails (proven 2026-09-18).

**Lesson:** any egui label that sits inside a click target must be
non-selectable, or the click target is only the part of the row that is not
text. Check for this whenever a "row is clickable" claim is made.

## BUG-076: The F10 sidebar rendered under the HUD and took no input (fixed 2026-09-18)

**Symptom:** the operator, 2026-09-18: "the F10 menu that allows me to
adjust clouds and other stuff pops up but, I can't interact with any of the
stuff inside it." The sidebar drew, the cursor was free, nothing on it
responded to a click. It had been this way since the panel became a LEFT
SIDEBAR on 2026-09-05 (U1); the Window it replaced had worked.

**Cause:** the in-game HUD (`hud::draw`) is a non-interactable egui `Area`
in the Middle order that began with `ui.allocate_rect(screen,
Sense::hover())`, a full-screen widget rect "so the layer has a size".
egui's hit test (`hit_test.rs`, the `included_layers` walk) does not consult
an Area's `interactable` flag and does not care that a rect only senses
hover: it walks widget rects top-down and STOPS at the first one that covers
the search area, so every widget in a Background-order layer underneath is
dropped. A `SidePanel` lives in the Background order. The old F10 `Window`
was a Middle-order Area created AFTER the HUD, so it sat above it; the
sidebar sat below it and could never be clicked. The construction editor's
side panels had hit the same wall in v0.461, which is why lib.rs skips the
HUD in build mode; the sidebar was the first Background panel drawn
alongside the HUD since. `wants_pointer_input()` still read true over the
sidebar (the HUD is skipped by `layer_id_at`, so the panel counted as an
area), which is why the click did not reach the game either: it went
nowhere.

Candidates ruled out on the way: `reconcile_cursor` runs every redraw and
its predicate includes the expanded sidebar (the cursor WAS free);
`egui_consumed` only gates the game's handling, never egui's; the screens'
`route_button` runs only when `!egui_consumed`, so it never saw the click;
the crosshair is a `layer_painter` with no widget rect; the panel is drawn
by exactly one code path (`lib.rs`, the main egui closure); winit passes
F10's WM_SYSKEYUP to `DefWindowProc` only when the window has a native
menu, so Windows menu mode is never entered.

**Fix:** the HUD allocates no rect at all; every element paints at absolute
screen coordinates through the painter, whose clip rect is the whole screen
regardless of the Area's (now empty) size, so nothing visible changed.
Pinned by `cloud_dev::tests::a_sidebar_checkbox_click_flips_its_flag_under_the_hud`,
which draws the HUD and the sidebar in one headless frame, locates a
checkbox by its drawn text and clicks it with the canonical move / press /
release; with the allocation restored it fails (`before == after`, proven
2026-09-18). Its twin `..._when_the_panel_is_alone` passes either way, which
is what localised the fault to the shared frame rather than the panel.

**Lesson:** a full-screen widget rect in a higher egui order is opaque to
every panel below it, even with `Sense::hover` and even in an Area marked
non-interactable. A paint-only overlay must allocate nothing: use the
painter (or a `layer_painter`) and no `allocate_rect`. When a panel "shows
but won't click", list every Area drawn in the same frame and check their
rects before reading the input path.

## BUG-077: `vsync: false` killed the app on its first frame (fixed v0.1315.0)

**Symptom:** with `"vsync": false` in config.json the app died on its FIRST
rendered frame, menu or world (`wgpu_hal::dx12 ResizeBuffers failed ...
0x887A0001`, `surface configuration failed: window is in use`, then
`PANIC ... In Surface::configure / Invalid surface`). Toggling VSync off in
Settings at runtime would hit the same panic and, because the setting is
persisted, every later launch would die the same way. Found by the
2026-09-18 frame-cost measurement, reproduced twice by the runtime verifier
(once without ever entering the world, which is what placed it on the first
frame rather than at world entry).

**Cause:** the settings-apply block runs at the TAIL of the frame arm in
`src/lib.rs`, where that frame's swapchain `TextureView` is still in scope,
and `Renderer::set_vsync` reconfigured the surface right there. DXGI refuses
`ResizeBuffers` while a back buffer is still referenced (wgpu-core's own
comment at the acquired-texture check names this hole, gfx-rs/wgpu#4105),
and wgpu's default fatal error handler ends the process. `vsync: true` never
showed it because the surface already sat in AutoVsync and the old code only
reconfigured on a change.

**Fix:** `set_vsync` records the wanted present mode
(`Renderer::pending_present_mode`) and `apply_pending_surface_config` applies
it at the start of the next frame, before the surface texture is acquired,
when no view of it can be alive. Pinned by
`renderer::vsync_deferral_tests` (the mapping and the change-only rule);
the runtime proof is a rig boot with `vsync: false` in the rig's config that
reaches the world with zero panics.

**Lesson:** never reconfigure the surface from inside the frame arm. Window
resize already runs from `WindowEvent::Resized`, outside it, which is why
resizing never crashed.

## BUG-078: pressing Play threw the player onto the Profile page instead of into the world (fixed v0.1323.0)

**Symptom:** the operator, 2026-09-19: "I tried to log into the game but
instead of loading the game world it threw me into the profile page. I can't
seem to get into game." Play was unusable for the whole life of v0.1322.0 and
v0.1322.1. Reproduced exactly with the dev IPC: boot, click Play at (42, 18),
read back `active_page`. v0.1321.1 returns `None` with `world_loaded: true`
(in the world); v0.1322.1 returns `Real` (the page titled Profile).

**Cause:** v0.1322.0's home redesign put a standing mirror in the bedroom, and
that mirror is a screen. It declares `source: "profile"`, which is exactly the
pattern the home is arranged by: the page shown where a person would go to use
it. In-world wall screens draw real pages through
`gui::dispatch::draw_tool_page`, the same dispatch the main UI uses, against
the same `GuiState` (the screen surface swaps the texture handles, not the
navigation state). And `pages::profile::draw` opened by writing
`state.active_page = GuiPage::Real`, which was its way of aliasing the Profile
page id onto the canonical editor.

So Play loaded the world, the world drew the mirror among the nearest screens
in range, the mirror set the app's page, and the player was back out of the
world before seeing a frame of it. The world stayed loaded and ticking behind
the page, which is why the run log of a failed attempt is indistinguishable
from a normal session: NPCs moving, position updates, a clean save on exit.

**Fix:** a page draw may not steer the app. `profile::draw` now only draws.
The alias is resolved once, in the main UI's own egui frame in `src/lib.rs`,
immediately before the nav bar, where which page the app is on is that code's
business and nothing rendering on a wall can reach it.

**Gate:** `no_page_drawn_on_a_wall_screen_changes_the_app_page`
(`src/gui/screen_surface.rs`) loads `data/machines/home.ron`, takes every
screen the catalog and its instances declare, and for each one that resolves
to a page draws it on a `ScreenCore` and asserts `active_page` is untouched.
It walks the shipped home rather than a hand-kept list, so a screen added
later is covered without anyone remembering the test exists, and it refuses to
pass quietly if the home stops declaring page screens or if the mirror is not
among the ones it checked. Proven red by restoring the old line: it fails
naming `standing_mirror`.

**Lesson:** the moment a rendering surface can show a real page, every page
draw becomes a shared function with two callers, and anything it writes to
global state is written on behalf of both. Nothing static could see this:
the page rendered correctly, the world loaded correctly, the only symptom was
that the game would not start. What found it was driving the real build and
reading one value back.

## BUG-080: the cloud deck changes coverage AND position as you change altitude (FIXED v0.1330.0)

**Symptom:** the operator, descending toward Earth: "As I descend the clouds
become 100% thick. From high orbit they look okay but, as I get closer to the
cloud layer it fills in slowly ... it is very immersion breaking for it to morph
instead of remaining consistent in its positioning." Four screenshots at 2770,
88.7, 71.4 and 47.9 km show steadily growing cover.

**Reproduced on a fixture.** `stormfade-*` holds one column with a storm pinned
and coverage deliberately unpinned, so only camera altitude varies. Mean
luminance in the centre crop: 72.3 at 200 km, 63.3 at 120, 62.3 at 90, 61.5 at
60, then **203.2 at 30 km** and 200.8 at 10 km. At 60 km the HUD reads Storm over
an empty ocean; at 30 km the frame is an edge-to-edge cloud carpet matching his
own 47.9 km shot. A 3.3x change from flying lower.

**Cause:** `src/engine/frame_shells.rs` derives two numbers from the weather
condition (Storm = 0.95 coverage floor, 0.95 placement bypass) and then fades
BOTH by camera altitude: `wx_fade = (1 - (h_km - 30) / 90).clamp(0, 1)`. The
bypass is `params2.w`, which the shader reads as the fraction of the live MODIS
placement to abandon in favour of the procedural field. So descending does two
things at once: coverage climbs toward the storm floor, and the cloud LAYOUT
cross-fades between two unrelated spatial patterns. The second one is the
"morphing" he reported.

**Root cause:** `src/systems/weather.rs` carries ONE condition for the whole
body, with no latitude, longitude or extent. A global fact applied to a visible
hemisphere paints the hemisphere, so the only lever available was to turn it
down as the visible area grows. The ramp was itself the fix for an earlier
report by the same person (v0.1183: "from high orbit the operator watched a
local Storm paint the ENTIRE planet 95% white"). It traded a planet-wide
artifact for a descent artifact; both are the missing spatial dimension.

**NOT the renderer.** The `covladder-*` fixtures pin coverage and type, which
bypasses the weather path, and they stay flat from 20,000 km down to 20 km. The
defect is in how weather reaches the deck, not in how the deck is drawn.

**Not this bug:** the HUD temperature falling from 20C to -8C during the descent
is `temperature_at_player` carrying a real altitude lapse, working as designed.
The condition changing Clear to Storm was the global sim rolling over during the
minutes of flight, not an altitude effect.

**Fix designed, not built:** `docs/design/weather-spatial-extent.md`. Give a
weather system a position and a radius, weight it by distance from the SAMPLE's
ground point instead of camera altitude, and delete `wx_fade`. Blocked on one
new per-frame scalar with no free channel to carry it; the doc records which
pads are actually taken (swizzles hide most of them) and why the uniform
extension needs its own commit with a world-entry boot.

**FIXED v0.1330.0.** Weather stopped being a global scalar faded by camera
altitude and became an environment REGION with a position and a radius
(`docs/design/environment-fields.md`). `wx_fade` is deleted; nothing in the
cloud path reads camera altitude any more. The shader resolves the region once
per ray at the point where the ray ENTERS the cloud slab and uses it for both
the coverage floor and the placement weight.

Measured on `stormfade-*`, one column, storm pinned, only altitude varying:

| altitude | before | after |
| --- | --- | --- |
| 200 km | 72.3 | 200.6 |
| 120 km | 63.3 | 201.9 |
| 90 km | 62.3 | 202.5 |
| 60 km | 61.5 | 202.5 |
| 30 km | 203.2 | 202.7 |
| 10 km | 200.8 | 201.0 |

A 1 percent spread across the whole descent, against a 3.3x cliff before. The
orbital guard holds too: 12,000 km is unchanged at 22.1 and 2,000 km moves
106.1 to 107.8, so a local storm does NOT whiten the marble and the v0.1183
artifact the altitude ramp existed to prevent has not come back.

**Two bugs were found on the way and are worth keeping:**

1. The anchor is sticky by design, so the rig pinned the weather and THEN flew
   to the vantage, leaving the storm at the spawn point. A dev teleport now
   re-places the weather, because a teleport is not travel.
2. The ray's ground point must be the slab ENTRY, not its closest approach to
   the body centre. For a nadir ray the closest approach IS the centre, so the
   normalize was of a zero vector. Every nadir fixture read exactly zero
   influence while the CPU instrument reported 1.0, and it was that
   CONTRADICTION that localised it. The 1 Hz `[EnvRegions]` line in
   `frame_shells.rs` exists for exactly this and stays.

## BUG-079: clouds from orbit rendered as black-and-white static at every quality (PARTIALLY fixed v0.1326.0, REOPENED 2026-09-20)

**Symptom:** the operator, flying above the Earth: "the clouds look weird in
that they're awfully dark and white in spots to the point of looking like old
TV static ... Up close the clouds look better but, far away they look weirdly
dark." He then produced the A/B that settled it: five screenshots from one
camera at 752 km, cloud quality off / low / medium / high / ultra. Off is
clean. Low is blurry with straight tile-boundary lines. Medium, high and ultra
are per-pixel black/white/grey noise. **Ultra looked and performed the worst of
the five**, at 10 fps.

**Cause:** a cloud edge in this renderer is two hard things - a carve hinge
0.005 noise units wide on the noise body (`cloud_carve` in `40-clouds.wgsl`)
and a 90 m rind on the constructed body (`cv2_density_tail` in
`41-cloud-bodies.wgsl`). One pixel at 873 km covers about 700 m, so both sit
far below a single sample. A pixel landing on an edge is therefore a coin flip,
and from orbit nearly every pixel lands on an edge. The mean is right and every
individual pixel is binary, which is what static IS.

That also explains the quality ladder being INVERTED: Ultra does not march more
finely, it asks for finer clouds at the same sampling rate, and detail below
the sampling limit can only alias - while costing more to produce.

**Fix:** both edges widen with the sample footprint
(`cloud_edge_foot_ramp`): 1x below a ~170 m footprint, 3x above ~700 m, so a
sample standing for a mixture of cloud and gap returns the mixture. The
constructed body's skirt is on by default. Both were already in the tree as dev
experiment bit 6; the experiment was right about what to do and wrong about
scale, applying full width at every distance, which is why it never became the
default.

**Three things the A/B ladder settled** (fixtures committed in
`tests/visual/vantages.json`, all runnable with
`node scripts/probe-sweep.js --only <id> --operator-config`):

- `orbit-873-carve-x3` / `-x6` / `-x10`: three times is enough. The three are
  indistinguishable and x20 flattens the deck into a sheet.
- `orbit-873-carve-only` / `orbit-873-skirt-only`: **both edges must move
  together.** Widening either alone leaves the static exactly where it was,
  because the orbital deck mixes both body models and whichever one stays hard
  keeps flipping.
- The ramp must read the UNCLAMPED footprint (`lodb`), not a mip level.
  `cloud_lod` clamps at 0 and BOTH ends of the range land there: 12 km altitude
  gives a 25 m footprint and 873 km gives 726 m, and both read as lod 0 after
  the clamp. The first attempt gated on the mip and did nothing at all.

**Verified** at the operator's own settings, panics 0: 873 km ultra clean,
2000 km much improved (speckle remains near the limb and on some coasts - still
open), 12 km unharmed, forest ground unharmed.

**Known and documented in the shader:** the skirt is NOT footprint-ramped.
Publishing the ramp from `cloud_v2_body` did not survive to the density tail -
the static came straight back with it in place, while the same build with the
skirt unconditional was clean - and the per-ray body cache is the likeliest
reason. It does not need the ramp today (the near-field guard is
indistinguishable either way) but that is the first thing to try if a close-up
vantage ever disagrees.

**Lesson, and it cost hours:** when a change that should be arithmetically
identical to a working experiment does nothing, stop reasoning and force the
whole effect on with NO conditions to confirm the look first. That proved the
model in one build. Then add one condition at a time, capturing between each,
and the broken one names itself. Every wrong turn came from deriving a
footprint instead of measuring one, and from changing two things per build.


**REOPENED 2026-09-20, and the cause above is only half of it.** The edge
widening did its job on the coverage channel: at 2000 km the marched alpha now
comes back clean. The static did not go with it. Captured at the operator's own
graphics settings, `approach-2000km-high` (High is the DEFAULT tier, so this is
what every user sees) shows cloud masses with correct, coherent silhouettes
whose interiors are per-pixel black-and-white noise.

Bisected with the screen-path channel instrument in
`assets/shaders/pbr/45-cloud-temporal.wgsl`, which renders one raw ingredient of
the march as greyscale THROUGH the same resolve and composite as content:

| channel | vantage | result |
| --- | --- | --- |
| 1, coverage alpha | `orbit-2000-high-mask` | clean, coherent, crisp edges |
| 3, ambient luminance | `orbit-2000-high-amb` | clean, flat even grey |
| 2, direct-sun luminance | `orbit-2000-high-sun` | GRAINY, the carrier |

Also refuted, each built and captured: the sun-shadow CACHE
(`orbit-2000-high-nolight`, `cloud_light 0`, indistinguishable from baseline)
and the per-pixel march DITHER (`orbit-2000-high-nodither`, `cloud_dither 0`,
indistinguishable). What remains inside channel 2, per its own doc: sun taps,
powder, cavity-on-direct.

Note for whoever picks this up: the quality-ladder inversion explained above is
still true and still unfixed at Ultra, but it is a SEPARATE defect from the
static on the default tier. Fixing Ultra's coverage collapse would not have
touched what the operator reported.

## BUG-081: coastlines glowed cyan on the night side of the planet (FIXED v0.1331.20)

**Symptom:** from orbit, with the sun nowhere in frame, every shallow shelf on
the dark half of the Earth was traced in pale cyan: the Bahamas bank, the
Yucatan shelf, the Gulf coast. Deep ocean and land were correctly black. The
operator reported it across several releases, and the report that finally made
it reproducible was "I'm still able to see it even when on the dark side of the
Earth without the sun visible."

**Why it survived four earlier fixes.** Each was a real defect of the same
shape, a light term with no local sun gate, and each was fixed correctly:

| earlier fix | what it gated | why it did not end the glow |
| --- | --- | --- |
| water sky mirror (`w_lut_day`, 20-surface-detail) | the camera-centric sky LUT reflected by water | the water shell is transparent over the shelf |
| BUG-057 #3 (`underwater_apply`) | the water-column in-scatter, by `sun_direction.w` | that value is a GLOBAL, and the celestial pass stamps it at 2.5 |
| ambient floor (`under_sky`, 80-fragment-shared) | the silhouette floor on planets | the floor was not what lit it |
| fill gate (v0.1186) | the fixed cool fill light on type 12 | nor was the fill |

The second row is the one. It looked fixed, and the comment beside it said
"no sun, no glow", but the number it multiplied by is one value for the whole
frame. The celestial pass that draws planet terrain writes it as a hardcoded
2.5 (the TERRAIN TERMINATOR GATE note in `80-fragment-shared.wgsl` records
this), so the night half of the planet read as full daylight.

**Cause:** `underwater_apply` adds a navy in-scatter
`(0.008, 0.030, 0.055) * sun_day` to every seabed fragment below sea level,
weighted by the water column's opacity. With `sun_day` stuck at 1.0 it lit the
seabed at midnight. It traced coastlines because the opaque ocean shell hides
the seabed over deep water and fades out over the shallow shelf, which is
exactly where the seabed shows through.

**How it was found**, since four rounds of reasoning had not: a fixture that
reproduces it (`coast-night-2000`, local midnight over the Bahamas bank,
clouds off) and elimination, scoring blue-dominant pixels inside the planet
disc (5.066 percent baseline). Unchanged by the ocean shell painted black in
both branches, `sky_ambient` zeroed, the ambient floor removed, the fill light
zeroed, the water emissive zeroed, and terrain albedo plus emissive zeroed
together. Gone (0.004 percent, disc mean 2.717 to 0.190) with the in-scatter
alone zeroed, which matched painting the entire terrain entry black.

Two traps worth keeping. First, a magenta positive control could not have
failed: magenta saturates red and blue, so a `blue > red` classifier can never
count a cyan overlay drawn on top of it. Painting BLACK is the non-saturating
version and is the one that proved ownership. Second, the first whole-frame
metric was dominated by the Milky Way at the frame edge; restricting to a disc
fitted from the terrain itself is what made every later number mean something.

**Fix:** the in-scatter is gated by the fragment's own sun elevation, taken
from the sea sphere the function already reads, through the same
`TERRAIN_TERMINATOR_LO/HI` band that lights the seabed beneath it, so the
column and the seabed go dark together. No call site changed.

**Verified** before and after at identical settings (66 of 66 match):

| vantage | before | after | pixels changed |
| --- | --- | --- | --- |
| `coast-night-2000`, midnight | L 3.84, 5.589% blue | L 0.88, 0.011% | 13% |
| `coast-noon-2000`, same camera at noon | L 73.73 | L 73.73 | 0.014% |
| `shore-limb-0630`, terminator in frame | L 13.61 | L 13.58 | 0.6%, all night-side |

A change map of the dawn frame shows the changed pixels confined to the dark
side, with no seam along the terminator.

**Do not re-flag:** a black night side with no cyan is correct. The Earth has
no light of its own on a moonless night apart from city lights and airglow,
neither of which the renderer draws yet.

## BUG-082: nothing in the build menu could ever be built (FIXED v0.1339.0)

**Symptom:** every blueprint in `data/blueprints/basic.ron` (foundations,
walls, door, window, roof, crafting table, furnace, chest, bed) was refused
in play with "need 6x wood to build Wood Wall" and the like, however much
lumber, stone or iron the player carried.

**Cause:** the catalog named its materials `wood`, `stone`, `iron`,
`silicate` and `fiber`. None of those is an item id; the items are
`wood_plank_0`, `stone_brick_0`, `iron_ingot_0`, `glass_pane_0` and
`fiber_bundle_0`. Since v0.746 the ConstructionSystem consumes real
materials by item id, so a count of "wood" was always zero.

**Why the tests did not see it:** the build tests stock the player with the
blueprint's OWN material ids before building, so a fake id passed through
them untouched. The evidence was the setup, not the game.

**Fix:** the 15 material references mapped to the real items, each craftable
from an existing recipe (planks from logs, bricks from clay or cut stone,
ingots from ore, glass from sand, fiber from plants).
`every_blueprint_material_is_a_real_item` reads the shipped `items.csv`
and fails on any blueprint material that is not an item; it was run red on
the old catalog first (15 missing references, every blueprint).

**Found by:** reading what `Structure.provides` feeds while persisting
builds for offline progression (2026-09-25).

## BUG-083: tanning turned one hide into two hides, forever (FIXED v0.1351.0)

**Symptom:** `tan_leather` took one `leather_hide_0` and salt and gave two
`leather_hide_0`, the same item, so every tanning doubled the hides.

**Cause:** there was no separate item for tanned leather in the recipe; the
output reused the input id.

**Fix:** tanning now turns one raw hide into one `leather_0` (tanned
leather, 35% of the raw hide's mass per UNIDO's leather mass balance), and
16 leatherwork recipes take `leather_0`. `tests/byproduct_use_lint.rs`
fails any recipe that hands back more of an input than it takes (four older
multipliers are allowlisted with reasons; the list may only shrink).

**Found by:** the byproducts rung's mass-balance review (2026-09-26).

## BUG-084: a worn tool came back new after being stored (FIXED v0.1351.0)

**Symptom:** putting a worn hammer into home storage and taking it back gave
a fresh one: tool wear (v0.1348.0) did not survive the backpack/storage
transfer.

**Cause:** the transfer carried only an item id and a quantity, and the
storage pool's `PlacedItem` had no wear field.

**Fix:** the transfer is a `TransferOp` carrying wear and grade both ways,
`PlacedItem` stores them, `Inventory::add_item_worn` / `remove_worn` keep
the picked stack's wear, and storage entries only merge when wear and grade
match. `a_worn_tool_keeps_its_wear_through_storage` was run red both ways.

**Found by:** reviewing the tool-wear rung the same day it shipped.

## BUG-085: 13 crafting stations could be crafted but never placed (FIXED v0.1357.0)

**Symptom:** in the one-person home about 160 recipes could not be made: the
forge, anvil, stove, oven, electronics bench, sewing machine, loom,
chemistry set, composter, sawmill, grain mill, fuel refinery and water
purifier never existed, although `build_*` recipes crafted each one.

**Cause:** the `build_*` recipes produced station ITEMS that nothing could set
down; the station gate counts placed home machines and built structures.

**Fix:** a blueprint per station that consumes the crafted item
(`data/blueprints/basic.ron`); `recipe_sources_lint`
`every_recipe_station_can_be_built` fails on a station no blueprint builds
(red on the old catalog, naming all 13). Built electric stations draw home
power since v0.1358.0.

**Found by:** the gameplay gap survey (2026-09-25).

## BUG-086: the water pump offered "Drink" (FIXED v0.1354.0)

**Symptom:** the inventory's Drink button appeared on the water pump, the
water purifier, the water tester, empty bottles and shampoo.

**Cause:** the button guessed from the item id prefix `water_` and from the
subcategory "liquid" instead of the food data.

**Fix:** the buttons ask `food::consume_kinds`, built from the same
`item_profiles.ron` and `food_system.ron` the food system uses.
`consume_kinds_says_what_is_drunk_what_is_eaten_and_what_is_neither` was
run red first.

## BUG-087: NPC shops sold 16 items that do not exist (FIXED v0.1354.0)

**Symptom:** `data/npcs.ron` shop lists named `seed_bag_0`, `torch_0`,
`poultice_0`, `ale_0`, `engine_gasoline_0` and others with no items.csv row.

**Cause:** shop lists were written before or apart from the item catalog, and
nothing checked them.

**Fix:** mapped to the real items (or removed where none exists; the glass
flask became a real item); `recipe_sources_lint` `npc_shops_sell_real_items`
fails on any shop item that is not in items.csv (it failed on the real data
first). `data/tech_tree.ron` is left out on purpose: the game does not read
it yet and it names future items by design.

## BUG-088: eight defects in the day's crafting and storage work, found by review (FIXED v0.1360.0)

An independent review of v0.1345.0..v0.1359.0 (a read-only critic agent that
reproduced seven of them against the crate) found:

1. **Saving wiped the wear and grade of everything carried.** The save kept
   only (id, count). Now `WorldSave.inventory_state` keeps each stack's
   (wear, grade) in order and the restore puts each stack back as it was.
   Test `carried_tools_keep_their_wear_and_grade_across_a_save`.
2. **Power shedding flip-flopped every tick,** so a craft could start without
   power and the "paused" notice repeated. `ElectricalSystem` counted a load
   shed last tick as 0 W, so it fit and switched back on. It now counts every
   load's draw, and a station with a craft on it keeps its working demand
   while shed. Tests `a_shed_load_stays_shed`,
   `a_stove_short_of_power_pauses_once_and_stays_paused` (both systems ticking).
3. **The vendor bought defective goods for 0 CR and sold whichever grade was
   last.** `vendor_sell` now sells one named grade at its price, refuses
   defective goods, and the Sell tab lists each stack with its grade and price.
4. **Filling a jerrycan was refused with room in the pack:** the empty's
   volume was counted twice. Filling is now a swap.
5. **Storing a tool in a machine vessel (the Tool Rack) renewed it:** a
   vessel keeps a count, not each tool's wear and grade. Durable goods are no
   longer offered to vessels, and machine outputs skip vessels for them.
6. **Putting away one grade could take another** when the grade spanned
   several stacks. `remove_worn` takes every matching stack first.
7. **A pack with every slot full lost the empty bottle after a drink.** A slot
   is made for it.
8. **A restored automated batch ignored its machine's power,** holding the
   stand-in entity it was restored with. Held and busy checks find the machine
   by id. Test `a_restored_batch_holds_on_its_own_machines_power`.

Each test was run red by undoing its fix first.

## BUG-089: crops sown from the Garden panel's bed Plant button were never drawn (FIXED v0.1364.0)

The bed/tray/field Plant button tagged each crop's `tower_id` with the machine
TYPE ("staple_grain_tray") and numbered the units 0..count. The plant renderer
(`src/engine/home_meshes.rs`) places crops by tower design id or machine
INSTANCE id, found neither, and skipped them, so the crops grew, were
harvestable in the panel, and were invisible in the world. The showcase
auto-seed tagged instances, which is why the showcase garden always showed.
Found while making grow lights reach only nearby machines, which needs the
same thing the renderer does: where each crop stands. Fix: the farming system
expands a TYPE into its machines ("grow_instances", published by the engine)
and sows one crop per plot of each, tagged with the machine. Test
`planting_a_bed_type_sows_every_plot_of_every_machine`, red with the
expansion switched off.

## BUG-090: the gameplay sun and the drawn sun disagree aboard the home station (FIXED v0.1397.0, 2026-09-28)

Found by a read-only survey while preparing seasonal daylight for the garden.
The home is a station in equatorial geosynchronous orbit (`data/stations/home.ron`:
`period: Synchronous`, `inclination_deg: 0.0`, phased over Silverdale). Two
separate suns run the game:

- GAMEPLAY: `solar::sun_factor(hour)` is a fixed 6:00 to 18:00 arc on the
  global game hour. Solar panels, crop light, the grow-light timer
  (`farming::lighting::lamps_on_at`) and `farming::DAYLIGHT_FRACTION` all read it.
- DRAWN: the sky's sun is the real Keplerian ephemeris on the WALL clock
  (`src/lib.rs` ~9395, `sun_rel_earth_m`), at a permanent equinox
  (`dev_travel.rs` "eternal equinox"), lighting the hull through
  `station::to_hull`.

Aboard, the deck's local solar time is about the game hour + 3.85 h + the Sun's
real ecliptic longitude / 15 (derived, not yet measured), so on 2026-09-26 the
deck's noon falls near game hour 20, when `sun_factor` says night, and the
offset drifts through a full day each real year. A player can see the sun up
while the crops and panels are in the dark, or the reverse. Not measured in the
running game yet: confirm with a rig capture at game hours 12 and 20.

Also found: the gameplay latitude aboard is a default 45 degrees
(`body_environment::REFERENCE_LATITUDE_DEG`, used only for temperature), the
data files size the home as a GROUND site at Silverdale 47.6 N
(`data/home_outline.json`, `data/world/spawn.ron` `homestead_orbit`, parsed by
nothing), and the station is really at 0 degrees. Which of these the home is
decides whether it has seasons at all (at equatorial GEO the sun is up about
12 h all year), so the fix waits on that decision (docs/PRIORITIES.md).

**Fixed 2026-09-28**, after the operator settled the day (24 hours by default,
configurable, an hour always an hour) and put the spaceship first. The survey
was right about the effect and wrong about one cause: the drawn Sun already
turned the planet on the GAME clock (`dev_travel::planet_spin_from_time`,
since v0.878), but the planet's spin is tied to the sun's azimuth, which
creeps a full turn a year, while `station::orbit::propagate` placed the home by
the game clock alone. So the longitude below the home was `-122.3 - azimuth +
180` degrees: right in late September, a full turn out by the next. Two parts:

1. **The home hangs over its longitude on every date.** `orbit::over_its_longitude`
   adds `azimuth - 180` degrees to a synchronous orbit's phase, from the same
   `engine::frame_lock::sun_azimuth` the spin uses. The home's noon is now at
   20:09 on the game clock (longitude 0's time) all year, which is where the
   pinned home vantages already put it.
2. **The home reads its own time.** The engine publishes the home's longitude
   (`HOME_LONGITUDE_KEY`, from `orbit::hang_longitude_deg`), and the solar
   panels, the grow lights' timer and the crops' sunlight read
   `GameTime::solar_hour_at` there; the HUD aboard and the wake notice show the
   home's time, as the HUD on a planet already showed the place's.

Test `a_synchronous_station_hangs_over_its_longitude_on_every_date` checks, at
four sun azimuths, three days and seven hours, that the home is over -122.3
and that the deck (LVLH) has the sun above it exactly when the home's clock
says the sun is up; seen red by propagating without the re-phase.

Closed after the fix, same day: a panel at a planet build site uses its own
site's longitude (`solar::site_longitude_deg`, v0.1413.0),
the weather's day and night warmth follows the hour where the player is
(`weather::local_solar_hour`: the player's longitude on a world, the home's
elsewhere; test `the_day_warmth_follows_the_players_own_hour`, seen red by
reading the game clock's hour, v0.1414.0), and the F11 panel's hour
slider already reads the HUD's local hour, which the fix set aboard the home.
Still open: the latitude question (the station is at 0 degrees, the
temperature reference at 45, the data at Silverdale's 47.6) waits for ground
farming.

## BUG-091: plot areas were never published on a fresh boot, so every plot counted one plant (FIXED v0.1369.0)

v0.1364.0 added `publish_grow_plots` (the grow machines, their positions and plot areas, for the
grow lights, the per-plot plant count, soil pH buffering and pollination) and called it from
`rebuild_machine_objects` only. A fresh boot records the grow machines in
`engine::world_load` and never runs that rebuild, so until the player first edited a machine
the DataStore had no "grow_plots", "grow_instances" or "grow_plot_area_m2": every plot
harvested, fed and watered as one plant, grow lights lit nothing, and the bed Plant button fell
back to TYPE-tagged units. Every unit test inserted its own maps, so none could see it. Found by
the plot-drawing work (2026-09-26) when its first after-photo showed no change. Fix:
`world_load` publishes the plots as soon as it records the grow anchors.

## BUG-092: nine defects in the day's garden releases, found by review (FIXED v0.1373.0; the last item closed by the one clock, v0.1395.0)

An independent review of v0.1363.0 to v0.1372.1 (a read-only critic agent that
drove the shipped crate from a scratch test package) found:

1. **Every crop ripened at (n-1)/n of its growth_days,** 17% early for a
   six-stage tomato, against plants.csv ("days from planting to harvest"), the
   light model's doc and the food model. `stage_from_progress` now reaches the
   last stage at progress 1.0 and the soil uptake follows. Test
   `a_crop_ripens_at_its_growth_days_and_not_before`.
2. **Pest controls charged per crop entity,** not per plant: a 128-plant bean
   field was hosed with 2 L and sprayed with one bottle. Now per plant. Test
   `a_control_is_charged_for_every_plant_in_the_plots`.
3. **The grow-light timer lit the plots but the light drew 100 W around the
   clock;** the editor's meter assumed 14 h. `GrowLight` carries its watts and
   the tick sets the draw from the timer; the meter charges the timer's 6 h.
   Tests `a_grow_light_draws_power_only_while_its_timer_has_it_on`,
   `the_power_meter_charges_the_timer_hours`.
4. **Weeds on an emptied bed** prompted "hoe them", then the hoe was refused
   and no row showed. An empty bed can be hoed or mulched bare and is listed.
   Test `an_emptied_bed_can_be_hoed_and_is_shown`.
5. **The showcase stagger ignored the 10x growth speed,** so nearly every
   staggered crop spawned ripe. Ages now run on the growth clock.
6. **Hand-planted crops were held to a soil pH no one could change.** A crop
   with no grow unit is not pH-modelled. Test
   `a_hand_planted_crop_is_not_held_to_soil_ph`.
7. **OPEN: room air and the water tanks run on clocks 72x apart.** The
   humidity balance (and the crops' breathing) runs on game hours; the tanks
   are billed per real day (irrigation, humidifiers). Mass is not conserved.
   This is the same question as the body clock against the garden clock and
   waits on the operator's decision (docs/PRIORITIES.md). **Mass half fixed
   2026-09-26 (ship life support):** the air now condenses the water back and
   every flow crosses between the clocks as litres a day on both sides, so a
   day's water balances on each clock (`the_gardens_water_balances_across_the_two_clocks`).
   Which clock is right, and so how long a garden day lasts, was answered by
   the operator on 2026-09-27 (a 24-hour day, an hour always an hour) and
   built as ONE game clock in v0.1395.0: the room air and the crops run on
   game hours, and the irrigation and humidifier litres are billed per game
   minute (`irrigation_demand_lpm`), so both sides are on the same clock.
   CLOSED 2026-09-28.
8. **Later picks are rescaled by season health at each pick,** though the code
   comment said they were not. The behaviour is kept (a drought during the
   window lowers the picks after it) and the comment corrected.
9. **The rubber tree was felled at its only harvest** after six years. It is
   now tapped every two days for 20 years (harvest_windows.ron, Britannica).

Also: the legume, tuber and oilseed fields sowed wheat from the Plant button
(own grow media now), and the mushroom rack's Humidity slider did nothing (the
tent humidifier holds the air; the slider is gone). Each test was run red by
undoing its fix first.

## BUG-093: threaded AV1 decode aborted the lib test run; rav1d over-borrowed at the frame's left edge (FIXED v0.1374.0)

**Symptom.** `just verify` intermittently aborted the whole lib test process
(1 in 5 full runs on 2026-09-25), hiding every other result:
`thread 'rav1d-worker-6' panicked at ...\rav1d-1.1.0\src\disjoint_mut.rs:837:13:
overlapping DisjointMut: current: & _[1566..1578] ... cdef.rs:207:28, existing:
&mut _[800..1568] ... cdef_apply.rs:56:22`, then "panic in a function that
cannot unwind" and exit 0xc0000409 (STATUS_STACK_BUFFER_OVERRUN).

**Which tests.** The only non-ignored tests that decode AV1 with rav1d's
worker threads are the `engine::screens::video` tests: the screen opens its
player with `VideoPlayer::open`, which asks rav1d for its automatic thread
count (one per logical processor, 12 here), and cargo runs several of them at
once. The `media::tests` pin one thread.

**Reproduction (2026-09-27, debug test build).** `engine::screens::video`
at 12 test threads: 1 abort in 150 runs, the exact signature above. A new
ignored stress, `media::tests::rav1d_threaded_decode_stress` (six decoders at
once, automatic threads, 40 decodes each): 8 aborts in 20 runs, every one the
same two-element overlap (e.g. `& _[3102..3114]` against `&mut _[2336..3104]`).

**What was actually wrong: rav1d 1.1.0, not our use of it.** Our wrapper
(`src/media/video.rs`) drives each context from one thread, releases every
picture before returning and unrefs every packet. The overlap is inside rav1d:
`padding` in `src/cdef.rs` copies the two rows above a block from the CDEF
line buffer by borrowing columns `0..x_end`, starting two pixels left of the
block. At the frame's left edge (`HAVE_LEFT` is cleared for every block at
`bx = 0`, `cdef_apply.rs`) it reads only columns `2..x_end`, but it still
borrowed the two before, which in the line buffer are the last two elements of
the previous superblock row's region; with postfilter threading another worker
can be writing that region at the same time (`backup2lines`, cdef_apply.rs:56,
the `&mut [800..1568]` whose end is the reader's row start at 1568). The same
over-borrow was in the bottom-row copy. So no pixel was read while written (the
decode is byte-identical threaded and single-threaded), but a shared borrow of
memory another thread holds mutably is undefined behaviour in Rust, and a
release build keeps it: the checker is compiled out, not the overlap. rav1d
1.1.0 is the latest release; `main` had the same code and the issue tracker no
report of it on 2026-09-27. The 2026-09-17 note in docs/design/media-player.md
had called it "rav1d's instrumentation, not this code" and the release build
unaffected; the first half was right, the second was not.

**Why not single-threaded tests or a single-threaded product.** Tests on one
thread would hide it while the shipped build kept the overlapping borrow, and
the product cannot drop rav1d's threads: 1080p decodes at 28.4 fps on one
thread against 69 with them (docs/design/media-player.md).

**Fix.** `vendor/rav1d`: the published 1.1.0 (minus the assembly our build
never compiles) with `padding` borrowing exactly the columns it reads,
`x_start..x_end`, for the top and bottom rows; wired through
`[patch.crates-io]` in Cargo.toml. vendor/README.md records the change and how
to return to crates.io once upstream fixes it. Upstream not yet told (a report
is a public post; the operator's call).

**Verified (2026-09-27, the vendored crate in this checkout).**

| Run | Runs | Aborts | "overlapping DisjointMut" |
|---|---|---|---|
| `engine::screens::video`, 12 test threads | 150 | 0 (published crate: 1) | 0 |
| `rav1d_threaded_decode_stress` | 20 | 0 (published crate: 8) | 0 |
| the whole lib suite | 5 | 0 | 0 |

At the published crate's stress rate, twenty clean runs would happen by chance
about 4 times in 100,000. New test `threaded_decode_is_bit_identical_to_single_threaded`
(not ignored): the fixture decoded with automatic threads is byte-for-byte the
single-threaded decode, hash `bc2eee527943655c` with the published crate and
with the patched one. The two "run it in release" notes on the audio-device
tests are gone.

**Found alongside, not this bug.** In the 150 screens runs, 4 failed
normally (no abort), all in
`engine::screens::video::tests::a_looked_at_or_paused_clip_draws_and_keeps_input_while_an_unwatched_one_drops_it`;
the 150 runs on the published crate had 2 such failures. A test that assumed a
paused clip can deliver no new frame: BUG-094, fixed.

## BUG-094: a video screen test assumed a paused clip delivers no new frame (FIXED v0.1374.1)

**Symptom.** `engine::screens::video::tests::a_looked_at_or_paused_clip_draws_and_keeps_input_while_an_unwatched_one_drops_it`
failed now and then under load, with no abort: 4 in 150 runs of the screens
tests at 12 test threads (2026-09-27), then 1 in 150 on a rerun, and 2 in 60
with six copies of the tests running at once. The rerun's message:
`unwatched, nothing new: no draw`, left `Compose { px: (1280, 720), upload:
true, strip: false }`, right `Keep`. `upload: true` means a newly decoded
frame had arrived. With six copies the same test also failed one step earlier,
on `the paused draw uploaded what there was to upload`.

**What was wrong: the test, not the player.** The test pauses the clip and
then expects a tick with nothing new to draw. Pausing stops the player's
CLOCK, not its decode thread. On a loaded machine the decoder can still be
behind the position where the clock stopped, so frames at or before that
position keep arriving after the pause and are due. Handing them out is right
(a paused film should show the picture at the paused position), and this is
the draw the test saw. The earlier assertion was wrong as well:
`frame_dirty` is cleared only by the GPU upload, and the test's draw helper
has no GPU, so "false after the paused draw" held only while no frame had
arrived at all.

**Fix (test only, plus one test-only accessor).** After pausing, the test
rewinds the paused clip to its first frame (`seek_to_start`), so where it
stops no longer depends on how long the first half took. It then waits until
the decoder has got past the paused clock: the new `VideoPlayer::decoder_past_clock`
(`#[cfg(test)]`, `src/media/mod.rs`) is true once a queued frame is ahead of
the clock or the decoder has finished. From then on no frame can fall due. The
test now stands in for the upload its GPU-free draw cannot do
(`frame_dirty = false`), and asserts that the paused clip has its first
picture in hand instead.

**Verified (2026-09-27).** Screens tests at 12 test threads: 0 in 150 runs failed.
Six copies at once: the target test failed 0 in 60 (the old test: 2 in 60) runs. Still able to fail:
with the drop removed from `plan_frame`'s Keep path, the backlog assertion
reads 26 events, not 0.

**Found, left alone.** With six copies at once (72 test threads on 12 logical
processors), two real-time tests fail on their wall-clock floors:
`frames_arrive_at_the_clip_size_in_pts_order_and_loop` (36 and then 51 of 60
runs; its floor is 30 of the clip's 59 frames in 2.6 s, and at that
oversubscription the player drops frames by design) and
`notices_and_the_paused_picture_lay_out_at_the_display_size` (2 of 60; it
gives the first frame 2 s to arrive). Neither failed at the normal test run's
load: 0 in 450 screens runs and 5 whole-suite runs on 2026-09-27.

## BUG-095: every boot played a looping 440 Hz tone from the workshop screen (FIXED v0.1376.0)

**Symptom (operator, 2026-09-27).** "The screen that's playing the constant
high pitch buzz. That's very distracting every single time a fresh instance
boots up, especially when I'm watching a movie or playing some other game."

**Cause.** `wall_screen_5` in the workshop (`data/machines/home.ron`) plays
`media/demo_colour_bar.webm` on loop, and that clip's audio is a 440 Hz test
tone (`data/media/README.md`). A video screen attached its sound on the first
tick that had a player and an audio device, so the tone started on every
boot. That included every instance an agent or a rig launched in the
background, which already opened without taking focus but still made sound.

**Fix, two layers.**
- A video screen starts MUTED, the rule browsers apply to autoplaying video.
  The strip shows Unmute on a clip with a sound track. No stream is attached
  until the first Unmute; a Mute after that keeps the stream and sends
  silence. The state is not saved, so every boot is quiet
  (`engine/screens/video.rs`, `VideoProvider::sound_on`).
- An instance launched in the background by a script opens no audio device
  (`lib.rs`, the same `launch_focus::launch_in_background()` decision that
  keeps it from taking focus). The operator's own launches keep their sound.

**Found on the way.** `mix_moved`, the gate that keeps a still listener from
re-sending the mix every frame, held a Mute from a quiet mix: 0.0012 to 0.0
sat inside its 0.004 epsilon, so the silence was never sent. Going into or
out of silence now always counts.

**Verified.** New test `a_screen_starts_muted_and_unmute_turns_its_sound_on`
clicks the real Unmute button through egui; it was seen red with the screen
starting unmuted. The audio-device test now also checks that nothing attaches
before Unmute and that Mute sends silence; it runs on this machine and
passes. The 28 screen tests pass.

## BUG-096: the water tanks lost every small flow at frame rate (FIXED, ship life support, 2026-09-26)

Found by the clock-boundary balance test while building ship life support
(docs/design/ship-life-support.md). `PlumbingSystem` integrated each island's net
flow into its tanks in f32, one frame at a time. A tank's level near 8,000 L has
a float step of about 0.0005 L, and any change smaller than half of that rounds
to nothing. At 60 frames a second that is any net flow under about 0.9 L/min: a
household tap's 0.17 L/min (0.00005 L a frame) never left the cistern, and a
garden whose draw and return nearly cancel left the level frozen. The "days of
water left" the home showed drained far slower than its flows said. Fix:
`plumbing.rs` integrates in f64 and carries what rounding keeps out of the tanks
to the next tick, per island, so every litre that crosses lands; what a full or
dry tank cannot take or give is still spilled or unmet. Test
`a_trickle_reaches_a_big_tank_at_frame_rate` (an hour of 0.17 L/min at 60
frames a second takes 10.2 L), seen red by dropping the carry.

## BUG-097: seams in the v0.1377 and v0.1378 garden batch, found by review (FIXED v0.1379.0 and v0.1381.0)

A critic review of the batch (911b9fc3..b20b24d7), made because systems built
side by side disagree where they meet. It found nine; seven are fixed here.

**Fixed in v0.1381.0 (on the mushroom-CO2 branch, which owned those files).**
Loads are now fed critical first, a load drawing nothing is never shed, and
every air machine writes the draw it would take each step, so a shed one
comes back; a tent breathes only the blocks planted, at each species' rate.
The two defects as found:
1. HIGH: the power system switched the air handlers and the CO2 scrubber off
   whenever the grid was short, every night. A 0 W consumer (Station-supplied
   mode, or an idle handler) always fails `remaining >= draw && draw > 0`, and
   an unpowered unit never asks for power again. The tests never ran the power
   system. Related: `electrical.rs` feeds consumers priority-descending, so a
   "priority 1, shed last" load is shed first.
2. MEDIUM: a mushroom tent breathed for all ten blocks however many shelves
   were planted.

**Fixed here:**
3. Taking a row cover off just before harvest cancelled the whole pollination
   penalty (`harvest_set` read the cover at harvest). A field crop with a
   record now keeps it, and its flowers after the cover comes off count as
   pollinated.
4. With pests Off, a cover left on stayed invisible and unremovable but kept
   cutting the fruit set. Pollination now ignores covers with pests Off, as
   warming already did.
5. A row cover on an indoor bed said it shut the bees out, but the hive still
   pollinated it. A cover now removes hives and devices from the areas under
   it, and the flowering notice under a cover says what works there.
6. Taking a cover off recounted its pieces from the ground covered then, so
   pieces appeared or vanished on the hand-planted area. The pieces laid are
   now recorded (`AreaPests::cover_pieces`) and handed back exactly.
7. Moving a rack in the editor left its tent, plants and published plot where
   it had stood. The editor's fast path now moves grow anchors and replants
   only the moved machines.
8. Any launch not started from Explorer (Steam, a .bat, a terminal) was
   silent with no way back. The first click into such a window now opens the
   audio device; an agent's rig, never clicked into, stays silent.
9. The pest notice offered a row cover on towers and racks and greenhouse
   natural enemies on outdoor fields; it now offers only what can be used
   there. The rack's shelves sat from 1/5 to 5/5 of its height, so the top
   shelf's mushrooms pushed through the tent roof; shelves now start just
   above the floor with each shelf's spacing free above it.

Each code fix has a test seen red on a deliberate break.

## BUG-098: dry mushroom caps drew as tree bark (FIXED v0.1386.0)

Found by a read-only lighting review on 2026-09-27. The procedural plant mesh
tags each face with an organ in spare UV bits, and a face with no bit takes
the shader's bark branch. The mushroom blocks (v0.1385.0) put every dry cap
(shiitake, button) and its stem, the bare shiitake block, the bed's compost and
casing, the X-cut and the filter patch on that untagged `Stem` organ. Bark
fissure cells are tens of centimetres across, so a whole 5 cm cap landed inside
or outside one crack and single caps drew at about 0.4 of their colour, one
darker than its neighbour for no reason. Fix: a fourth organ, `Organ::Plain`
(bit 21), matte with a fine mottle at the object's own scale
(`90-fragment-main.wgsl`, the PLAIN MATTE TISSUE branch), used for all of
those; only the bed's wooden tray keeps the grained wood look. Tests
`fungus_nothing_but_the_tray_is_shaded_as_bark` (seen red with dry caps back
on `Stem`) and `plain_organ_bit_rides_alone_and_keeps_the_color`.

## BUG-099: a video test counted frames against the wall clock and failed under load (FIXED v0.1387.0)

`frames_arrive_at_the_clip_size_in_pts_order_and_loop` (src/engine/screens/video.rs)
watched the demo clip for a fixed 2.6 s and wanted 30 frames. With five agent builds
running beside `just verify` on 2026-09-27, only 15 arrived, twice in one afternoon:
`poll` drops late frames by design, and a starved decoder is late. The player was
right and the test measured the machine. It now runs until it has what it checks
(30 frames, and frames from after a wrap), with a 30 s deadline, so a quiet machine
still finishes in about 2.1 s and a loaded one simply takes longer. The defects the
count exists for (a decoder that never wakes, a stale generation) deliver a handful
and never reach 30, so they still fail, at the deadline. Same class as BUG-094.

The same afternoon `synthetic_tile_samples_through_the_global_grid`
(src/terrain/terrain_tiles.rs) reported "tile never arrived". It gave its loader
thread 5 s, and it wrote its tile to a FIXED path under the system temp directory,
`hos_tile_test`, which every test process on the machine shares: with several
worktree agents running `cargo test` at once, another run could rewrite or delete
the file mid-read. Which of the two it was is not known, so both are fixed: the
deadline is 30 s, and that test and twelve others that wrote fixed temp paths
(machines.rs, persistence.rs, ship/home_structure.rs) now put the process id in
the name, so concurrent runs never share a file.

Later the same day, with the suite taking twice its usual time beside several
agent builds and a GPU session, two more failed and passed alone:
`notices_and_the_paused_picture_lay_out_at_the_display_size` (a frame within a
fixed 2 s) and `a_real_frame_survives_the_whole_pipeline_to_a_viewer` (the
publisher checked for a connection after a fixed 400 ms, and one frame sent
once after a fixed 150 ms settle, which can go out before the viewer is
registered). Both now wait for their condition up to 30 s, and the live test
offers the frame every 200 ms until the viewer has one (v0.1392.0).

## BUG-100: seams in the sleep and offline batch, found by review (FIXED v0.1387.0, before release)

A critic review of the built beds and chests work and offline progression rung 2,
merged side by side, found no high defect and four others, all fixed before either
shipped:

1. While asleep the clock runs at 120x, and crafting, mining and farming follow it
   through `time::scaled_dt`, but the power grid and the animals ran on raw frame
   time. A night slept cost the batteries and the genset 1/120 of the night while
   the crafts they powered ran all of it, so sleeping erased the night's power
   deficit, and hens laid nothing overnight. `ElectricalSystem` (fuel and battery
   charge) and the livestock yield timers now follow `scaled_dt`; movement and the
   log cooldown stay on real time. At normal speed nothing changes. Tests
   `a_fast_clock_drains_the_bank_by_game_time` (red: 1.37 Wh drawn where 166.7
   were due) and `yields_ripen_on_the_game_clock`.
2. A sleep in progress survived loading another save (ESC > Play or Characters):
   the clock stayed at 120x for the loaded character, or a character whose clock
   was ahead woke "rested" without sleeping. A clock outside the night (before it
   began, or more than 60 game seconds past its end) now ends the sleep unrested
   and puts the clock back. Test
   `a_clock_that_jumps_out_of_the_night_ends_the_sleep_unrested`.
3. The saved storage pool used an empty list to mean "this save never wrote one",
   so a home whose containers were all emptied, saved, then stashed into and
   re-applied kept the live pool beside the rewound backpack, doubling the stashed
   goods (a path character select now takes). `WorldSave.placed_items` is an
   Option: None keeps the seeded default, an empty list comes back empty. Test
   `a_pool_saved_empty_comes_back_empty`.
4. Two tests were weaker than their comments: the toggle-off test promised the
   drone stays where it was with no drone in the save, and the save round trip
   never went through JSON or carried the standing order. Both now do.

## BUG-101: parallel straight stripes across the ocean sun glint from 55 km (FIXED v0.1389.0)

Seen at fixture `deck-55-nadir`: the glint was crossed by a grating of
parallel diagonal stripes 18 px apart, one wavelength of the 850 m train (the
first report guessed 2 km). Cause: every analytic wave train
bends its crests with a domain warp, and a v0.1020 perf gate switched the
whole warp off below about 24 px per wavelength, on the assumption that the
anti-alias fade had already blurred the train away by then. It had not: the
fade (`detail_octave_fade_aa`) keeps a train visible down to 9 px, so between
9 and 36 px each train was drawn with dead-straight crests. From 55 km that is
the 850 m train (proven by a same-boot arm with that train removed: the
grating vanished); from about 105 to 280 km the 2 km train does the same.

Fix (`20-surface-detail.wgsl` `wave_octave`): the COARSE warp now runs
wherever a train is drawn; only the fine warp keeps the perf gate. Measured in
one boot with `scripts/ocean-stripe-metric.mjs` (share of the glint's detail
energy in its strongest spectral bin): 0.59 to 0.05 at 55 km, 0.21 to 0.03 at
150 km; the 700 m sea view differs from the old shader no more than the old
shader differs from itself a few minutes later; no measurable cost (under
0.1 ms of `gpu.celestial_t`). Rust mirror `renderer::water::wave_warp`, test
`a_visible_wave_train_never_draws_straight_crests`, which also evaluates the
old law and asserts it was straight.

## BUG-102: built pieces and the shelter test used the wrong frame off the ship (GATED v0.1390.0; FIXED v0.1393.0 with planet build sites)

Found by a critic review of the shelter commit (961745f7) on 2026-09-27, before
it shipped. Built pieces live in the HOME frame and are drawn at the station
offset, but placement (`engine/build_place.rs`) and the shelter test
(`engine/survival_env.rs`) used the raw camera position. Aboard, where the two
frames agree, that is right, but aboard there is no weather: outside the home
is vacuum. On a planet, where the weather is, walking moves the ship frame and
not the camera, so a piece placed from the surface was drawn at the station
hundreds of km up, and the HUD said "Sheltered" wherever the player walked in
the rain. The root is older than the shelter commit (the old build path and
the bed and chest use test took the raw camera position too), and the
commit's tests could not see it: every one runs in a flat Y-up world.

Gated in v0.1390.0: a piece in hand places only while `aboard_station` (the
flag that already gates the home's walls, floors and elevators), with a plain
hint off the ship, and the shelter test runs only aboard, so nothing claims
shelter where it cannot be true. The real fix, pieces anchored to the planet
they stand on, is its own increment, with the review's other findings: a
building bigger than one roof tile never counts as sheltered (the wall search
reaches only 0.5 m past the one roof overhead); `OnTop` pieces can land on a
lower storey; the machine and door "[E]" prompts still show while E would
build; nothing stops two pieces being built in one spot; small overlaps float
a piece; and the build pose is recomputed at the key press rather than taken
from the ghost.

**Fixed 2026-09-27: planet build sites.** A piece placed on a planet now
stands in a BUILD SITE (`systems/construction/site.rs`): the body it stands
on and an origin in that body's unrotated frame, in f64, with a flat tangent
frame (Y the local up) that follows from the origin alone. The piece's
Transform is site-local and it carries a `PlanetSite` component; a home piece
has none. A new piece joins the site of the nearest piece already standing
within 1 km of it (and that site's metre grid), and the player is at the site
of the nearest piece within 1 km. Every query that compares pieces (what a
piece rests on, the shelter, the look ray, a duplicate) runs in one frame
only. The ghost, the build pose, the shelter test, the stations and the bed
and chest look ray all convert the player into that frame from the frame
lock's anchor (the eye in the body's frame) and the camera's look
(`engine/planet_build.rs`, read from the engine in one place, `PlayerView`),
and the aim meets the drawn ground under the crosshair
(`placement::aim_point_on_ground`, sampling the same surface the walk clamp
stands the player on). Each frame a site piece is drawn at `render_off + rot *
p`, with the very `render_off` and `rot_d` the celestial loop places that
body's terrain with (one binding, handed to `note_body`), in the celestial list
only, at every distance: a hill hides it, it casts the sun's shadow, and
whatever stands in front of it stays in front. The celestial pass's near plane
is 5 cm (`renderer::camera::CELESTIAL_NEAR_M`), so a wall beside the eye is not
cut open. The save carries the site. The v0.1390.0 gate is gone; building still refuses, with a
plain hint, in open space, in a vehicle, while flying, and on water. The
review's other findings, fixed with it: roofs that touch are one covered area
(`uses::covered_run`), so an 8 x 8 m hall under four tiles shelters every
point inside; OnTop rests only on pieces standing on the player's own storey
(`placement::STOREY_STEP_M`); the machine, door, talk, vehicle and livestock
"[E]" prompts hide while E would build; a second build of the same box, or of
one still going up, is refused before anything is spent; overlaps under 5 cm
no longer lift a piece; and E builds the ghost's pose, carried on the build
request. The survival context's "inside the home" test also now requires the
home frame, since on a planet the parked camera's local position could sit
inside the home's box. Tests (each seen red by a mutation that was run): the
planet-fixed draw while the ship frame moves, the planet shelter following the
player, the site round-tripping a save, the 8 x 8 hall, the storey filter, the
duplicate, the ghost's exact pose, the site frame's f64 exactness, the ground
aim, the 5 cm overlap, the frame filter.

**The review of that fix (2026-09-27), all fixed before it shipped.** (1) HIGH:
the first version drew a second copy of every site piece within 2 m of the eye
in the scene pass, because the celestial pass's near plane was 1 m (its corner
reaches 2.27 m from the eye at 90 degrees vertical and 16:9, not the 1.8 m first
written here). The scene pass clears depth, so the copy was painted over
everything drawn only in the celestial pass: a chest inside the hut vanished
behind its own wall, and a wall's buried base painted over the grass on a
slope. The near copy is deleted; the celestial near plane went from 1 m to 5 cm,
one shared constant used by every site that builds or reads that projection
(the frame loop's culling frustum, the god rays, the SSAO, the cloud composite,
the emission pass, the tests). Reverse-Z into a float depth buffer keeps the
same relative precision whatever the near plane is (depth = near / distance),
so no range gains z-fighting (tested at every range from 0.5 m to the Sun, and
compared in the rig from orbit and at the limb); the two shaders that tested
depth against a fixed 1e-7 for "sky" now take the threshold from the projection.
(2) Overlapping sites: membership followed the nearest site ORIGIN, so a hut
900 m from its origin was ignored once a second site started 200 m from it.
Membership now follows the pieces (above). (3) Home-frame leaks now reachable
on a planet: a stove or oven built on Earth was wired onto the orbiting home's
power island, a station built on a planet counted for crafting aboard, and a
planet build took materials from the home's storage. Now a planet station is
on no grid until its own site makes power (it reads "no power: nothing at this
site makes power, and the home's grid is in orbit" on the Crafting page),
station availability follows where the player is (the home's machines and
built stations aboard, the site's built stations on a planet, none elsewhere),
and a planet build takes only what the player carries, with a hint under the
crosshair saying what is missing. (4) The dev `build` showcase verb has no dev
gate, like every other showcase verb; its doc comment now says so. (5) Tests
the review asked for: the chest in front of its wall (routing plus depth order,
and the planet-built-inside rig vantage, which stands the eye inside an open
hut with a chest and a furnace in front of the north wall), note_body handed
the terrain's own `rot_d`, the player's frame following the anchor and not the
parked camera, and "inside the home" requiring aboard. Each new test was seen
red by the mutation its comment names.

## BUG-103: rectangular blocks in the open-sea colour seen from orbit (FIXED v0.1392.0)

Seen at fixtures `deck-55-nadir` and `ocean-glint-150km`: hard, straight-edged
blocks in the regional sea colour. They were seams along the lattice lines of
`value_noise`, which `sea_var` in `ocean_shell` sums over three octaves. The
old `value_noise` hashed its corners with the FLOAT `hash21` of
`i + vec2(1.0, 0.0)` and so on. Two neighbouring cells share a corner, and the
shader compiler is free to round the two routes to it differently (for
example by folding the `+ 1.0` into hash21's first multiply for one cell). At
lattice coordinates in the thousands, which every planet-scale caller reaches
(sea colour 700 to 3,500, shore 70,000, the finest land octave 800,000), one
ulp is enough for the float hash to return a different number, so the cells
disagreed about the corner they share and the field stepped at every line.

Proven three ways. (1) On the real GPU, in a compute test that runs the
shipped WGSL (`lattice_noise::device_tests`, DXC and FXC alike): the old
`value_noise` stepped at 12,203 of 32,768 lattice-line crossings, worst step
0.98. (2) In one boot through the shader hot reload, with a debug arm that
paints `sea_var` (red) and a lattice-cell ID (green), scored by the new
`scripts/lattice-seam-metric.mjs` (mean step across a lattice line over the
mean step elsewhere; a smooth field reads under 1): 3.34 at 55 km and 1.68 at
150 km, the repeated old arm 3.36 and 1.68. (3) Integer corners feeding the
SAME float hash also read 0.83 and 0.81, so the mechanism is the corner route,
not the hash.

Fix (`10-lighting-patterns.wgsl`): `lattice_hash(vec2<i32>)`, pure integer (an
odd-constant combine and the lowbias32 finaliser, an exact 24-bit conversion),
and `value_noise` floors once, converts to `i32` and names every corner by an
integer add. Measured after: 0.83 (99th-percentile step across a line 12
levels before, 1 after) at 55 km and 0.815 (14 to 3) at 150 km; on the real
render at 150 km, scored on the same lines, luma 1.59 to 0.86 and hue 1.52 to
0.85. No measurable cost (`gpu.celestial` and `gpu.celestial_t` within the
same-boot spread at `ocean-700m`, `ocean-glint-150km` and
`sahara-noon-ground`). The CPU twin moved from `clouds` to
`renderer::lattice_noise`: 0 bit mismatches against the GPU over 24,576
probes up to 8,000,000, where the float `hash21` mismatches the GPU on 25% of
integer inputs, so no float-hash twin could ever be exact. Tests: the
shared-corner property to 8,000,000, the red check
`the_old_float_hash_fails_the_shared_corner_test`, known answers, the WGSL
text pin, statistics at every magnitude, and the repeat vector (52,932 cells).

Patterns: everything built on `value_noise` re-rolled with the same
statistics: sea colour, shore depth noise and surf, the wave-crest warp, land
detail, the gas giant band wobble and the material textures that use
`value_noise` or `fbm`. The same float corner route existed in `voronoi`,
`voronoi_edge`, `cloud_noise` (hash13) and `micro_noise` (a sin hash); those
now take integer corners converted to the identical float values, so their
patterns did not change (cloud pixels in the same boot differ from the old
shader no more than the old shader differs from itself). Look checks, old
against new in one boot: the 55 km glint stripe gate did not move (share3 0.083
against 0.078 to 0.088 on the old shader), sand and ocean from 700 m kept their
character, and a straight seam across the Bahama Bank shallows (a custom
`bahamas-bank-shore` camera, 0.6 km) is gone.

## BUG-104: flat rings on every dark gradient (8-bit banding) (FIXED v0.1398.0)

Reported 2026-09-24 while fixing the aurora: its faint glow drew as nested
ellipses with crisp outlines. The cause was not the aurora. The scene rendered
straight into the 8-bit display format, each pass tonemapped in its own shader,
and nothing dithered before the 8-bit write, so any slow dark gradient (the
night sky, the atmosphere's limb and twilight falloff, dusk terrain, fog)
quantised into flat bands one display code apart, which the eye reads as hard
edges. v0.1331.21 dithered the aurora alone (`srgb_dither`) as a stopgap.

Fixed by the HDR scene target (`docs/design/hdr-scene-target.md`): every pass
draws into an `Rgba16Float` target (increment 3), and the present pass adds ONE
triangular dither before the 8-bit write (increment 4), replacing the aurora's
own and `srgb_dither`. Measured in one boot per vantage (section 7 of the
design doc): the mean run of one identical code in the dark parts fell at every
banding vantage (aurora-over-land-dark 7.1 to 2.3 pixels, night-horizon 4.8 to
1.8, shore-dawn 2.6 to 1.7), the share of dark pixels in long flat runs fell
from 53 to 10 %, 70 to 27 % and 54 to 9 % there, and the means and the aurora
comb held. GPU test `the_dither_breaks_every_band_of_a_dark_ramp_and_keeps_the_mean`
guards the ramp; `scripts/band-census.js` is the measure.

## BUG-105: the HUD's Air bar warned on breathable open air (FIXED v0.1420.0)

**Symptom:** standing on Earth's ground, the HUD showed a yellow "Air" bar at
full, as if the air were running out. Found by the first rig capture of a
player on foot (`planet-open-noon-walk`, 2026-09-28): every earlier capture
was taken in fly mode, which suspends survival and shows the indoor line.

**Root cause:** `hud::vital_rows` showed the Air row, in warning colour,
whenever the player was not sealed, a rule written when "outside" always
meant vacuum. Since open air became breathable on a world with breathable
air (artificial-planet increment 4), "outside" and "cannot breathe" are no
longer the same thing, and the row kept the old meaning.

**Fix:** `GuiVitals.breathing` carries the survival context's `oxygenated`,
and the row keys on it: shown even at full only while the air cannot be
breathed; in breathable open air only when the player is short of breath.
Test `vital_rows_show_what_needs_attention` gained the breathable-ground
case, seen red by keying the row on `sealed` again.

## BUG-106: web chat refused to connect on phones without WebCrypto Ed25519 (FIXED v0.1435.2)

**Symptom:** a user in Nigeria (2026-09-30), on an Android phone, got
"Post-quantum identity could not be initialized. This client cannot connect
without it" on united-humanity.us/chat, every time. Hardware was not the
cause.

**Root cause:** the chat identity is a 32-byte seed (Dilithium3 and Kyber768
derive from it), but the seed was only ever kept inside a WebCrypto Ed25519
key. WebCrypto Ed25519 is on by default only from Chrome 137 (2025), and
many phones run an older Chrome or a Chromium browser built on one. There
`getOrCreateIdentity` fell back to a key with no private half, and
`attachPqIdentity` refused at its first check. The PQ library itself needs
only Chrome 85 (it uses `||=`), so every Chrome from 85 to 136 was shut out
by this alone.

**Fix:** a SEED-ONLY identity (`web/chat/crypto.js`): where Ed25519 is
missing the seed is held directly and saved in the same PKCS8 backup the
Ed25519 path reads, and `restoreKeyFromLocalStorage` works out the public
key when a backup has none, so a browser that later gains Ed25519 keeps the
same identity. Every seed read goes through `identitySeed()`: the PQ
derivation, recovery phrase, backups, passphrase wrap, sync key and contact
card. Checked in the browser pane with Ed25519 switched off: connects with no
alert; the same Dilithium key after "updating" to Ed25519; the same key
restored from the recovery phrase and from a backup file on both kinds of
browser. Also fixed on the way: importing any backup made since the PQ
cutover failed (it demanded a 64-character public key; those carry the
3,904-character Dilithium one), and the unused `ed25519PublicKeyHex` field
could be overwritten with the Dilithium key when setup ran twice. Lost
without Ed25519: the Solana wallet's public key only.

## BUG-107: the off-box backup copied the same 24 August file for 39 days (FIXED v0.1436.0)

**Symptom:** none visible: `backup-pull.log` said "pulled OK" every run.
Found by the week-plan survey (2026-10-02) and confirmed: all 60 copies in
%USERPROFILE%\HumanityBackups were byte-identical, the 24 August state.

**Root cause:** since 2026-08-24 the VPS seals every 30-minute snapshot to
`relay-<ts>.db.aes` and deletes the plain copy, but
`scripts/backup-relay-from-vps.ps1` still listed only `relay-*.db`, so it
kept finding the last plain file (the VPS keeps 15 plain ones forever because
no new ones arrive to push them out). Its only check was a valid SQLite
header, which the stale file passes: a check that could not fail.

**Fix:** pull the newest stamped `.db.aes`, check the `Salted__` header,
and FAIL loudly (non-zero exit, ERROR in the log) when the newest VPS snapshot
is over 2 h old or stamped in the future (exit 7), when the copy's SHA-256
differs from the VPS file's (a pull taken mid-seal, exit 8), or when it is
byte-identical to the previous pull (exit 6). Each failure was produced on
purpose before trusting it; a real run then pulled a current snapshot whose
hash matches the VPS. The old plain copies (15 on the VPS from 23 and 24
August, 60 stale local ones) were deleted on 2026-10-02 at the operator's word,
the VPS ones with shred.
**Still needed, operator only:** a copy of `/opt/Humanity/data/backup.key`
off the VPS; without it no pulled backup can be opened.

## BUG-108: a Settings button could replace a person's identity in one click (FIXED v0.1436.0)

**Symptom:** web Settings > View Recovery Phrase, when it could not show the
words, offered "Rotate Key", promising a "dual-signature certificate" would
carry profile and messages over. One click made a brand-new identity,
overwrote the stored one and reloaded; the old recovery phrase no longer
matched and the relay refused the saved name.

**Root cause:** the overlay was written for old non-extractable keys, the
promise has been false since 2026-03-25 (the certificate step was removed)
and the relay's rotation route since v0.265.0. It also appeared for HEALTHY
identities whenever the BIP39 word list failed to load.

**Fix:** the button and its handler are gone. The page now says why the
phrase cannot be shown (the word list or the identity tools did not load:
reload; or this browser holds no recovery phrase: use the restore actions)
and changes nothing. Checked in the browser pane on all three paths. The
recovery phrase also lost its seedling icons for a key (CLAUDE.md, account
words).

## BUG-109: the main Windows download had no models or textures (FIXED v0.1436.0)

**Symptom:** a new Windows player got machines drawn as boxes and flat
ground, with no warning.

**Root cause:** `web/pages/download.html` picked `HumanityOS-windows-x64.exe`
(75 MB, the program alone, which exists for the updater) instead of the zip
(about 385 MB) that carries models, textures and data. Mac and Linux already
picked their zip.

**Fix:** the Windows button now prefers the zip (the exe only as a fallback
for old releases), the Windows card says to Extract All and not to run the
exe from inside the zip, and the sizes are current. Checked in the browser
pane: the button resolves to the v0.1435.2 zip.

## BUG-110: upvoting a bug on the website failed for everyone (FIXED v0.1436.0)

**Symptom:** "You need to be logged in", signed in or not.

**Root cause:** since 2026-09-06 the relay requires a Dilithium-signed vote
(`voter_key`, `timestamp`, `sig` over `bug_vote\n<ts>`), but
`web/pages/bugs-app.js` sent only `voter_key`, from a key the page never
had.

**Fix:** `bugs.html` loads `/chat/pq.js` and `/shared/pq-relay-auth.js`, and
the vote is signed with `getPqSignedAuth('bug_vote')`, which reads the
recovery-phrase backup without needing WebCrypto Ed25519 (so it also works on
the BUG-106 phones). The shape was checked against the relay's own
verification code.

## BUG-111: the website's Tasks page never signed in on its live socket (FIXED v0.1436.0)

**Symptom:** creating a task timed out; status, priority and assignee
changes showed on screen but never reached the server.

**Root cause:** `web/pages/tasks-app.js` identified with the old Ed25519 key
and had no answer to the relay's Dilithium identify challenge, so the relay
never bound the socket and dropped everything it sent.

**Fix:** the page identifies with the Dilithium identity from the recovery-
phrase backup and answers the challenge; its local-only test votes are
labelled as local and no longer carry a decorative Ed25519 signature.

## BUG-112: closing one of two tabs signed the person out everywhere (FIXED v0.1436.0)

**Symptom:** with web Chat (or the desktop app) and another page signed in
as the same person, closing the NEWER one ran the full departure: the person
showed as left, was dropped from voice, and their game connection went to
the link-dead grace period, while the other stayed open. Fixing BUG-111 would
have made this happen every time someone closed the Tasks board.

**Root cause:** the relay keeps one registration per identity, owned by the
newest socket, and ran its departure cleanup whenever that socket closed.

**Fix:** `RelayState::live_conns` counts every signed-in socket per
identity. Closing one that is not the last hands the registration to a
socket still open and tidies nothing else; a second socket for someone
already here is not announced as a new arrival; and a socket that signs in
without its DM key is announced with the key on file (web Chat also keeps a
known key over an empty one), so no open Chat loses the ability to send them
DMs. Test `closing_one_of_two_sockets_keeps_the_person_signed_in` runs the
real relay and the real handshake; seen red without the fix.

## BUG-113: desktop trades never moved items, and the Trade page never listed any (FIXED v0.1437.0)

**Symptom:** a finished trade changed nothing in the desktop backpack; the
desktop Trade page always said "No trades"; raw `__trade_data__:{...}` lines
appeared in chat. Found by the review of v0.1428.0 to v0.1436.0.

**Root cause:** the relay sends every private message as `type: "system"`
(relay.rs, the send loop), but the desktop client handled the trade wrappers
only in its `"private"` arm, which never runs. The v0.1433.0 tests called
`settle_completed` directly, so they could not see it: a check that could
not fail.

**Fix:** one router, `trade::route_trade_frame`, called from both arms, and
a test that feeds the frame the client really receives (a serialized
`RelayMessage::System` around a real `TradeData`), seen red.

## BUG-114: confirming a trade did not hold the offered items (FIXED v0.1437.0)

**Symptom:** after confirming, a player could use or store the offered items;
when the trade completed the partner still received them, and the giver was
told they "left your backpack".

**Fix (client side; true escrow needs server-held inventories):** every
frame, with the page open or not, a confirmed offer the backpack no longer
covers is re-sent, which makes the relay clear both confirmations, and the
player is told why. Settling takes only what the backpack still holds and
says so honestly; the inventory step reports a shortfall instead of
ignoring it.

## BUG-115: a player away when a trade completed never settled their side (FIXED v0.1437.0)

**Symptom:** the completion notice is sent once; a player offline or
reconnecting missed it, and nothing settled their side later.

**Fix:** a trade settles whenever a completed record reaches the client (a
trade update, the trade list, or the notice); settled ids are kept with the
save (`TradeSettlements`, `WorldSave.settled_trades`); the list is requested
once per connection, page open or not. A second review found that a trade
settled at the main menu was lost when Play then loaded the save: every
frame now settles any completed trade the loaded world does not know, sized
against the loaded backpack, announced once. On a server with the game
switched off, the automatic request's refusal is no longer printed in chat.
Known gap: settled ids live per save, so a completed trade can replay into
another home of the same identity.

## BUG-116: quitting the game with a web tab open left the avatar and the voice seat behind (FIXED v0.1437.0)

**Symptom:** a regression from BUG-112: the game and voice departures waited
for the identity's LAST socket, so quitting the desktop game while web Chat
stayed open left the avatar in the shared world and the person in voice.

**Fix:** the relay records which socket holds the game seat and which holds
the voice seat (`LiveConns`, `relay/handlers/live_conns.rs`) and gives those
up when THAT socket closes. Because a reconnecting voice client's new socket
signs in before the old one is noticed closed, both clients now re-send
their voice join once the new socket is accepted, and the relay moves the
seat to it without a leave and join; the departure is decided under the
voice lock so a re-join cannot slip between. Real-relay tests for each case,
seen red.

## BUG-117: a socket that signed in without a name showed the person as "Anonymous", or under an old name (FIXED v0.1437.0)

**Symptom:** the web Tasks tab signs in without a name; the person showed
nameless to others. The first fix fell back to the oldest name a key ever
registered (undoing renames), and the second trusted the member row even for
a revoked device, which would have let a revoked laptop come back as its old
owner's name.

**Fix:** a nameless socket keeps the live registration's name, or the
person's current name: the member row's name only while that name is still
registered to the key, else the newest registered name (`Storage::
current_name_for_key`). Status text is cleared on disconnect under the name
the session used. Tests for a rename and a revoked device, seen red.

## BUG-118: the clip maker left the game running after Ctrl+C (FIXED v0.1437.0)

**Fix:** `scripts/make-clips.js` kills the game's process tree on Ctrl+C,
Ctrl+Break or a closed console (exit 130), notices the game exiting
mid-shot, and cancels a shot it gives up on (`debug/record_cancel.json`, which
finishes the file cleanly). A second record request is refused without
touching the running one's done file; ffmpeg's own error text is kept; clips
are tagged and converted as BT.709. Tests in `scripts/tests/make-clips.test.js`
(in `just rig-tests`).

## BUG-119: stall roofs and machine ducts were see-through from below (FIXED v0.1437.0)

**Root cause:** a raised part skipped its underside whenever ANY part ended at
its height, so a roof on posts and a duct on its riser lost theirs.

**Fix:** an underside is skipped only when the part beneath covers this one's
whole footprint (a machine on its plinth). Also: a narrow stall or cradle
footprint could panic (`f32::clamp` with min above max), and the
faces-point-out test now checks each face points away from the box centre.

## BUG-120: other players' heads sat inside their bodies, and the figure floated (FIXED v0.1437.0)

**Fix:** `net_route::remote_figure_parts` builds the figure up from the floor
under the eye: the body box from the feet to the shoulders, the head on top,
the hair over the head, all scaled by the player's height. lib.rs builds the
meshes from the same constants. (The crew figures still have the old fault:
see PRIORITIES.)

## BUG-121: a long server message could crash the desktop app (FIXED v0.1437.0)

**Root cause:** a debug preview cut every system message at byte 300
(`&raw[..300]`), which panics when byte 300 falls inside a multi-byte
character (an accented name, an emoji). Found by the trades fix agent.

**Fix:** cut at a character boundary (`frame_ws_poll::clip`); test seen red
with the old slicing.

## BUG-122: the 30-minute VPS backup stopped pruning, so sealed copies piled up (FIXED v0.1439.0)

**Symptom:** `humanity-backup-db.service` showed failed (status 2) every run
from 2026-10-03 04:14Z, and `/opt/Humanity/backups` grew past its 15-copy cap
(16, then 17, about 48 more a day). Each snapshot was still written and sealed.

**Root cause:** the script runs under `set -euo pipefail` and pruned with
`ls -1t relay-*.db | tail -n +16 | xargs rm`. When the last plain
pre-encryption copy was deleted (2026-10-02, at the operator's word), that ls
matched nothing, exited 2, and ended the script before the `.db.aes` line.
Caused by our own cleanup: deleting the last file a glob matches broke a
script that assumed one would always exist.

**Fix:** rotation through a `rotate_keep` function using `find`, which never
fails on no match; the count can be set with `HUMANITY_DB_BACKUP_ROTATE_KEEP`
(default 15). `scripts/tests/backup-rotate.test.js` (in `just rig-tests`) runs
the function under the script's shell options in a folder of sealed copies only,
and runs the old line there as a control that must fail.

## BUG-123: the relay's 6-hourly snapshot, and the mail expiry with it, restarted at every deploy (FIXED v0.1439.0)

**Symptom:** on a server deployed more often than every 6 hours the relay
took no snapshot of its own (`data/backups/*.db.enc`, the only copies its
crash recovery reads) and never ran the expiry pass that shares the loop:
sealed mail past its TTL and public messages past the retention window stayed.

**Root cause:** the loop slept a flat 6 hours from every start.

**Fix:** the first pass is due when the newest existing snapshot turns 6 hours
old (at least 2 minutes after start; at once if there is none),
`storage::backups::first_snapshot_wait`; test seen red.

## BUG-124: the in-app backup status and list showed no backups, and "Back up now" left a readable copy (FIXED v0.1439.0)

**Symptom:** after the old plain copies were deleted, the Relay Control "Last
backup" row and the Backups panel showed nothing, though 16 sealed copies
existed; `provision-vps.sh` would have failed its "a backup was written" check
on a fresh server. The "Back up now" button wrote `manual-<ts>.db` in the clear,
and nothing ever rotated it.

**Fix:** `storage::backups::is_backup_file` counts `.db`, `.db.aes` and
`.db.enc` for the panel and the status row; provision checks `relay-*.db*`; the
button seals its snapshot to `manual-<ts>.db.enc` with the key beside the live
database and removes the plain copy. Tests seen red.

## BUG-125: other players moved in stop-go steps and froze on any late update (FIXED v0.1440.0)

**Symptom:** another player's figure jerked forward in small steps instead of
walking, and stood still whenever an update came late.

**Root cause:** `src/net/sync.rs` eased each received position in and out over
50 ms (`smooth_step`), so the figure stopped and started on every update, and
the desktop always sent a velocity of zero, so there was nothing to carry the
figure across a late one.

**Fix:** snapshot interpolation, the way real-time games draw other players.
Each update carries the sender's own steady clock in `timestamp` (seconds,
real time; 0 means none) and its real velocity. The receiver keeps a short
buffer per player, places updates on the sender's clock plus a learned offset,
draws each player `INTERP_DELAY_S` in the past at constant speed between the
two updates around that moment, walks on along the last velocity for at most
0.25 s when the buffer runs dry, blends back without a jump, and snaps
teleports. On leaving the world view (a menu) one standing-still update goes
out, so the figure does not walk on and park. Two critics' reviews shaped it:
the first build only held a steady speed at exactly 15 updates a second (the
desktop really sends every 4 or 5 frames: 2.4 to 6.8 m/s); the second stamped
the capped frame step, so a sender in a heavy scene (131 ms frames) stalled on
everyone's screen. Both are tests now (12 Hz, 30 fps, the real 4-or-5-frame
pattern, a 131 ms sender, a stall burst, a pre-join backlog, our own 300 ms
frame), each seen red.

## BUG-126: leaving the shared world on purpose, or being banned from it, left the figure standing for 90 s (FIXED v0.1440.0)

**Symptom:** a player who stepped out of the shared world (solo mode) stayed
frozen in it on everyone's screen for the reconnect grace; a banned player
stayed too, and their movement kept reaching everyone for those 90 s.

**Root cause:** `handle_game_leave` and `handle_game_ban` both used
`handle_game_disconnect`, the dropped-connection path, which holds a player's
place for the grace.

**Fix:** both call `despawn_player_now`; the grace is for dropped sockets
only. Real-relay tests for a leave against a drop, and for a ban, seen red.
Found by the scripted second player.

## BUG-127: another player's hair drew across their face, and the crew's heads sat inside their bodies (FIXED v0.1442.0)

**Symptom:** in the co-presence rig's screenshots (2026-10-03) another
player's hair cap showed as a dark band across the face; the amber crew
figures still had the head-inside-body fault BUG-120 fixed for players.

**Root cause:** the engine's sphere mesh (`Mesh::sphere`,
`src/renderer/mesh.rs:436`) is wound inside out, `[a, b, a+1]`, so the
opaque pipeline (counter-clockwise front faces, back faces culled) drew the
FAR inside of the head and the hair, and the hair's near side vanished. The
crew built their own fixed-offset body and head.

**Fix:** figures use their own outward-wound head mesh
(`net_route::figure_head_mesh_data`), the hair sits over the top and back of
the head and turns with the player's facing, and the crew are built by
`crew_figure_parts` from the same constants as players. Tests seen red; the
rig's screenshots show the face and the crew standing on the floor.

## BUG-128: the engine's sphere mesh is inside out (FIXED v0.1445.0, found 2026-10-03)

`Mesh::sphere` (`src/renderer/mesh.rs:436`) emits triangles as
`[a, b, a+1]`, which face inward; the opaque pipeline culls back faces, so
every sphere drawn with it shows its far inside instead of its outside. The
figures work around it (BUG-127). Eight other callers still draw it inside
out: `src/engine/home_meshes.rs:306` and `:1952`, `src/engine/world_load.rs:60`
(your own avatar's head) and `:1215`, and `src/lib.rs` around 7826, 8132,
8245 and 8978. The right fix is the one-line index swap in `Mesh::sphere`,
then a look at each caller (some may have been tuned to the inside view),
proven on the rig.

**Fixed (v0.1445.0):** the sphere's rows run DOWN from the north pole, and
its index pattern had been copied from the open cylinder, whose rings run UP,
so every triangle faced in (352 of 352 at 12 x 16). `Mesh::sphere` now emits
`[a, a+1, b, a+1, b+1, b]`. No caller had compensated (no negative scale, no
reversed culling), so nothing double-flips. The same mistake was found and
fixed in six more shapes: `hologram::sphere_mesh` (the orrery planets and the
HOME blip, now just `Mesh::sphere`), the pin marker's head and cone stem, the
orbit-ring tube, the pyramid's base, and the plant fruit sphere and cone (which
`tri` also LIT from inside, since it takes each face's normal from its
winding). The BUG-127 duplicate head mesh in `net_route.rs` now calls
`Mesh::sphere_data`. Every generator in `mesh.rs` is covered by
`every_generator_here_is_wound_to_face_its_normals`, the fruit by
`fruit_spheres_and_cones_face_out`, each seen red on the old orders. The
operator's v0.622 "inverted normals" report about the pipe flow beads was very
likely this bug.

## BUG-129: the player's game save had no backups, and a crash mid-save could destroy it (FIXED v0.1442.0)

**Symptom:** none yet, which is the point: `saves/offline_home.json` (and
the auto and named saves) was a single file rewritten in place with
`std::fs::write`, which empties the file before writing, and CLAUDE.md
promised a `backups/` folder that nothing wrote. Found 2026-10-02.

**Fix:** every save is written to a temp file, flushed and renamed over the
real one, so a crash leaves the previous save whole. Before a save is
overwritten, its previous version is kept under `backups/saves/<slot>/`: the
newest 10, at most one every 15 minutes, never a copy already held (so
"Snapshot now" clicks and stepping back through restores cannot push history
out), with the spacing measured from copies not stamped in the future (a
clock set back cannot switch it off). Settings > Data lists the copies with
Restore (two clicks), which first keeps what it replaces; saves are held
until the restored home is loaded, and with "Start every session from the
default home" on it says plainly that only the character is loaded. Tests for
each case, seen red; reviewed twice.

## BUG-130: "Back up now" copies were never pruned (FIXED v0.1442.0)

**Fix:** the relay keeps the newest 10 manual copies by stamp
(`MANUAL_BACKUPS_KEPT`), tells the admin in its reply which copy it removed,
skips pruning on a press when a copy is stamped in the future (a clock that
went back), and the retention document now lists manual copies (capped by
count, not age). Removing a single copy in-app is logged in
docs/design/in-app-ops.md.

## BUG-131: a combined verify hung for hours on one library test (MITIGATED v0.1442.0)

**Symptom:** the night of 2026-10-02 to 10-03, `just verify`'s library test
run sat for hours with the test binary alive and no output; the night's
release waited behind it until the operator asked in the morning. The same
suite then passed 2665/2665 in 152 s.

**Cause:** not proven. The one test the rerun reported running past 60 s
was `renderer::billboard_bake::tests::cluster_sprites_carry_real_per_leaf_colour_variation`
(it bakes sprites, slow but finished). During the hang, rigs booted the game
on the same GPU, so a test that asks for a GPU device stalling behind them is
the leading suspect, not a confirmed one.

**Mitigation:** `just verify` runs the library tests under a 40-minute
time limit and fails with a message pointing at the "running for over 60
seconds" lines, so a hang costs at most 40 minutes. The orchestrator's own
lesson: give every long background step a time limit and check it, rather
than waiting on a notification that may never come.

## BUG-132: probe-sweep's station vantages park the camera in space (FIXED v0.1444.0, found 2026-10-03)

**Symptom:** `home-overview-noon` (and every vantage using the camera
request's `{"station":"home","pose":...}`) captured empty space on
v0.1441.0 and v0.1442.0 alike; the same vantage had captured the home at
04:13 and 12:32 the same day.

**What the log shows:** probe-sweep's warm-up flies the camera to Earth
(lat 23, lon 13, 50 m) and sets the clock; the station request then sets
`ship_world_pos = station_world_pos` and `station_ride = true` but computes
`position = pose + state.station_off` from the PREVIOUS frame's offset,
taken while the camera was still at Earth ("parked aboard the home station
at pose Vec3(32629394.0, -2489632.5, -26913838.0)"). The request's own time
jump also moves the station between frames (the second pass landed at
(270, 26, -1230) m, not the pose). Whether a run lands is timing, which is
why the earlier runs looked fine.

**Fix to make:** in the station verb, compute the pose in the home frame
with the offset of the frame it is riding from (zero once `station_ride` is
set) and apply the clock jump before placing the camera; and make
probe-sweep CHECK that `camera_done.position` equals the requested pose
inside the home, failing the vantage when it does not (today the capture is
"ok" whatever it shows).

**Fixed (v0.1444.0):** `src/engine/ipc.rs` places the camera with the
offset of the station frame it now rides (`station_park_render_pos`), and
`advance_station_park` writes `camera_done` a frame or two later, once the
camera has ridden through the requested clock change, with the MEASURED
home-frame position, `error_m`, `yaw_pitch`, `station_ride` and
`clock_settled`. probe-sweep judges every station park
(`scripts/lib/station-park-check.js`) and fails the vantage when the camera
is not on its pose, not riding, or facing the wrong way; a park without a
pose (the default view, or a screen) is judged against the look the engine
reported choosing. `screenshot_done` carries `camera_home` too. The vantage
order in `tests/visual/vantages.json` now puts a station view straight after
a planet view, the order that exposed the bug, and a rig test pins that.

## BUG-133: the freshness gate passes a binary built from a different tree (FIXED v0.1446.0, found 2026-10-03)

**Symptom:** `scripts/check-fresh-exe.js`, which every rig runs before it
boots ("is this binary actually the build I am about to make claims about?"),
judges by file dates only: the exe must be newer than every compiled-in source
file. A binary built from a DIFFERENT tree after those files were last edited
passes. Shown 2026-10-03: the v0.1444.0 archive (main, increment 1a code)
passed the ship-homes-1b worktree's check with "PASS: the binary under test is
the current build", although it contains none of 1b. That was used on purpose
for a red run (the 1b rig against a 1a build, which failed `camera_in_p2` as it
should), but the same path lets a wrong binary pass a green one: a build from
main tested in a worktree, or the reverse, whenever its date happens to be
newer.

**Fix to make:** stamp a fingerprint of the compiled-in sources into the exe at
build time and have the gate compare it with the tree it is run from, refusing
on a mismatch; keep an explicit flag for a deliberate other-build run (a red
check), recorded in the rig's manifest.

**Fixed (v0.1446.0):** build.rs hashes every compiled-in source
(FINGERPRINT_INPUTS: src, assets/shaders, Cargo.toml, Cargo.lock, build.rs;
CRLF folded to LF, so line endings do not matter) into a stamp the exe
carries (src/main.rs, include_str! from OUT_DIR); check-fresh-exe.js rehashes
the tree and refuses on no stamp or any mismatch, naming the differing files.
File dates no longer decide anything. --allow-other-build "<reason>" runs a
deliberate other build and the rigs record it as other_build in their
manifests. 24 gate tests, red on the old gate (the BUG-133 case itself:
"PASS: the binary under test is the current build" for another tree's exe).
Side effect: build.rs now declares rerun-if-changed, so a docs or web edit no
longer recompiles the whole crate. v0.1446.1: `just check-delivery` (and the DELIVERY row of
`just brief`) answers "does the taskbar exe hold this tree's code" from the
same fingerprint, still ignoring the live PBR shaders and the version files.

**The three gaps left open, closed (2026-10-03, follow-up):**
1. *Every script that boots the game gates it.* probe-sweep (and so
   verify-runtime's sweep, which now forwards --allow-other-build), photograph-home,
   make-clips and boot-timing run runFreshGate before they touch their rig, and
   record `binary` (and `other_build` for a deliberate other-build run) in
   their manifests, like the verify-* rigs. An archive or another worktree's
   exe needs --allow-other-build "<why>". `just probe-sweep` and `just clips`
   pass their arguments with positional-arguments so a quoted reason arrives
   whole. `just launch`/`just play` (scripts/archive-build.js) stay ungated on
   purpose: they boot an archive for the operator to play and verify nothing.
2. *The binary that boots is the one checked.* The gate records the SHA-256 of
   the bytes it judged; after copying the exe into its rig every rig calls
   requireBootCopy, which refuses (nothing booted) unless the copy is
   byte-identical, and the throwaway relay does the same with expectSha256.
   find_newer_exe had no switch; it now returns at once when
   HUMANITY_NO_HANDOFF is set (every rig and `just launch-bg` set it) or a
   portable.txt sits beside the exe (every rig writes one, and a portable
   instance handed to an exe elsewhere would also leave its own storage):
   release_update::handoff_block_reason, unit-tested. probe-sweep also reports
   a game that exits before boot finishes as such (code 0 that early is a
   hand-off by a build from before the switch) instead of a 3-minute timeout.
3. *Compiled-in files.* scripts/lib/compiled-in.js finds every
   include_str!/include_bytes! in src/, follows the bytes to every reader, and
   classifies each target: fingerprinted, the stamp, test-only, data read
   disk-first (every read sits in a fn that reads the disk first, or is a
   reviewed fallback on FALLBACK_SITES), or allowlisted with a reason;
   scripts/tests/compiled-in.test.js fails on anything else (red first on 26
   files). The 18 compiled-in-only files that change behaviour (the nine
   ship-structure registries in data/blueprints, light types, LOD categories,
   conduits, reactions, the performance budget, the UI font, the release
   signing keys, the Accord text, the window icon) and assets/icon.ico went
   into FINGERPRINT_INPUTS: 580 files, 19.0 MB, build script about 0.95 s
   before; 599 files after (the timing is in the commit). Seven embedded-only
   reads of data that is otherwise disk-first were made disk-first instead,
   so editing items.csv, item_profiles.ron, food_system.ron, the star
   catalogs or harvest_windows.ron still needs no rebuild: the inventory's
   item details, its Eat and Drink buttons, the Maps page's stars, and the
   self-sufficiency figure had each been ignoring the data folder.

**What a review of that follow-up found, and fixed (2026-10-03):**
1. *A vendored crate was compiled in but not fingerprinted.* Cargo.toml's
   `[patch.crates-io] rav1d = { path = "vendor/rav1d" }` (the BUG-093 decoder)
   compiles 73 files from vendor/, and Cargo.lock holds no hash for a path
   dependency, so an edit there left the stamp saying "current". `vendor` is
   now in FINGERPRINT_INPUTS, and compiled-in.js reads Cargo.toml: every
   `path = ".."` / `build = ".."` there, and every `#[path]` module in src/,
   must sit under an input.
2. *A data file that fails to parse was masked by the old built-in copy.*
   Disk-first loaders fall back to the copy built into the exe when the disk
   file is missing or does not parse (ground/materials.ron, the garden tables,
   the AssetManager helpers...). The gate passed (data/ is not stamped), the
   rig was green, and the next build would embed the broken file and behave
   differently (materials.ron panics at its expect). Every fallback now logs
   one marker line, `embedded_data::note_builtin_copy` ("[built-in data
   copy]"), and compiled-in.js only counts a read as disk-first when its fn
   reads the disk BEFORE it and calls the note naming that same file, so a new
   silent fallback fails the test (that also closes the review's third point:
   an unrelated `.exists()` in the same fn no longer counts, and a grouped,
   aliased or glob import of a const, or a qualified read through an inline
   module, is followed; a `pub use` re-export is refused). Every rig fails a
   run whose run.log (and relay log) holds the marker: BUILT-IN DATA
   (scripts/lib/game-launch.js builtinDataLines; the marker is pinned to the
   Rust const by a test). Seen on a real boot: with a line of junk appended
   to materials.ron, the gate still passed and the sweep captured 1/1 with no
   panic, and it now exits 2 with "BUILT-IN DATA ... data/ground/materials.ron:
   it does not parse (186:1: Non-whitespace trailing characters)"; with the
   file intact the same exe captures 1/1 with built-in data=0.
3. *Throwaway relays never saw the tree's data/.* The relay reads data/
   relative to its working folder, and the throwaway's folder held only
   server-config.json, so crew.ron and room_equipment.ron came from the
   built-in copies and chores.ron and market/categories.json (no built-in
   copy) were simply absent. `mirrorData` now puts the tree's data/ in the
   relay's folder (files up to 1 MiB copied, larger ones hard-linked, never
   the relay's own names or the tree's server-config.json); verify-live-screen's
   relay folder gets the same.
4. *An older build could still hand off.* Only builds with this fix honour
   HUMANITY_NO_HANDOFF, and only probe-sweep noticed an early exit. Every rig
   now starts the game through `spawnGame` (scripts/lib/game-launch.js), which
   checks the copy against the judged bytes immediately before it spawns, sets
   HUMANITY_NO_HANDOFF, and, on an exit the rig did not ask for, finds any
   HumanityOS process the game started (by parent process id, which Windows
   keeps after the parent exits), stops it and names it in the failure.
5. *probe-sweep gated, then waited for the machine, then copied:* a sweep that
   waited for a cargo build of the same exe always ended in "BOOT COPY
   CHANGED". After a real wait it now gates again.
6. *A relay with no judged hash skipped the copy check silently;*
   startRelay now requires expectSha256.
7. *The documented manual boot skipped the gate and the hand-off switch:*
   CLAUDE.md and the runtime-verifier agent now say `just launch-bg`.
8. *Weak test:* rig-boot.test.js only checked that the right words appeared in
   each rig. The order now lives in spawnGame (tested by running it), each rig
   may start the game only through it, and every rig is run on an unstamped
   file to show it refuses on the gate before it copies anything.
Not changed, on purpose: an installed copy reads its extracted data/, which is
never refreshed after an update; the seven reads made disk-first above now
agree with the rest of their files' readers, which already did that. Nobody
has an installed copy yet; refreshing extracted data is its own job.

## BUG-134: a Library rebuild that dies part-way leaves data/library half-written (FIXED v0.1447.1, found 2026-10-03)

**Symptom:** twice in one evening, the first `node scripts/build-library.js`
after a merge died with only "Node.js v24.13.1" visible, and git then showed 72
shipped guides changed by 703 lines (ignoring line endings). A second run was
clean. Committing after the first run would have shipped a mix of new and old
copies with a stale search index.

**Cause:** the script rewrites about 145 files in passes (copy each doc,
rewrite its links, write the indexes) with plain synchronous writes, so any
error in the middle leaves the passes before it applied and the ones after it
not. Caught on a loop of runs: `Error: UNKNOWN: unknown error, open
'C:\Humanity\data\library\search-index.json'` on 1 run in 4, a Windows sharing
violation (Node reports some as UNKNOWN rather than EBUSY) while something held
the 4 MB index a previous run had just written.

**Fix:** every write retries a short lock (EBUSY, EPERM, EACCES, UNKNOWN; 8
tries, 150 ms apart), and a run that still fails prints the error and says
plainly that data/library/ may be half-written and must not be committed until
a rerun finishes. Checked with a simulated two-time lock (recovers) and a
permanent one (fails loudly, exit 1); then 30 real runs, 0 failures.

## BUG-135: after erasing your account, the app reconnects and recreates it without being asked (FIXED v0.1449.0, found 2026-10-03)

**Symptom (found by the final review of ship-homes increment 1b, read from the
code, not yet seen in a running game):** erasing your account (Settings, the
relay's account_delete) removes your name, plot, game progress and the rest, but
the app stays connected to that server. Three paths then recreate data the
person just asked to erase, without them choosing to come back:
- an automatic reconnect (a relay restart, which every deploy causes, or a
  network drop) identifies again, which registers the name again
  (src/relay/relay.rs ~2844), clears the game's refusal on the fresh connection
  (src/engine/home_plot.rs ~445), and the join gate joins and claims a new plot;
- an erase made BEFORE entering the world (chat connects on the main menu) sends
  no refusal, so pressing Enter World joins and claims a plot under the erased
  key (src/relay/handlers/home_plots.rs ~97-107 only tells a game that was in
  the world);
- the game's sentence says "reconnect", but no control in the app is called
  that (the way back is Chat > the server's menu > Disconnect, then Connect).

**Fix to make:** when the relay confirms an erase, the app disconnects from that
server and marks it manually disconnected (no auto-reconnect), and the Chat page
shows the plain way back (a Connect button with a sentence saying it signs you
up again). Consider also refusing a game join on the relay for a non-bot key
with no registered name, after confirming no other path leaves an identified key
without one. Web mirror: the same after an erase in the browser.

**Fixed (v0.1449.0):** the relay sends the erasing account's own clients
`account_erased` (with `partial` when any part of the erase failed). The
native app disconnects that server through the shared Disconnect path and
records the erase per identity and server in its config (`account_erased_on`),
so neither the backoff, the boot or unlock auto-connect nor the background links
dial it again; the Chat page's Connect box says connecting signs you up again (an
unfinished erase says to erase again instead); the game's sentence names Chat
and Connect. Web: it stops reconnecting, ends a call in progress, forgets the
saved name (never the key) and shows the note. Not added: refusing a game join
for a key with no registered name, because placeholder names (DesktopUser_NNNN)
are never registered and real players would be refused.

**Known limit, closed by the operator's option 2 (2026-10-04):** only clients
online at the moment of the erase found out, so a second device with the app
closed, a web tab mid-reconnect, or a socket that dropped between the erase and
its receipt signed up again on its next connection. The operator chose, verbatim:
"Let's go with option 2 that way we have a way to cull the list over a period of
time. That way we don't end up with a massive log of all the accounts that erased
themselves after many years or a malicious attack." Now the relay remembers an
erase for a limited time (storage/erased_accounts.rs: a one-way keyed fingerprint
of the key and the day only, kept for `erased_accounts_ttl_days`, default 30, and
never more than `erased_accounts_cap` rows, default 100,000, oldest first; both
editable in Server Settings > ADMIN > Server policy > Erased accounts). A key with
an entry that connects is answered with `account_erased` (`earlier: true`) and is
not signed in at all; a game join from a device still connected from before the
erase is refused with reason `account_erased`; only the person's Connect (native)
or Enter (web) under the erase note sends `sign_up_again`, which forgets the entry
and signs them up again (handlers/sign_ups.rs). The person reads the real number
of days before erasing.

**Review of option 2 (2026-10-04, seventeen findings, two skeptics each).**
Fixed: each entry keeps the window in force when it was made, so raising the
setting later never stretches what a person was told, while lowering it shortens
every entry at once; "for N days" became "for up to N days" and the match is
strict, so N is never exceeded; a row dated in the future (a clock jump) is culled;
the sentence says the backups keep a copy until each is deleted, and the log lines
about erased accounts (including the erase's own) name no key; one expiry pass
(`run_expiry_sweeps`, storage/expiry.rs) runs at start, every six hours and after
every saved settings change; the relay closes a refused identify at once, and the
Tasks page takes `account_erased` as a refusal instead of retrying; a game join
checks the erase again under the world lock, and the name and member row are
written only through a check made in the same step, so an erase landing between
the identify and those writes still wins; a device that was offline is told
whether the erase finished, read from what it left (`erase_left_rows`); the
native in-world sentence for an earlier erase no longer clears itself a frame
later; native asks the server for its settings once the sign-in completes (the
request sent at connect had always been dropped), and says the sentence only when
the server sent its number of days; the fingerprint secret is written atomically,
and a damaged one is reported by `/health` (`erase_memory`) and `just brief`.

What remains: a device that stays offline for longer than the window signs up
again when it next connects, as before (that is the cull the operator chose); the
entry rides into backups until each is deleted, and a "Back up now" copy has no
age limit; if the relay's `data/erased-accounts.key` is lost, old entries stop
matching (and are culled on schedule); a device still connected from before the
erase that ignores `account_erased` can still write chat-side data (a message, a
profile) under the erased key until it disconnects: only its sign-up and its game
join are refused; and desktop apps from v0.1449.0 and older never send
`sign_up_again`, so on a relay with this change an account erased from such an
app can come back from it only after the window (the web is served by the relay
and always matches it). Ship the relay and the desktop app in the same release,
and say this in its notes.

**Second review of option 2 (2026-10-04, eleven findings).** Fixed: the source
test that pins `erase_left_rows` to `delete_account` had its "\r\n" escape
turned into raw line breaks, so on a CRLF checkout (the operator's) it panicked
and `cargo test --lib` went red there while Linux CI stayed green; it now reads
every `del(` call however it is laid out (one also in features.rs, a failure
message only, had the same raw breaks). No log line about an erase names the key
any more, including the game's "left" line and the socket teardown that runs when
the erasing client closes (it names an erased key "an erased account"); the test
now reads those too. The native Server Settings page no longer makes its working
copy from defaults before the server's settings arrive (Save wrote every default
back, and a lowered mailbox or retention window then deleted at once): no form
until they arrive, and a copy with no unsaved edits follows new settings. The
mailbox and retention hints say that saving a lower number deletes older items
at once, for good. Two relays (or tests) creating a key file at once now end with
the same key. `erase_left_rows` reads only what the erase answers for: not a
profile another server gossips back, while a failed delete by name or of listing
images keeps the registration or the listings, so it shows. A link code and the
member row check the erase in the same step, and a refusal on either tells and
closes before anything is bound. The in-world follow reads the erase on the
connected server. The answer to `server_settings_request` (and its role list) goes
to the client that asked, not to everyone; an admin's saved change still goes to
all.

**Not fixed, on purpose: desktop apps from before v0.1449.0.** They do not know
`account_erased`. To such an app, the relay's answer to an erased key is an
unknown message followed by a closed socket; each socket it opens resets its
reconnect backoff (src/lib.rs ~14372) before being closed, so it redials at once,
over and over, until the per-IP identify limit (30 a minute) holds it. Nobody runs
those builds yet, and the project adds no compatibility code before launch (the
no-backwards-compatibility rule in CLAUDE.md), so this is written down instead of
handled: if it is ever seen, the fix is to update the app.

## BUG-136: the carry limit is shown as a fixed 50 kg, and being overloaded does nothing (FIXED v0.1452.0, found 2026-10-04)

**Found by** the Library writer checking the "Force, Levers and Mechanical
Advantage" guide's game tie-in against the code. The inventory system computes
the real limit and an encumbered flag (src/systems/inventory/mod.rs ~1251-1258:
`weight_capacity + carry_bonus`, where the bonus is the equipped outfit's
`carry_capacity`, e.g. a backpack), but:
- the Inventory page's Weight tile reads its own `max_carry_weight`, a fixed
  50.0 (src/gui/pages/inventory.rs ~38, shown at ~1453), so a backpack never
  shows on it;
- nothing reads `inventory.encumbered` (it is only written and logged), so
  carrying more than the limit has no effect on the player at all.

**Fix to make:** the tile shows the inventory's real limit (capacity plus the
outfit bonus). What being overloaded DOES is a gameplay decision for the
design (the dual-modes rule: a realistic mode and a softened one): slower
walking is the usual answer. Until then, say on the tile that the limit is
advisory.

**Decided (operator, 2026-10-04):** "slower walking in realistic mode and no
jumping in nonzero G based on weight/mass. We can obviously carry heavier in
low-g to zero-g but, mass still applies."

**Fixed (2026-10-04):** the rules live in `src/systems/encumbrance.rs` and
`src/engine/carry_load.rs` applies them each frame.
- The Weight tile reads the inventory system's own numbers (`GuiState::carry`):
  the pack's 50 kg plus the worn gear's `carry_capacity` (the system now records
  it as `Inventory::carry_bonus_kg`), scaled by the gravity the walk applies
  (9.81 / g: about 2.6 times on Mars, no limit when weightless). A line under
  the tiles says where the limit comes from and why the player is slow. The
  page's own fixed 50 kg, and its weight and volume sums made once on the first
  draw, are gone. `encumbered` uses the same gravity-aware limit.
- Settings > Gameplay > Carrying weight. **Forgiving** (default): a warning on
  the tile and the HUD, no change to movement. **Realistic**: over the limit,
  walking slows by how far over (10% over walks at 90%, never below a 15%
  crawl) and Space does not leave the ground (`CameraController::jump_scale`
  for the homestead, `surface_move::carry_gated_radial` on a planet); and any
  load weighs on the jump's launch speed, sqrt(70 / (70 + load)).
- The slowdown is WALKING only (`CameraController::carry_speed_factor`, and
  `surface_move::carry_walk_factor` on a planet). The first version multiplied
  it into the controller-wide `speed_multiplier`, which on a planet also drives
  dev flight, the flight band and swimming, so an overloaded player in dev
  flight crawled while the HUD said "walking" (caught in review, same day).
- Tests besides the rules: the Weight tile and its note are drawn from
  `GuiState::carry` (`screen_surface` tests), the HUD draws the overload line,
  `carry_load::steer` sets the walk and the jump without touching the effects
  multiplier, and the setting survives a save and a load.
- Left: starting and stopping are instant in both walks (no horizontal
  inertia), and there is no zero-g pushing model, so the jump is the only place
  carried mass acts. Ladders climb at the same rate whatever is carried. The
  mode is each player's own choice, also in the shared world; a server-side
  rule is tracked in `docs/design/in-app-ops.md`.

## BUG-137: the desktop Server Settings page can show defaults, and Save writes them over the real settings (FIXED v0.1450.0, found 2026-10-04)

**Found by** the review of the erase-marker fix round, read from the code. The
desktop app asks for the server's settings in the same moment it identifies
(src/net/ws_client.rs ~222), but the relay ignores everything a socket sends
before it is signed in, so the request is dropped and the app's cached settings
stay empty unless an admin's save happens to broadcast them. The Server Settings
page then seeds its editable copy from DEFAULTS (src/gui/pages/server_settings.rs
~2098-2107: `server_settings.clone().unwrap_or_default()`), shows them as the
server's values, and Save (or the #local checkbox, which sends the whole copy)
writes every default over the real settings: the server name and description,
limits, and windows such as how long DMs are kept, which the expiry pass then
applies.

**Until fixed:** do not press Save in the desktop app's Server Settings.

**Fix in flight (the erase-marker branch's second fix round):** ask for the
settings after sign-in, seed the editable copy only from the real settings and
refresh it when they arrive, keep Save disabled until they have, and answer the
request to the asking client only.

**Fixed** on the erase-marker branch (BUG-135 option 2, its second and final
review rounds). The desktop app asks for the server's settings only once the
sign-in has completed, and again on EVERY newly signed-in socket, whether or
not it already has them (src/gui/connections.rs `socket_signed_in`,
`ask_server_settings_once`); Disconnect, a dropped socket and a server switch
also clear the "asked" mark, so a lost or unreadable answer never leaves the
page waiting for the whole session, and a change another admin saved while
this app was offline is seen after the reconnect. The page waits for the real
settings before it shows a form: until they arrive it says so, and Save, the
Server master row and the feature switches are not there to press
(`page_draft`). When settings arrive, a working copy with no unsaved edits is
refreshed from them (`draft_after_settings_arrive`); one with edits is kept,
and the page says in one line that the server's settings changed while you
were editing, because Save sends the whole copy (`changed_under_edits`). The
relay sends its answer to `server_settings_request` only to the one who asked
(the connections of that key); an admin's saved change still goes to everyone.

## BUG-138: the night sky over Silverdale was black, with a dozen stars and no Milky Way (FIXED v0.1452.0, found 2026-10-04)

**Symptom (found filming the landing page's night-to-sunrise shot):** at 04:00
local over Silverdale, Washington, the sky showed about fifteen points and no
Milky Way. The star pass was being skipped as though it were day.

**Cause:** the daylight gate that skips the star pass when the sun is up
(`engine::ipc::sky_daylight`, v0.1059) compared the frame lock's anchor with the
sun. The anchor is kept in the planet's UNTURNED frame and the sun is in the
world frame, so the gate judged the sun over the wrong longitude by the planet's
whole turn, and the same longitude at every hour: whether it called it day did
not depend on the time at all. At Silverdale on 2026-10-04 it said "day" all
night. Any place could be hit, depending on the date and longitude.

**Fix:** the anchor is turned by the spin the frame lock rides before the sun is
compared with it (`ipc::sun_over_anchor`, shared by the gate and the twilight
fades). Tests: `daylight_gate_tests` (seen red with the unturned anchor) and the
call-site check in `tests/engine_wiring_lint.rs` that both readers pass the live
spin. Vantage `silverdale-night-sky` asks for a full star field at night.

## BUG-139: every night sky was the wrong one: the star catalogue was drawn in its raw axes (FIXED v0.1452.0, found 2026-10-04)

**Symptom (found by the review of the Silverdale clip):** the star points, the
Milky Way glow, the halos and the constellation figures were drawn in the
catalogue's own equatorial axes (north celestial pole along +z), while the
engine's Earth spins about world +Y. So the sky's pole sat on the sky's equator:
Polaris rose and set, and from Silverdale before dawn the south-east showed the
southern Milky Way around Crux and Carina, which never rises at 47.6 degrees
north. The clip's headline, "The real sky over a real place", was false.

**Fix:** a new `renderer/sky_frame.rs` turns the catalogue into the world: the
pole to world +Y, then a turn about the pole that puts the real sun's right
ascension for the date (the Astronomical Almanac's solar formula) on the world
sun. Because the game clock is defined by the sun, that sets the sidereal time
at every place and hour, and the stars stand where they really stand. Both star
pass call sites use it through `ipc::sky_rotation`. Tests: `sky_frame` (Polaris
due north at the latitude at every hour; Sirius, Procyon, Orion, Capella,
Regulus and Deneb within 0.1 degree of the IAU sidereal-time sky over Silverdale
on 2026-10-04; Acrux and Canopus below the horizon; the turn proper, never a
mirror), each seen red against the old identity turn.

**Same change, the stars in daylight:** the renderer has no eye adaptation, so
the star layers kept their night brightness under a dawn sky (the Milky Way
stood in a gold sky with the sun's disc up). `sky_frame::twilight_fades` now
fades the Milky Way first, then the star field, then the brightest stars, as
the DRAWN sky brightens over them, through the star camera's spare `sun_color`
slot in all three sky shaders. The ramps are matched to the drawn sky, not to
the naked-eye limits (`sky_frame::NAKED_EYE`), because the drawn sky is black
through nautical twilight (BUG-141) and the real limits left the frame empty.

**Same change, the pink dome before sunrise:** the sun's corona, a sprite three
times the disc's width, stood up over the horizon before every sunrise and
after every sunset while the ground hid the disc. Glare comes from the disc, so
the disc and the corona are now scaled by how much of the disc is clear of the
planet (`frame_lock::sun_disc_clear`, `sun_disc_tests`, seen red).

**Still not right:** the game has no axial tilt, so the sun sits on the sky's
equator (on 2026-10-04 the real sun is 4.5 degrees south of it), and the planets
and the Moon have the problem in BUG-140.

## BUG-140: the orbit model maps the ecliptic into the world with a reflection (OPEN, found 2026-10-04)

**Found by reading the code while fixing BUG-139, not yet seen in a picture.**
`cosmos.rs` (`body_position_relative_au`, ~398) maps the ecliptic's x, y, z to
world x, z, y. Swapping two axes is a mirror, not a rotation. Planets therefore
go round the sun from +X toward +Z, which is clockwise seen from world +Y, while
Earth spins counter-clockwise about +Y (`DQuat::from_rotation_y`, east is the
direction of the turn). Real orbits and the real spin turn the same way. The
cosmos page's sub-point maths (`subpoint_lat_lon_deg`, which takes the
equatorial y axis as `P x X`, a proper frame) inherits the same sign problem in
longitude; its tests only check latitude.

**What it would show:** the sun is unaffected on the ground (the spin is defined
from the sun, and BUG-139's sky turn is set from the sun each day). The Moon and
the planets are placed mirror-wise about the sun: by the reasoning above a waxing
Moon would be drawn where a waning one stands, at the matching time of day, and a
planet's place among the stars is off by twice its angle from the sun. Confirm in
a running game against a real ephemeris before fixing.

**Fix to make:** map the ecliptic with a proper rotation (ecliptic y to world -Z),
then follow every reader of the world-frame sun and planets (the spin's sun
azimuth, the home station's phase, BUG-090's `over_its_longitude`, the travel
tool, the hologram, the cosmos page).

## BUG-141: the drawn sky stays black through nautical twilight (OPEN, found 2026-10-04)

**Seen filming the Silverdale clip** (the probe rig, operator graphics, stills
looking east at 04:45 to 06:06 local): with the sun 10 degrees below the horizon
the frame over Silverdale is black, sky and ground alike, and still black at
7.6 degrees down; the first orange band shows at about 5 degrees down, and the
sky is lit only in the last 2 or 3. A real sky at 10 degrees down is a deep blue
with an orange band where the sun will rise and a horizon you can see.

**Why, as far as it is known:** the drawn atmosphere is single scattering with a
flat multiple-scatter stand-in (`renderer/atmosphere.rs` says so: "twilight is a
little darker than reality"), and the frame has one fixed exposure, with no eye
adaptation, so a sky ten thousand times dimmer than day draws as black. The
multiple-scattering LUT in `renderer/atmo_luts.rs` is the physics that is
missing, and eye adaptation in the HDR present pass is the other half.

**What depends on it:** the star layers' twilight fades are matched to the
drawn sky (`sky_frame::DRAWN_SKY`) so the stars carry the frame through the
black stretch. When this is fixed, switch them to `sky_frame::NAKED_EYE`, the
real limits (the Milky Way gone 12 degrees down).

## BUG-142: a rig loses a run to EBUSY when its last game is slow to let go of the exe (FIXED v0.1452.0, found 2026-10-04)

**Seen:** `verify-copresence --plots` passed its first join order (60/60) and
then died before the second: `EBUSY: resource busy or locked, copyfile
target\release\HumanityOS.exe -> .probe-rig\copresence\HumanityOS.exe`. The
first order had stopped its game, but Windows keeps the image file locked for
a while after the process is gone, and longer when the game is still letting
go of its GPU resources. The same run had failed this way once before (the
`--entry menu` game-first order, the same day).

**Why:** every rig copied the exe, and on EBUSY waited two seconds
(`ping -n 3`) and tried exactly once more. Four rigs carried the same copy:
probe-sweep, verify-copresence, verify-screens and verify-live-screen.

**Fix:** one shared copy, `scripts/lib/rig-exe-copy.js` `copyExeIntoRig`, used
by all four. It stops anything running from the rig copy, then retries every
500 ms for up to a minute on EBUSY, EPERM or EACCES, stops the rig game again
every ten failed tries (a game that was still starting the first time), says
when it had to wait and how long, and on giving up names the folder that
stayed locked. Any other error is thrown at once. Tests:
`scripts/tests/rig-exe-copy.test.js` (in `just rig-tests`), including a lock
that lasts 4 s, which the old single retry after 2 s could not survive.

## BUG-143: 140 of the vendor's 300 trade goods are not items, so the shop silently never offers them (FIXED v0.1453.0, found 2026-10-04)

**Seen:** the fact check of the Stone, Clay and Earth guide noticed `clay_0` in
`data/trade_goods.ron` with no such item in `data/items.csv` (which calls it
`clay_raw_0`). A count over both files: 140 of the 300 trade-good ids are not
items, among them dirt, bamboo, sulfur, cotton, flax, hemp, lumber, brick, tin
ingots, cotton cloth, nails and steel pipe.

**Effect:** nothing breaks, which is why it went unseen. The vendor's catalog
(`GuiState::vendor_goods`, built in `lib.rs`) keeps only goods present in BOTH
files, so the 140 are dropped without a word, and a player can never buy or
sell clay, fibre, lumber or brick at a trading post.

**Fix:** all 140 resolved. 64 renamed to the item they mean (clay_0 to
clay_raw_0, lumber_0 to wood_plank_0, brick_0 to stone_brick_0 ...); 26
added to items.csv as real materials (sulfur, saltpeter, tin, lead and zinc
ores, gems, resin, plywood, tin and bronze ingots, linen and silk cloth,
steel and ceramic plate, and four that other data already named: poultice_0
and med_kit_advanced_0 from medical.ron, lockpick_0, sextant_0); 50 removed
(fantasy goods, armour and weapons with no item or equipment stats, building
pieces the construction editor makes from planks and bricks, foods with no
nutrition profile, and a generic key that would open every metal-key lock).
Three renamed goods were repriced so they create no new buy-craft-sell loop
(wood_plank_0 3, carbon_fiber_sheet_0 20, boat_sailboat_0 300). The vendor
now has 250 goods, every one an item. Test
`systems::economy::tests::every_shipped_trade_good_is_an_item` reads both
shipped files through the game's own loader and fails on a missing or
duplicated id; seen red on the original files ("140 trade goods ... are not
items"). Old ids still named elsewhere were renamed too: processor_0 to
cpu_0 in medical.ron and tech_tree.ron, torch_0 to torch_handheld_0 in
tech_tree.ron. Left for the food work: herbal_tea_0 (named by medical.ron and
tech_tree.ron, no item yet), and lock_types.ron's brass_key_0 (no item yet).

Also found by the same checks, smaller: the water pump's card said 12 L/min
while its own water port and the self-sufficiency data say 2 L/min (fixed
2026-10-04 in `home.ron` and `home_solo.ron`; `home_outline.json` had listed
it as a contradiction to fix since September).

## BUG-144: the Library quotation gate blamed quotes on the wrong source, and read prose between quotes as a quote (FIXED v0.1452.2, found 2026-10-04)

**Seen:** `scripts/check-library-quotes.js` reported 6 problems after two
Library batches merged, which made `just preflight` fail. None was a real
restate-only quotation:

- Two were public-domain quotations ("The CDC's wording: ...", "the USDA
  guide adds: ...") blamed on NCHFP and Penn State, named a sentence or two
  earlier.
- Two were the prose BETWEEN two short quotes, e.g. Gildan's "Heavy Cotton"
  T-shirt ... its "safety" colours, and the cell text after "vegan
  leather" in a table.
- Two named a claim in order to correct it or to say which claim a source
  is cited for.

**Why:** the pairing was a regex, `/"([^"]{20,})"/g`, which after failing
on a short quote restarts one character later and takes the closing mark as
an opening one. The attribution took the first restate-only source named
ANYWHERE in the 220-character lookback (longest name first) and did not know
the quotable sources at all, although its own comment said "the nearest
recognisable source name".

**Fix:** marks are paired in order within a paragraph; the source is the
NEAREST name before the quotation among every registry source, with short
deliberate aliases for the quotable publishers (CDC, USDA, EPA, OSHA, NIOSH,
FEMA, USGS) matched as whole words; only a nearest `use: facts` source is
flagged. The logic is exported and `scripts/tests/check-library-quotes.test.js`
(8 tests, in `just preflight` and `just check-library-quotes`) holds the
cases; three of them were seen failing against a copy carrying the old
pairing and attribution. With the fix the real Library had 2 reports, one
new (a guide quoting its own former wording, hidden before by the
desynchronised pairing); those two and the 95-percent source-list line got
`quote-ok` markers with their reasons. 0 problems.

## BUG-145: recipes turn vendor-bought inputs into goods that sell back for more (FIXED v0.1455.0, found 2026-10-04)

**Seen:** while fixing BUG-143, a check over every recipe whose inputs the
vendor sells found six where buying the inputs (at 1.25x base) and selling
the result (at 0.5x base) makes money, before BUG-143 and after it:

| Recipe | Inputs cost | Sells for |
|---|---|---|
| craft_stim_pack | 13 | 85 |
| craft_antibiotics | 13 | 50 |
| craft_painkillers | 11 | 40 |
| build_spacecraft_pod | 876 | 2500 |
| build_motorcycle_full | 364 | 400 |
| make_wire | 13 | 15 |

**Why it matters:** an endless money loop at any trading post. In a shared
world it inflates everyone's prices.

**There were 24, not 6.** Counting only recipes whose inputs the vendor sells
misses most loops: an input it does not sell can usually be crafted from goods
it does (buy logs, saw planks, build a bow; buy copper ore, smelt, make servo
motors, build a light mech that sold for 4000 from 1317 of goods).

**Fix:** test `no_recipe_resells_for_more_than_its_inputs_cost`
(`src/systems/economy/mod.rs`) works out every item's cheapest cost, buying
it or crafting it from cheaper inputs with byproducts credited, through the
runtime's own loaders and the vendor's own price functions, and names every
recipe whose outputs sell for more; it also fails on a cycle of recipes that
makes goods from nothing and on fewer than 300 recipes checked. Seen red on
24. Fixed by repricing 22 outputs.

That first fix left vehicles absurd (a spacecraft pod cheaper than a sedan),
because the vehicle recipes put 38 to 203 kg of parts into vehicles of 180 kg
to 50 t. All 25 vehicle recipes now carry a bill of materials of 1.0 to 1.25
times the vehicle's weight, and every vehicle sells for between its parts'
cost and twice it (tests `vehicle_recipes_weigh_what_the_vehicle_weighs` and
`no_vehicle_sells_for_less_than_its_parts`, each seen red). Vehicles now cost
3 to 26 times their old prices (a motorcycle 2500, a light mech 120,000),
still below real-world prices on the project's own wage scale.

**Left:** quality grades still loop (BUG-146), and the big vehicles can no
longer be hand-crafted from the backpack (BUG-147).

## BUG-146: a better craft grade still makes a money loop at the vendor (OPEN, found 2026-10-04)

**Seen:** while fixing BUG-145. The vendor pays 0.5x a good's price times its
craft grade (`data/manufacturing.ron`: good 1.5, excellent 2.5, masterwork
5.0), so a crafted good loops at good grade once its price is over 1.33x its
parts, and at masterwork once it is over 0.4x. On the v0.1455.0 data, good
grade loops in about 33 recipes, excellent 67, masterwork 115; every vehicle
(priced at up to 2x its parts) loops at good grade and above. The BUG-145
test checks standard grade only, on purpose.

**Why it is not just a price:** a skilled crafter turning cheap inputs into
valuable goods is real labour value and should pay. What makes it a loop is
a vendor that buys any quantity at a fixed price. The fix belongs in how the
vendor values grade and quantity (a price that responds to how much of a
good it already holds, or grade paid on labour rather than on the whole
price), not in lowering every price below 0.4x its parts.

## BUG-147: the big vehicles cannot be hand-crafted from the backpack (FIXED v0.1457.0, found 2026-10-04)

**Seen:** after the vehicle bills of materials (BUG-145) became realistic. A
manual craft draws only on the backpack (36 slots, about 65 L). Only the
bicycles, hand cart, sled and e-scooter fit; a spacecraft pod needs about
158 slots and 2200 L, a freighter about 2132 slots and 29,000 L. The boats
were already over 65 L before. Every vehicle, spacecraft and the locomotive
included, is built at `workbench_0`.

**Not affected:** the default Dev play mode, where crafting takes no inputs;
the automation path, which already counts home storage.

**Fix:** a hand craft draws on the home's storage too, the way the machines
and the build menu already did (`src/systems/crafting/home_store.rs`): the
backpack first, then home storage, then the tanks for tap water, spending
exactly what the recipe needs. What the backpack cannot take of the result
goes to home storage with the crafter's grade (a new channel beside the
machines', `home_stock_hand_made`, filed by `receive_machine_outputs`); the
three kit vehicles still roll out in front of the crafter. It counts only the
player's own home and only where they are: aboard with the home on this ship.
On a planet, in open space and as a guest on a shared ship (home put away),
only the backpack counts, and a craft whose result would not fit is refused
before anything is spent, as before. A short craft now says what is short and
where it looked. The Crafting page shows each input's split between backpack
and storage, what is missing, and where the result will go. Found on the
way: the `home_stock` mirror counted chests built on a planet, so the home's
machines and the build menu could draw on what was on Earth; they are left
out now (`inventory::placed::stock_counts`, `uses::planet_store_paths`).
Tests in `src/systems/crafting/home_store_tests.rs`, each seen red.

**Review fixes (same day):** a result bigger than the whole backpack (a pod)
that finishes, or is asked for, where home storage does not count no longer
says "make room": it says home storage takes it once the player is back at
their home, and why it does not count here. A guest's put-away home now
serves as no crafting station (its machines and the pieces built in it,
`engine::built_uses::station_types_here`), matching its storage and tanks. A
guest's build aboard counts only the pack, as the structures list already
did (`ConstructionSystem`). The Tools card and the greyed-button line name
tools readably ("Wrench Adjustable"), as the Ingredients card names parts.

**Left:** the right station per vehicle class (a shipyard for the
spacecraft, a boatyard for the boats, rather than every vehicle at
`workbench_0`); a vehicle filed in home storage is an item in the Barn, not
yet something to launch or drive (only the three kit vehicles are); a
confirmed trade offer is still not reserved from crafting (backpack-first
can spend offered items, which withdraws the confirmation, BUG-114's guard).
The `assemble_*` kit recipes have the same toy bills of materials (a 1497 kg
car from 81 kg of parts) and are not yet covered by the weight test.

## BUG-148: the home marker never shows from the ground: it is cut off at the render distance (FIXED v0.1459.0, found 2026-10-04)

**Seen (by reading the code, during the fact check of the navigation guides):** the
tracked Home Station ring, which is meant to show the way and the distance to your
home in orbit, is projected with the gameplay camera. That camera's far plane is the
Render distance setting, 500 m by default and 2,000 m at most (config.rs ~996, ~1573,
applied at boot, lib.rs ~1726 and ~16389), and it uses reverse depth (camera.rs ~493),
so anything past the far plane gets a negative depth, which hud.rs ~1119-1120 rejects.
The station is about 36,000 km up, so from the ground the ring never appears and
nothing shows the way home.

**Fix (d7ee64a38):** `hud::marker_placement` places a tracked marker by direction: it
reads clip x, y and w, which a perspective projection builds without the near or far
plane, and never z. In view the ring goes where the target projects; off screen or
behind, it is pinned to the screen edge on the side to turn toward, with an arrow, and
its label stays on screen. The distance reads "36,000 km". Tests, each seen red first:
`marker_and_mode_tests::a_tracked_station_36000_km_away_draws_its_marker` and
`a_tracked_station_behind_the_player_is_pinned_to_the_screen_edge` ("none was drawn").
Probe vantage `silverdale-home-marker` shows it from the ground: "Home Station ·
38,240 km".

## BUG-149: the Dev page's Land and Travel leave fly mode on while the HUD reads WALK (FIXED v0.1459.0, found 2026-10-04)

**Seen (code reading, the same fact check):** the Dev page's Land and Travel buttons set
fly mode on (lib.rs ~5965-5966, ~6082-6083) and lib.rs ~3571 keeps it on every frame,
while the HUD still reads "WALK x1 [F9 to fly]" (hud.rs ~374-387). With fly mode on: no
footsteps (the stride meter needs it off, lib.rs ~15693-15695), the weather never reaches
body heat (survival_env.rs ~178-181 uses the indoor default), and swimming runs at 5 m/s
instead of about 2.5 (lib.rs ~4830-4831 is the swim cap; walking is 5 m/s either way).
A player who lands believes they are walking. (The speed detail was corrected the same day
by the fixer of the navigation guides: the first version said walking.)

**Fix (d7ee64a38), the HUD half:** the movement line read only the F9 hover bit;
`hud::movement_line` now reads both, so it says FLY whenever fly mode is on ("FLY x1 -
gravity on [F9 to hover]"), and Travel's free flight in space shows "FLY x1" again.
Test `marker_and_mode_tests::the_movement_line_reads_fly_whenever_fly_mode_is_on`, seen
red first (it read "WALK x1 [F9 to fly]").

**Still open (lib.rs, not touched by the fix):** Land should put the player down in walk
mode; F9 should flip from `dev_fly_mode || dev_hover`, not the hover bit alone; turning
the dev tools off should clear `dev_hover` as well as fly mode; and the Dev page's "Fly
mode" checkbox hover text does not describe what it does.

## BUG-150: the home's machines take the player's carried items: the sawmill eats stocked logs (FIXED v0.1459.0, found 2026-10-04)

**Seen (code reading, the same fact check):** automated machines take their real inputs
from the backpack first, even in Dev mode (crafting/mod.rs ~1012-1025, ~1186-1196). The
home's sawmill runs Saw Planks by itself, 2 logs every 5 s (home.ron ~2023-2039 and
~3465, recipes.csv ~68), so the 10 logs the Crafting page's "Dev: stock all materials"
adds are gone within about 10 s, before a player can craft the raft that needs 6 of them.
In real life a machine takes what is put into it or what is in the store it is fed from,
never what is in your pockets.

**Fix (643cc524e):** automated machines count and spend their inputs from home storage
(and, for tap water, the tanks), never the backpack, in the session and through the time
away; the status line says where the machine looks ("waiting for Wood Log x2 in home
storage"). The drone unloads into home storage when the home has it, so the drone,
smelter and workbench chain still runs, and the first quest's "Acquire 3 iron ore" step
counts home storage as well as the backpack. Tests, each seen red first:
`machine_inputs_tests::the_sawmill_saws_the_stored_logs_and_leaves_the_carried_ones_alone`,
`a_machine_with_its_inputs_only_in_the_backpack_waits_for_home_storage`,
`through_the_time_away_the_machines_leave_the_backpack_alone`,
`drone_tests::the_drone_unloads_into_home_storage_when_the_home_has_it` and
`quest_tests::gather_counts_what_the_home_holds_as_well_as_the_backpack`.

**Still open, smaller:** "Dev: stock all materials" supplies no tools, and since BUG-147 a
craft's parts come from home storage but its tools must still be carried.

## BUG-151: opening and shutting the build editor with no edit rewrote the home, ship and machine data files (FIXED v0.1459.0, found 2026-10-04)

**Seen:** by the first run of increment 4's new rig legs (the editor jump). The
editor's opening rebuild set `construction_structure_dirty`, and the choke point in
lib.rs armed the autosave on any dirty flag, so a minute after a player merely
opened and shut the editor, the autosave rewrote `data/homes/<kind>.ron`, the ship
file, `data/machines/home.ron` and `data/machines/ship.ron`, dropping their comments.
In a checkout (the rigs' data folder is the checkout) that rewrote four tracked files.

**Fix:** `arms_autosave(edited, entry_rebuild)` ignores the editor's own opening
rebuild (755018fa0). Test `engine::editor::autosave_tests::opening_the_editor_is_not_an_edit`,
seen red: "the editor's own rebuild as it opens armed the autosave".

## BUG-152: three tests fail under machine load, not on their code (FIXED next release, found 2026-10-04; the rig judges of the same class split out as BUG-161)

**Seen:** while increment 4's fixes were checked, with other builds running: the media
seek tests `a_seek_lands_where_it_was_asked_and_the_picture_agrees`,
`a_seek_with_no_keyframe_in_the_window_still_arrives` and once
`seeking_while_paused_shows_the_frame_it_landed_on` (they wait 5 s of wall-clock time
for frames), and the fleet ledger's `the_fleet_ledger_end_to_end` ("rate_limited",
features.rs ~1980). Each passed alone and on reruns; src/media was unchanged.

**Why it matters:** a check that fails on a busy machine teaches people to rerun it
until it is green, which is how a real failure gets waved through.

**More of the same class, seen the same night:**
- Relay storage tests fail with "failed to build SQLite read pool: timed out waiting
  for connection" when several test builds run at once (world-friction lane, 5
  failures in one run; 8 of 8 passed alone).
- The co-presence rig's `steady_speed` and `meet_steady_speed` judges. v0.1459.0's
  first `--plots` runs, with ten agents compiling, drew the game at 6 to 16 fps with
  single frames of 160 to 465 ms, and three of six orders failed those two checks
  (the remote walker extrapolated to 1.1 to 2.4 m/s against 1.4). The rerun with
  every cargo, rustc and link process held at BelowNormal priority (a scratchpad
  loop, deprioritize-builds.ps1) drew 16 to 24 fps and passed 88/88 in every
  order. The judge cannot tell a starved machine from a regression. (Split out
  2026-10-05 as BUG-161, still OPEN: they are a rig's judges, not cargo tests.)

**Causes (found 2026-10-05, each reproduced under a deliberate load:** CPU-burning
node threads, and for the relay three copies of its 556-test suite running at once):
- **The media seek tests** gave the decoder a fixed 3 or 5 s of wall-clock time to
  reach a seek target while the player's clock (an `Instant`) ran on. A busy machine
  decodes slower than that and the wait ran out. Under 96 burner threads with two test
  processes at once, 20 of 20 runs failed: `a_seek_lands_where_it_was_asked...`,
  `a_seek_with_no_keyframe_in_the_window...`, `seeking_while_paused...` and
  `seek_to_start_rewinds_and_replays` in all 20, `a_short_preroll...` in 15, every
  failure a wait running out ("frames resume after a seek", "the fallback still
  delivers", "a paused seek still shows a frame", "never got past 0.4 s"). The landing
  bound itself never failed: the 4-frame queue keeps the first frame within four
  frames of the target.
- **`the_fleet_ledger_end_to_end`** sends two gives back to back and expects the 200 ms
  limit to turn the second away. The limit measures the time between the relay
  REACHING the two, which is the time it spends on the first: writing it to SQLite and
  flushing it to disk, 2.4 ms on an idle machine. In a reproduced failure that write
  took 437 ms and the second give went through. Beside three relay suites and 64
  burner threads, 6 of 16 runs failed with exactly the reported assertion (the limit
  saw the two 310 to 702 ms apart; in the closest passing runs, 169 and 179 ms).
- **The relay storage tests:** r2d2's `Pool::build` returns only once all 8 read
  connections are open, and gives them `connection_timeout` (5 s); three worker threads
  per pool open them, and every test that opens a database builds a pool. With three
  relay suites at once under load, building a pool took 3.3 s at the median and up to
  5.6 s, and 335 of 1,013 builds failed, every one with no error from SQLite at all.
  In the unchanged suites, 373 of the 375 failures across three runs were this; the
  other 2 were the fleet ledger's.

**Fix (no tolerance widened, no timeout lengthened):** the media and pool tests now wait
on the work's own progress, and fail as stuck only when it stops (30 s with no picture
decoded, or no connection opened) or, for the media waits, when the decoder goes round a
loop or a wait passes an absolute two-minute backstop. The fleet ledger test's fix is of
another kind: its rate limit reads a clock the test moves, while its waits for the
relay's answers keep their 5 s of wall-clock time (`next_game_of`, `wait_until`), so
that test is not free of the wall clock.
- `src/test_clock.rs`: `ManualClock`, a clock a test moves by hand. Test builds only.
- Media (`src/media/mod.rs`, `src/media/tests.rs`): the player's `Clock` reads a test's
  manual clock when it has one (`VideoPlayer::use_manual_clock`, `#[cfg(test)]`; the
  product reads `Instant::now()` exactly as before). The seek tests hold the clock at
  the target and wait on the DECODER (`wait_for_frame_at`: pictures decoded, end of
  pass, thread alive), with no wall-clock budget for the work. A wait still cannot hang
  the run: it fails when the decoder produces no picture for 30 s (stopped), more than
  120 pictures during the wait (twice the fixture's 60 frames: going round a loop), or
  after two minutes in all. The picture cap and the backstop came from the 2026-10-05
  review, which hung the run by injecting a restart where the awaited frame would be
  queued (the stall check counted the looping decoder's pictures as work);
  `a_decoder_going_round_a_loop_fails_the_wait_instead_of_hanging_the_run` keeps that
  fault in the suite. Every check is kept and the landing one is now exact: the first
  frame after a seek is the first frame at or after the target (it was "within 0.35 s
  after it"), and the clock reads the target exactly (it was "within 50 ms"). The paused
  seek keeps the wall clock, because only the wall can show a paused clock standing
  still, checks its position did not move, and since the review seeks to 1.21 s,
  BETWEEN two frames, so only `show_next_frame` can hand out its frame (1.2333 s): at
  1.2 s, a frame time, that frame was due at once and the test passed with the arm
  taken out. Seen red at 1.21 s with the arm taken out. The fallback seek lands exactly
  too since BUG-158 (found on the way) was fixed.
- Relay: the perception rate limit reads `RelayState::perception_now()`, which a test
  can point at a manual clock (`perception_clock`, `#[cfg(test)]`). The ledger test
  moves it 250 ms where it used to sleep, so the gives meant to arrive together are
  0 ms apart however long the relay takes over the first.
  `the_give_limit_reads_the_test_clock_not_the_wall` (fleet_ledger_tests.rs) pins the
  hook; seen red with `perception_now` reading the wall, which also fails the ledger
  test on every machine (its repeated give comes back `rate_limited`).
- Pool (`src/relay/storage/pool.rs`): `build_read_pool` builds through `build_pool`,
  which in a test build builds the same pool (the product's builder: same manager, size,
  checkout timeout and checkout validation) with `build_unchecked` and waits for its
  connections to OPEN, failing only when none has opened for 30 s (`build_for_tests`;
  its error handler logs as the product's does and also keeps the newest error for that
  message); the product still uses `build` and its 5 s (`build_by_deadline`).
  `a_pool_whose_connections_open_slowly_is_waited_for_in_tests` (700 ms a connection
  against a 1 s limit) builds through `build_pool` and passes, and its control shows the
  product's build failing the same pool; seen red with `build_pool`'s test arm switched
  back to `build` (it used to call `build_for_tests` directly, so that switch failed
  nothing: the review). `the_products_build_gives_up_at_its_deadline_and_opens_a_real_database`
  runs the product's build, which nothing else in a test build does; seen red with it
  bypassed.

**Loaded loops after the fix, same loads:** the media seek tests 40 of 40 runs passed
(about 24 s a run instead of 4: the tests waited for the decoder instead of failing);
the ledger test beside three relay suites 16 of 16 passed; the three relay suites
themselves passed 558 of 558 tests in each of 3 runs (375 failures before). Idle:
`cargo test --features native --lib` 3,141 passed, `just verify-relay` 2,226 passed.
These loops ran before the review's changes (the picture cap, the backstop, the paused
seek at 1.21 s, the pool's two tests), which were checked idle only.

**Split out:** the co-presence rig's steady-speed judges above are a rig's judges, not
cargo tests, and this fix does not touch them: BUG-161, OPEN.

**Seen while measuring:** the test runs' temporary databases piling up in the temp
folder, filed as BUG-159. Not the cause here (opening a file there took the same 42
microseconds as in an empty folder).

## BUG-153: the Campfire ability promises a fire with warmth and light, and only heals 3 health (PARTLY FIXED (merging in v0.1463.0), found 2026-10-04)

**Seen (code reading, by the check of the heat, fire and fuel guides; confirmed by the
orchestrator):** `data/abilities.csv` row `campfire` is described as "Build a campfire that
provides warmth light and slow healing". It is a non-offensive ability, so casting it
(`src/systems/abilities.rs`, the self-castable arm) spends 15 energy and restores its
`healing_base` of 3 health, and nothing else: no fire is placed, nothing warms or lights,
and the `campfire_warmth` status effect (`data/status_effects.csv`) is applied by no code.
For a new character it sorts first among castable abilities, so it sits in hotbar slot 1.

**Fixed (2026-10-05):** the ability now builds a real campfire, and the fire is a heat
source the body heat model sees. Step by step:

1. The engine works out where it goes when the cast is pressed: the spot a piece in hand
   would be placed at (`engine::build_place::publish_cast_spot`, the same
   `planet_build::ghost`), handed to the ability system in `abilities::BUILD_SPOT_SLOT`.
   An ability row names what it builds in the new `builds` column of `data/abilities.csv`.
2. The cast goes through the one build path every piece takes, now
   `construction::begin_build` (moved out of the ConstructionSystem's tick): the
   `campfire` blueprint (`data/blueprints/basic.ron`, 6 Raw Stone and 3 Wood Logs, as
   `data/structures.csv` always listed it) is refused with the reason, and nothing spent,
   aboard the ship, under a built roof, where there is no air to burn, or without the
   materials; otherwise its materials leave the pack and its scaffold goes up. Only then
   are the 15 energy spent and the cooldown started. No heal: `healing_base` is 0.
3. Finished, it is lit with its own three logs (2 h), burns them down on the game clock,
   and goes out; E at it puts another log from the pack on (it holds four) and relights
   it when out (`src/systems/construction/fires.rs`). Its fuel is saved with it and burns
   down while the game is closed.
4. While it burns it radiates 16 kW (a Forest Service campground fire ring's burn rate,
   NIST's effective heat of combustion and radiative fraction for wood; the sources are
   in the module's doc), falling off as the inverse square, and `engine::survival_env`
   adds it to the mean radiant temperature of a person near it. On a clear, calm 0 C
   night that is about 16 C at 1.5 m and nothing at 20 m; an out fire gives nothing.
5. The dead `campfire_warmth` effect is deleted, and the description says what it does.

Tests, each seen red on the code before the fix:
`the_campfire_ability_builds_a_campfire_outdoors_from_the_pack`,
`a_campfire_cast_that_cannot_build_spends_nothing` (`src/systems/abilities.rs`);
`a_campfire_by_a_cold_night_keeps_a_body_warmer_than_one_20_m_away` and
`an_out_campfire_gives_no_heat` (`src/engine/survival_env.rs`); with the fire's own
burning, fuel, take-down and warmth tests in `fires.rs`.

**Still open:** the campfire gives NO LIGHT. The renderer's point lights are not
evaluated in the celestial pass, where a planet's ground and everything built on it is
drawn (that pass's light count is 0 by design since v0.1155, `80-fragment-shared.wgsl`),
so a campfire light needs renderer work, not a data entry. Also not modelled: smoke,
sparks or spreading, carbon monoxide, and needing a light (tinder, a match, the Campfire
Kit) to start or relight it. In the Normal play mode nobody leaves the ship, so there the
ability is always refused; it can be used only where the Dev travel tools reach a planet.

## BUG-154: the backup generator runs on Paint, Glue or Crude Oil (FIXED (merging in v0.1463.0), found 2026-10-04)

**Seen (the same check, confirmed):** the generator burns whatever flammable-class item is
in its drum (`src/systems/electrical.rs`, `fuel_ok`: any item whose class is
"flammable"). Crude Oil, Glue and Paint are all that class (`data/items.csv`), the drum
accepts them, and the Store button offers them, so the house runs on a can of paint. A real
generator burns the fuel its engine is made for (gasoline, diesel or propane), and the fuels
guide teaches exactly that.

**Fix:** a generator names the fuels its engine burns, as data: `fuels` on its `Generator`
power role (`src/machines.rs`; item ids from `data/items.csv`). Both homes'
`generator_portable` names `fuel_refined_0`: its own item calls it a "Gasoline electric
generator", the game has no gasoline item, and Refined Fuel, the fuel refinery's product, is
the one that stands for it. The spawn puts the list on the generator's entity
(`ecs::components::BurnsFuels`, `src/engine/home_spawn.rs`), and from there only those fuels
run it (`src/systems/electrical.rs`), its drum takes only those whatever asks (the Store
action in `src/lib.rs`, harvest surplus in `src/systems/farming/mod.rs` and a machine's own
craft output in `src/systems/crafting/mod.rs`, all through `containers::vessel_takes_item`),
and its Store buttons offer only those (`containers::store_offers`, the card's list moved
out of `src/lib.rs`). A fuelled generator whose data names no fuel burns nothing and its
drum takes nothing, with a warning at spawn: no fuel can safely be assumed for it, and
guessing from a class is what put the paint in. Tests, each seen red on the old rule first:
`systems::electrical::tests::paint_in_the_drum_makes_no_power` (it ran on Paint; the test
checks Glue and Crude Oil too), `a_generator_that_names_no_fuel_burns_nothing` (it ran on
Refined Fuel with no list),
`systems::inventory::containers::tests::a_generators_drum_offers_only_its_fuel` (the drum
offered Crude Oil, Glue, Paint and Refined Fuel) and
`engine::home_spawn::tests::the_shipped_generator_burns_its_own_fuel_and_not_paint` (red
twice: the shipped data named no fuel, then, with the data in, the drum offered Paint). The
positive control, `systems::electrical::tests::the_generators_own_fuel_runs_it`, passes on
the old rule by design; it, the shipped-generator test and
`backstop_genset_runs_when_needed_and_burns_its_drum_dry` were seen red against a rule that
burns nothing, so the Paint test cannot pass merely because no genset runs.

## BUG-155: the greenhouse quest asks for a heater that does nothing (FIXED (merging in v0.1463.0), found 2026-10-04)

**Seen (the same check, confirmed):** the Greenhouse Construction quest's step reads
"Build a heater for temperature regulation" (`data/quests/farming.ron`, objective
`Craft(recipe_id: "build_heater")`), but no system gives a built heater any effect: it
warms neither the air, the plants nor the player. The heating guide says plainly that the
heater does nothing; the quest says the opposite. Worse than "does nothing": no machine
catalog had a `heater`, so the crafted `heater_0` could not even be placed.

**Fix (ba1ee5e0b):** the catalogs (`data/machines/home.ron`, `home_solo.ron`) carry a
`heater` machine, placed from `heater_0` like every machine: a 1,500 W electric ceramic
heater (the Lasko 754200), all of its draw heat, on a thermostat at 24 C (a game choice,
the middle of UGA Bulletin 792's 70 to 80 F days), heating only while the electrical sim
powers it and drawing its watts for the share of the time it runs. Its heat goes into the
air it stands in, a grow room's or fruiting tent's own, else the home's own air, through a
linear heat balance stepped with the airs' water and gases in the farming tick
(`src/systems/farming/heat.rs`; numbers and sources in `data/garden/humidity.ron`, THE
HEAT): `C dtheta/dt = Q - G (theta - theta_around) - G_coil theta`, with G = U x walls and
ceiling (U 6.24 W/(m2 K), single glass, UGA B792's R 0.91, a labelled game choice for every
grow room) plus the room's air changes x its heat capacity (FAO-56's cp), and an air
handler's coil taking the warmth of the air it moves. A room's heat passes on to the home's
own air, which loses it through its walls and roof. Each air sits at its own temperature
(the station holds it there) plus what heaters add, and what reads that air reads the sum:
a grow room's humidity, every humidity setpoint, its CO2 and its pests; the home's air
space, so the body heat model inside the home and food spoilage; and the body feels a grow
room's own air where the player stands in one (`engine::survival_env::indoor_air`). The
Garden panel says what each heater is doing. The quest step now reads "Build a space heater
to warm a grow room's air". Tests, each seen red first (on the code before this, in
effect, by making `heat::heaters` find no heater, and by the mutation each test names):
`farming::heat_tests::a_powered_heater_warms_its_room_to_the_steady_state_worked_by_hand`
(300 m3: 1.054 C over, by hand), `one_heater_barely_warms_a_greenhouse_the_size_of_the_family_homes`
(0.163 C), `an_unpowered_heater_warms_nothing_and_asks_for_its_power`,
`the_thermostat_holds_its_setpoint`, `a_heater_outside_the_grow_rooms_warms_the_home_air_the_body_reads`,
`a_grow_rooms_heat_reaches_the_home_air_and_none_is_lost`, `the_heat_step_keeps_every_joule`,
`the_garden_panel_says_what_each_heater_is_doing`,
`the_greenhouse_quests_heater_warms_a_grow_rooms_air` (the quest's words, the recipe's item,
the catalog machine, spawned the way the engine spawns it),
`engine::survival_env::tests::indoors_the_body_feels_the_air_of_the_room_it_stands_in`, and,
with BUG-153's campfire merged, `a_heaters_warm_air_and_a_fires_warmth_add_up`: a heater
warms the air and a fire the surroundings, from that air, so the two add up and neither
replaces the other (seen red with the fire's warmth taken from a fixed 21 C).

**Still open:** the crops' growth does not answer an indoor room's temperature (indoors
they still grow as if every room were inside their range, so the heater warms the plants'
air, their humidity and their pests, but not their growth rate; a design call, because 17
crops' windows exclude the rooms' 21 C); only the air stores heat, so a heated room warms
and cools in minutes; every grow room is taken as single glass rather than its real walls;
rooms with no grow machine share the home's one air, so a heater in a bedroom warms the
whole home by a few hundredths of a degree; a heater's radiant warmth on a body beside it
is not modelled; the thermostat is set in data, not from a dial in the game
(docs/design/in-app-ops.md); heaters can be placed only aboard, not in a shelter built on
a planet (BUG-153's campfire is the planet side); and `data/hvac.ron`'s other heat makers
(heat pump, wood stove) have no machine yet. The never-registered `HvacSystem`
(`src/systems/hvac.rs`) is superseded by this and could be deleted.

## BUG-156: trees float in the air beside the Silverdale waterfront (OPEN, found 2026-10-05)

**Seen:** in v0.1459.0's probe capture of the new vantage `silverdale-home-marker`
(`.probe-rig/sweeps/20261005-071141/silverdale-home-marker.png`, the right third of the
frame): a stand of full-geometry trees west of the camera is drawn with its trunks ending
in open sky, well above the hillside behind them, leaning only by the camera's upward
pitch. The vantage starts the camera 300 m up, then stands on the ground 200 m north of
the Dyes Inlet waterfront and settles for 8 s before the capture.

**Not yet known:** whether the trees keep heights sampled from a coarser terrain level of
detail while the camera was 300 m up (a teleport artifact a walking player would never
see: arriving by teleport and arriving the way a player does have differed before), or whether trees there float
for anyone. First step: capture the same place after a longer settle and after walking
in, and compare each tree's base with the terrain height under it.

## BUG-157: two data files are silently ignored: their field names do not match the code that reads them (FIXED v0.1462.0, found 2026-10-05)

**Seen (by the leaving-the-ship design proposal, confirmed):** `data/docking.ron` writes
`docking_ports: [...]` and `docking_procedures: [...]`, but `src/systems/docking.rs` reads
`ports` and `procedures`; `data/transportation.ron` writes `space: [...]`, but
`src/systems/transportation.rs` reads `space_infrastructure`. Every one of those loader
fields is `#[serde(default)]`, so each mismatched list loads as EMPTY with no error and no
warning: the ports, procedures and space infrastructure written in the data never reach
the game. Three module headers also name data files that do not exist (`data/vehicles.csv`,
`data/ship_classes.csv`, `data/propulsion.csv`).

**Fix:** the data keys renamed to the loaders' names (ports, procedures,
space_infrastructure; no aliases, nothing else read the old names), with
`docking::tests::the_shipped_docking_file_fills_every_list` and
`transportation::tests::the_shipped_transportation_file_fills_every_list`, both seen red
first (the ports and the space infrastructure arrived empty). The three module headers now
say what loads (or that nothing does yet). Both systems are still unwired scaffolds, so
nothing in the game changed. Still open, the class: a lint that every top-level key in a
shipped RON file is a field its loader knows.

## BUG-158: a seek that falls back to decoding from the top shows the clip from its first frame on the way (FIXED next release, found 2026-10-05)

**Seen (while fixing BUG-152):** when a seek finds no keyframe in its rewind window (a
long-GOP file we did not encode), the decode pass retries from the top of the file
(`PassEnd::RetryFromStart`) so the seek always arrives. Its own comment says the retry
lets "the pts filter drop everything before the target", but `decode_thread` hands the
retry `start_s = 0.0`, and `start_s` is both where the demuxer repositions and where the
pts filter starts keeping frames. So the retry queues every frame from the first one, and
as each is at or before the clock (which already stands at the target) the screen plays
the clip from its start at decode speed before it lands, while the sound has already
jumped. With the clock held at a 1.5 s target on the test fixture, the first frame the
player hands out after the seek is the one at 0 s (3 of 3 runs).

**Fix:** `decode_pass` takes the target and a separate `from_top` flag
(src/media/mod.rs): the retry skips only the reposition and still queues nothing older
than the target, which is what its comment always said it did. The screen now holds the
last picture while the retry decodes its way to the target, then lands on the first frame
at or after it, exactly as the fast start does (the next frame for a target between two;
none, so the picture stays, for a target after the last frame).
`a_seek_with_no_keyframe_in_the_window_still_arrives` requires the first frame after the
seek to be the frame at the target, as the other seek tests do; seen red before the fix
with the message above, green 3 of 3 after, and 20 of 20 loaded runs of the six seek
tests (96 burner threads, two test processes at once) passed with it.

## BUG-159: the tests leave their temporary databases and files behind (FIXED, merging in v0.1463.0; found 2026-10-05)

**Seen (by the BUG-152 fix, counted):** the system temp folder held about 182,000 entries,
174,578 of them `hum_*` files left by test runs (76,906 SQLite databases). Fifty-four test
files each build their own path (`std::env::temp_dir().join(format!("hum_..."))`) and
nothing deletes it afterwards, so the pile grows with every `just verify` and every
worktree agent's test run. It did not cause BUG-152's timeouts (opening a file there is
as fast as in an empty folder), but it is disk and directory growth with no end.

**Why nothing deleted them:** most of those tests did end with `remove_file(&path)`, but it
ran while the test still held the database open, which Windows refuses (SQLite opens its
files without delete sharing), and nothing ever removed the `-wal` and `-shm` beside it. A
relay a test starts holds its database in its tasks, which the test's runtime drops only
after the test body (and anything in it) is gone.

**Fix:**
- `src/test_temp.rs` (test builds only): `db(tag)`, `file(tag, ext)`, `path(tag)` and
  `dir(tag)` hand out `hum_<tag>_<pid>_<nanos>_<n>` paths in the temp folder as a guard
  (`TempPath`, derefs to `Path`) that deletes the file with its `-wal`, `-shm` and
  `-journal`, or the folder, when it is dropped, also while a failed assertion unwinds the
  test. A delete that fails because something still has the file open is kept and tried
  again at every later guard drop and once more at exit (an `atexit` hook); whatever is
  still open then is named on stderr.
- The relay's storage keeps the guard inside itself: `Storage::open_temp(tag)`,
  `open_temp_dir(tag)` (a `relay.db` in a folder of its own) and `open_sharing(&guard)`, a
  `#[cfg(test)]` field declared last so it is dropped after the writer and the read pool
  have closed the file. A relay a test starts deletes its database when its last task
  lets go of it. The plots tests that restart a relay on one file share one guard among the
  relays (`plots_db`, `relay_on`), so the file outlives every relay on it.
- 85 path-building sites in 59 files moved onto it: every `hum_*` one, plus the four
  `hos_*` helpers that leaked too (storage.rs, file_browser.rs, own_home.rs,
  ship_structure.rs `temp_path`). The hand-written `remove_file` / `remove_dir_all` lines at
  the ends of tests went; moves.rs keeps its `remove_dir_all`, which is part of the test.
- `test_temp::tests::a_dropped_guard_deletes_what_it_made_even_when_the_test_panics`, seen
  red with the delete taken out of `Drop`: "the guards left these behind after the test
  panicked: [...hum_guard_db_..._2.db-wal, ...db-shm, ...db-journal, ...db,
  ...hum_guard_dir_..._5, ...hum_guard_file_..._6.ron]".
  `a_database_still_open_when_its_guard_drops_goes_once_it_closes`, seen red on Windows
  with the retry taken out: "the database its guard could not delete while open was still
  there after it closed".
- `just clean-test-temp` (scripts/clean-test-temp.js): deletes the `hum_*` entries in the
  temp folder nothing has touched for a day, refuses while cargo, rustc or a test binary
  runs, and prints the count and the space (`--dry-run`, `--dir <folder>`, `--days <n>`).
  Its tests (scripts/tests/clean-test-temp.test.js, added to `just rig-tests`) work in
  scratch folders only; seen red with the age check taken out.

**Measured:** before the fix, every full test run left 740 entries in the temp folder (288
databases, 191 `-wal`, 191 `-shm`, 70 folders): the leftovers of 239 `cargo test --features
native --lib` runs and 82 relay-only runs already there, grouped by the process id in their
names (median 738 and 734 over the last 20 of each). After it, `cargo test --features native
--lib` (3,152 passed) and `just verify-relay` (2,235 passed) left 0 each.

**Still to do:** the old pile (186,523 `hum_*` entries on 2026-10-05) goes with one
`just clean-test-temp` when no build or test run is going. Left on their own names: 24
test paths in 9 files (machines.rs, persistence.rs, save_load.rs, home_structure.rs,
stars.rs, terrain_tiles.rs, assets/mod.rs, cosmos.rs, plant_pass.rs) that delete their own
files when they pass, so they leave something only when a test fails, and the recipe does
not sweep their names; host_node.rs's scratch path, which a passing test never creates; and
tests/federation_two_relays.rs, an integration test, which cannot see a `#[cfg(test)]`
module of the library.

## BUG-160: an empty server address turns into the live server, and five rigs sent one (rigs FIXED v0.1462.0; the game FIXED, merging in v0.1463.0; found 2026-10-05)

**Seen:** v0.1462.0's screens check failed `no_builtin_data`. Its game identified on the
live server (wss://united-humanity.us/ws), read its chat, tried to join its shared world
and was refused its ship, although the rig had pinned the sandbox's config to a dead
loopback port (http://127.0.0.1:9; scripts/lib/rig-gameplay.js). Two faults together:

1. **The rigs (FIXED v0.1462.0).** Five rigs (boot-timing, make-clips, photograph-home,
   probe-sweep, verify-screens) sent `{ server_url: "" }` in their autopilot request, and
   the game applies that over the pinned config (src/engine/ipc.rs,
   `poll_autopilot_request`). They now send no address, so the pin stands. A rig test,
   "no rig's autopilot request sends an empty or public server address"
   (scripts/tests/rig-gameplay.test.js), refuses either in any rig's request; the one
   deliberate clear, verify-live-screen's "No server set" step, carries the marker
   `rig-clears-server:`. Seen red first: run over the committed scripts, the check listed
   all six. The screens check then passed on the same build.
2. **The game (FIXED, merging in v0.1463.0).** Drawing the chat page's connect form filled an empty
   server address with the live server's (src/gui/pages/chat/left_panel.rs, `if
   state.server_url.is_empty() { state.server_url = "https://united-humanity.us" }`), and
   the auto-connect then dials it. So a player who cleared their server is put back on the
   live server just by opening Chat, without pressing anything. Now the form writes no
   server: the empty field shows the official server as a suggestion (`OFFICIAL_SERVER`),
   and only the Connect button turns an empty address into it. The test
   `drawing_the_connect_form_sets_no_server` (src/gui/pages/chat/left_panel.rs) draws the
   form with no server set and checks none is set, nothing may dial, the form was drawn and
   the field shows the suggestion. Seen red first with the old line in place: "drawing the
   connect form set a server" (left: "https://united-humanity.us", right: "").

Checked 2026-10-05 (read-only): no member has joined the live server since 2026-10-01, so
today's rig visits left no rows in its member list.

## BUG-161: the co-presence rig's steady-speed judges fail a starved machine as a regression (OPEN, found 2026-10-04; split from BUG-152 2026-10-05)

**Seen (first filed under BUG-152):** v0.1459.0's first `--plots` runs of the co-presence
rig, with ten agents compiling, drew the game at 6 to 16 fps with single frames of 160 to
465 ms, and three of six orders failed the `steady_speed` and `meet_steady_speed` judges
(the remote walker extrapolated to 1.1 to 2.4 m/s against 1.4). The rerun with every
cargo, rustc and link process held at BelowNormal priority (a scratchpad loop,
deprioritize-builds.ps1) drew 16 to 24 fps and passed 88/88 in every order. The judges
cannot tell a starved machine from a regression, and a check that fails on a busy machine
teaches people to rerun it until it is green. BUG-152 fixed the cargo tests of the same
class; these judges are a rig's (scripts/lib/copresence-judge.js), and nothing changed
for them.

**Fix (not started):** the machine guard holds builds at BelowNormal priority for the
length of a capture (what the scratchpad loop did), and the steady-speed judges report the
frame rate and refuse to judge, as "contaminated" rather than FAIL, when frames run long
enough to break the interpolation the check measures. A judge test feeds a capture with
400 ms frames and expects "contaminated", not a FAIL.

## BUG-162: medicine cures nothing, and food poisoning kills in about 8 minutes (OPEN, found 2026-10-05)

**Seen (by the sanitation guides' fixer, confirmed by reading the code):**
- The inventory's generic **Use** button discards its click (src/gui/pages/inventory.rs,
  `let _ = widgets::compact_button(ui, theme, "Use", ...)`), so a Bandage, Medkit, Advanced
  Medkit or Antibiotics does nothing when used.
- data/status_effects.csv's `dispel_type` column (food poisoning's is "medicine") is not a
  field of `StatusEffectDef` (src/systems/status_effects.rs), so nothing removes an effect.
- Food Poisoning takes 3 health every 15 s for 5,400 s (`data/status_effects.csv`; the tick
  in src/systems/food.rs): from full health it kills in about 8 minutes 20 seconds unless
  Well Fed (1 health a second) or the First Aid ability (35) outpaces it. Under the default
  Simplified death mode a respawn clears it.

**Why it matters:** real food poisoning is not a fast poison. It is mostly fluid loss over
hours to days; most healthy adults recover with fluids and rest, and the danger is
dehydration, worst in babies, older people and anyone already weak (the Library's When Food
or Water Makes You Sick says exactly this). Antibiotics help only some bacterial illnesses,
and the advice is not to take them for ordinary food poisoning. The game teaches the
opposite: an eight-minute death with medicine in the pack that cannot be taken.

**Fix (not started):** illness as fluid loss on the body's water (the vitals already track
it), on the game clock, cleared by time and helped by drinking (oral rehydration more than
plain water), with the severe course for the vulnerable in Realistic and a milder one in the
simplified mode (the dual-mode house rule); the Use button applying each medical item's
effect from data (what it heals, what it removes, what it does not help), so antibiotics
clear only effects marked as bacterial; a test that food poisoning untreated does not kill a
healthy adult in minutes, that drinking shortens it, and that Use on each medical item does
what its data says.
