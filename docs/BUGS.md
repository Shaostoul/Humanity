# Bug Tracker

All known bugs and their resolution status. Check here BEFORE fixing any bug to avoid duplicate work.

## Resolved Bugs

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
- **Description**: User sends a message in #general; text appears in their chat (local echo). WebSocket has a transient drop/reconnect. After reconnect, the user's message is gone from their own view. The server *did* receive and store the message — on a later session it shows up in history. Net effect: user thinks their message was lost, sends it again, ends up double-posting.
- **Root cause(s)**: Two bugs compounded:
  1. **Same-channel-click clears chat_messages** — Every click on a channel/DM/group/scratchpad row in the sidebar called `chat_messages.clear()` and `history_fetched = false` unconditionally, even if the click was on the *active* row. After a connection blip the user often clicks the channel they're already in (to "refresh"), which nuked any local-echoed unsent text.
  2. **HTTP history fetch on reconnect doesn't dedup** against `chat_sent_timestamps`. The WS broadcast handler at `lib.rs:1139` already dedups server echoes of locally-sent messages by matching `(sender_key == my_key) && timestamp ∈ chat_sent_timestamps`. The HTTP `/api/messages` history fetch in the same file (`lib.rs:1830`) ran no such check, so the user's own message reappeared as a duplicate when it came back from history — and since the local echo was likely cleared by (1), the only visible copy was the server's at the bottom of a freshly-fetched 50-message window.
- **Fix**: `src/gui/pages/chat.rs` — every channel-switch site now no-ops when the click target equals `state.chat_active_channel`. `src/lib.rs` — the HTTP history-fetch loop dedups against `chat_sent_timestamps` mirroring the WS broadcast dedup logic.

### BUG-036: Deleted system channels resurrect on every relay restart
- **Status**: Fixed
- **Version Found**: long-standing (since the seed list landed)
- **Version Fixed**: v0.125.0
- **Description**: An admin opens the cog menu on a system channel (welcome/announcements/rules/stream/dev), confirms delete. The channel disappears for the rest of that session. After the next relay restart — which happens automatically on every git push to main via the deploy CI — the deleted channel is back.
- **Root cause**: `src/relay/mod.rs:170-175` re-ran `create_channel("welcome", ...)` etc. on every boot. `INSERT OR IGNORE` only suppresses on conflict; once a channel was deleted the row was gone, so the next restart's INSERT succeeded and resurrected it. The 6 system channels (welcome, announcements, rules, general, stream, dev) were re-seeded every restart with `created_by = "system"`.
- **Fix**: The seed list now runs **once on first boot** and is gated by a `default_channels_seeded` row in the existing `server_state` key/value table. Subsequent boots skip the seed. The catch-all `general` channel is still always ensured (it's protected from deletion server-side anyway). For pre-v0.125.0 deployments, a one-shot migration sets the seeded flag if the messages table already has rows — so existing operators inherit their current channel set rather than re-seeding deleted channels one last time. To deliberately re-seed (e.g. after wiping the database), delete the `default_channels_seeded` row from `server_state`.

### BUG-034: In-app updater corrupted the local exe ("Unsupported 16-Bit Application")
- **Status**: Fixed
- **Version Found**: v0.122.0 (long-standing — every release of build-desktop.yml since the bundle change)
- **Version Fixed**: v0.124.0
- **Description**: The Build Desktop App workflow only published a single asset per platform — `HumanityOS-<platform>.tar.gz` containing the binary plus `data/` and `assets/`. The in-app updater downloaded that asset, wrote the bytes straight to disk, and renamed it to the exe path. The result was a gzipped tar archive masquerading as `HumanityOS.exe`. Windows refused to load it with `Unsupported 16-Bit Application` because the gzip magic bytes look nothing like a PE header.
- **Fix**: Two changes:
  1. `.github/workflows/build-desktop.yml` now also publishes the raw binary (`HumanityOS-windows-x64.exe`, `HumanityOS-linux-x64`, `HumanityOS-macos-arm64`, `HumanityOS-macos-x64`) alongside the existing `.tar.gz` bundle. Bundles still ship for fresh installs that need the data/assets too.
  2. `src/updater.rs::find_platform_asset` now prefers a raw binary asset and **refuses** archive-only releases instead of silently corrupting the install. Pre-v0.124.0 releases will surface "No binary for this platform" — operators must wait for the next tag (which will ship with raw binaries).

## Open Bugs

None currently tracked. Report bugs at https://github.com/Shaostoul/Humanity/issues
