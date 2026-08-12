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

# ── Deployment identity ──────────────────────────────────────────────────────
# Everything site-specific is a variable with the canonical instance as its
# default, so `bash provision-vps.sh` still rebuilds united-humanity.us exactly,
# while a self-hoster overrides via a node.env file or the environment:
#
#   HUMANITY_DOMAIN=shop.example.com HUMANITY_CERT_MAIL=me@example.com \
#     bash provision-vps.sh
#
# or drop /opt/Humanity/node.env (gitignored) with KEY=value lines. This is the
# seam that turns "our recovery script" into "anyone's installer" - a merchant
# federating into the network changes these and nothing else in this file.
REPO="${HUMANITY_REPO:-/opt/Humanity}"
[ -f "$REPO/node.env" ] && { set -a; . "$REPO/node.env"; set +a; }
DOMAIN="${HUMANITY_DOMAIN:-united-humanity.us}"
CHAT_DOMAIN="${HUMANITY_CHAT_DOMAIN:-chat.$DOMAIN}"
CERT_MAIL="${HUMANITY_CERT_MAIL:-shaostoul@gmail.com}"
# Extra cert names (space-separated). The canonical instance keeps its www-less
# apex + chat subdomain; a self-hoster with only an apex leaves this empty.
CERT_EXTRA_DOMAINS="${HUMANITY_CERT_EXTRA:-}"

say() { echo -e "\n=== $* ==="; }

[ "$(id -u)" = 0 ] || { echo "run as root"; exit 1; }
[ -d "$REPO/.git" ] || { echo "clone the repo to $REPO first"; exit 1; }
say "provisioning $DOMAIN (chat: $CHAT_DOMAIN, cert mail: $CERT_MAIL)"

# ── 1. Base packages ─────────────────────────────────────────────────────────
say "packages"
export DEBIAN_FRONTEND=noninteractive
# Provider templates ship with interrupted package state more often than not,
# and grub-pc's postinst asks an interactive "which disk?" question that hangs
# forever without a tty (it cost this script's first real run 20 minutes).
# Answer it up front and repair dpkg before asking apt for anything.
BOOT_DISK="/dev/$(lsblk -dno NAME,TYPE | awk '$2=="disk"{print $1; exit}')"
echo "grub-pc grub-pc/install_devices multiselect $BOOT_DISK" | debconf-set-selections
dpkg --configure -a || true
apt-get update -qq
apt-get install -y -qq nginx certbot fail2ban nftables git curl rsync \
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
# Minimal Debian 12 has no rsyslog, so /var/log/auth.log does not exist and
# fail2ban's default sshd jail CRASHES - a few seconds AFTER systemctl has
# already returned success, so a naive `enable --now` looks green while the
# service dies behind it. Point it at the systemd journal instead, and let the
# assertion block at the end catch any regression.
printf '[sshd]
enabled = true
backend = systemd
' > /etc/fail2ban/jail.d/sshd-systemd.conf
systemctl enable --now fail2ban
systemctl restart fail2ban
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
# Build only if no binary exists. Provisioning defines the MACHINE; keeping
# the binary current is the deploy pipeline's job. (On this VPS a build is
# ~31 minutes and something invalidates cargo's cache between provision runs,
# so an unconditional build turned every re-run into an hour.)
[ -x "$REPO/target/release/HumanityOS" ] || \
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
# One layout definition, shared with CI's deploy step (see the header of
# sync-web-root.sh for why a second copy of this logic is banned).
bash "$REPO/scripts/sync-web-root.sh" "$REPO" /var/www/humanity
if [ ! -d "/etc/letsencrypt/live/$DOMAIN" ]; then
  systemctl stop nginx || true
  # One cert covering apex + chat subdomain + any HUMANITY_CERT_EXTRA. Each name
  # must resolve to THIS box or certbot fails the whole order (a www that never
  # existed cost this script's second run - so www is opt-in, not assumed).
  cert_args=(-d "$DOMAIN" -d "$CHAT_DOMAIN")
  for extra in $CERT_EXTRA_DOMAINS; do cert_args+=(-d "$extra"); done
  certbot certonly --standalone -n --agree-tos -m "$CERT_MAIL" "${cert_args[@]}" || {
      echo "!! certbot failed (DNS for $DOMAIN / $CHAT_DOMAIN not pointing here yet?) - nginx left stopped; re-run when ready"; exit 1; }
fi
# The site config includes two certbot companion files that only the
# certbot-NGINX plugin creates; standalone certonly does not (third landmine
# this script's first live run found). Both are public, stable content:
# the options file is certbot's standard TLS config, the dhparams are the
# RFC 7919 ffdhe2048 group - the very bytes certbot ships.
if [ ! -f /etc/letsencrypt/options-ssl-nginx.conf ]; then
  cat > /etc/letsencrypt/options-ssl-nginx.conf <<'SSLOPT'
ssl_session_cache shared:le_nginx_SSL:10m;
ssl_session_timeout 1440m;
ssl_session_tickets off;
ssl_protocols TLSv1.2 TLSv1.3;
ssl_prefer_server_ciphers off;
ssl_ciphers "ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384:ECDHE-ECDSA-CHACHA20-POLY1305:ECDHE-RSA-CHACHA20-POLY1305:DHE-RSA-AES128-GCM-SHA256:DHE-RSA-AES256-GCM-SHA384:DHE-RSA-CHACHA20-POLY1305";
SSLOPT
fi
if [ ! -f /etc/letsencrypt/ssl-dhparams.pem ]; then
  cat > /etc/letsencrypt/ssl-dhparams.pem <<'DHPARAM'
-----BEGIN DH PARAMETERS-----
MIIBCAKCAQEA//////////+t+FRYortKmq/cViAnPTzx2LnFg84tNpWp4TZBFGQz
+8yTnc4kmz75fS/jY2MMddj2gbICrsRhetPfHtXV/WVhJDP1H18GbtCFY2VVPe0a
87VXE15/V8k1mE8McODmi3fipona8+/och3xWKE2rec1MKzKT0g6eXq8CrGCsyT7
YdEIqUuyyOP7uWrat2DX9GgdT0Kj3jlN9K5W7edjcrsZCwenyO4KbXCeAvzhzffi
7MA0BM0oNC9hkXL+nOmFg/+OTxIy7vKBg8P+OxtMb61zO7X8vC7CIAXFjvGDfRaD
ssbzSibBsu/6iGtCOGEoXJf//////////wIBAg==
-----END DH PARAMETERS-----
DHPARAM
fi
# Debian's nginx computes a 32-byte server-names hash bucket on this CPU,
# too small for our domain set (fourth landmine); 64 is the universal fix.
printf 'server_names_hash_bucket_size 64;\n' > /etc/nginx/conf.d/hash-bucket.conf
# The rate-limit ZONES the site config consumes. Their declarations lived in
# a hand-edited nginx.conf on the old box and were never in the repo (fifth
# landmine, and the incident's whole lesson in miniature). limit_req_zone is
# only legal at http{} scope, hence a conf.d snippet rather than the vhost.
cat > /etc/nginx/conf.d/humanity-limits.conf <<'LIMITS'
limit_req_zone $binary_remote_addr zone=api_read_limit:10m rate=10r/s;
limit_req_zone $binary_remote_addr zone=upload_limit:10m rate=2r/m;
LIMITS
install -m 644 "$REPO/scripts/nginx/humanity.conf" /etc/nginx/sites-available/humanity
# The repo config is written for the canonical instance; substitute this
# node's domains into the installed copy (server_name lines AND letsencrypt
# paths). Ordered: the chat subdomain first, else the apex rule eats it.
sed -i "s/chat\.united-humanity\.us/$CHAT_DOMAIN/g; s/united-humanity\.us/$DOMAIN/g" \
  /etc/nginx/sites-available/humanity
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
# Captured, not piped: under pipefail, `producer | grep -q` fails on the
# producer's SIGPIPE when grep quits at the first match (sixth landmine).
sshd_eff="$(/usr/sbin/sshd -T 2>/dev/null || true)"
grep -q "^passwordauthentication no" <<<"$sshd_eff" || { echo "!! sshd accepts passwords"; fail=1; }
nft_rules="$(/usr/sbin/nft list ruleset 2>/dev/null || true)"
grep -q "policy drop" <<<"$nft_rules" || { echo "!! firewall is not default-deny"; fail=1; }
swapon --show | grep -q swap || { echo "!! no swap"; fail=1; }
[ -d /var/log/journal ] || { echo "!! journal is not persistent"; fail=1; }
ok=0; for i in $(seq 1 15); do curl -fsS -m 3 http://127.0.0.1:3210/health >/dev/null 2>&1 && { ok=1; break; }; sleep 2; done
[ $ok = 1 ] || { echo "!! relay /health not answering after 30s"; fail=1; }
sleep 3; systemctl is-active --quiet fail2ban || { echo "!! fail2ban is not running (it can die AFTER start reports success)"; fail=1; }
[ $fail = 0 ] && say "PROVISION COMPLETE - all assertions pass" || { say "PROVISION FINISHED WITH FAILURES"; exit 1; }
