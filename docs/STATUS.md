# HumanityOS: Feature Status

> **Last updated:** 2026-06-16 | **Version:** v0.469.3 (construction editor arc v0.463-469: 3D room grab, 3-column layout, openings as placed objects + garden-wall regen fix; website parity v0.469.1-3: landing = app Mission Dashboard, header = app nav, SW network-first; inventory/garden UI overhaul on universal expandable_row v0.400-414; homes-as-profiles + aeroponic towers + seed economy v0.379-399)
>
> **⚠️ KNOWN GAPS (the strategic backlog lives in [ROADMAP.md](ROADMAP.md)):** multiplayer
> co-presence is NOT wired on the client (the relay game world is live, but `NetSyncSystem` is never
> instantiated -- see [docs/design/first-playable.md](design/first-playable.md) +
> [characters-and-servers.md](design/characters-and-servers.md)). RESOLVED 2026-06-16: desktop
> auto-update (v0.470.0 now signed) and the web a11y/i18n/glossary modules (now wired into every
> page, v0.471.0).
>
> This is a **feature inventory**, what is built, partial, or planned.
> Update this file every time features are added or status changes.
>
> **⚠️ This file is NOT the backlog.** The authoritative, strict-ranked "what gets
> worked on next" lives in **`docs/PRIORITIES.md`** (TIER 0 → TIER 4). If STATUS.md
> and PRIORITIES.md ever disagree, **PRIORITIES.md wins**, it is kept current every
> session; this file is a slower-moving inventory. See the "What's NOT done" section
> at the bottom for the gap list, which is a summary of PRIORITIES, not a replacement.

**Legend:** ✅ Built/working | ⚠️ Partial/needs work | ❌ Not yet built | 🔜 Next priority

> **Truthfulness rule:** A feature is ✅ only when its behaviour runs end-to-end.
> "Module exists and ticks" without behaviour is ⚠️, not ✅. A ✅ that is not actually
> verified end-to-end should be downgraded to ⚠️ (partial / unverified) rather than
> left as a false "done", lying to STATUS.md means future agents skip work that
> wasn't done. **Many ✅ rows below predate v0.132 and have not been re-verified
> against the current binary; treat older ✅ rows as "claimed built," not "audited."**

---

## Architecture & crypto reality check (read before trusting old rows)

Several rows further down were written before major migrations and are now
**factually wrong about HOW a feature works** (the feature exists, but the
described mechanism is stale). The current ground truth:

- **Single crate, not a workspace.** Everything compiles from `src/` at the repo
  root into one `HumanityOS.exe`. The server/relay is `src/relay/`. Feature flags
  (`native`, `relay`, `wasm`) select what's built. The pre-v0.90 `server/`,
  `native/`, and `crates/` directories **no longer exist**. (See `CLAUDE.md`.)
- **Chat identity is Dilithium3 / ML-DSA-65, not Ed25519.** Since the post-quantum
  cutover (v0.262.x–v0.264.x), the chat identity = a Dilithium3 keypair derived
  from the BIP39 seed. Ed25519's **only** remaining roles are the seed source and
  the Solana wallet address. Old "Ed25519 identity" rows describe the pre-PQ world.
- **DM E2EE is pure Kyber768 / ML-KEM-768 → BLAKE3-KDF → AES-256-GCM, not ECDH.**
  The ECDH P-256 DM path was **deleted** (web v0.263.4, native v0.264.0). Any row
  or note below mentioning "ECDH P-256" for DMs is stale.
- **Native vault PBKDF2 is 600,000 iterations** (v0.277.0), matching web, not the
  100k "pending upgrade" some notes still claim.
- **Full crypto inventory:** `CLAUDE.md` → "Cryptography (canonical)" table is the
  authority. Read it before quoting any algorithm. Activation note: the PQ stack is
  shipped AND the attended Inc6 wipe ran (confirmed 2026-05-20), so PQ chat/DM is
  live, not merely "awaiting activation."

---

## Civilization Trust Layer (v0.98.0 – v0.109.0)

Strategic build executed across 7 phases. All server-side, all PQ-native, all
purely additive (existing chat untouched). See
`~/.claude/plans/okay-claude-here-s-a-floating-wozniak.md` for the full plan.

| Layer | Status | Release | Implementation |
|-------|--------|---------|----------------|
| **Post-quantum crypto core** | ✅ | v0.98.0 | ML-DSA-65 (Dilithium3) signing, ML-KEM-768 (Kyber768) KEM, Argon2id KDF, BLAKE3 hashing. `src/relay/core/pq_crypto.rs`, `kdf.rs`. 14 PQ + 7 KDF tests. |
| **Signed-object substrate** | ✅ | v0.98.0 | Generic `signed_objects` table backs every higher-level domain. Auto-indexes VCs, governance, AI status, recovery, disputes on insert. `src/relay/storage/signed_objects.rs`. `POST/GET/LIST/COUNT /api/v2/objects`. |
| **DID resolver** | ✅ | v0.100.0 | `did:hum:<base58(BLAKE3(pubkey)[..16])>`. Resolves to current Dilithium3 pubkey + first/last seen + activity count. `src/relay/core/did.rs`, `src/relay/storage/dids.rs`. `GET /api/v2/did/{did}`. |
| **Verifiable Credentials** | ✅ | v0.101.0 | 12 indexed schema types (vouch, verified_human, member, role, account_age, skill_endorsement, graduation, employment, controlled_by, juror, trust_score, ai_consent, attested_session, liveness). Issuer-auth-checked revocation, subject-auth-checked withdrawal. `src/relay/storage/credentials.rs`. `GET /api/v2/credentials`. |
| **Multi-layer trust score** | ✅ | v0.102.0 | 0..1 normalized total + 6 sub-scores (vcs, vouching_graph, activity_diversity, age, economic_stake, reputation). 5-min cache. Inputs always exposed (Accord transparency). Tunable weights at `data/identity/trust_weights.ron`. `src/relay/storage/trust_score.rs`. `GET /api/v2/trust/{did}`. |
| **Governance** | ✅ | v0.103.0 | 9 proposal types in `data/governance/proposal_types.ron` (5 local, 4 civilization scope). Vote weight = trust score capped at 0.95 (Accord power-asymmetry mitigation). One vote per voter per proposal. AI agents excluded from voting. Deterministic tally. `src/relay/storage/governance.rs`. `GET /api/v2/proposals`. |
| **AI-as-citizen** | ✅ | v0.104.0 | Mandatory `subject_class_v1` (`human`/`ai_agent`/`institution`) and `controlled_by_v1` operator binding. AI silently excluded from governance voting per Accord. Same trust curve as humans (no flag discount). `src/relay/storage/ai_status.rs`. `GET /api/v2/ai-status/{did}`. |
| **Social key recovery** | ✅ | v0.105.0 + v0.109.0 | Shamir share storage (PR 1) + recovery_request_v1 / recovery_approval_v1 with guardian-auth flow (PR 2). Server stores opaque ciphertext only; reassembly is client-side. Auto-flips request status to "ready" when threshold met. `src/relay/storage/recovery.rs`. `GET /api/v2/recovery/setup/{holder_did}`. |
| **Federation v2** | ✅ | v0.107.0 + v0.108.0 | `SignedObjectGossip` RelayMessage federates ANY post-quantum object across servers (PR 1). Per-observer per-issuer continuous trust + dispute_v1 auto-discount + multi-hop gossip with cycle-breaking via dedup (PR 2). `src/relay/handlers/federation.rs`, `src/relay/storage/issuer_trust.rs`. |
| **Schema registry** | ✅ | v0.98.0+ | `data/identity/schemas.ron`, hot-reloadable, infinite-of-X compliant. 4 active substrate schemas, 22 reserved per-phase schemas. |
| **Documentation pages** | ✅ | v0.106.0 | `/`, `/onboarding`, `/download` web pages + native `main_menu.rs`/`onboarding.rs` updated to describe new architecture (PQ + DIDs + VCs + trust + governance + AI + recovery). |
| **Quest chains** | ✅ | v0.110.0 | `data/onboarding/quests.json` schema_version 2: 6 chains covering identity & trust, civic participation, interaction preferences. Teach the new layers by doing. |

### Core counts after Phase 0–5 + 8 + 3 + 4:

- 144 unit tests passing
- ~5000 LOC added across crypto/identity/credentials/trust/governance/recovery/federation
- 14 new REST endpoints under `/api/v2/`
- 0 changes to existing chat, all additive
- 13 production releases shipped this session

### Still in flight:

- ⚠️ Phase 6c liveness: schemas wired, no signaling integration yet
- ⚠️ Phase 6a Solana RPC: read-only balance proxy ships (v0.110.0); no transaction signing in relay (intentional, client-side per BIP39 path)
- ⚠️ Phase 6b STARK selective disclosure: scaffold + Merkle disclosure verifier wired (v0.111.0–v0.112.0); full STARK verifier circuit deferred
- ⚠️ Phase 7a Litestream replication: ops doc shipped (v0.110.0), VPS deployment is operator action
- ⚠️ Phase 7b LoRa mesh: serial driver landed (v0.112.0), real radio integration deferred
- ✅ PQ-native chat: **SHIPPED** in the v0.262.x–v0.264.x cutover (this "still pending" note is obsolete). Chat identity = Dilithium3, DM = pure Kyber768, ECDH removed, KAT-locked web↔native↔relay. See the crypto reality-check at the top + `CLAUDE.md`.

---

## Releases v0.110.0 – v0.121.3

| Version | Theme | Highlights |
|---------|-------|------------|
| v0.110.0 | Solana RPC + Liveness + Litestream + FEATURES sync | Balance proxy, liveness schema docs, Litestream ops guide |
| v0.111.0 | STARK ZK scaffold + LoRa stub + PQ JS bridge | Pieces in place for selective disclosure + mesh radios |
| v0.112.0 | Final TODO closeout | Merkle disclosure verifier, LoRa serial driver, JS PQ crypto |
| v0.113.0 | Universal Button widget | Single source of truth for every button (`src/gui/widgets/button.rs`) |
| v0.114.0 | Tab/nav buttons converge on Button + theme color editor | In-app theme editing |
| v0.115.0 | Page audit | Filled governance/identity/recovery gap, modal cleanup |
| v0.116.0 | Storage architecture doc + content sweep + multi-AI coordination + items/toxicology DB | Coordination layer for multiple AI agents working in parallel |
| v0.116.1 | Items refactor + 1B-item performance section | Items hold pure ingredient lists; toxicology derived from ingredients sidecar |
| v0.117.0 | Commit `src/relay/storage/issuer_trust.rs` (was untracked, breaking CI) | Fix CI breakage from un-staged file |
| v0.117.1 | Cargo.toml drift fix | Local 0.116.5 vs git tag v0.117.0 |
| v0.118.0 | Agent dashboard (`/agents`) + AI usage tracker (`/ai-usage`) + orchestrator continuity | Multi-AI coordination tooling lands |
| v0.119.0 | Server-signed announcements channel | Agent overrides + external triggers |
| v0.120.0 | Wire 5 missing pages into native escape-menu nav | Reduce dual-UI parity gap |
| v0.121.0 | 21/21 scopes audited + native onboarding styling fix + native AI Usage form | Full agent-coordination audit pass |
| v0.121.2 | README rewrite | Current architecture, accessible language |
| v0.121.3 | Worker agent expands items-game scope | 500-item milestone hit |
| v0.131.0 | AI Perception API, headless gameplay for AI agents | `game_perceive`/`game_interact`/`game_query_inventory`/`game_query_entity` WebSocket messages; ship layout loading; spatial queries. AI experiences the game world as JSON instead of pixels. (`docs/ai/onboarding.md` §"Playing the Game"; `docs/design/ai_interface.md` §5 Game Participation) |
| v0.131.1 | Version sync after `just build-game` | No code changes, version-string-only patch |
| v0.132.0 | Perception API bug fix, typed RON deserialization | Caught via new unit tests: `room_type: bridge` (unquoted RON enum identifier) was parsing as `Value::Unit` not String, so all room equipment lists came up empty. Now uses typed `ShipDef` from `src/ship/layout.rs`. 6 new GameWorld tests prevent regression. |

See `git log --oneline` for the per-commit detail; the rows above call out
the user-facing theme.

---

## Architecture (v0.90.0)

| Feature | Status | Details |
|---------|--------|---------|
| Unified binary | ✅ | Single crate at `src/`; former `server/`/`native/`/`crates/` folded in (v0.90.0). Server relay = `src/relay/`. One binary: `HumanityOS.exe` |
| Headless mode | ✅ | `HumanityOS --headless` for server-only (VPS, Raspberry Pi) |
| Default build | ✅ | Includes relay + renderer + game. Feature flags: `native`, `relay`, `wasm` |
| Zero sub-crates | ✅ | 17 unused crates deleted, zero workspace complexity |
| Config location | ✅ | `%APPDATA%/HumanityOS/` (stable across exe versions) |
| Versioned exe archives | ✅ | `v{version}_HumanityOS.exe` in repo root, auto-purge keeps last 5 |
| Auto version bump | ✅ | `just build-game` bumps version automatically |
| Auto git tagging | ✅ | `just _commit` auto-tags releases (v0.90.5) |
| GitHub auto-publish | ✅ | Releases auto-publish on tag push via CI (v0.90.5) |
| Cross-platform build | ✅ | Replaced dirs:: crate with std::env::var for portability (v0.90.6) |
| Worktree hygiene | ✅ | `just clean-worktrees` prevents AI context rot (v0.90.2) |
| Launcher scripts | ✅ | Taskbar pinning support |

---

## Communication

Everything in this section is **built and working**.

| Feature | Status | Details |
|---------|--------|---------|
| WebSocket relay | ✅ | relay.rs ~5800 LOC, message routing, Fibonacci rate limiting, Dilithium3 identify (proof-of-possession nonce challenge, v0.274.0) |
| Channels | ✅ | Create, switch, ordering, read-only, invite codes, auto-lockdown |
| Direct messages | ✅ | E2E encrypted, **pure Kyber768/ML-KEM-768 → BLAKE3-KDF → AES-256-GCM** dual-seal (web v0.263.0, native v0.264.0; ECDH P-256 deleted), @mentions, notifications. KAT-locked web↔native |
| Threaded replies | ✅ | Thread view panel, reply indicators, reply count tracking |
| Message editing | ✅ | Server-side edit history, client UI |
| Pins | ✅ | Server-side + client UI, per-channel |
| Emoji reactions | ✅ | Persistent storage, Twemoji rendering |
| Markdown rendering | ✅ | Collapsible quotes, code blocks |
| Message search | ✅ | FTS5 full-text search with LIKE fallback |
| WebRTC voice calls | ✅ | 1-on-1 audio, group voice rooms, TURN server |
| Video calls | ✅ | Camera selection, PiP overlay, gallery view |
| Screen sharing | ✅ | Concurrent camera+screen layers, draggable PiP |
| Streaming system | ✅ | Streamer dashboard, WebRTC relay, scenes/presets |
| Voice join/leave sounds | ✅ | Audio cues when users enter/leave voice channels (v0.35.1) |
| Role badges in sidebar | ✅ | Visual role indicators next to usernames in member lists (v0.35.1) |

---

## Identity & Security

| Feature | Status | Details |
|---------|--------|---------|
| Dilithium3 chat identity | ✅ | ML-DSA-65 keypair derived from BIP39 seed = the chat identity (PQ cutover v0.262.x–v0.264.x). Sign/verify on chat messages + relay-auth endpoints. Ed25519 retained only as seed source + Solana wallet |
| Key rotation | ⚠️ | Dual-signed-certificate design existed for Ed25519; the relay `key_rotation` route/handler was **removed** in the PQ trim (v0.265.0). Re-verify before claiming an end-to-end rotation flow |
| BIP39 seed phrase | ✅ | 24-word backup & restore (single seed derives Ed25519 + Dilithium3 + Kyber768) |
| Encrypted backup (web) | ✅ | AES-256-GCM + PBKDF2-SHA256, 600k iterations |
| Encrypted vault (native) | ✅ | AES-256-GCM + PBKDF2-SHA256, **600k iterations** (v0.277.0, was 100k); legacy vaults auto-re-encrypt on next unlock |
| Auto-unlock (native) | ✅ | 3 modes: AlwaysPrompt / OS-keychain / KeychainPin (v0.278.0) |
| Device management | ✅ | List, label, revoke devices; QR code linking |
| Vault sync | ✅ | Encrypted cross-device sync, auto-lock, timestamp freshness |
| Seed phrase recovery | ✅ | "Recover from Seed Phrase" button on login screen (v0.25.0) |
| Security hardening | ✅ | Error boundary, pagination guards, env validation, automated DB backups (v0.35.0) |

---

## Push Notifications

| Feature | Status | Details |
|---------|--------|---------|
| VAPID keys | ✅ | Server-side key pair configured |
| Service worker push handler | ✅ | Receives and displays push events |
| Subscription management | ✅ | Save, get, remove subscriptions |
| DM and @mention triggers | ✅ | Offline-only delivery to prevent duplicates |
| Stale subscription cleanup | ✅ | Auto-removes expired/invalid subscriptions |
| Notification preferences | ✅ | Per-user DM/mention/task/DND settings, server-side storage (v0.31.0) |
| Notification actions | ✅ | Reply and mark-read buttons on push notifications (v0.31.0) |

---

## Task Board

| Feature | Status | Details |
|---------|--------|---------|
| Kanban board | ✅ | Create, edit, move, delete tasks |
| Real-time updates | ✅ | WebSocket sync across clients |
| Task comments | ✅ | REST API + WebSocket + detail drawer UI |
| REST API endpoints | ✅ | GET/POST /api/tasks, PATCH/DELETE /api/tasks/{id}, comments |
| Fibonacci scope system | ✅ | Civilization-scale task scoping |
| Projects system | ✅ | Project CRUD, color/icon picker, task filtering by project (v0.25.0) |

---

## Marketplace

| Feature | Status | Details |
|---------|--------|---------|
| CRUD operations | ✅ | Create, read, update, delete listings |
| WebSocket real-time sync | ✅ | Live updates across clients |
| REST API | ✅ | GET/POST /api/listings, FTS5 search via ?q= parameter |
| Role-based access | ✅ | Verified+ users can create listings |
| Category filtering | ✅ | Search, sort, filter by category |
| Create/edit/delete modals | ✅ | Full UI for listing management |
| Image support | ✅ | Upload (drag-and-drop), carousel gallery, thumbnails (v0.25.0) |
| Full-text search | ✅ | FTS5 MATCH + LIKE fallback (v0.25.0) |
| Seller profiles | ✅ | Clickable seller names, profile modal with listings and ratings (v0.25.0) |
| Ratings and reviews | ✅ | Star ratings, review form, sort options, aggregate display (v0.25.0) |
| Buyer-seller messaging | ✅ | listing_messages table, WebSocket send/history (v0.31.0) |
| P2P trading with escrow | ✅ | Peer-to-peer trade system with escrow protection (v0.40.0) |

---

## Wallet & Funding

| Feature | Status | Details |
|---------|--------|---------|
| Solana wallet | ✅ | Balance, send, receive -- Ed25519 identity IS the Solana address (v0.25.0) |
| Token swaps | ✅ | Jupiter API integration, slippage settings, price impact warnings (v0.25.0) |
| Staking | ✅ | Validator picker, stake/unstake flows (v0.25.0) |
| NFT support | ✅ | Detection, Metaplex metadata, grid display with detail modals (v0.25.0) |
| Donation page | ✅ | Progress bar, dynamic multi-crypto address cards, FAQ (v0.25.0, enhanced v0.73.0) |
| Server funding config | ✅ | data/server-config.json with flexible addresses array supporting unlimited networks (v0.25.0, enhanced v0.73.0) |
| Wallet settings | ✅ | Network selection, custom RPC URL, nav balance toggle (v0.25.0) |
| Wallet on profile | ✅ | Solana address and balance shown on profile cards (v0.25.0) |
| Wallet guide | ✅ | 9-section beginner guide: wallet basics, send, receive, buy, sell, swap, backup, glossary (v0.73.0) |
| Admin donation addresses | ✅ | Add/edit/remove/reorder donation addresses in native settings, dynamic rendering in web+native (v0.73.0) |

---

## Game Engine

| Feature | Status | Details |
|---------|--------|---------|
| Rust/wgpu renderer | ✅ | PBR-lite pipeline, depth buffer, mesh/material system |
| Dual-target compilation | ✅ | Native (winit) + WASM (WebGPU) from same codebase (v0.25.0) |
| Three-mode camera | ✅ | First-person, third-person, orbit with smooth transitions (v0.26.0) |
| Platform abstraction | ✅ | platform.rs: logging, timing, asset loading across native/WASM |
| WGSL shaders | ✅ | 41 shaders (planets, PBR, procedural materials, particles, bloom) |
| Game data files | ✅ | 108 data files, ~3000+ entries across CSV/TOML/RON/JSON |
| Gardening activity | ✅ | Playable 2D canvas farming (6 crops, save/load) |
| Data loading (AssetManager) | ✅ | load_csv/toml/ron/json, FileWatcher, HotReloadCoordinator (v0.28.0) |
| ECS system runner | ✅ | System trait, SystemRunner, 20 game components, per-frame tick (v0.29.0) |
| Icosphere planet terrain | ✅ | Icosahedron subdivision, PlanetDef (RON), LOD levels, PlanetRenderer (v0.30.0) |
| Voxel asteroid system | ✅ | Sparse octree, greedy meshing, ore veins (C/S/M-type), mining (v0.31.0) |
| Rapier3d physics | ✅ | Rigid bodies, colliders, raycasting, step simulation (v0.31.0) |
| Player controller | ✅ | Registered + ticks; WASD look/move works (camera-driven). NOTE: the ECS physics path (gravity/jump/ground via a `PhysicsBody`) is inert, `physics_world` is never inserted into the DataStore and the player entity has no `PhysicsBody`. |
| Interaction system | ✅ | Raycast from camera, find interactables within range (v0.31.0) |
| Day/night cycle | ✅ | GameTime with seasons, sun direction/color (v0.31.0) |
| Inventory system | ✅ | ItemStack slots, add/remove/transfer (v0.31.0) |
| Crafting system | ✅ | Recipe matching from recipes.csv (v0.31.0) |
| Farming / gardening loop | ✅ | `FarmingSystem` registered + ticking: growth (game_time + water/health) **plus the full loop (v0.331.0)**, Plant a seed (Plant button → spawn CropInstance), Water, Harvest a mature crop → produce into inventory → despawn. Garden panel on the inventory page + a "Dev: grow all" affordance. Proven by `farming::gardening_tests`. **#4b** (tracked in gameplay-loops.md): data-driven `harvest_item` column (124/129 plants have no produce item yet), soil/irrigation entities, 3D crop placement/visuals. |
| InputState | ✅ | Cross-system input sharing (v0.31.0) |
| Ship interior system | ✅ | ShipDef/DeckDef/RoomDef from RON, room mesh generation, BFS pathfinding (v0.33.0) |
| AI behavior system | ⚠️ | Native `AISystem` (state machines) implemented but **NOT registered**, never ticks in the native runtime. (The relay drives ambient NPC wander separately, server-side.) See `tests/engine_wiring_lint.rs::DEFERRED_SYSTEMS`. |
| Vehicle/mech system | ⚠️ | `VehicleSystem` implemented but **NOT registered**, never ticks. See `tests/engine_wiring_lint.rs::DEFERRED_SYSTEMS`. |
| Ecology simulation | ⚠️ | `EcologySystem` implemented but **NOT registered**, never ticks. See `tests/engine_wiring_lint.rs::DEFERRED_SYSTEMS`. |
| Quest system | ✅ | **Registered + ticking since v0.342.0** (this row predated that; see the detailed "Quests" row further down). The relay still runs its separate authoritative chain for MMO, native-vs-relay reconciliation is the #8c-tail. |
| GLTF model loading | ✅ | Load .glb models via gltf crate, mesh caching in AssetManager (v0.34.0) |
| Instanced rendering | ✅ | InstanceBatch, pre-allocated uniform buffer, no per-frame GPU alloc (v0.34.0) |
| Global error boundary | ✅ | window.onerror + unhandledrejection, toast UI instead of white screen (v0.35.0) |
| Env var validation | ✅ | Fail-fast startup, clear messages for missing/invalid config (v0.35.0) |
| Automated DB backup | ✅ | SQLite backup every 6 hours, keep last 5, tokio background task (v0.35.0) |
| Weather system | ✅ | `WeatherSystem` **registered + ticking** (v0.337.0): 7 conditions, season-driven temperature, smooth transitions; exports `Weather` to the DataStore (Mutex). Real consumer: the survival layer uses its temperature as the **exposed-environment ambient temp** (winter/storms → faster hypothermia outside); the weather HUD bridge reads it. (Renderer/sky visual consumer still pending.) |
| Day/night sky renderer | ✅ | Procedural sky with stars, sun, moon, atmospheric scattering (v0.40.0) |
| Audio system | ✅ | kira crate, spatial 3D audio, music, SFX (v0.39.0) |
| Multiplayer networking | ✅ | WebSocket client, ECS state sync, server authority (v0.39.0) |
| Construction system | ⚠️ | `ConstructionSystem` + `PlacementSystem` implemented but **NOT registered**, never tick; need build-mode UI + placement-event wiring. See `tests/engine_wiring_lint.rs::DEFERRED_SYSTEMS`. |
| Skills progression | ✅ | `SkillSystem` **registered + ticking** (v0.340.0): `SkillRegistry` loads `data/skills/skills.csv` (20 skills, exponential `level^1.5` curve); `PlayerSkills` on the player; XP earned from actions via a shared `xp_grants` channel, **craft → the recipe's skill** (scaled by skill_level), **harvest → farming**, **mine-deliver → mining**; level-ups apply live. recipes.csv `skill_required` was reconciled to canonical skill ids (a non-canonical vocabulary would have silently no-op'd every craft XP) + locked by a drift lint (`skills::skill_tests::every_recipe_skill_is_a_real_skill`). Live levels + XP render in the profile **Skills** panel. **#8b tech-unlock ✅ (v0.341.0):** skills GATE crafting, `CraftingSystem` authoritatively rejects a craft when the crafter is under the recipe's `skill_level`; the crafting page shows "Requires {skill} Lv N (you: Lv M)" + locks the button; a **Dev: max skills** button preserves the 100%-unlocked testing posture. (v0.342.0 fixed a fresh-player deadlock: skill_level 0/1 recipes are the free starter tier; gating begins at level 2.) |
| Quests | ✅ | `QuestSystem` **registered + ticking** (v0.342.0): `QuestRegistry::from_ron_dir` loads `data/quests/*.ron`; the player auto-accepts the **Getting Started** chain. Gather objectives check live inventory; Craft/Harvest advance via a shared `quest_events` channel the action systems push to on completion; completion grants item rewards + **auto-accepts prerequisite-chained** quests. A profile **Quests** panel shows active steps + completed. (The older tutorial/farming/construction chains load but use Build objectives needing the deferred ConstructionSystem.) The relay runs a separate authoritative quest chain for MMO, native-vs-relay reconciliation is the #8c-tail. |
| Mod support framework | ✅ | Mod manifest, load order, data override system (v0.40.0) |
| Heightmap terrain | ✅ | Procedural terrain generation with 16 biome types (v0.42.0) |
| Hydrological system | ⚠️ | `HydrologySystem` implemented but **NOT registered**, never ticks; operates on WaterBody entities not yet spawned. See `tests/engine_wiring_lint.rs::DEFERRED_SYSTEMS`. |
| Atmospheric system | ⚠️ | `AtmosphereSystem` implemented but **NOT registered**, never ticks; operates on EnclosedSpace entities not yet spawned. See `tests/engine_wiring_lint.rs::DEFERRED_SYSTEMS`. |
| Disaster system | ⚠️ | `DisasterSystem` implemented but **NOT registered**, never ticks; spawn is manual + operates on Disaster entities not yet spawned. See `tests/engine_wiring_lint.rs::DEFERRED_SYSTEMS`. |
| World persistence | ✅ | Save/load game world state, entities, terrain (v0.42.0) |
| Data-driven tools | ✅ | tools.rs loads from data/tools/catalog.json, not hardcoded (v0.90.7) |
| Data-driven sounds | ✅ | sounds.rs loads from data/sounds.toml, not hardcoded (v0.90.7) |
| Chat tint colors in theme | ✅ | Moved from hardcoded to theme.ron (v0.90.7) |
| Server config externalized | ✅ | Constants moved to data/server-config.json (v0.90.7) |
| 16 scaffolded system modules | ⚠️ | `aging`, `astronomy`, `creative_arts`, `docking`, `fire`, `genetics`, `geology`, `governance` (system, not the trust-layer governance), `hvac`, `manufacturing`, `medical`, `oceanography`, `offline`, `plumbing`, `transportation`, `waste`, implemented but **NOT registered, so they never tick** (corrected 2026-05-29 game-code audit; the earlier "registered and ticking" was wrong, only 7 systems are actually registered in the runtime). Deferred with reasons in `tests/engine_wiring_lint.rs::DEFERRED_SYSTEMS`. |
| ⚠️ SYSTEMS-TABLE ACCURACY (2026-05-29 audit) | ⚠️ | Many ✅ rows above mean "implemented in code," NOT "registered + running." Only 12 systems actually tick: Time, PlayerController, Interaction, Farming, Inventory, ContainerCompatibility, Crafting, Food (nutrition/hunger/energy/oxygen/temp, v0.330–0.336), Drone (asteroid mining, v0.332.0), Weather (season-driven; drives exposed-env temperature, v0.337.0), Skill (skills/XP from craft/harvest/mine, v0.340.0), Quest (data-driven quests; Gather/Craft/Harvest objectives + rewards + prerequisite chaining, v0.342.0). The rest (ecology, hydrology, atmosphere, disasters, AI, vehicles, construction + the 16 scaffolds) compile but are unregistered, they never run. **`tests/engine_wiring_lint.rs` is the authoritative registered-vs-deferred list** (build fails if a system is neither). The individual system rows above/below are now downgraded to ⚠️ accordingly (done 2026-05-29). |
| Electrical system | ⚠️ | `src/systems/electrical.rs` (~120 LOC), partial AND **NOT registered** (never ticks); needs `PowerGenerator` / `PowerConsumer` ECS components. See `tests/engine_wiring_lint.rs::DEFERRED_SYSTEMS`. |
| Psychology system | ⚠️ | `src/systems/psychology.rs` (~144 LOC), partial AND **NOT registered** (never ticks); `Needs` lives as side state instead of a proper ECS component. See `tests/engine_wiring_lint.rs::DEFERRED_SYSTEMS`. |
| Nutrition / food system | ✅ | `FoodSystem` **registered + ticking** (v0.330.0): Eat → satiation/hydration from `food_system.ron` nutrition profiles, raw food rolls `raw_consumption_risk`→`food_poisoning`, full meal→`well_fed`; hunger/thirst decay → `hungry`/`thirsty` conditions → starvation/dehydration health drain; timed effects expire; plus the original spoilage. `Vitals`+`StatusEffects` ECS components; `StatusEffectRegistry` (status_effects.csv) keeps durations/mods in data. Inventory page shows vitals bars + effect chips. **#3b (v0.334.0):** SPEED modifiers now mechanically applied, the camera scales movement by the player's active effects (`well_nourished` +10% from a good meal, `thirsty`/`flu` −20%); `stamina_regen` + `vision_range` mods still pending (need a stamina system / renderer wiring). **#7a (v0.335.0):** `Vitals.energy` drains while awake → `fatigued` (−15% speed) below 25% → a **Rest** button refills it. **#7b (v0.336.0):** oxygen + body-temperature are environment-coupled, an `EnvironmentContext` from player-position-vs-homestead-AABB drives oxygen drain (hypoxia → suffocation) + body-temp drift (hypothermia / heat exhaustion) when exposed to vacuum/cold, with Health loss; re-entering recovers. Hunger is now tangible (speed) too. **#7c (v0.338.0):** sanitation, organic `waste` accrues → `unsanitary` debuff → **Compost** → `fertilizer_0` → a **Fertilize** crop action boosts growth (closes food→waste→compost→soil→food). Survival baseline = satiation + hydration + energy + oxygen + temperature + waste/sanitation (all 5 listed needs live). See `docs/design/gameplay-loops.md`. |
| Drone↔asteroid mining | ✅ | `DroneSystem` **registered + ticking** (v0.332.0): commission a drone for an ore → Outbound→Mining→Returning state machine → delivers mined ore to the player; an asteroid mined empty is deleted. `AsteroidBody` (finite multi-ore) + `Drone` ECS components; Mining panel on the inventory page. Proven by `mining::drone_tests`. **#5b** (tracked in gameplay-loops.md): server-authoritative MMO asteroids + swarm/abandoned-deletion, 3D voxel asteroid visuals + drone flight, nickel/platinum refine recipes. |
| Emissive materials | ✅ | PBR shader emissive support (params.w = emissive_strength) (v0.90.0) |
| 12 procedural materials | ✅ | Glass, ice, water, leather, crystal, rust, moss, lava + original brick, metal, wood, concrete (v0.90.0) |
| Particle system | ✅ | particles.rs + particle.wgsl, 12 data-driven emitter types from particles.ron (v0.90.0) |
| Bloom post-process | ⚠️ | bloom.rs + bloom.wgsl scaffolding built, needs render loop integration (v0.90.0) |
| Sun direction uniform | ✅ | Data-driven sun direction as shader uniform, not hardcoded (v0.90.8) |
| Planet registry | ✅ | Unified celestial body management for renderer and terrain (v0.90.8) |
| Construction placement | ⚠️ | `PlacementSystem` scaffolded AND **NOT registered** (never ticks); needs full integration. See `tests/engine_wiring_lint.rs::DEFERRED_SYSTEMS`. |

---

## Server & Infrastructure

| Feature | Status | Details |
|---------|--------|---------|
| Rust/axum/tokio server | ✅ | Production-ready relay, now part of unified binary |
| SQLite via rusqlite | ✅ | relay.db at /opt/Humanity/data/ on VPS |
| REST API | ✅ | 30+ endpoints (messages, tasks, projects, listings, reviews, members, etc.) |
| Federation Phase 1+2 | ✅ | Server registry, discovery, S2S WebSocket |
| Signed profile replication | ✅ | signed_profiles table, ProfileGossip between servers (v0.27.0) |
| Federated message persistence | ✅ | Messages persisted with origin_server tag, survive restarts (v0.27.0) |
| Profile lookup API | ✅ | GET /api/profile/{key} for public key lookup (v0.27.0) |
| GitHub webhook | ✅ | Deploy bot announces in chat |
| Admin system | ✅ | Roles, verify, lockdown, wipe, garbage collection |
| Server→Services toggles | ✅ | Soft feature gate + allowlisted OS-daemon (coturn/transmission) start/stop from Server Settings, no SSH (v0.262.16) |
| nginx + VPS pipeline | ✅ | Push to main triggers build + deploy |
| Server membership | ✅ | Auto-join on identify, member roster, paginated search (v0.25.0) |
| Server-info endpoint | ✅ | Description, owner_key, funding, member_count (v0.25.0) |
| Server game state authority | ✅ | Authoritative server for game state validation (v0.40.0) |
| Admin analytics dashboard | ✅ | Server metrics, user activity, system health monitoring (v0.40.0) |
| Guild system | ✅ | Create, join, search guilds with invite codes (v0.41.0) |
| Reputation system | ✅ | Points, levels, leaderboard for community standing (v0.41.0) |
| Unified binary deploy | ✅ | systemd service uses `HumanityOS --headless`, relay.db at /opt/Humanity/data/ (v0.90.0) |

---

## Navigation & UX

| Feature | Status | Details |
|---------|--------|---------|
| shell.js hub navigation | ✅ | Injected on every page, 20+ nav links |
| Standalone pages | ✅ | 20+ pages (tasks, maps, wallet, donate, settings, etc.) |
| Mobile navigation | ✅ | Touch drawer menus |
| Light/dark theme | ✅ | Toggle in shell, persisted |
| PWA support | ✅ | Manifest + service worker |
| Keyboard shortcuts | ✅ | Global shortcuts via shell.js |
| Onboarding tour | ✅ | 8-step guided walkthrough for new users (v0.25.0) |
| Real/Sim context toggle | ✅ | Global mode switch between real-life tools and simulation (v0.38.1) |
| Color-coded nav groups | ✅ | Red (identity), green (context-sensitive), blue (system) nav groups (v0.37.2) |
| Localization | ✅ | 5 language translations (v0.40.0) |
| Accessibility | ✅ | High contrast, colorblind modes, reduced motion support (v0.40.0) |

---

## Native Desktop Client

| Feature | Status | Details |
|---------|--------|---------|
| Standalone Rust binary | ✅ | egui + wgpu, DX12 on Windows, unified binary with relay (v0.90.0) |
| egui GUI system | ✅ | Immediate-mode UI with theme.ron, reusable widgets (v0.36.0) |
| Settings page | ✅ | Theme, display, controls, security (key unlock button) (v0.36.0, v0.89.0) |
| Inventory page | ✅ | Item management UI (v0.36.0) |
| Chat page (3-panel) | ✅ | DMs (red), Groups (green cards), Servers (blue), message feed, input bar (v0.89.0) |
| HUD page | ✅ | Health, status, interaction prompts (v0.36.0) |
| Hot-reloadable theme | ✅ | theme.ron for colors, spacing, fonts; live reload (v0.36.0) |
| Deferred 3D loading | ✅ | Chat loads instantly; 3D world loads on Enter World (v0.89.0) |
| Zero-friction startup | ✅ | No passphrase prompt, no main menu; returning users go straight to chat (v0.89.0) |
| Config persistence | ✅ | config.json at %APPDATA%/HumanityOS/; panel widths, collapse state, server URL, key encryption (v0.89.0, v0.90.0) |
| 13 universal widgets | ✅ | badge, detail_row, search_bar, sidebar_nav, category_filter, stat_card, button, data_table, icons, item_list, modal, row, toolbar (v0.90.1) |
| 6 new theme colors | ✅ | bg_panel, bg_sidebar, bg_sidebar_dark, badge styling (v0.90.1) |
| Compact theme values | ✅ | All spacing/sizes halved for visual density (v0.90.3) |
| 35+ theme variables | ✅ | Editable in Settings > Widgets section (v0.90.3) |
| Slider widget | ✅ | Blue-green-red gradient + animated RGB knob (v0.90.1) |
| DMs/Groups cog menus | ✅ | Settings cog menus on DMs/Groups headers (v0.90.1) |
| Server header cog | ✅ | Cog replaces X disconnect button (v0.90.1) |
| All 27 pages refactored | ✅ | Every page uses theme + universal widgets consistently (v0.90.1) |
| ~~ECDH P-256 DM encryption~~ | ❌ | **Removed (v0.264.0).** Native DM now uses pure Kyber768/ML-KEM-768 (`src/net/dm_pq.rs`), byte-identical to web. The ECDH path and its Settings import UI were deleted in the PQ cutover |
| PQ DM (native) | ✅ | Kyber768 dual-seal `{v:1,r,s}` envelope via `src/net/dm_pq.rs`; recipient key deterministic from seed; KAT-locked with web (v0.264.0) |

> **Note:** Source code lives in `src/` at the repo root (unified binary). `native/` and `server/` directories no longer exist. Binary output is `target/release/HumanityOS.exe`.

---

## Web Tools & Utilities

| Feature | Status | Details |
|---------|--------|---------|
| Civilization dashboard | ✅ | Macro community/infrastructure view with live API data (v0.39.0) |
| File browser/editor | ✅ | Browse, view, and edit files with built-in viewers (v0.39.0) |
| Tools catalog | ✅ | 37 open-source apps across 11 categories, data-driven from catalog.json (v0.39.0, v0.90.7) |
| Calculator | ✅ | Basic, scientific, and unit converter modes (v0.39.0) |
| Calendar/planner | ✅ | Event creation, scheduling, and reminders (v0.39.0) |
| Notes/journal | ✅ | Markdown preview, encrypted notes, daily log (v0.39.0) |
| Resources page | ✅ | 45 curated real-world resource links across categories (v0.39.0) |
| Glossary system | ✅ | 150+ terms with definitions, searchable overlay (v0.41.0) |
| Projects page | ✅ | Project Universe timeline (Dec 2017 ICU through Jan 2026 rename) (v0.90.0) |

---

## Local-First Storage

| Feature | Status | Details |
|---------|--------|---------|
| OS-standard data dir | ✅ | `%APPDATA%\HumanityOS\` with identity, saves, settings, cache, backups |
| Save slots | ⚠️ | The full multi-slot model (profile/farm/quests/world) is design + test-only. What actually persists between sessions today (v0.381 `src/save_load.rs`, single `offline_home.json`): **inventory + skills**, applied at startup, saved on close + periodically. Health/position/game-time/vitals/crops/quests still reset every launch (the next persistence increment). |
| Auto-rotating backups | ✅ | Keeps last 5 timestamped snapshots |
| USB drive detection | ✅ | Detects removable drives for export/import |
| Tiered sync config | ✅ | Configurable sync levels |
| Data management UI | ✅ | web/pages/data.html with saves, backups, sync settings, USB tabs |

---

## Game Data

| Feature | Status | Details |
|---------|--------|---------|
| Chemistry database | ✅ | 118 elements, 59 alloys, 132 compounds, 35 gases, 52 toxins (v0.42.0) |
| Solar system database | ✅ | 70+ celestial bodies with orbital and physical data (v0.42.0) |
| Materials database | ✅ | 92 materials with properties (v0.42.0) |
| Components database | ✅ | 102 components for crafting/construction (v0.42.0) |
| Items and recipes | ✅ | 404 items, 371 recipes (expanded v0.90.0) |
| Plants database | ✅ | 161 plants with growth stages and requirements (expanded v0.90.0) |
| Creatures database | ✅ | 123 creatures with behaviors and stats (v0.90.0) |
| Spells database | ✅ | 149 spells across multiple schools of magic (v0.90.0) |
| Structures database | ✅ | 163 structures for construction (v0.90.0) |
| Status effects | ✅ | 80 status effects (buffs, debuffs, conditions) (v0.90.0) |
| Enchantments | ✅ | 133 enchantments for equipment (v0.90.0) |
| Trade goods | ✅ | 185 items with balanced pricing (v0.90.0) |
| Factions | ✅ | Faction definitions with relations and territories (v0.90.0) |
| Biomes | ✅ | Biome definitions with flora/fauna/climate (v0.90.0) |
| Tech tree | ✅ | Technology progression tree (v0.90.0) |
| NPCs | ✅ | NPC definitions with dialogue triggers (v0.90.0) |
| Dialogues | ✅ | Dialogue trees with branching choices (v0.90.0) |
| Particles | ✅ | 12 particle emitter definitions for effects (v0.90.0) |
| Sounds | ✅ | Sound configuration (sounds.toml) (v0.90.0) |
| Offline behaviors | ✅ | Autonomous agent presets for off-screen NPCs (v0.90.0) |
| Simulation systems | ✅ | electrical, plumbing, hvac, transportation, fire, docking RON files (v0.90.0) |
| Real-world systems | ✅ | governance, psychology, medical, food_system, economy, creative_arts, aging_fitness RON files (v0.90.0) |
| Science systems | ✅ | geology, oceanography, astronomy_tools, genetics, manufacturing, waste_management RON files (v0.90.0) |
| Data schemas | ✅ | 22 TOML schemas documenting all data formats (v0.90.0) |
| Platform brand SVGs | ✅ | Steam, Epic, GOG, PlayStation, Xbox icons (v0.41.0) |
| 108 total data files | ✅ | ~3000+ entries across CSV/TOML/RON/JSON (v0.90.0) |

---

## What to Build Next

**This section is a pointer, not a plan.** The authoritative ranked backlog is
**`docs/PRIORITIES.md`**, read it for the current top item and full ordering.
The summary below reflects PRIORITIES as of 2026-05-28; if it has drifted from
PRIORITIES.md, trust PRIORITIES.md.

- **Currently active (Track W):** clean rebuild of the web chat *view* to mirror
  native 1:1, keeping the proven JS engine (WS/crypto/WebRTC). Spec:
  `docs/design/chat-layout.md`.
- **TIER 0, pre-public-launch blockers:** the only open item is fixing nginx
  `/health` routing (public `https://united-humanity.us/health` returns 404 while
  internal returns 200). Everything else in TIER 0 is DONE (off-site backup,
  release-mirror retention, Inc6 wipe, orphan-admin cleanup, TLS auto-renew, etc.).
- **TIER 1, hardening:** effectively closed (fail2ban, watchdog+alerting,
  SQLite corruption recovery all shipped; off-box monitor skipped by operator).
- **TIER 2, big-feature gaps (weeks each):** web↔native parity (Track W),
  Studio+streaming, in-app ops console, **native voice (no WebRTC stack at all
  today)**, federation *activation* (designed, dormant=safe, not turned on),
  native trade UI completion (events not dispatched), Litestream continuous
  backup, mobile clients (Android/iOS), device mesh, federated Library, and
  **P2P groups phases 3–5** (P2P transport / relay-independence / mDNS-DHT, 
  phases 1–2 are done: create/invite/join/E2EE-chat/leave/disband work on both
  clients as of v0.301–v0.304).
- **TIER 3, ELI5 accessibility mandate:** tooltip pass, first-5-minutes
  onboarding flow, localization expansion (5→11+ languages), full WCAG 2.1 AA
  audit, native glossary widget. Largely NOT done.
- **TIER 4, long horizon:** LoRa hardware, STARK selective disclosure, deep
  game-world simulation, AI-agent governance enforcement, distribution beyond
  GitHub/Forgejo.

---

## What's NOT done (corrective summary)

> **The previous version of this file claimed "0 missing" across every category.
> That was wrong and dangerously misleading**, it contradicted the live backlog.
> HumanityOS has a large amount of built infrastructure AND a large amount of
> remaining work. The honest high-level picture:

**Known partial / unverified (not "done"):**
- Native client is **chat-first and missing real-time media**: no native WebRTC
  stack → native voice, video, screen-share, and streaming are stubs/observer-only
  (web has them). (PRIORITIES TIER 2 #2, #4, #6.)
- Native **trade UI** page exists but trade events aren't dispatched (TIER 2 #7).
- **Federation** code is fail-closed and dormant, designed, not activated; no
  admin UI to add/trust peers yet (TIER 2 #5).
- **Bloom post-process** scaffolding built, render-loop integration unverified.
- **16+ scaffolded game systems** (aging, astronomy, fire, genetics, geology,
  hvac, manufacturing, medical, plumbing, transportation, waste, etc.) tick but
  have no real behaviour; **electrical** and **psychology** are partial. (These
  were flipped ✅→⚠️ in the v0.122 truthfulness pass; the "Game Engine" rows below
  still over-count them as built.)
- **Most ops/config is CLI/SSH-only** (alerts, backups, fail2ban, relay control,
  secrets), violates the GUI-first mandate; tracked as debt in
  `docs/design/in-app-ops.md` (TIER 2 #3).
- Many ✅ rows below predate v0.132 and have **not been re-verified** against the
  current binary.

**Known missing / not built:**
- Mobile clients (Android, iOS), TIER 2 #5.
- Device mesh (system-info reporting, designate-backup, restore, LAN sync), TIER 2 #6.
- Federated Library (files/software/web catalog), TIER 2 #7.
- P2P groups phases 3–5 (true P2P transport + relay-independence + serverless
  discovery), TIER 2 #8.
- The entire TIER 3 accessibility layer (tooltips, onboarding flow polish,
  expanded localization, WCAG audit, native glossary), mission-critical, mostly
  not started.

**Do not treat the per-category counts in old revisions of this file as a
completeness score.** Use `docs/PRIORITIES.md` for what remains.
