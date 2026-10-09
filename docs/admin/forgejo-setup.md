# Forgejo Self-Host on the VPS

> **Status 2026-09-29: LIVE again, as a PULL mirror of GitHub.** Forgejo
> v16.0.5 fetches `github.com/Shaostoul/Humanity` every 8 hours by itself
> (all branches and tags; LFS on, for when the repo uses it), so it never
> depends on anyone's push. `just brief`'s MIRROR row reads its last sync from
> its own API and flags it when it is late or unreachable.
>
> History: live from v0.127.0 (2026-04-29) as a push mirror. It was lost
> when the VPS was rebuilt on Debian 12 in August 2026 (last push landed
> 2026-08-03), and `just ship` pushed to it with errors ignored, so nothing
> said so for eight weeks. Found and reinstalled on 2026-09-29; the push was
> taken out of `just ship` in the same change.
> Step 1 of the [distribution-mirrors](distribution-mirrors.md) plan.

This is the sovereignty layer for HumanityOS source code. GitHub stays the
discovery layer (download links, CI builds, the public face); Forgejo is the
copy you control, and it keeps itself current. If GitHub ever went away, turn
the mirror into an ordinary repository (its Settings, "Convert to regular
repository") and push to it directly.

## What's running

| Component | Where | Notes |
|-----------|-------|-------|
| Forgejo binary | `/usr/local/bin/forgejo` | v16.0.5 (2026-09-29, checksum verified against Codeberg's .sha256), single Go binary |
| Config | `/etc/forgejo/app.ini` | owned `forgejo:forgejo`, mode `640` |
| Data | `/var/lib/forgejo/data/` | SQLite DB at `forgejo.db`, repos at `forgejo-repositories/`, LFS at `lfs/` |
| Logs | `/var/lib/forgejo/log/` | one log per service component |
| systemd unit | `/etc/systemd/system/forgejo.service` | `User=forgejo`, hardening flags |
| nginx vhost | `/etc/nginx/sites-available/git.united-humanity.us` | reverse proxy `127.0.0.1:3000`, `client_max_body_size 1024m` for LFS |
| TLS | `/etc/letsencrypt/live/git.united-humanity.us/` | Let's Encrypt by **webroot** (`/var/www/letsencrypt`, served by the port-80 block), so it renews with nginx running; the main site's certificate uses standalone |
| Git | `/usr/bin/git` | Debian 12's 2.39.5, new enough (Forgejo needs 2.34.1 or later; Debian 11's 2.30.2 was not, which is why the first install built 2.45.2 from source) |

## What's the public surface

- **Web UI**: https://git.united-humanity.us
- **HTTPS clone**: `https://git.united-humanity.us/shaostoul/humanity.git`
- **SSH clone**: `forgejo@git.united-humanity.us:shaostoul/humanity.git` (system sshd on port 22; Forgejo's `RUN_USER=forgejo` so SSH user is `forgejo`, NOT `git`, there is no `git` system user on the VPS)
- **Self-registration**: disabled. Only the admin (`shaostoul`) can create accounts.
- **OpenID sign-in**: disabled.
- **API**: anonymous read access on public repos via `https://git.united-humanity.us/api/v1/...`

## Nothing pushes to it

Since 2026-09-29 the mirror PULLS from GitHub every 8 hours, so `just ship`
pushes only to `origin`. A pull mirror refuses pushes, so the SSH-key and
credential-manager setup this section used to describe no longer applies. The
local `forge` remote, if a clone has one, is
`https://git.united-humanity.us/shaostoul/Humanity.git`, fetch-only. To make it
take pushes again (only if GitHub is gone), convert it to a regular repository
first (see the top of this page).

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


## Archive downloads and crawlers (incident 2026-10-05, BUG-169)

Every commit page offers its source as a `.zip`, `.tar.gz` or `.bundle`.
Forgejo builds the file on disk the first time someone asks, in
`/var/lib/forgejo/data/repo-archive/`, and its `archive_cleanup` cron deletes
archives only once a day (at midnight, those older than 24 hours). On 5 October
a crawler (`ShapBot/0.1.0`) asked for archives of many commits and about 60 GB
piled up in a few hours: the VPS disk sat at 100% for a day, two relay deploys
failed and this mirror stopped syncing (its queue kept the write error after
space came back, until a restart).

What protects the disk now:

- **The disk guard** (`scripts/humanity-disk-guard.sh`) clears
  `repo-archive/` at 88% and logs a `WHERE:` line naming the biggest folders.
  The files are a cache: Forgejo builds an archive again when one is asked for.
- **Settings to apply on the VPS** (not applied yet; `app.ini` and the
  `robots.txt` live only on the server). Clean up hourly instead of daily:
  ```ini
  [cron.archive_cleanup]
  ENABLED = true
  RUN_AT_START = true
  SCHEDULE = @every 1h
  OLDER_THAN = 1h
  ```
  or, to stop offering archives at all (cloning and browsing are unaffected,
  and GitHub still offers them): `[repository] DISABLE_DOWNLOAD_SOURCE_ARCHIVES = true`.
  Then ask crawlers to skip archives and per-commit pages, keeping the current
  code browsable, in `/var/lib/forgejo/custom/public/robots.txt` (Forgejo
  serves it at `/robots.txt`):
  ```
  User-agent: *
  Disallow: /*/*/archive/
  Disallow: /*/*/commit/
  Disallow: /*/*/src/commit/
  Disallow: /*/*/raw/commit/
  Disallow: /*/*/blame/
  Disallow: /*/*/compare/
  ```
  Restart Forgejo after either change.
- **If the mirror stops syncing after a full disk:** `gitea.log` shows
  `MirrorsIterate: ... no space left on device` even with the disk below 90%.
  Restart Forgejo; the queue reopens and the next sync runs within the 8-hour
  interval (or press "Synchronize now" on the repository's settings page as the
  admin).

## Reproducing the install (for future operators)

**The safe order (2026-09-29).** Forgejo's first-run page lets whoever reaches
it first create the admin account, so keep it private until that is done:
`HTTP_ADDR = 127.0.0.1` in app.ini, and the nginx site answering 503 for
everything except the certificate challenge. The operator finishes the web
installer through an SSH tunnel (`ssh -L 3000:127.0.0.1:3000 humanity-vps`,
then http://localhost:3000); only then does nginx switch to the proxy block.
After it, `/install` is gone and sign-up says registration is disabled. Then
New Migration, GitHub, `https://github.com/Shaostoul/Humanity`, "This
repository will be a mirror" (and LFS). The older sequence below built git
from source and pushed; neither is needed on Debian 12 with a pull mirror.

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

# 4. app.ini (see /etc/forgejo/app.ini for current contents - pre-configured paths,
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
- ForgeFed, when Forgejo's federation protocol implementation ships, federate
  with Codeberg + other community Forgejo instances. Step beyond mere
  mirroring into actual federated source distribution.
- Mirror-mode pull from `Shaostoul/Humanity` on GitHub as a backstop, in case
  a `just ship` push to forge fails silently and isn't noticed.
- SSH push key setup for Linux/macOS contributors who don't get GCM's free
  browser SSO (mostly cosmetic, PAT works fine).
