# HumanityOS — Claude Context

Open-source cooperative platform. Goal: end poverty, unite humanity.
Live: https://united-humanity.us | GitHub: https://github.com/Shaostoul/Humanity
SSH alias: `humanity-vps` (server1.shaostoul.com)

> **⚠️ START HERE:** Read `docs/STATUS.md` for comprehensive feature inventory (what's built, what's not, what's next). Prevents re-researching existing features.

## Quick orientation

```
just ship "message"   # commit + push + force-sync VPS  ← daily driver
just sync             # force-sync VPS now               ← when CI breaks
just sync-web         # assets only, no rebuild (fast)   ← front-end changes
just status           # git + CI + live API health
just logs             # tail server logs
```

## Architecture

```
Client (browser/Tauri WebView2)
  ↕ WebSocket /ws         ← relay.rs  (~5800 LOC, message routing)
  ↕ HTTP /api/*           ← api.rs    (~2500 LOC, REST endpoints)

Server: Rust/axum/tokio, SQLite via rusqlite
Static HTML served by nginx from /var/www/humanity/

Game Engine: Rust/wgpu (native + WASM/WebGPU)
  ├ ECS: hecs + SystemRunner (System trait, per-frame tick)
  ├ Physics: rapier3d (rigid bodies, colliders, raycasting)
  ├ Terrain: icosphere planets (LOD subdivision) + voxel asteroids (sparse octree)
  ├ Ships: data-driven layouts from RON, room mesh generation
  ├ Data: AssetManager loads CSV/TOML/RON/JSON from external files
  └ Hot-reload: notify file watcher, cache invalidation per frame

Identity: Ed25519 key = identity = Solana wallet address
  ├ No home servers, no accounts, no passwords
  ├ Signed profiles replicate across all federated servers
  └ BIP39 24-word seed phrase backs up everything
```

## File map

| Path | Role |
|------|------|
| `server/src/relay.rs` | WS message routing, rate limiting, auth |
| `server/src/api.rs` | REST API handlers |
| `server/src/main.rs` | Router setup, CSP middleware, axum config |
| `server/src/storage/` | 17 domain modules (messages, channels, tasks, signed_profiles, notification_prefs…) |
| `server/src/handlers/` | broadcast.rs, federation.rs, msg_handlers.rs, utils.rs |
| `engine/src/` | Game engine: renderer, ECS, physics, audio, input, hot-reload, terrain, ship |
| `engine/src/systems/` | 11 game systems: farming, inventory, crafting, time, player, interaction, ai, vehicles, ecology, quests, combat |
| `engine/src/terrain/` | Icosphere planets (LOD), voxel asteroids (sparse octree, greedy mesh) |
| `engine/src/ship/` | Ship layouts from RON, room mesh generation, BFS pathfinding |
| `engine/src/assets/` | AssetManager (CSV/TOML/RON/GLTF loading), FileWatcher, hot-reload |
| `engine/src/physics/` | rapier3d wrapper: rigid bodies, colliders, raycasting, simulation step |
| `engine/crates/` | 19 sub-crates (core, modules, persistence, etc.) |
| `app/src/main.rs` | Tauri shell — local-first + background sync |
| `app/src/storage.rs` | Local-first data persistence (saves, backups, USB detection, sync config) |
| `ui/chat/app.js` | Core chat logic (~1700 LOC) |
| `ui/chat/chat-*.js` | messages, dms, social, ui, voice, profile, p2p |
| `ui/chat/crypto.js` | Ed25519/ECDH/AES + BIP39 + backup helpers |
| `ui/shared/events.js` | Lightweight event bus (`hos.on/off/emit/gather`) |
| `ui/shared/shell.js` | Nav injection IIFE — loaded first on every page |
| `ui/shared/settings.js` | Settings panel + gear button |
| `ui/pages/*.html` | Standalone feature pages — tasks, maps, settings, etc. |
| `ui/pages/data.html` | Data management UI (saves, backups, sync tiers, USB import/export) |
| `ui/activities/` | Game/real-world activities — gardening, download, etc. |
| `assets/` | All shared media — icons, shaders, models, textures, audio |
| `data/` | Hot-reloadable game data — CSV, TOML, RON, JSON |
| `docs/` | ALL documentation — design, accord, history, website |
| `Justfile` | Dev command runner — `just --list` for all recipes |

## Script load order (chat)

`crypto.js` → `events.js` → `app.js` → `chat-messages.js` → `chat-dms.js` → `chat-social.js` →
`chat-ui.js` → `chat-voice.js` → `chat-profile.js` → `qrcode.js` → `chat-p2p.js`

## All REST routes

```
GET  /health
WS   /ws                              ← main WebSocket

GET  /api/messages                    query: channel, before_id, limit
POST /api/send                        authenticated via Ed25519 sig
GET  /api/search                      query: q, channel, from
GET  /api/peers
GET  /api/stats
GET  /api/reactions                   query: message_id
GET  /api/pins                        query: channel
POST /api/upload                      multipart, requires upload token
POST /api/github-webhook

GET/POST   /api/tasks
PATCH/DEL  /api/tasks/{id}
GET/POST   /api/tasks/{id}/comments

GET/POST   /api/assets
DELETE     /api/assets/{id}
GET/POST   /api/listings
GET        /api/federation/servers
GET/POST   /api/skills/search
GET        /api/skills/{user_key}
GET/PUT/DEL /api/vault/sync           authenticated via Ed25519 sig
GET        /api/server-info
GET        /api/profile/{key}           signed profile lookup by public key
GET/POST   /api/projects
PATCH/DEL  /api/projects/{id}
GET/POST   /api/listings/{id}/images
DELETE     /api/listings/{id}/images/{img_id}
GET/POST   /api/listings/{id}/reviews
DELETE     /api/listings/{id}/reviews/{rev_id}
GET        /api/members                 paginated, ?search=
GET        /api/members/count
GET        /api/members/{key}
GET        /api/sellers/{key}/rating
```

## Key patterns

**Ed25519 identity** (set in app.js `connect()`):
```js
myIdentity = { publicKeyHex, privateKey, publicKey, canSign }
```

**Relay unicast**: `target: Option<String>` on message variant; broadcast loop `continue`s if target≠my_key

**Authenticated API requests** (vault sync, key rotation):
```js
sign("vault_sync\n" + timestamp, privateKey)   // timestamp = Date.now()
// Server validates: freshness ≤5 min + Ed25519 sig
```

**Key rotation certificate** (dual-sign):
```
sig_by_old = sign(new_key + "\n" + timestamp, old_private_key)
sig_by_new = sign(old_key + "\n" + timestamp, new_private_key)
```

**AES-256-GCM + PBKDF2-SHA256** (vault, notes, backup):
`deriveKeyFromPassphrase(passphrase, salt)` → CryptoKey (600k iterations)

**Rate limiting**: Fibonacci backoff per public key in `relay.rs`

**Game System trait** (engine/src/ecs/systems.rs):
```rust
trait System: Send + Sync {
    fn name(&self) -> &str;
    fn tick(&mut self, world: &mut hecs::World, dt: f32, data: &DataStore);
}
```
Systems registered with `SystemRunner`, ticked in order each frame.

**Signed profile replication** (no home server):
Profiles are signed objects. Any server caches them. Latest timestamp wins.
ProfileGossip messages propagate profiles between federated servers.

**Data-driven game content** (Space Engineers style):
Game data in external files next to exe. CSV for items/plants/recipes,
TOML for config, RON for quests/blueprints/ships/planets. Hot-reloadable
via notify file watcher. Mods = editing files in the data directory.

**Multiple `impl Storage` blocks** across `storage/*.rs` — Rust allows this within one crate

**Local-first storage** (app/src/storage.rs):
OS-standard data dir (`%APPDATA%\HumanityOS\` on Windows) with:
- `identity/` — encrypted Ed25519 keys
- `saves/` — named save slots (profile, inventory, farm, quests, skills, world)
- `settings/` — preferences, sync config, display state
- `cache/` — offline messages, avatars, manifests
- `backups/` — timestamped snapshots (auto-rotate, keep last 5)
Tauri commands: list_saves, create_save, delete_save, export_save, import_save, detect_drives, get_sync_config, set_sync_config, get_data_dir, get_storage_stats, create_backup, relocate_data

## Version SOP (MANDATORY before every push)

**Semver rules:**
- `0.X.0` → Rust code changed (requires recompile on VPS)
- `0.X.Y` → Non-Rust changes only (HTML/JS/CSS/docs/config)
- `1.0.0` → Reserved for fully functional product

**Before pushing, ALWAYS:**
1. Check current version: `gh release view --repo Shaostoul/Humanity --json tagName`
2. Bump the patch (Y) for non-Rust changes, minor (X) for Rust changes
3. Update ALL version strings (they MUST stay in sync):
   - `app/tauri.conf.json` → `"version"`
   - `app/Cargo.toml` → `version`
   - `ui/shared/sw.js` → `CACHE_NAME` (bump number)
   - `ui/pages/settings-app.js` → version tag text
   - `ui/pages/ops.html` → debug version text
   - `ui/activities/download.html` → fallback version badge + subtitle
   - `ui/shared/shell.js` → version string
4. Commit the version bump IN the same commit (not separate)
5. After push: `git tag vX.Y.Z && git push origin vX.Y.Z` (only if Rust changed or desktop release needed)

**Never delete/re-tag** — always increment to next version number.

## Deploy pipeline

Push to `main` → GitHub Actions → SSH to VPS → `cargo build` → rsync + copy → restart relay

When CI fails (server has local changes or build error):
```bash
just sync    # fetches, git reset --hard, rebuilds, rsyncs, restarts
```

**VPS paths**:
- Repo: `/opt/Humanity/`
- Web root: `/var/www/humanity/`
- Relay binary: `/opt/Humanity/target/release/humanity-relay`
- SQLite DB: `/opt/Humanity/data/relay.db`
- Uploads: `/opt/Humanity/data/uploads/`

## Storage schema (key tables)

```sql
messages       (id, channel, sender_name, sender_key, content, timestamp, signature, edit_history, thread_parent_id, reply_count, metadata)
channels       (id, name, description, created_by, created_at, topic, is_private)
profiles       (name, bio, socials, avatar_url, banner_url, pronouns, location, website, streaming_url, streaming_live)
tasks          (id, title, description, status, priority, assignee, created_by, created_at, updated_at, labels)
task_comments  (id, task_id, author_key, author_name, content, created_at)
follows        (follower_key, followee_key, created_at)
vault_blobs    (public_key, blob, updated_at)
key_rotations  (old_key, new_key, sig_by_old, sig_by_new, rotated_at)
uploads        (id, uploader_key, filename, url, size, mime_type, created_at)
signed_profiles (public_key, name, bio, avatar_url, socials, timestamp, signature)
projects       (id, name, description, owner_key, visibility, color, icon, created_at)
listing_images (id, listing_id, url, position, created_at)
listing_reviews (id, listing_id, reviewer_key, rating, comment, created_at)
listing_messages (id, listing_id, sender_key, sender_name, content, timestamp)
notification_prefs (public_key, dm_enabled, mentions_enabled, tasks_enabled, dnd_start, dnd_end)
server_members (public_key, name, role, joined_at, last_seen)
```

## Known gotchas

- `settings.js` has `injectGearButton()` — don't call it on pages that also load `shell.js` (already fixed: guards for `a[href="/settings"]`)
- Tasks scope filter: `activeScope = 'cosmos'` by default; task labels must match or they're filtered out
- WebView2 (Tauri desktop) aggressively caches — use Ctrl+Shift+Delete (or Settings → Advanced → Clear Cache) to bust; or manually delete `%LOCALAPPDATA%\HumanityOS\EBWebView\Default\Cache`
- Deploy `git pull` fails if server has local changes → `just sync` fixes it
- CSP `'unsafe-inline'` retained for inline event handlers on HTML pages

## Current targets (v0.35.0)

1. Multiplayer sync (networked ECS state replication)
2. In-game UI/HUD (health, inventory, interaction prompts)
3. Audio system (spatial audio, music, SFX via kira crate)
4. Map rework (replace 2D canvas solar system with 3D engine orbit mode)
5. Third-party logos on download page (replace placeholder SVGs)
