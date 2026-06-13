# HumanityOS: Bus Factor

> **The question this file answers:** if Shaostoul (operator) AND Claude (AI collaborator) both vanished for 6 months, what would the next person need to take this over?
>
> This is intentionally human-readable. The git history is the technical truth; this is the human-context wrapper around it.
>
> **Update rule:** quarterly, or on any personnel/infrastructure change.

## Project identity

- **Mission**: end poverty, unite humanity in peaceful harmony. Account for ALL humans and ALL AI (cloud + local). Permanent infrastructure for civilization, not a startup.
- **License**: open source. See repo root.
- **Started**: January 2019 as "Project Universe", renamed to HumanityOS January 2026.
- **Founder + sole maintainer (2026)**: Shaostoul.
- **AI collaborator**: Claude (Anthropic). Multiple sessions per week; outputs in repo as commits with `Co-Authored-By: Claude Opus 4.7` trailer.

## Source of truth

- **Repository**: https://github.com/Shaostoul/Humanity (canonical, deploy source)
- **Mirror**: https://git.united-humanity.us/shaostoul/humanity (Forgejo, self-hosted on VPS)
- **Web**: https://united-humanity.us (chat, marketing, docs)
- **Releases**: https://github.com/Shaostoul/Humanity/releases + https://united-humanity.us/releases/ + BitTorrent (50 swarms)

## Infrastructure

### Primary VPS (`humanity-vps` SSH alias)
- **Host**: server1.shaostoul.com (Debian Linux, AMD64)
- **Resources**: see `docs/admin/distribution-mirrors.md` + `docs/admin/torrent-infrastructure.md`
- **Services**:
  - `humanity-relay.service` (systemd), the Rust relay running `HumanityOS --headless`
  - `nginx`, reverse proxy, TLS termination (Let's Encrypt via certbot)
  - `forgejo`, self-hosted git mirror (port 3000, behind nginx)
  - `transmission-daemon`, BitTorrent seeder for release binaries + data
  - `humanity-disk-guard.timer`, periodic disk-pressure cleanup (rotates `backups/`)
- **Key paths**:
  - `/opt/Humanity/`, repo working tree (CI git-resets here on every deploy)
  - `/opt/Humanity/data/relay.db`, SQLite (the live state)
  - `/opt/Humanity/data/uploads/`, user-uploaded images/files
  - `/opt/Humanity/backups/`, automated DB snapshots (pq-wipe.sh writes here)
  - `/opt/Humanity/.env`, env vars (API_SECRET, WEBHOOK_SECRET, VAPID, etc.)
  - `/var/www/humanity/`, nginx-served web root (rsynced from `web/` on deploy)
  - `/etc/systemd/system/humanity-*.{service,timer}`, systemd units

### Domains + DNS
- `united-humanity.us`, primary
- `chat.united-humanity.us`, subdomain for the chat client (same VPS)
- `git.united-humanity.us`, Forgejo mirror (same VPS)
- DNS provider: see operator records (NOT in repo)

### Secrets: locations, not values
Secrets live in `/opt/Humanity/.env` on the VPS, mode 600, owned by the relay user. They are **never** committed.

| Key | Purpose | Rotation procedure |
|---|---|---|
| `API_SECRET` | Bot authentication for `bot_*` keys + REST `Authorization: Bearer` | `openssl rand -hex 32`, edit `.env`, `systemctl restart humanity-relay`. Then update any bot configs that hold the old value. |
| `WEBHOOK_SECRET` | GitHub webhook HMAC verification | `openssl rand -hex 32`, edit `.env`, **also** update the webhook secret in GitHub repo settings. |
| `VAPID_PUBLIC_KEY` + `VAPID_PRIVATE_KEY` | Web Push notifications (P-256/ES256). | `node -e "const k=require('web-push').generateVAPIDKeys(); console.log(k)"`. New keys invalidate existing subscriptions; users must re-allow notifications. |
| `SERVER_NAME` | Display name on federation hello | Edit `.env`, restart. Cosmetic. |

### Third-party accounts (operator-controlled; not in repo)
- **GitHub**: `Shaostoul` org. Required for releases, CI deploy SSH, GH_TOKEN env on dev machines.
- **GitHub Actions**: configured in `.github/workflows/`. Deploys to VPS via SSH keys stored as GitHub Secrets.
- **Patreon**: $20/mo Patreon account, low-priority income.
- **Solana wallet**: receives donations. Address in `data/donations/addresses.json` or similar (not embedded here; operator maintains).
- **Cloud LLM / Claude**: Anthropic API key on operator's dev machine. Cost is a real constraint (see `feedback_financial_context.md` in MEMORY).

### CI / deploy pipeline
1. `git push origin main` → GitHub Actions
2. `build-desktop.yml`: builds Windows/Linux/macOS binaries (Rust + tar.gz + raw exe), uploads to release assets
3. `deploy-to-vps.yml`: SSHes to humanity-vps, `git fetch + reset --hard origin/main`, `cargo build --release --features relay --no-default-features`, rsyncs `web/`, restarts `humanity-relay.service`, optionally posts to `#announcements` via Deploy Bot.
4. `mirror-to-vps.yml` (consolidated Linux job): downloads built binaries, scps to `/var/www/humanity/releases/`, runs `regen-releases-manifest` to seed new torrents + update `manifest.json`.

If CI is wedged: `just sync` from local force-pulls + rebuilds on VPS. See `Justfile`.

If GitHub vanishes: deploy continues from Forgejo (`git push forge main` already happens). The release-asset mirror at `https://united-humanity.us/releases/` is independent of GitHub.

## Architectural decisions worth knowing

The full reasoning lives in `data/coordination/orchestrator_state.json` `recent_decisions`. The high-leverage ones a successor should know:

1. **Single Rust crate, feature flags.** No workspace. `native`, `relay`, `wasm` features control what's built. Pre-v0.90 had `server/`, `native/`, `crates/`, gone, do not recreate.
2. **Identity = Dilithium3 (ML-DSA-65) post-quantum.** Ed25519 is now ONLY the BIP39 seed source + Solana wallet derivation. DMs = pure Kyber768. See CLAUDE.md "Cryptography" section.
3. **No home server.** Profiles are signed objects gossiped between federated servers. Any server can cache; latest timestamp wins.
4. **Data-driven everything.** Anything that can exist more than once is a data file (CSV/TOML/RON/JSON in `data/`), not Rust code. See `docs/design/infinite-of-x.md`.
5. **One theme source.** `data/gui/theme.ron` is canonical. Native reads directly; web's `theme.css` is regenerated. Don't hand-edit `theme.css`.
6. **Game data hot-reloads.** Edit `data/*` while the app runs; changes apply via `notify` file watcher.
7. **The /dm slash command was removed** (v0.279.0). Server can't E2EE on the user's behalf. DM UI handles encryption.

## Who can take over

### Operator (the person running the show)
Today: Shaostoul, sole operator. Family is in the loop and could continue commercial / community side but not the dev/ops side.

To take over the operator role you need:
- SSH access to `humanity-vps` (root + a non-root admin user)
- GitHub `Shaostoul` org owner permissions (or fork + reroute DNS)
- DNS provider access for `united-humanity.us`
- Patreon / Solana wallet access (if continuing fundraising)
- Read access to operator's password manager for the secrets above

### Code contributor (a Rust dev who's never seen this before)
Onboarding path:
1. Read `CLAUDE.md` (auto-loaded; everything load-bearing is there or linked from there)
2. Read `docs/PRIORITIES.md` (this is `next`)
3. Read `docs/SOP.md` for the version-bump / deploy ritual
4. Read `docs/design/ui-system.md` + `docs/design/infinite-of-x.md` before touching UI or data
5. `just --list` for the dev commands
6. `cargo test --features native` + `cargo test --features relay --no-default-features` should both pass on a fresh checkout
7. First task: pick from TIER 1 or TIER 3 of `docs/PRIORITIES.md`; nothing in TIER 0 is contributor-safe (operator-attended).

### AI session (Claude or other AI continuing this work)
The orchestrator_state.json is the running journal. `node scripts/agent-status.js` aggregates per-scope status. CLAUDE.md is the rulebook. Trust those plus the test suite, don't trust AI memory from prior sessions; everything load-bearing is checked into the repo.

## Financial floor

Operator's income cap is ~$1k/mo (see MEMORY `project_financial_context.md`). Claude API cost is a real constraint. If finances fail:
- The VPS bill (modest; ~$15-25/mo) is the only hard recurring cost.
- Domain renewals (~$15/yr).
- Letting either lapse = network goes dark.
- BitTorrent seeders + Forgejo mirror would survive a temporary VPS outage but only if seeds exist on other devices.

## "If I disappeared tomorrow" checklist for the operator to fill in

- [ ] Designate a backup operator (person or trust) with SSH + DNS + repo-owner access.
- [ ] Document the secrets-vault location (1Password, Bitwarden, paper-in-safe) where the backup operator can find creds.
- [ ] Pre-pay 12 months of VPS + domain.
- [ ] Write a "first 30 days as the new operator" letter explaining the project goals.
- [ ] Designate a community announcement channel for transition.

This list is intentionally NOT filled in. The operator should fill it as a real exercise; a populated version sits encrypted somewhere the backup operator can reach.

## Update log
- 2026-05-20, initial creation (v0.283.x); Shaostoul + Claude as the only humans/AIs in the loop.
