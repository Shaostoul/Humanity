#!/bin/bash
# =============================================================================
# provision-vps.sh — build the HumanityOS server from a bare Debian 12 install.
#
# Born from the 2026-08-07 incident (see docs/INCIDENT-PLAYBOOK.md, "The TURN
# relay abuse"): the old box was hand-built over months, so its real state and
# the documented state drifted — a security migration written in docs was never
# run on the machine, and the gap was found by an abuse team, not by us. THE
# RULE THIS SCRIPT ENFORCES: everything the server is, this script makes. If a
# change is not in here (or in a file this script installs from the repo), the
# change does not exist after the next rebuild — which is exactly the property
# that makes a rebuild survivable.
#
# Idempotent: safe to re-run at any time; every step checks before it changes.
# Run as root on the VPS, from a clone of the repo:
#   git clone https://github.com/Shaostoul/Humanity.git /opt/Humanity
#   bash /opt/Humanity/scripts/provision-vps.sh
#
# What it deliberately does NOT install:
#   - coturn: BANNED until the ephemeral-credential migration
#     (docs/admin/turn-rotation.md) is implemented IN THIS SCRIPT. The old
#     static-credential config is what got the server null-routed.
#   - transmission (release seeding): reinstate later with RPC bound to
#     localhost and upload caps; it is not launch-critical.
#   - forgejo (git mirror): reinstate later from docs/admin/forgejo-setup.md;
#     it is a mirror of GitHub and loses nothing by waiting.
# =============================================================================
set -euo pipefail

REPO=/opt/Humanity
DOMAIN=united-humanity.us
CHAT_DOMAIN=chat.united-humanity.us
CERT_MAIL=shaostoul@gmail.com

say() { echo -e "\n=== $* ==="; }

[ "$(id -u)" = 0 ] || { echo "run as root"; exit 1; }
[ -d "$REPO/.git" ] || { echo "clone the repo to $REPO first"; exit 1; }

# ── 1. Base packages ─────────────────────────────────────────────────────────
say "packages"
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq nginx certbot fail2ban nftables git curl \
  build-essential pkg-config libssl-dev sqlite3 unattended-upgrades

# ── 2. Rust toolchain (the relay builds on the box, same as CI deploy) ───────
say "rust"
if ! command -v cargo >/dev/null 2>&1; then
  curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal
fi
# shellcheck disable=SC1091
. "$HOME/.cargo/env" 2>/dev/null || true

# ── 3. Swap: the old box had 17 MB and died THRASHING instead of recovering ──
say "swap"
if ! swapon --show | grep -q /swapfile; then
  fallocate -l 4G /swapfile && chmod 600 /swapfile && mkswap /swapfile && swapon /swapfile
  grep -q '^/swapfile' /etc/fstab || echo '/swapfile none swap sw 0 0' >> /etc/fstab
fi

# ── 4. Persistent journal: the old box's journal lived in RAM, so the outage
#       investigation had no history. Never again. ───────────────────────────
say "journald"
mkdir -p /var/log/journal
systemd-tmpfiles --create --prefix /var/log/journal 2>/dev/null || true
systemctl restart systemd-journald

# ── 5. SSH: key-only, no passwords. (Console tty login is unaffected.) ───────
say "sshd"
install -d -m 700 /root/.ssh
# The operator key(s); public halves, committed to the repo on purpose.
install -m 600 "$REPO/ops/vps/operator-pubkeys.txt" /root/.ssh/authorized_keys
printf 'PermitRootLogin prohibit-password\nPasswordAuthentication no\n' \
  > /etc/ssh/sshd_config.d/harden.conf
systemctl reload ssh

# ── 6. No autologin consoles. The old provider image autologged root on the
#       serial console: a passwordless root shell for anyone with panel access.
say "getty"
rm -rf /etc/systemd/system/serial-getty@*.service.d \
       /etc/systemd/system/getty@tty1.service.d
systemctl daemon-reload

# ── 7. Firewall: default-deny inbound. 22/80/443 and nothing else.
#       NO 3478/5349 (TURN) — see the header. ────────────────────────────────
say "nftables"
cat > /etc/nftables.conf <<'NFT'
#!/usr/sbin/nft -f
flush ruleset
table inet filter {
  chain input {
    type filter hook input priority 0; policy drop;
    ct state established,related accept
    iif lo accept
    ip protocol icmp accept
    meta l4proto ipv6-icmp accept
    tcp dport { 22, 80, 443 } accept
  }
  chain forward { type filter hook forward priority 0; policy drop; }
  chain output  { type filter hook output  priority 0; policy accept; }
}
NFT
systemctl enable --now nftables
nft -f /etc/nftables.conf

# ── 8. fail2ban (sshd jail on by default) + unattended security updates ──────
say "fail2ban + unattended-upgrades"
systemctl enable --now fail2ban
printf 'APT::Periodic::Update-Package-Lists "1";\nAPT::Periodic::Unattended-Upgrade "1";\n' \
  > /etc/apt/apt.conf.d/20auto-upgrades

# ── 9. The relay: user, env, build, unit ─────────────────────────────────────
say "relay"
id -u humanity >/dev/null 2>&1 || useradd -r -s /bin/false humanity
mkdir -p "$REPO/data"
if [ ! -f "$REPO/.env" ]; then
  # ADMIN_KEYS intentionally left empty: it is the operator's chat public key,
  # set once by the operator (docs/admin/SELF-HOSTING.md "first admin").
  printf 'ADMIN_KEYS=\nAPI_SECRET=%s\nRUST_LOG=info\n' "$(openssl rand -hex 32)" > "$REPO/.env"
  chmod 600 "$REPO/.env"
fi
( cd "$REPO" && cargo build --release --features relay --no-default-features )
cat > /etc/systemd/system/humanity-relay.service <<UNIT
[Unit]
Description=Humanity Network Relay
After=network.target

[Service]
Type=simple
User=humanity
Group=humanity
WorkingDirectory=$REPO
ExecStart=$REPO/target/release/HumanityOS --headless
EnvironmentFile=$REPO/.env
Restart=always
RestartSec=5
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=$REPO/data
NoNewPrivileges=true
PrivateTmp=true

[Install]
WantedBy=multi-user.target
UNIT
chown -R humanity:humanity "$REPO/data"
systemctl daemon-reload
systemctl enable humanity-relay

# ── 10. Watchdog + disk guard (repo-owned units) ─────────────────────────────
say "watchdog"
install -m 644 "$REPO"/scripts/systemd/humanity-*.{service,timer} /etc/systemd/system/
systemctl daemon-reload
systemctl enable --now humanity-relay-watchdog.timer humanity-disk-guard.timer

# ── 11. Web root + nginx. Certs must exist before the TLS config loads, so:
#        certbot standalone first (needs :80 free), then the real config. ─────
say "nginx + certs"
mkdir -p /var/www/humanity
rsync -a --delete "$REPO/web/" /var/www/humanity/
if [ ! -d "/etc/letsencrypt/live/$DOMAIN" ]; then
  systemctl stop nginx || true
  certbot certonly --standalone -n --agree-tos -m "$CERT_MAIL" \
    -d "$DOMAIN" -d "www.$DOMAIN" -d "$CHAT_DOMAIN" || {
      echo "!! certbot failed (network/DNS not ready?) - nginx left stopped; re-run when ready"; exit 1; }
fi
install -m 644 "$REPO/scripts/nginx/humanity.conf" /etc/nginx/sites-available/humanity
ln -sf /etc/nginx/sites-available/humanity /etc/nginx/sites-enabled/humanity
rm -f /etc/nginx/sites-enabled/default
nginx -t
systemctl enable --now nginx
systemctl reload nginx

# ── 12. DB restore, if a backup was delivered ────────────────────────────────
say "database"
if [ -f /root/relay-restore.db ]; then
  systemctl stop humanity-relay || true
  install -o humanity -g humanity -m 644 /root/relay-restore.db "$REPO/data/relay.db"
  echo "restored relay.db from /root/relay-restore.db"
fi
mkdir -p "$REPO/backups" && chown humanity:humanity "$REPO/backups"
systemctl start humanity-relay

# ── 13. Config drift assertions: the class of failure that caused the rebuild.
#        Each one is a promise the deploy pipeline can also check remotely. ───
say "assertions"
fail=0
if [ -f /etc/turnserver.conf ] && grep -q "lt-cred-mech" /etc/turnserver.conf; then
  echo "!! coturn is installed with static credentials - the exact config that got the server null-routed"; fail=1
fi
sshd -T | grep -q "^passwordauthentication no" || { echo "!! sshd accepts passwords"; fail=1; }
nft list ruleset | grep -q "policy drop" || { echo "!! firewall is not default-deny"; fail=1; }
swapon --show | grep -q swap || { echo "!! no swap"; fail=1; }
[ -d /var/log/journal ] || { echo "!! journal is not persistent"; fail=1; }
curl -fsS -m 5 http://127.0.0.1:3210/health >/dev/null || { echo "!! relay /health not answering"; fail=1; }
[ $fail = 0 ] && say "PROVISION COMPLETE - all assertions pass" || { say "PROVISION FINISHED WITH FAILURES"; exit 1; }
