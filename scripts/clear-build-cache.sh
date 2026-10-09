#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────
# Clear the VPS's regenerable Rust build cache (target/) but KEEP the
# one file in it that runs: target/release/HumanityOS, the live relay
# (the humanity-relay unit's ExecStart, written by provision-vps.sh).
#
# WHY: every "clear the build cache" path used to be a bare
# `rm -rf target`, which deletes the running relay's binary with it.
# The process keeps running from the open file, so nothing looks wrong,
# but the next restart (the watchdog, a crash, a reboot) has nothing to
# start. That is the 2026-05-21 cascade, and on 2026-10-06 it happened
# again: the disk guard cleared target/ during a disk-full incident and
# the relay ran for days as "/opt/Humanity/target/release/HumanityOS
# (deleted)" (BUG-169).
#
# One script, three callers: humanity-disk-guard.sh, the deploy workflow's
# pre-build reclaim (.github/workflows/deploy.yml) and `just sync`.
#
# Usage: clear-build-cache.sh [repo dir, default /opt/Humanity]
# Never fails its caller: every step is best effort.
# ─────────────────────────────────────────────────────────────────────
set -uo pipefail

REPO="${1:-/opt/Humanity}"
TARGET="$REPO/target"
BIN="$TARGET/release/HumanityOS"
KEEP="$REPO/.relay-binary-kept"

[ -d "$TARGET" ] || exit 0

kept=0
if [ -f "$BIN" ]; then
  rm -f "$KEEP"
  # A hard link costs no space, so this works on a full disk. Fall back
  # to a copy only if linking is refused.
  if ln "$BIN" "$KEEP" 2>/dev/null || cp -p "$BIN" "$KEEP" 2>/dev/null; then
    kept=1
  fi
fi

rm -rf "$TARGET" || true

if [ "$kept" = 1 ]; then
  mkdir -p "$TARGET/release" && mv -f "$KEEP" "$BIN" && \
    echo "cleared $TARGET, kept the relay binary" || \
    echo "cleared $TARGET, but could not put the relay binary back (it is at $KEEP)"
else
  echo "cleared $TARGET (there was no relay binary in it to keep)"
fi
exit 0
