# Retention and Deletion Semantics

## Purpose
Define what "delete" means in an immutable-object and replication-capable system.

## Core constraint
If data is replicated to independent nodes, the platform cannot guarantee global deletion.
Deletion therefore means:
- removal from display by policy
- removal from indexes
- removal from centralized storage under platform control where possible
- prevention of future distribution where feasible

## Types of deletion

### Local deletion
A client may delete its local cached copy of content.
This does not affect other nodes.

### Central storage deletion
The platform may delete objects or blocks from platform-controlled storage.
This does not delete content from independent peers.

### Policy deletion (hide/quarantine)
A signed moderation action can hide or quarantine content.
Clients must not display hidden content.
Relays should not forward quarantined content where feasible.

### Cryptographic deletion
For private content, key rotation can prevent future access to content encrypted under new keys.
Previously distributed keys still allow decryption of old content.

## Retention policies
Spaces may define retention:
- retain forever
- time-limited retention (e.g., 90 days)
- message history limits per channel

Retention affects:
- what the server keeps
- what indexes keep
It does not guarantee deletion from peers.

## User expectations (must be stated)
- Public contributions may be retained even after leaving.
- Private spaces protect content via encryption and membership control.
- Replication reduces guarantees of deletion.

## Hard-delete remnants (WAL and backups)

When a message is hard-deleted, the row is removed from the live `messages` table,
but the content can briefly persist in two places. Both are readable only by the
server operator and root on the host, never by a chat client, a federated peer, or a
network attacker. DMs are unaffected either way, they are end-to-end encrypted
(Kyber768-sealed) and the server never holds their plaintext. The only content this
concerns is PUBLIC-channel messages, which were already broadcast to every connected
client at send time.

What we do to bound it (as of the 2026-06-12 security pass):
- `PRAGMA secure_delete=ON` on the writer connection zeroes the bytes of a deleted
  row in the main database file, so deleted content does not linger as free-page
  slack inside `relay.db`.
- A `wal_checkpoint(TRUNCATE)` runs after the bulk wipe paths so a wipe does not
  leave content in the `-wal` file.

The honest residual (NOT removed by code):
- Rotating backups still contain a deleted public message until they age out: the
  in-process snapshot (every 6h, keep 5), the VPS `.backup` (every 30 min, keep 15),
  and, if enabled, the Litestream replica (30-day retention). A message deleted at
  minute 5 survives in every snapshot taken before it until that snapshot rotates.
- Rows deleted BEFORE `secure_delete` was enabled, and all existing backups, still
  hold their old bytes; an operator who wants to scrub historical slack from the live
  DB can run an offline `VACUUM` during maintenance.

This is by design in a replicated system (consistent with the rest of this document):
operators who need a tighter window can lower the Litestream retention and the
backup-keep counts. This is operator-readable-by-design; note the old plaintext
`/dm` server command was removed in v0.279, so no server-mediated plaintext DM
path exists anymore.

## DM metadata and retention (sealed-sender cutover, 2026-08-23)

DMs moved from a conversation table to a sealed-sender mailbox (`dm_mailbox`).
What the server holds per DM is now exactly: `(rowid, to_key, sealed envelope,
arrival day)`. There is no sender column, no sender name, and no fine-grained
timestamp; the sender's identity travels Dilithium-signed INSIDE the ciphertext
and only the recipient can decrypt it. The legacy `direct_messages` table (which
stored from/to/timestamp in the clear and therefore constituted a subpoenable
social graph) is DROPPED by migration on first boot, with `secure_delete=ON`
zeroing the freed pages and a WAL truncate folding them out of the log.

Retention:
- Mailbox envelopes expire after `dm_mailbox_ttl_days` (server setting, default
  30, editable in Server Settings). The mailbox is a delivery window, not an
  archive.
- A user can scrub their own queue immediately ("Delete my server mailbox" in
  both clients sends `dm_purge`).
- Long-term DM history lives ONLY on the users' own devices (native: encrypted
  file under the seed-derived key; web: encrypted IndexedDB records), plus
  whatever the users themselves export.

The honest residuals:
- Rotating backups taken BEFORE the cutover still contain the old
  `direct_messages` graph until they age out; an operator wanting it gone
  sooner deletes old backups by hand.
- The relay necessarily knows, in the moment, which authenticated socket
  deposits mail (needed for abuse gates); it does not write that to the
  mailbox. A hostile operator could log it going forward; that and transport
  IP visibility are wiretap-class exposures, not database-subpoena exposures.
- Live traffic analysis (who is online when mail for X arrives) remains
  possible for an active observer; mitigating that is mixnet territory and
  out of scope for now.
