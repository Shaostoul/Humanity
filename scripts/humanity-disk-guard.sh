#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────
# HumanityOS VPS disk janitor + disk-pressure guard
# ─────────────────────────────────────────────────────────────────────
# WHY THIS EXISTS (incident 2026-05-17):
#   The deploy builds ON the VPS and keeps `target/` for speed. Over
#   ~262 releases that cache grew, unpruned + unmonitored, to 27 GB and
#   filled the 118 GB disk. With 0 bytes free the relay could not write
#   and crash-looped — production 502'd SILENTLY and Forgejo pushes
#   were rejected. Assessment that day also found `backups/` at 11 GB:
#   665 files, mostly old compiled relay binaries (one per deploy,
#   ~22 MB, never rotated) — the next disk bomb of the same class.
#
# WHAT THIS DOES (idempotent — safe every N minutes from the .timer):
#   ALWAYS (cheap hygiene — pure cache / build-artifacts / logs):
#     • rotate old compiled-binary backups   (keep newest $BIN_KEEP)
#     • rotate old deploy *.log              (keep newest $LOG_KEEP)
#     • clean the apt package cache
#     • cap journald                         (--vacuum-size=$JOURNAL_MAX)
#   THRESHOLD ($WARN_PCT):
#     • delete the regenerable Rust build cache (`target/`). NOT data —
#       the next deploy rebuilds it. This alone makes the original
#       incident structurally impossible.
#   THRESHOLD ($CRIT_PCT):
#     • harder journald vacuum + a VISIBLE chat alert (same mechanism
#       the deploy bot uses) so disk pressure is never silent again.
#   THRESHOLD ($WARN_PCT), added after the 2026-10-05 incident below:
#     • log the size of every place that has filled this disk before,
#       so the journal names the culprit instead of only saying "100%"
#     • clear Forgejo's cached source archives (regenerable)
#
# INCIDENT 2026-10-05 (the git mirror's archive cache):
#   A crawler walked the public git mirror (git.united-humanity.us) and
#   asked for source archives (.zip, .tar.gz, .bundle) of many commits.
#   Forgejo builds each one on disk and keeps it until its own midnight
#   cleanup deletes archives older than a day, so about 60 GB piled up
#   in /var/lib/forgejo/data/repo-archive in a few hours. The disk sat
#   at 100% for a day: the mirror stopped syncing (its queue database
#   kept the write error after space came back), two relay deploys
#   failed, this guard deleted target/ under a running deploy build,
#   and its critical alert went into #announcements 73 times, once per
#   run. All three of those are handled below. See docs/BUGS.md BUG-169.
#
#   OPT-IN ONLY ($HUMANITY_DB_BACKUP_KEEP set):
#     • rotate relay-*.db DATA backups, keeping that many newest.
#       DEFAULT OFF. The script NEVER deletes a data backup unless the
#       operator explicitly sets this — data deletion requires consent,
#       not an agent-inferred policy.
#
# Dependency-free (bash + coreutils + journalctl) so it cannot itself
# fail to run.
# ─────────────────────────────────────────────────────────────────────
set -euo pipefail

ROOT_MNT="/"
REPO="/opt/Humanity"
TARGET="$REPO/target"
BACKUPS="$REPO/backups"
LOG_TAG="humanity-disk-guard"

# ── Tunables (env-overridable) ──
# Thresholds tuned for the 118 GB VPS so the guard never THRASHES: a
# healthy clean relay build (~5-8 GB) must fit WITHOUT tripping reclaim
# (88% of 118 GB still leaves ~14 GB headroom). The deploy pipeline
# does its own PRE-build reclaim at 80% (safe — it rebuilds right
# after).
WARN_PCT="${HUMANITY_DISK_WARN_PCT:-88}"     # reclaim build cache at/above this %
CRIT_PCT="${HUMANITY_DISK_CRIT_PCT:-94}"     # vacuum logs + alert at/above this %
BIN_KEEP="${HUMANITY_BIN_BACKUP_KEEP:-10}"   # compiled-binary backups kept
LOG_KEEP="${HUMANITY_DEPLOY_LOG_KEEP:-20}"   # deploy *.log kept
JOURNAL_MAX="${HUMANITY_JOURNAL_MAX:-200M}"  # journald size cap
# Data: rotate relay-*.db only if the operator opts in (count of
# newest to keep). Empty = OFF = never touch data backups.
DB_KEEP="${HUMANITY_DB_BACKUP_KEEP:-}"

# Forgejo's cache of generated source archives. Pure cache: Forgejo
# builds an archive again when someone asks for one that is gone.
FORGEJO_ARCHIVES="/var/lib/forgejo/data/repo-archive"
# Remembers that the critical alert was already sent, so one incident
# posts one alert (and one "recovered" note), not one per run.
# /run is cleared by a reboot, which is fine: a reboot is a new incident.
CRIT_STATE="/run/humanity-disk-guard.critical"

log() { logger -t "$LOG_TAG" -- "$*" 2>/dev/null || echo "[$LOG_TAG] $*"; }
usage_pct() { df --output=pcent "$ROOT_MNT" | tail -1 | tr -dc '0-9'; }

# Post a line into #announcements through the local relay (lands only
# when the relay is up), and to any external alert channels the admin
# configured (those work even when the relay is down).
announce() { # $1=message  $2=level
  local msg="$1" level="$2"
  if [ -f "$REPO/.env" ]; then
    SECRET="$(grep '^API_SECRET' "$REPO/.env" | cut -d= -f2- | tr -d '\r' || true)"
    if [ -n "${SECRET:-}" ]; then
      # Plain ASCII on purpose: chat clients render this; no
      # emoji/variation-selector glyph risk.
      BODY="{\"channel\":\"announcements\",\"content\":\"${msg}\",\"from_name\":\"Disk Guard\"}"
      curl -s --max-time 10 -X POST "http://localhost:3210/api/send" \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer ${SECRET}" \
        -d "$BODY" >/dev/null 2>&1 || log "alert POST failed (relay down?)"
    fi
  fi
  if command -v node >/dev/null 2>&1 && [ -f "$REPO/scripts/humanity-alert.js" ]; then
    node "$REPO/scripts/humanity-alert.js" "$msg" "$level" >/dev/null 2>&1 || true
  fi
}

# True while a Rust build is running (a deploy compiling the relay).
# Deleting target/ under it fails that build with "could not write
# output", which is how the v0.1464.0 deploy died on 2026-10-06.
building() { pgrep -x cargo >/dev/null 2>&1 || pgrep -x rustc >/dev/null 2>&1; }

# Keep the newest N files matching a glob in $BACKUPS, delete the rest.
# Best-effort; never aborts the run.
rotate() { # $1=glob  $2=keep
  local glob="$1" keep="$2"
  [ -d "$BACKUPS" ] || return 0
  ( cd "$BACKUPS" 2>/dev/null || exit 0
    # shellcheck disable=SC2012  (filenames here are timestamped, no newlines)
    ls -1t $glob 2>/dev/null | tail -n +"$((keep + 1))" | xargs -r rm -f
  ) || true
}

pct="$(usage_pct)"
log "start: disk ${pct}% (warn=${WARN_PCT} crit=${CRIT_PCT})"

# ── ALWAYS: cheap hygiene (cache / build-artifacts / logs only) ──
before_files="$( ls -1 "$BACKUPS" 2>/dev/null | wc -l || echo '?' )"
rotate 'HumanityOS*'     "$BIN_KEEP"     # old compiled binaries
rotate 'humanity-relay*' "$BIN_KEEP"     # old compiled binaries (alt name)
rotate 'deploy*.log'     "$LOG_KEEP"     # deploy logs
after_files="$( ls -1 "$BACKUPS" 2>/dev/null | wc -l || echo '?' )"
log "backups rotated: ${before_files} -> ${after_files} files (kept ${BIN_KEEP} binaries / ${LOG_KEEP} logs; relay-*.db DATA untouched)"
apt-get clean >/dev/null 2>&1 || true
journalctl --vacuum-size="$JOURNAL_MAX" >/dev/null 2>&1 || true

# ── ALWAYS: release-mirror retention (added 2026-05-21 after the
# release-mirror-bloat incident). The /var/www/humanity/releases/
# tree accumulates one versioned dir per release (~345 MB each:
# Linux + macOS x64 + macOS arm64 + Windows binaries × raw + tar.gz
# + torrents + data archive). With 287 versions unrotated we hit
# 91 GB and the cascade chain went: disk 92% → target/ wipe →
# missing binary → relay crash-loop. Cap retention here so the
# cascade cannot re-fire. After deletion, regenerate manifest.json
# so it does not reference removed versions.
RELEASES_DIR="/var/www/humanity/releases"
RELEASES_KEEP="${HUMANITY_RELEASES_KEEP:-10}"
if [ -d "$RELEASES_DIR" ]; then
  before_releases="$( ls -1d "$RELEASES_DIR"/v* 2>/dev/null | wc -l || echo '?' )"
  # Sort version-aware (so v0.10.0 > v0.9.0, not lexical), drop the
  # newest N, rm the rest. Empty result on first run when count <= N.
  ls -1d "$RELEASES_DIR"/v* 2>/dev/null | sort -V | head -n -"$RELEASES_KEEP" | xargs -r rm -rf || true
  after_releases="$( ls -1d "$RELEASES_DIR"/v* 2>/dev/null | wc -l || echo '?' )"
  if [ "$before_releases" != "$after_releases" ]; then
    log "release mirror rotated: ${before_releases} -> ${after_releases} versions (kept ${RELEASES_KEEP}); regenerating manifest"
    # regen-releases-manifest is a separate VPS-only script; best-
    # effort. If absent, the manifest may reference deleted versions
    # but the actual download endpoints just 404 them.
    if [ -x /usr/local/bin/regen-releases-manifest ]; then
      /usr/local/bin/regen-releases-manifest >/dev/null 2>&1 || log "manifest regen failed (non-fatal)"
    fi
  fi
fi

# ── OPT-IN: data-backup rotation (only if operator set DB_KEEP) ──
if [ -n "$DB_KEEP" ]; then
  log "DB rotation ENABLED by operator (HUMANITY_DB_BACKUP_KEEP=${DB_KEEP}) — keeping newest ${DB_KEEP} relay-*.db"
  rotate 'relay-*.db' "$DB_KEEP"
else
  log "DB rotation OFF (default) — relay-*.db data backups left 100% intact"
fi

# ── THRESHOLD: name what is using the disk ──
# Each of these has filled this disk at least once. One line in the
# journal per run while the disk is high, so the next incident's cause
# is read from the log instead of reconstructed from directory dates.
pct="$(usage_pct)"
if [ "${pct:-0}" -ge "$WARN_PCT" ]; then
  sizes=""
  for d in "$RELEASES_DIR" "$FORGEJO_ARCHIVES" /var/lib/forgejo "$TARGET" "$BACKUPS" \
           "$REPO/data" /var/log /var/lib/nginx /tmp /root; do
    [ -e "$d" ] || continue
    sizes="${sizes} ${d}=$(du -xsh "$d" 2>/dev/null | cut -f1 || echo '?')"
  done
  log "WHERE: disk ${pct}%:${sizes}"
fi

# ── THRESHOLD: clear the git mirror's archive cache ──
# Regenerable, and the cause of the 2026-10-05 incident. Forgejo's own
# cleanup only runs once a day; at this threshold that is too slow.
pct="$(usage_pct)"
if [ "${pct:-0}" -ge "$WARN_PCT" ] && [ -d "$FORGEJO_ARCHIVES" ]; then
  sz="$(du -xsh "$FORGEJO_ARCHIVES" 2>/dev/null | cut -f1 || echo '?')"
  find "$FORGEJO_ARCHIVES" -type f -delete 2>/dev/null || true
  log "RECLAIM: disk ${pct}% >= ${WARN_PCT}% -- cleared the git mirror's archive cache (${sz})"
fi

# ── THRESHOLD: reclaim the regenerable build cache ──
pct="$(usage_pct)"
if [ "${pct:-0}" -ge "$WARN_PCT" ]; then
  if [ -d "$TARGET" ]; then
    sz="$(du -sh "$TARGET" 2>/dev/null | cut -f1 || echo '?')"
    if building; then
      log "RECLAIM deferred: disk ${pct}% but a build is running in ${TARGET} (${sz}); deleting it now would fail that build"
    else
      # clear-build-cache.sh keeps target/release/HumanityOS, the file the
      # relay runs from; a bare rm -rf here left the relay unrestartable
      # twice (2026-05-21, 2026-10-06).
      log "RECLAIM: disk ${pct}% >= ${WARN_PCT}% -- clearing build cache ${TARGET} (${sz}), keeping the relay binary; next deploy rebuilds clean"
      bash "$REPO/scripts/clear-build-cache.sh" "$REPO" >/dev/null 2>&1 || true
    fi
  fi
fi

# ── ALWAYS: the relay must have a binary to restart from ──
# The relay keeps running from an open file even after that file is
# deleted, so a missing binary looks fine until the next restart. If it
# is missing and no build is putting it back, restore the newest build
# the deploy saved in backups/ (built on this machine, so it runs here;
# the release mirror's Linux build does not, see the 2026-05-21 entry
# in docs/INCIDENT-PLAYBOOK.md).
RELAY_BIN="$TARGET/release/HumanityOS"
if [ ! -f "$RELAY_BIN" ] && ! building; then
  newest="$( ls -1t "$BACKUPS"/HumanityOS-* 2>/dev/null | head -n 1 || true )"
  if [ -n "$newest" ] && mkdir -p "$TARGET/release" && cp -p "$newest" "$RELAY_BIN.restoring" && \
     chmod +x "$RELAY_BIN.restoring" && mv -f "$RELAY_BIN.restoring" "$RELAY_BIN"; then
    log "RELAY BINARY RESTORED: ${RELAY_BIN} was missing; copied ${newest}"
  else
    rm -f "$RELAY_BIN.restoring" 2>/dev/null || true
    log "RELAY BINARY MISSING: ${RELAY_BIN} is gone and no backup could be restored; the relay cannot restart until a deploy"
  fi
fi

# ── THRESHOLD: still critical -> harder vacuum + ONE visible alert ──
pct="$(usage_pct)"
if [ "${pct:-0}" -ge "$CRIT_PCT" ]; then
  log "CRITICAL: disk still ${pct}% -- vacuuming journald to 50M"
  journalctl --vacuum-size=50M >/dev/null 2>&1 || true
  pct="$(usage_pct)"
  if [ -e "$CRIT_STATE" ]; then
    log "critical alert already sent at $(cat "$CRIT_STATE" 2>/dev/null || echo '?'); not posting again"
  else
    announce "[Disk Guard] VPS root disk at ${pct}% after automatic cleanup. Investigate: the journal line starting WHERE names the biggest folders (journalctl -t humanity-disk-guard)." "critical"
    date -u '+%Y-%m-%d %H:%M UTC' > "$CRIT_STATE" 2>/dev/null || true
  fi
elif [ -e "$CRIT_STATE" ]; then
  announce "[Disk Guard] VPS root disk back to ${pct}% (critical since $(cat "$CRIT_STATE" 2>/dev/null || echo '?'))." "info"
  rm -f "$CRIT_STATE" || true
fi

log "done: disk at $(usage_pct)%"
