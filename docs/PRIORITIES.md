# HumanityOS — Priorities

> **Read this file first if you're picking up work without context.** This is the strict-ranked backlog. The TOP item of TIER 0 is what gets worked on next; everything else waits.
>
> **Update rule:** every session that meaningfully changes scope updates this file before ending. The orchestrator_state.json journal records WHY a decision was made; this file records WHAT comes next. Don't mistake one for the other.

## Active focus
<!-- Set this to the single most important thing right now. Should match the top item in TIER 0. -->
**TIER 0 #1 — re-point or remove the stale GitHub webhook.** Off-site backup is now SOLVED (the device-mesh stopgap: operator's PC pulls the relay DB every 6h via scheduled task — see below + `docs/design/device-mesh.md`). Next smallest blocker is the webhook pointing at a dead ngrok URL.

## TIER 0 — pre-public launch blockers
Items here are mandatory before inviting public users. Operator-attended where noted. **Order matters within the tier.**

1. **Re-point or remove the GitHub webhook.** The configured webhook points to a stale ngrok URL (`pandanaceous-equationally-chia.ngrok-free.dev`) and has been returning 404 for months. Operator decides: (a) re-point to `https://united-humanity.us/api/github-webhook` + generate fresh `WEBHOOK_SECRET` + set on both sides, OR (b) delete the webhook entirely if deploy-bot announcements aren't wanted. The relay's `/api/github-webhook` endpoint currently accepts forgeries (no secret set) but nothing reaches it.

2. **Fix nginx `/health` routing.** Internal `http://localhost:3210/health` returns 200; public `https://united-humanity.us/health` returns 404. nginx isn't routing the path. Trivial nginx config addition (`location = /health { proxy_pass http://127.0.0.1:3210; }`). Matters because off-site monitoring (TIER 1 #2) needs the public endpoint to work.

3. **DONE: off-site backup (stopgap).** 2026-05-20: `scripts/backup-relay-from-vps.ps1` + a Windows scheduled task ("HumanityOS Relay Backup Pull", every 6h) now pull the live relay DB from the VPS to the operator's PC — genuine 3-2-1 backup (live DB / VPS-local 30-min snapshots / off-site PC). This is the "immediate" half of the device-mesh vision (`docs/design/device-mesh.md`); the full in-app version is TIER 2. NOTE: the PC backup is off-site but a SINGLE off-site copy. A second target (phone, NAS, or a cheap second VPS) would make it 3-2-1-with-redundancy. Phase B of the device mesh generalizes this.

4. **DONE: 2026-05-21 release-mirror cleanup + retention automation.** Cleaned 277 old release dirs from `/var/www/humanity/releases/` (freed 91 GB; 91% → 13%). v0.283.4 extends `scripts/humanity-disk-guard.sh` to enforce 10-version retention automatically on every 20-min cycle + regenerate the manifest. Cascade is structurally prevented from recurring.

5. **DONE: backup script repaired + in-repo.** The pre-v0.90.0 path bug was silently backing up an empty fossil DB for over a month. v0.283.4 ships `scripts/humanity-backup-db.sh` as the source of truth, the `deploy.yml` workflow now copies it to `/usr/local/bin/humanity-backup-db` on every deploy. Fossil backups moved to `backups/fossil-pre-v0.90/` for historical interest only.

6. **DONE: Orphan Ed25519 admin rows cleanup.** 2026-05-21: ADMIN_KEYS env updated to Shaostoul's Dilithium hex (3904 chars), 4 orphan rows DELETEd, relay restarted, verified `user_roles` is Dilithium-only.

7. **DONE: Inc6 attended wipe.** Verified 2026-05-20 by direct SQL.

8. **DONE: TLS auto-renew sanity check.** certbot.timer runs on a 12h cycle; last run 2026-05-20 16:42, next 2026-05-21 06:15. All 3 certs valid 50-68 days out. No action needed.

9. **DONE: API_SECRET length audit.** 64 chars (above 32-char threshold). No action needed.

## TIER 1 — hardening before invites scale beyond known group
Items here protect against the realistic adversary (script kiddie, opportunistic abuser, eager fan with sticky fingers). Order within tier is flexible; pick what's cheapest first.

1. **DDoS protection.** Today: nginx rate-limit per IP, the v0.279.0 + v0.280.0 in-app gates. No L7 WAF in front. Options: Cloudflare free tier (proxy + DDoS Pro), or `fail2ban` tuned for nginx access logs. Cloudflare adds dependency on a third party for the chat-tab path; document the trade-off before committing.

2. **Monitoring + alerting.** Today: zero. No alert when the relay dies between deploys. Bare minimum: curl-cron from a second host hitting `https://united-humanity.us/health` every 60s + ntfy.sh push on failure. Better: Prometheus + Grafana via the existing relay endpoints.

3. **SQLite WAL corruption recovery.** What happens if the WAL gets torn (power loss, kernel panic)? Today: unclear; `Storage::open` panics on schema mismatch and probably on WAL corruption too. Add a recovery path: `.recover` mode that detects corruption, copies the DB aside, replays the WAL, falls back to the most recent backup if unrecoverable. Document in INCIDENT-PLAYBOOK.

4. **Federation activation decision.** The federation code is implemented but zero peers are configured. Decision: leave it dormant (and disable the inbound `federation_hello` accept) OR activate it (need to vet trust tiers, federation policy, abuse model). Either way: stop the ambiguous "implemented but untested" middle ground.

5. **Crash-loop autorestart caps + alerts.** systemd will restart `humanity-relay` forever today. If a bug causes immediate crash, the relay flaps without anyone noticing. Set `StartLimitInterval` + `StartLimitBurst` in the unit; pipe failure to a notification.

## TIER 2 — big-feature gaps
Items here are real features the system promises but doesn't deliver on every platform. Weeks of work each.

1. **Native voice.** Channel-list voice icon click is a TODO (chat.rs:1060). No WebRTC stack at all. Needs: `webrtc-rs` integration, audio capture → kira pipeline, playback routing for N peers, mute/deafen UI, connection state machine. Web users have voice; native users are observer-only today.

2. **Native streaming viewer.** Web can view streams. Native has no streaming UI dispatcher arms (`stream_*` events go nowhere). Similar scope to voice but viewer-only (not capture).

3. **Native trade UI completion.** Trade page exists in `src/gui/pages/`. Trade events (`trade_response`, `trade_confirm`, etc.) aren't dispatched. Either wire them up or remove the page until ready.

4. **Litestream / continuous backup.** Beyond the nightly rsync floor in TIER 0, set up real continuous replication. SQLite WAL → S3-compatible blob storage. RPO ~1 minute, RTO ~10 minutes from cold.

5. **Mobile clients.** Android (JNI bridge for keyring + AndroidKeyStore; new keychain backend), iOS (Keychain Services already works via `keyring` crate — needs only an iOS build target). Big effort either way.

6. **Device mesh** (design doc: `docs/design/device-mesh.md`). The operator's vision: your devices back up each other + the relay; review all devices' system-info (hardware, storage, health) from any one device; device roles (battle-station / accessory / relay / archive). Phased: A) system-info reporting + "My Devices" dashboard, B) backup designation + pull + encryption-at-rest (subsumes the shipped PowerShell stopgap), C) restore flow, D) LAN direct-sync + mobile mesh members + remote wipe. The VPS-as-rendezvous architecture (devices report up, read all-devices down) fits the existing federation model. On-mission sovereignty tooling — give it to every user, not just the operator.

## TIER 3 — UX accessibility (the ELI5 mandate)
The platform's mission requires this layer. Not optional, just sequenced after the load-bearing security/feature work.

1. **Tooltip pass on every interactive element.** Every button, every input, every icon: short tooltip explaining what it does in plain language. Audit pages one at a time.

2. **"First 5 minutes" onboarding flow.** New user opens the app — what do they see? Today: a chat with no context. Build a guided tour: identity → seed backup → join your first channel → send your first message → set your status → done. The Onboarding page exists but needs flow polish.

3. **Localization expansion.** 5 languages today (en, es, fr, ja, zh). Add: ar, hi, pt, ru, de, sw at minimum. Existing infrastructure (`data/i18n/`) supports it; the work is translation, not code.

4. **Full accessibility audit.** High-contrast, screen-reader, colorblind, reduced-motion modes already in code (`src/gui/theme.rs` has the tokens). Audit every page against WCAG 2.1 AA. Fix violations. Document the audit in `docs/accessibility-audit.md`.

5. **Glossary integration on every page.** 150+ terms in `data/glossary.json`. Right-click any unfamiliar term → glossary popup. Native widget doesn't exist yet; web has it.

## TIER 4 — long horizon
Don't touch these until TIERs 0-3 are mostly done. Listing them so they're not forgotten.

1. **LoRa mesh hardware integration.** Roadmap item. Requires actual radio hardware on hand.
2. **STARK selective disclosure.** Scaffold exists; circuit design deferred.
3. **Game-world depth.** The simulation/educational gameplay loop. Big. Cosmos Phase 4d shipped; ship-at-origin world exists; voxel asteroids exist. Lots of content + system work left.
4. **AI agent governance.** First-class AI participation is in `docs/ai-onboarding.md`. As more AI participants connect, governance protocols (Article 14 of the Humanity Accord) need to evolve from "documented intent" to "enforced rules with appeals."
5. **Distribution layer beyond GitHub.** Forgejo mirror exists. BitTorrent + IPFS scaffolded. Codeberg + Software Heritage + WinGet manifest still pending per `docs/distribution-mirrors.md`.

## Done — recent (last 30 days, newest at top)
- v0.283.0 — voice signaling no-op stubs + deferred-feature note
- v0.282.0 — typing + message_deleted + federated_chat propagation
- v0.281.0 — admin/mod right-click → Delete any message
- v0.280.0 — anti-spam: new-identity time-gate + per-IP cap
- v0.279.0 — pre-public hardening trio: bot_secret + /dm + identify rate-limit
- v0.278.0 — auto-unlock: 3 modes (always prompt / OS keychain / quick PIN)
- v0.277.0 — native vault PBKDF2 100k → 600k
- v0.276.0 — federation gossip Ed25519 → Dilithium3
- v0.275.0 — native chat signing (closes MED-1) + pq-wipe.sh hardening
- v0.274.0 — Inc3b identify proof-of-possession (closes HIGH-2)

For older history see `docs/history/<date>.md` files + git log.

## Tier criteria — how to decide where something goes

- **TIER 0**: "We can't credibly invite strangers until this is done." Operator-attended OK.
- **TIER 1**: "We can invite known people but not unknown people until this is done." Self-service operator can fix.
- **TIER 2**: "Feature is promised but doesn't fully work." Multi-week effort.
- **TIER 3**: "Real users can use the app but they need help understanding it." Mission-critical for accessibility.
- **TIER 4**: "Nice eventually; don't let it crowd out the load-bearing work."

When adding an item, pick the LOWEST tier it could justifiably go in (i.e., the most urgent). Tier-up is rare; tier-down is normal as we discover things are less critical than they felt.
