# Roles System (data-driven custom roles + per-role capabilities)

**Status:** Implementing (2026-05-15)
**Affects:** `src/relay/storage/roles.rs` (new), `src/relay/storage/server_settings.rs`,
`src/relay/relay.rs`, `src/relay/api.rs`, `src/gui/pages/chat.rs`
(user modal), `src/gui/pages/server_settings.rs`, `src/lib.rs` (WS parse)
**Supersedes:** the hardcoded 5-string role ladder
(`"" < verified < donor < mod < admin/owner`).

---

## 1. Why

Operator (2026-05-15): *"We also want to include additional ranks ...
Someone might have a server where they allow certain people to
livestream to their relay. Like my lil bro, big bro, dad, etc. should
be able to stream to our vps relay."*

The role system was a fixed ladder of magic strings checked with
`match role { "admin" => ... }` scattered across the relay. There was
no way to (a) define a new role, (b) grant a *capability* (like
streaming) to a subset of users without promoting them to mod/admin,
or (c) restyle role badges. This doc specifies a data-driven roles
model where roles are rows the operator can create/edit, each carrying
a capability set, and the 4 historical ranks become seed data.

Per the project **infinite-of-x** rule: anything that can exist more
than once is data, not code. Roles qualify.

---

## 2. Data model

New singleton-per-server table `roles` (one row per role):

```sql
CREATE TABLE roles (
    id          TEXT PRIMARY KEY,   -- stable key: "verified","family",...
    label       TEXT NOT NULL,      -- display name: "Verified","Family"
    color       TEXT NOT NULL,      -- badge color hex, e.g. "#4FC3F7"
    trust_level INTEGER NOT NULL,   -- ordering; higher = more trusted
    built_in    INTEGER NOT NULL,   -- 1 = seed role, cannot be deleted
    can_stream  INTEGER NOT NULL,   -- may start a livestream
    can_upload  INTEGER NOT NULL,   -- may upload files/images (legacy general)
    can_voice   INTEGER NOT NULL,   -- may create/use voice channels
    can_image_share INTEGER NOT NULL DEFAULT 1, -- v0.261: per-role image attach
    can_file_share  INTEGER NOT NULL DEFAULT 1, -- v0.261: per-role non-image file
    base_tier   TEXT NOT NULL,      -- which server_settings limit tier
                                    -- this role inherits (one of
                                    -- unverified/verified/mod/admin)
    sort_order  INTEGER NOT NULL DEFAULT 0
);
```

### Why `base_tier` instead of moving limits into this table

The per-role numeric **limits** (max_chars, max_upload_mb,
max_uploads_kept) already live as 4-column tiers on `server_settings`
(v0.201 + v0.238). Re-homing them into `roles` is a large migration
with no functional gain for the operator's stated need. Instead each
role declares which of the four existing limit tiers it inherits via
`base_tier`. A custom "Family" role can inherit `verified` limits while
also having `can_stream = 1`. The four built-ins set
`base_tier = <self>`.

**v0.261 — per-role image/file sharing.** `image_sharing_enabled` /
`file_sharing_enabled` were server-wide booleans (and, until v0.261,
never actually enforced server-side — only the chat UI hid the button).
Now `roles.can_image_share` / `can_file_share` give the same
master∧capability model as streaming: an upload is allowed iff
`server_settings.<x>_sharing_enabled AND role.can_<x>_share`, enforced
in `api.rs::upload_file`. Migration seeds every existing/built-in role
to `1` so the upgrade is non-breaking (sharing stays gated only by the
server master exactly as before; the per-role denial is opt-in).

> **R4 — BUILT (v0.262).** `roles` now owns `max_chars`,
> `max_upload_mb`, `max_uploads_kept` per-role. The "Per-role limits"
> matrix is gone; everything per-role lives in ONE cohesive Roles table
> (caps + numeric limits, all editable incl. built-ins). Enforcement
> reads `role_def(role).max_*` directly — no `server_settings` tier hop
> (`relay.rs` chat length, `api.rs` upload size + FIFO). This also
> closed a latent gap: per-tier `max_upload_mb` was never enforced
> server-side before R4 (only a hard const). Non-breaking upgrade: the
> migration backfills every existing role from the live
> `server_settings` value of its old `base_tier`, so effective numbers
> are identical post-upgrade. `base_tier` is retained ONLY as the
> migration source + the add-form "Prefill…" preset convenience; it is
> no longer a runtime indirection. The legacy
> `server_settings.max_*_{tier}` columns remain as inert back-compat
> shadows (still accepted by `server_settings_update`, unused by
> enforcement).

### Assignment

No schema change. The existing `user_roles (public_key, role)` table
already stores an arbitrary role string. Assigning the "family" role
to dad = `set_role(dad_key, "family")`. Unknown/legacy role strings
resolve to the implicit base role (empty string → "unverified"
behavior), so nothing breaks if a role is deleted while assigned.

---

## 3. Seed roles (built-in, `built_in = 1`, cannot be deleted)

| id | label | trust | can_stream | can_upload | can_voice | base_tier |
|----|-------|-------|-----------|-----------|----------|-----------|
| `unverified` | Unverified | 0 | no | no  | no  | unverified |
| `verified`   | Verified   | 1 | no | yes | yes | verified   |
| `mod`        | Moderator  | 3 | yes | yes | yes | mod        |
| `admin`      | Admin      | 4 | yes | yes | yes | admin      |

`donor` (trust 2, = verified caps) is also seeded for backward compat
with existing `user_roles` rows that say `"donor"`.

Streaming defaults **off** for everyone except mod/admin — the operator
opts a role in (e.g. creates a "Streamer" or "Family" role with
`can_stream = 1`). The server-wide `video_streaming_enabled` bool is
retained as a **master kill-switch**: if it's off, nobody streams
regardless of role; if on, the per-role `can_stream` decides.

Same kill-switch composition for upload (`image_sharing_enabled` /
`file_sharing_enabled`) and voice (`voice_channels_enabled`):

```
effective_can_X(user) = server_master_X_enabled
                        && role_of(user).can_X
```

---

## 4. Capability check API

`Storage::role_def(role_id) -> RoleDef` (falls back to the `unverified`
seed for unknown ids — safe default-deny). `RoleDef` has the bool
capabilities + `base_tier`. The scattered `match role { ... }` checks
become:

```rust
let rd = state.db.role_def(&role).unwrap_or_default();   // default-deny
let may_stream = settings.video_streaming_enabled && rd.can_stream;
```

`ServerSettings::max_chars_for_role` etc. take the role id, look up
`role_def(id).base_tier`, then index the existing tier columns. So a
custom role's *limits* follow its `base_tier` while its *capabilities*
are its own.

---

## 5. WS protocol

- `role_list` (server → client, on connect + after any role change):
  the full `Vec<RoleDef>` so clients can render badges + populate the
  assignment dropdown.
- `role_upsert` (admin → server): create or edit a role. Server
  validates admin, protects `built_in` ids from id/trust changes
  (capabilities still editable on built-ins), broadcasts new `role_list`.
- `role_delete` (admin → server): delete a custom role. Built-ins
  rejected. Any users holding the deleted role fall back to unverified
  behavior (no DB rewrite needed — `role_def` fallback handles it).
- `set_user_role` (admin → server): assign a role id to a user. This
  generalizes the existing `mod_action` "mod"/"unmod" actions (those
  remain as shortcuts).

---

## 6. UI

- **User-profile modal** (operator's chosen assignment path): admins
  see a role dropdown listing every `role_list` entry; selecting one
  sends `set_user_role`. Replaces/augments the mod/unmod buttons.
- **Server Settings → Roles**: ONE cohesive table. The header is
  followed by a single **"Server master"** row (accent-colored, no
  swatch) carrying the 4 server-wide kill-switches in the
  Stream/Voice/Image/File columns (`Upload` is a dash — legacy general
  `can_upload` has no server-wide master); every row below is a role
  (add custom, edit label/color/capabilities + per-role numeric limits,
  delete custom). **v0.262.6:** the kill-switches were previously a
  detached "Sharing & policy toggles" checkbox group in *Server policy*;
  the operator read that as a *duplicate* of the per-role
  Stream/Voice/Image/File columns. They are not duplicates (effective =
  master ∧ role) but two detached checkbox groups looked redundant, so
  they were folded into this table's top row. Both the Server-policy
  "Save Changes" button and the master row's own Save send the *same*
  `server_settings_update` payload via the shared
  `send_server_settings_update` helper (one builder, can't drift). Only
  the genuinely-global `require_pq_signatures` gate (no per-role column)
  remains in *Server policy*.
- **Badges**: role badge color comes from `RoleDef.color` instead of
  the hardcoded badge palette. Custom roles get a badge automatically.

---

## 7. Phasing

| Phase | Scope | Status |
|-------|-------|--------|
| R1 | `roles` table + seed + storage CRUD + `role_def` + capability composition (streaming/upload/voice gates read it) + WS `role_list`/`role_upsert`/`role_delete`/`set_user_role` | building (v0.239) |
| R2 | User-modal role dropdown (assignment) + client `role_list` parse + badge color from RoleDef | next (v0.240) |
| R3 | Server Settings → Roles management section (add/edit/delete custom roles, capability toggles) | after R2 |
| R4 | Consolidate per-role limits into `roles` (deprecate `base_tier`) | when per-custom-role limits are actually wanted |

Defaults at every phase preserve current behavior: the 4 seed roles
keep their historical limits (via `base_tier`) and only mod/admin can
stream until the operator creates/edits a role to opt in.
