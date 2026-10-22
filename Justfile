# HumanityOS — dev command runner
# Install: winget install Casey.Just
# Usage:   just <recipe>   (run from anywhere inside the repo)
#
# ── QUICK REFERENCE ──────────────────────────────────────────────────────────
#   just ship "msg"     Commit + push + force-sync VPS  ← use this every day
#   just sync           Force-sync VPS now (no commit)  ← when CI breaks
#   just sync-web       Sync HTML/JS/CSS only, fast     ← for front-end only
#   just logs           Tail live server logs
#   just status         Git + CI + live API health
#   just ping           Is the site alive?
#   just tasks          List tasks from live API
#   just check          Catch Rust errors before shipping
#   just bump           Bump version (patch/minor/major)
# ─────────────────────────────────────────────────────────────────────────────

set shell := ["C:/Apps/Git/usr/bin/bash.exe", "-c"]

# List all recipes
default:
    @just --list

# ══════════════════════════════════════════════════════════════════════════════
# VERSION — automated version bumping
# ══════════════════════════════════════════════════════════════════════════════

# Bump version across all 6 locations (patch/minor/major, default: patch)
# Usage: just bump          → 0.5.2 → 0.5.3
#        just bump minor    → 0.5.2 → 0.6.0
#        just bump major    → 0.5.2 → 1.0.0
bump kind="patch":
    node scripts/bump-version.js {{kind}}

# ══════════════════════════════════════════════════════════════════════════════
# DEPLOY — getting code to production
# ══════════════════════════════════════════════════════════════════════════════

# THE ONE COMMAND: bump + regen theme + bundle web + commit + push + force-sync VPS
ship msg="chore: update":
    @if git diff --cached --quiet; then just _nothing-staged; exit 1; fi
    @just bump
    @just theme
    @just bundle-web
    @just _stage-generated
    @just _commit "{{msg}}"
    @just sync
    -@node scripts/check-delivery.js --warn

# Internal: stage the files `ship` itself generates (version stamps, theme CSS,
# the bundled app/web tree). These are ship's own output, so they belong in the
# commit, but they must be added by NAME rather than by `git add -A`, which
# would also sweep in whatever the other sessions have open.
_stage-generated:
    -@git add -- Cargo.toml web/shared/sw.js web/pages/settings-app.js web/pages/ops.html web/shared/shell.js web/pages/download.html web/pages/index.html 2>/dev/null
    -@git add -- web/shared/theme.css web/shared/theme-presets.js 2>/dev/null
    -@git add -- app/web 2>/dev/null

# Same as `ship`, but stages EVERYTHING in the tree first (the pre-2026-07-30
# behaviour). Only correct when you are the only one working in this checkout.
# With several Claude sessions sharing the directory this commits their
# half-finished work under your message, so reach for plain `just ship` and
# stage your own files instead.
ship-all msg="chore: update":
    git add -A
    @just ship "{{msg}}"

# Print the lane map: who owns what while multiple sessions share this checkout.
# Read this before editing outside your lane, and before staging anything.
lanes:
    @node scripts/lanes.js

# Stage exactly the paths you name, show what you staged, and stop. Commit with
# `just ship "msg"` afterwards. The safe way to prepare a commit in a shared tree.
mine +paths:
    git add -- {{paths}}
    @echo "staged:"
    @git diff --cached --name-only | sed 's/^/  /'
    @node scripts/lanes.js --check-staged

# Internal: the message shown when `just ship` finds nothing staged.
# NOTE: single quotes only. Backticks inside a double-quoted echo are COMMAND
# SUBSTITUTION, so an earlier version of this help text containing `just ship`
# actually RAN just ship, recursively, bumping the version five times before it
# was caught.
_nothing-staged:
    @echo ''
    @echo 'x Nothing staged, so there is nothing to ship.'
    @echo ''
    @echo '  just ship commits only what YOU staged. It does not stage the whole'
    @echo '  tree, because other Claude sessions share this checkout and a blanket'
    @echo '  add would sweep their in-flight work into your commit.'
    @echo ''
    @echo '  Stage your own files, then ship:'
    @echo '      just mine src/gui/pages/library.rs web/pages/library.html'
    @echo '      just ship "what you changed and why"'
    @echo ''
    @echo '  Truly want everything in the tree? just ship-all "msg"'
    @echo ''

# Regenerate data/stars.bin (compact binary star catalog, ~1.8 MB) from
# data/stars.csv (HYG, 34 MB). The native renderer parses the .bin at startup;
# the CSV stays as the human-readable source of truth. Self-verifying: the
# script cross-checks sampled stars against the CSV before reporting success.
# Run after ANY edit to data/stars.csv and commit the regenerated .bin.
build-stars:
    node scripts/build-stars-bin.js

# Regenerate data/stars-map.bin (galactic-cartesian star positions in light-years,
# ~2.1 MB) from data/stars.csv. This is the Maps page GALAXY VIEW catalog, distinct
# from data/stars.bin (skybox directions, `just build-stars`). Self-verifying: the
# script re-opens the file it wrote and runs 31 structural + astronomical checks
# (Sirius/Polaris/Rigil Kentaurus positions, sort order, galactic-plane sanity).
# Run after ANY edit to data/stars.csv and commit the regenerated .bin.
build-stars-map:
    node scripts/build-stars-map-bin.js

# Regenerate web/shared/theme.css + web/shared/settings.js theme-preset block
# from data/gui/theme.ron + data/themes/presets.json (one-source theme rule).
# Idempotent — running twice produces no diff. Wired into `just ship`.
theme:
    node scripts/gen-theme-css.js
    node scripts/gen-theme-presets-js.js

# Cross-language post-quantum known-answer test: the vendored chat
# bundle MUST derive the same Dilithium3 identity as the Rust relay.
# Run before shipping any PQ-touching change.
pq-kat:
    node scripts/pq-kat.mjs

# Cross-language canonical-CBOR / signed-object KAT: asserts the web encoder
# (web/shared/canonical-cbor.js) builds byte-identical objects to the Rust relay
# for a fixed group_v1 input. Run before shipping any signed-object change.
group-kat:
    node scripts/group-object-kat.mjs
    node scripts/object-verify-kat.mjs

# Cross-language vote_v1 KAT: the web governance page's Dilithium-signed votes
# (web/pages/governance.html via pq-object.js buildVoteV1) must be byte-identical
# to the Rust relay encoder. Pairs with object.rs::vote_v1_cross_language_kat.
vote-kat:
    node scripts/vote-object-kat.mjs

# Cross-language moderation KAT: mod_action_v1 + space_policy_v1. The MODERATOR'S
# CLIENT signs these (the relay cannot hold a moderator key), so a browser and the
# Rust relay must encode them byte-identically or browser-signed moderation is
# unverifiable. Pairs with moderation.rs::cross_language_kat.
mod-kat:
    node scripts/mod-object-kat.mjs

# Rebuild the vendored same-origin post-quantum bundle (pins versions,
# bundles with esbuild, verifies the KAT). Needs network for npm.
pq-vendor:
    node scripts/build-noble-bundle.mjs

# Commit + push only — waits for CI to deploy (~5 min)
deploy msg="chore: update":
    @just _commit "{{msg}}"
    @echo ""
    @echo "✓ Pushed. CI deploy triggered — watch with: just ci"

# Internal: commit what is ALREADY STAGED, then push (skips if nothing staged).
#
# This deliberately does NOT `git add -A`. Several Claude sessions share this one
# working directory, so a blanket add sweeps every other session's in-flight work
# into your commit. That happened three times on 2026-07-30: another lane's edits
# shipped under an unrelated commit message, and one of those sweeps came within a
# minute of committing a renderer mid-refactor that did not compile. Stage your own
# files (`git add <paths>`), then ship. Use `just ship-all` if you genuinely want
# everything in the tree.
# Pushes to `origin` (GitHub: visibility, CI, releases API). The self-hosted
# mirror at git.united-humanity.us is a PULL mirror since 2026-09-29: it fetches
# from GitHub every 8 hours by itself and refuses pushes, so nothing here pushes
# to it (the old push-with-errors-ignored let it sit dead for eight weeks;
# `just brief`'s MIRROR row shows its last sync). Tags trigger the Build Desktop
# App workflow on GitHub, which produces the platform binaries linked from
# download.html.
_commit msg:
    @if git diff --cached --quiet; then just _nothing-staged; exit 1; fi
    @node scripts/lanes.js --check-staged
    @printf '%s' "{{msg}}" > .git/COMMIT_HUMANITY_MSG
    git diff --cached --quiet || git commit -F .git/COMMIT_HUMANITY_MSG
    @rm -f .git/COMMIT_HUMANITY_MSG
    git push origin main
    @node -e "const fs=require('fs'),{execSync}=require('child_process');const v=fs.readFileSync('Cargo.toml','utf8').match(/^version = \"(.+)\"/m)[1];const tag='v'+v;try{execSync('git rev-parse '+tag,{stdio:'ignore'});console.log('Tag '+tag+' already exists');}catch(e){try{execSync('git tag '+tag,{stdio:'inherit'});execSync('git push origin '+tag,{stdio:'inherit'});console.log('Tagged and pushed '+tag+' to origin (the forge mirror pulls it)');}catch(err){console.warn('Tag failed: '+err.message);}}"

# Full-PQ cutover: WIPE the live relay DB to a fresh schema (backs up
# first; everyone re-onboards from seed). Guarded — must pass `yes`:
#   just pq-wipe yes
# DO NOT run until the full-PQ stack is shipped + security-reviewed.
pq-wipe CONFIRM="no":
    @if [ "{{CONFIRM}}" != "yes" ]; then echo "Refusing. Run: just pq-wipe yes  (irreversible-ish; backs up first)"; exit 2; fi
    ssh humanity-vps 'cd /opt/Humanity && git fetch origin main -q && git reset --hard -q origin/main && bash scripts/pq-wipe.sh --yes'

# Force-sync VPS with current origin/main right now (full rebuild)
# Use when: CI fails, server is out of sync, or you need it live immediately
sync:
    @echo "→ Syncing VPS (full rebuild)..."
    ssh humanity-vps " \
        set -e && \
        cd /opt/Humanity && \
        git fetch origin main && \
        git reset --hard origin/main && \
        git clean -fd --exclude=backups/ --exclude=data/ --exclude=target/ && \
        ( DISK_PCT=\$(df --output=pcent / | tail -1 | tr -dc '0-9' || echo 0); if [ \"\${DISK_PCT:-0}\" -ge 80 ]; then echo \"disk \${DISK_PCT}%>=80 - clearing regenerable target/, keeping the relay binary\"; bash scripts/clear-build-cache.sh /opt/Humanity; fi; cp -f scripts/systemd/humanity-disk-guard.service /etc/systemd/system/humanity-disk-guard.service; cp -f scripts/systemd/humanity-disk-guard.timer /etc/systemd/system/humanity-disk-guard.timer; chmod +x scripts/humanity-disk-guard.sh; systemctl daemon-reload; systemctl enable --now humanity-disk-guard.timer; if visudo -cf scripts/sudoers.d/humanity-relay-services >/dev/null 2>&1; then install -m 0440 -o root -g root scripts/sudoers.d/humanity-relay-services /etc/sudoers.d/humanity-relay-services; fi; ) || true && \
        export PATH=\$HOME/.cargo/bin:\$PATH && \
        cargo build --release --features relay --no-default-features 2>&1 | tail -4 && \
        rsync -a --delete /opt/Humanity/web/chat/ /var/www/humanity/chat/ && \
        rsync -a /opt/Humanity/web/shared/ /var/www/humanity/shared/ && \
        rsync -a /opt/Humanity/assets/ /var/www/humanity/assets/ && \
        mkdir -p /var/www/humanity/pages /var/www/humanity/activities /var/www/humanity/data && \
        for f in /opt/Humanity/web/pages/*.html; do \
            [ -f \"\$f\" ] && cp \"\$f\" \"/var/www/humanity/\$(basename \"\$f\")\"; \
        done && \
        for f in /opt/Humanity/web/pages/*.js; do \
            [ -f \"\$f\" ] && cp \"\$f\" \"/var/www/humanity/pages/\$(basename \"\$f\")\"; \
        done && \
        ( ! [ -d /opt/Humanity/web/activities ] || rsync -a /opt/Humanity/web/activities/ /var/www/humanity/activities/ ) && \
        rsync -a -m --include='*/' --include='*.json' --exclude='*' /opt/Humanity/data/ /var/www/humanity/data/ && \
        systemctl restart humanity-relay && \
        sleep 2 && systemctl is-active humanity-relay \
    "
    @echo "✓ VPS synced and relay restarted."

# Sync web assets only — skips Rust rebuild (fast, use for HTML/JS/CSS changes)
sync-web:
    @echo "→ Syncing web assets only..."
    ssh humanity-vps " \
        set -e && \
        cd /opt/Humanity && \
        git fetch origin main && \
        git reset --hard origin/main && \
        git clean -fd --exclude=backups/ --exclude=data/ --exclude=target/ && \
        rsync -a --delete /opt/Humanity/web/chat/ /var/www/humanity/chat/ && \
        rsync -a /opt/Humanity/web/shared/ /var/www/humanity/shared/ && \
        rsync -a /opt/Humanity/assets/ /var/www/humanity/assets/ && \
        mkdir -p /var/www/humanity/pages /var/www/humanity/activities /var/www/humanity/data && \
        for f in /opt/Humanity/web/pages/*.html; do \
            [ -f \"\$f\" ] && cp \"\$f\" \"/var/www/humanity/\$(basename \"\$f\")\"; \
        done && \
        for f in /opt/Humanity/web/pages/*.js; do \
            [ -f \"\$f\" ] && cp \"\$f\" \"/var/www/humanity/pages/\$(basename \"\$f\")\"; \
        done && \
        ( ! [ -d /opt/Humanity/web/activities ] || rsync -a /opt/Humanity/web/activities/ /var/www/humanity/activities/ ) && \
        rsync -a -m --include='*/' --include='*.json' --exclude='*' /opt/Humanity/data/ /var/www/humanity/data/ \
    "
    @echo "✓ Web assets synced (relay not restarted)."

# ══════════════════════════════════════════════════════════════════════════════
# STATUS — know what's happening
# ══════════════════════════════════════════════════════════════════════════════

# Full status: recent commits + CI runs + live API health
status:
    @echo "── Recent commits ───────────────────────────"
    @git log --oneline -5
    @echo ""
    @echo "── CI deploy runs ───────────────────────────"
    @gh run list --repo Shaostoul/Humanity --workflow "Deploy to VPS" --limit 5
    @echo ""
    @node scripts/check-release-signing.js 6 || true
    @echo ""
    @echo "── Live site ────────────────────────────────"
    @just ping

# One-shot ORIENTATION for a fresh session (AI or human): local version vs the latest
# release (LOCAL AHEAD flag), the last CI deploy status, release-signing state, and the
# newest journal decision + a session-data staleness check. Run this FIRST -- it
# surfaces in one screen the things the START-HERE reads check for one by one.
brief:
    @node scripts/brief.js

# The code map (docs/ai/code-map.md, 2026-10-18): one line per source file under src/ and web/,
# what it is and its main items, with a folder index first. Generated; `just brief` refreshes it.
code-map:
    @node scripts/code-map.js

# Clear build output before it fills C: (2026-09-29; C: reached 91% with 1.13 TB
# of cargo caches). Idle worktree targets always; the main target's incremental
# cache at 75% full; the whole main target at 85%; never while cargo, rustc or
# the game runs. The installed task runs this hourly; add --dry-run to only report.
disk-guard *args:
    @node scripts/pc-disk-guard.js {{args}}

# Install (or replace) the hourly, silent "HumanityOS Dev Disk Guard" task. One UAC prompt.
install-disk-guard:
    powershell -NoProfile -Command "Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile -ExecutionPolicy Bypass -File \"{{justfile_directory()}}\scripts\install-pc-disk-guard.ps1\"'"

# Delete what test runs left in the temp folder (BUG-159, 2026-10-05): the `hum_*`
# files and folders nothing has touched for a day. The tests delete their own now
# (src/test_temp.rs); this sweeps the old pile and whatever a killed run leaves.
# Refuses while cargo, rustc or a test binary runs. Prints the count and the space.
# --dry-run only reports; --dir <folder> sweeps another folder; --days <n> the age.
clean-test-temp *args:
    @node scripts/clean-test-temp.js {{args}}

# Rotate the orchestrator journal: archive decisions older than the recent `keep` tail
# into docs/history/journal-archive-<month>.md so the session-loaded JSON stays small.
# Order is preserved (newest stays at the bottom). Run at session end when it has grown.
rotate-journal keep="40":
    @node scripts/rotate-journal.js {{keep}}

# Check whether recent releases are signed (the desktop auto-update gate). An unsigned LATEST
# means the updater offers nothing -- fix with `just sign-release vX.Y.Z` (operator only).
check-signing:
    @node scripts/check-release-signing.js

# Check recent CI deploy runs
ci:
    gh run list --repo Shaostoul/Humanity --workflow "Deploy to VPS" --limit 8

# Watch CI run in real time (uses the most recent run)
ci-watch:
    gh run watch --repo Shaostoul/Humanity $(gh run list --repo Shaostoul/Humanity --workflow "Deploy to VPS" --limit 1 --json databaseId --jq '.[0].databaseId')

# Check if the live site and API are responding
ping:
    @curl -sf https://united-humanity.us/api/tasks \
        | grep -o '"tasks":\[' \
        && echo "✓ API alive" \
        || echo "✗ API unreachable"

# Show git diff of what's staged (about to be committed)
diff:
    git diff --cached --stat

# ══════════════════════════════════════════════════════════════════════════════
# SERVER — ops and monitoring
# ══════════════════════════════════════════════════════════════════════════════

# Tail live server logs (Ctrl+C to stop)
logs:
    ssh humanity-vps "journalctl -u humanity-relay -f --no-pager"

# Show last 80 log lines (non-streaming)
log-tail:
    ssh humanity-vps "journalctl -u humanity-relay -n 80 --no-pager"

# Show only errors from server logs
log-errors:
    ssh humanity-vps "journalctl -u humanity-relay -n 200 --no-pager -p err"

# Open SSH session on the VPS
server:
    ssh humanity-vps

# Restart relay without rebuilding (instant)
restart:
    ssh humanity-vps "systemctl restart humanity-relay && sleep 1 && systemctl is-active humanity-relay"

# Show relay service status
relay-status:
    ssh humanity-vps "systemctl status humanity-relay --no-pager"

# Open the live SQLite database in an interactive shell
db:
    ssh humanity-vps "sqlite3 /opt/Humanity/data/relay.db"

# Run a SQL query on the live database
# Usage: just sql "SELECT count(*) FROM messages"
sql query:
    ssh humanity-vps "sqlite3 /opt/Humanity/data/relay.db '{{query}}'"

# Disk and memory usage on VPS
vps-health:
    ssh humanity-vps "df -h / && echo '' && free -h && echo '' && uptime"

# ══════════════════════════════════════════════════════════════════════════════
# TASKS — manage project tasks via API
# ══════════════════════════════════════════════════════════════════════════════

# List all tasks (id, title, status, priority)
tasks:
    @curl -sf https://united-humanity.us/api/tasks \
        | grep -o '"id":[0-9]*,"title":"[^"]*","[^}]*"status":"[^"]*","priority":"[^"]*"' \
        | sed 's/"id":\([0-9]*\),"title":"\([^"]*\)".*"status":"\([^"]*\)","priority":"\([^"]*\)"/  #\1  [\3] \4  \2/' \
        || echo "Could not fetch tasks"

# Show tasks grouped by status using jq (requires jq installed)
tasks-board:
    curl -sf https://united-humanity.us/api/tasks | jq -r '.tasks[] | "[\(.status)] \(.priority) — \(.title)"' | sort

# ══════════════════════════════════════════════════════════════════════════════
# RUST — local build and check
# ══════════════════════════════════════════════════════════════════════════════

# Check for errors (fast, no binary output)
check:
    cargo check --features native

# The library's unit tests, all or filtered: `just test-lib`, `just test-lib net::webrtc`,
# `just test-lib dm_store --nocapture`. Use this rather than a bare
# `cargo test --features native --lib`: on Windows the bare form fails to link
# (LNK1318, BUG-174) and scripts/lib-tests.js links the test program without a debug file.
test-lib *args:
    node scripts/lib-tests.js {{args}}

# One-shot DATA validation (~0.2 s once built): every data-file loader +
# data-wiring test in the lib suite -- item/recipe/plant/skill/blueprint/kit
# registries, RON/CSV parsers, shipped-data lints (recipes reference real items,
# kits resolve, machines exist). Run after editing anything under data/ for a
# fast signal without the full verify. The filters are OR'd libtest substrings.
validate-data:
    node scripts/lib-tests.js registry parses data_is_wired shipped from_csv from_ron

# Full local verification: both feature builds + lib tests + the GUI lints.
# Mirrors what CI cares about (the relay build is the one CI deploys with).
# just aborts the recipe on the first failing line, so this fails fast.
#
# STATIC ONLY: nothing here boots the app, and nothing here can (CI runs it
# headless with no GPU). Every failure that only appears once the process is
# running - device-limit rejections at startup (v0.782-784), a bind-group panic
# on world entry (v0.1029-1038) - lives past the last line of this recipe.
# `just verify-runtime` is the half that boots. Run both before a renderer push.
#
# Static gate: both feature builds + lib tests + the relay test build + every lint in `just lints`. Boot half: verify-runtime.
verify:
    cargo check --features native
    cargo check --features relay --no-default-features
    # A time limit (2026-10-03): one library test hung during a combined
    # verify overnight (the run normally takes about 3 minutes once built) and
    # held the night's release for hours. 40 minutes covers a full rebuild;
    # past it the verify FAILS, naming the stall, instead of waiting forever.
    timeout 2400 node scripts/lib-tests.js || { echo "FAILED: the library tests failed or ran past 40 minutes (a hung test: rerun and look for 'has been running for over 60 seconds')"; exit 1; }
    just verify-relay-compiles
    just lints
    just rig-tests
    node --test scripts/tests/obj-to-plant-gltf.test.js
    @echo "-- cross-language KATs: a browser and the relay must encode identically --"
    just vote-kat
    just mod-kat
    @echo "OK: verify passed"
    @echo "   (static only - nothing booted the app. For renderer/shader/world changes run: just verify-runtime)"

# The RELAY test target must at least COMPILE. This is not a formality: it had
# not compiled for some time before 2026-09-19 and nobody noticed, because
# `verify` only ever built the NATIVE test target. Twenty errors had piled up -
# test modules reaching for `pollster`, `crate::config` and the renderer's
# `Vertex` without a native gate - so the feature set CI deploys and the live
# server runs had no working test battery at all, and 1,508 tests were dark.
#
# COMPILE ONLY, deliberately, and here is the arithmetic. Running them costs
# about 200 s on top of the 53 s compile (measured 2026-09-19), and it would be
# re-running tests that finished minutes earlier in the same recipe: `native`
# IMPLIES `relay` in Cargo.toml, so every one of these 1,508 tests already ran
# in the library-test line above (scripts/lib-tests.js), over the same code.
# There are ZERO `cfg(feature = "native")` branches anywhere under `src/relay/`,
# so no relay test can take a different path under the two feature sets; what
# the relay build changes is which code EXISTS, and that is a compile question.
#
# So: `verify` pays 53 s for the gate that catches the real failure class, and
# the full run lives in `verify-relay` below for when relay code actually
# changed. If a relay-only behavioural difference ever does appear, move the
# full run up into `verify` and delete this paragraph.
#
# Build the relay test target (~53s): prove the server build's tests still exist.
verify-relay-compiles:
    @echo "-- the relay test target must build (CI deploys this feature set) --"
    cargo test --features relay --no-default-features --lib --no-run

# Worth doing after any change under src/relay/, and before a release, since
# this is the binary the VPS runs.
#
# RUN the relay test battery (~200s): the server build's own 1,508 tests.
verify-relay:
    cargo test --features relay --no-default-features --lib

# The BOOT half of verification, and the answer to "ten releases panicked on
# world entry while verify stayed green". Boots target/release/HumanityOS.exe in
# a portable probe rig, enters the world through autopilot, and visits three
# vantages picked from real incidents (full-disc Earth, a textured forest
# ground, a low storm sea). FAILS if any vantage cannot be reached or captured,
# or if the run log holds a single PANIC.
#
# Refuses before booting anything when:
#   - target/release/HumanityOS.exe is missing
#   - it was not built from THIS tree's sources: build.rs stamps a fingerprint
#     of every compiled-in source into the exe and scripts/check-fresh-exe.js
#     compares it with the tree (BUG-133: the old date-only check let a build of
#     main pass a feature worktree's check). Edited-after-the-build and
#     built-in-another-checkout both refuse, naming the files that differ.
#   - another verify-runtime probe is already running
#   - the machine never went quiet in --wait-min (see TWO CLOCKS below)
#   - an argument it does not know
#
# A red check against an old or other build on purpose:
#   just verify-runtime --exe v0.1444.0_HumanityOS.exe --allow-other-build "red check: <why>"
# passes the gate loudly and records other_build in the sweep manifest. The
# same flag works on every script that boots the game (BUG-133): probe-sweep,
# verify-screens, verify-live-screen, verify-copresence, photograph-home,
# make-clips (just clips) and boot-timing. Each also checks, after copying the
# exe into its rig, that the copy is byte-identical to what the gate judged,
# and starts the game with HUMANITY_NO_HANDOFF=1 so it cannot hand itself to a
# newer v*_HumanityOS.exe (src/main.rs find_newer_exe).
#
# NOT in `just verify` and never will be: verify runs headless in CI with no
# GPU. This needs the dev machine. Takes ~3 minutes. Run it before pushing
# anything renderer, shader, world or asset shaped. Pass driver args through,
# e.g. `just verify-runtime --exe v0.1069.1_HumanityOS.exe --allow-other-build "<why>"`
# (an archive is another tree's build, so it needs the reason).
#
# `just verify-runtime --operator-config` runs the gate at the operator's LIVE
# graphics settings instead of the rig's (heavier on every axis: render_distance
# 2000 vs 500, tree_model_distance 300 vs 120). Either way the verdict now
# prints WHICH settings it is a verdict about.
#
# TWO CLOCKS (2026-10-21). Before the boot the sweep waits for a quiet machine
# (other games, builds, another rig's machine-wide boot lock) for up to
# --wait-min N (60); if it never comes: "REFUSED: the machine never went quiet
# in N min; nothing was booted", exit 1, which is not a verdict on the build.
# The gate's own --timeout-min N (12) starts when the GAME starts and times
# boot, world entry and the vantages, so a timeout is a real hang. (The clock
# used to start with the recipe: twice on 2026-10-21 it ran out during the
# wait for other agents' builds and printed a false "boot or world-entry hang".)
# --game-only waits for other games and rigs but not builds (this gate reads
# panics and captures, not timings). An argument the gate does not know is
# refused, never ignored.
#
# Boot the real binary, enter the world, fail on any panic or missed capture.
[positional-arguments]
verify-runtime *ARGS:
    node scripts/verify-runtime.js "$@"

# The in-world SCREENS gate: boots target/release/HumanityOS.exe in its own
# portable rig (.probe-rig/screens, readable_web written on before boot),
# enters the world, parks in the console room, and proves with nobody at the
# keyboard that the wall screens are interactive: a click on the inventory
# wall's "Home" header (found by its drawn text) changes the screen, a click
# on the web wall's first link navigates to a new page (our own site only),
# the tasks wall drew a page, and run.log holds no PANIC. Every event goes
# through debug/screen_request.json into the same event API the look ray
# uses. Refuses while ANY HumanityOS.exe is running (one GPU) or the exe is
# stale. Exit 0 pass / 1 refused / 2 failed; evidence (PNG pairs, viewport,
# manifest, log) under .probe-rig/screens/runs/<stamp>/.
#   just verify-screens --dry-verdict <manifest.json>   re-judge without booting
#   node scripts/verify-screens.js --self-test           the verdict on the fixtures (no GPU)
# Boot the real binary, click the wall screens through the dev IPC, judge.
[positional-arguments]
verify-screens *ARGS:
    node scripts/verify-screens.js "$@"

# The `watch:` wall screen, end to end, against a relay this rig starts itself.
# There is no third-party stream this feature can watch (it watches our own
# relay), so the rig publishes one: a local headless relay on a spare port with
# a fresh database, scripts/live-publish.js pushing a moving test pattern, the
# game pointed at THAT relay. Proves the picture moves (two snapshots a second
# apart, against a static screen over the same second), then that a stopped
# stream says so, then that a session with NO SERVER says so and opens no
# socket at all (read out of run.log, not taken on trust). Never touches
# united-humanity.us. Refuses while ANY HumanityOS.exe is running (one GPU) or
# the exe is stale. Exit 0 pass / 1 refused / 2 failed; evidence under
# .probe-rig/live-screen/runs/<stamp>/.
#   just verify-live-screen --dry-verdict <manifest.json>   re-judge without booting
[positional-arguments]
verify-live-screen *ARGS:
    node scripts/verify-live-screen.js "$@"

# Publish a test pattern to a LOCAL relay by hand, to watch a wall screen play
# something while you work on it. Start a relay first, e.g.
#   BIND_ADDRESS=127.0.0.1 PORT=3399 DATABASE_PATH=.probe-rig/live-screen/relay/relay.db \
#       target/release/HumanityOS.exe --headless
# (BIND_ADDRESS=127.0.0.1 keeps Windows from raising a firewall prompt for it.)
# then point the app at http://127.0.0.1:3399 and run this. It refuses any
# server that is not loopback unless given --allow-remote.
live-publish *ARGS:
    node scripts/live-publish.js {{ARGS}}

# The std-only file-scanner lints (the loop below is the count): no em dashes, theme
# tokens, theme-editor coverage, tofu glyphs, engine wiring, the page registry,
# page parity, settings persistence, the monolith size ratchet, the focus opt-in
# allowlist, account SQL, and the rig determinism pins (rig_pin_lint, added
# 2026-09-18 after the hi-res capture path was found about to ignore the new
# anim_clock pin). The NAMES DO NOT ALL MATCH `*lint*` - file_size_ratchet and
# theme_editor_coverage are required checks that a `tests/*lint*` glob would
# silently skip, so this list is the authority, not a wildcard.
# scene_format_lint (2026-09-27, the HDR scene target): every scene pipeline is
# built for the scene format, never the display's; see its header.
# schema_alter_order_lint (2026-10-19, BUG-046): nothing in Storage::open names a
# column before the ALTER TABLE that adds it (the v0.675.0 index and the v0.262.2
# seed INSERT, each of which stopped the live relay at startup with every test green).
# bind_group_entry_lint (2026-10-19, the v0.1029-1038 world-entry panic): every
# create_bind_group in src/ gives its layout exactly the layout's bindings, the
# lazily built ones included; a site it cannot read fails rather than passes.
#
# Compiled standalone with rustc so they never link the native bin, which dodges
# the Windows LNK1318 PDB limit (see CLAUDE.md gotcha).
# One bash line so the loop + the CARGO_MANIFEST_DIR env share a shell.
#
# After the loop, one node check (2026-10-19, the PowerShell round trip in
# CLAUDE.md): no tracked file holds a byte-order mark, UTF-16 or Windows-1252
# mojibake. About 1.5 s over the whole tree; see scripts/check-encoding.js.
lints:
    # Each run builds into a folder of its own (2026-10-20): a fixed /tmp/<lint>.test.exe was shared by
    # every worktree, so two sessions running lints at once overwrote or locked each other's programs and
    # a lint "failed" with no message.
    export CARGO_MANIFEST_DIR="$(pwd)"; D=$(mktemp -d); for t in emdash_lint theme_token_lint theme_editor_coverage icon_glyph_lint engine_wiring_lint page_registry_lint page_parity_lint settings_persistence_lint file_size_ratchet focus_optin_lint account_sql_lint rig_pin_lint snapshot_recipe_lint recipe_sources_lint byproduct_use_lint scene_format_lint terminology_lint schema_alter_order_lint bind_group_entry_lint; do rustc --test --edition 2021 -A warnings "tests/$t.rs" -o "$D/$t.test.exe" 2>/dev/null && "$D/$t.test.exe" >/dev/null 2>&1 && echo ">> lint ok: $t" || { echo "LINT FAILED: $t"; "$D/$t.test.exe"; rm -rf "$D"; exit 1; }; done; rm -rf "$D"
    @node scripts/check-encoding.js --all > /dev/null && echo ">> lint ok: check-encoding --all" || { echo "LINT FAILED: check-encoding --all"; node scripts/check-encoding.js --all; exit 1; }

# Text garbled by a PowerShell round trip (Get-Content/Set-Content, > or Out-File):
# a byte-order mark a file did not have, UTF-16, or Windows-1252 mojibake ("the em
# dash came back as three characters"). With no arguments it checks your changes
# against HEAD (staged, unstaged, untracked) without a commit; the pre-commit hook
# runs `--staged`; `--all` reads every tracked file; `--fix <file>...` repairs.
#   just check-encoding
#   just check-encoding --fix src/gui/pages/chat.rs
# Refuse (or --fix) text garbled by a PowerShell round trip: byte-order mark, UTF-16, mojibake
check-encoding *ARGS:
    node scripts/check-encoding.js {{ARGS}}

# The RIG's own tests: the measuring instruments, checked the way the code they
# measure is checked. Pure node, no GPU, under a second, so it runs inside
# `just verify` rather than waiting for someone to remember it.
#
# Covers today: the one-machine guard (scripts/lib/machine-guard.js) and
# perf-report's refusal to grade a contaminated capture. Both exist because on
# 2026-09-18 two separate readings came back wrong and looked right: a second
# builder's instance arrived mid-sweep and its 9 ms of stolen GPU sat in a
# comparison table as data (docs/design/frame-cost-arc.md, "V1 outcome"), and a
# concurrent cargo/rustc build read 6.6 fps at the limb against a 15 ms GPU sum.
# And the cloud grain measure (scripts/terminator-grain.js): on 2026-09-22 the
# old grain metric ranked a 3x3 box blur as the best fix at the dusk line, so
# its test proves on synthetic captures that a blur is judged "same", that
# averaging independent noise is judged BETTER, and that each of the measure's
# two locks goes red when the other is switched off. That one takes a few
# seconds (it blurs and re-scores six controls per run), not under one.
# And the co-presence judge (scripts/lib/copresence-judge.js): smooth samples
# pass, and round one's stop-go drawing, a figure that never appears, a step
# back, a snap, a drift off the line, a walk out of view and a figure covered
# by a window in the screenshot each FAIL; and its plot checks (increment 1b):
# the 1a shape (no plots handed out, the game's camera still on p1), two
# players on one plot, and a walker drawn outside its plot each FAIL. BUG-161:
# a capture starved by builds (400 ms frames, and the four real 2026-10-05
# recordings) reads CONTAMINATED, never PASS and never FAIL, while a wrong
# speed on a quiet or a busy machine still FAILS; and the guard's build hold
# lowers only builds, puts each back (on an error and Ctrl+C too), and is
# proven once on a real rustc it starts itself (Windows; about a second).
# And the station park check (scripts/lib/station-park-check.js, BUG-132): a
# {"station":"home"} park on its pose passes, and the two misses BUG-132
# wrote (the pose plus the Earth-to-home offset, and a kilometre out), a camera
# not riding the station and a wrong look each FAIL.
# And the freshness gate every rig runs before it boots (scripts/check-fresh-exe.js,
# BUG-133): a build of the same tree passes, one changed source byte, a build of
# another tree (even a NEWER one), no stamp, two stamps and a garbled stamp each
# REFUSE, line endings alone do not count, --allow-other-build passes loudly and
# is recorded, and every rig reaches the gate through runFreshGate. Its node
# fingerprint is pinned to the number build.rs produced on the same fixture, and
# when a build script sits under target/ the test runs it on the fixture too.
# And the throwaway relay's loopback-only check (scripts/lib/throwaway-relay.js,
# 2026-10-03): its env sets BIND_ADDRESS=127.0.0.1 whatever the shell says,
# netstat/ss/lsof output is read for the right PID only, every wildcard
# spelling and an empty listing are REFUSED, and a real 127.0.0.1 listener is
# read from the OS and passed. A dev relay on 0.0.0.0 raises a Windows Firewall
# prompt for every new temp path; see docs/INCIDENT-PLAYBOOK.md.
# And the web chat's erase choice (scripts/tests/erase-sign-up-again.test.js, BUG-135,
# 2026-10-04): only the person's Enter under the erase note says sign_up_again, on that one
# socket, never a reload's auto-connect or an automatic reconnect.
# And the sticky showcase pins the rigs release before every vantage (scripts/tests/
# showcase-pins.test.js, 2026-10-04): one shared list, pipe_marking included.
# And the shared-build judge (scripts/lib/shared-build-judge.js, ship homes increment 5,
# 2026-10-05): a made-up `verify-shared-build` run that went right passes every check, each
# broken one (the builder's build kept inside the game's plot, a piece drawn late or off where
# it was built, planks spent twice, a wall not in the picture, ...) fails its own check and no
# other, and the same run saved to a folder is judged by `--dry-verdict`.
# And the household judge (scripts/lib/household-judge.js, ship homes increment 5b, 2026-10-12): a
# made-up `verify-household` run that went right passes every check, each broken one (the game
# walking through her shut corridor, a permit naming another server, a key in a relay permit-check
# line, Ada's build with a permit taken back KEPT, ...) fails its own check and no other, the probe's
# household fields are read in one place (PROBE), and the rig's corridor walks stay at walking pace.
# And the starts judge (scripts/lib/starts-judge.js, ship homes increment 6, 2026-10-12): a made-up
# `verify-starts` run, built from the tree's own data, that went right passes every check, each
# broken one (the home's things carried into the server's save, the counter's offline meal in a
# server's world, a key on the relay's home-chosen line, home again to the default home, ...) fails
# its own check and no other, the counter's words and the probe's field names are the game's own,
# and the same run saved to a folder is judged by `--dry-verdict`.
# And the probe sweep's ground_probe judgement (scripts/tests/tree-ground-check.test.js,
# BUG-156, 2026-10-05): trees floating over the drawn ground, or an eye off it, fail.
# And the friendship pass words (scripts/tests/friend-pass.test.js, passes v2, 2026-10-09):
# the web builder (web/shared/friend-pass.js) is held to the relay's pinned preimage, and the web
# chat client gives, keeps and withdraws passes as native does (friend-pass-web.test.js).
# And "who can reach me" on the web (reach-web.test.js, step B, 2026-10-09): Safety shows the relay's
# reach_settings, reach_set goes out per kind, refusals offer a contact request that carries my pass.
# And calls through the server on the web (calls-through-server.test.js, step E, 2026-10-09): no Google
# STUN under web/, rooms and calls ask call_credentials and connect relay only, no credentials says so.
# And an admin erasing another person's data on the web (admin-erase-web.test.js, 10i, 2026-10-10):
# offered only to an admin or the owner and never on an admin's row, Erase needs the exact name, the
# frame is the spec's, the receipt is shown, and the erased person's client says an admin did it.
# And what kind of file a received file is on the web (file-risk-web.test.js, 2026-10-17): the
# website's checker (web/shared/file-risk.js) answers the shared cases the desktop app's twin
# answers (scripts/tests/fixtures/file-risk-cases.json), from data/safety/file_risks.json.
# And files dragged from the computer onto the website (drop-files-web.test.js, 2026-10-17): the
# overlay's words while they are over a page, the Media page's and Shared Files' drops taking their
# picker's path, the reasons where a drop would do nothing (the chat's drops: private-files-web.test.js).
# And the encoding check (check-encoding.test.js, 2026-10-19): a PowerShell round trip of a tracked
# file, a gained byte-order mark, UTF-16 and one garbled added line are each refused in a throwaway
# repo, every character U+0080..U+FFFF garbled by ICU's decoder is caught, and --fix restores it.
# And `just tag` (tag-release.test.js, 2026-10-19): an empty, malformed or uncommitted version, a tag
# already here or on origin, origin unreachable, HEAD not on origin/main and a missing notes file each
# REFUSE with nothing tagged; pushes and gh are recorded by an allowlist runner, never made.
# And the falls judge (scripts/lib/falls-judge.js, 2026-10-20): a made-up `verify-falls` run, built
# from the tree's falls.ron, gravity and stairs, that went right passes every check; the run a build
# with the lib.rs landing hand-off taken out reports (the camera drops 3 m, the body does not move)
# fails, a Forgiving leg that never fell is not a pass, a placement's drop that hurts and a ladder
# that does not hold on fail, the two REAL runs in tests/fixtures/falls/ (green, and that red build)
# are judged as they were, and the same runs saved to a folder are judged by `--dry-verdict`.
# And the request writer every rig uses (scripts/lib/write-request.js, 2026-10-20): a reader that
# races it the way the game polls (read, delete, every frame) sees nothing or the whole request, never
# part of one, and the same reader catches a plain fs.writeFileSync empty, so the race can fail
# (write-request.test.js); and no script under scripts/ writes a request file any other way, the
# names checked against the game's own request constants (request-writes.test.js). A plain write lost
# verify-falls' `illness` request on 2026-10-20 and a Forgiving leg ran in Realistic.
# And the storeys judge (scripts/lib/storeys-judge.js, 2026-10-21): a made-up `verify-storeys` run,
# built from the homestead's own stair, slabs, zones, elevator and ladder, that went right passes
# every check; the shape the running game takes when the floor slabs never reach the walk (up the
# treads, then 3 m down past the top, the HUD naming the hall downstairs) fails, and so does a wall,
# guard, gate or landing door that does not hold, a wall upstairs that stops a body downstairs, a
# jump through the slab, an elevator that does not ride or jumps its rider, the HUD naming the wrong
# storey, and a placement passed off as a walk; a layout that moved what the route walks refuses
# before anything boots; the two REAL runs in tests/fixtures/storeys/ are judged as they were.
# And the machine-wide rig boot lock (scripts/lib/boot-lock.js, 2026-10-21): two rigs woken by the
# same build ending (real processes, same moment) never boot together; another rig's held lock is a
# busy machine to the guard; a lock whose holder was killed, or whose pid now belongs to another
# process, is taken over; spawnGame will not start a game without it; probe-sweep waits for it.
# And verify-runtime's two clocks (verify-runtime-gate.test.js): a machine wait longer than the gate
# does not trip it, a game that hangs after it starts still does (node, stamped, as the "game");
# a machine that never goes quiet is REFUSED (exit 1), not FAIL; --game-only reaches the sweep; an
# unknown flag or a non-number time is refused, not ignored. Every lock here is the test's own file.
# And the DXC pair from a worktree (dxc-dlls.test.js, 2026-10-21): a worktree's rig finds the pair in
# the main checkout's root (found as `git rev-parse --git-common-dir` finds it, checked against git),
# dxc-beside-exe.js too, and a rig whose game boots on FXC ends its output with a banner saying so.
# Add a file here whenever a rig script grows a judgement of its own.
rig-tests:
    node --test scripts/tests/machine-guard.test.js scripts/tests/perf-report.test.js scripts/tests/terminator-grain.test.js scripts/tests/make-clips.test.js scripts/tests/voice-rejoin.test.js scripts/tests/p2p-sync-own-devices.test.js scripts/tests/p2p-direct-offers.test.js scripts/tests/link-preview-pictures.test.js scripts/tests/erase-sign-up-again.test.js scripts/tests/backup-rotate.test.js scripts/tests/second-player.test.js scripts/tests/sealed-dm.test.js scripts/tests/copresence-judge.test.js scripts/tests/dxc-dlls.test.js scripts/tests/station-park-check.test.js scripts/tests/check-fresh-exe.test.js scripts/tests/check-delivery.test.js scripts/tests/throwaway-relay.test.js scripts/tests/rig-boot.test.js scripts/tests/compiled-in.test.js scripts/tests/game-launch.test.js scripts/tests/rig-exe-copy.test.js scripts/tests/showcase-pins.test.js scripts/tests/rig-gameplay.test.js scripts/tests/clean-test-temp.test.js scripts/tests/shared-build-judge.test.js scripts/tests/household-judge.test.js scripts/tests/starts-judge.test.js scripts/tests/tree-ground-check.test.js scripts/tests/friend-pass.test.js scripts/tests/friend-pass-web.test.js scripts/tests/reach-web.test.js scripts/tests/block-web.test.js scripts/tests/server-list-backoff.test.js scripts/tests/report-web.test.js scripts/tests/outside-help-web.test.js scripts/tests/calls-through-server.test.js scripts/tests/warnings-web.test.js scripts/tests/protected-web.test.js scripts/tests/admin-erase-web.test.js scripts/tests/group-report-web.test.js scripts/tests/trade-page-web.test.js scripts/tests/private-files-web.test.js scripts/tests/files-web.test.js scripts/tests/files-web-round2.test.js scripts/tests/strip-metadata.test.js scripts/tests/files-web-round3.test.js scripts/tests/files-web-round4.test.js scripts/tests/upload-clean-web.test.js scripts/tests/files-in-messages-web.test.js scripts/tests/kept-files-web.test.js scripts/tests/files-web-files.test.js scripts/tests/describe-agree.test.js scripts/tests/relay-follow.test.js scripts/tests/file-transfer-web.test.js scripts/tests/file-transfer-chat-web.test.js scripts/tests/profile-copy-web.test.js scripts/tests/chat-home.test.js scripts/tests/choice-web.test.js scripts/tests/library-order-web.test.js scripts/tests/files-web-round5.test.js scripts/tests/server-strip-web.test.js scripts/tests/files-web-item.test.js scripts/tests/files-takedown-web.test.js scripts/tests/phrase-cover-web.test.js scripts/tests/model-view-web.test.js scripts/tests/file-risk-web.test.js scripts/tests/drop-files-web.test.js scripts/tests/post-in-chat-web.test.js scripts/tests/files-card-web.test.js scripts/tests/check-encoding.test.js scripts/tests/tag-release.test.js scripts/tests/apps-web.test.js scripts/tests/files-web-panes.test.js scripts/tests/files-web-places.test.js scripts/tests/web-theme-light.test.js scripts/tests/web-root-retired.test.js scripts/tests/falls-judge.test.js scripts/tests/write-request.test.js scripts/tests/request-writes.test.js scripts/tests/storeys-judge.test.js scripts/tests/map-centre-check.test.js scripts/tests/boot-lock.test.js scripts/tests/verify-runtime-gate.test.js scripts/tests/recovery-web.test.js scripts/tests/map-overview-check.test.js scripts/tests/sol-orbits-web.test.js

# The scripted second player (scripts/second-player.js) against a REAL relay.
# NOT pure node, so NOT in rig-tests or `just verify` (rig-tests keeps the
# script's no-relay checks: paths, facing, step cap, clock, names). It BOOTS a
# throwaway relay: a COPY of target/release/HumanityOS.exe, run --headless
# (no window, no GPU) from a temp folder on a free port with its own database,
# killed by PID at the end, also on Ctrl+C. Why boot one: whether a scripted
# player is seen walking, chatting and leaving can only be told by a relay.
# Checks the exe with scripts/check-fresh-exe.js first and SKIPS, printing the
# verdict, when the build is older than the source. SECOND_PLAYER_RELAY_EXE=
# <path> tests another build (a relay-only debug build will do). About 20 s.
verify-second-player:
    node --test scripts/tests/second-player-relay.test.js

# Proves in the REAL game that a second player is drawn and walks smoothly.
# BOOTS TWO THINGS: a throwaway relay (a COPY of target/release/HumanityOS.exe,
# --headless, temp folder, free port, own database; scripts/lib/throwaway-relay.js)
# and ONE game instance in its own portable probe-rig sandbox (.probe-rig/copresence,
# background, never focused, silent) pointed at that relay by the autopilot. It
# parks the camera at a known home pose, has scripts/second-player.js walk a
# straight line across the view, records where the game DREW the figure on every
# frame (debug/remote_players_request.json, src/engine/ipc.rs), takes two
# screenshots, and judges it (scripts/lib/copresence-judge.js): seen, never
# backwards, per-frame speed near the walker's by the recorded frame times, no
# jump, on its line, in view, and VISIBLE in both screenshots (its teal body
# counted under its nameplate). Refuses (exit 1) while ANY HumanityOS.exe runs (one
# GPU) or when the exe is older than the source; exit 2 = a check failed; exit 3 =
# CONTAMINATED: nothing failed, but the walk's speed could not be judged because the
# machine was starved (frames over 150 ms, every off-pace frame right after one;
# BUG-161). That is NOT a pass: run it again on a quieter machine. For the length of
# each recording it judges, every cargo, rustc, link and cl on the computer is held
# at BelowNormal priority and put back after (the manifest's builds_held). About a
# minute. Evidence in .probe-rig/copresence/runs/<stamp>/.
#   just verify-copresence --plots    homes on plots (increments 1b and 2 of
#     docs/design/ship-homes-and-logistics.md): runs, walker first then game
#     first, each with its own relay and boot. At home: the two plot ids differ by
#     the join order, the game's camera is inside the plot it should hold after
#     joining, every position the game drew for the walker is inside the
#     walker's plot, and one forward leg of the walk is smooth. Then the two MEET
#     IN THE COMMONS (meet_* checks): the game reports its door points, is moved
#     from its door into the Commons in 40 m steps, and the walker walks out of
#     its home through its own corridor into the Commons and across the game's
#     view; the view, the smoothness and both screenshots (its teal body counted)
#     are judged there. Then the game steps out of the shared world and back,
#     moved over 100 m from its door while out: it must stand where the relay
#     respawns it, and its next move must reach the walker; Respawn and the build
#     editor the same way. Last the game boots a SECOND time against the same
#     relay: it must build its home on the plot it held before joining (reboot_*).
#     A third run, the GUEST (guest_*): two scripted players take both plots and the
#     game comes in third; its home is put away with none of its things on a plot,
#     it stands and respawns in the Commons, B opens no editor and says why,
#     stepping out brings the home back on the default plot, and a dropped
#     connection that comes back inside the relay's grace stands it off the plot
#     its home came back on. --order walker-first|game-first|guest runs one, both
#     the first two, all (the default) all three. game-first comes into the world
#     the way a returning player does: connected and identified on the main menu
#     first, then the menu's Enter World pressed (the join gate then runs before
#     the world has loaded); walker-first by the autopilot. --entry
#     menu|autopilot picks one for every order. About 3 minutes per order on a
#     quiet machine, the guest about 1 (2026-10-04).
#   just verify-copresence --dry-verdict <manifest.json>   re-judge without booting
[positional-arguments]
verify-copresence *ARGS:
    node scripts/verify-copresence.js "$@"

# Proves in the REAL game that building in the shared world keeps the rules of ship homes
# increment 5, "building only on your own plot" (docs/design/ship-homes-increment-5-plan.md,
# section 5): `verify-copresence --build`. BOOTS TWO THINGS, like verify-copresence: a throwaway
# relay (loopback only, its own database, its admin the rig's rank holder through ADMIN_KEYS) and
# ONE game in the .probe-rig/copresence sandbox (background, never focused, silent), plus two
# scripted builders (scripts/second-player.js --path still). The plain builder holds p1, the game
# p2, the rank holder p3. Judged (scripts/lib/shared-build-judge.js): the builder's pieces drawn in
# the game in time, where built, as scaffolds finishing on the relay's clock and never in its save;
# the game's own piece spent once and seen by the builder; each one's build inside the other's
# plot refused (by the relay for the builder, at the game's own crosshair for the game); only the
# plot's holder takes its pieces down, the planks coming back once; the Commons built only with
# the rank, the rank holder's wall drawn and counted in a picture; ship editing off while joined
# and back offline, the same pieces returning by the same ids. Refuses (exit 1) while ANY
# HumanityOS.exe runs (one GPU) or when the exe is older than the source; exit 2 = a check failed.
# Needs a build with increment 5's Wave 2 (the relay's game_build and the game's `place`,
# `take_down` and `stock` verbs and `shared` recorder rows). Evidence in
# .probe-rig/copresence/runs/<stamp>-build/.
#   just verify-shared-build --dry-verdict <manifest.json>   re-judge without booting
[positional-arguments]
verify-shared-build *ARGS:
    node scripts/verify-copresence.js --build "$@"

# Proves in the REAL game that a player who chooses a Cabin on a server starts there fresh, eats at
# the mess hall and finds their own home untouched (ship homes increment 6, "the starts as data, the
# Cabin, the mess hall", docs/design/ship-homes-increment-6-plan.md section 6): `verify-copresence
# --starts`. BOOTS TWO THINGS, like verify-shared-build: a throwaway relay (loopback only, its own
# database, the tree's data/ mirrored) and ONE game in the .probe-rig/copresence sandbox
# (background, never focused, silent), paired with its own home so it boots there alone and keeping
# its progress between worlds, plus one scripted walker, TestBotCabin (scripts/second-player.js
# --start cabin). Judged (scripts/lib/starts-judge.js) against the tree's own data (the Cabin's kit
# and opening from data/homes/starts.ron, the plots from the ship file, the serving counter from
# data/machines/ship.ron): the game's own home stocked (OFFLINE_MARK); the walker given the first
# cabin and told what is free; the game choosing a Cabin on the server and arriving fresh (the
# Cabin's kit only, no skills, a new character's credits, the Cabin's opening) with the same look,
# in its cabin on the next cabin plot; walking through the doors to the mess hall's counter, never
# corrected; a meal on the fleet ledger, a second refused for now, its bottles filled, the walker
# eating too; the opening following; home again to OFFLINE_MARK with the server's save kept on disk;
# back on the server to its cabin. Refuses (exit 1) while ANY HumanityOS.exe runs (one GPU: check
# `tasklist | grep -i humanityos` first) or when the exe is older than the source; exit 2 = a check
# failed. Needs a build with increment 6's Wave 2 (the probe's `home_session` block and the
# `choose_start`, `pairing`, `meal` and `water` verbs). Evidence in
# .probe-rig/copresence/runs/<stamp>-starts/.
#   just verify-starts --dry-verdict <manifest.json>   re-judge without booting
[positional-arguments]
verify-starts *ARGS:
    node scripts/verify-copresence.js --starts "$@"

# Proves in the REAL game that household permits work end to end (ship homes increment 5b, "the
# household page, and walking into a permitted plot", docs/design/ship-homes-increment-5b-plan.md
# section 6, with its section 8 amendments: a permit can be taken back): `verify-copresence
# --household`. BOOTS TWO THINGS, like verify-shared-build: a throwaway relay (loopback only, its own
# database) and ONE game in the .probe-rig/copresence sandbox (background, never focused, silent),
# plus one scripted friend, Ada (scripts/second-player.js --path still). Ada holds p1 with a wall on
# it, the game p2. Judged (scripts/lib/household-judge.js): they become friends through a contact
# request the game accepts; before any permit the game cannot walk into her corridor; the game
# gives her a permit by clicking its Household card, she builds on its plot with it and takes down
# only her own; Renew gives her a second and takes the first back; she gives the game one, the relay
# checks it within 2 s, and the game walks into her home, never corrected, lit, sees her wall in a
# picture, builds there and takes down only its own; Take back on the card makes her next build with
# that permit refused permit_withdrawn; no relay permit-check line names anyone. Refuses (exit 1)
# while ANY HumanityOS.exe runs (one GPU: check `tasklist | grep -i humanityos` first) or when the
# exe is older than the source; exit 2 = a check failed. Needs a build with ship homes 5b Wave 2
# (the probe's `household` block, the showcase `page` and `accept_request` verbs, and the Household
# card). Evidence in .probe-rig/copresence/runs/<stamp>-household/.
#   just verify-household --dry-verdict <manifest.json>   re-judge without booting
[positional-arguments]
verify-household *ARGS:
    node scripts/verify-copresence.js --household "$@"

# Proves in the REAL game that falls hurt the player the way src/systems/falls.rs and
# data/medical/falls.ron say (2026-10-20). BOOTS ONE game in its own portable sandbox
# (.probe-rig/falls: background, never focused, silent, no server), enters the world by the dev
# autopilot (the rig's own home, aboard the homestead) and walks the player up stairs-1 and off
# its 3 m top by holding forward (the showcase `hold` verb, the controller's own action path):
# in Forgiving a landing reaches the body and does nothing (7.67 m/s read at 70%, under the first
# harm), in Realistic it leaves a Sprained Ankle and takes 9.7 health on the frame it lands. Then
# it climbs ladder-1 by holding Space, lets go at the top (the body must hold on) and steps back
# off: a Broken Ankle and 11.5 health. Then it PLACES the player 3 m up beside the stairs: that
# drop must reach no body at all. And world entry must hand the body no landing. Judged by
# scripts/lib/falls-judge.js against the tree's falls.ron, read by its own code (proven red on a
# build with lib.rs's landing hand-off taken out: tests/fixtures/falls/). Refuses (exit 1) while
# ANY HumanityOS runs (one GPU: check `tasklist | grep -i humanityos` first) or when the exe is
# older than the source; exit 2 = a check failed. About two minutes after boot.
# Evidence in .probe-rig/falls/runs/<stamp>/.
#   just verify-falls --dry-verdict <manifest.json>   re-judge without booting
[positional-arguments]
verify-falls *ARGS:
    node scripts/verify-falls.js "$@"

# Proves in the REAL game that the homestead's upper floor works (ship homes increment 7,
# 2026-10-21; docs/design/ship-homes-increment-7-plan.md "Rig (V)", the layout in
# docs/design/homestead-upper-floor.md). BOOTS ONE game in its own portable sandbox
# (.probe-rig/storeys: background, never focused, silent, no server), enters the world by the dev
# autopilot (the rig's own home, the homestead) and walks it on the held movement keys: in at the
# front door, down the hall and up stairs-1 (the floor stood on reaches 3.0, the HUD names the upper
# hall, no landing reaches the body), into the upstairs bedroom (the HUD names it); a wall upstairs
# stops the body upstairs and not on the ground floor under it; a jump under the slab stops under
# its underside; the guard over the stairwell holds; elevator-1 rides up on Space and down on Shift
# and comes when called from the upper landing, whose door holds the body while the car is away;
# ladder-1 climbs through its hatch onto the upper floor and its gate holds a walker. Three
# screenshots kept with the run (the upper hall, the bedroom, the ground hall up the stair): LOOK at
# them. Judged by scripts/lib/storeys-judge.js against the tree's data (proven red on a homestead
# with its floor slabs taken out, through --data: tests/fixtures/storeys/). Waits while another
# HumanityOS runs (one GPU; --no-wait to refuse instead); refuses (exit 1) when the exe is older than
# the source or the layout moved what the route walks; exit 2 = a check failed. About four minutes
# after boot. Evidence in .probe-rig/storeys/runs/<stamp>/.
#   just verify-storeys --dry-verdict <manifest.json>   re-judge without booting
#   just verify-storeys --data <copy of data/>          the game reads that data; the judge, the tree's
[positional-arguments]
verify-storeys *ARGS:
    node scripts/verify-storeys.js "$@"

# Render every native UI snapshot (97 on 2026-10-12) to PNGs in tests/snapshots/ for review.
# NEEDS A GPU: without an adapter every page is SKIPPED with a printed note and the
# run still exits 0, so a green line here is not evidence that anything rendered -
# read the output. Open the PNGs after. For ONE page use `just snapshot <name>`.
snapshots:
    node scripts/lib-tests.js snapshot_ --ignored --test-threads=1 --nocapture
    @echo "UI snapshots written to tests/snapshots/, open them to review."

# Render ONE named page to tests/snapshots/<name>.png. Names are the test suffix in
# src/gui/ui_snapshots.rs; `just snapshot <a bad name>` prints all of them (the count drifts,
# so trust that list over any number written here).
# A page missing from that list has no snapshot test yet - docs/PAGES.md is the full
# page registry.
#
# The name is checked BEFORE cargo runs, because cargo exits 0 when a test filter
# matches nothing and this recipe used to follow that with an unconditional
# `echo "Wrote ..."`. A typo reported success and rendered nothing.
snapshot name:
    @node scripts/snapshot-name.js {{name}}
    node scripts/lib-tests.js snapshot_{{name}} --ignored --nocapture
    @echo "Wrote tests/snapshots/{{name}}.png (a GPU-less run SKIPS and still exits 0 - check the lines above)"

# Pre-push checklist: the recurring CI gotchas in one shot (untracked source
# files that fail a fresh checkout, broken doc links) plus full verify.
preflight:
    @echo ">> untracked source files (would fail CI on a fresh checkout; empty = good):"; git ls-files --others --exclude-standard -- '*.rs' '*.ron' '*.csv'
    node scripts/check-doc-links.js
    node scripts/curriculum-status.js
    node scripts/check-locale-species.js
    node scripts/check-locale-phenology.js
    node scripts/check-library-dashes.js
    node scripts/check-library-fresh.js
    node scripts/check-library-render.js
    node scripts/check-library-counts.js
    node scripts/check-schema-paths.js
    node scripts/check-library-quotes.js
    node --test scripts/tests/check-library-quotes.test.js
    node scripts/check-web-sites.js
    node scripts/verify-screens.js --self-test
    just verify
    @echo "OK: preflight passed, safe to push"

# Regenerate the readable locale guide for every place in data/locales/ from its
# own data files, then rebuild the Library to ship it. The data drives the
# simulation AND the document, so the two cannot disagree. Re-run after editing
# anything under data/locales/; never edit the generated markdown.
locale-docs:
    node scripts/build-locale-doc.js
    node scripts/build-library.js

# Refuse a shipped Library copy that no longer matches its source. data/library/
# is GENERATED from docs/, so editing a source without rebuilding ships the old
# text while everything still looks right. CLAUDE.md records that this drifted
# silently for months once; this is the check it says did not exist.
check-library-fresh:
    node scripts/check-library-fresh.js
# Run the REAL web Library reader over the REAL shipped documents and look at
# what comes out. Every other Library gate reads the source; none of them had
# ever looked at the rendered page, which is this repo s dominant defect class.
# First run found two: an underscore inside a word opened emphasis, so
# what_soil_is.md, silverdale_wa and a set of cited source URLs lost their
# underscores and went italic (82 spans, 23 documents, and the NATIVE reader
# showed every one correctly, so only opening both clients side by side would
# have found it); and one table never rendered because it sat indented inside
# a checklist item, leaving its separator row on the page as raw pipes.
check-library-render:
    node scripts/check-library-render.js --verbose
# Refuse a shipped document that states a count of locale records the data no
# longer supports. A guide that teaches from a dataset quotes its shape, and
# every one of those sentences was true the day it was written; then three sea
# star records landed and five of them were false across three documents, with
# nothing breaking. It was caught by a writer who happened to re-derive the
# numbers for a different guide, which is luck rather than a process. The
# counts are good teaching and should stay, so this keeps them true instead.
check-library-counts:
    node scripts/check-library-counts.js
# Refuse a schema that names a source path which does not exist. schemas/ is
# what somebody reads to learn what a data row IS, and sixteen of the thirty
# files still pointed at native/src or server/src, neither of which has existed
# since the unified-binary merge. CLAUDE.md carries a standing warning that an
# agent reading a stale path writes its edits to a dead location. Pass --docs
# to also REPORT on docs/ (never fails there: a resolved bug report naming where
# the code used to live is correct history, not rot).
check-schema-paths:
    node scripts/check-schema-paths.js --docs
# Refuse a verbatim quotation from a source the registry marks use: facts. The
# operator rule is that only public domain ships in the bundle, and the registry
# says per source which half it is in. Three guides broke this in one day and a
# fourth had 24 such quotations, none of them carelessly: the licence question is
# invisible at the moment of writing unless you go and look. Put
# <!-- quote-ok: reason --> on the line before a quotation that is genuinely
# legitimate, such as reproducing a copyright notice in order to attribute it.
check-library-quotes:
    node scripts/check-library-quotes.js --verbose
    node --test scripts/tests/check-library-quotes.test.js
# Refuse a websites database (data/web/sites.json) that claims an embed
# decision without recording who made it, when and on what clause, or an
# affiliate tag without its disclosure. Also shape-checks readability.json.
check-web-sites:
    node scripts/check-web-sites.js
# Refuse an em or en dash in any document the Library ships. The house rule says
# no em dashes anywhere including docs, and the only thing enforcing it scanned
# src/gui/, so eight shipped documents had drifted, the Humanity Accord among
# them. A document marked "verbatim" in the catalogue is exempt, because the US
# Constitution's dashes are the 1787 parchment's and not ours to tidy.
check-dashes:
    node scripts/check-library-dashes.js
# Validate every locale's species.json: the shape, and the rule that matters,
# which is that anything edible or toxic must name what it is confused with and
# say how to tell them apart. An empty lookalike list was the single most common
# finding when the Silverdale plant records were put to an adversarial verifier,
# and confusion is how foraging kills people, not ignorance.
# Validate every locale phenology calendar, and keep its confidence labels
# honest: a measured or published_range event must carry a real source, because
# those grades assert somebody measured or published it. local_consensus need
# not, that grade says nobody published a date, but it must still point at the
# record its timing comes from. Also reports the ratio the per-event labels
# hide: how many biological events are actually measured.
check-phenology:
    node scripts/check-locale-phenology.js
check-species:
    node scripts/check-locale-species.js
# Check that every citation in data/sources/registry.json still resolves.
# Talks to the network, so it is NOT in verify or preflight, both of which must
# work offline. Catches the failure a status check misses: a link that answers
# 200 with the publisher's homepage instead of the document, which is how the
# retired DOE Energy Saver pages sat in the registry looking healthy.
check-sources *args:
    node scripts/check-source-urls.js {{args}}
# How complete the teaching is, and the gate that keeps the claim honest.
# Every skill, document, prerequisite and source the syllabus names must
# resolve, and a topic marked teaching-grade must not rest on a citation that
# names nobody. Flags: --gaps (every unwritten topic), --hazard (what can kill
# if taught wrong), --sources (the citation registry, grouped by licence).
curriculum *args:
    node scripts/curriculum-status.js {{args}}

# Build relay-only (headless server, no GPU)
build-relay:
    cargo build --release --features relay --no-default-features

# Run relay locally for development, on THIS computer only (BIND_ADDRESS=127.0.0.1):
# a dev relay listening on every interface raises a Windows Firewall prompt for
# each new exe path (docs/INCIDENT-PLAYBOOK.md, 2026-10-03). To try it from a
# phone on your Wi-Fi, run the cargo line by hand with BIND_ADDRESS=0.0.0.0.
run-relay:
    BIND_ADDRESS=127.0.0.1 cargo run --features relay --no-default-features -- --headless

# cargo fmt is BANNED in this repo (Incident v0.390): it reformats ~240 files and
# moves inline `// theme-exempt` markers onto their own line, silently breaking
# theme_token_lint. Match surrounding style by hand. This recipe is a hard stop so
# `just fmt` can never trigger the incident by reflex.
fmt:
    @echo "cargo fmt is BANNED here -- reformats ~240 files, moves inline // theme-exempt markers, breaks theme_token_lint (Incident v0.390). Match style by hand. See CLAUDE.md."
    @exit 1

# Run clippy linter
clippy:
    cargo clippy --features native -- -D warnings

# ══════════════════════════════════════════════════════════════════════════════
# GAME — native desktop client (Rust/wgpu/egui)
# ══════════════════════════════════════════════════════════════════════════════

# Build the game: bump patch version, compile, archive versioned exe, purge old builds
build-game:
    @just bump
    cargo build --features native --release
    @node scripts/archive-build.js
    @node scripts/check-delivery.js

# Put the current work in the OPERATOR HANDS: compile, archive, and refresh the
# stable HumanityOS.exe that his taskbar shortcut points at.
#
# Run this after any Rust change he should be able to test. It is build-game
# without the version bump, which is what you want when the version is already
# stamped and tagged and you just need the binary to catch up -- bumping again
# there strands local ahead of GitHub (see the Version SOP).
#
# Five releases shipped without this step in September 2026. The code was
# right, the tags were right, and the operator spent two days testing a build
# from before all of it.
#
# Build + archive + refresh the taskbar exe (build-game without the bump)
deliver:
    cargo build --features native --release
    @node scripts/archive-build.js
    @node scripts/check-delivery.js

# The one question `just brief`, CI and the release tag all fail to ask.
#
# Is the exe the operator launches actually the work we just did?
check-delivery:
    @node scripts/check-delivery.js

# Build and launch the game. HUMANITY_TAKE_FOCUS is the operator's explicit
# focus opt-in (v0.1081): script-launched instances now open in the background
# BY DEFAULT (focus requires proof of a human launch -- see
# src/engine/launch_focus.rs), and these two recipes are the operator's own
# interactive launches, so they carry the proof. Agents must NEVER use
# play/launch; they get launch-bg / probe-sweep.
play: build-game
    HUMANITY_TAKE_FOCUS=1 node scripts/archive-build.js --launch

# Launch latest build without rebuilding (operator's interactive launch)
launch:
    HUMANITY_TAKE_FOCUS=1 node scripts/archive-build.js --launch-only

# Launch the FRESHLY BUILT exe without stealing focus: the window opens behind
# whatever you are doing and never grabs the cursor.
#
# Boots target/release/ DIRECTLY and deliberately. It must not go through
# archive-build.js --launch-only, which boots the newest v*_HumanityOS.exe in the
# repo root instead: that is the last `just build-game` ARCHIVE, which is older
# than the build you just made. An agent verifying a renderer change against a
# binary that predates the change sees no panic and reports success, which is
# exactly the v0.782-784 / v0.1029-1038 failure class. Found by a doc-truth audit
# on 2026-07-30, hours after this recipe was written with that bug in it.
#
# NOTE this is still only a MENU boot. World entry is the real bar, and the
# in-place exe refuses autopilot when a real identity exists, so it cannot reach
# the world unattended. For anything renderer-shaped use `just verify-runtime`
# (or `just probe-sweep` for a full tour), which builds a portable sandbox,
# enters the world, and sets HUMANITY_NO_FOCUS for you.
#
# The staleness guard is now mechanical (scripts/check-fresh-exe.js) instead of
# a comment asking you to remember: it refuses when the exe is missing or was not
# built from this tree's compiled-in sources (its source stamp, BUG-133).
#
# Launch the freshly built exe behind your work, no focus steal, menu only.
launch-bg:
    @node scripts/check-fresh-exe.js --quiet
    @node scripts/dxc-beside-exe.js
    HUMANITY_NO_FOCUS=1 HUMANITY_NO_HANDOFF=1 ./target/release/HumanityOS.exe

# Check game code for errors (fast, no binary)
check-game:
    cargo check --features native

# Bundle web files (for deploy pipeline)
bundle-web:
    node scripts/bundle-web.js

# ══════════════════════════════════════════════════════════════════════════════
# RELEASE SIGNING — supply-chain root of trust (audit 2026-06-12 CRITICAL fix)
# ══════════════════════════════════════════════════════════════════════════════
# The updater + the local exe-launcher refuse to run a build unless it carries a
# valid HYBRID signature (Ed25519 + Dilithium3, both must verify) from the
# operator's dedicated signing key. The PRIVATE key lives ONLY here, encrypted —
# never in CI (the whole point is to survive a GitHub/CI compromise).
# Full procedure: docs/release-signing.md

# ONE-TIME: generate the hybrid signing keypair. Set HUMANITY_SIGNING_PASSPHRASE
# (>= 12 chars) first. Writes the PUBLIC keys to data/release/signing_pubkeys.json
# (COMMIT it — it compiles into every build) and the encrypted PRIVATE vault to
# release-signing-key.enc (gitignored — BACK IT UP encrypted + offline). Then
# commit the pubkeys and ship a release so the updater begins enforcing.
gen-release-key:
    @test -n "$HUMANITY_SIGNING_PASSPHRASE" || (echo "✗ Set HUMANITY_SIGNING_PASSPHRASE first (>= 12 chars)"; exit 1)
    @test -f target/release/HumanityOS.exe || cargo build --release --features native
    ./target/release/HumanityOS.exe --gen-release-key data/release/signing_pubkeys.json release-signing-key.enc
    @grep -q '"ed25519": "[0-9a-f]' data/release/signing_pubkeys.json \
        && echo "✓ Keypair generated. COMMIT data/release/signing_pubkeys.json, rebuild, then ship + sign a release." \
        || (echo "✗ keygen produced no pubkeys — check the passphrase + try again"; exit 1)

# Sign a PUBLISHED release so the auto-updater will trust it. Run AFTER the tag's
# Build-Desktop workflow has uploaded the platform binaries. Needs
# HUMANITY_SIGNING_PASSPHRASE + release-signing-key.enc.
# Usage:  just sign-release v0.418.0
sign-release version:
    @test -n "$HUMANITY_SIGNING_PASSPHRASE" || (echo "✗ Set HUMANITY_SIGNING_PASSPHRASE first"; exit 1)
    @test -f release-signing-key.enc || (echo "✗ No release-signing-key.enc — run 'just gen-release-key' first"; exit 1)
    @gh release view {{version}} --repo Shaostoul/Humanity --json assets --jq '.assets[].name' 2>/dev/null | grep -q 'HumanityOS-windows-x64.exe' || (echo "✗ No platform binaries on {{version}} yet. The 'Build Desktop App' workflow (triggered by the tag) is still building them -- it takes ~35-40 min from the tag push. Watch it:  gh run list --repo Shaostoul/Humanity --workflow 'Build Desktop App' --limit 3 . Re-run 'just sign-release {{version}}' once that run is green."; exit 1)
    @rm -rf /tmp/humanity-relsign && mkdir -p /tmp/humanity-relsign
    gh release download {{version}} --repo Shaostoul/Humanity -D /tmp/humanity-relsign \
        --pattern 'HumanityOS-windows-x64.exe' --pattern 'HumanityOS-linux-x64' \
        --pattern 'HumanityOS-macos-arm64' --pattern 'HumanityOS-macos-x64'
    node scripts/make-release-manifest.js {{version}} /tmp/humanity-relsign
    @test -f target/release/HumanityOS.exe || cargo build --release --features native
    ./target/release/HumanityOS.exe --sign-release release-manifest.json release-signing-key.enc
    @test -f release-manifest.json.sig.json || (echo "✗ no signature produced — wrong passphrase?"; exit 1)
    gh release upload {{version}} --repo Shaostoul/Humanity release-manifest.json release-manifest.json.sig.json --clobber
    @echo "✓ {{version}} signed — auto-update now trusts it (manifest + sig uploaded)."

# Independently VERIFY a signed release: downloads the manifest + signature +
# the Windows binary and checks both signatures against the committed public
# keys using DIFFERENT crypto impls than the signer (noble ml-dsa + Node's
# Ed25519) plus the artifact hash. Anyone can run this — no key needed.
# Usage: just verify-release v0.421.3
verify-release version:
    @rm -rf /tmp/relverify && mkdir -p /tmp/relverify
    gh release download {{version}} --repo Shaostoul/Humanity -D /tmp/relverify \
        --pattern 'release-manifest.json' --pattern 'release-manifest.json.sig.json' \
        --pattern 'HumanityOS-windows-x64.exe'
    node scripts/verify-release-signature.mjs /tmp/relverify

# ══════════════════════════════════════════════════════════════════════════════
# WORKTREE HYGIENE — prevent context rot
# ══════════════════════════════════════════════════════════════════════════════

# Remove stale git worktrees (keeps main + current). Prevents AI agents
# from writing to cached stale paths. Run this whenever a worktree drifts
# behind main, or after long AI sessions.
#
# By default it REFUSES any worktree holding uncommitted or unmerged work.
# Pass the script's own flag through to override that:
#   just clean-worktrees --force-unmerged
# Without the `*flags` below, just read that argument as the name of a second
# recipe and failed with "Justfile does not contain recipe `--force-unmerged`",
# even though the script has accepted the flag since it was hardened.
clean-worktrees *flags:
    @bash scripts/clean-worktrees.sh --yes {{flags}}

# List all current worktrees with sizes
worktrees:
    @git worktree list

# Reclaim disk: wipe the main Rust build cache AND every agent worktree's target/
# dir (build artifacts ONLY -- source, git, and unmerged worktree commits untouched).
# The main target/ can grow to ~1 TB over many builds; worktree targets pile up ~7 GB
# each. Run when the repo gets huge. `cargo build` repopulates target/ afterward.
clean-heavy:
    @bash scripts/clean-heavy.sh
    @echo ""
    @du -sh .claude/worktrees/ 2>/dev/null || echo "No worktrees directory"

# ══════════════════════════════════════════════════════════════════════════════
# SHORTCUTS — convenience
# ══════════════════════════════════════════════════════════════════════════════

# Open the live site in your default browser
open:
    start https://united-humanity.us

# Search the codebase for a pattern
# Usage: just grep "functionName"
grep pattern:
    grep -r "{{pattern}}" --include="*.js" --include="*.rs" --include="*.html" --include="*.css" -l

# Count lines of code by file type
loc:
    @echo "── Rust ──────────────" && find . -name "*.rs" -not -path "*/target/*" | xargs wc -l 2>/dev/null | tail -1
    @echo "── JavaScript ────────" && find . -name "*.js" -not -path "*/target/*" -not -path "*/node_modules/*" | xargs wc -l 2>/dev/null | tail -1
    @echo "── HTML ──────────────" && find . -name "*.html" -not -path "*/target/*" | xargs wc -l 2>/dev/null | tail -1
    @echo "── CSS ───────────────" && find . -name "*.css" -not -path "*/target/*" | xargs wc -l 2>/dev/null | tail -1

# ══════════════════════════════════════════════════════════════════════════════
# CACHE & DESKTOP — fix stale UI
# ══════════════════════════════════════════════════════════════════════════════

# Clear the WebView2 cache for the Humanity desktop app (Windows)
# Run this if the app is showing old HTML/JS after a deploy
# Make sure the app is CLOSED before running this
clear-desktop-cache:
    @echo "Clearing Humanity desktop WebView2 cache..."
    @rm -rf "$LOCALAPPDATA/us.united-humanity.app/EBWebView/Default/Cache" \
            "$LOCALAPPDATA/us.united-humanity.app/EBWebView/Default/Code Cache" \
            "$LOCALAPPDATA/us.united-humanity.app/EBWebView/Default/GPUCache" \
        && echo "✓ Cache cleared. Reopen the app." \
        || echo "Cache folder not found (app may not have run yet, or path differs)"

# ══════════════════════════════════════════════════════════════════════════════
# GIT HOOKS — install once, runs automatically on every commit
# ══════════════════════════════════════════════════════════════════════════════

# Install GIT pre-commit + pre-push hooks that turn the recurring CI gotchas into fast,
# self-explaining local gates (dead-path block, cargo-fmt blast guard, relay build, the
# five lints; pre-push blocks untracked source). Node-based so it works on the Windows
# host (a `just` shebang recipe needs cygpath, which is absent here). CI (verify.yml) is
# the unconditional backstop; this is the fast local layer. Bypass with --no-verify.
# (Distinct from `node scripts/install-hooks.js`, which installs CLAUDE CODE hooks.)
install-hooks:
    node scripts/install-git-hooks.js

# ══════════════════════════════════════════════════════════════════════════════
# WATCH — auto-deploy on file change (requires watchexec: scoop install watchexec)
# ══════════════════════════════════════════════════════════════════════════════

# Watch web files and auto-sync to VPS on save (HTML/JS/CSS only — fast)
# Good for iterating on front-end. Ctrl+C to stop.
watch-web:
    watchexec --exts html,js,css --on-busy-update restart -- just sync-web

# Watch Rust files and auto-check on save (shows errors without building)
watch-check:
    watchexec --exts rs --on-busy-update restart -- cargo check --features relay --no-default-features 2>&1

# ══════════════════════════════════════════════════════════════════════════════
# NEW PAGE — scaffold a new standalone HTML page
# ══════════════════════════════════════════════════════════════════════════════

# Create a new standalone page from the standard template
# Usage: just new-page market   (creates market.html)
new-page name:
    #!/usr/bin/env bash
    FILE="{{name}}.html"
    if [ -f "$FILE" ]; then
        echo "✗ $FILE already exists"
        exit 1
    fi
    TITLE="$(echo {{name}} | sed 's/\b./\u&/g')"
    cat > "$FILE" << EOF
    <!DOCTYPE html>
    <html lang=en>
    <head>
      <meta charset=UTF-8>
      <meta name=viewport content=width=device-width,initial-scale=1.0>
      <title>${TITLE} — HumanityOS</title>
      <link rel=stylesheet href="/shared/theme.css">
      <style>
        body { background: var(--bg); color: var(--text); font-family: 'Segoe UI', system-ui, sans-serif; min-height: 100vh; display: flex; flex-direction: column; }
        #page-app { flex: 1; padding: 1.5rem; max-width: 960px; margin: 0 auto; width: 100%; }
        h1 { font-size: 1.3rem; font-weight: 700; color: var(--accent); margin-bottom: 0.5rem; }
        p { color: var(--text-muted, #888); font-size: 0.9rem; }
      </style>
    </head>
    <body>
    <script src="/shared/shell.js" data-active="{{name}}"></script>
    <div id="page-app">
      <h1>${TITLE}</h1>
      <p>Coming soon.</p>
    </div>
    <script src="/shared/settings.js"></script>
    </body>
    </html>
    EOF
    echo "✓ Created $FILE — add it to shell.js nav if needed"

# ── Release train (added 2026-07-02, operator-approved dev tooling) ──
# One command for the bump -> commit -> push -> tag -> GitHub-release cycle
# that was previously ~5 hand-built commands per release (and the reason the
# git-commit -m footgun existed at all: every message goes through a file).
# kind = minor (Rust changed) | patch (web/docs/data only).
# Usage: just release minor "short title" path/to/notes.md
#        (notes.md body becomes both the commit body and the release notes;
#        write it first with the Write tool / an editor.)
release kind title notes:
    #!/usr/bin/env bash
    set -euo pipefail
    node scripts/bump-version.js {{kind}}
    VER=$(node -p "require('fs').readFileSync('Cargo.toml','utf8').match(/^version\s*=\s*\"(.+?)\"/m)[1]")
    # Refresh Cargo.lock's own version stamp so it never ships stale.
    cargo check --features relay --no-default-features -q 2>/dev/null || cargo check --features relay --no-default-features
    MSG=$(mktemp)
    { echo "v${VER} - {{title}}"; echo; cat "{{notes}}"; echo; echo "Co-Authored-By: Claude <noreply@anthropic.com>"; } > "$MSG"
    # Stage ONLY the version stamps; stage your own files by name BEFORE calling this
    # (just mine). It used `git add -u`, which in a shared checkout swept every other
    # session's tracked edits into the release commit (flagged 2026-10-19).
    git add -- Cargo.toml Cargo.lock web/shared/sw.js web/pages/settings-app.js web/pages/ops.html web/shared/shell.js web/pages/download.html web/pages/index.html
    git commit -F "$MSG"
    git push origin main
    # The tag through scripts/tag-release.js: it reads the version back from the committed
    # Cargo.toml and refuses an empty version, an existing tag, or a HEAD not on origin/main.
    node scripts/tag-release.js --release "{{notes}}"
    rm -f "$MSG"
    echo "RELEASED v${VER}"

# Tag HEAD v<version> with the version READ from Cargo.toml, and push the tag.
# Refuses, before changing anything: an empty or non-X.Y.Z version, a bump not
# committed yet (HEAD's Cargo.toml says another number), a tag that exists here
# or on origin (or origin not answering), and HEAD not on origin/main. Never runs
# git commit, never moves a tag. scripts/tag-release.js; its test runs in rig-tests.
#   just tag                       tag + push the tag
#   just tag --release notes.md    ... then gh release create v<version> --notes-file notes.md
#   just tag --dry-run             every check, nothing changed
# Tag HEAD v<Cargo.toml version> and push the tag, refusing unsafe states (never commits)
tag *ARGS:
    node scripts/tag-release.js {{ARGS}}

# Fleet overview: every worktree + its branch, commits ahead of main, and
# uncommitted-change count -- the at-a-glance answer to "is any agent work
# unmerged?" that fleet sessions previously hand-rolled with bash loops.
fleet-status:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "main: $(git log --oneline -1)"
    for p in .claude/worktrees/*/; do
      [ -d "$p" ] || continue
      b=$(git -C "$p" rev-parse --abbrev-ref HEAD 2>/dev/null || echo "?")
      ahead=$(git -C "$p" rev-list --count main..HEAD 2>/dev/null || echo "?")
      dirty=$(git -C "$p" status --porcelain 2>/dev/null | grep -vc "settings.local.json" || true)
      echo "$(basename $p): branch=$b ahead=$ahead dirty=$dirty"
    done

# Regenerate ALL page snapshots, then mechanically report which pages CHANGED
# versus the committed PNGs (with PSNR scores when ffmpeg exists: inf = byte
# noise, < ~45 dB = go look). The "did I break a page I didn't touch?" check.
snapshot-check:
    just snapshots
    node scripts/snapshot-diff.js

# ── 3D SCENE sweeps (the running game, not the egui pages) ──
# Capture the canonical 3D vantages (tests/visual/vantages.json) into
# .probe-rig/sweeps/<ts>/: one screenshot + live fps each. Sets up a portable
# rig (junctions to data/ + assets/), boots target/release/HumanityOS.exe,
# drives the tour, and kills it. Needs a release exe + a GPU. Pass driver args
# through, e.g. `just probe-sweep --only moon-surface-200m`.
# This is the exploratory tour (every vantage in the spec, no verdict). For the
# pass/fail gate over the three incident vantages, use `just verify-runtime`.
#
# SETTINGS MATTER AS MUCH AS THE BUILD. The rig's config.json is not the
# operator's: on 2026-08-02 an SSAO A/B on the rig read "nothing here, +-3%
# noise" for a bug that separates 6.6-9.5% at the operator's settings, because
# the rig sat at ssao 0.55 / veg 0.6 / fog 1 / rd 500 while the operator plays
# at 0.96 / 0.10 / 4 / 2000. So:
#   just probe-sweep --operator-config       mirror the operator's live graphics
#                                            settings into the rig before boot
#                                            (visual settings only, never
#                                            identity/vault/server/window)
# Every sweep now records what it ran at in manifest.graphics, and a run WITHOUT
# the flag prints the gap against the operator up front. Never call a visual
# effect absent, weak or fixed from a run that did not mirror.
# GAMEPLAY is the opposite: never mirrored, always pinned. Fresh installs start in
# Normal mode with progress kept (2026-10-04), and every rig is a Dev sandbox that
# starts from the default home, so spawnGame pins play_mode Dev and
# fresh_world_each_launch true into the rig's config.json before every boot
# (scripts/lib/rig-gameplay.js; scripts/tests/rig-gameplay.test.js).
#
# THE BINARY: the exe must be this tree's build (scripts/check-fresh-exe.js, run
# first, before the rig is touched). An A/B against an archive or another
# worktree's exe is a deliberate other-build run and says so:
#   just probe-sweep --exe v0.1444.0_HumanityOS.exe --only blue-marble-12000km --allow-other-build "env control: same cap on the old build?"
# (positional-arguments, so a quoted reason arrives as one argument).
#
# Tour every canonical 3D vantage and capture a screenshot + fps at each.
[positional-arguments]
probe-sweep *ARGS:
    node scripts/probe-sweep.js "$@"

# Film the game for social media: every shot in scripts/clips.json, recorded
# frame-exact in the engine's movie mode (src/engine/movie.rs) and cut to
# 1920x1080, 1080x1920 and a still, into Videos\HumanityOS clips\<date-time>
# with a clips.md of suggested captions. Background and silent like every
# rig, at the operator's graphics settings. Needs a release exe and ffmpeg.
# Gated like every rig (BUG-133): another tree's build needs
# --allow-other-build "<why>".
#   just clips --only orbit-drop-fuji,open-sea
[positional-arguments]
clips *ARGS:
    node scripts/make-clips.js "$@"

# Score cloud GRAIN on captures of one vantage, band by band across the
# terminator, with a measure a blur cannot win: noise over real cloud detail at
# ~15 px, credited only when the real detail is kept. First file = reference;
# arms after it; a <name>-b.png beside a capture adds the fizz between the two.
#   just terminator-grain base.png arm.png --band 5 --controls
# --controls blurs every capture and exits 1 if any blur is judged BETTER. The
# old 4-neighbour number is still printed as "speckle (old)" for comparison
# with pre-2026-09-27 figures; a 3x3 blur wins it, so never rank arms by it.
# Capture the pair with:
#   just probe-sweep --only orbit-terminator-3000km,orbit-terminator-3000km-b --operator-config
terminator-grain *ARGS:
    node scripts/terminator-grain.js {{ARGS}}

# Capture + print the fps/frame-time table across every canonical vantage,
# flagging any stop below its advisory floor. Exit 2 if any is below floor or
# a PANIC occurred - so it can gate. The perf half of the sweep; deterministic,
# no AI. For a visual-regression pass, run the visual-sweep workflow.
perf-sweep:
    node scripts/probe-sweep.js
    node scripts/perf-report.js

# Diff the newest sweep's fps against a baseline manifest to catch a slowdown
# that is still above floor: `just perf-diff .probe-rig/sweeps/<old>/manifest.json`
perf-diff baseline:
    node scripts/perf-report.js --baseline {{baseline}}

# Live game-data counts (items/plants/creatures/recipes + data files) so the
# inventory figures in STATUS.md/FEATURES.md can be re-synced instead of drifting.
data-counts:
    @node scripts/data-counts.js

# Regenerate data/roadmap.json (the website mirror) from docs/ROADMAP.md.
# Run after editing ROADMAP.md - the site renders the JSON, not the markdown.
roadmap:
    node scripts/roadmap-to-json.js
