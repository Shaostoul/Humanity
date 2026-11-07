#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────
# HumanityOS relay watchdog + self-heal — oneshot, driven by the .timer
# ─────────────────────────────────────────────────────────────────────
# WHY THIS EXISTS (incident 2026-05-21):
#   The relay binary was disk-guard-reclaimed with no deploy follow-up
#   and systemd crash-looped for ~25 minutes before a human noticed.
#   systemd's Restart=always handles a CLEAN crash, but it does NOT
#   catch a process that is alive-but-hung (deadlock, not answering
#   /health), and when it hits its StartLimit it gives up SILENTLY.
#   This watchdog closes both gaps: it checks actual HTTP liveness, and
#   it can reset-failed + restart a unit systemd has given up on.
#
# WHAT IT DOES (every 2 min from the .timer):
#   - GET /health. HTTP 200 = healthy.
#   - Healthy + we were previously DOWN  -> a recovery notice to the
#     server's admins: their alert channels (Server Settings > ADMIN >
#     Alert channels, sent by scripts/humanity-alert.js) and the journal.
#     NEVER a chat channel (BUG-252, 2026-11-06): it used to post into
#     #announcements, and when public.guide restarted every 30 minutes for
#     two days the public channel every visitor reads first filled with
#     "[Watchdog] Relay recovered and is responding again." A server's
#     health is its admins' business, and a flapping server must not be
#     able to fill a public channel. scripts/tests/watchdog-target.test.js
#     runs this script and fails on any chat post.
#   - Not healthy:
#       * 1st consecutive failure -> log + record "down" (NO restart;
#         absorbs a single transient slow response without thrashing).
#       * 2nd+ consecutive failure -> self-heal: if the binary exists,
#         reset-failed + restart. If the binary is MISSING, log CRITICAL
#         and do NOT attempt a rebuild (too risky unattended — needs a
#         deploy). The 2026-05-21 outage was exactly this: a rebuild,
#         not a restart, was required, so the watchdog must surface it
#         loudly rather than spin uselessly.
#
# Self-contained: bash + coreutils + curl + systemctl. Logs to journald
# (tag humanity-relay-watchdog) so `journalctl -t humanity-relay-watchdog`
# is the audit trail.
# ─────────────────────────────────────────────────────────────────────
set -uo pipefail   # deliberately NOT -e: we handle every failure path

# REPO and STATE_FILE can be moved for scripts/tests/watchdog-target.test.js;
# the unit sets neither.
REPO="${REPO:-/opt/Humanity}"
HEALTH_URL="http://localhost:3210/health"
BINARY="$REPO/target/release/HumanityOS"
UNIT="humanity-relay"
STATE_FILE="${STATE_FILE:-/run/humanity-relay-watchdog.state}"   # tmpfs; resets on reboot (fine)
LOG_TAG="humanity-relay-watchdog"

log() { logger -t "$LOG_TAG" -- "$*" 2>/dev/null || echo "[$LOG_TAG] $*"; }

# EXTERNAL alert fanout (ntfy/Discord/Telegram/webhook) via the shared
# node helper. Works even when the relay is DOWN (this is a separate
# process). No-op if the admin hasn't configured any channels. Best-
# effort; never blocks. severity: info | warn | critical.
external_alert() {
  local msg="$1" sev="${2:-warn}"
  command -v node >/dev/null 2>&1 || return 0
  [ -f "$REPO/scripts/humanity-alert.js" ] || return 0
  node "$REPO/scripts/humanity-alert.js" "$msg" "$sev" >/dev/null 2>&1 || true
}

code="$(curl -s -o /dev/null -w '%{http_code}' --max-time 8 "$HEALTH_URL" 2>/dev/null || echo '000')"
prev="$(cat "$STATE_FILE" 2>/dev/null || echo 'unknown')"

# State machine (anti-spam: external alerts fire ONCE on the down
# transition and ONCE on recovery, never every 2-min cycle):
#   up           healthy
#   suspect      one failed check (absorb a transient slow response)
#   healing      confirmed down, binary present, restart attempted
#   down-critical confirmed down, binary MISSING (restart can't fix)

if [ "$code" = "200" ]; then
  case "$prev" in
    suspect)
      # ONE failed check and then healthy: a deploy's restart, or one slow
      # answer. Nothing was confirmed down and no "down" alert went out, so
      # a "recovered" post would announce an outage nobody had. It did, 73
      # times in #announcements (2026-05 to 2026-10, BUG-169): every deploy
      # restart posted "Relay recovered" with no outage before it.
      log "healthy again after one failed check (was 'suspect'): not an outage, nothing sent"
      ;;
    healing|down-critical)
      # To the admins only: the journal and their alert channels (BUG-252).
      log "RECOVERED: relay healthy again (HTTP 200, was '$prev')"
      external_alert "HumanityOS relay recovered and is responding again." "info"
      ;;
  esac
  echo "up" > "$STATE_FILE"
  exit 0
fi

# --- Not healthy ---
log "health check FAILED (HTTP $code)"

if [ "$prev" = "up" ] || [ "$prev" = "unknown" ]; then
  # First failure this episode. Record + wait one cycle before acting,
  # so a single transient slow response doesn't trigger a restart/alert.
  echo "suspect" > "$STATE_FILE"
  log "first failure recorded; will self-heal next cycle if still down"
  exit 0
fi

# Second+ consecutive failure -> self-heal.
if [ ! -x "$BINARY" ]; then
  # Binary missing -> a restart cannot fix this (needs a deploy/rebuild,
  # per the 2026-05-21 incident). Alert CRITICAL once, then go quiet.
  if [ "$prev" != "down-critical" ]; then
    log "CRITICAL: relay binary missing/not executable at $BINARY -- CANNOT self-heal (needs a deploy/rebuild). See INCIDENT-PLAYBOOK 2026-05-21."
    external_alert "HumanityOS relay DOWN and binary is MISSING at $BINARY -- a restart cannot fix this, it needs a deploy/rebuild. (Likely cause: build cache reclaimed without a follow-up deploy.)" "critical"
  fi
  echo "down-critical" > "$STATE_FILE"
  exit 1
fi

# Binary present -> self-heal by restart. Alert WARN once (on the first
# confirmed-down transition from 'suspect'); subsequent restart attempts
# while still down stay quiet to avoid spam.
if [ "$prev" = "suspect" ]; then
  external_alert "HumanityOS relay is DOWN (health check failing). Watchdog is auto-restarting it." "warn"
fi
log "self-heal: reset-failed + restart $UNIT"
systemctl reset-failed "$UNIT" 2>/dev/null || true
systemctl restart "$UNIT" 2>/dev/null || true
echo "healing" > "$STATE_FILE"
exit 0
