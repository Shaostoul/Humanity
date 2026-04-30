# Forgejo Self-Host on the VPS

> Status: live at [git.united-humanity.us](https://git.united-humanity.us) since v0.127.0 (2026-04-29).
> Step 1 of the [distribution-mirrors](distribution-mirrors.md) plan.

This is the sovereignty layer for HumanityOS source code. GitHub stays the
discovery layer (download links, CI builds, the public face); Forgejo is the
copy you control. `just ship` pushes to **both** so neither one is a single
point of failure.

## What's running

| Component | Where | Notes |
|-----------|-------|-------|
| Forgejo binary | `/usr/local/bin/forgejo` | v15.0.0, single Go binary, statically linked, ~113 MB |
| Config | `/etc/forgejo/app.ini` | owned `forgejo:forgejo`, mode `640` |
| Data | `/var/lib/forgejo/data/` | SQLite DB at `forgejo.db`, repos at `forgejo-repositories/`, LFS at `lfs/` |
| Logs | `/var/lib/forgejo/log/` | one log per service component |
| systemd unit | `/etc/systemd/system/forgejo.service` | `User=forgejo`, hardening flags |
| nginx vhost | `/etc/nginx/sites-available/git.united-humanity.us` | reverse proxy `127.0.0.1:3000`, `client_max_body_size 1024m` for LFS |
| TLS | `/etc/letsencrypt/live/git.united-humanity.us/` | Let's Encrypt, auto-renews via certbot timer |
| Git | `/usr/local/bin/git` | 2.45.2 built from source — Debian 11's bundled 2.30.2 was too old for Forgejo (needs ≥2.34.1) |

## What's the public surface

- **Web UI**: https://git.united-humanity.us
- **HTTPS clone**: `https://git.united-humanity.us/shaostoul/humanity.git`
- **SSH clone**: `forgejo@git.united-humanity.us:shaostoul/humanity.git` (system sshd on port 22; Forgejo's `RUN_USER=forgejo` so SSH user is `forgejo`, NOT `git` — there is no `git` system user on the VPS)
- **Self-registration**: disabled. Only the admin (`shaostoul`) can create accounts.
- **OpenID sign-in**: disabled.
- **API**: anonymous read access on public repos via `https://git.united-humanity.us/api/v1/...`

## Multi-remote `just ship`

The `_commit` recipe in the Justfile now pushes to both remotes:

```
git push origin main      # required (GitHub)
git push forge main       # best-effort (Forgejo) — `-` prefix so it doesn't block ship
```

Tag push works the same way: tags go to `origin` first (GitHub Actions Build
Desktop App workflow keys off this), then to `forge`. A transient Forgejo
outage doesn't block a ship.

To register the second remote on a fresh clone, **prefer SSH** — credentials never expire, no token rotation, no GCM cache invalidation. HTTPS is supported but is fragile; see "Why SSH" below.

```bash
# SSH (recommended)
git remote add forge forgejo@git.united-humanity.us:<user>/humanity.git
```

### SSH setup (one-time)

1. **Add your public key to Forgejo:** https://git.united-humanity.us/-/user/settings/keys → "Add Key"
2. **Add a Host entry to `~/.ssh/config`** so git uses the right key:
   ```
   Host git.united-humanity.us
     User forgejo
     IdentityFile ~/.ssh/<your_key>
     IdentitiesOnly yes
     StrictHostKeyChecking accept-new
   ```
3. **Verify auth:**
   ```bash
   ssh -T forgejo@git.united-humanity.us
   # → "Hi there, <user>! You've successfully authenticated with the key named ..."
   ```
4. **Verify git can reach it:**
   ```bash
   git ls-remote forge HEAD
   # → prints the HEAD commit hash
   ```

### Why SSH (avoid HTTPS)

HTTPS to Forgejo uses Windows Git Credential Manager via browser SSO. The cached
token expires/invalidates without warning, and `git push forge main` then dies
with `Credentials are incorrect or have expired`. Recovery is non-obvious — you
have to manually erase the cached creds before the next push will re-prompt:

```bash
printf "protocol=https\nhost=git.united-humanity.us\n\n" | git credential reject
printf "protocol=https\nhost=git.united-humanity.us\n\n" | git credential-manager erase
git push forge main   # GCM re-prompts and refreshes
```

SSH keys don't expire, don't depend on browser SSO, and don't need GCM.
Once the per-host config in `~/.ssh/config` is in place, every clone, fetch,
push, and tag-push works without ceremony.

### HTTPS fallback (only if SSH unavailable)

```bash
git remote add forge https://git.united-humanity.us/<user>/humanity.git
```

On Windows, GCM auto-prompts via browser SSO on first push. On Linux/macOS,
generate a Personal Access Token in **Settings → Applications → Generate New
Token** and paste as the password on first push.

## Operations

| Action | Command |
|--------|---------|
| Service status | `ssh humanity-vps 'systemctl status forgejo --no-pager'` |
| Restart | `ssh humanity-vps 'sudo systemctl restart forgejo'` |
| Tail logs | `ssh humanity-vps 'sudo journalctl -u forgejo -f'` |
| Tail HTTP error log | `ssh humanity-vps 'sudo tail -f /var/lib/forgejo/log/forgejo.log'` |
| App config | `ssh humanity-vps 'sudo nano /etc/forgejo/app.ini'` (then restart) |
| Database backup | `ssh humanity-vps 'sudo -u forgejo cp /var/lib/forgejo/data/forgejo.db /var/lib/forgejo/data/forgejo.db.bak'` |
| Upgrade Forgejo | replace `/usr/local/bin/forgejo` with new release binary, restart service. Read release notes for migrations. |

## Reproducing the install (for future operators)

The exact sequence used for the original setup, ordered:

```bash
# DNS A record: git.united-humanity.us → VPS IP (out of scope for this doc)

# On VPS:
# 1. Install Forgejo
sudo curl -sL -o /usr/local/bin/forgejo \
    'https://codeberg.org/forgejo/forgejo/releases/download/v15.0.0/forgejo-15.0.0-linux-amd64'
sudo chmod +x /usr/local/bin/forgejo

# 2. Create user + dirs
sudo useradd --system --shell /bin/bash --create-home \
    --home-dir /var/lib/forgejo --user-group forgejo
sudo mkdir -p /var/lib/forgejo/{custom,data,log}
sudo chown -R forgejo:forgejo /var/lib/forgejo
sudo chmod 750 /var/lib/forgejo
sudo mkdir -p /etc/forgejo
sudo chown forgejo:forgejo /etc/forgejo
sudo chmod 750 /etc/forgejo

# 3. Newer git (Debian 11 ships 2.30.2, Forgejo needs ≥2.34.1)
sudo apt install -y build-essential libssl-dev libcurl4-openssl-dev libexpat1-dev gettext zlib1g-dev
cd /tmp && curl -sLO https://mirrors.edge.kernel.org/pub/software/scm/git/git-2.45.2.tar.gz
tar xzf git-2.45.2.tar.gz && cd git-2.45.2
make prefix=/usr/local NO_TCLTK=1 NO_GETTEXT=1 NO_PERL=1 -j4 all
sudo make prefix=/usr/local NO_TCLTK=1 NO_GETTEXT=1 NO_PERL=1 install

# 4. app.ini (see /etc/forgejo/app.ini for current contents — pre-configured paths,
# git binary path, server domain, root URL, disable self-registration, SQLite3)

# 5. systemd unit (see /etc/systemd/system/forgejo.service)
sudo systemctl daemon-reload
sudo systemctl enable --now forgejo

# 6. nginx vhost (reverse proxy 127.0.0.1:3000, client_max_body_size 1024m for LFS)
# bump server_names_hash_bucket_size to 128 if nginx complains about hash overflow

# 7. TLS via certbot
sudo certbot --nginx -d git.united-humanity.us \
    --non-interactive --agree-tos --email <admin-email> --redirect

# 8. Browser: visit https://git.united-humanity.us
# Complete the web installer:
#   - leave database / paths at the pre-configured defaults
#   - untick OpenID sign-in
#   - fill in admin username, email, password
#   - click Install Forgejo

# 9. Log in, create the empty `humanity` repo (don't initialize)

# 10. From a developer machine with the GitHub repo cloned:
git remote add forge https://git.united-humanity.us/<user>/humanity.git
git push forge --all
git push forge --tags
```

## Future work tracked separately

- Wire CI on Forgejo (Forgejo Actions or external Woodpecker) so the build
  doesn't depend on GitHub Actions exclusively.
- ForgeFed — when Forgejo's federation protocol implementation ships, federate
  with Codeberg + other community Forgejo instances. Step beyond mere
  mirroring into actual federated source distribution.
- Mirror-mode pull from `Shaostoul/Humanity` on GitHub as a backstop, in case
  a `just ship` push to forge fails silently and isn't noticed.
- SSH push key setup for Linux/macOS contributors who don't get GCM's free
  browser SSO (mostly cosmetic — PAT works fine).
