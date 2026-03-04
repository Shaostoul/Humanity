#!/usr/bin/env bash
set -euo pipefail
LATEST="$(ls -1t /opt/Humanity/backups/deploy-*.log 2>/dev/null | head -n 1 || true)"
if [[ -z "${LATEST}" ]]; then
  echo "No deploy report found."
  exit 1
fi
echo "latest_report=${LATEST}"
tail -n 120 "$LATEST"
