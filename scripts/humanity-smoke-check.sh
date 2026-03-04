#!/usr/bin/env bash
set -euo pipefail

SERVICE="humanity-relay.service"
REPO_JS="/opt/Humanity/crates/humanity-relay/client/app.js"
RUNTIME_JS="/var/www/humanity/chat/app.js"

echo "[smoke] checking service health"
systemctl is-active --quiet "$SERVICE"
echo "[smoke] service_active=ok"

echo "[smoke] checking runtime/repo js sync"
REPO_HASH="$(sha256sum "$REPO_JS" | awk '{print $1}')"
RUNTIME_HASH="$(sha256sum "$RUNTIME_JS" | awk '{print $1}')"
if [[ "$REPO_HASH" != "$RUNTIME_HASH" ]]; then
  echo "[smoke] ERROR: web runtime drift detected"
  echo "[smoke] repo_hash=$REPO_HASH"
  echo "[smoke] runtime_hash=$RUNTIME_HASH"
  exit 2
fi
echo "[smoke] js_sync=ok hash=$REPO_HASH"

echo "[smoke] checking command handlers in source"
SRC="/opt/Humanity/crates/humanity-relay/src/relay.rs"
for needle in '"/channel-edit"' '"/channel-delete"'; do
  if ! grep -q "$needle" "$SRC"; then
    echo "[smoke] ERROR: missing command handler in source: $needle"
    exit 3
  fi
done
echo "[smoke] command_handlers=ok"

echo "[smoke] all checks passed"
