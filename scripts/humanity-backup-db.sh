#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────
# HumanityOS relay SQLite hot backup (online .backup; relay stays up)
# ─────────────────────────────────────────────────────────────────────
# Installed on the VPS at /usr/local/bin/humanity-backup-db, called
# every 30 min by humanity-backup-db.timer (systemd).
#
# WHY THIS EXISTS:
#   `cp relay.db backup.db` while the relay is writing produces a
#   corrupt copy (WAL torn). sqlite3 .backup does an online consistent
#   backup with no relay downtime. Pages are streamed in DB-page order
#   under a read lock; the relay's writes can continue against the
#   live file. This is the only safe live-backup primitive SQLite has.
#
# 2026-05-21 INCIDENT — DON'T REPEAT:
#   The original on-VPS copy of this script pointed at a PRE-v0.90.0
#   path (`/opt/Humanity/crates/humanity-relay/data/relay.db`) that
#   didn't exist anymore. sqlite3 silently created an empty DB there
#   on first .backup invocation and faithfully backed up THAT empty
#   DB every 30 minutes for over a month. The operator believed they
#   had backups; they had snapshots of a fossilized April 8 state
#   that did not include the PQ cutover, the Inc6 wipe, or any chat
#   history since. **Fossil pre-v0.90 backups were moved aside to
#   `/opt/Humanity/backups/fossil-pre-v0.90/` for historical safekeeping
#   but they are NOT current and must not be restored from.**
#
# Lesson: the script is now in-repo at scripts/humanity-backup-db.sh.
# Drift between deployed copy and source-of-truth is exactly how the
# above happened. Future operator: if you change the deployed copy on
# the VPS, also update this file + commit; if you change this file,
# also push to VPS via `cp scripts/humanity-backup-db.sh
# /usr/local/bin/humanity-backup-db`.

set -euo pipefail

# Path to the live SQLite. SINGLE source of truth — if the layout
# changes (again), update HERE and verify against the actual on-disk
# file. The 2026-05-21 incident's root failure was this path going
# stale silently.
DB_PATH="/opt/Humanity/data/relay.db"

# Where backups land. Disk-guard's HUMANITY_DB_BACKUP_KEEP env (default
# OFF) controls retention; this script keeps the last 15 regardless,
# matching the original behaviour.
BACKUP_DIR="/opt/Humanity/backups"

# UTC timestamps so backups from different timezones still sort
# chronologically.
TS="$(date -u +%Y%m%d-%H%M%S)"
OUT="$BACKUP_DIR/relay-$TS.db"

# Refuse if the source DB doesn't exist. Catches the 2026-05-21 silent-
# auto-create-empty-file failure mode at the source. sqlite3 .backup
# would otherwise create the directory + an empty source DB and back
# up THAT — exactly the bug we're guarding against.
if [ ! -f "$DB_PATH" ]; then
  echo "humanity-backup-db: SOURCE DB MISSING at $DB_PATH" >&2
  echo "humanity-backup-db: refusing to back up an empty placeholder DB" >&2
  exit 2
fi

# Sanity check: source DB is non-trivial. A very small DB suggests the
# fresh-schema bootstrap on a never-used relay; backups of that are
# fine, but it's worth a notice in the log so the operator sees if
# something's reset itself without their knowledge.
SRC_SIZE_BYTES="$(stat -c%s "$DB_PATH" 2>/dev/null || echo 0)"
if [ "$SRC_SIZE_BYTES" -lt 16384 ]; then
  echo "humanity-backup-db: NOTICE — source DB is only ${SRC_SIZE_BYTES} bytes (fresh schema?)" >&2
fi

mkdir -p "$BACKUP_DIR"

# .backup is the SQLite online-backup primitive. It opens a separate
# read connection, walks DB pages, and writes a coherent snapshot
# even while the source is being written. Crash-safe.
sqlite3 "$DB_PATH" ".backup '$OUT'"

# Restrict mode on the backup — same secret-handling posture as the
# live DB. The relay runs as user `humanity`; backups should be readable
# only by that user (or root).
chmod 640 "$OUT"
chown humanity:humanity "$OUT" || true

# Rotate: keep newest 15. Independent of the disk-guard's
# HUMANITY_DB_BACKUP_KEEP (which is OFF by default and would otherwise
# never rotate); we always cap at 15 here so backups/ doesn't grow
# unbounded if the disk-guard is disabled.
ls -1t "$BACKUP_DIR"/relay-*.db 2>/dev/null | tail -n +16 | xargs -r rm -f

# Emit a parseable line for journalctl. The .timer captures stdout, so
# this becomes a discoverable per-run audit trail.
printf 'backup_created=%s size=%s\n' "$OUT" "$(stat -c%s "$OUT")"
