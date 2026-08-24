# 2026-08-23, part 2: the privacy-hardening sweep + privacy tiers

Same day as the sealed-sender DM cutover. The operator asked "is there
anything else we can do to protect users?", got an audit of every remaining
stored-data class, and said: do all of it, plus let users choose exactly how
private to be at onboarding, defaulting to maximum privacy, with a maximum-
publicity tier for streamers who want to be found. All of it shipped.

## Data classes removed outright

**Marketplace threads.** `listing_messages` stored buyer-seller conversations
in plaintext with sender identity, and, worse, broadcast every new message to
every connected client. The table is dropped by migration and the protocol
deleted; "Message Seller" now opens a sealed-sender E2EE DM prefilled with the
listing reference. The relay stores no marketplace correspondence at all.

**Legacy groups.** The relay-mediated group system (plaintext rosters and
messages in `groups`/`group_members`/`group_messages`) is fully retired:
protocol variants, handlers, storage, tables (dropped by migration), the
native sidebar/world-panel rendering, and the web handlers and slash commands.
Groups are exclusively the E2EE P2P signed-object system now. The group voice
special-case in the voice-room join gate went with it.

**Image metadata.** Every uploaded JPEG, PNG, and WebP is stripped of
EXIF/XMP/IPTC/text metadata before the bytes touch disk, losslessly (whole
segments removed, no re-encode; GIF passes through, it has no standardized
location metadata). Tests prove the GPS bytes are gone AND the image still
decodes. A member photographing their garden no longer publishes their home
coordinates.

## Data bounded or sealed

**Presence.** A server-enforced `privacy_update` flag: hidden members never
appear online, are excluded from the live peer list, produce no join/leave
announcements and no typing signals, and their `last_seen` is not merely
hidden but never written (and scrubbed when hiding is enabled — the gate is in
the SQL so every call site inherits it). New members join hidden: fail-private
until they choose. Hidden members stay in the roster masked as offline so
friends can reach them and DM keys distribute. The old Settings toggle "Show
Online Status" — persisted since v0.1066 and read by nothing — is now the real
switch.

**Backups.** Both backup layers are encrypted at rest: the in-process 6-hour
snapshots with AES-256-GCM (`.db.enc`), the VPS 30-minute script snapshots via
openssl (`.db.aes`). The key lives at `data/backup.key`, deliberately outside
the backups directory, created at relay boot; backup copies that travel are
ciphertext. Crash recovery decrypts transparently, now scans BOTH backup
directories (the in-process one was never consulted before — its underscore
filenames failed the old filter, a latent recovery gap found in passing), and
never leaves a decrypted scratch file behind. `scripts/decrypt-backup.sh`
covers manual restores. Operator note: keep a copy of the key somewhere safe;
a backup without its key is unreadable on purpose.

**IP logs.** nginx rotation on the VPS cut from 14 days to 2, the accumulated
history purged live. fail2ban keeps the live log it needs to ban abusers.

## The privacy tiers (the operator's onboarding feature)

`data/gui/privacy_tiers.json` — one data file, both clients (infinite-of-x):

- **Private** — maximum privacy, THE DEFAULT: presence hidden, unlisted in
  the public directory.
- **Balanced** — visible to your server, out of the public web directory.
- **Open** — findable: presence on, directory listed.
- **Spotlight** — maximum publicity by explicit choice, for streamers and
  creators: everything discoverable, live-stream status promoted.

A tier is a preset over two real switches (presence, directory listing), each
individually overridable in Settings → Privacy. First connect shows a chooser
(native: global modal over any page; web: overlay) with Private preselected;
the choice persists and is re-asserted per server. Until a choice lands, new
accounts are presence-hidden — a client that never implements the picker
fail-privates rather than fail-exposes. On web, applying a tier walks an
unwrapped seed straight into the existing passphrase-protection flow.

## Account sovereignty

Self-service export and erasure, no admin involved: `account_export` returns
everything the server stores about you (messages authored, profile, follows
both directions, uploads, listings, reviews, tasks, vault size, queued sealed
mail count) as a JSON download (web) or a local file (native, under
`%APPDATA%/HumanityOS/exports/`). `account_delete` takes your exact display
name as typed confirmation and erases all of it including upload files on
disk, with a per-table receipt; admins must hand the role off first so a
server can't be orphaned by a typo. Buttons live in native Settings → Account
and the web identity block.

Also: a web "relay my calls" toggle (fail-closed TURN-only mode so callers
can't learn your IP; native equivalent waits on the str0m TURN client).

## Verification

Storage suite 157/157 plus new: metadata-stripper integrity (decode after
strip), sealed-backup crash recovery, presence defaults + last_seen gate,
export-then-erase-leaves-no-trace, account handlers, features gate updated
(privacy/account types classified always-on — a feature toggle must never be
able to switch off someone's privacy controls). All five standalone GUI lints
green; the settings-persistence lint was extended to follow the presence
toggle to its new home rather than deleted.
