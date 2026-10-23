# Storage: the relay's database and the desktop's data folder

**Who reads this:** anyone touching `src/relay/storage/`, a table, a schema migration, save files,
config, or anything under the desktop's data folder. Read `docs/design/storage-architecture.md`
first for the design.

Moved from `CLAUDE.md` on 2026-10-23 with the wording unchanged. The code is the canonical schema
(`src/relay/storage/mod.rs`); the block below is a map, verified on the date it carries.

## Storage schema (key tables)

```sql
-- VERIFIED AGAINST src/relay/storage/mod.rs 2026-09-20. Columns added by the
-- ALTER migration block are marked (alter); see the BUG-046 gotcha before you
-- write an index over one.
messages       (id, msg_type, from_key, from_name, content, timestamp, signature, raw_json,
                channel_id (alter), reply_to_from (alter), reply_to_timestamp (alter), origin_server (alter))
channels       (id, name, description, created_by, created_at,
                read_only (alter), local_only (alter), position (alter), category_id (alter),
                federated (alter), voice_enabled (alter))
profiles       (name, bio, socials, avatar_url, banner_url, pronouns, location, website, streaming_url, streaming_live)
project_tasks  (id, title, description, status, priority, assignee, created_by, created_at, updated_at, labels)   -- NOT `tasks`
task_comments  (id, task_id, author_key, author_name, content, created_at)
vault_blobs    (public_key, blob, updated_at)
key_rotations  (old_key, new_key, sig_by_old, sig_by_new, rotated_at)   -- INERT: the route and handler were deleted in Inc5b; the table was left in place rather than churn positional SQL
user_uploads   (id, uploader_key, filename, url, size, mime_type, created_at)   -- NOT `uploads`
signed_profiles (public_key, name, bio, avatar_url, banner_url, socials, pronouns, location, website, timestamp, signature)
               -- the copy other servers keep (BUG-191, verified 2026-10-12): only what anyone may see, signed by the
               -- person's own app; one row per key, the latest wins; an empty name is a withdrawal; unsigned rows dropped at start
projects       (id, name, description, owner_key, visibility, color, icon, created_at)
listing_images (id, listing_id, url, position, created_at)
listing_reviews (id, listing_id, reviewer_key, rating, comment, created_at)
notification_prefs (public_key, dm_enabled, mentions_enabled, tasks_enabled, dnd_start, dnd_end)
server_members (public_key, name, role, joined_at, last_seen, hide_presence)
dm_mailbox     (id, to_key, content, received_day)  -- sealed-sender DMs; NO sender column by design
erased_accounts (fingerprint, erased_day, ttl_days)  -- BUG-135: keyed BLAKE3 of the key (secret in data/erased-accounts.key), the day and the window promised; WITHOUT ROWID; culled by storage/expiry.rs
reach_settings (public_key, kind, audience)  -- "who can reach me" (step B, storage/reach.rs + handlers/reach.rs): one audience per kind (message, call, trade); no row = the safe default; WITHOUT ROWID; exported and erased
friend_cert_revocations (issuer_fingerprint, serial, revoked_day)  -- withdrawn friendship passes (v2, 2026-10-09); keyed BLAKE3 of the issuer's key (the erased-accounts secret, domain hum/withdrawn-pass/v1); WITHOUT ROWID; kept for good, account erase included; at most 4,096 per issuer
reports_v2     (id, reporter_key, target_key, context, reason, note, evidence, evidence_signed, evidence_hash,
                report_ts, report_sig, created_at, state, decision, decision_note, decided_by, decided_at)
               -- reports the admins can check (step D, 2026-10-09, storage/reports.rs + handlers/reports.rs): keys, never names;
               -- evidence items carry `checked`; open ones kept, decided ones culled 90 days after decided_at (storage/expiry.rs);
               -- in the reporter's export; their erase blanks the reporter (and the DM evidence's `to` and signatures), the report stays.
               -- REPLACED the old name-and-reason `reports` table, which is dropped at start.
```

## Removed tables (privacy hardening, do NOT re-add)

**Removed (privacy hardening, do NOT re-add):** `direct_messages` (the DM
social graph, 2026-08-23), `listing_messages` (plaintext buyer-seller threads,
broadcast to everyone, 2026-08-23), `groups`/`group_members`/`group_messages`
(plaintext legacy groups, 2026-08-23), and `follows` (the last server-side
social graph, 2026-08-24). Marketplace contact rides sealed-sender DMs; groups
are the E2EE P2P signed-object system; following/friendship are client-side
(sealed control DMs + client-held Dilithium friendship certificates verified
statelessly). If a feature seems to need a server-side social edge, the
certificate pattern (client-held, statelessly verified) is the house answer.

## ALTER-added columns (BUG-046)

- **SQLite: never reference an ALTER-added column from the main schema batch in `src/relay/storage/mod.rs`.** The batch runs BEFORE the ALTER block, so on the LIVE database an index, trigger or view over the new column aborts it and the relay dies at startup, while every fresh-DB test passes (BUG-046 in `docs/BUGS.md`). Such indexes go AFTER the ALTER block, and any change to an existing table's shape needs a pre-migration-shape `Storage::open` test (pattern: `opens_a_pre_v0675_database_and_migrates_it` in `src/relay/storage/uploads.rs`). Enforced since 2026-10-19 by `tests/schema_alter_order_lint.rs` (in `just lints` and CI): nothing in `Storage::open` may name table.column before the ALTER that adds it (indexes, triggers, views, INSERT, UPDATE, DELETE).

## Several `impl Storage` blocks

**Multiple `impl Storage` blocks** across `src/relay/storage/*.rs` -- Rust allows this within one crate

## Moving a server: the keys that travel

- **Keys that must travel with a moved server:** `data/backup.key` and `data/erased-accounts.key`
  beside `relay.db`.

The rows for both keys, and what is lost without each, are in `docs/reference/cryptography.md`
("Backups at rest", "Erased-account fingerprint"); the admin's view is in
`docs/admin/SELF-HOSTING.md`.

## The desktop's data folder (local-first storage)

**Local-first storage** (native binary):
OS-standard data dir (`%APPDATA%\HumanityOS\` on Windows) with:
- `config.json`: preferences AND the identity vault: `encrypted_private_key` + `key_salt` (AES-256-GCM over the Ed25519 seed key, PBKDF2 600k, `src/config.rs`); the Dilithium3 chat identity and Kyber keys are re-derived from it, never stored. There is no `identity/` or `settings/` folder (checked 2026-10-19)
- `dms/`: the encrypted direct-message history (`src/net/dm_store.rs`, one `.dmstore` per conversation)
- `saves/`: game saves. Your own home is `saves/offline_home.json` (the picker lists the top level only)
- `saves/servers/<server hash>-<identity>.json`: one save per server and identity (ship homes increment 6, `src/engine/save_slot.rs`, `persistence::slot_path`): the first 16 hex of BLAKE3 of the server's address, then the first 16 hex of the public key, so no file name holds an address. Made fresh from the start chosen there; an erased account's is renamed `<name>.erased-<yyyy-mm-dd>.json`, never deleted
- `cache/`: offline messages, avatars, manifests
- `backups/saves/<slot>/`: snapshots of each game save (2026-10-03, `src/persistence.rs`): before `save_world` overwrites a save it keeps the previous version as `<slot>_<UTC time>.json`, at most one every 15 min (`SNAPSHOT_SPACING_SECS`), newest 10 kept (`SNAPSHOTS_KEPT`); every save and snapshot is written to a `.tmp` and renamed, so a crash mid-write never tears one. See and restore them in Settings > Data > Save snapshots (a restore keeps what it replaced as a snapshot and reloads the home into the running game)
