# HumanityOS — Claude Context

Open-source cooperative platform. Goal: end poverty, unite humanity.
Live: https://united-humanity.us | GitHub: https://github.com/Shaostoul/Humanity
SSH alias: `humanity-vps` (server1.shaostoul.com)

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
  ↕ HTTP /api/*           ← api.rs    (~1500 LOC, REST endpoints)

Server: Rust/axum/tokio, SQLite via rusqlite
Static HTML served by nginx from /var/www/humanity/
```

## File map

| Path | Role |
|------|------|
| `crates/humanity-relay/src/relay.rs` | WS message routing, rate limiting, auth |
| `crates/humanity-relay/src/api.rs` | REST API handlers |
| `crates/humanity-relay/src/main.rs` | Router setup, CSP middleware, axum config |
| `crates/humanity-relay/src/storage/` | 14 domain modules (messages, channels, tasks…) |
| `crates/humanity-relay/src/handlers/` | broadcast.rs, federation.rs, utils.rs |
| `crates/humanity-relay/client/app.js` | Core chat logic (~1700 LOC) |
| `crates/humanity-relay/client/chat-*.js` | messages, dms, social, ui, voice, profile, p2p |
| `crates/humanity-relay/client/crypto.js` | Ed25519/ECDH/AES + BIP39 + backup helpers |
| `shared/shell.js` | Nav injection IIFE — loaded first on every page |
| `shared/settings.js` | Settings panel + gear button (don't call injectGearButton on pages with shell.js) |
| `desktop/src-tauri/src/main.rs` | Tauri wrapper — loads united-humanity.us |
| `pages/*.html` | Standalone feature pages — vault, tasks, studio, etc. |
| `docs/` | ALL documentation — design specs, guides, operations, schemas |
| `accord/` | Humanity Accord governance docs (21 files) |
| `website/` | Jekyll source for GitHub Pages (.io site) |
| `Justfile` | Dev command runner — `just --list` for all recipes |

## Script load order (chat)

`crypto.js` → `app.js` → `chat-messages.js` → `chat-dms.js` → `chat-social.js` →
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

**Multiple `impl Storage` blocks** across `storage/*.rs` — Rust allows this within one crate

## Version SOP (MANDATORY before every push)

**Semver rules:**
- `0.X.0` → Rust code changed (requires recompile on VPS)
- `0.X.Y` → Non-Rust changes only (HTML/JS/CSS/docs/config)
- `1.0.0` → Reserved for fully functional product

**Before pushing, ALWAYS:**
1. Check current version: `gh release view --repo Shaostoul/Humanity --json tagName`
2. Bump the patch (Y) for non-Rust changes, minor (X) for Rust changes
3. Update ALL version strings (they MUST stay in sync):
   - `desktop/src-tauri/tauri.conf.json` → `"version"`
   - `desktop/src-tauri/Cargo.toml` → `version`
   - `shared/sw.js` → `CACHE_NAME` (bump number)
   - `pages/settings.html` → version tag text
   - `pages/ops.html` → debug version text
   - `game/download.html` → fallback version badge + subtitle
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
```

## Known gotchas

- `settings.js` has `injectGearButton()` — don't call it on pages that also load `shell.js` (already fixed: guards for `a[href="/settings"]`)
- Tasks scope filter: `activeScope = 'cosmos'` by default; task labels must match or they're filtered out
- WebView2 (Tauri desktop) aggressively caches — use Ctrl+Shift+Delete (or Settings → Advanced → Clear Cache) to bust; or manually delete `%LOCALAPPDATA%\HumanityOS\EBWebView\Default\Cache`
- Deploy `git pull` fails if server has local changes → `just sync` fixes it
- CSP `'unsafe-inline'` retained for inline event handlers on HTML pages

## Current targets (see docs/roadmap.md)

1. Push notifications (WebPush API)
2. FTS5 full-text message search (upgrade from LIKE queries)
3. Server-side user directory endpoint (paginated roster with profiles)
4. Task assignments + email/push notifications when assigned
5. Crypto payment layer (see design/economy/crypto_exchange.md)
6. In-app browser (webview tabs for external URLs — Tauri webview panels)
