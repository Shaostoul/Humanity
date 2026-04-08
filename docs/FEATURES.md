# HumanityOS Features Directory

Complete inventory of every feature, where it lives, and what it does. Updated v0.90.0.

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

## Communication

### Chat (Text Messaging)
Real-time text chat with channels, threads, and message history.
- Web: `web/chat/app.js`, `web/chat/chat-messages.js`
- Server: `src/relay/relay.rs` (WebSocket routing), `src/relay/storage/messages.rs`

### Direct Messages (E2E Encrypted)
Private 1-on-1 conversations encrypted with ECDH + AES-256-GCM.
- Web: `web/chat/chat-dms.js`
- Server: `src/relay/storage/dms.rs`

### Voice Channels
Group voice chat rooms with join/leave sounds.
- Web: `web/chat/chat-voice.js`, `web/chat/chat-voice-rooms.js`

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

### Ed25519 Identity
Cryptographic keypair IS your identity. No accounts, no passwords.
- Web: `web/chat/crypto.js` (key generation, signing)
- Server: `src/relay/relay.rs` (signature verification)

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
Encrypted cloud backup of settings/keys (AES-256-GCM + PBKDF2).
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

### Projects Page
Project Universe timeline (Dec 2017 ICU through Jan 2026 rename to HumanityOS).
- Web: `web/pages/projects.html`

---

## Maps

### Multi-Scale Map
Galaxy to street level zoom on 2D canvas. Galaxy spiral, solar system, planet globe, OpenStreetMap tiles. Moon orbit fixed (v0.90.0).
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
Server-side game world with entity management, position validation, player sync.
- Server: `src/relay/handlers/game_state.rs`

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

### 8 Procedural Materials (v0.90.0)
Glass, ice, water, leather, crystal, rust, moss, lava added to procedural material system.
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

### Shader Library
41 WGSL shaders: planet surfaces (earth, mars, venus, mercury, jupiter, saturn, uranus, neptune, moon, pluto), sun surface/glow, PBR, procedural materials (brick, metal, wood, concrete, fabric, aperiodic), stars, constellations, orbit rings, ghost preview, particles, bloom.
- Shaders: `assets/shaders/`, `assets/shaders/procedural/`

---

## Game Systems

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
7 conditions (clear, cloudy, rain, storm, snow, fog, sandstorm). Seasonal transitions.
- Native: `src/systems/weather.rs`

### Hydrological System
Rain cycle, rivers, aquifers, contamination tracking, water table simulation.
- Native: `src/systems/hydrology.rs`

### Atmospheric System
Gas tracking, explosions, suffocation, pressure simulation.
- Native: `src/systems/atmosphere.rs`

### Disaster System
21 disaster types with chain reactions, severity scaling, black holes.
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
Blueprint placement, snap-to-grid, timed building, material consumption.
- Native: `src/systems/construction/mod.rs`
- Data: `data/blueprints/basic.ron`

### Skills/Progression
20 skills across 5 categories, XP curves, level-up notifications.
- Native: `src/systems/skills/mod.rs`
- Data: `data/skills/skills.csv`

### AI Behaviors
5 behavior types (passive, aggressive, herd, predator, guard) with state machines.
- Native: `src/systems/ai/mod.rs`

### Vehicles/Mechs
Enter/exit vehicles, torso twist, jump jets, heat management.
- Native: `src/systems/vehicles/mod.rs`

### Ecology/Disease
Disease spread by proximity, seasonal effects, population tracking.
- Native: `src/systems/ecology.rs`

### Quests
Data-driven quest progression from RON files. 6 objective types.
- Native: `src/systems/quests/mod.rs`
- Data: `data/quests/*.ron`

### Combat
Damage calculation, status effects.
- Native: `src/systems/combat/`

### Economy
Fleet resource management.
- Native: `src/systems/economy/`

### Navigation
Multi-scale navigation (galaxy, system, orbital, surface).
- Native: `src/systems/navigation/`

### Logistics
Cargo transport and shipping routes.
- Native: `src/systems/logistics/`

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
Data-driven simulation modules for engineering and infrastructure.
- Data: `data/electrical.ron`, `data/plumbing.ron`, `data/hvac.ron`, `data/transportation.ron`, `data/fire_system.ron`, `data/docking.ron`

### Real-World Systems (v0.90.0)
Data definitions for social and biological simulation.
- Data: `data/governance.ron`, `data/psychology.ron`, `data/medical.ron`, `data/food_system.ron`, `data/economy.ron`, `data/creative_arts.ron`, `data/aging_fitness.ron`

### Science Systems (v0.90.0)
Data definitions for natural science simulation.
- Data: `data/geology.ron`, `data/oceanography.ron`, `data/astronomy_tools.ron`, `data/genetics.ron`, `data/manufacturing.ron`, `data/waste_management.ron`

### Data Schemas (v0.90.0)
22 TOML schema files documenting all data formats for modding and validation.
- Data: `schemas/*.toml` (item, material, component, creature, spell, structure, status_effect, enchantment, recipe, quest, biome, celestial_body, faction, npc, skill, sound, vehicle, weather, economy, offline_agent, equipment_slot, container)

### Platform Brand SVGs
Platform detection icons (Steam, Epic, GOG, PlayStation, Xbox) as inline SVGs.
- Assets: `assets/icons/platforms/`

### Total: 108 data files, ~3000+ entries (v0.90.0)
