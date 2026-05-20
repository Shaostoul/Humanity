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
#  - Refuses to run while `cargo build --release --features relay` is
#    in progress (HARDENING: 2026-05-19 we lost this race — CI's
#    disk-guard had rm'd target/ mid-rebuild; the wipe started before
#    the binary was back; the new schema never built; the offline seed
#    ran against an empty DB; 888 sqlite "no such table" errors; relay
#    crash-looped. The recovery was manual.)
#  - Refuses to run if the relay binary is missing/not executable
#    (same root cause as above).
#  - Always takes a timestamped backup into backups/ FIRST (the disk
#    janitor rotates those), so the wipe is reversible.
#  - POLLS for schema readiness up to 30s after relay start (previously
#    a bare `sleep 4` — too short on cold start) before running the
#    offline #announcements re-seed.
#  - Verifies the re-seed actually populated rows (count > 0 and
#    matches the archive length) before declaring success.
#
# Run ON the VPS:  bash scripts/pq-wipe.sh --yes
# Or from dev:     just pq-wipe         (prompts, then ssh + --yes)
# ─────────────────────────────────────────────────────────────────────
set -euo pipefail

REPO="/opt/Humanity"
DB="$REPO/data/relay.db"
BACKUPS="$REPO/backups"
UNIT="humanity-relay"
BINARY="$REPO/target/release/HumanityOS"
SEED_JS="$REPO/scripts/seed-announcements.js"
ARCHIVE="$REPO/data/announcements_archive.json"

# ── Gate 1: explicit confirmation ──
if [ "${1:-}" != "--yes" ]; then
  echo "REFUSING: this WIPES all identity/account/message data on the"
  echo "live relay (fresh schema; everyone re-onboards from seed)."
  echo "Re-run with --yes:"
  echo "    bash scripts/pq-wipe.sh --yes"
  exit 2
fi

# ── Gate 2: no concurrent cargo build ──
# 2026-05-19 incident: disk-guard rm -rf target/ inside a CI deploy
# coincided with a manual pq-wipe. The relay binary was missing when
# pq-wipe stopped/started the service → no fresh schema built → the
# offline seed step ran against an empty DB → 888 "no such table"
# errors → relay crash-loop. Refuse if a build is in flight.
if pgrep -f 'cargo build .* --features relay' >/dev/null 2>&1; then
  echo "REFUSING: a 'cargo build --features relay' is currently running."
  echo "Wait for it to finish, then re-run pq-wipe. Active processes:"
  pgrep -af 'cargo|rustc' 2>/dev/null | head -3
  exit 3
fi

# ── Gate 3: relay binary must exist ──
if [ ! -x "$BINARY" ]; then
  echo "REFUSING: relay binary missing or not executable: $BINARY"
  echo "Build first:"
  echo "    cargo build --release --features relay --no-default-features"
  exit 4
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

echo "[pq-wipe] starting $UNIT so Storage::open builds the fresh schema ..."
systemctl start "$UNIT"

# ── Schema-readiness poll ──
# Previously a bare `sleep 4` — insufficient on cold start. Poll up to
# 30s for the `messages` table to exist; abort with diagnostic if not.
SCHEMA_TIMEOUT=30
echo "[pq-wipe] waiting up to ${SCHEMA_TIMEOUT}s for schema to build ..."
schema_ready=0
for i in $(seq 1 $SCHEMA_TIMEOUT); do
  if sqlite3 "$DB" "SELECT 1 FROM messages LIMIT 0;" >/dev/null 2>&1 \
     && sqlite3 "$DB" "SELECT 1 FROM channels LIMIT 0;" >/dev/null 2>&1; then
    schema_ready=1
    echo "[pq-wipe] schema built after ${i}s"
    break
  fi
  sleep 1
done

if [ "$schema_ready" -ne 1 ]; then
  echo "[pq-wipe] ERROR: schema not built within ${SCHEMA_TIMEOUT}s — relay failed to start"
  echo "[pq-wipe] systemd is-active: $(systemctl is-active "$UNIT" 2>&1 || true)"
  echo "[pq-wipe] last journal lines:"
  journalctl -u "$UNIT" --no-pager -n 8 2>&1 | tail -8
  echo "[pq-wipe] pre-wipe backup is intact at: $BACKUPS/relay-PREWIPE-$ts.db"
  echo "[pq-wipe] Restore: systemctl stop $UNIT; cp '$BACKUPS/relay-PREWIPE-$ts.db' '$DB'; systemctl start $UNIT"
  exit 5
fi

# ── Re-seed #announcements (the only history the operator wants kept) ──
# data/announcements_archive.json is the durable pre-wipe export. Insert
# OFFLINE (relay stopped) so there's no WAL race, then restart.
if [ -f "$SEED_JS" ] && [ -f "$ARCHIVE" ] && command -v node >/dev/null 2>&1 && command -v sqlite3 >/dev/null 2>&1; then
  echo "[pq-wipe] stopping $UNIT to re-seed #announcements offline ..."
  systemctl stop "$UNIT" || true
  sleep 1
  seed_log="$(mktemp)"
  if node "$SEED_JS" 2>/dev/null | sqlite3 "$DB" 2>&1 | tee "$seed_log" >/dev/null; then
    : # sqlite returned 0
  fi
  err_lines="$(grep -c '^Error' "$seed_log" 2>/dev/null || echo 0)"
  if [ "$err_lines" -gt 0 ]; then
    echo "[pq-wipe] WARN: re-seed produced $err_lines sqlite errors. First few:"
    grep '^Error' "$seed_log" | head -3
  fi
  rm -f "$seed_log"

  # Verify announcements actually populated.
  seeded="$(sqlite3 "$DB" "SELECT COUNT(*) FROM messages WHERE channel_id='announcements';" 2>/dev/null || echo 0)"
  expected="$(node -e "console.log(require('$ARCHIVE').length)" 2>/dev/null || echo '?')"
  if [ "$seeded" -ge 1 ] && [ "$seeded" = "$expected" ]; then
    echo "[pq-wipe] re-seeded #announcements: $seeded messages restored (expected $expected) ✓"
  elif [ "$seeded" -ge 1 ]; then
    echo "[pq-wipe] WARN: PARTIAL re-seed — $seeded restored, expected $expected"
  else
    echo "[pq-wipe] ERROR: re-seed produced 0 messages. The DB schema may not have been ready."
    echo "[pq-wipe] pre-wipe backup at $BACKUPS/relay-PREWIPE-$ts.db. Manual recovery:"
    echo "    systemctl stop $UNIT && node $SEED_JS | sqlite3 $DB && systemctl start $UNIT"
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
echo "[pq-wipe] Restore (if needed): systemctl stop $UNIT; cp that file to $DB; systemctl start $UNIT"
