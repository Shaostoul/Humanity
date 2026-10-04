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
- The in-app "Back up now" copies (`manual-<ts>.db.enc` in the same backups folder)
  are capped by COUNT, not by age (since 2026-10-03): the server keeps the newest 10
  (`MANUAL_BACKUPS_KEPT` in `src/relay/storage/backups.rs`) and removes the oldest
  only when another press makes a new one. So a manual copy holds a deleted public
  message until ten more presses come after it, which on a server whose admin seldom
  presses the button can be months, or never. The chat confirmation for each press
  names any copy it removed. An operator who wants a particular copy gone sooner
  deletes it by hand. While a copy dated later than the current clock is in the
  folder (the host clock went back), presses remove nothing at all and say so.
- Rows deleted BEFORE `secure_delete` was enabled, and all existing backups, still
  hold their old bytes; an operator who wants to scrub historical slack from the live
  DB can run an offline `VACUUM` during maintenance.

This is by design in a replicated system (consistent with the rest of this document):
operators who need a tighter window can lower the Litestream retention and the
backup-keep counts (the manual-copy count of 10 is a fixed constant for now, not a
setting). This is operator-readable-by-design; note the old plaintext
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

## Privacy-hardening sweep (2026-08-23, same day, second pass)

The stored-data classes removed or bounded after the sealed-sender cutover:

- **Marketplace buyer-seller threads**: the `listing_messages` table (plaintext
  content with sender identity, and every new message was broadcast to ALL
  connected clients) is DROPPED by migration. Marketplace contact now opens a
  sealed-sender E2EE DM with the seller; the relay stores no marketplace
  correspondence at all.
- **Legacy relay groups**: the `groups` / `group_members` / `group_messages`
  tables (plaintext rosters and messages) are DROPPED by migration. Groups are
  exclusively the E2EE P2P signed-object system (`groups_p2p.rs`); the relay
  stores opaque ciphertext plus signed membership objects.
- **Image metadata**: every uploaded JPEG/PNG/WebP is stripped of
  EXIF/XMP/IPTC/text metadata BEFORE touching disk (GPS coordinates in a phone
  photo were a publish-your-home-address bug nobody consented to). Lossless
  segment removal; see `relay/core/strip_metadata.rs`.
- **Presence**: members can hide presence entirely (`privacy_update`):
  never shown online, `last_seen` is not merely hidden but NEVER WRITTEN
  (and scrubbed when hiding is enabled), no join/leave announcements, no
  typing signals. New members start hidden until they choose a privacy tier
  (the onboarding default is maximum privacy). The member stays listed in
  the in-server roster (masked as offline) so friends can still reach them
  and DM keys distribute; the public web directory listing remains the
  separate `privacy.directory` opt-out.
- **Database backups**: encrypted at rest. The in-process 6-hour snapshots
  are sealed AES-256-GCM (`.db.enc`); the VPS 30-minute snapshots are sealed
  via openssl (`.db.aes`). The key (`data/backup.key`) deliberately lives
  OUTSIDE the backups directory, so backup copies that travel (rsync, the
  operator's off-box pull) are ciphertext. Crash recovery decrypts
  transparently; manual restores use `scripts/decrypt-backup.sh`. Keep a copy
  of the key somewhere safe: a backup without its key is unreadable by design.
- **Web-server IP logs**: nginx rotation cut from 14 days to 2 on the VPS and
  the accumulated history purged. The live log remains (fail2ban needs it to
  ban abusers); two days is ample for that and keeps no meaningful visit
  history. (CLI-configured; tracked as GUI-first debt in in-app-ops.)
- **Account sovereignty**: any member can self-service EXPORT what this server
  stores about them and ERASE it. Both are self-service; no admin is involved.
  - EXPORT is `POST /api/account/export`, Dilithium3-signed over
    `account_export\ntimestamp`, rate limited per key, and served as a JSON
    file download. It was a WebSocket message until 2026-09-06; the relay has
    no per-connection sender, so delivering it meant broadcasting the whole
    export to EVERY connected client task and filtering it down to one, against
    a 128 KB socket message ceiling.
  - ERASE is `account_delete` with typed-name confirmation: messages, uploads
    and their files on disk, profile, mailbox, vault, push subscriptions,
    listings, reviews, tasks, reactions, codes, membership, registered name,
    your progress in the shared world (quest, XP, reputation there), and your
    home's plot on the ship (it goes to the next player; if you come back you
    get a free plot or a guest place). If you are in the shared world when you
    erase, you leave it in the same step the plot is freed, so nobody is handed
    a plot you still stand on; the server's stored copy of the world loses your
    figure in that step too, so a crash straight after cannot bring your
    progress back; and your game is told you left, with one sentence saying how
    to come back (open Chat and press Connect), and does not join again on its
    own. Every client of yours that is online at that moment leaves the server
    and does not reconnect by itself (BUG-135).
    Admins must hand off the admin role first so a server is never orphaned.
    secure_delete zeroes the freed pages and the WAL is truncated; rotating
    backups hold prior snapshots until they age out, as everywhere else here.
  - WHAT AN ERASE LEAVES BEHIND, FOR A WHILE (2026-10-04, BUG-135; the
    operator's decision, verbatim: "Let's go with option 2 that way we have a
    way to cull the list over a period of time. That way we don't end up with a
    massive log of all the accounts that erased themselves after many years or a
    malicious attack."). Before this, a device that was offline during the erase
    (a second computer with the app closed, a web tab between reconnects) came
    back later with the same key and was signed up again by itself. Now the
    server remembers that the account was erased, and only that:
    - as a ONE-WAY FINGERPRINT of the public key (BLAKE3 keyed with a 32-byte
      secret the relay creates once and keeps beside the live database, like
      `backup.key`, in `data/erased-accounts.key`), never the key itself, never
      the name; with the secret kept outside the database, a copy of the
      database alone cannot be checked against a list of known keys;
    - with the DAY of the erase and the number of days in force that day, and
      nothing else (table `erased_accounts (fingerprint, erased_day,
      ttl_days)`, WITHOUT ROWID, so not even the order of the erases within a
      day is kept);
    - for UP TO that many days: an entry is matched while it is younger than
      the smaller of its own number and the server's current
      `erased_accounts_ttl_days` (default 30), counted in whole UTC days, so a
      30-day entry is matched on the day of the erase and the 29 after it, and
      the number a person reads is never exceeded. The window is stored with
      the entry because it is what the person read before deciding: an admin
      who later RAISES the setting does not stretch earlier entries, while one
      who LOWERS it shortens every entry at once (review of 2026-10-04,
      finding 1). An entry dated after tomorrow (a clock that jumped forward)
      is not matched either;
    - deleted by the expiry pass as soon as it stops matching: at relay
      start, every six hours (before that pass's backup, beside the DM mailbox
      expiry), and straight after every saved change to the server settings
      (`run_expiry_sweeps`, storage/expiry.rs). So the row is on disk at most
      about six hours past its last day;
    - and never more than `erased_accounts_cap` rows (server setting, default
      100,000): when full, the oldest go first, so a flood of erases cannot grow
      it without bound.
    Both settings are in Server Settings > ADMIN > Server policy > Erased
    accounts. The person reads the real number of days before erasing (native
    Settings > Account, web Erase account) and again in the receipt:
    "After the erase this server remembers for up to 30 days that this account
    was erased, as a one-way fingerprint that is not your name or your data,
    so your other devices do not sign you up again by themselves; then the
    entry is deleted here, and a copy of it in this server's backups lasts
    until that backup is deleted."
    An app that has not received the server's number (a relay too old to have
    the setting) shows no such sentence at all rather than a promise the
    server may not keep.
    What it does: a device of the erased account that connects is told
    `account_erased` (with `partial` read from what the erase left: if it did
    not finish, the device is told to erase again, as the erasing device was)
    and the relay closes that connection; nothing is signed in (no name
    registered, no member row, no presence). The name registration and the
    member row on that path check the erase again in the same step as they
    write, and a game join checks it again while it holds the game world's
    lock, so an erase that lands in the middle of either still wins. A game
    join from a device still connected from before the erase is refused.
    Pressing Connect (native) or Enter (web) under the erase note says
    `sign_up_again` in that connection's identify, which forgets the entry
    and signs the person up again as a new account; an automatic reconnect
    never says it. Bots are never affected. The entry is in the person's own
    export (`erased_here`: the day and the number of days); it is the one row
    an erase writes instead of deleting.
    Honest residuals, what outlives the entry:
    - **Backups.** A copy of the row is in every backup of the database taken
      while it existed, until that backup is deleted: the relay's own 6-hourly
      `.db.enc` (5 kept, so about 30 hours), the VPS 30-minute `.db.aes` (15
      kept, about 7.5 hours), the operator's off-box pull (60 kept), and the
      admin's "Back up now" copies (`manual-*.db.enc`), which are kept by
      count (`MANUAL_BACKUPS_KEPT`, 10) with NO age limit: one taken while the
      entry existed keeps it until ten newer manual copies replace it or the
      admin deletes it. Every one of them is sealed with `backup.key`, and
      none holds the fingerprint secret, so a backup on its own cannot be
      checked against a list of known keys.
    - **Server logs.** The relay logs that an erase happened (with the count
      of what went), that an erased account reconnected, that one chose to sign
      up again, and that a game join from one was refused, each with the time
      and with NO key or part of one (until 2026-10-04 these lines carried the
      first 12 hex characters of the key, enough to pick it out of a list of
      known keys). journald on the VPS, or run.log on a Host Node, keeps them
      on its own schedule, which has nothing to do with the window.
    - **The secret file.** If `data/erased-accounts.key` is lost (a server
      moved without it), the old fingerprints simply never match again and are
      culled on schedule, at the cost of the old behaviour (another device may
      sign up again). If it is there but damaged, it is never overwritten: the
      relay runs with a secret for that run only, and `/health` says
      `"erase_memory": "this_run_only"` (shown by `just brief`) until the
      operator fixes or removes the file. Moving a server: carry
      `data/relay.db`, `data/backup.key` AND `data/erased-accounts.key`.
    - **Desktop apps from before this (v0.1449.0 and older)** never send
      `sign_up_again`, so on a relay with this change an account erased from
      such an app can only come back from it once the window has passed; the
      web, served by the relay itself, always matches it.

  Two corrections to what this section said before 2026-09-06, both of which
  were live for months:

  1. The export and the erase each named two tables that DO NOT EXIST
     (`uploads` and `tasks`; the real names are `user_uploads` and
     `project_tasks`). Neither failed loudly: the export helper turns a bad
     table name into an empty array and the delete helper only logged. So the
     export reported the member had no uploads, and every uploaded FILE
     survived "erase everything" on disk and in the database while the receipt
     said otherwise. Guarded now by `tests/account_sql_lint.rs`.
  2. Erasure is deliberately NOT total, and saying "permanently" without
     saying what survives was itself misleading. Moderation records (bans,
     mutes, reports you filed) and reputation history are kept, because a
     record that exists to hold someone to account cannot be erasable by that
     person: otherwise deleting your account is a self-service unban, and
     `register_name` is a bare INSERT OR IGNORE with no ban check, so the same
     key would simply walk back in. Every one of those records IS in the
     export, so nothing is hidden from the person it concerns. The Settings
     page now states this before the button, not after.
## Privacy maximization (2026-08-24, the follow-up arc)

The remaining server-held data classes and length/transport leaks, closed:

- **Follows graph GONE.** The `follows` table was the last server-side social
  graph; it is DROPPED by migration. Following is now sealed client-to-client
  control messages (`[[hum:follow]]`/`[[hum:unfollow]]` over the DM mailbox),
  and each client keeps its own following/followers sets in its local
  encrypted store. Friendship is a client-held Dilithium CERTIFICATE (the
  recipient authorizes the sender) that the relay verifies statelessly at
  dm_put — no friends table exists. A subpoena of the server yields NOTHING
  about who follows or is friends with whom. Strangers without a certificate
  can still "knock" (sealed DMs, capped at 20 per sender per day) so cold
  outreach works without enabling floods.
- **DM length no longer leaks.** Sealed DM plaintext is padded up to size
  buckets (256 / 1024 / 4096 / 16384 bytes) before encryption, so ciphertext
  length no longer distinguishes "ok" from a paragraph.
- **Message retention (server setting).** `message_retention_days` (default 0
  = keep forever) auto-expires public channel messages past the window;
  pinned messages are always kept. Bounds how long even public history
  lingers, on the same maintenance sweep as the DM-mailbox TTL.
- **Federation gossip respects unlisted.** A user who opts out of the public
  directory (Private/Balanced tiers) no longer has their profile replicated
  across federated servers — the gossip + signed-profile cache are gated on
  the directory choice, and going unlisted retracts the local replicated
  copy. Listed users (Open/Spotlight) gossip as before. Note that gossip only
  ever carried user-authored PUBLIC profile fields (name/bio/avatar/socials/
  location/website), never presence, IP, or DM metadata.
- **Transport IP privacy (opt-in).** An optional Tor v3 onion service
  (`scripts/tor-onion-setup.sh`, `docs/admin/tor-onion-service.md`) lets users
  reach the relay without revealing their IP at all — the honest answer to the
  wiretap-class exposure this document has flagged throughout. Additive; the
  clearnet endpoint is unchanged.

- **Encrypted DM attachments (2026-08-24).** A file or photo shared in a DM is
  now encrypted client-side (fresh AES-256-GCM key per file) BEFORE upload; the
  server stores only opaque ciphertext at a public URL (harmless without the
  key), and the key + nonce + metadata ride inside the sealed DM envelope as a
  `[[hum:file:v1]]` marker. The recipient decrypts the envelope, fetches the
  ciphertext, and decrypts it locally. This closes the gap where a "private" DM
  photo used to sit as readable bytes at a public URL, exposed to the operator,
  to anyone who obtained the URL, and to EXIF-style leakage. Public-channel
  uploads are unchanged (public is public). Web renders encrypted images inline;
  native encrypts on send and shows a labeled card on receive (full native
  inline decrypt is a tracked follow-up). Relay side: `?encrypted=1` upload
  mode stores an inert `.enc` blob, skipping format/EXIF handling on ciphertext.

Remaining honest limits after this arc: certificates in v1 don't expire and
can't be server-side revoked (unfriending is client-side); live traffic
analysis by an active wire observer on the clearnet endpoint is still a
mixnet problem the onion service sidesteps but doesn't universally solve;
group-chat attachments are not yet encrypted the way DM attachments are (a
follow-up once the pattern is proven on DMs).
