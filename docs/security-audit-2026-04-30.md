# Security Audit — 2026-04-30 (v0.130.0 snapshot)

> Three parallel Explore agents reviewed crypto + auth, distribution
> infrastructure, and repo hygiene. The repo's secret-hygiene posture is
> clean. The substantive findings are in the federation, updater, and
> distribution-manifest layers — areas that grew rapidly across
> v0.122.0 → v0.130.0 and weren't all covered by the original v0.122.0
> audit.

This doc records what was found and a recommended fix order. Findings
are grouped by severity. File paths and line ranges are approximate
references; verify before fixing.

---

## BLOCKERS — fix soon

### B1. `FederatedChat` accepts inbound messages with no signature verification

**Where**: `src/relay/handlers/msg_handlers.rs:2149-2171` (`handle_federated_chat`).

**Issue**: The handler accepts a `signature: Option<String>` field from
inbound `FederatedChat` messages but never validates it. It only checks
that the source server's `trust_tier ≥ 2`. Any compromised or malicious
trust-tier-2 federated server can forge chat messages from any user
(any `from_name`, any `sender_key`) and inject them into federated
channels — recipients see them as legitimate.

**Why this matters**: Trust-tier-2 means "verified but not
Accord-adopted." It's the broad middle of the federation. A server
operator who turns malicious can impersonate every user on the
network.

**Fix shape**: Use the v0.122.0 `verify_ed25519_signature` helper
against a canonical `FederatedChat` message format (analogous to
`canonical_profile_message`). Reject when sig is non-empty and bad;
trust-by-source when sig is empty (same gradient as profile gossip
until signed clients exist).

---

### B2. `ProfileGossip` and `FederationHello` have no timestamp freshness window

**Where**:
- `src/relay/relay.rs:4687-4699` (ProfileGossip direct-WS handler)
- `src/relay/handlers/federation.rs` ProfileGossip fed-loop handler
- `src/relay/handlers/msg_handlers.rs:2120-2127` (FederationHello)

**Issue**: Inbound profile/hello messages accept any `timestamp: u64`.
The signed-profiles store accepts when `timestamp > existing.timestamp`,
so an attacker can replay an old (e.g. 5-year-old) profile gossip and
it will be accepted if the current stored profile is older. There is
no absolute "must be within ±N minutes of now" check (compare to vault
sync at api.rs:2209 which enforces a 5-min freshness window).

**Fix shape**: Add `now_ms.saturating_sub(message_ts) > 5*60*1000`
rejection at the inbound handler. Optionally also reject future
timestamps beyond a small clock-skew window.

---

### B3. DM crypto silently downgrades to plaintext when peer ECDH key is missing

**Where**: `src/gui/pages/chat.rs:1735-1758`.

**Issue**: If `state.peer_ecdh_keys.get(partner_key)` returns None or
the encrypt step fails, the code logs `"sending plaintext"` and continues
with the DM unencrypted. The `encrypted` flag is never set true. The
user gets no warning. An attacker who can suppress ECDH key
announcements (or target a peer who never set one) silently strips
encryption from a DM the user thinks is private.

**Fix shape**: On encryption failure, refuse to send and surface a
modal dialog asking the user to confirm "send unencrypted" or wait for
the peer's ECDH key. Default to refuse.

---

### B4. Auto-updater has no signature verification on downloaded binaries

**Where**: `src/updater.rs::download_and_apply` (lines ~415-498).

**Issue**: The updater downloads from GitHub via TLS, checks
`MIN_BINARY_SIZE` (1 MB), and applies. There is no cryptographic
signature verification. If GitHub is compromised — repo takeover,
account compromise, supply-chain attack on `softprops/action-gh-release`
— the updater silently installs whatever GitHub serves. The "auth" is
GitHub's TLS cert. Single point of trust.

**Fix shape**:
1. Generate an offline Ed25519 signing key (operator's responsibility).
2. CI signs each released binary with it (or signs a release manifest).
3. The public key is pinned in the binary itself (compiled in).
4. Updater verifies signature before `apply_update`.

This is a substantial change but unblocks turning down the trust
attached to GitHub — which is the whole point of the sovereignty plan.

---

### B5. `data-manifest-<tag>.json` has no integrity protection

**Where**: `data-manifest-<tag>.json` design (Step 4.5, v0.130.0).

**Issue**: The manifest lists every file's SHA-256 hash and per-file
URL. It's served plain over HTTPS from the VPS. Anyone with VPS write
access (compromised CI, compromised SSH, supply-chain on the deploy
secrets) can swap the manifest for a malicious one with different
hashes pointing at attacker-controlled files. Future client-side delta
sync that consumes this manifest silently installs whatever files the
manifest references.

**Fix shape**: Sign each manifest with the same offline key from B4.
Embed a merkle root over the manifest in the GitHub release notes so
the signature is anchored to immutable upstream record. Verify before
trust.

---

## IMPORTANT — fix in coming sessions

### I1. CI uses root SSH to the VPS

**Where**: `.github/workflows/build-desktop.yml`, `deploy.yml` —
`username: root` + `key: ${{ secrets.VPS_SSH_KEY }}`.

**Issue**: A successful GitHub Actions environment compromise (workflow
injection, dependency attack, runner escape) gets root on the VPS.
That's full game over: torrent poisoning, manifest swap, Forgejo
backdoor, cert theft.

**Fix shape**: Create a `cicd-deploy` user with minimal privileges:
- write only to `/var/www/humanity/releases/`
- execute only `/usr/local/bin/regen-releases-manifest`
- no read on `/root/`, `/etc/forgejo/`
- forced-command `authorized_keys` entries
- key rotation schedule

### I2. CI shell-substitutes `github.ref_name` in scp target without validation

**Where**: `.github/workflows/build-desktop.yml:146` —
`target: "/var/www/humanity/releases/${{ github.ref_name }}/"`.

**Issue**: A tag like `v1.0.0/../../../tmp` would traverse upward.
A malicious git push (compromised account, compromised forge) could
land files outside the releases directory.

**Fix shape**: Validate `github.ref_name` against
`^v\d+\.\d+\.\d+(\.\d+)?$` regex before the scp step. Reject anything
with `..`, `/`, or unusual characters.

### I3. `gen-data-manifest.js` doesn't validate the `version` argument

**Where**: `scripts/gen-data-manifest.js`.

**Issue**: The version arg is interpolated into output filenames and
Forgejo URLs. A non-validated tag with shell metacharacters could
poison the manifest.

**Fix shape**: Same regex validation as I2. Reject before generating.

### I4. Forgejo install lock not explicitly documented as required

**Where**: `docs/forgejo-setup.md`.

**Issue**: After completing the web installer, `app.ini` should have
`INSTALL_LOCK = true`. If missing, the installer remains accessible —
anyone visiting `/install` could re-run setup and seize the instance.
Forgejo writes the lock automatically after the first install
completes, but the doc doesn't say to verify it.

**Fix shape**: Add a "Verify after install" step: confirm
`INSTALL_LOCK = true` is in `app.ini`, document re-locking if missing.

### I5. Forgejo session-signing secrets potentially defaulted

**Where**: `docs/forgejo-setup.md`.

**Issue**: Setup doesn't mention regenerating Forgejo's `SECRET_KEY`
and `INTERNAL_TOKEN` — used for session signing and internal API auth.
If left at defaults (or installer-generated weakly), session hijacking
or internal API forgery becomes possible.

**Fix shape**: Add `forgejo generate secret SECRET_KEY` +
`forgejo generate secret INTERNAL_TOKEN` to install procedure; verify
both rotated.

### I6. Inbound federation gossip has no per-peer rate limit

**Where**: `src/relay/handlers/federation.rs` — federated read loop.

**Issue**: Outbound has 10 msg/sec/server cap; inbound has none. A
malicious federated peer can flood signed-object gossip, each
requiring Dilithium3 verification (CPU-expensive). DoS via gossip flood.

**Fix shape**: Add per-peer Fibonacci/leaky-bucket rate limit on the
inbound federation read path. Reject when exceeded.

### I7. Vault sync accepts future timestamps (lookahead)

**Where**: `src/relay/api.rs:2204-2210`.

**Issue**: The 5-min freshness check rejects timestamps OLDER than
5 min, but not timestamps in the FUTURE. A client can sign with
`timestamp = now + 1h` and reuse the signature for an hour.

**Fix shape**: Symmetric window —
`(now - 5min) < timestamp < (now + 30s)` — small forward tolerance
for clock skew, hard reject beyond.

---

## MINOR — track but not urgent

### M1. `MIN_BINARY_SIZE` (1 MB) is a weak integrity check

`src/updater.rs:432`. Once B4 is fixed (signature verification), this
becomes a sanity check rather than a security control. Keep it.

### M2. GitHub API rate limit on auto-updater

`src/updater.rs:368` — 60 req/hr unauth. Many users checking for
updates simultaneously could DoS the endpoint. Add backoff on 429.

### M3. Manifest JSON not in canonical form

`scripts/gen-data-manifest.js`. If we sign the manifest (B5),
non-canonical JSON formatting could break signature verification.
Use sorted keys + deterministic formatting before signing lands.

### M4. VPS-side scripts not version-controlled in repo

`/usr/local/bin/torrent-create-and-seed`,
`/usr/local/bin/regen-releases-manifest` live only on the VPS. Audit
blind spot — copy them into `scripts/vps/` for inspection and version
tracking.

### M5. TransmissionRPC password file path

`/root/.transmission-rpc-password` — root home is unusual storage.
Cosmetic; consider `/var/lib/transmission/.rpc-password` with mode 600
in future setup.

---

## CLEAN — no findings

- **Hardcoded credentials, API keys, tokens**: none found.
- **Files in git that shouldn't be**: none.
- **Secrets in docs**: none. Real values (passwords, keys) are
  consistently in env vars or generated locally.
- **`.gitignore` coverage**: comprehensive — env files, keys, certs,
  databases, build artifacts, OS junk all covered.
- **Git history**: no leaked secrets in earlier commits, no big binaries
  later removed (which would persist in pack files).

---

## Recommended fix order

### Tier 1 — security gaps that need closing soon (~1-2 sessions)

1. **B3** — DM downgrade refusal + warning UI. Smallest scope, biggest user-trust win.
2. **B1** — FederatedChat signature verification (mirror the v0.122.0 ProfileGossip pattern).
3. **B2** — ProfileGossip + FederationHello timestamp freshness.
4. **I7** — Vault sync future-timestamp rejection.
5. **I6** — Inbound federation gossip rate limit.

### Tier 2 — supply-chain hardening (~1 session)

6. **I2 + I3** — Validate `github.ref_name` in workflow + script.
7. **I4 + I5** — Document Forgejo install lock + secret rotation.
8. **I1** — Replace root SSH with restricted `cicd-deploy` user. (Bigger;
   plan separately because it requires creating + privileging the user
   and rotating the SSH key.)

### Tier 3 — signed releases (multi-session, the big lift)

9. **B4 + B5** — Generate offline Ed25519 signing key, sign release
   binaries + manifest, embed verification key in updater. The hardest
   change but the one that turns down GitHub trust to "discoverability
   layer" instead of "supply-chain root."

### Tier 4 — minor cleanup

10. **M1, M2, M3, M4, M5** — opportunistic.

---

## Notes on scope

- BUG-034 (updater corrupts on tar.gz), BUG-035 (chat reply vanishes),
  BUG-036 (channel resurrection) — already fixed in v0.124.0/v0.125.0.
  Not repeated here.
- The original v0.122.0 audit covered the federation profile-gossip
  signature verification gap — that's fixed. The findings here are
  additional federation surfaces and the post-v0.122 distribution work.
- The repo-hygiene audit was clean. No urgent action.
