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

set shell := ["bash", "-c"]

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

# THE ONE COMMAND: bump version + bundle web + commit + push + immediately force-sync VPS
ship msg="chore: update":
    @just bump
    @just bundle-web
    @just _commit "{{msg}}"
    @just sync

# Commit + push only — waits for CI to deploy (~5 min)
deploy msg="chore: update":
    @just _commit "{{msg}}"
    @echo ""
    @echo "✓ Pushed. CI deploy triggered — watch with: just ci"

# Internal: stage everything and commit (skips if nothing staged)
_commit msg:
    git add -A
    git diff --cached --quiet || git commit -m "{{msg}}"
    git push origin main

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
        export PATH=\$HOME/.cargo/bin:\$PATH && \
        cargo build --release --bin humanity-relay 2>&1 | tail -4 && \
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
        rsync -a /opt/Humanity/web/activities/ /var/www/humanity/activities/ && \
        rsync -a --include='*.json' --exclude='*' /opt/Humanity/data/ /var/www/humanity/data/ && \
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
        rsync -a /opt/Humanity/web/activities/ /var/www/humanity/activities/ && \
        rsync -a --include='*.json' --exclude='*' /opt/Humanity/data/ /var/www/humanity/data/ \
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
    @echo "── Live site ────────────────────────────────"
    @just ping

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

# Check relay for errors (fast, no binary output)
check:
    cargo check --bin humanity-relay

# Build relay binary locally
build:
    cargo build --release --bin humanity-relay

# Run relay locally for development (uses local SQLite)
run:
    cargo run --bin humanity-relay

# Run formatter
fmt:
    cargo fmt

# Run clippy linter
clippy:
    cargo clippy --bin humanity-relay -- -D warnings

# ══════════════════════════════════════════════════════════════════════════════
# DESKTOP APP — local-first Tauri wrapper
# ══════════════════════════════════════════════════════════════════════════════

# Bundle web files into app/web/ (run before tauri build)
bundle-web:
    node scripts/bundle-web.js

# Build the desktop app (bundles web + compiles Tauri)
build-desktop: bundle-web
    cd app && npx tauri build

# Run the desktop app in dev mode (hot reload)
dev-desktop:
    cd app && npx tauri dev

# Check desktop app for Rust errors
check-desktop:
    cd app && cargo check

# Full release: bump version, bundle, build, ship
# Usage: just release          → patch bump + build + ship
#        just release minor    → minor bump + build + ship
release kind="patch":
    node scripts/bump-version.js {{kind}}
    node scripts/bundle-web.js
    just ship "Release v$(node -p \"require('./app/tauri.conf.json').version\")"

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

# Install a pre-commit hook that runs `cargo check` before every git commit
# Prevents shipping broken Rust — run once after cloning
install-hooks:
    #!/usr/bin/env bash
    HOOK=.git/hooks/pre-commit
    cat > "$HOOK" << 'HOOKEOF'
    #!/usr/bin/env bash
    # Auto-installed by: just install-hooks
    echo "→ pre-commit: cargo check..."
    if ! cargo check --bin humanity-relay -q 2>&1; then
        echo "✗ Rust errors found. Fix before committing. (bypass with git commit --no-verify)"
        exit 1
    fi
    echo "✓ Rust OK"
    HOOKEOF
    chmod +x "$HOOK"
    echo "✓ Pre-commit hook installed at $HOOK"

# ══════════════════════════════════════════════════════════════════════════════
# WATCH — auto-deploy on file change (requires watchexec: scoop install watchexec)
# ══════════════════════════════════════════════════════════════════════════════

# Watch web files and auto-sync to VPS on save (HTML/JS/CSS only — fast)
# Good for iterating on front-end. Ctrl+C to stop.
watch-web:
    watchexec --exts html,js,css --on-busy-update restart -- just sync-web

# Watch Rust files and auto-check on save (shows errors without building)
watch-check:
    watchexec --exts rs --on-busy-update restart -- cargo check --bin humanity-relay 2>&1

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
