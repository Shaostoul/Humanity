# HumanityOS — Priorities

> **Read this file first if you're picking up work without context.** This is the strict-ranked backlog. The TOP item of TIER 0 is what gets worked on next; everything else waits.
>
> **Update rule:** every session that meaningfully changes scope updates this file before ending. The orchestrator_state.json journal records WHY a decision was made; this file records WHAT comes next. Don't mistake one for the other.

## Active focus
<!-- Set this to the single most important thing right now. Should match the top item in TIER 0. -->
**v0.283.x — strategic infrastructure docs (this file, BUS-FACTOR, INCIDENT-PLAYBOOK, SECURITY-CADENCE, HEALTH-DASHBOARD)** — once shipped, switch to TIER 0 #1.

## TIER 0 — pre-public launch blockers
Items here are mandatory before inviting public users. Operator-attended where noted. **Order matters within the tier.**

1. **Inc6 — attended fresh-slate wipe.** Operator runs `just pq-wipe yes` on VPS; re-onboards web+native from seed; verifies cross-client DM round-trip. All supporting code shipped; this is the activation flip.
   - Blockers: nothing in code; needs operator availability.
   - Recovery if it goes wrong: `/opt/Humanity/backups/relay-PREWIPE-<ts>.db` exists by construction.
   - Doc: orchestrator_state.json `open_questions_for_human` Inc6 entry.

2. **TLS auto-renew sanity check.** SSH `humanity-vps` → `systemctl list-timers | grep certbot` → confirm a recent run + a near-future scheduled one. If absent, `apt-get install certbot python3-certbot-nginx && certbot --nginx`.

3. **API_SECRET length audit.** SSH `humanity-vps` → `grep API_SECRET /opt/Humanity/.env | cut -d= -f2 | tr -d '\r\n' | wc -c`. If < 32 chars: rotate. v0.279.0 warns at startup but doesn't refuse to boot.

4. **VPS backup automation.** Currently relies on `pq-wipe.sh` snapshots + nothing else for disaster recovery. Real solution: Litestream replication to S3-compatible storage (or another VPS). Interim: nightly `rsync` of `/opt/Humanity/data/relay.db` to a second box via cron. Disk-guard rotates `backups/` already.

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
