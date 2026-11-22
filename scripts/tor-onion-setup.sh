#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────
# Set up a Tor v3 onion service for a HumanityOS relay (privacy, 2026-08-24)
# ─────────────────────────────────────────────────────────────────────
# Gives users transport-level IP privacy: over the .onion address the relay
# never learns their network location. Opt-in and additive; the clearnet
# https:// endpoint is untouched. Full rationale + limits:
# docs/admin/tor-onion-service.md
#
# Usage: sudo scripts/tor-onion-setup.sh [local_port]   (default 3210)
#
# local_port is the relay's PORT (default 3210). This said 8080 until
# 2026-10-03, a port nothing listens on. Tor reaches the relay over loopback,
# so this works with BIND_ADDRESS=127.0.0.1 as well as the default.

set -euo pipefail

PORT="${1:-3210}"
HS_DIR="/var/lib/tor/humanity"
TORRC="/etc/tor/torrc"

if [ "$(id -u)" -ne 0 ]; then
  echo "Run with sudo (Tor config + service restart need root)." >&2
  exit 1
fi

if ! command -v tor >/dev/null 2>&1; then
  echo "Installing tor..."
  apt-get update -qq && apt-get install -y tor
fi

# Add the hidden-service block once (idempotent).
if ! grep -q "HiddenServiceDir $HS_DIR" "$TORRC"; then
  {
    echo ""
    echo "# HumanityOS relay onion service (scripts/tor-onion-setup.sh)"
    echo "HiddenServiceDir $HS_DIR/"
    echo "HiddenServicePort 80 127.0.0.1:$PORT"
    echo "HiddenServiceVersion 3"
  } >> "$TORRC"
  echo "Added onion-service block to $TORRC (local port $PORT)."
else
  echo "Onion-service block already present in $TORRC; leaving it."
fi

systemctl restart tor

# The hostname file appears a moment after restart.
for _ in $(seq 1 20); do
  if [ -f "$HS_DIR/hostname" ]; then break; fi
  sleep 0.5
done

if [ -f "$HS_DIR/hostname" ]; then
  ONION="$(tr -d '[:space:]' < "$HS_DIR/hostname")"
  echo ""
  echo "Onion address (publish this alongside your clearnet URL):"
  echo "$ONION"
  # The relay must answer to the onion name (2026-11-21): every signed request and sign-in names
  # the address the app connected to, and the relay refuses a name it was not given
  # (src/relay/audience.rs). Tor's port 80 is the default, so the name has no port.
  ENV_FILE="${HUMANITY_REPO:-/opt/Humanity}/.env"
  if [ -f "$ENV_FILE" ]; then
    if grep -q '^PUBLIC_HOSTS=' "$ENV_FILE"; then
      grep -q "^PUBLIC_HOSTS=.*$ONION" "$ENV_FILE" || sed -i "s|^PUBLIC_HOSTS=\(.*\)|PUBLIC_HOSTS=\1,$ONION|" "$ENV_FILE"
    else
      printf 'PUBLIC_HOSTS=%s\n' "$ONION" >> "$ENV_FILE"
    fi
    echo "PUBLIC_HOSTS in $ENV_FILE names the onion address; restart the relay to take it: sudo systemctl restart humanity-relay"
  else
    echo "Add the onion address to the relay's PUBLIC_HOSTS setting (no $ENV_FILE here), then restart the relay."
  fi
else
  echo "Tor restarted but no hostname yet; check 'journalctl -u tor' and re-run." >&2
  exit 2
fi
