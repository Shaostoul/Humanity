#!/bin/bash
# =============================================================================
# install-server-files.sh: install the release's own copy of every file a
# server runs from outside its checkout (scripts/server-files.txt), so no
# server can keep a stale one.
#
# Why (BUG-252, 2026-11-05): public.guide restarted every 30 minutes for two
# days. Its /usr/local/bin/humanity-backup-db was a copy installed by hand in
# August that opened the database read-write, and the sqlite3 shell emptied
# the relay's log under it when it closed. BUG-215 had fixed the script in
# the repo two weeks before, and public.guide's checkout carried the fix,
# but relay-follow-releases.sh updated the checkout and never /usr/local/bin.
# The deploy refreshed united-humanity.us's copy with a plain `cp -f`, which
# rewrites the file in place under a run that may be reading it.
#
# What a run does, for each `file` line whose role is `all` or this server's:
#   - the repo's copy missing (an older release): says so, leaves the
#     installed one as it is;
#   - the installed copy byte for byte the same: nothing;
#   - otherwise: installs the repo's copy beside it as <name>.new with the
#     line's mode, then renames it over the old one (one rename: a run of the
#     old script that is reading it keeps reading the old file to its end),
#     and says what it replaced (its md5 and date) and with what.
# If it changed a systemd unit it runs `systemctl daemon-reload`, so the new
# unit is the one the next timer starts. It never enables or starts a unit:
# the deploy, provision-vps.sh and the first install do that.
#
# Usage: install-server-files.sh <repo> <role>
#   repo  the checkout to install from (/opt/Humanity)
#   role  main (the server the deploy updates) or follow (a node that keeps
#         itself on the latest release, relay-follow-releases.sh)
# Settings (environment): INSTALL_ROOT, put in front of every installed path
# (empty on a server; scripts/tests/server-files.test.js installs into a
# scratch tree with it), and MANIFEST (<repo>/scripts/server-files.txt).
# Exits 1 if a file could not be installed (the others are still tried),
# 2 on a usage error or a missing list, otherwise 0.
# =============================================================================
set -u

REPO="${1:-}"
ROLE="${2:-}"
INSTALL_ROOT="${INSTALL_ROOT:-}"
say() { echo "[server files] $*"; }

if [ -z "$REPO" ] || [ -z "$ROLE" ]; then
  say "usage: install-server-files.sh <repo> <role: main or follow>"
  exit 2
fi
case "$ROLE" in
  main|follow) ;;
  *) say "unknown role '$ROLE' (main or follow)"; exit 2 ;;
esac
MANIFEST="${MANIFEST:-$REPO/scripts/server-files.txt}"
if [ ! -f "$MANIFEST" ]; then
  say "no list of server files at $MANIFEST; nothing installed"
  exit 2
fi

# A file's md5, the first 8 characters (enough to tell copies apart in a log line).
short_md5() { md5sum "$1" 2>/dev/null | cut -c1-8; }

changed=0 current=0 failed=0 units=0
# (`|| [ -n "$kind" ]`: a last line without its newline is read too.)
while read -r kind role mode src dest _rest || [ -n "${kind:-}" ]; do
  # Comments, blank lines, and the `server` lines (for `just brief`, not for this script).
  [ "$kind" = file ] || continue
  dest="${dest%$'\r'}"
  if [ "$role" != all ] && [ "$role" != "$ROLE" ]; then continue; fi
  if [ -z "$dest" ] || [ "${dest#/}" = "$dest" ] || ! [[ "$mode" =~ ^[0-7]{3,4}$ ]]; then
    say "a line of $MANIFEST does not read as 'file <role> <mode> <source> </installed/path>': $kind $role $mode $src $dest"
    failed=$((failed + 1))
    continue
  fi
  from="$REPO/$src"
  to="$INSTALL_ROOT$dest"
  if [ ! -f "$from" ]; then
    say "this release has no $src; $dest is left as it is"
    continue
  fi
  if [ -f "$to" ] && cmp -s "$from" "$to"; then
    current=$((current + 1))
    continue
  fi
  if [ -f "$to" ]; then
    was="md5 $(short_md5 "$to"), dated $(date -u -r "$to" '+%Y-%m-%d' 2>/dev/null || echo '?')"
  else
    was=""
  fi
  # umask 022: the folder and the file are readable by everyone, as /usr/local/bin is.
  if (umask 022; mkdir -p "$(dirname "$to")") \
     && install -m "$mode" "$from" "$to.new" \
     && mv -f "$to.new" "$to"; then
    changed=$((changed + 1))
    if [ -n "$was" ]; then
      say "replaced $dest ($was) with this release's copy (md5 $(short_md5 "$to"))"
    else
      say "installed $dest (there was none) from $src"
    fi
    case "$dest" in /etc/systemd/system/*) units=1 ;; esac
  else
    rm -f "$to.new" 2>/dev/null
    say "could not install $src at $dest; the copy there is unchanged"
    failed=$((failed + 1))
  fi
done < "$MANIFEST"

if [ "$units" = 1 ]; then
  if systemctl daemon-reload; then
    say "systemd reloaded, so the new units are the ones the timers start"
  else
    say "could not reload systemd; the changed units take effect at the next daemon-reload"
    failed=$((failed + 1))
  fi
fi
say "$changed installed or replaced, $current already the release's copy, $failed failed (role $ROLE)"
[ "$failed" = 0 ]
