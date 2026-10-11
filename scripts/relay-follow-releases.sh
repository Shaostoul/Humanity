#!/bin/bash
# =============================================================================
# relay-follow-releases.sh: keep a self-hosted relay on the latest release.
#
# Why (BUG-192, 2026-10-11): public.guide, the operator's second relay, was
# updated by hand and sat on v0.1147.0 for two months. When server keys moved
# to Dilithium3 it could no longer talk to united-humanity.us, and chat between
# the two was down for five weeks without anyone noticing. The deploy workflow
# updates only the main server; any other relay has to keep itself current.
#
# What it does, run by humanity-relay-follow.timer (hourly):
#   1. Asks GitHub for the latest release (not a pre-release or a draft).
#   2. Does nothing if that is the release already installed.
#   3. Downloads HumanityOS-relay-linux-x64 and its .sha256 from that release
#      and refuses a file whose hash does not match (the same check
#      provision-vps.sh makes).
#   4. Backs up the database (sqlite3 .backup) and the running binary.
#   5. Checks the repo out at the release (for its data files and scripts),
#      installs the binary, restarts the relay, and waits for /health.
#   6. Healthy: lays out the web root the way the deploy does and records the
#      release. Not healthy within a minute: puts back the previous binary,
#      the previous checkout and the database backup, restarts, and exits 1 so
#      the journal and the unit's status say so.
#
# Settings (environment, all optional): REPO (/opt/Humanity), WEB_ROOT
# (/var/www/humanity), SERVICE (humanity-relay), PORT (3210),
# STATE_DIR (/var/lib/humanity-follow), KEEP_BACKUPS (3).
# =============================================================================
set -u

REPO="${REPO:-/opt/Humanity}"
WEB_ROOT="${WEB_ROOT:-/var/www/humanity}"
SERVICE="${SERVICE:-humanity-relay}"
PORT="${PORT:-3210}"
STATE_DIR="${STATE_DIR:-/var/lib/humanity-follow}"
KEEP_BACKUPS="${KEEP_BACKUPS:-3}"
GITHUB="https://github.com/Shaostoul/Humanity"
API="https://api.github.com/repos/Shaostoul/Humanity/releases/latest"
BIN="$REPO/target/release/HumanityOS"

say() { echo "[follow] $*"; }

mkdir -p "$STATE_DIR"

# 1. The latest release's tag. /releases/latest never returns a pre-release or a draft.
latest="$(curl -fsSL -H 'Accept: application/vnd.github+json' "$API" | grep -o '"tag_name": *"[^"]*"' | head -1 | cut -d'"' -f4)"
if [ -z "$latest" ]; then
  say "could not read the latest release from GitHub; trying again next time"
  exit 0
fi

# 2. What is installed: the recorded tag, else the tag the checkout sits on.
installed="$(cat "$STATE_DIR/installed_tag" 2>/dev/null || true)"
if [ -z "$installed" ]; then
  installed="$(git -C "$REPO" describe --tags --exact-match 2>/dev/null || true)"
fi
if [ "$latest" = "$installed" ]; then
  say "already on $latest"
  echo "$latest" > "$STATE_DIR/installed_tag"
  exit 0
fi
# A release that failed its health check here and was rolled back waits for a newer one, instead
# of being tried (and rolled back) every hour.
if [ "$latest" = "$(cat "$STATE_DIR/skipped_tag" 2>/dev/null || true)" ]; then
  say "$latest was rolled back here before; waiting for a newer release"
  exit 0
fi
say "updating ${installed:-an unknown version} to $latest"

# 3. Download and verify.
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
url="$GITHUB/releases/download/$latest/HumanityOS-relay-linux-x64"
if ! curl -fsSL "$url" -o "$tmp/relay" || ! curl -fsSL "$url.sha256" -o "$tmp/relay.sha256"; then
  say "could not download the relay for $latest; trying again next time"
  exit 0
fi
want="$(awk '{print $1}' "$tmp/relay.sha256")"
got="$(sha256sum "$tmp/relay" | awk '{print $1}')"
if [ -z "$want" ] || [ "$want" != "$got" ]; then
  say "checksum mismatch on the relay for $latest (expected $want, got $got): refusing it"
  exit 1
fi

# 4. Backups: the database, the binary and where the checkout was.
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
backup_db="$STATE_DIR/relay.db.before-$latest-$stamp"
if ! sqlite3 "$REPO/data/relay.db" ".backup '$backup_db'"; then
  say "could not back up the database; not updating"
  exit 1
fi
cp -a "$BIN" "$STATE_DIR/HumanityOS.previous"
prev_ref="$(git -C "$REPO" rev-parse HEAD)"
# Keep only the newest KEEP_BACKUPS database backups.
ls -1t "$STATE_DIR"/relay.db.before-* 2>/dev/null | tail -n +"$((KEEP_BACKUPS + 1))" | xargs -r rm -f

# 5. The release's files, the new binary, a restart, and the health check.
if ! git -C "$REPO" fetch -q --tags origin || ! git -C "$REPO" checkout -q -f "$latest"; then
  say "could not check the repo out at $latest; not updating"
  exit 1
fi
install -m 755 "$tmp/relay" "$BIN"
systemctl restart "$SERVICE"
healthy=0
for _ in $(seq 1 30); do
  if curl -fs "http://127.0.0.1:$PORT/health" >/dev/null 2>&1; then healthy=1; break; fi
  sleep 2
done

if [ "$healthy" = 1 ]; then
  # 6. The web root, laid out the way the deploy does it.
  if [ -d "$WEB_ROOT" ] && [ -f "$REPO/scripts/sync-web-root.sh" ]; then
    bash "$REPO/scripts/sync-web-root.sh" "$REPO" "$WEB_ROOT" || say "the web root sync failed (the relay itself is updated)"
  fi
  echo "$latest" > "$STATE_DIR/installed_tag"
  say "updated to $latest; the relay is healthy"
  exit 0
fi

say "the relay did not come back healthy on $latest within a minute: rolling back"
systemctl stop "$SERVICE"
install -m 755 "$STATE_DIR/HumanityOS.previous" "$BIN"
cp -a "$backup_db" "$REPO/data/relay.db"
rm -f "$REPO/data/relay.db-wal" "$REPO/data/relay.db-shm"
git -C "$REPO" checkout -q -f "$prev_ref"
systemctl start "$SERVICE"
say "rolled back to ${installed:-the previous version}; $latest is skipped until a newer release"
echo "$latest" > "$STATE_DIR/skipped_tag"
exit 1
