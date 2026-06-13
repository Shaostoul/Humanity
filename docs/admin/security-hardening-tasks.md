# Operator security-hardening tasks

These are the security items from the 2026-06-12 audit that the code cannot do for
itself because they live in GitHub settings or the VPS global nginx config. Each is
low-to-medium priority (the high and critical findings are already fixed in code: see
the release notes for v0.417 to v0.426 and the cryptography/audit notes in CLAUDE.md).
Do them when convenient; none blocks day-to-day use.

---

## 1. Rate-limit `/api/send` at the nginx edge (low priority) — DONE 2026-06-13

**Applied live and verified.** The `send_limit` zone was added to
`/etc/nginx/conf.d/rate-limits.conf` and a `location = /api/send` block to both server
blocks in `/etc/nginx/sites-enabled/humanity`; `nginx -t` passed and nginx was reloaded.
Verified: health 200, `/api/send` still proxies (415 without a JSON content-type, the
relay's normal response), and a 10-request burst returned 6 OK + 4 rate-limited (503,
nginx `limit_req`'s default over-limit status). Live config backups:
`*.bak-sendlimit`. NOTE: the VPS site config drifted from the repo's
`scripts/nginx/humanity.conf` during the 2026-06-12 audit, so the VPS config is the
source of truth; the exact change applied is recorded below.

Applied zone (`conf.d/rate-limits.conf`):
```nginx
limit_req_zone $binary_remote_addr zone=send_limit:10m rate=30r/m;
```
Applied location (before each `location /api/` in `sites-enabled/humanity`):
```nginx
location = /api/send {
    limit_req zone=send_limit burst=5 nodelay;
    proxy_pass http://127.0.0.1:3210;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
}
```

<details><summary>Original how-to (kept for reference)</summary>

**Why:** `POST /api/send` is already hard-gated by the `API_SECRET` bearer token, so
there is no untrusted caller to throttle. The only real benefit is dropping a flood
(including 401-spam from someone without the secret) at the edge, before it reaches the
relay. A relay-side per-IP cap was considered and deliberately skipped: it would only
ever throttle already-authenticated (trusted) callers and, if done wrong, becomes its
own memory-amplification vector. The edge limit is the right tool.

**This is a PAIRED change. Do the steps in this order or `nginx -t` will fail** (the
`location` block references a zone that must already exist):

1. On the VPS, in the **global** nginx `http { }` block (the same place
   `api_read_limit` and `upload_limit` are defined, typically `/etc/nginx/nginx.conf`),
   add:
   ```nginx
   limit_req_zone $binary_remote_addr zone=send_limit:10m rate=30r/m;
   ```
   `$binary_remote_addr` is the real client IP because nginx terminates TLS at the edge.
   `30r/m` is comfortable for a human-cadence bot; raise it if your CI deploy bot bursts.

2. In `scripts/nginx/humanity.conf`, add a `location = /api/send` block **before** the
   generic `location /api/` block, mirroring the existing upload block:
   ```nginx
   location = /api/send {
       limit_req zone=send_limit burst=5 nodelay;
       proxy_pass http://127.0.0.1:3210;
       proxy_set_header Host $host;
       proxy_set_header X-Real-IP $remote_addr;
       proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
       proxy_set_header X-Forwarded-Proto $scheme;
   }
   ```
   Add it to each `server` block where `/api/send` is reachable.

3. Deploy the conf (or copy it to the VPS), then `sudo nginx -t && sudo systemctl reload nginx`.

**Caution:** if step 2 lands without step 1, `nginx -t` fails on the undefined zone and a
reload can take the site down. Always do the zone first. Pick a generous rate so you do
not throttle your own CI deploy announcements.

</details>

---

## 2. GitHub branch + tag protection (medium priority)

**Why:** `deploy.yml` auto-deploys to the VPS on any push to `main`, and
`build-desktop.yml` auto-builds/releases on any `v*` tag, with no approval gate. Release
signing already closed the desktop-RCE half (v0.421+ clients reject unsigned binaries,
and CI cannot sign because the key is operator-local). The residual is that a compromised
GitHub credential could push malicious code to `main` (which auto-deploys to the relay)
or cut a tag. Branch protection closes that.

In **GitHub → Settings → Branches → Add branch protection rule** for `main`:
- Require a pull request before merging (even working solo, this stops a stolen token
  from pushing straight to `main`).
- Require signed commits.
- Include administrators (do not allow bypass).
- Require status checks to pass before merging.

Optional, stronger: gate the VPS deploy behind a manual approval.
- Add `environment: production` to the deploy job in `.github/workflows/deploy.yml`.
- In **Settings → Environments → `production`**, add yourself as a required reviewer.
- Then every VPS deploy waits for your click. (Land the YAML change and the environment
  setup together, or deploys silently change behavior.)

---

## 3. Sign every release (ongoing duty, already active)

Release signing is ACTIVE as of v0.421.0. After a tag's Build-Desktop workflow uploads
the platform binaries, run:
```bash
export HUMANITY_SIGNING_PASSPHRASE='...'
just sign-release vX.Y.Z
```
An unsigned release is invisible to v0.421+ auto-update (fail-safe, not an error). Full
procedure: [release-signing.md](release-signing.md). Only you can do this (it needs the
passphrase + `release-signing-key.enc`); CI and AI cannot.
