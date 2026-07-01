# Bug Tracker

All known bugs and their resolution status. Check here BEFORE fixing any bug to avoid duplicate work.

## Resolved Bugs

### BUG-044: Spoiled food had zero gameplay consequence -- tracked but never checked when eaten
- **Status**: Fixed
- **Version Fixed**: v0.646.0 (pending release)
- **Reported**: found during the 2026-07-01 overnight autonomous-loop broader stub-completion sweep (repo-wide TODO scan), not operator-reported.
- **Root cause**: `src/systems/food.rs`'s spoilage pass (§3 of `FoodSystem::tick`) correctly ages every food item in every inventory and flips a per-slot `spoiled: bool` once `spoilage_timer >= max_freshness` -- but the EAT handler (§1, drains `consume_request`) resolved nutrition purely from the item's static `NutritionProfile` (by item_id) and never consulted the spoilage side-table at all. A player could eat a fully-spoiled item with full nutrition and zero risk, forever, as long as the item_id's own `raw_consumption_risk` was 0 (true for all cooked/canned/preserved food). The `TODO: Replace item with "spoiled_food" variant or reduce nutrition value` comment right at the spoiled-flip site documented the gap but nothing implemented it.
- **Fix**: the EAT handler now looks up the eaten item's inventory slot (`inv.slots.iter().position(...)`), checks `self.spoilage.get(&(entity_bits, slot_idx))` for `spoiled`, and if true applies a `nutrition_mult` of `0.25` to both satiation and hydration gain AND guarantees `food_poisoning` regardless of the profile's own `raw_consumption_risk`. Fresh food is unaffected (`nutrition_mult = 1.0`, existing risk-roll logic unchanged). New test `eating_spoiled_food_poisons_and_reduces_nutrition` (`src/systems/food.rs::nutrition_tests`), confirmed to actually catch the bug via a temporary revert-and-retest (fails against the reverted code with the exact expected wrong behavior -- no poisoning, full nutrition). Files: `src/systems/food.rs`.

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

## Open Bugs

None currently tracked. Report bugs at https://github.com/Shaostoul/Humanity/issues
