#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────
# HumanityOS — full-PQ cutover: clean fresh-schema identity wipe
# ─────────────────────────────────────────────────────────────────────
# Operator (2026-05-18): "assume all accounts broken, fresh slate,
# screw backwards compat ... if we lost everything data-wise it
# wouldn't be a bother ... we could just wipe it if that'd be easier."
#
# It IS easier: the simplest, zero-migration "full wipe" is to back up
# then delete relay.db and let `Storage::open` recreate the schema
# fresh on restart. No selective table surgery, no migration code —
# maximally trims cruft, exactly the cutover's spirit. Old & new users
# just re-onboard from their seed (trivial: seed phrase -> done).
#
# SAFE BY CONSTRUCTION:
#  - Refuses to run without the explicit `--yes` arg (no accidental
#    fire; the cutover must be a deliberate, attended step).
#  - Always takes a timestamped backup into backups/ FIRST (the disk
#    janitor rotates those), so the wipe is reversible.
#  - Idempotent; verifies the relay comes back healthy on a fresh DB.
#
# Run ON the VPS:  bash scripts/pq-wipe.sh --yes
# Or from dev:     just pq-wipe         (prompts, then ssh + --yes)
# DO NOT run until the full-PQ stack is shipped + security-reviewed.
# ─────────────────────────────────────────────────────────────────────
set -euo pipefail

REPO="/opt/Humanity"
DB="$REPO/data/relay.db"
BACKUPS="$REPO/backups"
UNIT="humanity-relay"

if [ "${1:-}" != "--yes" ]; then
  echo "REFUSING: this WIPES all identity/account/message data on the"
  echo "live relay (fresh schema; everyone re-onboards from seed)."
  echo "Re-run with --yes once the full-PQ stack is shipped + reviewed:"
  echo "    bash scripts/pq-wipe.sh --yes"
  exit 2
fi

ts="$(date +%Y%m%d-%H%M%S)"
mkdir -p "$BACKUPS"

echo "[pq-wipe] stopping $UNIT ..."
systemctl stop "$UNIT" || true

if [ -f "$DB" ]; then
  echo "[pq-wipe] backing up $DB -> $BACKUPS/relay-PREWIPE-$ts.db"
  cp -f "$DB" "$BACKUPS/relay-PREWIPE-$ts.db"
  echo "[pq-wipe] deleting db (+ wal/shm) for a clean fresh schema"
  rm -f "$DB" "$DB-wal" "$DB-shm"
else
  echo "[pq-wipe] no $DB present — nothing to back up; fresh start anyway"
fi

echo "[pq-wipe] starting $UNIT once so Storage::open builds the fresh schema ..."
systemctl start "$UNIT"
sleep 4

# ── Re-seed #announcements (the only history the operator wants kept) ──
# data/announcements_archive.json is the durable pre-wipe export. We
# insert OFFLINE (relay stopped) so there is no WAL race, then restart.
# If the archive or node is missing we DO NOT fail the wipe — a fresh
# relay with no announcement history is still fully functional.
SEED_JS="$REPO/scripts/seed-announcements.js"
ARCHIVE="$REPO/data/announcements_archive.json"
if [ -f "$SEED_JS" ] && [ -f "$ARCHIVE" ] && command -v node >/dev/null 2>&1 && command -v sqlite3 >/dev/null 2>&1; then
  echo "[pq-wipe] stopping $UNIT to re-seed #announcements offline ..."
  systemctl stop "$UNIT" || true
  sleep 1
  if node "$SEED_JS" | sqlite3 "$DB"; then
    seeded="$(sqlite3 "$DB" "SELECT COUNT(*) FROM messages WHERE channel_id='announcements';" 2>/dev/null || echo '?')"
    echo "[pq-wipe] re-seeded #announcements: $seeded messages restored"
  else
    echo "[pq-wipe] WARN: announcements re-seed failed — continuing with empty channel"
  fi
  echo "[pq-wipe] restarting $UNIT on the seeded DB ..."
  systemctl start "$UNIT"
  sleep 3
else
  echo "[pq-wipe] skip re-seed (archive/node/sqlite3 absent) — fresh empty relay"
fi

state="$(systemctl is-active "$UNIT" 2>/dev/null || true)"
echo "[pq-wipe] $UNIT is now: $state"
code="$(curl -s -o /dev/null -w '%{http_code}' --max-time 10 http://localhost:3210/health 2>/dev/null || echo '?')"
echo "[pq-wipe] local /health -> $code"
echo "[pq-wipe] DONE. Pre-wipe backup: $BACKUPS/relay-PREWIPE-$ts.db"
echo "[pq-wipe] Restore (if needed): stop $UNIT; cp that file to $DB; start $UNIT"
