#!/usr/bin/env bash
set -euo pipefail

SERVICE="humanity-relay.service"

echo "[smoke] checking service health"
systemctl is-active --quiet "$SERVICE"
echo "[smoke] service_active=ok"

echo "[smoke] checking runtime/repo asset sync"
check_sync() {
  local repo_file="$1"
  local runtime_file="$2"
  local label="$3"
  local repo_hash runtime_hash
  repo_hash="$(sha256sum "$repo_file" | awk '{print $1}')"
  runtime_hash="$(sha256sum "$runtime_file" | awk '{print $1}')"
  if [[ "$repo_hash" != "$runtime_hash" ]]; then
    echo "[smoke] ERROR: drift detected for $label"
    echo "[smoke] repo_hash=$repo_hash"
    echo "[smoke] runtime_hash=$runtime_hash"
    exit 2
  fi
  echo "[smoke] ${label}_sync=ok hash=$repo_hash"
}

check_sync "/opt/Humanity/crates/humanity-relay/client/index.html" "/var/www/humanity/chat/index.html" "chat_index_html"
check_sync "/opt/Humanity/crates/humanity-relay/client/app.js" "/var/www/humanity/chat/app.js" "chat_js"
check_sync "/opt/Humanity/crates/humanity-relay/client/style.css" "/var/www/humanity/chat/style.css" "chat_css"
check_sync "/opt/Humanity/shared/shell.js" "/var/www/humanity/shared/shell.js" "shared_shell_js"
check_sync "/opt/Humanity/game/index.html" "/var/www/humanity/game/index.html" "game_index_html"
check_sync "/opt/Humanity/game/index.html" "/var/www/humanity/app.html" "app_html"
check_sync "/opt/Humanity/assets/ui/icons/warning.png" "/var/www/humanity/shared/ui-icons/warning.png" "ui_icon_warning"

echo "[smoke] checking command handlers in source"
SRC="/opt/Humanity/crates/humanity-relay/src/relay.rs"
for needle in '"/channel-edit"' '"/channel-delete"'; do
  if ! grep -q "$needle" "$SRC"; then
    echo "[smoke] ERROR: missing command handler in source: $needle"
    exit 3
  fi
done
echo "[smoke] command_handlers=ok"

echo "[smoke] checking stream block for mojibake markers"
python3 - <<'PY'
from pathlib import Path
import re, sys
s = Path('/var/www/humanity/app.html').read_text(encoding='utf-8', errors='ignore')
a = s.find('<div id="tab-streams"')
b = s.find('<div id="tab-info"', a)
if a == -1 or b == -1:
    print('[smoke] ERROR: stream block boundary not found')
    sys.exit(4)
blk = s[a:b]
if re.search(r'(Â|â|ð|�|dY)', blk):
    print('[smoke] ERROR: mojibake marker detected in stream block')
    sys.exit(5)
print('[smoke] stream_mojibake_check=ok')
PY

echo "[smoke] all checks passed"
