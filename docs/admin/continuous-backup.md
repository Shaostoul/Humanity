# Continuous backup

> Built 2026-10-22 (PRIORITIES Tier 2 item 12). Code: `src/relay/backup_stream/`. In the app:
> Server Settings > ADMIN > Backups (desktop) and the Backups panel on the admin page (web), both
> through `POST /api/admin/backups`. Every action is in `data/admin/ops_registry.json`.

A relay keeps everything in one SQLite database. The older layers of backup (a sealed snapshot
every 30 minutes on the VPS, every 6 hours inside the relay, a pull to the operator's PC) can lose
up to half an hour of messages, sign-ups and settings if the disk dies between two of them.
Continuous backup closes that gap: **every change reaches another place within seconds, sealed
with the server's backup key, so a server loses at most about a minute of data wherever its disk
dies**, and any moment inside the history you keep can be restored.

## How it works, and why this way

SQLite runs in WAL mode: every committed change is first written to `relay.db-wal` as whole pages
("frames"), and copied into `relay.db` later by a checkpoint. The relay reads those frames
straight from the log every 10 seconds, seals them, and sends them in order to the destination.
Replaying the frames onto a snapshot rebuilds the database page for page.

This is the method Litestream made standard. It is built into the relay, in Rust, rather than run
as Litestream beside it, for four reasons:

- **Nothing else to install, configure or keep running.** A separate replication program is one
  more service per server, with its own configuration file outside the app, and a self-hoster on
  a desktop node would never have it.
- **The relay owns its checkpoints.** Inside the relay the one rule that makes WAL shipping safe,
  a frame must never leave the log before it is copied, holds exactly: SQLite's automatic
  checkpoint is off while the stream runs, and every checkpoint (the shipper's own, and the
  relay's privacy scrubs after a bulk delete) takes the writer lock and copies first. A program
  outside the process can only approximate this with a held read lock.
- **The app can show and control it.** Status, the restore drill and the destination editor are
  in the app (the GUI-first rule), and every action is an entry in the ops registry.
- **One key, one format, open source only.** Objects are sealed with the backup key the relay
  already has; the S3 client, the signing and the server-to-server protocol are a few hundred
  lines checked against published test vectors, no SDK and no paid service.

Alternatives weighed: full snapshots on a timer (bandwidth grows with the database, recovery point
stays at the interval); the SQLite session extension (misses tables without a primary key, needs
every write path changed); filesystem snapshots (ZFS or btrfs only, not portable); LiteFS (FUSE,
Linux only, a cluster design). WAL shipping is the one with a recovery point measured in seconds,
cost proportional to change, and no dependency on the host.

### The pieces

| Piece | File | What it does |
|---|---|---|
| Log reader | `wal.rs` | Reads `relay.db-wal` per SQLite's file format; trusts a frame only when its salts and running checksum match, and ships only up to the last commit. |
| Capturer | `capture.rs` | Turns new frames into segments (at most 4 MiB each), takes snapshots with SQLite's online backup API from a read transaction opened at the exact log position, stages everything in `data/backup-stream/pending/`. |
| Sealing | `seal.rs` | Deflate, then AES-256-GCM under a key derived (BLAKE3) from `data/backup.key`. The object's name is part of the authenticated data, so a renamed or swapped object does not open. |
| Names, restore plan, retention | `names.rs` | What is at a destination, sorted into generations, snapshots and segments; which of them a restore uses; what retention may delete. |
| Destinations | `dest.rs`, `s3.rs`, `peer.rs`, `host.rs` | A folder, an S3-compatible bucket, or another HumanityOS server. |
| The loop | `mod.rs` | Every 10 s: capture, snapshot when due, send what is staged (oldest first, stop at the first failure, back off up to 5 minutes), list and apply retention every 10 minutes, update the status, alert. |
| Restore | `restore.rs` | Rebuild a database from a destination; the drill; the command-line restore. |

**Generations.** A generation is one unbroken run of the log. It starts with a snapshot; every
segment after it continues exactly from the one before. When the shipper cannot prove the run is
unbroken (the relay was killed and its log is gone, a program other than the relay checkpointed
the file, the database was swapped while the relay was stopped), it does not guess: it starts a
new generation with a fresh full copy. A clean shutdown leaves a marker (the database file's
BLAKE3 hash after the log is folded in), so an ordinary deploy or restart continues the same
generation and sends no new full copy. A crash with the log still on disk also continues, once the
log is shown to reach the last copy (same salts, same running checksum).

**Crash safety.** Every staged and every stored object is written to a temporary name, flushed and
renamed, so it is whole or absent. The state file is written after the segment it describes, and a
segment staged just before a crash is picked up from its own header at the next start. A local
staged object is deleted only after the destination has it; one sent twice is harmless. A restore
uses a snapshot only when its manifest (sent last) and every chunk are there, applies segments only
in unbroken order, and writes the segments of a transaction only once the segment that ends it has
been read and opened, so a damaged or missing object stops a restore at the last whole transaction
before it. The restored file appears under its name only after SQLite's full integrity check passes.

**Owner mark.** The first use of a destination writes a sealed `owner` object naming this server.
A destination whose mark this server's key cannot open, or that names another server, is refused
before anything is written: two servers can never mix their backups in one place.

**Retention.** Every complete snapshot taken within the history you keep (default 72 hours), plus
the newest one before it, plus every segment after the oldest snapshot kept. Everything older is
deleted, as is a generation that never got a complete snapshot. Nothing is ever deleted while no
complete snapshot exists: the only copy is never thrown away before its replacement has arrived.

## The three kinds of destination

1. **A folder.** Another disk, or a share mounted from another machine. A folder on the same disk
   as the database does not survive that disk failing. On a systemd host the relay may only write
   where the unit's `ReadWritePaths` allows: add the folder there.
2. **An S3-compatible bucket.** Any provider, or your own Garage or MinIO. Endpoint, region,
   bucket, a folder inside it, an access key ID and secret. Requests are signed with AWS
   Signature Version 4 in a header (never a presigned address). The secret is kept in
   `backup-destination.secrets.json`, never sent back to an app, never logged.
3. **Another HumanityOS server.** Two servers that trust each other keep each other's backups.

### Server to server: who may write, and how much

- **Proving who is writing.** Each server has a *backup sending key*: a Dilithium3 key made for this
  purpose and kept in `backup-destination.secrets.json` (not the server's federation identity,
  which lives inside the database and is therefore not there when a lost server has to be
  restored). Every request carries, in headers, the key's fingerprint, the time, and a signature
  over `hos-backup-host`, the operation, the object's name, the SHA-256 of the body and the time.
  The keeping server checks the fingerprint is on its list, the time is within five minutes, the
  signature is that key's over exactly this request, and that the same request was not used
  before. A replay, a changed body, another object's name or an unlisted key is refused before a
  byte is stored.
- **Capping the space.** The keeping server's admin gives each listed server a size limit (Backups
  > Keep other servers' backups here). A copy that would pass it is refused (HTTP 507), and the
  sending server shows "no more room" and alerts. Independently, the keeping server refuses every
  other server's copies once its own disk is 90% full, so keeping someone else's backups can never
  fill the disk its own database is on (the VPS disk filled once, BUG-169).
- **What the keeping server sees.** Ciphertext sealed with the sender's own key, in
  `data/backup-host/<sender fingerprint>/`: object names, sizes and times, nothing else. It cannot
  read a single row. Stopping (Stop keeping) either keeps or deletes the copies already there.

## The operator's setup (3-2-1)

- Copy 1: each server's live database.
- Copy 2: united-humanity.us streams to public.guide, and public.guide to united-humanity.us. Each
  keeps the other's ciphertext only. Recovery point about a minute either way.
- Copy 3: the operator's PC keeps pulling a sealed snapshot from the VPS (the "HumanityOS Relay
  Backup Pull" task, `scripts/backup-relay-from-vps.ps1`, unchanged): the offline copy.
- A monthly restore drill from the PC (`scripts/backup-drill-pc.ps1`), and the in-app Restore drill.

### Deployment steps (operator)

Nothing here was done on the live servers; these are the steps, in order.

1. **Release and deploy** the version carrying continuous backup to both servers: CI deploys
   united-humanity.us from `main`; public.guide updates itself through `humanity-relay-follow`
   once the release is published. Confirm both with `curl -s https://<server>/health`.
2. **Nothing to change in nginx or systemd** on a standard install: the endpoints are under `/api/`,
   which nginx already proxies with a 6 MB body limit (objects are at most about 4 MB), and
   everything the relay writes for this is under `/opt/Humanity/data/` (`backup-stream/`,
   `backup-host/`, `backup-destination.secrets.json`, `backup-host.json`), already in the unit's
   `ReadWritePaths`. On public.guide, check its nginx site has `location /api/` proxied and
   `client_max_body_size` of at least 6m (`grep -n client_max_body_size /etc/nginx/sites-enabled/*`).
3. **Exchange sending keys.** In the desktop app connected to united-humanity.us as admin: Server
   Settings > ADMIN > Backups > Keep other servers' backups here > Copy (the backup sending key).
   Connected to public.guide as admin: same place, paste it into "Its backup sending key", name it
   `united-humanity.us`, give it space (20 GB is ample: about 3 days of history plus the full
   copies), press **Keep its backups here**. Then the same the other way: public.guide's key into
   united-humanity.us.
4. **Turn it on.** On united-humanity.us: Change where the copies go > Another HumanityOS server >
   `https://public.guide`, history 72 hours, a full copy every 24 hours, alert after 5 minutes >
   **Save** (it tests the destination first and says so). On public.guide: the same with
   `https://united-humanity.us`.
5. **Watch it reach "Up to date"** (the first full copy takes as long as the database takes to send),
   then press **Restore drill** on each server. Expect "Passed: restored ... every table matches".
6. **Keep the keys off the servers.** Copy each server's `data/backup.key` and
   `data/backup-destination.secrets.json` to the PC, one folder per server:
   ```
   mkdir %USERPROFILE%\HumanityBackups\keys\united-humanity.us
   scp humanity-vps:/opt/Humanity/data/backup.key %USERPROFILE%\HumanityBackups\keys\united-humanity.us\
   scp humanity-vps:/opt/Humanity/data/backup-destination.secrets.json %USERPROFILE%\HumanityBackups\keys\united-humanity.us\
   ```
   (and the same from public.guide into `...\keys\public.guide\`). The Backups panel shows each
   key's fingerprint; `HumanityOS --backup-drill` prints the fingerprint of the key it uses, so a
   copy can be checked against the server. These two files open the backups: keep the folder as
   private as the backups. `backup-destination.secrets.json` changes only when you change the
   destination in the app (copy it again then).
7. **The PC's monthly drill.** Run it once by hand:
   ```
   powershell -NoProfile -ExecutionPolicy Bypass -File C:\Humanity\scripts\backup-drill-pc.ps1
   ```
   It restores united-humanity.us's newest state from public.guide into a temporary file (integrity
   check, no older than an hour, rows per table), then decrypts and checks the newest snapshot the
   pull task brought to the PC, and writes everything to `%USERPROFILE%\HumanityBackups\backup-drill.log`.
   Then register the silent monthly task with the lines at the top of the script. For public.guide's
   stream (kept on united-humanity.us), a second task with
   `-KeyDir %USERPROFILE%\HumanityBackups\keys\public.guide -SkipSnapshot`.
8. **Alerts.** The relay sends "behind" and "caught up" through the same
   `data/alert-channels.secrets.json` and `scripts/humanity-alert.js` the watchdog uses. On each
   server, check the file exists with a channel enabled (ntfy is the easiest), and that `node` is
   installed (`node --version`). The alert words name no folder, bucket or address.

## Restoring

- **The drill (in the app).** Restore drill restores from the destination into a scratch file in
  `data/backup-stream/drill/`, runs SQLite's full integrity check, and compares every table with the
  live database at the same moment (the live file is only read), then deletes the scratch file.
- **A lost server (command line, no relay running).** On the new machine, with the release's
  `HumanityOS` and a `data/` folder holding copies of the old server's `backup.key` and
  `backup-destination.secrets.json`:
  ```bash
  ./HumanityOS --restore-backup data/relay.restored.db              # the newest state
  ./HumanityOS --restore-backup data/relay.restored.db --at 2026-10-22T14:00:00Z   # a moment in the history
  ```
  It prints the time it restored to and checks integrity. Then stop the relay, move the old
  `relay.db` (and its `-wal` and `-shm` files) aside, put the restored file in its place as
  `relay.db`, and start the relay. Also carry `erased-accounts.key` if it survived (SELF-HOSTING,
  "Moving the server"). The restored server keeps streaming to the same destination: it starts a new
  generation there.
- **Undoing a mistake (a bulk delete) on a running server:** restore to a file with `--at` just
  before the mistake, as above, on the server itself (`--data /opt/Humanity/data`), then swap it in
  during a restart. An in-app "restore on next restart" does not exist yet (`docs/design/in-app-ops.md`).

## What it costs

Each segment is the pages changed in those ten seconds, compressed (a message touches a few
pages: its table, its indexes, the search index), so the traffic follows how busy the server is,
not how big its database is; a quiet ten seconds sends nothing. A full copy is the database
compressed, once a day and when a new generation starts. Capture holds
the writer lock only while it reads the new frames and writes a small file; a snapshot is copied
from a read transaction, so writes go on during it.

## Files

| File | What |
|---|---|
| `data/backup.key` | The backup key (existing). Every object is sealed with a key derived from it. |
| `data/backup-destination.secrets.json` | Where the copies go, the history kept, the alert threshold, the S3 secret if any, the backup sending key's seed. Private to the relay's user. |
| `data/backup-stream/state.json` | The shipper's place in the log. |
| `data/backup-stream/pending/` | Sealed objects waiting to be sent. |
| `data/backup-stream/clean-close.json` | Left by a clean shutdown, read and removed at the next start. |
| `data/backup-host.json` | Servers whose backups this one keeps (public keys and limits). |
| `data/backup-host/<fingerprint>/` | Their sealed copies. |

## Privacy

What a person deletes leaves the live database at once, but stays in the continuous backup's
history until retention removes it (72 hours by default; shorten it in the editor for a tighter
window). This is the same residual every backup has, written up in
`docs/reference/retention_and_deletion_semantics.md`. The copies are sealed, and whoever keeps
them cannot read them.

## Not built yet

- An in-app "restore on next restart" (point in time) for a running server; today the swap is the
  attended step above.
- A per-row remove for the destination's objects, and a download of the key files from the app.
- The alert-channels editor: alerts go to the channels in `data/alert-channels.secrets.json`, edited
  over SSH (`docs/design/in-app-ops.md`).
- Automatic recovery from the continuous backup at start (`Storage::open_resilient` still restores
  the newest local snapshot when the database is corrupt).
