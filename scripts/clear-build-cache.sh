#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────
# Clear the build leftovers in a server's target/ and KEEP the files in
# it that are not build output: target/release/HumanityOS, the program
# the relay runs (the humanity-relay unit's ExecStart, written by
# provision-vps.sh), and the deploy's markers beside it.
#
# WHAT IS KEPT, in target/release/ (everything else in target/ goes):
#   HumanityOS                  the program the relay runs from
#   HumanityOS.source           the commit it was built from (the deploy's
#   HumanityOS.started          plan reads both: without them the next push
#                               rebuilds and restarts the relay for nothing)
#   HumanityOS.incoming(.sha256) the next program, copied up by the deploy
#                               before its step that swaps it in
# The program is never moved or copied: the leftovers are deleted around
# it, so the running relay's file and a restart's file stay the same one.
#
# WHY (2026-11-06, found beside BUG-252): since 2026-10-30 the relay is
# built on GitHub and the deploy only copies the finished program here,
# so nothing in target/ but those files is used again. united-humanity.us
# still held 29 GB of leftovers from when it compiled on the server
# (deps/, build/, incremental/, .fingerprint/, a 175 MB rlib), public.guide
# its own, because this ran only when the disk passed 80% (the deploy) or
# 88% (the disk guard). Now the deploy runs it on every push and the disk
# guard every 20 minutes. `just sync`, the fallback that does build on the
# server, runs it after its build.
#
# EARLIER (BUG-169): every "clear the build cache" path used to be a bare
# `rm -rf target`, which deleted the running relay's program with it. The
# process kept running from the open file, so nothing looked wrong, but
# the next restart had nothing to start: the 2026-05-21 cascade, and again
# on 2026-10-06. The version after that kept the program by hard-linking it
# aside, but still deleted the markers and an incoming program.
#
# Callers: humanity-disk-guard.sh, the deploy workflow
# (.github/workflows/deploy.yml) and `just sync`.
# scripts/tests/build-leftovers.test.js runs it over a scratch target/.
#
# Usage: clear-build-cache.sh [repo dir, default /opt/Humanity]
# Never fails its caller: every step is best effort. Prints one line:
# what it removed, or "nothing to clear".
# ─────────────────────────────────────────────────────────────────────
set -uo pipefail

REPO="${1:-/opt/Humanity}"
TARGET="$REPO/target"
RELEASE="$TARGET/release"

if [ ! -d "$TARGET" ]; then
  echo "there is no $TARGET: nothing to clear"
  exit 0
fi

# True for a name in target/release/ that is not build output.
kept() {
  case "$1" in
    HumanityOS|HumanityOS.source|HumanityOS.started|HumanityOS.incoming|HumanityOS.incoming.sha256) return 0 ;;
  esac
  return 1
}

before_kb="$(du -sk "$TARGET" 2>/dev/null | cut -f1)"
removed=()
failed=()
# Every entry of a folder, dot names included (the globs that match nothing stay literal and are
# skipped by the -e test).
entries() { for p in "$1"/* "$1"/.[!.]* "$1"/..?*; do if [ -e "$p" ] || [ -L "$p" ]; then printf '%s\n' "$p"; fi; done; }

while IFS= read -r p; do
  name="${p##*/}"
  if [ "$name" = release ] && [ -d "$p" ] && [ ! -L "$p" ]; then continue; fi
  if rm -rf -- "$p"; then removed+=("$name"); else failed+=("$name"); fi
done < <(entries "$TARGET")

if [ -d "$RELEASE" ]; then
  while IFS= read -r p; do
    name="${p##*/}"
    if kept "$name"; then continue; fi
    if rm -rf -- "$p"; then removed+=("release/$name"); else failed+=("release/$name"); fi
  done < <(entries "$RELEASE")
fi

if [ "${#removed[@]}" -eq 0 ] && [ "${#failed[@]}" -eq 0 ]; then
  echo "$TARGET holds only the relay program and its markers: nothing to clear"
  exit 0
fi
after_kb="$(du -sk "$TARGET" 2>/dev/null | cut -f1)"
freed_mb=$(( (${before_kb:-0} - ${after_kb:-0}) / 1024 ))
line="cleared ${#removed[@]} build leftovers from $TARGET (${freed_mb} MB): ${removed[*]}; kept the relay program and its markers"
if [ "${#failed[@]}" -gt 0 ]; then
  line="$line; could not remove: ${failed[*]}"
fi
echo "$line"
exit 0
