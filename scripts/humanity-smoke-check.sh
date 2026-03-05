#!/usr/bin/env bash
set -euo pipefail

SERVICE="humanity-relay.service"

echo "[smoke] checking service health"
systemctl is-active --quiet "$SERVICE"
echo "[smoke] service_active=ok"

echo "[smoke] checking runtime/repo asset sync"
/usr/local/bin/humanity-verify-runtime-sync

echo "[smoke] checking command handlers in source"
SRC="/opt/Humanity/crates/humanity-relay/src/relay.rs"
for needle in '"/channel-edit"' '"/channel-delete"'; do
  if ! grep -q "$needle" "$SRC"; then
    echo "[smoke] ERROR: missing command handler in source: $needle"
    exit 3
  fi
done
echo "[smoke] command_handlers=ok"

echo "[smoke] checking runtime HTML/JS for mojibake markers"
python3 - <<'PY'
from pathlib import Path
import sys
roots = [Path('/var/www/humanity')]
needles_any = ['�']
needles_seq = []
issues = []
for root in roots:
    for p in root.rglob('*'):
        if not p.is_file():
            continue
        if p.suffix.lower() not in {'.html', '.js'}:
            continue
        txt = p.read_text(encoding='utf-8', errors='ignore')
        hit = any(n in txt for n in needles_any) or any(n in txt for n in needles_seq)
        if not hit and 'dY' in txt:
            # Guard against false positives like "body"/"ready" by requiring non-letter after dY
            for i in range(len(txt) - 2):
                if txt[i:i+2] == 'dY':
                    c = txt[i+2]
                    if not (('a' <= c <= 'z') or ('A' <= c <= 'Z')):
                        hit = True
                        break
        if hit:
            issues.append(str(p))
if issues:
    print('[smoke] ERROR: mojibake markers detected in runtime files:')
    for i in issues[:20]:
        print(' -', i)
    sys.exit(5)
print('[smoke] mojibake_scan=ok')
PY

echo "[smoke] all checks passed"
