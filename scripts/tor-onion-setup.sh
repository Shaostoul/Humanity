#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────
# Set up a Tor v3 onion service for a HumanityOS relay (privacy, 2026-08-24)
# ─────────────────────────────────────────────────────────────────────
# Gives users transport-level IP privacy: over the .onion address the relay
# never learns their network location. Opt-in and additive; the clearnet
# https:// endpoint is untouched. Full rationale + limits:
# docs/admin/tor-onion-service.md
#
# Usage: sudo scripts/tor-onion-setup.sh [local_port]   (default 8080)

set -euo pipefail

PORT="${1:-8080}"
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
  echo ""
  echo "Onion address (publish this alongside your clearnet URL):"
  cat "$HS_DIR/hostname"
else
  echo "Tor restarted but no hostname yet; check 'journalctl -u tor' and re-run." >&2
  exit 2
fi
