# HumanityOS — Priorities

> **Read this file first if you're picking up work without context.** This is the strict-ranked backlog. The TOP item of TIER 0 is what gets worked on next; everything else waits.
>
> **Update rule:** every session that meaningfully changes scope updates this file before ending. The orchestrator_state.json journal records WHY a decision was made; this file records WHAT comes next. Don't mistake one for the other.

## Active focus
<!-- Set this to the single most important thing right now. -->
**ACTIVE: Clean web chat VIEW rebuild (Track W, pivoted 2026-05-26).** Operator's call: stop incrementally patching the tangled web view — rebuild it from scratch to mirror native 1:1, **keep the proven JS engine** (WS/crypto/WebRTC), and make sync *mechanical*. Live chat is non-precious (no users) so we rebuild in place. **Spec + sync backbone: `docs/design/chat-layout.md`** — one web `view/*` module per native `draw_*` (same names), engine↔view boundary via the `hos` event bus, DOM stays (accessibility improves, never canvas; WASM considered + rejected for canvas/a11y reasons). Build order: scaffold + constants + event-bus boundary → engine extraction from app.js → centerPanel/messageRow/timestampPill → leftRail → rightRail → composer/header → modals → sweep old view files + dead CSS. Incremental-patch history (now superseded): left+right rails done (v0.287.4-.9), nav labels (v0.287.7), message-row flatten + grouped pill rows (v0.287.10/.11) — these stay live as the clean rebuild replaces them section by section. Native WebRTC transport remains the committed parallel next-major-effort (unblocks native voice + streaming).

## TIER 0 — pre-public launch blockers
Items here are mandatory before inviting public users. Operator-attended where noted. **Order matters within the tier.**

1. **Fix nginx `/health` routing.** Internal `http://localhost:3210/health` returns 200; public `https://united-humanity.us/health` returns 404. nginx isn't routing the path. Trivial nginx config addition (`location = /health { proxy_pass http://127.0.0.1:3210; }`). Matters because off-site monitoring (TIER 1 #2) needs the public endpoint to work.

2. **DONE: GitHub webhook deleted + endpoint fail-closed (v0.285.0).** The stale webhook (pointed at a dead ngrok URL, 404 for months) was deleted from the GitHub repo. The relay's `/api/github-webhook` endpoint now FAILS CLOSED — rejects when `WEBHOOK_SECRET` is unset (was fail-open, a forged-announcement spoof vector). Note: this webhook was NEVER the update-autoposter — that's the CI Deploy Bot via `/api/send` + `API_SECRET`, a separate path that's unaffected and healthy.

3. **DONE: off-site backup (stopgap).** 2026-05-20: `scripts/backup-relay-from-vps.ps1` + a Windows scheduled task ("HumanityOS Relay Backup Pull", every 6h) now pull the live relay DB from the VPS to the operator's PC — genuine 3-2-1 backup (live DB / VPS-local 30-min snapshots / off-site PC). This is the "immediate" half of the device-mesh vision (`docs/design/device-mesh.md`); the full in-app version is TIER 2. NOTE: the PC backup is off-site but a SINGLE off-site copy. A second target (phone, NAS, or a cheap second VPS) would make it 3-2-1-with-redundancy. Phase B of the device mesh generalizes this.

4. **DONE: 2026-05-21 release-mirror cleanup + retention automation.** Cleaned 277 old release dirs from `/var/www/humanity/releases/` (freed 91 GB; 91% → 13%). v0.283.4 extends `scripts/humanity-disk-guard.sh` to enforce 10-version retention automatically on every 20-min cycle + regenerate the manifest. Cascade is structurally prevented from recurring.

5. **DONE: backup script repaired + in-repo.** The pre-v0.90.0 path bug was silently backing up an empty fossil DB for over a month. v0.283.4 ships `scripts/humanity-backup-db.sh` as the source of truth, the `deploy.yml` workflow now copies it to `/usr/local/bin/humanity-backup-db` on every deploy. Fossil backups moved to `backups/fossil-pre-v0.90/` for historical interest only.

6. **DONE: Orphan Ed25519 admin rows cleanup.** 2026-05-21: ADMIN_KEYS env updated to Shaostoul's Dilithium hex (3904 chars), 4 orphan rows DELETEd, relay restarted, verified `user_roles` is Dilithium-only.

7. **DONE: Inc6 attended wipe.** Verified 2026-05-20 by direct SQL.

8. **DONE: TLS auto-renew sanity check.** certbot.timer runs on a 12h cycle; last run 2026-05-20 16:42, next 2026-05-21 06:15. All 3 certs valid 50-68 days out. No action needed.

9. **DONE: API_SECRET length audit.** 64 chars (above 32-char threshold). No action needed.

## TIER 1 — hardening before invites scale beyond known group
Items here protect against the realistic adversary (script kiddie, opportunistic abuser, eager fan with sticky fingers). Order within tier is flexible; pick what's cheapest first.

**TIER 1 is effectively closed.** All code-actionable items shipped; the two decision-gated items were decided by the operator 2026-05-20 (fail2ban over Cloudflare; skip off-box monitor; plan federation). Remaining federation *implementation* is tracked in TIER 2.

1. **DONE: DDoS protection — fail2ban (v0.286.x).** Operator chose self-hosted fail2ban over Cloudflare. nginx jails added (`scripts/fail2ban/nginx.local`): `nginx-limit-req` (bans IPs repeatedly tripping nginx rate limits) + `nginx-botsearch` (bans exploit-path scanners), conservative thresholds + `ignoreip` for loopback/private. sshd jail was already active. Installed live + version-controlled (deploy.yml installs + reloads). Composes with the in-app gates (v0.279/v0.280).

2. **DONE (VPS-side): Monitoring + alerting (v0.286.2).** Watchdog (2-min liveness + self-heal) + `scripts/humanity-alert.js` configurable multi-channel external alerting (ntfy/Discord/Telegram/webhook), wired into watchdog + disk-guard. Admin opt-in via `data/alert-channels.secrets.json`. **Off-box monitor (whole-VPS-down) explicitly SKIPPED per operator 2026-05-20** ("not too concerned"). If revisited: a free uptime service or PC scheduled task can reuse the same alert channels.

3. **DONE: SQLite corruption recovery (v0.286.0).** `Storage::open_resilient` — boot integrity check + restore-newest-healthy-backup or refuse-to-start. 4 tests.

4. **Federation: design DONE, implementation in TIER 2.** Operator chose "plan activation." Design + vetting + abuse model + 4-phase plan in `docs/design/federation-activation.md`. Key finding: federation is already fail-closed (trust_tier 0 default; unknown peers can't connect), so dormant = safe; the implementation phases (admin UI, profile-gossip rate limit, second-VPS end-to-end test, then third-party peers) are the work. Moved to TIER 2 #1.

5. **DONE (via watchdog, v0.285.2): crash-loop detection.** Watchdog self-heals + alerts (chose this over systemd StartLimit, which would give up + leave the relay dead — bad for unattended).

## TIER 2 — big-feature gaps
Items here are real features the system promises but doesn't deliver on every platform. Weeks of work each.

> **Cross-cutting mandate (CLAUDE.md non-negotiable rule, 2026-05-20): GUI-first configurability.** Every ops/config capability must be reachable in-app, not CLI-only. The recent TIER 0/1 ops work (alerts, backups, fail2ban, watchdog, secrets) is all CLI/SSH today — that's tracked debt. See `docs/design/in-app-ops.md` for the audit + the north-star admin action registry (GUI renders it AND an AI can enumerate it) + the build order. NEW features with an ops dimension build their in-app control in the same increment.

1. **Web-mirrors-native parity (Track W — ACTIVE).** Full divergence map + migration order in `docs/design/web-native-parity.md`. Native chat is the parent; web is the old UI being rebuilt to mirror it, incrementally (web stays usable throughout; theme tokens already shared). Migration order: (1) left-rail tabs→stacked-collapsible-sections ✅ + 1b studio→right ✅ + 1c scratchpad top-row ✅ + 1d identity→account-menu ✅, (2) right-rail Friends/Members ✅, (3) message rows + timestamp pill + inline reactions **[NEXT]**, (4) header + composer, (5) top-nav alignment (labels ✅ v0.287.7; native tiering pending), (6) spacing sweep + dead-CSS removal (`style.css`, `chat-voice.js` are dead). Each step = its own increment + version bump.

2. **Studio + streaming (Track S — phased, dependency-ordered).** Full vision in `docs/design/studio-streaming.md`. Right-rail studio widget (top, for streamers) + full Studio modal + docked inverted chat + per-friend viewer widgets + multi-stream viewer modal + **persistent stream across all pages** + **privacy guard** (auto-hide on sensitive pages/buttons). KEY CONSTRAINT: streaming transport exists on WEB (real WebRTC) but NOT native (stubs only); native Studio is a UI page with no transport. So build the widget on web first (functional), mirror to native once native transport exists. Order: S0 persistent session (gate for "always stream" + viewers) → S1 web studio widget+modal → S2 viewer widgets+modal → S3 privacy guard (can land early, independent) → S4 native mirror. Native transport = the same weeks-long WebRTC lift as native voice (#4).

3. **In-app ops console (phased — pays down the CLI debt).** Per `docs/design/in-app-ops.md`. Slice 1 (System/Health dashboard) SHIPPED v0.287.0 (web). Remaining: native parity for it, then (2) Alert-channels editor (first write panel), (3) Backups panel, (4) Federation panel (= #5 Phase 1), (5) fail2ban/relay-control/secrets (need a sudo-gated relay→system bridge), (6) factor out the action registry + AI-facing list/run endpoints + a coverage test.

4. **Native voice.** Channel-list voice icon click is a TODO (chat.rs:1060). No WebRTC stack at all. Needs: `webrtc-rs` integration, audio capture → kira pipeline, playback routing for N peers, mute/deafen UI, connection state machine. Web users have voice; native users are observer-only today. (Shares the WebRTC-transport lift with Track S native streaming — do them together.)

5. **Federation activation (phased).** Design done — `docs/design/federation-activation.md`. Phase 1: Server Settings → Federation admin UI (list/add/trust/defederate peers + per-channel federation toggle), native + web. Phase 2: per-peer profile-gossip rate limit. Phase 3: second operator-controlled relay, federate the two, verify end-to-end — esp. whether moderation propagates to federated content (load-bearing test). Phase 4: open to vetted third-party peers. Fail-closed default = safe to build incrementally.

6. **Native streaming viewer.** Subsumed into Track S (S4 native mirror).

7. **Native trade UI completion.** Trade page exists in `src/gui/pages/`. Trade events (`trade_response`, `trade_confirm`, etc.) aren't dispatched. Either wire them up or remove the page until ready.

4. **Litestream / continuous backup.** Beyond the nightly rsync floor in TIER 0, set up real continuous replication. SQLite WAL → S3-compatible blob storage. RPO ~1 minute, RTO ~10 minutes from cold.

5. **Mobile clients.** Android (JNI bridge for keyring + AndroidKeyStore; new keychain backend), iOS (Keychain Services already works via `keyring` crate — needs only an iOS build target). Big effort either way.

6. **Device mesh** (design doc: `docs/design/device-mesh.md`). The operator's vision: your devices back up each other + the relay; review all devices' system-info (hardware, storage, health) from any one device; device roles (battle-station / accessory / relay / archive). Phased: A) system-info reporting + "My Devices" dashboard, B) backup designation + pull + encryption-at-rest (subsumes the shipped PowerShell stopgap), C) restore flow, D) LAN direct-sync + mobile mesh members + remote wipe. The VPS-as-rendezvous architecture (devices report up, read all-devices down) fits the existing federation model. On-mission sovereignty tooling — give it to every user, not just the operator.

7. **Library — federated file/media catalog (NEW, designed 2026-05-26).** Full design in `docs/design/library.md`. One "free public access" page, tabbed by consume-mode: **Files** (federation-hosted media/art/3D models — download in, upload, pin) + **Software** (folds in the Tools page) + **Web** (folds in Browser + Resources). Files engine = trust-tiered LRU cache (unverified shared pool + per-user sub-cap; verified+ per-user quota → **bounded disk by construction**) + curated permanent tier (roled pin → permanent + quota refund → routed to the existing torrent seeder + Internet Archive). Identity by **content hash (SHA-256)**: exact dupes auto-link; near-dupes (image perceptual hash) trigger a side-by-side **preview-confirmation dialog** (3D/binaries: exact-hash only). Rule: **ephemeral = server-local; pinned = federated**; catalog aggregates lightweight metadata across `/api/federation/servers`, grouped by source server. Extends `assets.rs`/`uploads.rs`/`roles.rs`/`pins.rs`/`server_settings.rs` + `docs/torrent-infrastructure.md`. Phased: Files engine → Library/Files UI (web→native) → pin/torrent → perceptual dedup → federation aggregation → fold Tools/Browser/Resources in. Seed content: the 187 archived Project Universe media files. GUI-first quota/cap config per server admin.

8. **P2P Groups — relay-independent groups (NEW, designed 2026-05-27; operator chose "true P2P" over relay-mediated/federated-fallback).** Full design + phased plan in `docs/design/p2p-groups.md`. Today groups are 100% relay-mediated (`handle_group_create/join/msg` → relay SQLite), so a relay outage breaks create/join/messaging and the invite URL 404s — contradicts "no single point of failure." Target: a group is a **signed object + append-only signed membership/message logs** replicated peer-to-peer over the existing WebRTC DataChannels (`web/chat/chat-p2p.js`); relays are **optional accelerators** only. Invite = **signed connection ticket** (not a URL). E2EE via a per-epoch group key (generalize the Kyber768 dual-seal in `src/net/dm_pq.rs`), re-keyed on membership change. **Core gap = relay-independent signaling** (today `webrtc_signal` rides one relay) → solved by multi-relay failover + peer-assisted signaling (+ TURN/peer-relay for NAT). Phased: **P1** sovereign data + working signed-ticket invite (fixes the 404; relay still signals) → **P2** signed + E2EE messages → **P3** P2P transport (relay = signaling-only) → **P4** relay-independence (the payoff: kill the home relay, a group with ≥1 reachable peer still works) → **P5** serverless discovery (mDNS/DHT). **Open decisions need operator review before P2/P4** (in the doc): TURN strategy, epoch-key vs per-message KEM, relay-as-accelerator. Builds on the signed-object/gossip model (`storage-architecture.md`) + signed-log governance (`signed_moderation_logs.md`).

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
