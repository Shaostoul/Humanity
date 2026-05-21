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
#   - Healthy + we were previously DOWN  -> post a recovery notice to
#     #announcements (relay is up now, so the post lands) so there's a
#     visible "it was down, it's back" record.
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

REPO="/opt/Humanity"
HEALTH_URL="http://localhost:3210/health"
BINARY="$REPO/target/release/HumanityOS"
UNIT="humanity-relay"
STATE_FILE="/run/humanity-relay-watchdog.state"   # tmpfs; resets on reboot (fine)
LOG_TAG="humanity-relay-watchdog"

log() { logger -t "$LOG_TAG" -- "$*" 2>/dev/null || echo "[$LOG_TAG] $*"; }

# Post to #announcements via the bot API (best-effort; only works when
# the relay is up, which for the recovery notice it is by definition).
# In-app announce to #announcements (works only when the relay is up,
# which for the recovery notice it is by definition).
announce() {
  local content="$1"
  [ -f "$REPO/.env" ] || return 0
  local secret
  secret="$(grep '^API_SECRET' "$REPO/.env" | cut -d= -f2- | tr -d '\r' || true)"
  [ -n "${secret:-}" ] || return 0
  # Plain ASCII content — chat clients render this; no glyph risk.
  local body
  body="{\"channel\":\"announcements\",\"content\":\"${content}\",\"from_name\":\"Watchdog\"}"
  curl -s --max-time 10 -X POST "http://localhost:3210/api/send" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer ${secret}" \
    -d "$body" >/dev/null 2>&1 || true
}

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
    suspect|healing|down-critical)
      log "RECOVERED: relay healthy again (HTTP 200, was '$prev')"
      announce "[Watchdog] Relay recovered and is responding again."
      external_alert "HumanityOS relay RECOVERED and is responding again." "info"
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
