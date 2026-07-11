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

### Native Governance page: live feed + signed voting + proposal form (v0.660)
The native Governance page went from static instructional text to fully live: background
fetch joins GET /api/v2/proposals with each proposal object's CBOR payload (title/body) and
its weighted tally; proposals render as expandable rows (OPEN/CLOSED chip, scope, voting
window, tally bars for yes/no/abstain weights); voting and proposal creation build
`vote_v1`/`proposal_v1` signed objects with the SAME in-crate `ObjectBuilder` +
`DilithiumKeypair` the relay verifies with (zero cross-language canonicalization risk) and
POST them as `SignedObjectSubmission` JSON. Hardened by adversarial review pre-commit:
proposals from a previously-connected server are cleared before rendering (a vote must
never reference server A's proposal while POSTing to server B); the feed and vote buttons
gate on `server_connected`; an in-flight fetch aimed at an old server is replaced, not
awaited; a consecutive-failure circuit breaker bounds the per-proposal join against a
hung server; vote-success wording is honest about the server keeping the FIRST vote per
identity (INSERT OR IGNORE, votes are final); session vote-tracking resets on identity
switch. 7 regression tests, including a full round-trip through the real relay storage and
a wire-struct deserialization lock. Web's vote button remains a stub (real web voting
needs canonical-CBOR signing in JS + a cross-language KAT -- tracked as its own item).
- Native: `src/gui/pages/governance.rs` (fetch/build/post/draw + tests), `src/gui/mod.rs` (`governance_*` GuiState fields; `apply_pq_identity` clears per-identity vote tracking)
- Snapshot: `src/gui/ui_snapshots.rs::snapshot_governance` (state-injected feed render)

### Laws (location-aware rules and rights) (v0.496, chips v0.661)
Nested jurisdiction tree (Humanity -> Earth -> country -> state -> county -> locality); pick where you live and see the rules that apply, broadest first. Two kinds: HumanityOS base set (our framework, from the Humanity Accord) and real laws (plain-language summaries with a source, not legal advice). Condense, do not ingest. v0.661: the data file's own `categories` list (loaded since v0.496 but never surfaced) renders as clickable filter chips (Rights, Privacy, Work and money, ...), and the BASE/REAL kind badge is a real bordered chip instead of bare text.
- Native: `src/gui/pages/laws.rs` (`GuiPage::Laws`, reached from the Humanity hub "Laws" section; `kind_chip` + category chip row), loader `src/gui/laws.rs`
- Data: `data/laws/laws.json` (jurisdictions + rules + categories, hot-reloadable)
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

**P2P group voice** (native, v0.642.0): native's group-channel voice icon was
previously a `// TODO` no-op. Now wired to the same `voice_room` protocol,
using the group channel's synthetic `"group:<id>"` id as the room_id. The
relay's join handler special-cases this prefix and gates it on real GROUP
MEMBERSHIP (`Storage::is_group_member`) rather than the `channels` table's
`voice_enabled` flag (which has no row for a group room at all) -- verified
live against a local relay: a member joins silently, a non-member gets
"You are not a member of this group." **Known follow-up**: group voice rooms
don't yet appear in the `voice_channel_list` broadcast
(`build_voice_channel_list_msg`, `src/relay/handlers/broadcast.rs`, only
enumerates the `channels` table) -- join/leave and the underlying
`VoiceRoomSignal` audio signaling both work correctly (your own client's
`voice_joined` state and the existing-participant dial-in both work), but the
roster list won't show OTHER participants in a group voice room yet. Real
fix: extend that function to also report ad-hoc `state.voice_rooms` entries
with no backing `channels` row.

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
Share your screen or stream to a channel. Server-wide master switch
(`video_streaming_enabled`, off by default) plus per-role `can_stream`
(mod/admin by default) both gate who can start one. Verified end-to-end
live against a local relay (2026-07-01 overnight-loop priority #2 sweep):
start/stop, viewer join/leave, and stream chat send + persistence all work
correctly. Found and fixed a real bug in the process (v0.645.0): the
persisted `viewer_peak` was fed the LIVE viewer count at leave/stop time,
which is only ever highest right at a join and decreases from there -- by
the time a stream actually ends (viewers have often already left), the
recorded peak was frequently 0 or far below the true maximum. `ActiveStream`
now tracks a `peak_viewers` high-water mark, updated on every join, used
instead of the live count when persisting. 4 regression tests
(`src/relay/handlers/msg_handlers.rs::stream_tests`), proven to actually
catch the bug via a revert-and-retest.
- Web: `web/chat/chat-voice-streaming.js`
- Server: `src/relay/storage/streams.rs`, `src/relay/handlers/msg_handlers.rs` (`handle_stream_start/stop/viewer_join/viewer_leave/chat`)

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

### Shared-File Library (v0.675)
Public library of real, useful files (3D-printable parts, models, designs) shared by
people on the server. Uploads sent with `share=1` are publicly listed via
`GET /api/uploads` (search + limit params) and EXEMPT from the per-user media FIFO,
so a shared .blend never vanishes because its uploader later posted chat photos.
Chat auto-shares only 3D/model formats (`.blend .stl .obj .gltf .glb`) -- attaching
one signals intent to publish; photos and other chat media stay unlisted/private.
Original filename is preserved for display (stored name is timestamp-mangled).
Removal (v0.709): signed `POST /api/uploads/delete` — the uploader can remove their
own file; an admin can remove any (Dilithium-signed "delete_upload" request).
- Server: `src/relay/api.rs` (`GET /api/uploads`, `share` query param on upload, `POST /api/uploads/delete`), `src/relay/storage/uploads.rs` (`list_shared_uploads`, `delete_shared_upload`, FIFO exemption + tests), schema `user_uploads` (+`shared`, `original_name`, `size_bytes`)
- Web: `web/pages/shared-files.html` (browse/search/download), `web/chat/chat-messages.js` (auto-share on attach)
- Native (v0.710): Files page "Shared files on the server" manager — list (auto-load + Refresh), Upload via the in-app file browser (share=1), per-row Remove (own files, or any as admin). `src/gui/pages/files.rs`

### In-App File Browser (native, v0.708)
Universal file-picker widget (NOT the OS dialog — the all-in-one direction): quick
roots (Home/Downloads/Documents/Desktop/Game data/App folder), dirs-first ci-alpha
listing, extension filtering (incl. compound `.tar.gz`), max-size guard. Feeds chat
attach (v0.708) and the Files page shared-uploads manager (v0.710).
- Native: `src/gui/widgets/file_browser.rs` (`FilePickerState`, `file_picker_modal`), chat attach wiring in `src/gui/pages/chat.rs` (`ATTACH_EXTS`, `upload_file_blocking`)

### Unread Indicators (native, v0.715–v0.719)
Sidebar-wide unread story, matching the web client's renderUnreadDots:
DM rows show a decrypted last-message preview line + unread dot (v0.715; own sends
show "You: …"; opening clears); group headers (v0.717) and channel rows (v0.718)
get the same dot, preserved across group_list/channel_list rebuilds; the Chat nav
tab paints a theme.danger() dot whenever ANY dm/group/channel is unread, visible
from every page (v0.719). P2P-group unread waits for native P2P push (closed P2P
groups only poll their list — no message event exists to flag).
NOTE: the WEB DM sidebar deliberately stays name-only (operator 2026-05-27; its
stored DM bodies are opaque E2EE envelopes) — do not "fix" either side for parity.
- Native: `src/lib.rs` (dm/group_msg/chat WS handlers), `src/gui/pages/chat.rs` (row painters), `src/gui/pages/escape_menu.rs` (nav dot)

### Saved Servers: add / switch / forget (native, v0.712–v0.714)
Add Server accepts a bare host (assumes https, v0.714); clicking a saved server's
name switches to it (reconnects with the same identity, lands on #general;
v0.712–713); a small "x" forgets the bookmark. The active server shows "(current)".
- Native: `src/gui/pages/chat.rs` (saved-servers loop + `draw_add_server_modal`)

### System Health Panel (native, v0.720)
Server Settings → admin tab → "System health": read-only live snapshot of the
connected server via its public `/health` + `/api/stats` — status, deployed build
(git commit; a stale deploy is visible in-app), humanized uptime, messages stored,
connected peers. Worker-thread fetch + Refresh. In-app ops slice 1 native parity
(`docs/design/in-app-ops.md`).
- Native: `src/gui/pages/server_settings.rs` (`draw_system_health_admin`), `src/gui/mod.rs` (`SystemHealth`)

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
- Web: `web/pages/settings-app.js` (full toggle set + DND time range)
- Native: `src/gui/pages/chat.rs`'s DM-list "DM Notifications" button (v0.641.0) fetches +
  toggles the DM flag only; mentions/tasks/DND are fetched and preserved (so the DM toggle
  never clobbers them) but have no native UI control yet -- a later increment should add a
  proper Settings-page section mirroring the web client's, for full dual-UI parity.
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

### Native Market Sync (v0.752)
The native Market page speaks the same WS listing protocol as web:
`listing_browse` on view, broadcast-synced list (`listing_new` / `listing_updated` /
`listing_deleted`), Publish via `listing_create` (client-minted id, the broadcast
round-trip is the confirm), Delete on own listings. `GuiListing` mirrors the relay's
`ListingData`; the wire contract is pinned by a frame round-trip test.
- Native: `src/gui/pages/market.rs`, `src/gui/mod.rs` (`GuiListing::from_relay_json`),
  lib.rs WS dispatch arms (`listing_list` / `listing_new` / `listing_updated` / `listing_deleted`)

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

### Crew Chore AI (v0.663; nameplates v0.667)
Relay-side crew NPCs work through a data-driven chore rotation instead of the old Brownian wander: walk (straight line, no pathfinding yet) to a chore's room, dwell there "working" for its duration, rotate to the next chore allowed for their role. Deterministic rotation staggered per crew member; chore state + the human-readable label live in the entity's components (`chore`, `activity`, `chores_done`) so world snapshots / AI perception carry them automatically. State transitions plus 2 Hz travel positions broadcast as `game_npc_update` while at least one player is in the world. Native client spawns/interpolates `RemoteNpc` entities and renders amber humanoid markers for them. Nameplates SHIPPED (v0.667): the HUD floats each crew member's name over their head out to 40 m, plus the live chore line ("Taking reactor readings") within 15 m -- accent-colored while working at the site, muted while walking to it. Rebuilt every frame from the RemoteNpc components via `GuiState::crew_labels`, drawn through the machine-label world_to_screen path.
- Server: `src/relay/handlers/game_state.rs` (ChoreDef, tick, tick_chore_agent, next_chore_index, step_toward), `src/relay/mod.rs` (broadcast loop)
- Native: `src/net/protocol.rs` (NetMessage::NpcUpdate), `src/net/sync.rs` (RemoteNpc), `src/lib.rs` (route_game_message + render pass + crew_labels refresh), `src/gui/pages/hud.rs` (crew_label_lines + nameplate draw)
- Data: `data/npc/chores.ron` (14 chores across 6 rooms), `schemas/chore.toml`

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
Multi-scale map with celestial navigation (nav label "Maps"; internally the
`GuiPage::Cosmos`/`GuiPage::Maps` enum variants are aliases that both render
the same page, since v0.203.2). Real Kepler orbital mechanics (`src/cosmos.rs`,
one `SolBody` set + one propagator shared by the Maps page, FPS world spawn,
and the ECS position resolver -- see `src/ecs/cosmos.rs`), a "Focus" button
(one-shot camera snap-to a body) and a "Track" button (continuous camera
follow as the body orbits, v0.647.0).
- Native: `src/gui/pages/cosmos.rs` (the OLDER `src/gui/pages/maps.rs` is dead
  code as of this fix -- zero callers anywhere; `GuiPage::Maps` has forwarded
  to `cosmos::draw` since v0.203.2, found + corrected 2026-07-01 during the
  overnight autonomous-loop broader stub sweep, which had already found this
  same "superseded file left in place" pattern 3 times this session --
  `src/renderer/sky.rs`, `src/systems/navigation/orbital.rs`,
  `src/systems/skills/learning.rs`)

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
Content creation / streaming rehearsal tools: scenes, sources, resolution/bitrate/FPS,
chat overlay. Real mic-level meter (v0.658, `crate::net::voice::mic_level()` -- reads
0 unless a mic test or live voice session is actually capturing, matching the page's
"rehearsal only, no real transport yet" honesty fix from v0.652.0). First-ever adoption
of the built-but-previously-unused help_modal system (4 topics: scenes/sources, stream
settings, chat overlay, program/preview) -- see the Help Modal entry below.
OBS-style Program/Preview split (v0.664): clicking a scene stages it into PREVIEW only;
a "Cut to Program" transition button deliberately pushes it live. Source editing always
targets the preview working set while PROGRAM renders a frozen snapshot from the last
cut, so a streamer can rearrange safely mid-broadcast. Center canvas is two panes side
by side (Program left / Preview right) when wide, one pane + toggle when narrow. Scene
list marks PGM/PRE. State + transitions live in `GuiState.studio` (`StudioState::
select_preview_scene` / `cut_to_program`, unit-tested; persists across page switches).
Headless snapshot: `just snapshot studio`.
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
Donation page. As of v0.659 native fetches the CONNECTED server's real funding info
(GET /api/server-info `funding.addresses` + `goal_usd`/`goal_label`) on connect --
previously only the web client did this and native's list stayed empty unless a
self-hosting operator hand-typed addresses into Settings. Server-fetched funding
lives in `donate_addresses_server` (never persisted, so it can't clobber the
operator's local Settings list), is cleared/discarded whenever the connected server
changes (money-routing data: server A's addresses must never display as server B's
-- `GuiState::apply_server_funding` + tests), and the old hardcoded fake
"$350 / $1000" progress bar is replaced by a card showing the server's REAL goal
only when one exists. Preference order on the page: server list > local Settings
list > legacy fallback.
- Native: `src/gui/pages/donate.rs` (`build_donation_sources`), `src/gui/mod.rs` (`ServerInfo.funding`, `DonateAddress::from_funding_json`, `GuiState::apply_server_funding`), `src/lib.rs` (connect-time fetch in the `peer_list` handler + per-frame drain)

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

### Procedural Sky-Planet Surfaces (v0.763)
Sky planets render as fractal-surfaced icospheres instead of flat-color
spheres: seeded FBM elevation displaces land (oceans stay smooth at the
sphere radius), per-face colors classify from elevation + latitude (ocean,
shore, lowland, highland, mountain, polar cap; dark basins on dry worlds),
plus a translucent fresnel atmosphere shell for bodies with air. Subdivision
level is screen-size-driven (one more level per doubling of projected pixel
size; threshold + max level + master toggle live in Settings, Graphics,
Planets, persisted in AppConfig). Meshes cache per (body, level). Colors ride
packed in the UV channel (PBR shader material types 12/13), so no new vertex
layout or pipeline. Planet defs HOT-RELOAD (v0.764): saving a
`data/planets/<id>.ron` mid-game re-reads the defs + drops cached meshes, so
palette/noise/sea-level tuning shows in the sky within a frame.
- Native: `src/terrain/planet_surface.rs`, `src/terrain/planet.rs`
  (`lod_level_for_pixels`), `src/renderer/mesh.rs` (`from_planet_surface`),
  `assets/shaders/pbr_simple.wgsl` (types 12/13), sky loop in `src/lib.rs`
- Data: `data/planets/earth.ron`, `mars.ron`, `moon.ron` (palette + noise
  params per body; a new planet look = a new RON file)

### Chunked Planetary LOD (2026-07-11)
When a heightmap-bearing planet's disc overflows the screen (the LOD
ladder's level-8 rung), the whole-sphere path hands off to a quadtree of
surface patches whose detail follows the camera: 20 icosahedron roots, each
patch a 16x16-tessellated spherical triangle (256 grid + 96 skirt
triangles), refined by screen-space error down to depth 13 (~54 m triangle
edges near the surface; the ~1 m goal is a documented follow-up at depth
~19 + micro-detail synthesis). Per-patch f64 anchors keep sub-cm precision
(vertices are small f32 offsets; translations compose in f64 and narrow
last). Culling: horizon cone test (the far side costs zero geometry) +
frustum planes, both during tree descent; built patches carry measured
radial bounds for tight culls. Depth-scaled skirts seal cross-depth seams.
Streaming: budgeted builds (6/frame, worst-error first), restricted descent
(no holes ever), 256 MB LRU cache (64 MB warm floor after departure, roots
pinned, renderer slots recycled). Below the heightmap's 0.1 deg resolution,
3 octaves of seeded land-masked detail noise (~30 m) keep close terrain
from going flat. Toggle in Settings > Graphics > Planets ("Chunked surface
detail"), persisted in AppConfig.
- Native: `src/terrain/planet_chunks.rs` (pure math + 17 headless tests),
  sky loop in `src/lib.rs` (selection/build/draw), `src/renderer/mesh.rs`
  (`placeholder` slot recycling)
- Data: any `data/planets/<id>.ron` with a `heightmap` (Earth today)

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

> **Registration status (recounted 2026-07-08 after the combat arc; older per-system notes below may be stale):** **24** systems are actually registered + tick in the runtime (`src/lib.rs`, grep `system_runner.register`): Time/Day-Night, Weather, Solar, Electrical, Plumbing, Atmosphere, Player Controller, Interaction, Farming, Inventory, ContainerCompatibility, Crafting, Construction, Manufacturing, Food, Drone, Vehicles, Livestock (v0.751), Ability (v0.753), Combat (v0.760), AI (v0.761), Economy (v0.747), Skills, Quests. Systems still **implemented but NOT registered** never tick (Ecology, Hydrology, Disaster, Psychology, and the scaffold-tier systems). `tests/engine_wiring_lint.rs::DEFERRED_SYSTEMS` is the authoritative live list (the build fails if a system is neither registered nor deferred-with-reason) -- always cross-check THAT before trusting any per-system note below, since these notes silently go stale as systems get registered over time. `docs/STATUS.md` has the per-system status.

### Combat (v0.760 - v0.765)
The X1 combat arc live end to end at the wolf tier. CombatSystem (registered
v0.760): hits arrive via the `damage_events` channel, armor mitigates per
damage type, death inserts Dead, kills roll the creature's authored loot table
(chance-gated, min..=max counts) into the killer's pack when the PLAYER landed
the blow, creature corpses decay after 20s. AISystem (registered v0.761):
predators hunt prey INCLUDING the player, close distance under their own
locomotion, and bite (~15% of beast health per 1.5s, kinetic). Combat deaths
publish "killed by a Wolf" to the death screen. Armor-on-equip: worn
equipment.csv armor columns rebuild the player Armor component every frame,
per-type sums capped 0.85. Livestock flee living predators within 12m at
double speed (v0.762). Kills train melee (kinetic) / ranged (else) via
xp_grants. Melee (v0.765): F swings the worn hands-slot weapon (tool/blade
equipment rows: axe 14, spear 15 at 3m, pickaxe 12...) or bare hands (3 dmg);
offensive abilities cast at the faced creature (v0.760). Wild spawns are data.
- Native: `src/systems/combat/` (system, `player_swing`, damage types),
  `src/systems/ai/mod.rs` (behavior machine + bites), lib.rs bridges (swing,
  loot settle, armor rebuild, cast targeting)
- Data: `data/entities/wild_spawns.ron`, `data/creatures.csv` loot tables,
  `data/equipment.csv` damage/armor columns, 128 generated loot items in
  `data/items.csv` (`scripts/gen-loot-items.js`)

### Livestock (v0.751)
CreatureRegistry parses all 92 `data/creatures.csv` species (new `renewable_product`
column: `item:amount:regrow_seconds`). Starter herd (chickens/goats/sheep) placed by
`data/entities/livestock.ron` near the outdoor field plots, wandering on per-animal
lissajous graze paths. Walk-up `[E]` collects the regrown product (egg/milk/wool)
volume-gated into the pack, grants farming XP + a `harvest_<creature>` quest event.
Placeholder block bodies sized from real species mass.
- Native: `src/systems/livestock.rs` (registry, spawn list, collect, LivestockSystem),
  `src/ecs/components.rs` (`Creature`, `Harvestable`), lib.rs walk-up + collect bridges + render pass
- Data: `data/creatures.csv`, `data/entities/livestock.ron`, `wool_0` in `data/items.csv`

### Abilities (v0.753)
AbilityRegistry parses all 110 `data/abilities.csv` rows (renamed from spells.csv;
new `flavor` column: real | tech | fantasy). AbilitySystem drains `ability_request`
casts: skill gate (level-1 gates baseline-open), energy cost (mana + stamina columns
both pay from the energy vital), live cooldowns. v1 effects are self-scoped healing
(first_aid, cauterize, repair, heal...); offensive rows load but honestly wait for
the combat arc. Profile > Skills gains the Abilities panel with Cast buttons.
- Native: `src/systems/abilities.rs`, `src/gui/pages/profile.rs` (panel),
  lib.rs bridge (`pending_cast`, `ability_status`, `ability_cooldowns`)
- Data: `data/abilities.csv`

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
7 conditions (clear, cloudy, rain, storm, snow, fog, sandstorm). Seasonal transitions. **Registered, ticks live** (`WeatherSystem` is NOT in `tests/engine_wiring_lint.rs::DEFERRED_SYSTEMS` -- this "NOT registered" note was stale, corrected 2026-07-01 during the overnight loop's registration-status sweep).
- Native: `src/systems/weather.rs`

### Hydrological System
Rain cycle, rivers, aquifers, contamination tracking, water table simulation. **⚠️ NOT registered, never ticks (see the lint).**
- Native: `src/systems/hydrology.rs`

### Atmospheric System
Gas tracking, explosions, suffocation, pressure simulation. **Registered (v0.617), ticks the home's sealed EnclosedSpace + publishes live AirStatus** (`AtmosphereSystem` is NOT in `DEFERRED_SYSTEMS` -- the lint's own comment there notes it was removed at v0.617; this "NOT registered" note was stale, corrected 2026-07-01).
- Native: `src/systems/atmosphere.rs`

### Disaster System
21 disaster types with chain reactions, severity scaling, black holes. **⚠️ NOT registered, never ticks (see the lint).**
- Native: `src/systems/disasters.rs`

### Farming
6 growth stages, water/health simulation, seasonal effects. Plantable grow
areas beyond towers (v0.738 grain loop): beds/trays/fields render as Garden
groups with a Plant button (crop = edit-modal Crop field or the medium's
`default_crop` in grow_media.ron; survival consumes one seed per unit,
replant is idempotent); bed crops carry the grow-area MACHINE id in
CropInstance.tower_id + tower_slot = unit index, so grouping/irrigation/
harvest reuse the tower paths. 8 `grain_<plant>_0` harvest items (dry_goods,
silo-compatible) + fruit_sunflower_0. Garden slots render as a compact TILE
GRID with one detail card (v0.739) and each group has a "Harvest N ready"
bulk button (`harvest_many_request` channel). Harvest overflow routes to a
compatible home vessel (v0.729 silo routing).
- Native: `src/systems/farming/mod.rs`, `src/gui/pages/inventory.rs` (Garden)
- Data: `data/plants.csv`, `data/garden/grow_media.ron`, `data/items.csv`

### Inventory
ItemStack slots, add/remove/transfer, max stack from data. Volume tracking
(v0.726, material-storage Stage A slice 1): every item carries `volume_l`
(items.csv column generated by `scripts/gen-item-volumes.js` from mass /
material density × per-category packing fraction; `RECOMPUTE_MATS=` env to
refresh a material's rows); `Inventory.volume_current_l` recalculated every
tick beside weight, `volume_capacity_l` default 65 L; the Inventory page shows
a Volume tile + per-item Volume detail row. Slice 2 ENFORCEMENT (v0.727):
`add_item_volume_gated` caps by remaining litres on transfers/crafting/
harvest/compost; `outputs_fit` volume headroom pauses auto-machines; a
volume-legal add GROWS the slot grid (v0.735 — volume is the real pack limit,
raw `add_item` stays slot-bound for bandolier-likes). Transfer UX (v0.736):
right-click an item tile for a Stash/Move/Take context menu at the cursor,
or DRAG a tile onto any container header (accent highlight + floating label).
Auto machines draw recipe inputs from HOME STORAGE backpack-first (v0.737):
lib.rs mirrors the organize-layer placed items into a `home_stock` map
pre-tick; crafting counts + consumes; the diff drains back post-tick.
- Native: `src/systems/inventory/mod.rs`, `src/gui/pages/inventory.rs`,
  `src/systems/crafting/mod.rs` (home_stock), `src/lib.rs` (bridge)
- Data: `data/items.csv` (volume_l), `data/materials.csv` (densities)

### Typed Containers (volume-capped vessels)
Volume-capped, content-class-typed vessels (barrels, tanks, jars:
capacity_liters, no-mixing, wrong-class damage/breakage) with registry +
tests. WIRED (v0.728+): `MachineDef.container_type` in home.ron spawns a
`Container` component on the machine entity (grain silo bin, steel fuel
drum); the walk-up machine card shows "Holds: Nx item" + Take + per-item
Store buttons; harvest overflow and refinery output fill them; the backstop
genset burns its drum's flammable contents (`src/systems/electrical.rs`).
Always pre-check `registry.check().is_accepted()` before `try_store` — a
wrong-class store DAMAGES the vessel by design.
- Native: `src/systems/inventory/containers.rs`, `src/machines.rs`
- Data: `data/containers/types.csv`, `data/machines/home.ron`

### Machine Walk-Up Cards (live)
Distance-LOD machine labels (dot → name → stat card) with LIVE values
(v0.724): a per-frame pass patches each label's stats from its entity via
`MachineInstanceId` — cistern litres from the plumbing sim, battery kWh from
the electrical sim, "low" status under 15%. The pinned (E-opened) card grows
an "Auto-build:" recipe dropdown (v0.725): same-station recipes.csv rows,
selection rewrites the entity's AutoRefine (home.ron auto_recipe = default
only). The dropdown is its own interactable Area — the HUD layer is
deliberately paint-only (v0.461 click-eating lesson).
- Native: `src/gui/pages/hud.rs` (cards + `draw_machine_recipe_selector`), `src/lib.rs` (live-stat + selector bridges), `src/ecs/components.rs` (`MachineInstanceId`)

### Crafting
Recipe matching from CSV, input validation, timed crafting.
- Native: `src/systems/crafting/mod.rs`
- Data: `data/recipes.csv`

### Construction
Blueprint placement, snap-to-grid, timed building, material consumption. **⚠️ NOT registered, never ticks (see the lint).**
- Native: `src/systems/construction/mod.rs`
- Data: `data/blueprints/basic.ron`

### Skills/Progression
20 skills across 5 categories, XP curves, level-up notifications. **Registered, ticks live** (`SkillSystem` is NOT in `DEFERRED_SYSTEMS` -- this "NOT registered" note was stale, corrected 2026-07-01). Note: `src/systems/skills/learning.rs`'s `Skill`/`add_practice` is a SEPARATE, unused struct with its own unresolved TODO (learning-curve level thresholds) -- it has zero callers anywhere in the tree and is not what the live, registered `SkillSystem` actually uses; treat it as dead/superseded code, not a gap in the live skill system.
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
Data-driven quest progression from RON files. 6 objective types. **Registered, ticks live in native single-player too** (`QuestSystem` is NOT in `DEFERRED_SYSTEMS`; this "NOT registered, native single-player doesn't work" note was stale, corrected 2026-07-01. The relay separately runs the authoritative quest chain for multiplayer -- that part remains accurate.)
- Native: `src/systems/quests/mod.rs`
- Data: `data/quests/*.ron`

### Combat
Damage calculation, status effects. **⚠️ `CombatSystem` NOT registered, never ticks (see the lint).**
- Native: `src/systems/combat/`

### Economy
Fleet resource management. **⚠️ `EconomySystem` NOT registered, never ticks (see the lint).**
- Native: `src/systems/economy/`

### Economy Automation Phase 1: the living production chain (v0.663)
The operator's living-ecosystem loop, running with ZERO player interaction after one
click: commission a mining drone (with "Keep mining" checked) and raw asteroid ore
becomes finished tools on its own. Three pieces:
- **AutoRefine machines** (`data/machines/home.ron` `auto_recipe` -> `AutoRefine` ECS
  component): a machine continuously runs one recipe against the HOME inventory
  whenever the inputs are in stock -- the smelter auto-runs `smelt_iron` (drone ore +
  coal -> ingots), the NEW workbench machine auto-runs `craft_hammer` (ingot + plank
  -> hammer). Inputs consumed from home stock, timed batch per machine (machines run
  concurrently), outputs land back in home stock. Deliberately no skill gate: owning
  the machine is the unlock. Data-driven -- any machine gains auto production by
  adding `auto_recipe` in the RON, no code.
- **Drone standing orders**: the Mining panel's "Keep mining" checkbox turns a
  commission into a standing order (`auto_mine_order`); the drone re-launches the
  same trip after every delivery until the asteroid depletes (which removes the
  target and ends the loop naturally) or the box is unchecked.
- **Game-time economy clocks** (`systems::time::scaled_dt`): craft timers, drone
  phase timers, and manufacturing progress all scale with `time_scale`, so
  "accelerated for testing" speeds the whole economy, not just the wall clock
  (previously they all ran on raw frame dt and ignored the time scale).
Locked by 5 new tests, incl. `full_chain_drone_ore_becomes_a_hammer_untouched`: one
drone commission -> ore -> auto-smelted ingot -> auto-crafted hammer, untouched.
- Native: `src/systems/crafting/mod.rs` (AutoRefine arm + completion redirect), `src/systems/mining.rs` (standing order), `src/systems/time.rs` (`scaled_dt`), `src/systems/manufacturing.rs`, `src/ecs/components.rs` (`AutoRefine`), `src/machines.rs` (`MachineDef::auto_recipe`), `src/lib.rs` (spawn + bridge), `src/gui/pages/inventory.rs` ("Keep mining")
- Data: `data/machines/home.ron` (smelter `auto_recipe` + the new workbench machine)

### Economy Automation Phase 2, Stage 1: Vehicle Kits (v0.677)
The operator's staged vehicle pipeline (decision 2026-07-02: BOTH item and world-spawn
models, staged). Stage 1 makes big end-products CRAFTABLE as an oversized flat-pack
"kit" ITEM that reuses the whole existing crafting/storage chain, then DEPLOYS into a
real, persistent Vehicle entity standing in the world:
- **Kit items + recipes are pure data**: `truck_pickup_kit_0` / `rover_kit_0` rows in
  `data/items.csv` (category `vehicle`, subcategory `kit`), workbench recipes in
  `data/recipes.csv` (steel + iron + rubber), and the kit->vehicle mapping with body
  proportions in `data/vehicles/kits.ron` (`VehicleKitRegistry`). Adding a deployable
  vehicle is data rows, no code (infinite-of-X).
- **Deploy action**: the inventory item card shows Deploy for vehicle kits ->
  `pending_deploy_kit` -> `deploy_kit_request` channel -> `VehicleSystem::handle_deploy`
  (first registration of VehicleSystem; its enter/exit/mech arms stay dormant until
  Stage 3). Registry lookup happens BEFORE consume so an unknown kit never costs the
  item; survival consumes the kit, creative deploys free (same semantics as crafting).
- **The vehicle is real world content**: spawned 6 m in front of the camera at floor
  level facing the player's look direction, rendered as body/cabin/4-wheel primitives
  scaled from the registry (build-once meshes, drone-dock pattern), and persisted in
  `WorldSave.deployed_vehicles` so a parked truck survives an app restart.
Locked by 8 tests incl. one-kit-cannot-become-two-vehicles and the save round-trip.
Next: Stage 3 (physical transport the player can follow or take over).

### Economy Automation Phase 2, Stage 2: Factory World-Spawn (v0.679)
Factories now END the production chain in a vehicle standing on the lot, not an item
on a shelf. The new `vehicle_assembler` machine (data/machines/home.ron, placeable
from the build palette) auto-runs `assemble_rover`: steel/iron ingots + rubber in the
home stock become a REAL rover that rolls out onto the pad 3 m in front of the
machine -- so drone -> smelter -> assembler is now mine-ore-to-vehicle, untouched.
- **Vehicle-class outputs**: any recipe output the kit registry resolves as an
  assembled vehicle world-spawns at the factory pad instead of the inventory
  (`CraftingSystem::deliver_outputs`, shared by timed + instant completion). A full
  backpack cannot stall the line (`outputs_fit` skips vehicle outputs); a machine
  despawned mid-batch still delivers at the pad captured at start; a MANUAL
  assemble craft rolls the vehicle out in front of the player.
- **Machines have a world pose now**: every home-machine ECS entity carries a
  Transform (resolved position from load_world; raw offset in menu mode) -- the
  factory pad, and the anchor for future per-machine spatial behavior.
- Factory-spawned vehicles are the same persistent Vehicle entities Stage 1 deploys
  (rendered by the same pass, saved in `WorldSave.deployed_vehicles`).
Locked by 5 tests incl. the full-backpack line, mid-batch despawn, and a data lint.
- Native: `src/systems/crafting/mod.rs` (`deliver_outputs`, `ActiveCraft::pad`), `src/lib.rs` (machine Transform)
- Data: `data/machines/home.ron` (`vehicle_assembler`), `data/recipes.csv` (`assemble_rover`/`assemble_truck`)

### Economy Automation Phase 2, Stage 3 slice 1: Vehicles Move (v0.680-v0.682)
The first moving vehicles. The Inventory page's **Vehicles section** lists every
vehicle standing in the world (name, live distance, status); **Summon** a distant
one and it drives itself to you -- straight-line travel on game time, yawing to
face its motion, parking within 4 m ("En route to you..." while moving). Stage 2's
pad lanes automatically reuse the slot a summoned vehicle vacates.
- `VehicleRoute { dest, speed_mps, arrive_radius }` ticked by VehicleSystem;
  removed on arrival. Transit is deliberately NOT persisted (a mid-transit save
  restores the vehicle parked in place, consistent with drone flights).
- Per-vehicle speeds in `data/vehicles/kits.ron` (truck 8 m/s, rover 6 m/s).
- **Live production status (v0.681)**: the same section shows one honest line per
  auto machine each tick -- "Assemble Rover — 42%", "waiting for Steel Ingot x6",
  "pad full, line paused" -- covering assembler, smelter, and workbench (from the
  operator's field test: the assembler sat idle with no rubber and nothing said so).
- **Docking sequence (v0.682)**: the hangar drone lifts off (~2 s to +4 m) before
  vanishing and settles back down on return, instead of popping.
- Web parity: none by design -- these are views into the live 3D world, which the
  web client does not render.
Remaining Stage 3: follow-cam, take-over driving (VehicleSystem's dormant
enter/exit arms), the buy-side order flow (gated on the wallet/currency decision).
- Native: `src/systems/vehicles/mod.rs` (summon + `tick_routes`), `src/systems/crafting/mod.rs` (`auto_craft_status`), `src/gui/pages/inventory.rs` (Vehicles section), `src/net/sync.rs` (crew grounding, v0.681), `src/lib.rs` (sync + bridges + `drone_dock_anim`)
- Native: `src/systems/vehicles/mod.rs` (registry + deploy arm), `src/ecs/components.rs` (`Vehicle`), `src/lib.rs` (registry load + channel + bridge + render pass), `src/gui/pages/inventory.rs` (Deploy button), `src/persistence.rs` + `src/save_load.rs` (`deployed_vehicles`)
- Data: `data/vehicles/kits.ron`, `data/items.csv` (kit rows), `data/recipes.csv` (kit recipes)

> The old "Navigation" and "Logistics" support-module entries were removed 2026-07-02:
> `src/systems/navigation/` and `src/systems/logistics/` were deleted in the v0.674.0
> dead-code sweep (zero callers). `src/systems/transportation.rs` (CargoVehicle routes)
> still exists unregistered and is the Stage 3 raw material.

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

### Conduit Flow Visualization (v0.622, refined v0.623)
Makes connections legible in a dark room. Every pipe is drawn as a STATIC line in its utility colour
(the `connection_color` legend: yellow=power, blue=water, red=hot water, cyan=air, violet=data, olive=fuel,
brown=nutrient, ...), faintly emissive so the run reads even in the dark. The SELECTED machine's
connections additionally get animated rainbow marker spheres travelling along their routed path in the
flow direction, so it's obvious which runs go to/from the thing you're inspecting (v0.623: selected-only,
where v0.622 animated every pipe -- the change both declutters the view and keeps the render loop cheap
no matter how many conduits the home has). Build-mode only, gated on the "Helper gizmos" toggle; the
markers are small (0.10 m) beads with moderate emissive so they read as spheres, not flat discs.
- Native: `src/lib.rs` (`connection_flow_paths` carries `(path, from_id, to_id)` from `rebuild_connection_objects`; the render loop animates only `from_id`/`to_id == construction_machine_selected` via the `flow_rgb_mats` rainbow), `src/machines.rs` (`connection_color` legend -- now also the pipe material so each run is its own utility colour)

### Rail cars -- the rail line comes alive (M2b, v0.637)
A small car now animates along each rail edge in build mode, so the rail graph (v0.635) reads as a living
transit line rather than static topology. Render-only (no state, no new field): mirrors the conduit flow-
marker pattern -- a box `RenderObject` slides from rail-node A to B by a `start_time`-driven phase, looping,
oriented along the track, one per edge (staggered). Confirms the rail graph at a glance; full passenger/
cargo routing + stops is later M2b.
- Native: `src/lib.rs` (`rail_car_mesh`/`rail_car_mat` + the per-edge animated car in the render loop)

### Viewport hide-per-type (declutter) (v0.636)
A per-type HIDE toggle in the object browser, beside the existing Lock toggle, for each type group
(Walls / Structures / Machines / Lights / Road nodes / Pipe nodes). Hiding a type skips its meshes +
gizmos in the 3D view and makes it un-pickable -- so a busy build can be decluttered to focus on one
system at a time. Machines hides the machine bodies + bounds cubes + port nodes; Pipe hides the routed
pipes + conduit-node rings; Road hides the road-graph gizmo; all six disable picking (like Lock). The
group header shows a `[hidden]` badge. Not serialized (a pure view toggle), mirroring lock-per-type.
- Native: `src/gui/mod.rs` (`construction_hidden_types`), `src/gui/pages/construction.rs` (Hide/Show toggle + `Act::ToggleHidden`), `src/lib.rs` (render + pick gating on the hidden set)

### Mothership rail graph (M2 transit) (v0.635)
The first transit network of superstructure M2 (`docs/design/mothership-superstructure.md`): a RAIL line as
a NODE + EDGE graph, generalising the v0.592 paired-train-platform link into a multi-stop line. Mirrors the
road graph exactly. `HomeStructure` gained `rail_nodes` + `rail_edges` (serde-default; `RailNode` = id +
(x,z) like `RoadNode`, `RailEdge` = from/to) with `add_rail_node` / `remove_rail_node` (prunes touching
edges) / `add_rail_edge` (refuses self-loop, unknown endpoint, or either-direction duplicate). A "Rail"
editor panel drops stops + wires edges (from/to pickers); a gizmo renders each stop as a pale-gold ring
and each edge as a straight track line. Cars + multi-stop routing are M2b.
- Native: `src/ship/home_structure.rs` (`RailNode`/`RailEdge` + the rail-graph methods + tests), `src/gui/pages/construction.rs` (`draw_rail_editor`), `src/lib.rs` (rail-graph gizmo render), `src/gui/mod.rs` (`rail_edge_from`/`rail_edge_to`)

### Zone interactivity -- click / drag / duplicate (v0.634)
Zones (the macro districts from M1) became first-class gizmo objects, like machines/nodes. Click a zone
box in the 3D view to SELECT it (ray-vs-AABB pick, runs last so it never steals a click from anything in
front); the selected box highlights bright white and its detail shows on the right -- type + purpose,
editable origin/size, and Duplicate / Remove / Deselect. Drag a selected zone on the floor (its centre
follows the cursor). `duplicate_zone` clones a zone with a fresh id nudged +2 m. No new persisted field
(`construction_zone_selected` is a pure selection); zones render live so edits show immediately.
- Native: `src/lib.rs` (`try_pick_zone` + `ObjectGrab::Zone` drag + selected-zone highlight), `src/ship/home_structure.rs` (`duplicate_zone`), `src/gui/pages/construction.rs` (`draw_zone_detail` + dispatch), `src/gui/mod.rs` (`construction_zone_selected`)

### Machine rotation (yaw) (v0.633)
A placed machine can now be ROTATED about its vertical axis -- so a box-shaped machine (a teleporter, a
server, a battery bank) can face a chosen direction instead of always being axis-aligned. `MachineInstance`
gained a `rotation` yaw field (degrees, serde-default 0, so every existing home + array cell is unchanged);
it flows through `PlacedMachine` into the renderer (`Quat::from_rotation_y`). The machine detail panel has
a Rotation control (a degree DragValue + a "+90" button); direct instances only (an array member has no
individual pose). Persists with Save.
- Native: `src/machines.rs` (`MachineInstance.rotation` + `PlacedMachine.rotation` + placements carry it), `src/lib.rs` (yaw threaded through `machine_objects` + applied in the machine RenderObject), `src/gui/pages/construction.rs` (Rotation control in `draw_machine_detail`)

### Conduit node TIERS + service-entrance grid-tie (v0.632)
The conduit pipe-graph gained its trunk-hierarchy controls (`conduits-node-graph.md` Stage 2 foundation +
`grid-hierarchy.md`). A selected conduit node's detail panel now sets its TIER -- Main (0) / Sub (1) /
Subsub (2) -- and the node renders sized by tier (a main line is a big ring, subs smaller) so the trunk-
and-branch reads at a glance. And a node can be flagged a GRID TIE / service entrance (where the home or
zone meets the external mothership/fleet grid), rendered as a distinct amber double ring. `ConduitNode`
gained a `grid_tie` bool (serde-default); both tier + grid_tie persist with the home.
- Native: `src/machines.rs` (`ConduitNode.grid_tie`), `src/gui/pages/construction.rs` (tier selector + grid-tie checkbox in `draw_conduit_node_detail`), `src/lib.rs` (tier-sized + grid-tie node rendering)

### Mothership ZONES, M1 -- the macro layout primitive (v0.631)
The first piece of the mothership superstructure (`docs/design/mothership-superstructure.md`): a ZONE is a
labelled, bounded VOLUME -- the macro analogue of a room. A `zone_types.ron` registry (infinite-of-X)
defines the districts the operator named -- residential, civic mall / meeting zone, industrial / factory,
hangar bay, mech bay, cargo, storage, agriculture, power, medical, transit hub -- each with a colour,
purpose, and default size. The Construction editor's "Zones" panel adds them (type picker -> drops one
centred in the footprint), lists them, and edits their origin/size; each renders as a colour-coded
wireframe box in build mode so the layout reads at a glance. Zones live on `HomeStructure` (serde-default,
so every existing home parses unchanged) and persist with Save. Data model only this stage -- transit
graphs (M2), the civic mall (M3), industrial+cargo (M4), hangar/mech (M5) build on it.
- Native: `src/ship/structure.rs` (`ZoneType` + `zone_types()`), `src/ship/home_structure.rs` (`Zone` + `zones` + `add_zone`/`remove_zone`), `data/blueprints/zone_types.ron`, `src/gui/pages/construction.rs` (`draw_zones_editor`), `src/lib.rs` (zone wireframe render)

### Grow-light power meter (v0.664, homestead gap #5)
The honest teaching artifact docs/design/self-sufficiency.md called for ("turns red the instant any LED
is added past the free pump headroom"). A `grow_light` machine (100 W LED panel, matching
`data/electrical.ron`) is placeable from the construction palette in both home designs; the moment one
is placed, the Buildability panel grows a dedicated "Grow-light power meter" row: GREEN while the lights'
draw (watts x a documented 14 h/day crop photoperiod, `GROW_LIGHT_DUTY_HOURS`) fits inside the home's
free headroom (generation minus all non-grow-light demand), AMBER when the home starts eating its battery
reserves every day, RED when the lights ALONE outdraw everything the home generates -- with the plain-
language note "this is why the garden uses the sun." Pure + world-free (`MachineHome::grow_light_report`),
so an AI can read it before committing a design. Neither seed design places a light (the reference gardens
are sun-lit; discovering the meter is the lesson). Same release also fixed `utility_meters` counting each
battery bank's bus terminal as 48 kWh/day of phantom demand.
- Native: `src/machines.rs` (`GrowLightReport`, `GrowLightVerdict`, `grow_light_report`, `GROW_LIGHT_DUTY_HOURS`), `src/gui/pages/construction.rs` (`draw_buildability` grow-light section), `data/machines/home.ron` + `home_solo.ron` (`grow_light` catalog entry)

### Usage meters + home self-sufficiency (grid S2, v0.630)
The Buildability panel now shows per-utility USAGE METERS: for power (kWh/day), water (L/day), and data
(Mbps) it reports the home's daily GENERATION vs DEMAND and a self-sufficiency fraction. Framed to TEACH,
non-punitively (grid-hierarchy.md): "makes 4.5, uses 2.4 kWh/day -- fully self-sufficient (+2.1 to share
with the community)" or "... 60% self-sufficient (X imported from the grid)". Pure + world-free
(`MachineHome::utility_meters`, computed from the placed machines' catalog roles), so it runs in the
editor and an AI can read it; no penalty for consuming, just understanding what you use + make.
- Native: `src/machines.rs` (`UtilityMeter` + `utility_meters` + `data_supply_mbps`), `src/gui/pages/construction.rs` (`draw_buildability` usage section)

### Viewport conduit-node placement + drag-a-port-to-a-node (build Phase 2, v0.629)
The pipe GRAPH is now built in the view, not via panel dropdowns. The Conduit-nodes panel has a "Place in
view" toggle: while active, clicking the floor drops a junction node there (a "main line" point); right-
click cancels. And a dragged machine PORT can now land on a conduit NODE (not just another machine) -- it
branches the machine onto the main line as a graph edge (`add_conduit_edge`, machine -> node), routed as a
real pipe. The drag preview snaps to a hovered node and rings it. Together this is the operator's "drag the
node to the main power line." Nodes already had drag + remove gizmos (v0.599); this adds in-view creation +
port-to-node wiring.
- Native: `src/lib.rs` (`construction_place_conduit_node` mode in the press chain; `port_drop_node_target`; the release-handler node branch; the preview snap-to-node), `src/gui/mod.rs` (the place flag), `src/gui/pages/construction.rs` ("Place in view" toggle)

### Pipes terminate at port nodes (grid S1, v0.628)
A wire/pipe now ROUTES TO the matching-utility port NODE (the v0.627 sphere+arrow gizmo above the machine)
instead of a generic floor anchor -- a water pipe plugs into the water node, the power wire into the power
node, so the two also leave the machine at different points (less overlap). Resolved from `port_pick` by
matching `port.utility.id()` to the connection's kind, falling back to the floor anchor if the machine
declares no port of that utility. First concrete step of the grid-hierarchy staging (`docs/design/grid-
hierarchy.md` S1: "cables/pipes go to the input/output nodes").
- Native: `src/lib.rs` (`rebuild_connection_objects` route building resolves each machine endpoint to its port-node position)

### Port NODE gizmo: central sphere + in/out arrows (v0.627)
The v0.625/626 in/out rings weren't legible at a glance, so a machine port is now drawn as a real NODE: a
solid ~10cm sphere (coloured by utility) with 4 CARDINAL ARROWS radiating from it. Arrows pointing IN
(heads near the sphere) mark an INPUT/draw port; pointing OUT mark an OUTPUT/supply port; both heads =
bidirectional (a battery terminal). Far clearer than the rings, and the sphere is a real object (toward
the operator's "eventually plug into a 3D model with real ports"). Shown for the selected machine; still
the drag-to-connect grab target. Captured alongside it: the GRID HIERARCHY vision (`docs/design/grid-
hierarchy.md`) -- home breakers -> residential substations -> generators -> the mothership/fleet grid,
with non-punitive consumption METERING to teach supply/demand + community self-sufficiency (the
civilization-on-a-mothership goal).
- Native: `src/lib.rs` (`port_node_mesh` central sphere in the all-objects pass; the 4-arrow in/out gizmo in the line overlay)

### Clickable pipes/wires + bracket dedup + bolder port handles (v0.626)
Pipes/wires became first-class clickable objects (they had no viewport interaction before). Click a
routed connection in the 3D view and it's selected (`try_pick_connection` ray-samples each route's
polyline): the pipe traces bright white and the right panel shows "Wire / pipe" with its utility,
endpoints, and a **Remove** button (`remove_connection_between`, either direction). Also: the conduit
support FITTINGS (ceiling hangers + wall gaskets) are now DEDUPED by position across all connections --
many pipes share a service-height run, so their brackets used to stack invisibly at the same spot,
wasting polygons; one bracket now serves every pipe through that point. And the v0.625 port handles got
a bolder double/triple ring so the drag-to-connect targets are obvious.
- Native: `src/lib.rs` (`try_pick_connection` + press-chain wiring + the selected-pipe highlight + the fitting dedup `HashMap` in `rebuild_connection_objects` + bigger port rings), `src/machines.rs` (`remove_connection_between`), `src/gui/mod.rs` (`construction_connection_selected`), `src/gui/pages/construction.rs` (`draw_connection_detail` + dispatch + `clear_sel`)

### Viewport drag-to-connect (machine ports) + array-member move (v0.625)
The panel dropdowns ("pick from, pick to, Connect") were a confusing way to wire machines, so wiring is
now a VIEWPORT gesture. Select a machine and its declared ports (`derive_ports()`) show as coloured
handles floating above it (amber power, blue water, violet data, ... -- the pipe legend; an OUT port
gets an outer "target" ring). Click-drag a handle and a rubber-band line follows the cursor: it turns
the utility colour over a machine that has a compatible (same-utility) port and RED over one that
doesn't. Release to create the connection (`add_connection`, oriented supply->demand). The dropdown
panel stays as a fallback. Also fixed: dragging a machine that is part of an ARRAY (e.g. a grain tray)
now WORKS -- the first drag explodes that array into individual instances (`detach_array_member`, ids +
positions preserved so nothing jumps), since array cells had no editable offset before.
- Native: `src/lib.rs` (`port_pick` built in `rebuild_machine_objects`; `port_gizmo_pos`; `try_pick_port` + `port_drop_target` + `machine_has_port`; the release-handler wiring; port-gizmo + rubber-band render; `ObjectGrab::Machine` detach-on-drag), `src/machines.rs` (`detach_array_member`), `src/gui/pages/construction.rs` (the "drag a port handle" tip)

### Build-editor click fix + lock/list footgun guards (v0.623 + v0.624)
**v0.624 (the real root cause):** entering build mode never rebuilt the machine PICK VOLUMES
(`machine_pick`), so machines were not clickable in the viewport until some *other* edit (e.g. nudging a
light -> `construction_structure_dirty` -> `rebuild_homestead` -> `rebuild_machine_objects`) repopulated
them -- the operator's "I have to drag a light first, then it updates and machines are clickable" repro.
Fix: build-mode entry now sets `construction_structure_dirty` so the pick volumes rebuild immediately, in
the editor's coordinate space. **v0.623 (partial mitigations, still useful):** a locked object type
silently blocks viewport picking, and a >12-row type group (e.g. 24 grow towers) collapses and egui
remembers it collapsed -- so the browser shows a loud "LOCKED (can't click in 3D): ..." banner with a
one-click "Unlock all", and the group holding the current selection is forced open.
- Native: `src/lib.rs` (force `construction_structure_dirty` on build-mode entry), `src/gui/pages/construction.rs` (locked-types banner + `Act::UnlockAll`; `CollapsingHeader::open(Some(true))` for the selected group)

### Capped-cylinder mesh fix (v0.624)
`Mesh::cylinder_capped` wound BOTH end caps inward (their front faces pointed into the cylinder), so under
the renderer's CCW-front + back-cull convention the caps were culled and tanks/cisterns rendered open-
topped. Flipped both caps to face outward (verified against the side-wall winding, which is the reference).
Fixes the missing tops on the water cistern + every other cylinder-shaped machine (and the podium).
- Native: `src/renderer/mesh.rs` (`cylinder_capped` cap winding)

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

### HomeStructure Model + the Ship Superstructure (v0.766 - v0.769)
The serialized home body: box dims + shell/roof material, interior walls, placed lights, placed
structures, a road graph (nodes + edges), and the player spawn point. Meshes regenerate on edit; rooms
detected by flood-fill. The ship-superstructure arc (docs/design/ship-superstructure.md, absorbs
decision-briefs Brief 1) generalized it into a whole vessel:
- **Zones (v0.766)**: the home body is ONE ZONE of `ShipStructure { zones }` - each zone carries
  id/label/purpose (residence | commons | bay | agriculture | corridor) + a world origin + the full
  home body unchanged. Editor Ship zone selector (Add zone, label/purpose/origin, confirmed delete);
  machines carry a `zone` id (default "home") and clamp into their zone's footprint; a lone legacy
  home_structure.ron adopts once as zone "home".
- **Corridors (v0.767)**: one data row (from_zone/door -> to_zone/door, width, glass_top) GENERATES the
  walled tube (floor, sides, glass-or-steel lid) and cuts walkable door apertures through both zones'
  perimeter shells in mesh AND collision. Straight/axis-aligned v1 with specific validation rejections;
  editor Corridors section; save prunes broken rows.
- **The Commons (v0.768, pure data)**: the seed ship's communal hall - a 34x55x8 glass-roofed commons
  zone with a 3x3 aeroponic tower grove (tank-watered, composter-fed like the home garden), apothecary
  towers + mushroom rack, three open-fronted Trading Post stalls, and a 10 m glass gallery from the
  home's east door. Zero engine code - the architecture's proof.
- **The Hull Wrap (v0.769)**: lofted plating swept through data-driven silhouette stations scaled to
  the cluster's real bounds (long-axis auto-detect), taper clamps that never slice a pressurized box,
  top cutouts over every glass roof + corridor lid, double-sided plating (look up through glass and see
  hull), greebles (engines/radiators/masts) as data rows. Regrows on any structure edit; H key /
  Settings "Show hull" toggle; purely visual (no exterior collision - bay doors/EVA are follow-ups).
- Native: `src/ship/ship_structure.rs` (zones + corridors, load/save/adopt, merged meshes),
  `src/ship/home_structure.rs` (the per-zone body incl. shell cuts), `src/ship/hull.rs` (profile +
  loft + greebles), `src/ship/wall_collision.rs` (`ship_wall_segments` + shell-cut gaps),
  `src/machines.rs` (per-zone clamping)
- Data: `data/blueprints/ship_structure.ron` (the seed ship: home + Commons + gallery),
  `data/blueprints/hull_profile.ron` (silhouette stations, margin, greebles - screenshot-tunable)

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

### Household size: Family vs Solo home design (v0.656)
A second, fully self-sufficient home layout for ONE person alongside the existing 3-person
`home.ron`: `data/machines/home_solo.ron` (see `docs/design/homestead-solo-design.md` for the
full sizing derivation -- 4 solar / 2 battery / 1 wind / 1 generator, 1 cistern / pump /
purifier / household tap, 1 air recycler, 2 composters, 9 nutrition towers + 1 apothecary + 8
potato beds + 3 oilseed beds + 2 grain trays + 2 mushroom racks + 1 aquaponic tank + 1 grain
field + 1 legume field + 1 silo + 1 irrigation -- the same 34x34 m garden room that covers
only ~half the calories for 3 people closes ~94% for 1). Selectable in Settings -> Data ->
"Home Design" (Family / Solo radio buttons); which file loads is resolved by
`machines::home_ron_path()` from the persisted `AppConfig.home_variant` at every real
`MachineHome::load` call site. Takes effect on next world load, not live mid-session.
- Native: `src/machines.rs` (`home_ron_path`), `src/config.rs` (`AppConfig::home_variant`, `default_home_variant`), `src/gui/mod.rs` (`SettingsState::home_variant`), `src/gui/pages/settings.rs` (`draw_data_content` "Home Design" section)
- Data: `data/machines/home_solo.ron`

### "What one home cannot close" panel (Home page)
The pedagogical payoff of the homestead design (operator: "people need to see the bare minimum
for 100% self-sufficiency so they understand the importance of all supporting civilizational
infrastructure"). Directly below the closed-loop summary, a visually distinct outlined panel
(warning stroke, muted treatment -- deliberately NOT the green closed-loop styling) lists the
five loops no single homestead can close: electronics/semiconductors, metal from raw ore,
medicine synthesis, equipment replacement, and raw chemistry inputs. Each is an expandable row:
collapsed shows title + a "traded" tag; expanded gives a plain-language body naming the game
recipe that abstracts the gap away (manufacture_cpu, smelt_steel, craft_antibiotics, ...) plus
a "provided by" trade line. Intro + footer carry the non-defeatist framing: these gaps ARE why
civilization exists. Data-driven (infinite-of-X): categories live in the RON, not code.
- Native: `src/gui/pages/homes.rs` (`CannotCloseEntry`, `CannotCloseData`, `load_cannot_close`, the panel in `draw_design`)
- Data: `data/self_sufficiency/cannot_close.ron` (distilled from `docs/design/homestead-solo-design.md` section 8)
- Tests: `gui::pages::homes::tests::{cannot_close_data_parses_and_is_complete, cannot_close_missing_file_is_empty}`

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

### Data Routing + Medium Picker (v0.621, telecom Stage 2)
The telecom utility's primary function: wire devices to the internet via a chosen medium, validated like
a power run. `Port` gained an `mbps` field; machines declare Data IN/OUT ports (a `home_server` demands
100 Mbps, a `network_uplink` supplies it). A "data" connection carries a medium `spec`; the editor's
utility-lines panel shows a per-data-run medium picker (auto / Cat6 / fibre / WiFi). A new "Data links"
buildability check sizes each data run (bandwidth + range via `check_data_link`) and CAUTIONS when the
medium is wireless (its RF can harm a grow -- the v0.620 consequence). The seed home wires its uplink to
its server over Cat6 (clean); swap it to WiFi in the editor to see the RF warning fire.
- Native: `src/utilities.rs` (`Port.mbps`, `Port::data_in`/`data_out`), `src/machines.rs` (`data_demand_mbps`, the "Data links" check), `src/gui/pages/construction.rs` (the "data" kind + the data-medium picker)
- Data: `data/machines/home.ron` (`network_uplink`, `home_server`, a `data` connection on `eth_cat6`)

### Telecom RF -> Plant Harm (v0.620)
The first telecom consequence, the operator's headline ("the user doesn't want a WiFi router because the
frequencies harm a plant they're growing"). A machine with `rf_emission > 0` (a `wifi_router`) spawns an
`RfEmitter`; while powered it adds to the home RF level; the FarmingSystem drains crop health by that
level (outpacing recovery at one router's worth). Run a wired link (Cat6/fibre, zero RF) -- or remove the
router -- to keep a clean grow. The `wifi_router` is placeable but NOT in the seed home, so the reference
grow stays safe until you choose to add one.
- Native: `src/ecs/components.rs` (`RfEmitter`), `src/machines.rs` (`MachineDef.rf_emission`), `src/lib.rs` (spawns `RfEmitter`), `src/systems/farming/mod.rs` (the home-RF sum + the crop RF-stress drain)
- Data: `data/machines/home.ron` (`wifi_router`: a powered wireless device, `rf_emission: 0.6`)

### Data / Telecom Media, Stage 1 (v0.619)
The internet/telecom utility: teach real telecommunications. Data is `Utility::Data` in the same
`conduits.ron` registry, with media that have real tradeoffs -- bandwidth, range, latency, cost, and RF
emission. Stage 1 ships the data model + link physics + 3 core media; the consequences (RF harms a
sensitive plant; emissions become detection signatures) + the full media catalog are later stages.
- Native: `src/utilities.rs` (`ConduitType` data fields `bandwidth_mbps`/`range_m`/`latency_ms`/`wireless`/`rf_emission`, `ConductorMaterial::{Glass,Radio}`, `check_data_link`/`cheapest_data_link_for`/`data_media`)
- Data: `data/utilities/conduits.ron` (`eth_cat6` quiet wired workhorse, `fiber_om4` high-bandwidth no-RF, `wifi_6` convenient but RF-loud)
- Design: `docs/design/telecom.md` (the 21-media catalog + the emissions-as-signature design + the staged plan)

### Conduit / Cable Data Model + Physics
A closed `Utility` enum (Electricity, Water, HotWater, Air, Data, Fuel, Nutrient, Waste);
`Port { utility, dir: In/Out/Bidirectional, label, watts, flow_lpm, anchor }`; a cable registry with real
NEC-ish copper specs (AWG, ampacity, voltage rating, ohm/m, cost/m, grade). `check_cable` computes amps +
round-trip voltage drop into Pass/Warn/Fail; `cheapest_cable_for` is the auto-picker; `awg_to_mm2` for
display.
- Native: `src/utilities.rs` (`Utility`, `Port`, `ConduitType`, `ConductorMaterial`, `Grade`, `check_cable`, `cheapest_cable_for`, `conduit_types`)
- Data: `data/utilities/conduits.ron` (copper 14/12/10 AWG home, 6 AWG industrial shielded, the `sc_room_temp` superconductor upgrade target, two water pipes)
- Design: `docs/design/utility-wiring.md`

### Superconductor Bulk-Upgrade (v0.616)
The late-game wiring payoff: an "Upgrade all power runs to superconductor" button in the utility-lines
editor sets every power connection's `spec` to the room-temperature superconductor (near-zero loss, huge
ampacity, so the Conduits check goes all-green); "Reset to auto" reverts to cheapest-copper auto-sizing.
The action ships now; a future quest gates earning it.
- Native: `src/gui/pages/construction.rs` (the bulk-spec buttons in `draw_machines_and_connections`), `data/utilities/conduits.ron` (`sc_room_temp`)

### Per-Connection Cable Picker (v0.615)
The utility-lines editor gives every POWER run a cable dropdown: "auto (cheapest copper)" or a pinned
type from the registry (copper 14/12/10/6 AWG ... the room-temp superconductor). Picking sets the
connection's `spec`; the Conduits buildability check then validates it (over-ampacity or >5% voltage drop
-> warn/fail), so the whole cable-physics system is finally interactive in the editor.
- Native: `src/gui/pages/construction.rs` (`draw_machines_and_connections`, the cable ComboBox writing `MachineConnection.spec`), choices from `src/utilities.rs::conduit_types`

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

### Live Air / Atmosphere Sim + power -> air -> Vitals (v0.617 - v0.618)
The 3rd life-support utility, with its consequence chain. The AtmosphereSystem (now REGISTERED) ticks the
home's sealed air space (a HomeAir + HomeMachine tagged `EnclosedSpace`), publishing a live `AirStatus`
(O2/CO2/pressure/temp/breathable) to a "Live air" Home-page card beside power + water. **Stage 2 (v0.618):**
occupancy (a ~3-person household) steadily drains O2 + raises CO2; a powered `air_recycler` (an `AirScrubber`
derived from an Air-OUT port, gated on its PowerConsumer) offsets it. Cut the grid -> the recycler sheds ->
O2 falls -> unbreathable -> the inside-homestead `EnvironmentContext.oxygenated` flips off -> the FoodSystem
drains the player's `Vitals.oxygen` -> hypoxia. So **power -> air -> Vitals** runs end to end.
- Native: `src/systems/atmosphere.rs` (`AtmosphereSystem`, `AirStatus`, `HomeAir`, `AirScrubber`, the occupancy/scrubber dynamics), `src/lib.rs` (`spawn_home_air_space`, the AirScrubber spawn from an Air-OUT port, `EnvironmentContext.oxygenated = breathable`), `src/gui/pages/homes.rs` (Live air card)
- Data: `data/machines/home.ron` (`air_recycler`: a critical-priority power consumer + an Air-OUT port, wired to the battery bus)

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
24 TOML schema files documenting all data formats for modding and validation. (Count corrected 2026-07-01: the old "22" omitted room; chore added with the crew chore AI.)
- Data: `schemas/*.toml` (item, material, component, creature, spell, structure, status_effect, enchantment, recipe, quest, biome, celestial_body, faction, npc, chore, room, skill, sound, vehicle, weather, economy, offline_agent, equipment_slot, container)

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
`data/help/topics.json` is shared between web and native. Both UIs load it on startup and show the same help content. Help buttons (`?`) anywhere in the UI open a themed modal with the topic body. The native `help_button`/`draw` plumbing was fully wired at the app level (registry loads at startup, modal draws in the render loop) since it shipped, but had ZERO real call sites in any native page until v0.658 wired 3 into the Studio page (Scenes, Resolution/Bitrate/FPS, Chat Overlay) -- the first native page to actually use it.
- Data: `data/help/topics.json`
- Native: `src/gui/widgets/help_modal.rs` (help_button + draw fn + HelpRegistry loader), `src/gui/pages/studio.rs` (first real adoption)
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

---

## Unified Chat + Co-presence + Dev Tools (v0.771 - v0.779)

### In-World Chat (feed + interactive panel)
The SAME relay chat visible in the 3D world: a paint-only bottom-left feed of the
active PUBLIC channel (DM/group channels fall back to #general so private text never
paints on the world overlay / a stream), and an interactive panel opened with Enter
(frees the cursor, disables look/move, swallows gameplay hotkeys while typing; Esc or
click-away closes, typed text survives a dismissed panel and an aborted send). Channel
switcher mirrors the Chat page semantics (clear + refetch + unread clear).
- Native: `src/gui/pages/hud.rs` (feed), `src/gui/pages/chat.rs` (`draw_ingame_chat`,
  `channel_display_label`), `src/lib.rs` (modal input guards keyed on
  `GuiState::in_world_modal_open`, Enter handler)

### Shared-World Co-presence (visible)
Auto-joins the relay's shared game world whenever in-world + connected (no launcher
step); HUD top-left shows "Shared world - <host>" + a live roster of other players
(entity-count, duplicate names NOT collapsed). The session survives menu round-trips
(net_sync keeps applying updates; the relay treats a duplicate game_join as a RESYNC
and re-sends the welcome + snapshot). Relay reaps ghost player entities on restart
(persisting their progress) so counts stay honest and rejoins work.
- Native: `src/lib.rs` (multiplayer block, roster mirror), `src/gui/pages/hud.rs`
- Server: `src/relay/handlers/msg_handlers.rs` (`handle_game_join` resync + 48-char
  name clamp + name stamp), `src/relay/handlers/game_state.rs` (ghost reap in
  `restore_from_db`)

### Server Join Surface (launcher + public counts)
The character-select launcher lists the server you are connected to (virtual row,
deduped against bookmarks; a bookmark of the live connection counts as connected) with
live `/api/server-info` details and a working "Enter World". `/api/server-info`
exposes `game_players` (avatars in-world, distinct from chat `users_online`) - the
public mirror of the in-world roster, shown as "In world" in the launcher pane.
- Native: `src/gui/pages/showroom.rs`
- Server: `src/relay/api.rs` (`get_server_info`)

### Dev Page: Spawn Any Creature + Walk-Up Editor
Platform > Dev (cheats-gated, like every dev affordance): searchable list of all 92
creatures.csv species, Spawn drops one ~2 m ahead (passive species keep the anchored
livestock graze - no AIBehavior; hunt species spawn as predators), Despawn-all + live
count. Walk-up editor: look at any creature, press G - rename, health (0 = kill via
the real Dead marker), Hostile toggle (maps to "predator", the behavior that actually
hunts the player), size, tint, despawn; the AI is only touched if the toggle is
actually flipped (opening the editor never rewrites predator/guard behaviors).
- Native: `src/gui/pages/dev.rs`, `src/lib.rs` (spawn/editor consumers),
  `src/systems/livestock.rs` (`spawn_creature_at`)
