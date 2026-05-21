# HumanityOS — Incident Playbook

> **Read this when something is on fire.** Each entry is a recipe: symptom → diagnosis → fix → root-cause notes.
>
> **Append rule:** any time we live through a real incident, write it up here while it's fresh. Patterns recur; future-us will thank present-us.

## Section 1 — Live failure recipes

### Relay is unreachable (curl /health → connection refused or timeout)

**Diagnosis:**
```bash
ssh humanity-vps "systemctl is-active humanity-relay; systemctl status humanity-relay --no-pager -n 20"
```
- `inactive (dead)` or `failed`: the relay crashed. Read the last journal lines for the panic.
- `activating (auto-restart)`: it's crash-looping (systemd keeps trying). Same — look at journal.
- `active (running)` but unreachable: nginx is the problem, not the relay.

**Common causes + fixes:**
1. **Schema panic on startup** (v0.262.2 incident, 2026-05-17): `messages` or `roles` table missing a column the seed INSERT names. Caused by a migration that added cols to CREATE TABLE without idempotent ALTER. Fix: write the ALTER, ensure it runs BEFORE the seed loop. Test with the rewind-schema regression pattern.
2. **Disk-guard nuked target/** mid-build (2026-05-19 incident): if the relay binary is gone, systemd reports "failed to start". Fix: `cd /opt/Humanity && cargo build --release --features relay --no-default-features`. v0.275.0 hardened `pq-wipe.sh` against this race; investigate if it recurs.
3. **API_SECRET missing** (after a `.env` edit gone wrong): relay starts but bot endpoints 401 everything. Confirm `grep API_SECRET /opt/Humanity/.env` returns a value. Restart after fixing.

**Recovery commands:**
```bash
ssh humanity-vps "systemctl restart humanity-relay && sleep 3 && systemctl is-active humanity-relay && curl -s http://localhost:3210/health"
```

### nginx is the problem (relay healthy, public URL doesn't work)
```bash
ssh humanity-vps "nginx -t; systemctl status nginx --no-pager -n 10"
```
- `nginx -t` config errors: fix the config in `/etc/nginx/sites-enabled/humanity`, `nginx -s reload`.
- `failed`: `journalctl -u nginx --no-pager -n 30`.

### TLS cert expired / about to expire
```bash
ssh humanity-vps "certbot certificates"
```
Expired: `certbot renew`. If renew fails: read the certbot log + fix the underlying issue (DNS, port 80 accessibility, rate limit).

### Disk full
```bash
ssh humanity-vps "df -h / /opt"
```
If `/opt` is near 100%:
- `/opt/Humanity/backups/` — disk-guard rotates these but check anyway. Manual: `ls -lh /opt/Humanity/backups/ | head -20`; `rm` anything older than ~30 days.
- `/opt/Humanity/data/uploads/` — user uploads. Quotas enforced (Roles v0.262) but check.
- `/opt/Humanity/target/` — Rust build artifacts. Can be removed safely if disk-pressured; CI rebuilds.

### SQLite corruption / WAL torn
**Symptom**: relay panics with "database disk image is malformed" or similar.

**Recovery, in order**:
1. **Stop the relay** to prevent further writes: `systemctl stop humanity-relay`.
2. **Back up the corrupt file**: `cp /opt/Humanity/data/relay.db /opt/Humanity/backups/relay-CORRUPT-$(date +%s).db`.
3. **Try sqlite3 .recover**: `sqlite3 /opt/Humanity/data/relay.db ".recover" | sqlite3 /opt/Humanity/data/relay-recovered.db`. If output is sensible: `mv` it into place and restart.
4. **Fall back to backup**: `ls /opt/Humanity/backups/ | sort -r | head -5` — pick the most recent good one. `cp` to `/opt/Humanity/data/relay.db`, restart.
5. **Inc6-style fresh slate**: `bash /opt/Humanity/scripts/pq-wipe.sh --yes`. Last resort; users re-onboard from seed.

> **AUTOMATED since v0.286.0:** the relay now boots via `Storage::open_resilient` — on a failed integrity check it auto-restores the newest *healthy* `backups/relay-*.db` (quarantining the corrupt file to `relay.db.corrupt-<ts>`), and if NO healthy backup exists it refuses to start (loud failure, watchdog alerts) rather than silently wiping. The manual steps above are the fallback if the automated recovery itself can't find a good backup. The relay watchdog (`humanity-relay-watchdog.timer`, every 2 min) catches the refuse-to-start case and logs CRITICAL.

### Stolen seed phrase (user reports)
The user's identity is the seed. Anyone with the seed is them. There is no central revocation today (key_rotation was removed in Inc5b).

**Communicate to the user**:
1. **Stop using the compromised identity for anything sensitive.** Treat it as burned.
2. **Generate a new identity** (Settings → Identity → Generate New Identity).
3. **Tell trusted contacts the new pubkey + name** via an out-of-band channel.
4. **The OLD identity will keep working for the attacker until the user's contacts learn the new one.** That's the consequence of seed-based identity with no rotation.

> TODO: TIER 1+? — design a soft "I'm switching to this new identity" attestation that contacts can pin. Different from cryptographic rotation but covers the practical case.

### Leaked API_SECRET
```bash
ssh humanity-vps "
  NEW=$(openssl rand -hex 32);
  sed -i \"s/^API_SECRET=.*/API_SECRET=$NEW/\" /opt/Humanity/.env;
  systemctl restart humanity-relay;
  echo NEW SECRET: $NEW
"
```
Then **update every bot config** that holds the old value (Deploy Bot in GitHub Actions secrets, any local bot dev configs). Bots will fail to identify until they get the new secret.

### DDoS in progress
```bash
ssh humanity-vps "tail -50 /var/log/nginx/access.log | awk '{print $1}' | sort | uniq -c | sort -rn | head"
```
If a small set of IPs dominate:
```bash
ssh humanity-vps "iptables -I INPUT -s <BAD_IP> -j DROP"
```
For distributed: tighten nginx rate limit:
```nginx
limit_req_zone $binary_remote_addr zone=ws:10m rate=5r/s;
location /ws { limit_req zone=ws burst=10 nodelay; ... }
```
Reload nginx. **Longer term**: TIER 1 #1, switch to Cloudflare proxy.

### Relay rejecting legit users with "Too many connection attempts" or "Welcome! New accounts wait Ns..."
The v0.279.0 + v0.280.0 anti-spam gates are firing. Check:
- `journalctl -u humanity-relay -n 100 | grep -E "rate limit|cap hit"` to see what's triggering.
- If a household / hackathon is legitimately onboarding fast, **temporarily** raise the per-IP caps in `src/relay/relay.rs` (`IDENTIFY_RATE_MAX`, `NEW_ID_MAX_PER_IP`) and redeploy. Document the elevated threshold; revert after.

### CI broken (deploy didn't fire after a push)
```bash
gh run list --limit 5 --workflow=deploy-to-vps.yml
```
- All green but VPS is on an older commit: VPS deploy SSH key was rotated / removed. Re-add via GitHub Secrets.
- `failure`: read the run logs (`gh run view <id> --log`).
- Workaround: `just sync` from a dev box force-pulls + rebuilds on VPS independently of CI.

## Section 2 — Past incidents (root-cause + lesson, append-only)

### 2026-05-21 — release-mirror bloat → disk-guard nuked target/ → relay crash-loop → discovered GLIBC mismatch in pre-built binaries

**What happened**: `/var/www/humanity/releases/` had accumulated 287 versioned release dirs since v0.122.0 (April 30) with no retention policy. Each dir is ~345 MB (Linux + macOS x64 + macOS arm64 + Windows binaries × 2 — raw + tar.gz — plus torrents and data archives). Total: 91 GB. At 00:01:55Z on 2026-05-21 the disk-guard timer fired (`disk 92% >= 88% — removing build cache /opt/Humanity/target`), correctly identified `target/` as a reclaimable build cache, but no subsequent deploy was triggered so the relay binary at `/opt/Humanity/target/release/HumanityOS` was simply gone. systemd crash-looped the service for ~25 minutes before discovery.

Compound lesson: while diagnosing, I attempted to recover by copying the pre-built Linux binary from the release mirror (`cp /var/www/humanity/releases/v0.283.1/HumanityOS-linux-x64 /opt/Humanity/target/release/HumanityOS`). The binary copied fine but failed to start with `libssl.so.3: cannot open shared object file` AND `GLIBC_2.32/2.33/2.34/2.35 not found`. **The pre-built binaries in the release mirror were built on the GitHub Actions Ubuntu runner (modern GLIBC 2.35+, OpenSSL 3.x) and CANNOT run on this VPS (Debian 11 / bullseye, GLIBC 2.31, OpenSSL 1.1).** Every previous CI deploy that "worked" actually rebuilt on the VPS via the `cargo build --release` step, never relying on the pre-built artifact. The release mirror's binaries serve END USERS (whose distros usually have newer libraries), NOT the relay's own boot.

**Fix (operational)**: 
1. Stopped disk-guard timer to avoid mid-recovery race.
2. Deleted 277 older release dirs (`v0.122.0` through `v0.275.1`), keeping the last 10. Disk went 91% → 13%.
3. Regenerated `manifest.json` via `/usr/local/bin/regen-releases-manifest`.
4. Re-enabled disk-guard timer.
5. Pushed a commit to trigger CI deploy → CI SSHed VPS and ran `cargo build --release --features relay --no-default-features`, restarted the service.

**Fix (preventive)**: 
1. Added TIER 0 hardening item to `docs/PRIORITIES.md`: extend disk-guard (or add a parallel `releases-rotator.timer`) to enforce retention on `/var/www/humanity/releases/` automatically. The disk-guard's current scope is `/opt/Humanity/backups/` and `/opt/Humanity/target/`, but the actual bloat lives elsewhere.
2. Documented the pre-built-binary GLIBC mismatch here so future recovery attempts skip the cp-from-mirror path and go straight to "trigger CI deploy" or "rebuild on VPS."

**Lessons**:
- Disk-guard's threshold trips on disk usage caused by something it doesn't manage. Cleanup scope and trigger scope must match: if disk-guard reclaims `target/`, it must ALSO be empowered to reclaim the real bloat (releases) OR a different mechanism must run with sufficient frequency.
- Pre-built CI artifacts are NOT portable to the VPS. They serve users; the relay's own binary always rebuilds locally. Don't conflate the two.
- Disk pressure can cascade. The release mirror's unrotated growth caused the disk-guard fire that caused the target/ wipe that caused the relay crash. Single-system disk hygiene needs an explicit owner per directory tree.


**What happened**: an attended `pq-wipe.sh` ran while a `cargo build --features relay` was in progress. The disk-guard had rm'd `target/` (legitimately, disk was pressured); the build was rebuilding; the wipe stopped systemd, started it, found no binary (build still running), the empty `relay.db` got created without the schema the new code expected, the offline seed step then hit 888 "no such table" sqlite errors, the relay crash-looped.

**Fix**: v0.275.0 hardened `pq-wipe.sh` — refuses to run if a `cargo build --features relay` process is alive (pgrep), refuses if the relay binary is missing, polls 30s for schema readiness with diagnostics on failure, verifies seeded message count matches the archive length.

**Lesson**: any "stop service, mutate state, start service" recipe needs explicit gates against concurrent state-mutating processes. Don't trust "it never happens"; CI + disk-guard + a manual wipe combined to make it happen exactly once and we lost an hour.

### 2026-05-17 — v0.262.2 schema-ordering panic, total outage
**What happened**: a migration added `can_image_share` + `can_file_share` columns to the `roles` table via `CREATE TABLE IF NOT EXISTS` AND a seed `INSERT` that referenced those columns. On fresh DBs (every test environment): `CREATE` created the cols → seed worked → fresh-DB tests all passed → shipped. On the live VPS: `CREATE TABLE IF NOT EXISTS` was a no-op against the existing 10-column table → seed referenced missing columns → `Storage::open().expect()` panicked → systemd crash-loop → nginx 502 → total outage.

**Fix**: v0.262.2 hotfix moved the guarded ALTER blocks (v0.261 image/file + v0.262 R4 + backfill) to run BEFORE the seed loop. Added regression test `upgrade_from_pre_v0261_roles_schema_does_not_panic` that rewinds the roles table to the pre-v0.261 shape and reopens Storage — panics pre-fix, clean post-fix.

**Lesson**: any migration that adds columns to a CREATE TABLE MUST also add them via idempotent ALTER, and the ALTERs MUST precede any seed/INSERT naming them. ALWAYS add an upgrade-path test (rewind schema + reopen). Fresh-DB tests are necessary but insufficient.

### 2026-04-27 — BUG-034 updater corrupted Windows binary
**What happened**: the in-app updater downloaded the only release asset (a tar.gz bundle) and renamed it to `.exe`. Result: a corrupted exe with gzip magic bytes that Windows reported as "Unsupported 16-Bit Application".

**Fix**: v0.124.0 made `build-desktop.yml` publish raw `HumanityOS-windows-x64.exe` alongside the tar.gz bundles. `src/updater.rs::find_platform_asset` now prefers raw and refuses archive-only releases ("No binary for this platform") instead of writing the gzip bytes as a corrupt exe.

**Lesson**: if your updater downloads an asset, validate the format before writing it to disk. The fix is forward-only: pre-v0.124.0 releases only have .tar.gz, and the updater correctly refuses them. Don't add a tar.gz fallback — refusing is the right behaviour.

### 2026-05-15 — name-only-kick admin protection gap
**What happened**: `handle_mod_action` only checked the "non-admin cannot act on an admin" gate when an explicit target KEY was supplied. The name-only path (target empty, target_name set) skipped the check entirely. A rogue mod could kick/ban an admin by name.

**Fix**: v0.247.0 resolved `candidate_keys = [target key] + keys_for_name(target_name)` and ran the admin/owner protection against the whole set. Also extended the self-action guard to cover name-only self-target + mute (footgun: a mod muting themselves had no easy self-recovery).

**Lesson**: when a feature has two code paths reaching the same outcome (here: key-targeted vs name-targeted moderation), audit BOTH paths against every invariant. Trust assumptions like "only admins use the UI" are not controls.

## Section 3 — Anticipated failures (haven't happened yet but probably will)

### nginx maps to wrong upstream after a restart
Mostly happens after an OS update reorders systemd unit start times. Symptom: nginx returns 502 for a few seconds then recovers. If persistent: `systemctl restart nginx`.

### A federated peer turns hostile
The trust-tier system gates inbound federation. If a peer goes bad:
1. Identify them in `federated_servers` table (`sqlite3 /opt/Humanity/data/relay.db "SELECT server_id, name, url, trust_tier FROM federated_servers"`).
2. Demote: `UPDATE federated_servers SET trust_tier = 0 WHERE server_id = '...';` (trust_tier < 2 disconnects them; see `federation.rs::start_federation_connections`).
3. Restart relay to drop active connection.

### Federation profile gossip flood
A misconfigured or hostile peer floods profile updates. We don't currently rate-limit profile gossip per-server. If symptoms (DB write storm, log noise): demote the peer per above, then add per-server profile-gossip rate limit (parallel to the federation chat rate limit already in `state.federation_rate`).

### Multi-device same-identity sync conflict
A user logs in from web + native simultaneously, edits profile from both, clocks are off by minutes. Last-timestamp-wins per `signed_profiles`. If a user reports "I edited my bio and it reverted", suspect clock skew. Mitigation: warn users in Settings → Profile about multi-device editing.

## Section 4 — Common "is this normal?" anchors

- **`/health` returns `{"status":"ok"}` with HTTP 200** — relay is alive.
- **Connection count via `/api/stats`** — should be > 0 during normal hours; near 0 at 3am is fine.
- **`journalctl -u humanity-relay -n 100`** — look for `ERROR` or `WARN` lines. Routine `INFO` chatter (peer joined / left, federation hellos) is normal.
- **Disk usage trend on `/opt`** — uploads + DB grow over time; 10GB free is comfortable for the operator's current scale.
- **`backups/` count** — disk-guard keeps the most recent 5 by default. More than that means the rotator isn't firing.

## Update log
- 2026-05-20 — initial creation; populated with the four real incidents from the last 30 days + the anticipated failure list seeded from the audit work.
