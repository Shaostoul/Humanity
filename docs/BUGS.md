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

- **BUG-052** (bottom of this file): Settings VSync OFF panics the app at boot.

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

## BUG-052: Settings VSync OFF panics the app at boot (OPEN, v0.1073.1)

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
