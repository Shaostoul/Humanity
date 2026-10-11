#!/bin/bash
# =============================================================================
# relay-follow-releases.sh: keep a self-hosted relay on the latest release.
#
# Why (BUG-192, 2026-10-11): public.guide, the operator's second relay, was
# updated by hand and sat on v0.1147.0 for two months. When server keys moved
# to Dilithium3 it could no longer talk to united-humanity.us, and chat between
# the two was down for five weeks without anyone noticing. The deploy workflow
# updates only the main server; any other relay has to keep itself current.
#
# Where it lives: /usr/local/lib/humanity/relay-follow-releases.sh, OUTSIDE the
# repo, run as root by humanity-relay-follow.service (from the hourly timer).
# A copy inside the checkout would be deleted by a roll back to a release that
# did not carry it yet. After every successful update the repo's copy is
# installed over that one, so fixes to this script reach the node by
# themselves; a roll back never touches it.
#
# What a run does:
#   0. If the last run stopped partway through an update (killed by the unit's
#      time limit, a reboot, anything), it rolls that update back first from
#      the record the run left in STATE_DIR, marks that release skipped and
#      stops. The new program may be running and hung, and the saved previous
#      program is the only copy of the old one, so nothing else happens first.
#      The database goes back only if the new release had started on it.
#   1. Asks GitHub for the latest release (never a pre-release or a draft) and
#      goes on only if it is NEWER than the installed one. GitHub's "Latest"
#      pointer has moved backwards before (a stray release took it once), so
#      an older one is never installed.
#   2. Downloads HumanityOS-relay-linux-x64 and its .sha256 from that release
#      and refuses a file whose hash does not match (the same check
#      provision-vps.sh makes).
#   3. Fetches the release's tag and checks, without changing anything, that
#      the node's own edits to tracked files (data/server-config.json, say)
#      fit onto the release. If they do not, it changes nothing, names the
#      files, skips that release and exits 1. A file-mode-only difference is
#      not an edit (the watchdog script's executable bit is one, on
#      public.guide).
#   4. Backs up the database (sqlite3 .backup), the running program, the
#      node's edits and any local file the release would overwrite, then
#      writes the pending record.
#   5. Checks the repo out at the release (for its data files and scripts),
#      puts the node's edits back on top, installs the program, restarts the
#      relay and waits for /health.
#   6. Healthy: records the release, lays out the web root the way the deploy
#      does, and installs the release's copy of this script. Not healthy
#      within about a minute: puts back the previous program, the previous
#      checkout with the node's edits, and the database (with the owner and
#      mode the live file had: the relay runs as its own user and must still
#      be able to write it), restarts, and exits 1 so the journal and the
#      unit's status say so. A rolled-back release waits for a newer one.
#
# Every curl call has a time limit, so a release that accepts a connection and
# then hangs cannot hold the run until systemd kills it. Backups are root-only
# (umask 077, STATE_DIR 700). git runs with safe.directory set, because the
# checkout belongs to the relay's user and git run as root refuses it
# otherwise ("dubious ownership"), and under umask 022, so the files it writes
# stay readable by that user. One run at a time (flock): a second run would
# take this one's pending record for a dead run's.
#
# Settings (environment, all optional): REPO (/opt/Humanity), WEB_ROOT
# (/var/www/humanity), SERVICE (humanity-relay), PORT (3210),
# STATE_DIR (/var/lib/humanity-follow), KEEP_BACKUPS (3),
# INSTALLED_COPY (/usr/local/lib/humanity/relay-follow-releases.sh),
# LATEST_API and DOWNLOAD_BASE (GitHub's), HEALTH_TRIES (30) and
# HEALTH_PAUSE (2 seconds). scripts/tests/relay-follow.test.js runs this
# script against a scratch repo and a stand-in GitHub through them.
# =============================================================================
set -u
umask 077

REPO="${REPO:-/opt/Humanity}"
WEB_ROOT="${WEB_ROOT:-/var/www/humanity}"
SERVICE="${SERVICE:-humanity-relay}"
PORT="${PORT:-3210}"
STATE_DIR="${STATE_DIR:-/var/lib/humanity-follow}"
KEEP_BACKUPS="${KEEP_BACKUPS:-3}"
INSTALLED_COPY="${INSTALLED_COPY:-/usr/local/lib/humanity/relay-follow-releases.sh}"
LATEST_API="${LATEST_API:-https://api.github.com/repos/Shaostoul/Humanity/releases/latest}"
DOWNLOAD_BASE="${DOWNLOAD_BASE:-https://github.com/Shaostoul/Humanity/releases/download}"
HEALTH_TRIES="${HEALTH_TRIES:-30}"
HEALTH_PAUSE="${HEALTH_PAUSE:-2}"

BIN="$REPO/target/release/HumanityOS"
DB="$REPO/data/relay.db"
WATCHDOG="$REPO/scripts/humanity-relay-watchdog.sh"
PENDING="$STATE_DIR/pending"
STUCK="$STATE_DIR/pending.failed"
PREV_BIN="$STATE_DIR/HumanityOS.previous"
SKIPPED="$STATE_DIR/skipped_tag"
INSTALLED="$STATE_DIR/installed_tag"

say() { echo "[follow] $*"; }

# git on the checkout. safe.directory: the checkout is the relay user's and this runs as root.
# core.fileMode=false: a local executable bit is not a local edit. umask 022: files git writes
# into the checkout must stay readable by the relay's user (this script's own umask is 077).
g() { (umask 022; git -c safe.directory="$REPO" -c core.fileMode=false -C "$REPO" "$@"); }

# True when version $1 is newer than version $2 ("v0.1486.0" and "0.1485.1" both read as versions).
newer() {
  local a="${1#v}" b="${2#v}"
  [ "$a" != "$b" ] && [ "$(printf '%s\n%s\n' "$a" "$b" | sort -V | tail -n 1)" = "$a" ]
}

# /health within HEALTH_TRIES tries. Each try may take 5 seconds at most.
healthy() {
  local _
  for _ in $(seq 1 "$HEALTH_TRIES"); do
    if curl -fs --max-time 5 "http://127.0.0.1:$PORT/health" >/dev/null 2>&1; then return 0; fi
    sleep "$HEALTH_PAUSE"
  done
  return 1
}

# The watchdog's unit runs its script directly, so it must stay executable whatever mode the
# checked-out release gave it.
watchdog_runnable() {
  if [ -f "$WATCHDOG" ]; then chmod +x "$WATCHDOG" || say "could not make $WATCHDOG executable"; fi
}

# Keep only the newest KEEP_BACKUPS of one kind of backup in STATE_DIR.
prune() {
  ls -1dt "$STATE_DIR"/$1 2>/dev/null | tail -n +"$((KEEP_BACKUPS + 1))" | xargs -r rm -rf
}

# One value from the pending record (lines of key=value).
recorded() { sed -n "s/^$1=//p" "$PENDING" 2>/dev/null | tail -n 1; }

# Put back what the pending record says was there before the update, mark the release skipped,
# and clear the record. A roll back that cannot finish leaves the record as pending.failed, and
# every later run stops at that until a person has looked.
roll_back() {
  local tag from prev_ref db_backup db_owner db_mode edits files restarted ok=1
  tag="$(recorded tag)"; from="$(recorded from)"; prev_ref="$(recorded prev_ref)"
  db_backup="$(recorded db_backup)"; db_owner="$(recorded db_owner)"; db_mode="$(recorded db_mode)"
  edits="$(recorded local_edits)"; files="$(recorded local_files)"; restarted="$(recorded restarted)"

  systemctl stop "$SERVICE" || say "could not stop $SERVICE; going on with the roll back"
  if [ -f "$PREV_BIN" ] && install -m 755 "$PREV_BIN" "$BIN"; then
    say "put the previous relay program back"
  else
    say "could not put the previous relay program back from $PREV_BIN"; ok=0
  fi
  # The database goes back only if the new release ran on it (it may have migrated it). Before
  # that, the old relay was still the one writing, and its newest writes are in the live file.
  if [ "$restarted" = 1 ]; then
    if [ -f "$db_backup" ] && rm -f "$DB-wal" "$DB-shm" \
       && install -o "${db_owner%%:*}" -g "${db_owner#*:}" -m "$db_mode" "$db_backup" "$DB"; then
      say "put the database back from $db_backup (owner $db_owner, mode $db_mode, as the live file had)"
    else
      say "could not put the database back from $db_backup"; ok=0
    fi
  fi
  if g checkout -q -f "$prev_ref"; then
    if [ -n "$files" ] && [ -d "$files" ]; then
      cp -a "$files/." "$REPO/" || { say "could not put the local files back from $files"; ok=0; }
    fi
    if [ -n "$edits" ] && [ -s "$edits" ]; then
      g apply "$edits" || { say "could not put the local edits back; they are saved in $edits"; ok=0; }
    fi
    say "the repo is back at ${from:-$prev_ref}"
  else
    say "could not check the repo out at $prev_ref"; ok=0
  fi
  watchdog_runnable
  systemctl start "$SERVICE" || { say "could not start $SERVICE"; ok=0; }
  if healthy; then
    say "the relay is healthy on ${from:-the previous version}"
  else
    say "the relay is NOT healthy after the roll back either; see journalctl -u $SERVICE"
  fi
  echo "$tag" > "$SKIPPED"
  if [ "$ok" = 1 ]; then
    rm -f "$PENDING"
    prune 'relay.db.before-*'; prune 'local-edits.before-*'; prune 'local-files.before-*'
    say "rolled back to ${from:-the previous version}; $tag is skipped until a newer release (delete $SKIPPED to try it again)"
  else
    mv -f "$PENDING" "$STUCK"
    say "the roll back did not finish; what it was putting back is listed in $STUCK. Nothing is updated until a person has fixed it and deleted that file"
  fi
}

mkdir -p "$STATE_DIR"
# Backups hold the whole database: root only, including any made before this was tightened.
chmod 700 "$STATE_DIR" && chmod -R go-rwx "$STATE_DIR"

# One run at a time: a second run would take this run's pending record for a dead one.
if command -v flock >/dev/null 2>&1; then
  exec 9>>"$STATE_DIR/lock"
  if ! flock -n 9; then
    say "another run is in progress; leaving it to finish"
    exit 0
  fi
fi

if [ -e "$STUCK" ]; then
  say "a roll back here did not finish; $STUCK lists what it was putting back. Nothing is updated until a person has fixed it and deleted that file"
  exit 1
fi

# 0. The last run stopped partway through an update: undo it before anything else.
if [ -e "$PENDING" ]; then
  say "the last run stopped partway through updating to $(recorded tag) (killed, timed out, or the machine restarted): rolling back from $PENDING"
  # A git command killed partway leaves its lock behind, and every git command after it would refuse.
  if [ -e "$REPO/.git/index.lock" ]; then
    say "removing the lock git left in $REPO/.git when that run was stopped"
    rm -f "$REPO/.git/index.lock"
  fi
  roll_back
  exit 1
fi

case "$(readlink -f "$0" 2>/dev/null || echo "$0")" in
  "$REPO"/*) say "this copy runs from inside $REPO, where a roll back to an older release can delete it; install it at $INSTALLED_COPY and point the unit there (docs/admin/SELF-HOSTING.md)" ;;
esac

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# 1. The latest release's tag. /releases/latest never returns a pre-release or a draft.
latest="$(curl -fsSL --max-time 30 -H 'Accept: application/vnd.github+json' "$LATEST_API" | grep -o '"tag_name": *"[^"]*"' | head -1 | cut -d'"' -f4)"
if [ -z "$latest" ]; then
  say "could not read the latest release from GitHub; trying again next time"
  exit 0
fi
if ! [[ "$latest" =~ ^v?[0-9][0-9A-Za-z.+-]*$ ]]; then
  say "GitHub's latest release has a tag that does not look like a version ($latest); not updating"
  exit 1
fi

# What is installed: the recorded tag, else the tag the checkout sits on.
installed="$(cat "$INSTALLED" 2>/dev/null || true)"
if [ -z "$installed" ]; then
  installed="$(g describe --tags --exact-match 2>/dev/null || true)"
fi
if [ -n "$installed" ] && ! newer "$latest" "$installed"; then
  if [ "${latest#v}" = "${installed#v}" ]; then
    say "already on $latest"
    echo "$latest" > "$INSTALLED"
  else
    say "GitHub's latest release, $latest, is older than $installed here; not going backwards"
  fi
  exit 0
fi
# A release that failed here and was rolled back waits for a newer one, instead of being tried
# (and rolled back) every hour.
if [ "$latest" = "$(cat "$SKIPPED" 2>/dev/null || true)" ]; then
  say "$latest was rolled back here before; waiting for a newer release"
  exit 0
fi
say "updating ${installed:-an unknown version} to $latest"

# 2. Download and verify.
url="$DOWNLOAD_BASE/$latest/HumanityOS-relay-linux-x64"
if ! curl -fsSL --max-time 600 "$url" -o "$tmp/relay" || ! curl -fsSL --max-time 30 "$url.sha256" -o "$tmp/relay.sha256"; then
  say "could not download the relay for $latest; trying again next time"
  exit 0
fi
want="$(awk '{print $1}' "$tmp/relay.sha256")"
got="$(sha256sum "$tmp/relay" | awk '{print $1}')"
if [ -z "$want" ] || [ "$want" != "$got" ]; then
  say "checksum mismatch on the relay for $latest (expected $want, got $got): refusing it"
  exit 1
fi

# 3. The release's tag, and whether this node's own edits fit onto it. Nothing changes here.
if ! (umask 022; timeout 300 git -c safe.directory="$REPO" -C "$REPO" fetch -q --tags origin); then
  say "could not fetch the release tags into $REPO; not updating"
  exit 1
fi
if ! g rev-parse -q --verify "refs/tags/$latest^{commit}" >/dev/null; then
  say "the repo has no tag $latest after fetching; not updating"
  exit 1
fi
prev_ref="$(g rev-parse HEAD)"
if [ -z "$prev_ref" ]; then
  say "could not read where the repo is checked out; not updating"
  exit 1
fi
g diff --binary HEAD > "$tmp/local-edits.patch"
if [ -s "$tmp/local-edits.patch" ]; then
  # Apply the edits onto the release in a scratch index (a three-way merge, as git would do it);
  # the checkout and its own index are not touched.
  if ! GIT_INDEX_FILE="$tmp/release.index" g read-tree "$latest" \
     || ! GIT_INDEX_FILE="$tmp/release.index" g apply --cached --3way "$tmp/local-edits.patch" >"$tmp/apply.log" 2>&1; then
    conflicts="$(GIT_INDEX_FILE="$tmp/release.index" g ls-files -u 2>/dev/null | cut -f2 | sort -u | tr '\n' ' ')"
    why="the release changed the same lines"
    if [ -z "$conflicts" ]; then
      conflicts="$(g apply --numstat "$tmp/local-edits.patch" 2>/dev/null | cut -f3 | tr '\n' ' ')"
      why="git said: $(tr '\n' ' ' < "$tmp/apply.log" 2>/dev/null)"
    fi
    say "the local edits to ${conflicts% } do not fit onto $latest (${why% }); not updating. Nothing was changed and the edits are still there. Merge them by hand, then delete $SKIPPED to try $latest again; until then it is skipped"
    echo "$latest" > "$SKIPPED"
    exit 1
  fi
fi

# 4. Backups: the database, the program, the node's edits, and local files the release would
# overwrite (a file it adds that already exists here, untracked).
if [ -e "$PENDING" ]; then
  # Never reached (handled above), but the saved previous program is the only copy of the old
  # one while a record exists, so it is never overwritten then.
  say "a pending record exists; not touching the saved previous program"
  exit 1
fi
if [ ! -f "$DB" ]; then
  say "there is no database at $DB to back up; not updating"
  exit 1
fi
if [ ! -f "$BIN" ]; then
  say "there is no relay program at $BIN to keep for a roll back; not updating"
  exit 1
fi
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
db_owner="$(stat -c '%u:%g' "$DB")"
db_mode="$(stat -c '%a' "$DB")"
db_backup="$STATE_DIR/relay.db.before-$latest-$stamp"
if ! sqlite3 "$DB" ".backup '$db_backup'"; then
  say "could not back up the database; not updating"
  exit 1
fi
# SQLite run as root can leave a -wal or -shm of its own; they belong to the relay's user.
for f in "$DB-wal" "$DB-shm"; do
  if [ -e "$f" ]; then chown "$db_owner" "$f" || say "could not give $f back to $db_owner"; fi
done
# A plain cp, so the copy is root-only under this run's umask; a roll back installs it as 755.
if ! cp "$BIN" "$PREV_BIN"; then
  say "could not keep a copy of the running program; not updating"
  exit 1
fi
edits=""
if [ -s "$tmp/local-edits.patch" ]; then
  edits="$STATE_DIR/local-edits.before-$latest-$stamp.patch"
  cp "$tmp/local-edits.patch" "$edits"
fi
files=""
while IFS= read -r f; do
  if [ -z "$f" ] || [ ! -e "$REPO/$f" ]; then continue; fi
  if [ "$(g hash-object -- "$f" 2>/dev/null)" = "$(g rev-parse -q --verify "$latest:$f" 2>/dev/null)" ]; then continue; fi
  files="$STATE_DIR/local-files.before-$latest-$stamp"
  mkdir -p "$files/$(dirname "$f")" && cp -a "$REPO/$f" "$files/$f"
  say "$f exists here and $latest adds its own; keeping this node's copy in $files/$f"
done < <(g -c core.quotePath=false diff --name-only --diff-filter=A HEAD "$latest")

{
  echo "tag=$latest"
  echo "from=$installed"
  echo "prev_ref=$prev_ref"
  echo "db_backup=$db_backup"
  echo "db_owner=$db_owner"
  echo "db_mode=$db_mode"
  echo "local_edits=$edits"
  echo "local_files=$files"
} > "$PENDING.new" && mv -f "$PENDING.new" "$PENDING"

# 5. The release's files with the node's edits on top, the new program, a restart, /health.
if ! g checkout -q -f "$latest"; then
  say "could not check the repo out at $latest: rolling back"
  roll_back
  exit 1
fi
watchdog_runnable
if [ -n "$edits" ]; then
  if g apply --3way "$edits" 2>"$tmp/apply.log" && g reset -q; then
    say "carried the local edits to $(g apply --numstat "$edits" | cut -f3 | tr '\n' ' ')onto $latest"
  else
    say "could not put the local edits onto $latest after all ($(tr '\n' ' ' < "$tmp/apply.log")): rolling back"
    roll_back
    exit 1
  fi
fi
if ! install -m 755 "$tmp/relay" "$BIN"; then
  say "could not install the new relay program: rolling back"
  roll_back
  exit 1
fi
echo "restarted=1" >> "$PENDING"
systemctl restart "$SERVICE"

if ! healthy; then
  say "the relay did not come back healthy on $latest: rolling back"
  roll_back
  exit 1
fi

# 6. Healthy. The record goes first, so a run killed during the steps below is not taken for a
# failed update.
echo "$latest" > "$INSTALLED"
rm -f "$PENDING"
say "updated to $latest; the relay is healthy"
if [ -n "$files" ]; then
  say "this node's own copies of files $latest now carries are in $files"
fi
# The web root, laid out the way the deploy does it, readable by the web server.
if [ -d "$WEB_ROOT" ] && [ -f "$REPO/scripts/sync-web-root.sh" ]; then
  (umask 022; bash "$REPO/scripts/sync-web-root.sh" "$REPO" "$WEB_ROOT") || say "the web root sync failed (the relay itself is updated)"
fi
# This release's copy of the follower, so fixes to it reach this node. Installed by rename: the
# copy bash is reading right now is never rewritten under it.
if [ -f "$REPO/scripts/relay-follow-releases.sh" ] && ! cmp -s "$REPO/scripts/relay-follow-releases.sh" "$INSTALLED_COPY"; then
  if mkdir -p "$(dirname "$INSTALLED_COPY")" \
     && install -m 755 "$REPO/scripts/relay-follow-releases.sh" "$INSTALLED_COPY.new" \
     && mv -f "$INSTALLED_COPY.new" "$INSTALLED_COPY"; then
    say "installed $latest's copy of this script at $INSTALLED_COPY"
  else
    say "could not install $latest's copy of this script at $INSTALLED_COPY"
  fi
fi
prune 'relay.db.before-*'; prune 'local-edits.before-*'; prune 'local-files.before-*'
exit 0
