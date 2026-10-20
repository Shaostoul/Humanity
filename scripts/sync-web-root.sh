#!/bin/bash
# =============================================================================
# sync-web-root.sh — lay out the public web root from a repo checkout.
#
# THE ONE definition of how /var/www/humanity is structured. Extracted from
# .github/workflows/deploy.yml (2026-08-11) because the provision script had
# re-implemented it naively and served a 404 homepage: two copies of layout
# logic is exactly the drift class the 2026-08-07 incident documents. CI's
# deploy step and provision-vps.sh BOTH call this; neither carries its own.
#
# Usage: sync-web-root.sh [repo_dir] [web_root]
# =============================================================================
set -euo pipefail

SRC="${1:-/opt/Humanity}"
DST="${2:-/var/www/humanity}"

mkdir -p "$DST"

# Chat client (its own dir, deletions tracked - stale chat JS is a live bug).
rsync -a --delete "$SRC/web/chat/" "$DST/chat/"
rsync -a "$SRC/web/shared/" "$DST/shared/"
rsync -a "$SRC/assets/" "$DST/assets/"

# Orphaned pages from retired designs. The pages copy below has no --delete
# (the web root also hosts releases/ and uploads/ that no rsync owns), so
# retired filenames must be removed by name or they serve forever.
for old in skills equipment quests logbook vault knowledge systems streams studio learn resources agents \
           ai-usage audit dashboard data landing projects; do
  rm -f "$DST/${old}.html"
done
rm -f "$DST/pages/resources-app.js"
# Retired 2026-10-20: the Tools page became Apps (apps.html), the Media page became Files
# (files.html) and the Shared Files page became one of its places. Without these lines the old
# pages kept loading on the live site and called routes that no longer exist.
for old in tools media shared-files; do
  rm -f "$DST/${old}.html"
done
rm -f "$DST/pages/tools-app.js" "$DST/shared/media.css"
rm -f "$DST"/pages/media-*.js
# Retired 2026-07-30: three catalogs merged into data/external/catalog.json.
rm -f "$DST/data/resources.json" "$DST/data/resources/catalog.json" "$DST/data/tools/catalog.json"

# Standalone pages: HTML flattened to the root (the site's URLs are
# /profile.html style), their JS under pages/.
mkdir -p "$DST/pages"
for f in "$SRC"/web/pages/*.html; do
  [ -f "$f" ] && cp "$f" "$DST/$(basename "$f")"
done
for f in "$SRC"/web/pages/*.js; do
  [ -f "$f" ] && cp "$f" "$DST/pages/$(basename "$f")"
done

# Root-level files the pages declare by ABSOLUTE path: /favicon.svg, /favicon.png,
# /favicon.ico. Nothing above copies web/*.* at the top level (the loops take
# web/pages/, web/shared/, web/chat/, web/home/), so these shipped in the repo
# and 404'd on every page of the site anyway. Caught 2026-08-25 by fetching them
# against the LIVE origin after a green deploy, which is the only way a
# never-copied file shows up: the repo looks correct and CI looks correct.
for f in "$SRC"/web/favicon.*; do
  [ -f "$f" ] && cp "$f" "$DST/$(basename "$f")"
done

# The homepage IS pages/index.html; the loop above already placed it at the
# root. Assert rather than assume - a 404 landing page is the whole reason
# this script exists.
[ -f "$DST/index.html" ] || { echo "!! no index.html landed at the web root"; exit 1; }

# Homepage FLAVORS (web/home/): served under /home/ so an operator can pick
# one for the per-node override exactly as SELF-HOSTING.md documents:
#   ln -sfn /var/www/humanity/home/technical.html /var/www/humanity-site/index.html
# A LINK, so the front page follows every update; a copy is frozen the day it
# is made (public.guide served a months-old copy until 2026-10-12).
# (v0.1144 fix: the flavors shipped in-repo since v0.1141 but this script
# never placed them, so the documented command had nothing to point at on a
# fresh deploy.) Note /home is the Home PAGE route; nginx's $uri.html try_files
# resolves the file before the directory, so home.html keeps winning.
if [ -d "$SRC/web/home" ]; then
  mkdir -p "$DST/home"
  rsync -a "$SRC/web/home/" "$DST/home/"
fi

for dir in activities; do
  if [ -d "$SRC/web/$dir" ]; then
    mkdir -p "$DST/$dir"
    rsync -a "$SRC/web/$dir/" "$DST/$dir/"
  fi
done

# The game data's JSON files, and ONLY those the repo tracks (2026-10-20). On the server $SRC/data
# is also the RUNNING relay's data folder, so copying every *.json under it (as this did until
# then) would publish anything the relay writes there as JSON: a members-only upload of a .json
# file, an upload waiting to be published, a fresh server's files. The repo's list is the game
# data the website is meant to show; nothing the server makes at run time is on it.
mkdir -p "$DST/data"
# safe.directory: on a self-hosted box the checkout can belong to the relay's user while this runs
# as root, and git refuses such a checkout without it (relay-follow-releases.sh does the same).
src_git() { git -c safe.directory="$SRC" -C "$SRC" "$@"; }
if ! src_git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "sync-web-root: $SRC is not a git checkout, so the game data's JSON cannot be told from the server's own files; refusing to copy data/" >&2
  exit 1
fi
src_git ls-files -z -- 'data/*.json' | rsync -a --from0 --files-from=- "$SRC/" "$DST/"
# The Library ships its markdown so the web library renders the SAME documents
# the native Library reads from disk.
rsync -a --delete "$SRC/data/library/" "$DST/data/library/"

echo "web root synced: $SRC -> $DST"
