# HumanityOS Features Directory

Complete inventory of every feature, where it lives, and what it does. Updated v0.607.x.

## How to Read This

Each feature lists:
- **What it does** (one line)
- **Web** (browser files, if any)
- **Native** (desktop Rust files, if any) -- all paths relative to repo root under `src/`
- **Server** (backend files, if any) -- relay code lives in `src/relay/`
- **Data** (config/data files, if any)

> **Architecture note (v0.90.0):** The `server/` and `native/` directories no longer exist.
> Everything is a single binary from `src/`. Server relay code is at `src/relay/`.
> Game/renderer/GUI code is at `src/renderer/`, `src/gui/`, `src/systems/`, etc.
> Run `HumanityOS --headless` for server-only mode (VPS, Raspberry Pi).

---

## Civilization Trust Layer (v0.98.0 – v0.109.0)

### Post-Quantum Crypto Core
ML-DSA-65 (Dilithium3) + ML-KEM-768 (Kyber768) + Argon2id + BLAKE3.
- Native: `src/relay/core/pq_crypto.rs`, `src/relay/core/kdf.rs`, `src/relay/core/canonical.rs`, `src/relay/core/object.rs`
- Tests: 14 PQ + 7 KDF + 5 object roundtrip
- Cargo deps: `ml-dsa = 0.1.0-rc.8`, `ml-kem = 0.3.0-rc.2` (with `getrandom` feature), `argon2 = 0.6.0-rc.8`

### Signed-Object Substrate
Generic SQLite-backed table that every higher-level domain (VCs, governance, recovery, AI status, disputes) projects from. Auto-indexes derived domains on insert.
- Storage: `src/relay/storage/signed_objects.rs`
- API: `POST/GET/LIST/COUNT /api/v2/objects` in `src/relay/api_v2_objects.rs`
- Schema registry: `data/identity/schemas.ron`

### DID Resolver
`did:hum:<base58(BLAKE3(pubkey)[..16])>` format. Short enough for QR codes.
- Core: `src/relay/core/did.rs` (parse, format, fingerprint, hex conversion)
- Storage: `src/relay/storage/dids.rs` (resolve to current pubkey + activity metadata)
- API: `GET /api/v2/did/{did}` in `src/relay/api_v2_did.rs`

### Verifiable Credentials
W3C-style VCs over the signed-object substrate. 12 indexed schemas. Issuer-auth-checked revocation, subject-auth-checked withdrawal.
- Storage: `src/relay/storage/credentials.rs`
- API: `GET /api/v2/credentials`, `GET /api/v2/credentials/{vc_object_id}` in `src/relay/api_v2_credentials.rs`

### Multi-Layer Trust Score
0..1 normalized total + 6 transparent sub-scores. Sybil-farm-resistant via graph entropy.
- Storage: `src/relay/storage/trust_score.rs`
- API: `GET /api/v2/trust/{did}` in `src/relay/api_v2_trust.rs`
- Weights: `data/identity/trust_weights.ron`

### Governance
9 proposal types, 5 local-scope, 4 civilization-scope. Vote weight = trust score capped at 0.95.
- Storage: `src/relay/storage/governance.rs`
- API: `/api/v2/proposals`, `/api/v2/proposals/{id}`, `/api/v2/proposals/{id}/tally` in `src/relay/api_v2_governance.rs`
- Types: `data/governance/proposal_types.ron`

### Laws (location-aware rules and rights) (v0.496)
Nested jurisdiction tree (Humanity -> Earth -> country -> state -> county -> locality); pick where you live and see the rules that apply, broadest first. Two kinds: HumanityOS base set (our framework, from the Humanity Accord) and real laws (plain-language summaries with a source, not legal advice). Condense, do not ingest.
- Native: `src/gui/pages/laws.rs` (`GuiPage::Laws`, reached from the Humanity hub "Laws" section), loader `src/gui/laws.rs`
- Data: `data/laws/laws.json` (jurisdictions + rules, hot-reloadable)
- Design: `docs/design/laws.md`. Web mirror is a follow-up.

### AI-as-Citizen
Mandatory `subject_class_v1` declaration + `controlled_by_v1` operator binding. AI excluded from governance voting per Accord.
- Storage: `src/relay/storage/ai_status.rs`
- API: `GET /api/v2/ai-status/{did}` in `src/relay/api_v2_ai.rs`

### Social Key Recovery
Shamir-shared seed via guardians. Server stores opaque ciphertext only.
- Storage: `src/relay/storage/recovery.rs`
- API: `/api/v2/recovery/setup/{holder_did}`, `/api/v2/recovery/shares-held-by/{guardian_did}`, `/api/v2/recovery/request/{request_object_id}` in `src/relay/api_v2_recovery.rs`

### Federation v2
Generic signed-object gossip + per-issuer continuous trust + dispute objects + multi-hop with cycle-breaking via dedup.
- Handler: `src/relay/handlers/federation.rs` (`SignedObjectGossip`, `gossip_signed_object`)
- Storage: `src/relay/storage/issuer_trust.rs`
- Wire format: `RelayMessage::SignedObjectGossip` in `src/relay/relay.rs`

---

## Communication

### Chat (Text Messaging)
Real-time text chat with channels, threads, and message history.
- Web: `web/chat/app.js`, `web/chat/chat-messages.js`
- Server: `src/relay/relay.rs` (WebSocket routing), `src/relay/storage/messages.rs`

### Direct Messages (E2E Encrypted)
Private 1-on-1 conversations encrypted with pure Kyber768 / ML-KEM-768 →
BLAKE3-KDF → AES-256-GCM (dual-seal envelope; the relay stores opaque
ciphertext only). The old ECDH P-256 path was deleted (web v0.263.4, native
v0.264.0), see the canonical crypto table in `CLAUDE.md`.
- Web: `web/chat/chat-dms.js`, `web/chat/pq.js` (`pqDmSeal`/`pqDmOpen`)
- Native: `src/net/dm_pq.rs`
- Server: `src/relay/storage/dms.rs` (zero-knowledge, never decrypts)

### Voice Channels
Group voice chat rooms with join/leave sounds. Voice is per-channel: the voice
room IS the text channel, keyed by the channel's own string id (the relay
validates the channel's `voice_enabled` flag, not a separate voice_channels
table), so clicking a channel's mic joins THAT channel. Web and native join by
the same channel id and are interoperable.
- Web: `web/chat/chat-voice.js`, `web/chat/chat-voice-rooms.js`
- Server: `src/relay/handlers/msg_handlers.rs` (voice_room join/leave + roster), `src/relay/handlers/broadcast.rs` (voice_room_signal relay)

### Native Voice (Live Audio, Pure-Rust)
Full live voice on the desktop app: captures mic → DSP → Opus encode → sends to
each connected peer; receives peers' Opus → per-peer decode → mix → playback.
Native↔web is audible both ways. Pure-Rust stack (no C toolchain): cpal (WASAPI)
capture/playback, unsafe-libopus encode/decode, rtrb ring buffers, str0m WebRTC
media. Mic test loopback with a live level meter accepts any device sample
format (i16/u16/f32) and any rate (streaming linear resampler to/from 48 kHz).
Input stack: mic gain 0–200% (clip-protected); noise FILTER modes Off / Light
(85 Hz biquad high-pass + noise gate) / Noise suppression (RNNoise via the
pure-Rust nnnoiseless crate); TRANSMIT modes Open mic / Push-to-talk /
Voice-activated / Push-to-mute (bindable push key via raw winit input + VAD
threshold). Defaults: Noise suppression + Push-to-talk on CapsLock. WebRTC audio
is strictly opt-in (a connection only negotiates an audio m-line when asked), so
the P2P data mesh is unchanged. The voice-room JOIN registers with the relay
(`{type:voice_room, action:join, room_id}`) and signaling rides the web's
`voice_room_signal` protocol (newcomer-offers / incumbents-wait glare rule).
- Native: `src/net/voice.rs` (capture/DSP/encode/decode/mix/playback, `run_voice_session`), `src/net/webrtc.rs` (str0m bidirectional Opus media + voice signaling), `src/gui/pages/settings.rs` (`draw_audio_content` mic test + input controls), `src/lib.rs` (winit push-key input + signaling routing)
- Config: `src/config.rs` (`VoiceFilterMode` + `VoiceTransmitMode` enums, mic gain / push key / VAD threshold persisted to `AppConfig`)
- Server: `src/relay/handlers/msg_handlers.rs` (per-channel voice_room, `VoiceChannelData.id` is a String channel id), `src/relay/handlers/broadcast.rs` (voice_room_signal relay)

### Voice/Video Calls
1-on-1 WebRTC calls with camera support.
- Web: `web/chat/chat-voice-calls.js`, `web/chat/chat-voice-webrtc.js`

### Screen Sharing / Streaming
Share your screen or stream to a channel.
- Web: `web/chat/chat-voice-streaming.js`
- Server: `src/relay/storage/streams.rs`

### Reactions
Emoji reactions on messages.
- Web: `web/chat/chat-ui.js` (reaction picker)
- Server: `src/relay/storage/reactions.rs`

### Pins
Pin important messages to a channel.
- Server: `src/relay/storage/pins.rs`

### Message Search
Full-text search across channels.
- Server: `src/relay/api.rs` (`GET /api/search`)

### File Upload
Upload images and files to chat (10MB limit).
- Server: `src/relay/api.rs` (`POST /api/upload`), `src/relay/storage/uploads.rs`

### Threads
Reply threads on messages.
- Server: `src/relay/storage/messages.rs` (thread_parent_id, reply_count)

---

## Identity and Security

### Dilithium3 / ML-DSA-65 Identity
Post-quantum cryptographic keypair (Dilithium3 / ML-DSA-65, FIPS 204) IS your identity, derived deterministically from the BIP39 24-word seed. No accounts, no passwords. (Ed25519 survives only as the seed scalar and Solana wallet address, see the canonical crypto table in `CLAUDE.md`.)
- Web: `web/chat/crypto.js` (key derivation), `web/shared/pq-identity.js` (Dilithium signing)
- Server: `src/relay/relay.rs` (signature verification), `src/relay/core/pq_crypto.rs`

### BIP39 Seed Phrase
24-word backup phrase for identity recovery.
- Web: `web/chat/crypto.js` (mnemonic generation/restoration)

### Key Rotation
Rotate keypair with dual-signed certificate (old + new keys).
- Web: `web/chat/crypto.js`
- Server: `src/relay/storage/key_rotation.rs`

### Signed Profiles
Profiles are cryptographically signed objects. Any server can cache and serve them.
- Server: `src/relay/storage/signed_profiles.rs`

### Vault Sync
Encrypted cloud backup of settings/keys (AES-256-GCM + PBKDF2-SHA-256 at 600,000 iterations, both web and native).
- Web: `web/chat/crypto.js` (encryption), `web/chat/chat-profile.js` (sync UI)
- Server: `src/relay/storage/vault_sync.rs`

### Rate Limiting
Fibonacci backoff per public key to prevent spam.
- Server: `src/relay/relay.rs`

---

## Push Notifications

### Push Subscribe/Unsubscribe
Web Push API with VAPID keys.
- Web: `web/shared/shell.js` (registration)
- Server: `src/relay/storage/push.rs`, `src/relay/api.rs`

### Notification Preferences
Per-user DM/mention/task/DND toggles synced to server.
- Web: `web/pages/settings-app.js`
- Server: `src/relay/storage/notification_prefs.rs`

### Notification Actions
Reply and Mark Read buttons on push notifications.
- Web: `web/shared/sw.js` (service worker)

---

## Task Board

### Task CRUD
Create, read, update, delete tasks with title, description, status, priority, assignee.
- Web: `web/pages/tasks.html`, `web/pages/tasks-app.js`
- Server: `src/relay/storage/board.rs`

### Task Comments
Threaded comments on tasks.
- Server: `src/relay/storage/board.rs`

### Project Grouping
Tasks grouped by project with color/icon pickers.
- Web: `web/pages/tasks-app.js` (project modal)
- Server: `src/relay/storage/projects.rs`

---

## Marketplace

### Listings
Create and browse marketplace listings.
- Web: `web/pages/market.html`, `web/pages/market-app.js`
- Server: `src/relay/storage/marketplace.rs`

### Listing Images
Image upload with drag-and-drop galleries (max 5 per listing).
- Server: `src/relay/storage/marketplace.rs`

### Full-Text Search (FTS5)
Search listings by keyword with SQLite FTS5.
- Server: `src/relay/storage/marketplace.rs`

### Reviews and Ratings
Star ratings and text reviews on listings.
- Server: `src/relay/storage/reviews.rs`

### Seller Profiles
Clickable seller names with aggregate ratings and listing count.
- Server: `src/relay/storage/members.rs`

### Buyer-Seller Messaging
Conversation threads on listings.
- Web: `web/pages/market-app.js`
- Server: `src/relay/storage/marketplace.rs` (listing_messages table)

### P2P Trading with Escrow
Direct player-to-player item exchange with dual confirmation.
- Web: `web/pages/trade.html`, `web/pages/trade-app.js`
- Server: `src/relay/storage/trading.rs`, `src/relay/relay.rs`

---

## Social

### Guild System
Create, join, search, and manage guilds with invite codes.
- Web: `web/pages/guilds.html`
- Server: `src/relay/storage/guilds.rs`

### Reputation System
Points, levels, and leaderboard for community standing.
- Server: `src/relay/storage/reputation.rs`

---

## Wallet and Funding

### Solana Wallet
Ed25519 identity IS a Solana wallet address. Send, receive, balance queries.
- Web: `web/shared/wallet.js`, `web/pages/wallet.html`, `web/pages/wallet-app.js`

### Token Swaps (Jupiter)
Swap tokens via Jupiter aggregator API.
- Web: `web/shared/wallet.js`

### Staking
Stake SOL with validators.
- Web: `web/shared/wallet.js`

### NFT Support
Detect and display NFTs with Metaplex metadata.
- Web: `web/shared/wallet.js`

### Donation Page
Funding tracker with progress bar, dynamic multi-crypto address support (unlimited networks).
- Web: `web/pages/donate.html`, `web/pages/donate-app.js`
- Native: `src/gui/pages/donate.rs`
- Data: `data/server-config.json` (funding.addresses array)

### Wallet Guide
Step-by-step beginner guide for all wallet operations (receive, send, buy, sell, swap, backup, glossary).
- Web: `web/pages/wallet-guide.html`, `web/pages/wallet-guide-app.js`
- Access: "?" help icon on wallet page tab bar

### Admin Donation Address Management
Admin settings UI for adding, editing, removing, and reordering donation addresses.
- Native: `src/gui/pages/settings.rs` (Donation Addresses section)

---

## Civilization Dashboard

### Live Community Stats
Aggregated population, infrastructure, economy, resources, social, activity metrics.
- Web: `web/pages/civilization.html`, `web/pages/civilization-app.js`
- Server: `src/relay/storage/civilization.rs`, `src/relay/api.rs` (`GET /api/civilization`)

---

## Web Tools and Utilities

### File Browser/Editor
Tree navigator for data/ directory. Built-in viewers for text, JSON, CSV, markdown, images, audio, video.
- Web: `web/pages/files.html`, `web/pages/files-app.js`
- Server: `src/relay/storage/files.rs`, `src/relay/api.rs`

### Calculator
Basic, scientific, unit converter modes with keyboard support and history.
- Web: `web/pages/calculator.html`, `web/pages/calculator-app.js`

### Calendar/Planner
Monthly/weekly view with event creation, color coding, localStorage persistence.
- Web: `web/pages/calendar.html`

### Notes/Journal
Local-first note editor with auto-save, search, markdown preview, export.
- Web: `web/pages/notes.html`, `web/pages/notes-app.js`

### Tools Catalog
37 free open-source apps across 11 categories with search/filter.
- Web: `web/pages/tools.html`, `web/pages/tools-app.js`
- Data: `data/tools/catalog.json`

### Resources Page
45 curated real-world resource links (education, health, housing, etc.) + in-game guides.
- Web: `web/pages/resources.html`, `web/pages/resources-app.js`

### Glossary System
150+ terms with definitions, searchable tooltip overlay on all pages.
- Web: `web/shared/glossary.js`
- Data: `data/glossary.json`

### Admin Dashboard
Server analytics for admins. Users, messages, channels, federation, game state.
- Web: `web/pages/admin.html`, `web/pages/admin-app.js`
- Server: `src/relay/api.rs` (`GET /api/admin/stats`)

### Server → Services (feature + daemon toggles)
Operator one-click control of features backed by OS daemons (coturn
voice/video relay; future P2P distribution via transmission) from the
native Server Settings page, no SSH. Two layers: a soft
`server_settings` gate (relay stops offering instantly) + an
allowlisted privilege bridge that start/stops the daemon. Non-root
relay + tightly-scoped sudoers + compile-time allowlist (no shell, no
client strings as args); security-reviewed (no HIGH/MEDIUM). v0.262.16.
- Server: `src/relay/services.rs`, `src/relay/relay.rs`
  (`service_control`/`service_state`), `scripts/sudoers.d/humanity-relay-services`
- Native: `src/gui/pages/server_settings.rs` (Services panel)
- Design: `docs/design/services-toggles.md`

### Projects Page
Project Universe timeline (Dec 2017 ICU through Jan 2026 rename to HumanityOS).
- Web: `web/pages/projects.html`

---

## Maps

### Multi-Scale Map
Galaxy to street level zoom on 2D canvas. Galaxy spiral, solar system, planet globe, OpenStreetMap tiles. Moon orbit fixed (v0.90.8).
- Web: `web/pages/maps.html`, `web/activities/map.js`, `web/activities/celestial.js`
- Data: `data/solar-system.json`, `data/stars-catalog.json`, `data/constellations.json`

---

## Navigation and UX

### Shell Navigation
Color-coded nav groups (red=identity, green=contextual, blue=system) with icon tooltips.
- Web: `web/shared/shell.js`

### Real/Sim Toggle
Global context switch between real-life tools and simulation mode. Stored in localStorage.
- Web: `web/shared/shell.js` (toggle UI), pages listen for `hos-context-change` event

### Dark/Light Theme
Theme toggle with CSS variables.
- Web: `web/shared/shell.js`, `web/shared/theme.css`

### Onboarding Tour
8-step guided walkthrough for new users.
- Web: `web/shared/onboarding-tour.js`

### Settings Panel
Gear button with theme, notifications, wallet, and display settings.
- Web: `web/shared/settings.js`, `web/pages/settings.html`

### Localization (i18n)
5 languages (English, Spanish, French, Chinese, Japanese) with fallback.
- Web: `web/shared/i18n.js`
- Data: `data/i18n/*.json`

### Accessibility
High contrast, reduced motion, font scaling, colorblind mode filters.
- Web: `web/shared/accessibility.js`, `web/shared/theme.css`

---

## Server and Infrastructure

### WebSocket Relay
Message routing with authentication, rate limiting, federation.
- Server: `src/relay/relay.rs` (~5800 LOC)

### REST API
50+ endpoints for all platform features.
- Server: `src/relay/api.rs` (~2800 LOC), `src/main.rs` (routing)

### Federation
Server-to-server WebSocket connections, trust tiers, profile gossip.
- Server: `src/relay/handlers/federation.rs`

### Server Membership
Auto-join on connect, paginated member roster, role management.
- Server: `src/relay/storage/members.rs`

### Database Backups
Automated SQLite backup every 6 hours, keep last 5.
- Server: `src/main.rs` (background task)

### Environment Validation
Fail-fast startup with clear error messages for missing config.
- Server: `src/main.rs`

### GitHub Webhook
Signature-verified webhook for CI/CD integration.
- Server: `src/relay/api.rs`

### Game State Authority
Server-side game world with entity management, position validation, player sync. Loads `data/ships/starter_fleet.ron` at startup; populates 6 Pioneer rooms with equipment + windows. Spatial queries (room_for_position, entities_near, room_by_id) for AI perception.
- Server: `src/relay/handlers/game_state.rs`

### AI Perception API (v0.131.0)
Headless gameplay protocol, AI agents perceive and act in the game world via structured JSON instead of rendered frames. Validates distance for interactions (5m), perception range (20m).
- WebSocket messages: `game_perceive` (room + nearby + environment), `game_interact` (action on entity), `game_query_inventory`, `game_query_entity`
- Server: `src/relay/handlers/msg_handlers.rs` (handle_game_perceive, handle_game_interact, etc.), `src/relay/relay.rs` (routing)
- Docs: `docs/ai/onboarding.md` (Playing the Game section), `docs/design/ai_interface.md` (Game Participation role)
- Test script: `scripts/test-perception-api.js`

### Unified Binary Deploy
VPS runs `HumanityOS --headless`. relay.db at `/opt/Humanity/data/`. systemd service updated (v0.90.0).
- Server: `src/main.rs`, `src/relay/`

---

## Native Desktop Client (egui)

### egui GUI System
Immediate-mode UI with theme.ron, 13 reusable widgets, 20+ pages.
- Native: `src/gui/` (theme.rs, widgets/, pages/)
- Data: `data/gui/theme.ron`

### Universal Widgets (v0.90.0)
13 widgets: badge, detail_row, search_bar, sidebar_nav, category_filter, stat_card, button, data_table, icons, item_list, modal, row, toolbar.
- Native: `src/gui/widgets/` (button.rs, data_table.rs, icons.rs, item_list.rs, modal.rs, row.rs, search_bar.rs, stat_display.rs, toolbar.rs, mod.rs)

### Theme System (v0.90.0)
6 new theme colors (bg_panel, bg_sidebar, bg_sidebar_dark, badge styling). Slider widget with blue-green-red gradient + animated RGB knob.
- Native: `src/gui/theme.rs`
- Data: `data/gui/theme.ron`

### Main Menu
Title screen with Play, Settings, Quit. Overlays on 3D scene.
- Native: `src/gui/pages/main_menu.rs`

### Escape Menu
In-game pause/settings overlay.
- Native: `src/gui/pages/escape_menu.rs`

### Settings Page
Graphics, audio, controls, game, account categories with sliders and toggles.
- Native: `src/gui/pages/settings.rs`

### Inventory Page
6-column item grid with selection and detail panel.
- Native: `src/gui/pages/inventory.rs`

### Chat Page (3-Panel, v0.89.0)
DMs (red), Groups (green cards), Servers (blue), message feed, input bar. DMs/Groups headers have settings cog menus (v0.90.0). Server header cog replaces X disconnect (v0.90.0).
- Native: `src/gui/pages/chat.rs`

### Compact Theme (v0.90.3)
All spacing/sizes halved for actual visual density. 35+ theme variables editable in Settings > Widgets.
- Native: `src/gui/theme.rs`
- Data: `data/gui/theme.ron`

### All Pages Refactored (v0.90.1)
All 27 pages refactored to use theme system and universal widgets consistently.
- Native: `src/gui/pages/*.rs`

### Post-Quantum DM Encryption (native)
Native DM encryption uses pure Kyber768 / ML-KEM-768 (FIPS 203) -> BLAKE3-KDF -> AES-256-GCM in a dual-seal `{v:1,r,s}` envelope, byte-identical to the web client. The Kyber recipient key derives deterministically from the BIP39 seed, so DMs round-trip cross-client. (The earlier ECDH P-256 path was deleted in v0.264.0, see the canonical crypto table in `CLAUDE.md`.)
- Native: `src/net/dm_pq.rs` (seal/open), `src/gui/pages/chat.rs` (send/receive UI)

### HUD
Health bar, hotbar, crosshair, compass, day/night indicator, FPS counter.
- Native: `src/gui/pages/hud.rs`

### Maps Page
Multi-scale map with celestial navigation.
- Native: `src/gui/pages/maps.rs`

### Profile Page
User profile view/edit.
- Native: `src/gui/pages/profile.rs`

### Tasks Page
Task board in native UI.
- Native: `src/gui/pages/tasks.rs`

### Wallet Page
Wallet management in native UI.
- Native: `src/gui/pages/wallet.rs`

### Market Page
Marketplace listings in native UI.
- Native: `src/gui/pages/market.rs`

### Crafting Page
Recipe browsing and crafting UI.
- Native: `src/gui/pages/crafting.rs`

### Guilds Page
Guild management in native UI.
- Native: `src/gui/pages/guilds.rs`

### Trade Page
P2P trading interface.
- Native: `src/gui/pages/trade.rs`

### Studio Page
Content creation tools.
- Native: `src/gui/pages/studio.rs`

### Civilization Page
Community stats dashboard.
- Native: `src/gui/pages/civilization.rs`

### Calculator Page
Calculator in native UI.
- Native: `src/gui/pages/calculator.rs`

### Calendar Page
Calendar/planner in native UI.
- Native: `src/gui/pages/calendar.rs`

### Notes Page
Notes/journal in native UI.
- Native: `src/gui/pages/notes.rs`

### Files Page
File browser in native UI.
- Native: `src/gui/pages/files.rs`

### Tools Page
Tools catalog in native UI.
- Native: `src/gui/pages/tools.rs`

### Resources Page
Resources directory in native UI.
- Native: `src/gui/pages/resources.rs`

### Bugs Page
Bug reporting/tracking.
- Native: `src/gui/pages/bugs.rs`

### Donate Page
Donation page with admin address management.
- Native: `src/gui/pages/donate.rs`

---

## Game Engine

### Three-Mode Camera
First-person, third-person, orbit/free with smooth transitions.
- Native: `src/renderer/camera.rs`

### wgpu Renderer
PBR-lite rendering with depth buffer, materials, instanced rendering.
- Native: `src/renderer/mod.rs`, `src/renderer/pipeline.rs`

### PBR Shader with Emissive (v0.90.0)
PBR material pipeline supports emissive strength via params.w.
- Shaders: `assets/shaders/pbr_simple.wgsl`

### 12 Procedural Materials (v0.90.0)
Glass, ice, water, leather, crystal, rust, moss, lava + original brick, metal, wood, concrete.
- Native: `src/renderer/pipeline.rs`
- Shaders: `assets/shaders/procedural_material.wgsl`, `assets/shaders/procedural/*.wgsl`
- Data: `data/materials/procedural_materials.ron`

### Sky Renderer
Time-of-day colors (dawn/day/dusk/night) modified by weather.
- Native: `src/renderer/sky.rs`

### Stars Renderer
Star field rendering for space scenes.
- Native: `src/renderer/stars.rs`

### Hologram Renderer
Holographic display rendering for ship interfaces.
- Native: `src/renderer/hologram.rs`

### Multi-Scale Renderer
Floating-origin and multi-scale rendering for planetary to galactic distances.
- Native: `src/renderer/multi_scale.rs`, `src/renderer/floating_origin.rs`

### Particle System (v0.90.0)
CPU-simulated, GPU-rendered billboarded point sprites. 12 data-driven emitter types from particles.ron (fire, smoke, sparks, rain, snow, dust, magic, explosion, bubbles, steam, ember, lightning).
- Native: `src/renderer/particles.rs`
- Shaders: `assets/shaders/particle.wgsl`
- Data: `data/particles.ron`

### Bloom Post-Process (v0.90.0, partial)
Half-resolution bright-pixel extraction, Gaussian blur, composite. Scaffolding built, needs render loop integration.
- Native: `src/renderer/bloom.rs`
- Shaders: `assets/shaders/bloom.wgsl`

### Sun Direction Uniform (v0.90.8)
Data-driven sun direction passed as shader uniform instead of hardcoded in WGSL.
- Native: `src/renderer/pipeline.rs`
- Shaders: `assets/shaders/pbr_simple.wgsl`

### Planet Registry (v0.90.8)
Unified celestial body management for renderer, terrain, and maps.
- Native: `src/terrain/planet.rs`

### Construction Placement (v0.90.8, partial)
Scaffolded placement system for building in the game world. Needs full integration. **⚠️ `PlacementSystem` NOT registered, never ticks (see the lint).**
- Native: `src/systems/construction/mod.rs`

### GLTF Model Loading
Load .glb/.gltf models with normal and UV fallbacks. Cached by path.
- Native: `src/assets/mod.rs`

### Instanced Rendering
Batched drawing for objects sharing mesh and material.
- Native: `src/renderer/mod.rs` (InstanceBatch)

### Icosphere Planet Terrain
Recursive subdivision from icosahedron. LOD from billboard to walkable surface.
- Native: `src/terrain/icosphere.rs`, `src/terrain/planet.rs`
- Data: `data/planets/*.ron`

### Heightmap Terrain Generation
Procedural terrain from heightmaps with 16 biome types.
- Native: `src/terrain/heightmap.rs`

### Voxel Asteroids
Sparse octree storage, greedy meshing, ore veins by classification, mining.
- Native: `src/terrain/asteroid.rs`
- Data: `data/asteroids/types.csv`

### Ship Interiors
Ship layouts from RON, room mesh generation, BFS pathfinding between rooms.
- Native: `src/ship/layout.rs`, `src/ship/rooms.rs`
- Data: `data/ships/starter_fleet.ron`

### Physics (rapier3d)
Rigid bodies, colliders, raycasting, simulation stepping.
- Native: `src/physics/mod.rs`

### Audio (kira)
Sound effects, music, spatial audio with distance falloff, volume controls.
- Native: `src/audio/mod.rs`, `src/audio/sounds.rs`

### ECS (hecs)
System trait, SystemRunner, 20+ components, per-frame tick.
- Native: `src/ecs/systems.rs`, `src/ecs/components.rs`

### Hot-Reload
File watcher (notify) invalidates asset cache per frame.
- Native: `src/hot_reload/`, `src/assets/mod.rs`

### Multiplayer Networking
WebSocket client (tungstenite), message protocol, ECS sync, position interpolation.
- Native: `src/net/protocol.rs`, `src/net/client.rs`, `src/net/sync.rs`

### Mod Support
Mod manifest format, directory scanning, load order, path override resolution.
- Native: `src/mods/mod.rs`
- Data: `data/mods/README.md`, `data/mods/example-mod/mod.json`

### World Persistence
Save and load game world state (entities, terrain, player progress).
- Native: `src/persistence.rs`

### Data-Driven Tools (v0.90.7)
tools.rs loads tool catalog from external JSON instead of hardcoded data.
- Native: `src/gui/pages/tools.rs`
- Data: `data/tools/catalog.json`

### Data-Driven Sounds (v0.90.7)
sounds.rs loads sound configuration from TOML instead of hardcoded data.
- Native: `src/audio/sounds.rs`
- Data: `data/sounds.toml`

### Chat Tint Colors in Theme (v0.90.7)
Chat channel tint colors moved from hardcoded values to theme.ron for customization.
- Data: `data/gui/theme.ron`

### Server Config Externalized (v0.90.7)
Server constants moved from hardcoded Rust to external JSON configuration.
- Data: `data/server-config.json`

### 8 Game System Modules (v0.90.7)
Scaffolded system modules for expanded gameplay.
- Native: `src/systems/`

### Shader Library
41 WGSL shaders: planet surfaces (earth, mars, venus, mercury, jupiter, saturn, uranus, neptune, moon, pluto), sun surface/glow, PBR, procedural materials (brick, metal, wood, concrete, fabric, aperiodic), stars, constellations, orbit rings, ghost preview, particles, bloom.
- Shaders: `assets/shaders/`, `assets/shaders/procedural/`

---

## Game Systems

> **⚠️ Registration status (2026-05-29 game-code audit):** only **7** of the systems below are actually registered + tick in the runtime, Player Controller, Interaction, Day/Night, Farming, Inventory, Crafting (+ ContainerCompatibility, not separately listed here). Every other system in this section is **implemented but NOT registered**, so it never ticks. `tests/engine_wiring_lint.rs::DEFERRED_SYSTEMS` is the authoritative list (the build fails if a system is neither registered nor deferred-with-reason); `docs/STATUS.md` has the per-system status.

### Player Controller
WASD movement, gravity, jump, ground detection via raycast.
- Native: `src/systems/player.rs`

### Interaction System
Raycast from camera, find nearest interactable entity.
- Native: `src/systems/interaction.rs`

### Day/Night Cycle
GameTime with seasons, sun direction/color computation. 20 real minutes = 1 game day.
- Native: `src/systems/time.rs`

### Weather System
7 conditions (clear, cloudy, rain, storm, snow, fog, sandstorm). Seasonal transitions. **⚠️ NOT registered, never ticks (see the lint).**
- Native: `src/systems/weather.rs`

### Hydrological System
Rain cycle, rivers, aquifers, contamination tracking, water table simulation. **⚠️ NOT registered, never ticks (see the lint).**
- Native: `src/systems/hydrology.rs`

### Atmospheric System
Gas tracking, explosions, suffocation, pressure simulation. **⚠️ NOT registered, never ticks (see the lint).**
- Native: `src/systems/atmosphere.rs`

### Disaster System
21 disaster types with chain reactions, severity scaling, black holes. **⚠️ NOT registered, never ticks (see the lint).**
- Native: `src/systems/disasters.rs`

### Farming
6 growth stages, water/health simulation, seasonal effects.
- Native: `src/systems/farming/mod.rs`

### Inventory
ItemStack slots, add/remove/transfer, max stack from data.
- Native: `src/systems/inventory/mod.rs`

### Crafting
Recipe matching from CSV, input validation, timed crafting.
- Native: `src/systems/crafting/mod.rs`
- Data: `data/recipes.csv`

### Construction
Blueprint placement, snap-to-grid, timed building, material consumption. **⚠️ NOT registered, never ticks (see the lint).**
- Native: `src/systems/construction/mod.rs`
- Data: `data/blueprints/basic.ron`

### Skills/Progression
20 skills across 5 categories, XP curves, level-up notifications. **⚠️ NOT registered, never ticks (see the lint).**
- Native: `src/systems/skills/mod.rs`
- Data: `data/skills/skills.csv`

### AI Behaviors
5 behavior types (passive, aggressive, herd, predator, guard) with state machines. **⚠️ Native `AISystem` NOT registered, never ticks (the relay drives ambient NPCs separately, server-side). See the lint.**
- Native: `src/systems/ai/mod.rs`

### Vehicles/Mechs
Enter/exit vehicles, torso twist, jump jets, heat management. **⚠️ NOT registered, never ticks (see the lint).**
- Native: `src/systems/vehicles/mod.rs`

### Ecology/Disease
Disease spread by proximity, seasonal effects, population tracking. **⚠️ NOT registered, never ticks (see the lint).**
- Native: `src/systems/ecology.rs`

### Quests
Data-driven quest progression from RON files. 6 objective types. **⚠️ Native `QuestSystem` NOT registered, never ticks. (The relay runs the authoritative quest chain, so quests work in multiplayer; the native single-player system does not.) See the lint.**
- Native: `src/systems/quests/mod.rs`
- Data: `data/quests/*.ron`

### Combat
Damage calculation, status effects. **⚠️ `CombatSystem` NOT registered, never ticks (see the lint).**
- Native: `src/systems/combat/`

### Economy
Fleet resource management. **⚠️ `EconomySystem` NOT registered, never ticks (see the lint).**
- Native: `src/systems/economy/`

### Navigation
Multi-scale navigation (galaxy, system, orbital, surface). **⚠️ Support module, not wired into the runtime.**
- Native: `src/systems/navigation/`

### Logistics
Cargo transport and shipping routes. **⚠️ Support module, not wired into the runtime.**
- Native: `src/systems/logistics/`

---

## Construction and Build Editor (v0.455 - v0.606)

The in-app homestead builder. An overlay editor (gated by the `construction_active` flag in
`src/gui/mod.rs`, NOT a `GuiPage` variant) over the 3D viewport: a left object browser + 3D astral
camera + a right details pane + a bottom placement palette. The panel UI lives in `construction.rs`;
the input/gizmo/grab/duplicate/snapshot logic lives in `src/lib.rs`.

### Build Editor Shell
Three-zone editor: resizable left `SidePanel` (search box at top + collapsible sections), center orbit
viewport, right detail pane that routes to the selected object's editor, and a bottom placement palette.
Save/Close pinned to the bottom of the left panel so "Save home" is never off-screen.
- Native: `src/gui/pages/construction.rs` (`draw`, `draw_wall_editor`)
- Flag: `src/gui/mod.rs` (`GuiState.construction_active`)

### Unified Object Browser (v0.596 - v0.598)
One single-line row per object across every type (walls, structures, machines, lights, roads, conduit
nodes), grouped into collapsible per-type sub-headers with counts. A filter box and per-type collapse.
Double-click a row to fly the camera to that object.
- Native: `src/gui/pages/construction.rs` (`draw_object_browser`); filter via `construction_object_filter`; focus via `construction_focus_request` (consumed in `src/lib.rs`)

### Move / Select / Duplicate Gizmos (v0.549 - v0.600)
Tap-to-select vs hold-to-move on every object; drag corner-nodes, machines, openings, lights,
road/conduit nodes, and the player-spawn avatar. Double-click-to-focus. Duplicate the selected object
with Ctrl+D. Grid-snap toggle (0.25 m). Constant-width "line circle" gizmo bounds visible through walls;
the active grabbed gizmo RGB-cycles.
- Native: `src/lib.rs` (`construction_duplicate`, the `construction_*_grab` states), `src/gui/pages/construction.rs` (browser hints, grid-snap toggle)

### Lock Per Object Type (v0.614)
Each object-browser type-group has a "Lock type" toggle: a locked type (walls, structures, machines,
lights, road/pipe nodes) can't be selected or grabbed in the viewport, and shows no `[x]` in the browser
-- so on a busy build you lock your walls while arranging machines and never fat-finger them. The group
title shows `[locked]` in the warning colour. (Viewport HIDE-per-type is a deferred follow-up.)
- Native: `src/gui/mod.rs` (`construction_locked_types`), `src/gui/pages/construction.rs` (the per-group toggle), `src/lib.rs` (the pick dispatch gates each `try_pick_*` on the type not being locked)

### Alignment Snap Guides (v0.613)
While dragging any object, its X and/or Z snaps to the nearest other object within 0.3 m (independent per
axis, applied after grid-snap), and a faint amber guide line spans the box along the snapped axis so you
see what you are lining up with. Walls contribute both corners; the dragged object is excluded.
- Native: `src/lib.rs` (`snap_to_alignment` pure helper + `gather_other_positions`, wired into `apply_object_drag`; the guide line drawn into the construction overlay's `ring_lines`)

### Multi-Select + Group Delete / Nudge (v0.612)
Ctrl+click rows in the object browser to build a multi-select set (across every type -- walls, machines,
lights, structures, road/pipe nodes); selected rows show a `*` and the accent colour. A group-action bar
gives Delete (removes them all, index-keyed types in descending order, id-keyed types via the pruning
helpers so connections stay consistent), Clear, and Nudge (+/-X, +/-Z by 0.5 m, keeping each object's
height). A plain click resets the set to single-selection.
- Native: `src/gui/pages/construction.rs` (`group_delete`, `group_nudge`, the browser's Ctrl+click + group bar), `src/gui/mod.rs` (`construction_multi`)

### Construction Console (AI / dev act surface) (v0.578 - v0.580)
A text-command console, the discoverable act surface for both a human and an AI. Verbs: `help`, `list`,
`add_wall`, `rm_wall`, `set_material`, `add_door`, `add_window`, `set_style`, `add_lock`, `add_light`,
`rm_light`, `add_structure`, `rm_structure`, `add_layer`, `rm_layer`, `add_road_node`, `rm_road_node`,
`add_road`, `rm_road`.
- Native: `src/gui/pages/construction.rs` (`exec_construction_command`, `CONSOLE_VERBS`)

### Live Home JSON Introspection (AI read surface) (v0.576)
Every rebuild writes a machine-readable snapshot of the live home so an AI can READ what the operator is
building, to `debug/home_snapshot.json`.
- Native: `src/ship/home_structure.rs` (`HomeStructure::to_introspection_json`), written by `src/lib.rs` (`rebuild_homestead`)

### CAD Dimension Overlay and Wall Wireframe (v0.545, v0.594)
A live measurement overlay: wall lengths, corner angles, and the angle where a custom wall meets the box
hull; per-wall length labels; a wall-wireframe (layout outline) debug toggle. Master "Helper gizmos" +
dimension-overlay toggles in the "Options / Dev" section.
- Native: `src/gui/pages/construction.rs` ("Options / Dev" header, `construction_dimension_overlay`, `construction_show_helpers`); overlay lines drawn engine-side in `src/lib.rs`

### Footer Placement Palette and Building Info (v0.527, v0.602, v0.605)
Bottom palette with a "Structure" tab plus per-category machine tabs, a 10-column grid, held-item
highlight, expand/collapse. Holding a building shows its info card: category, size, power role, stat
readouts, and its connection points (ports) with direction arrows and per-utility colors.
- Native: `src/gui/pages/construction.rs` (`draw_palette`, `draw_building_info`, `draw_held_structure_info`, `port_line`/`port_color`)

---

## Home Structure (fixed box + interior walls) (v0.532 - v0.591)

The home-construction data model (replaced the old rooms-as-sliding-AABBs approach): a FIXED outer box
(the mothership allotment, default 55x89x3 m steel, glass roof) plus freely-placed INTERIOR WALLS; rooms
EMERGE from the walls via grid flood-fill rather than being placed as boxes.

### HomeStructure Model
The serialized home: box dims + shell/roof material, interior walls, placed lights, placed structures, a
road graph (nodes + edges), and the player spawn point. Loaded at runtime from RON (save preserves the
file's `//` design header); meshes regenerate on edit; rooms detected by flood-fill.
- Native: `src/ship/home_structure.rs` (`HomeStructure`, `load`/`save`, `generate_meshes`, `detect_rooms`)
- Data: `data/blueprints/home_structure.ron` (the authored seed home)

### Interior Walls + Wall Materials (v0.552, v0.585)
Walls are corner-node segment chains with per-wall material, per-wall thickness (down to a 1 mm screen),
and stackable surface LAYERS. The wall material picker shows real engineering values (density, tensile
strength, cost/kg, renewable) while you build.
- Native: `src/ship/home_structure.rs` (`InteriorWall`, `SurfaceLayer`, `WallMaterial`, `wall_materials`)
- Data: `data/blueprints/wall_materials.ron` (8 materials: Steel, Concrete, Oak, Tempered glass, Aluminum, Pine, Granite, HDPE)

### Mitred Corners and Wall Joins (v0.549, v0.558, v0.566, v0.574)
Clean mitred corners where walls meet; round corner columns at 3+-wall joins; mid-span T-junction
clipping so a thick wall doesn't spear through another; corner-node snapping to a shared 5 cm grid.
- Native: `src/ship/home_structure.rs` (`wall_end_miter`, `clip_end_to_walls`, `corner_column`, `quantize_corner`, `CORNER_GRID`)

### Doors and Windows (openings) (v0.533 - v0.578)
Doors and windows are openings placed on still-solid walls, each with a position/width/sill/height,
draggable opening gizmos + edge resize handles, and a data-driven animation STYLE: swing, slide, iris,
rotate, fold, energy, nanowall, fixed. Doors carry auto-open vs manual states + an interaction distance,
and an optional control panel.
- Native: `src/ship/home_structure.rs` (`Opening`, `OpeningKind`; `style` is a data-driven String), `src/systems/door_anim.rs` (style to `PanelMotion`), `src/ship/door_panels.rs` (`panel_placements`, `PanelPlacement`)
- Editor: `src/gui/pages/construction.rs` (`OPENING_STYLES` const)

### Door Control Panels (v0.567)
Walk up to a manual door and press E at its control panel to open it; the panel mounts beside the door at
hand height.
- Native: `src/ship/door_panels.rs` (`control_panel_pos`), `src/systems/interaction.rs`

### Door Locks (v0.570)
Data-driven locks on a door; a door is passable only when every lock is Unlocked or Broken. Lock
interactions: KeyItem, Code (keypad), Knob, Crank (emergency no-power override), Biometric, Panel. Defeat
methods: Lockpick, HackPanel, ShootOut, BlowOpen, CutPower. Power-dependent flag per lock.
- Native: `src/ship/lock_types.rs` (`LockType`, `LockInteraction`, `DefeatMethod`, `LockState`), `src/ship/home_structure.rs` (`LockInstance`)
- Data: `data/blueprints/lock_types.ron` (metal_key, keypad, knob, crank, biometric)

### Per-Home Lights (v0.571 - v0.576)
Data-driven placeable lights; the renderer evaluates up to 8 point lights. Add lights from a picker,
click a light to edit it, drag light gizmos (RGB range sphere + diamond). Energy doors emit light
(emissive-as-light). Sun/global-illumination off toggle.
- Native: `src/ship/home_structure.rs` (`PlacedLight`), `src/renderer/light.rs` (loads the registry), editor in `src/gui/pages/construction.rs` (`draw_lights_editor`, `draw_light_detail`)
- Data: `data/lighting/light_types.ron` (ceiling_panel, warm_lamp, cool_panel, spotlight, strip; kinds Point/Spot/Bar)

### Wall and Door Collision (v0.556)
Geometric first-person collision against walls (substepped so a sprinter can't tunnel a thin wall); door
apertures are walk-through gaps, window spans stay solid (glass blocks).
- Native: `src/ship/wall_collision.rs` (`WallSegment`, `wall_segments`, `resolve`)

---

## Structural Pieces (v0.583 - v0.592)

A data-driven registry of buildable structural pieces, rendered by the construction "Structure" palette.
Add a buildable by adding one `.ron` line; no code.

### Structure Registry
Each piece has an id/label/category, a `kind` (drives behaviour) and a `shape` (placeholder geometry),
size, color, and step count. Kinds: Wall, Stairs (also Ramp via shape), Ladder, Elevator, Teleporter,
Train, Road, Deck. Shapes: Box, Steps, Ramp, Ladder, Frame, Slab.
- Native: `src/ship/structure.rs` (`StructureType`, `StructureKind`, `MeshShape`, `structure_types`, `structure_mesh`, `walk_surface`)
- Data: `data/blueprints/structure_types.ron`

### Walkable Stairs / Ramps / Decks (v0.584, v0.588 - v0.589)
Walk UP stairs and ramps (a ground-height sampler lifts you step to step); a floor/deck piece for
multi-level builds; "place at height" so a deck sits at the top of a staircase.
- Native: `src/ship/structure.rs` (`walk_surface`, `in_footprint`), placement in `src/lib.rs`

### Ladder Climb (v0.589)
Stand at a ladder and hold Space to climb (Shift to descend), step off onto a deck.
- Native: `src/lib.rs` (ladder-climb state), `src/ship/structure.rs` (`StructureKind::Ladder`)

### Elevator Ride (v0.590)
A moving car that carries the player between levels; wait in the shaft to recall it.
- Native: `src/lib.rs` (elevator-car state), `src/ship/structure.rs` (`StructureKind::Elevator`)

### Teleporters (v0.584)
Step through a teleport arch to jump to its paired pad (pair set in the detail panel).
- Native: `src/ship/structure.rs` (`StructureKind::Teleporter`), pairing via `PlacedStructure.pair`

### Train / Rail Line (v0.592)
Pair two train platforms and a rail track connects them.
- Native: `src/ship/structure.rs` (`StructureKind::Train`)

### Roads as a Node Graph (v0.585 - v0.591)
Roads are a node graph (nodes + edges); each edge is a road-class ribbon with a fixed top-to-bottom
material STACK (wearing course down to subgrade). Edge centerlines curve through the graph via
Catmull-Rom splines. Draggable road-node gizmos + per-node detail panels.
- Native: `src/ship/home_structure.rs` (`RoadNode`, `RoadEdge`, `road_edge_centerline`), `src/ship/structure.rs` (`RoadType`, `road_types`)
- Editor: `src/gui/pages/construction.rs` (`draw_roads_editor`, `draw_road_node_detail`)
- Data: `data/blueprints/road_types.ron` (footpath, residential, highway, runway)

---

## Home Power and Electrical Sim (v0.437 - v0.606)

The live electrical simulation for the home, plus the data-driven machine layout it runs on. Both
`ElectricalSystem` and `SolarSystem` ARE registered and tick the live home power sim (`src/lib.rs`).

### Live Electrical System
Each tick: sum active generators, sum enabled consumers, shed load by priority on a deficit, and
integrate the surplus/deficit into battery banks (charge/discharge with the day/night solar swing). As of
v0.607 the flow is PER ISLAND (a generator only feeds loads on its own wired circuit). Publishes a live
`PowerStatus` (generation, consumption, balance, battery Wh, autonomy hours) to the DataStore for the GUI.
- Native: `src/systems/electrical.rs` (`ElectricalSystem`, `integrate_battery`, `PowerStatus`), `src/systems/solar.rs` (`SolarSystem`)
- Data: `data/electrical.ron`
- ECS: `PowerGenerator`, `PowerConsumer`, `Battery`, `PowerCircuit` (island) components

### Home Machine Layout
The data-driven machine layout for the 3D home: a catalog of machine types, placed instances + arrays
(row x col grids), connections, conduit nodes/edges, and self-sufficiency loops. Machines carry a power
role (Solar / Generator / Consumer / Battery) and stat readouts; positioned by absolute box-home
coordinates. Editable live in the construction editor (place / move / wire / inspect).
- Native: `src/machines.rs` (`MachineHome`, `MachineDef`, `MachineInstance`, `MachineArray`, `MachineConnection`, `MachinePower`, `HomeLoop`, `placements`)
- Data: `data/machines/home.ron`
- Editor: `src/gui/pages/construction.rs` (`draw_machine_detail`)

### Buildability Report (v0.524, v0.605 - v0.606)
A design-time validator surfaced in the editor with check marks. Checks: Power source (a consumer needs a
generator/solar), Energy balance (kWh/day generated vs consumed + overnight battery sizing), Wiring (no
connection dangles to a missing machine), Conduits (per power run, validate the pinned cable or auto-pick
the cheapest copper against the load + run length: ampacity + voltage drop), and Power circuit (union-find
over the power graph: every electrical LOAD must share a wired component with real generation; a battery
is storage, not a source).
- Native: `src/machines.rs` (`buildability_report`, `power_circuit_check`, `electrical_islands`, `BuildabilityReport`, `CheckStatus`), `src/gui/pages/construction.rs` (`draw_buildability`)

---

## Utility Wiring (v0.604 - v0.607)

Power, water, air, and data do NOT magically transmit through the air; they travel through cables and
plumbing with real limits (volts, watts, amps, AWG gauge, ampacity, shielded vs unshielded). A machine
declares physical IN/OUT ports by utility. Stages 1-3 shipped; the wire-A-to-B gizmo + the superconductor
upgrade mission are the next stages.

### Conduit / Cable Data Model + Physics
A closed `Utility` enum (Electricity, Water, HotWater, Air, Data, Fuel, Nutrient, Waste);
`Port { utility, dir: In/Out/Bidirectional, label, watts, flow_lpm, anchor }`; a cable registry with real
NEC-ish copper specs (AWG, ampacity, voltage rating, ohm/m, cost/m, grade). `check_cable` computes amps +
round-trip voltage drop into Pass/Warn/Fail; `cheapest_cable_for` is the auto-picker; `awg_to_mm2` for
display.
- Native: `src/utilities.rs` (`Utility`, `Port`, `ConduitType`, `ConductorMaterial`, `Grade`, `check_cable`, `cheapest_cable_for`, `conduit_types`)
- Data: `data/utilities/conduits.ron` (copper 14/12/10 AWG home, 6 AWG industrial shielded, the `sc_room_temp` superconductor upgrade target, two water pipes)
- Design: `docs/design/utility-wiring.md`

### Machine Ports + Conduit Checks (v0.605 - v0.606)
`MachineDef` gained `ports: Vec<Port>` + a `derive_ports()` fallback (electrical ports inferred from the
power role; fluid ports declared); `MachineConnection` gained `spec: Option<String>` (a pinned cable id,
else auto-pick). The Conduits + Power circuit buildability checks consume these. The seed `home.ron` is a
physically connected network (PV + wind + generator to battery bus to loads).
- Native: `src/machines.rs` (`MachineDef::derive_ports`, `MachineConnection.spec`), `src/utilities.rs`

### Runtime Power-Flow Gating (v0.607)
Each spawned power entity carries a `PowerCircuit { island }` from `MachineHome::electrical_islands`, so
`ElectricalSystem` balances + sheds PER ISLAND instead of summing the whole world. A load on an
unconnected circuit is shed (no magic transmission). Entities without the component fall into one shared
bucket (the old global behaviour, for tests/legacy).
- Native: `src/ecs/components.rs` (`PowerCircuit`), `src/machines.rs` (`electrical_islands`, `power_component_roots`), `src/systems/electrical.rs`

### Live Water / Plumbing Sim + Power Coupling (v0.608)
The water mirror of the electrical sim, and the first POWER -> WATER consequence chain. A machine's
water producers/consumers derive from its PORTS (`flow_lpm`); a cistern's capacity from
`MachineDef.storage`; `water_islands` groups them per pipe network. `PlumbingSystem` fills/drains the
cistern per island and publishes a live `WaterStatus` (production, demand, stored, days autonomy). A
producer/consumer flagged `needs_power` only flows while the SAME entity is powered -- cut the power and
the pump stops, the cistern drains. Shown on the Home page next to Live power.
- Native: `src/systems/plumbing.rs` (`PlumbingSystem`, `WaterStatus`), `src/ecs/components.rs` (`WaterTank`, `WaterProducer`, `WaterConsumer`, `PlumbingCircuit`), `src/machines.rs` (`water_islands`, `MachineStorage`, `water_production_lpm`/`water_demand_lpm`/`water_capacity_l`), `src/gui/pages/homes.rs` (Live water card)
- Data: `data/machines/home.ron` (cistern storage + rain inflow, pump water-out, tower/irrigation water-in)

### Water to Food Coupling (v0.611)
The downstream end of the power to water to food consequence chain. The `FarmingSystem` reads the live
`WaterStatus`: if the home has a real cistern and it has run DRY, automated irrigation can no longer top
crops up, so they dehydrate and lose health (existing crop water-stress logic). Cut the power, the well
pump sheds, the cistern drains over days, then the garden starts to wilt. Absent water system / no
cistern = water available (un-plumbed homes + tests unchanged).
- Native: `src/systems/farming/mod.rs` (the `water_available` gate on the per-area irrigation top-up)

### Node-Based Conduits (v0.535, v0.581)
Conduit junction nodes + auto-routed edges in the editor (draggable node gizmos), plus the
Manhattan/service-height auto-router that runs pipes up to the ceiling and down to the fixture
(auto-placing brackets, elbows, and wall-passthrough gaskets).
- Native: `src/machines.rs` (`ConduitNode`, `ConduitEdge`, `ConduitEnd`), `src/ship/conduits.rs` (`ConduitKind`, `ConduitRoute`, `route_conduit`)
- Editor: `src/gui/pages/construction.rs` (`draw_conduit_node_detail`)

---

## Game Data

### Chemistry Database
118 elements, 59 alloys, 132 compounds, 35 gases, 52 toxins across 5 CSV datasets.
- Data: `data/chemistry/elements.csv`, `data/chemistry/alloys.csv`, `data/chemistry/compounds.csv`, `data/chemistry/gases.csv`, `data/chemistry/toxins.csv`

### Solar System Database
70+ celestial bodies with orbital parameters, physical properties, and RON planet definitions.
- Data: `data/solar_system/bodies.json`, `data/solar_system/earth.ron`, `data/solar_system/mars.ron`, `data/solar_system/sun.ron`

### Materials Database
92 materials with properties and categories.
- Data: `data/materials.csv`

### Components Database
102 components for crafting and construction.
- Data: `data/components.csv`

### Items Database (expanded v0.90.0)
404 items for crafting, construction, and gameplay.
- Data: `data/items.csv`

### Recipes Database (expanded v0.90.0)
371 recipes for crafting and construction.
- Data: `data/recipes.csv`

### Plants Database (expanded v0.90.0)
161 plants with growth stages, climate requirements, and harvest data. Expanded from 21 to 161.
- Data: `data/plants.csv`

### Creatures Database (v0.90.0)
123 creatures with behaviors, stats, habitats, and loot tables.
- Data: `data/creatures.csv`

### Spells Database (v0.90.0)
149 spells across multiple schools of magic with mana costs, cooldowns, and effects.
- Data: `data/spells.csv`

### Structures Database (v0.90.0)
163 structures for construction with material costs and placement rules.
- Data: `data/structures.csv`

### Status Effects Database (v0.90.0)
80 status effects (buffs, debuffs, conditions) with duration and stacking rules.
- Data: `data/status_effects.csv`

### Enchantments Database (v0.90.0)
133 enchantments for equipment with tier scaling and compatibility rules.
- Data: `data/enchantments.csv`

### Trade Goods (v0.90.0)
185 trade goods with balanced pricing, weight, categories, and regional availability.
- Data: `data/trade_goods.ron`

### Factions (v0.90.0)
Faction definitions with relations, territories, and reputation thresholds.
- Data: `data/factions.ron`

### Biomes (v0.90.0)
Biome definitions with flora, fauna, climate parameters, and resource distribution.
- Data: `data/biomes.ron`

### Tech Tree (v0.90.0)
Technology progression tree with prerequisites, costs, and unlock rewards.
- Data: `data/tech_tree.ron`

### NPCs (v0.90.0)
NPC definitions with dialogue triggers, schedules, and trade inventories.
- Data: `data/npcs.ron`

### Dialogues (v0.90.0)
Dialogue trees with branching choices, conditions, and consequences.
- Data: `data/dialogues.ron`

### Particle Emitters (v0.90.0)
12 particle emitter definitions (fire, smoke, sparks, rain, snow, dust, magic, explosion, bubbles, steam, ember, lightning).
- Data: `data/particles.ron`

### Sound Configuration (v0.90.0)
Sound effect and music configuration with volume, spatial, and category settings.
- Data: `data/sounds.toml`

### Offline Behaviors (v0.90.0)
Autonomous agent presets for off-screen NPC simulation (patrol, trade, farm, build, explore).
- Data: `data/offline_behaviors.ron`

### Simulation Systems (v0.90.0)
Data-driven simulation modules for engineering and infrastructure. **⚠️ Most consuming systems are still
unregistered scaffolds (see `tests/engine_wiring_lint.rs::DEFERRED_SYSTEMS`); their data files exist but
nothing consumes them at runtime. EXCEPTIONS now LIVE: `ElectricalSystem` + `SolarSystem` (home power
sim) and `PlumbingSystem` (home water sim, v0.608) -- see "Home Power and Electrical Sim" + "Utility
Wiring" above. The old `plumbing.ron`/`WaterFixture` scaffold was removed when PlumbingSystem went live.**
- Data: `data/electrical.ron`, `data/hvac.ron`, `data/transportation.ron`, `data/fire_system.ron`, `data/docking.ron`

### Real-World Systems (v0.90.0)
Data definitions for social and biological simulation. **⚠️ The consuming systems are scaffolds, NOT registered, they never tick (see the lint); data exists but isn't consumed at runtime.**
- Data: `data/governance.ron`, `data/psychology.ron`, `data/medical.ron`, `data/food_system.ron`, `data/economy.ron`, `data/creative_arts.ron`, `data/aging_fitness.ron`

### Science Systems (v0.90.0)
Data definitions for natural science simulation. **⚠️ The consuming systems are scaffolds, NOT registered, they never tick (see the lint); data exists but isn't consumed at runtime.**
- Data: `data/geology.ron`, `data/oceanography.ron`, `data/astronomy_tools.ron`, `data/genetics.ron`, `data/manufacturing.ron`, `data/waste_management.ron`

### Data Schemas (v0.90.0)
22 TOML schema files documenting all data formats for modding and validation.
- Data: `schemas/*.toml` (item, material, component, creature, spell, structure, status_effect, enchantment, recipe, quest, biome, celestial_body, faction, npc, skill, sound, vehicle, weather, economy, offline_agent, equipment_slot, container)

### Platform Brand SVGs
Platform detection icons (Steam, Epic, GOG, PlayStation, Xbox) as inline SVGs.
- Assets: `assets/icons/platforms/`

### Total: 108 data files, ~3000+ entries (v0.90.8)

---

## UI Foundation (v0.92.0)

### Design System Spec
Canonical reference for tokens, components, and dual-UI parity rules. Must be read before any widget or page work.
- Docs: `docs/design/ui-system.md`

### Infinite-of-X Principle
Enforced rule: anything that can exist more than once is a data file, not code. Includes a pre-ship checklist and current audit of hardcoded instances.
- Docs: `docs/design/infinite-of-x.md`

### Theme Token Pipeline
Single source of truth for colors, spacing, radii, fonts. Native reads `data/gui/theme.ron` directly. Web's `theme.css` is regenerated from the same file by a Node script. Editing the RON updates both UIs after running the generator.
- Data: `data/gui/theme.ron`
- Native: `src/gui/theme.rs`
- Web: `web/shared/theme.css` (auto-generated section marked with comments)
- Script: `scripts/gen-theme-css.js`

### Universal Help Modal
`data/help/topics.json` is shared between web and native. Both UIs load it on startup and show the same help content. Help buttons (`?`) anywhere in the UI open a themed modal with the topic body.
- Data: `data/help/topics.json`
- Native: `src/gui/widgets/help_modal.rs` (help_button + draw fn + HelpRegistry loader)
- Web: `window.hosHelp.register/show` in `web/shared/shell.js`, plus `[data-help-id]` attribute on any button

### Real/Sim Help Icon
Built-in help topic `real-sim` explains the context toggle. Rendered as a `?` next to the Real/Sim pill in both the native nav bar and the web hub nav.
- Native: `src/gui/pages/escape_menu.rs`
- Web: `web/shared/shell.js` (buildContextToggle)

### Onboarding Page (dual UI)
First-run orientation plus permanent reference. Four core concepts, core-pages overview, data-driven quest chains. Progress tracked locally per step.
- Data: `data/onboarding/quests.json` (three chains, 14 steps)
- Native: `src/gui/pages/onboarding.rs`, `GuiPage::Onboarding` enum variant
- Web: `web/pages/onboarding.html`
- Route: `/onboarding` (web), "Onboarding" nav tab (native)

### Universal Spreadsheet / Nested-Row Widgets (v0.400 - v0.517)
The one-panel inventory redesign's reusable primitives: a nested expandable row, a fixed-width row cell,
a collapsible section disclosure, item swatch tiles, and the recursive nested-container renderer (person
to shirt to pocket to wallet spatial inventory) with cross-container item transfer that persists across
restart.
- Native: `src/gui/widgets/mod.rs` (`expandable_row`, `row_cell`, `section_disclosure`), `src/gui/pages/inventory.rs` (`draw_container`, `item_tile`), `src/gui/mod.rs` (`Place`, `PlacedItem`)

---

## Developer Tooling

### Headless UI Snapshots
Renders native egui pages to PNGs via an offscreen egui-wgpu + wgpu pipeline (no extra dependency; egui_kittest was rejected over an accesskit / egui-winit 0.31.1 incompatibility), so the native UI can be reviewed without launching the app. Output lands in `tests/snapshots/`.
- Native: `src/gui/ui_snapshots.rs`
- Output: `tests/snapshots/` (PNG)
- Recipe: `just snapshots`

### Build / Verify Recipes
Convenience recipes for the pre-push gate. `just verify` runs both feature builds (native + relay) plus lib tests and lints; `just lints` runs the four `src/gui` file-scanner lints via standalone rustc (Windows-PDB-safe, dodges the LNK1318 limit); `just snapshots` renders the UI PNGs; `just preflight` checks untracked source + doc links then runs verify.
- Recipes: `Justfile` (`verify`, `lints`, `snapshots`, `preflight`)

### Crash-Safe Logging (v0.601)
A file logger that tees every log line to disk (flushed per line) plus a panic hook, so a windowed exe
that crashes leaves the cause on disk even with no console. Truncated `run.log` per launch + an appended
persistent `crash.log`, under `%APPDATA%/HumanityOS/logs` (Windows) / `~/.local/share/HumanityOS/logs`
(Linux).
- Native: `src/lib.rs` (`init_logging`, the `std::panic::set_hook` panic hook, `log_dir`)
