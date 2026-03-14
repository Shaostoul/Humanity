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
# ─────────────────────────────────────────────────────────────────────────────

set shell := ["bash", "-c"]

# List all recipes
default:
    @just --list

# ══════════════════════════════════════════════════════════════════════════════
# DEPLOY — getting code to production
# ══════════════════════════════════════════════════════════════════════════════

# THE ONE COMMAND: commit + push + immediately force-sync VPS (bypasses CI wait)
ship msg="chore: update":
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
        rsync -a --delete /opt/Humanity/crates/humanity-relay/client/ /var/www/humanity/chat/ && \
        rsync -a /opt/Humanity/shared/ /var/www/humanity/shared/ && \
        for f in /opt/Humanity/*.html; do \
            [ -f \"\$f\" ] && cp \"\$f\" \"/var/www/humanity/\$(basename \"\$f\")\"; \
        done && \
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
        rsync -a --delete /opt/Humanity/crates/humanity-relay/client/ /var/www/humanity/chat/ && \
        rsync -a /opt/Humanity/shared/ /var/www/humanity/shared/ && \
        for f in /opt/Humanity/*.html; do \
            [ -f \"\$f\" ] && cp \"\$f\" \"/var/www/humanity/\$(basename \"\$f\")\"; \
        done \
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
# DESKTOP APP — Tauri wrapper
# ══════════════════════════════════════════════════════════════════════════════

# Build the desktop app (release)
build-desktop:
    cd desktop && cargo tauri build

# Run the desktop app in dev mode (hot reload)
dev-desktop:
    cd desktop && cargo tauri dev

# Check desktop app for Rust errors
check-desktop:
    cd desktop/src-tauri && cargo check

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
