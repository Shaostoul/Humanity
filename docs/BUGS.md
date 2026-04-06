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

## Open Bugs

None currently tracked. Report bugs at https://github.com/Shaostoul/Humanity/issues
