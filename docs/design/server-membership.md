# Server Membership Model

## Core Concept

A "server" is someone's public deployment of HumanityOS. Example: Shaostoul runs `united-humanity.us`. Anyone can connect as a guest and chat. "Joining" means setting that server as your home server — your data syncs there, you appear in its directory, and you participate in its projects.

Your identity (Ed25519 keys) is portable. You are never locked into a server.

## Membership Tiers

| Tier | Access |
|------|--------|
| **Guest** | Connected via WebSocket, can chat, no data sync, not in directory |
| **Member** | Joined, profile syncs, appears in member directory |
| **Contributor** | Member + can create/edit tasks in public projects |
| **Moderator** | Existing mod role (channel management, message moderation) |
| **Admin** | Existing admin role (server configuration, member management) |
| **Owner** | Server operator, one per server, full control |

Trust is progressive: guest -> member -> contributor -> mod -> admin. Owner promotes members through tiers.

## Schema

```sql
CREATE TABLE IF NOT EXISTS server_members (
    public_key TEXT PRIMARY KEY,
    name TEXT,
    role TEXT NOT NULL DEFAULT 'member',  -- member, contributor, mod, admin
    joined_at TEXT NOT NULL,
    last_seen TEXT
);
```

Guests have no row — they're just connected WebSocket peers. Owner is stored in server config, not this table.

## API

| Method | Route | Auth | Description |
|--------|-------|------|-------------|
| GET | `/api/server-info` | No | Server name, owner profile, member count, public project count |
| POST | `/api/join` | Ed25519 | Join as member. Body: `{ name }`. Server creates member record |
| POST | `/api/leave` | Ed25519 | Leave server. Deletes member record, stops sync |
| GET | `/api/members` | No | Paginated directory. Query: `?page=1&limit=50`. Returns public profiles + roles |

Auth follows existing pattern: `sign("join\n" + timestamp, privateKey)`, server validates freshness <= 5 min + signature.

## WebSocket Messages

### Client -> Server

```json
{ "type": "server_join", "name": "Alice", "signature": "...", "timestamp": 1234567890 }
```
```json
{ "type": "server_leave", "signature": "...", "timestamp": 1234567890 }
```

### Server -> Client

```json
{ "type": "member_joined", "public_key": "abc...", "name": "Alice", "role": "member" }
```
```json
{ "type": "member_left", "public_key": "abc..." }
```
```json
{ "type": "member_list", "members": [{ "public_key": "...", "name": "...", "role": "...", "last_seen": "..." }], "page": 1, "total": 42 }
```
```json
{ "type": "role_changed", "public_key": "abc...", "role": "contributor", "changed_by": "def..." }
```

## Client-Side Integration

### Settings page (desktop app)

```
My Server: [united-humanity.us]  [Connect]
Status: Connected as Member
Sync: Profile [x]  Saves [x]  Vault [x]
```

Maps to existing `get_sync_config` / `set_sync_config` commands. The server URL is stored in `settings/sync.json` in the local data directory (`%APPDATA%\HumanityOS\` on Windows).

### Profile page

Shows home server and role: `Member of united-humanity.us`

### Navigation

Server name displayed at top of sidebar (like the current channel list header). Clicking it shows server info panel with owner, member count, and public projects.

## Join Flow

1. User enters `united-humanity.us` in Settings and clicks Connect
2. App establishes WebSocket connection (existing `connect()` in app.js)
3. App sends `server_join` message (signed with Ed25519 private key)
4. Server validates signature, creates `server_members` row with role `member`
5. Server broadcasts `member_joined` to all connected peers
6. App stores server URL in local sync config
7. Profile syncs to server via existing vault sync mechanism
8. User appears in `/api/members` directory
9. Saves/vault sync per user's sync tier preferences

## Leave Flow

1. User clicks Leave in Settings (or switches to a different server)
2. App sends `server_leave` message (signed)
3. Server deletes `server_members` row
4. Server broadcasts `member_left`
5. App clears server URL from local sync config
6. User's local data is untouched — data sovereignty preserved

## Permission Matrix

| Action | Guest | Member | Contributor | Mod | Admin | Owner |
|--------|-------|--------|-------------|-----|-------|-------|
| Chat in public channels | Yes | Yes | Yes | Yes | Yes | Yes |
| Appear in directory | No | Yes | Yes | Yes | Yes | Yes |
| Sync data to server | No | Yes | Yes | Yes | Yes | Yes |
| Create tasks (public projects) | No | No | Yes | Yes | Yes | Yes |
| Pin/delete messages | No | No | No | Yes | Yes | Yes |
| Manage channels | No | No | No | Yes | Yes | Yes |
| Promote to contributor/mod | No | No | No | No | Yes | Yes |
| Promote to admin | No | No | No | No | No | Yes |
| Server config (name, federation) | No | No | No | No | No | Yes |

## Federation

- Your Ed25519 identity works on any server — connect to Server B with the same keys
- You can be a **member** of multiple servers but designate one as your **home server** for sync
- Federated servers bridge messages across channels (existing federation design)
- Member directories are per-server; federated servers don't merge directories
- Server owner controls federation whitelist

## Implementation Order

1. **`server_members` table** — add to `server/src/storage/` as new module
2. **`/api/join`, `/api/leave`, `/api/members`** — add to `api.rs`
3. **WebSocket handlers** — add `server_join`/`server_leave` to `msg_handlers.rs`
4. **Role checks** — gate task creation, moderation actions on role
5. **Settings UI** — "My Server" field in settings page
6. **Directory UI** — member list page/panel
7. **`/api/server-info` extension** — add member count, owner profile
