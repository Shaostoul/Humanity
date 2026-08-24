#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────
# Decrypt a sealed HumanityOS relay backup (privacy hardening 2026-08-23)
# ─────────────────────────────────────────────────────────────────────
# Two sealed forms exist, both keyed by the machine-local backup key
# (default /opt/Humanity/data/backup.key — copy it somewhere safe; a
# backup without its key is deliberately unreadable):
#
#   *.db.aes  — VPS script snapshots (openssl AES-256-CBC + PBKDF2)
#   *.db.enc  — the relay's own 6-hour snapshots (AES-256-GCM,
#               12-byte nonce prefix). The relay decrypts these itself
#               during crash recovery; this script covers manual use.
#
# Usage: scripts/decrypt-backup.sh <sealed-file> [key-file] [out-file]

set -euo pipefail

IN="${1:?usage: decrypt-backup.sh <sealed-file> [key-file] [out-file]}"
KEY="${2:-/opt/Humanity/data/backup.key}"
OUT="${3:-${IN%.aes}}"
OUT="${OUT%.enc}"

if [ ! -f "$IN" ]; then echo "no such file: $IN" >&2; exit 2; fi
if [ ! -f "$KEY" ]; then echo "no key file at $KEY" >&2; exit 2; fi

case "$IN" in
  *.db.aes)
    openssl enc -d -aes-256-cbc -pbkdf2 -iter 200000 \
      -pass "file:$KEY" -in "$IN" -out "$OUT"
    ;;
  *.db.enc)
    # GCM via python3 (openssl enc can't do GCM). Debian ships python3;
    # cryptography is available via python3-cryptography or pip.
    python3 - "$IN" "$KEY" "$OUT" <<'PY'
import sys
from cryptography.hazmat.primitives.ciphers.aead import AESGCM
inp, keyf, outp = sys.argv[1:4]
raw = open(inp, 'rb').read()
key = open(keyf, 'rb').read()
open(outp, 'wb').write(AESGCM(key).decrypt(raw[:12], raw[12:], None))
PY
    ;;
  *)
    echo "unrecognized sealed-backup extension (expected .db.aes or .db.enc): $IN" >&2
    exit 2
    ;;
esac

# Sanity: the output should be a SQLite database.
if command -v sqlite3 >/dev/null 2>&1; then
  sqlite3 "$OUT" "PRAGMA quick_check;" | head -1
fi
printf 'decrypted=%s size=%s\n' "$OUT" "$(stat -c%s "$OUT" 2>/dev/null || wc -c < "$OUT")"
