# HumanityOS — dev command runner
# Install: winget install Casey.Just  (or: scoop install just)
# Run any recipe: just <name>
#
# Most useful daily commands:
#   just ship "my commit message"   ← commit + push + force-sync VPS (recommended)
#   just deploy "my message"        ← commit + push only (waits for CI)
#   just sync                       ← force-sync VPS right now, no commit needed
#   just logs                       ← watch live server logs
#   just ping                       ← check if the site is alive

set shell := ["bash", "-c"]

# ── Default: list all available recipes ──────────────────────────────────────
default:
    @just --list

# ══════════════════════════════════════════════════════════════════════════════
# DEPLOY
# ══════════════════════════════════════════════════════════════════════════════

# Commit everything and push — triggers CI deploy pipeline
deploy msg="chore: update":
    git add -A
    git diff --cached --quiet || git commit -m "{{msg}}"
    git push origin main
    @echo ""
    @echo "✓ Pushed to main. CI deploy triggered."
    @echo "  Watch it: just ci"

# Force-sync VPS with current main right now (no CI wait, no commit needed)
# Use this when CI fails or you need an instant deploy
sync:
    @echo "→ Syncing VPS with origin/main..."
    ssh humanity-vps " \
        cd /opt/Humanity && \
        git fetch origin main && \
        git reset --hard origin/main && \
        git clean -fd --exclude=backups/ --exclude=data/ --exclude=target/ && \
        export PATH=\$HOME/.cargo/bin:\$PATH && \
        cargo build --release --bin humanity-relay 2>&1 | tail -3 && \
        rsync -a --delete /opt/Humanity/crates/humanity-relay/client/ /var/www/humanity/chat/ && \
        rsync -a /opt/Humanity/shared/ /var/www/humanity/shared/ && \
        for f in /opt/Humanity/*.html; do \
            [ -f \"\$f\" ] && cp \"\$f\" \"/var/www/humanity/\$(basename \"\$f\")\"; \
        done && \
        systemctl restart humanity-relay && sleep 2 && systemctl is-active humanity-relay \
    "
    @echo "✓ VPS synced and relay restarted."

# THE ONE COMMAND: commit + push + immediately force-sync VPS
# Bypasses CI wait — great when you want changes live *right now*
ship msg="chore: update":
    just deploy "{{msg}}"
    just sync

# Sync web assets only (no Rust rebuild — fast, for HTML/JS/CSS changes)
sync-web:
    @echo "→ Syncing web assets only (no rebuild)..."
    ssh humanity-vps " \
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
    @echo "✓ Web assets synced (relay NOT restarted)."

# ══════════════════════════════════════════════════════════════════════════════
# STATUS & MONITORING
# ══════════════════════════════════════════════════════════════════════════════

# Check recent CI deploy runs
ci:
    gh run list --repo Shaostoul/Humanity --workflow "Deploy to VPS" --limit 5

# Check if site is alive and API is responding
ping:
    @curl -sf https://united-humanity.us/api/tasks \
        | grep -o '"tasks":\[' \
        && echo "✓ API alive" \
        || echo "✗ API unreachable"

# Full status check: git, CI, and live API
status:
    @echo "── Git ──────────────────────────────────────"
    @git log --oneline -3
    @echo ""
    @echo "── CI deploys ───────────────────────────────"
    @gh run list --repo Shaostoul/Humanity --workflow "Deploy to VPS" --limit 3
    @echo ""
    @echo "── Live API ─────────────────────────────────"
    @just ping

# ══════════════════════════════════════════════════════════════════════════════
# SERVER
# ══════════════════════════════════════════════════════════════════════════════

# Tail live server logs (Ctrl+C to stop)
logs:
    ssh humanity-vps "journalctl -u humanity-relay -f --no-pager"

# Open SSH session on the VPS
server:
    ssh humanity-vps

# Restart relay only (no rebuild, instant)
restart:
    ssh humanity-vps "systemctl restart humanity-relay && systemctl is-active humanity-relay"

# Show last 50 log lines (non-streaming)
log-tail:
    ssh humanity-vps "journalctl -u humanity-relay -n 50 --no-pager"

# ══════════════════════════════════════════════════════════════════════════════
# LOCAL DEV
# ══════════════════════════════════════════════════════════════════════════════

# Build relay binary locally
build:
    cargo build --release --bin humanity-relay

# Run relay locally for development
run:
    cargo run --bin humanity-relay

# Check for Rust warnings / errors without building
check:
    cargo check --bin humanity-relay
