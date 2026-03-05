#!/usr/bin/env bash
set -euo pipefail
MANIFEST="/opt/Humanity/scripts/runtime-sync-manifest.txt"

if [[ ! -f "$MANIFEST" ]]; then
  echo "[sync] ERROR: manifest missing: $MANIFEST"
  exit 10
fi

while IFS='|' read -r SRC DST; do
  [[ -z "${SRC}" ]] && continue
  [[ "$SRC" =~ ^# ]] && continue
  if [[ ! -f "$SRC" ]]; then
    echo "[sync] ERROR: source missing: $SRC"
    exit 11
  fi
  if [[ ! -f "$DST" ]]; then
    echo "[sync] ERROR: runtime missing: $DST"
    exit 12
  fi
  SH_SRC="$(sha256sum "$SRC" | awk '{print $1}')"
  SH_DST="$(sha256sum "$DST" | awk '{print $1}')"
  if [[ "$SH_SRC" != "$SH_DST" ]]; then
    echo "[sync] ERROR: drift: $DST"
    echo "[sync] src=$SH_SRC dst=$SH_DST"
    exit 13
  fi
  echo "[sync] ok: $DST"
done < "$MANIFEST"

echo "[sync] runtime_sync=ok"
