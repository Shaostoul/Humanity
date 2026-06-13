# HumanityOS: Security Cadence

> **Security is not a thing you ship once; it's a posture you maintain.** This file is the calendar of mandatory exercises that keep the posture honest.
>
> **Update rule:** every completed exercise gets a row appended to its table. Overdue exercises are visible at a glance.

## Cadence overview

| Exercise | Frequency | Effort | Owner |
|---|---|---|---|
| Dependency audit (`cargo audit` + `npm audit`) | **Monthly** | 30 min if green; hours if a CVE hits a load-bearing dep | Operator or scheduled AI session |
| Pre-release smoke test | **Every minor release** | 5-10 min | Operator (manual + the build-game step) |
| Independent code review of recent diff | **Quarterly** | 1-2 hours (independent AI agent or trusted human) | Operator |
| Full pentest | **Annual** | days; budget-gated | Hired security firm if/when affordable |
| Backup-restore drill | **Quarterly** | 30 min on a staging VPS | Operator |
| Secrets rotation | **Annual** (or on suspected leak) | 15 min | Operator |
| Threat-model review | **Annual** or after major architectural change | 1 hour | Operator + AI session |

## 1. Dependency audit: monthly

### Procedure
```bash
cd /path/to/Humanity
cargo audit                              # Rust deps
cd web && npm audit --omit=dev 2>&1 | tail -20  # JS deps (when web has package.json)
```

### Pass criteria
- `cargo audit` reports `Success No vulnerable packages found`.
- `npm audit` reports `found 0 vulnerabilities` for production deps.

### Fail handling
- **High/Critical CVE in a load-bearing dep**: stop, fix today. Patch version bump → test → deploy.
- **Low/Moderate**: schedule for the next regular release; document the deferral here.

### Log
| Date | `cargo audit` | `npm audit` | Notes |
|---|---|---|---|
| 2026-05-20 | _scheduled_ | _scheduled_ | First run pending. Set calendar reminder. |

## 2. Pre-release smoke test: every minor release (0.X.0)

### Procedure
For every minor bump (Rust code changed), before `gh release create`:
1. `cargo test --features native`, full native suite green.
2. `cargo test --features relay --no-default-features`, full relay suite green.
3. `cargo build --features native --release` (`just build-game` does this + archive).
4. Launch the new exe, perform the **smoke checklist** below.
5. Only THEN tag + release.

### Smoke checklist (manual, ~5 min)
- [ ] App opens, connects to relay, identifies successfully.
- [ ] Send a message in #general → it appears for self.
- [ ] Send a DM to a test recipient (different identity) → both parties see it.
- [ ] Open Settings → confirm no panic, sliders move, theme tokens render.
- [ ] Right-click a message → context menu shows expected entries for your role.
- [ ] Reload the app → identity persists, auto-unlock works if configured.

### Fail handling
Any step fails: do NOT release. File the failure as an incident in INCIDENT-PLAYBOOK.md, fix, retest.

### Log
Maintain in git history: every `v0.X.0` release implies a passed smoke test by definition (no half-baked tags). Add explicit notes here when something almost-failed.

| Release | Date | Notes |
|---|---|---|
| _none_ | _ongoing_ | Convention adopted v0.283.x. Prior releases relied on test suite + ad-hoc verification. |

## 3. Independent code review: quarterly

### Procedure
Spawn a dedicated **independent** AI session (or hire a human reviewer) with no context from the active development session. Provide:
- The diff range to review (e.g., `git log v0.X.0..HEAD`).
- A scope statement: "audit for security flaws, no feature work, no code rewrites without an identified flaw, report MUST/SHOULD/NICE findings with severity."
- Output expected: written report committed to `docs/security-reviews/<date>-<scope>.md`.

The reviewer must NOT have access to the development session's context, independence is the whole point. The PQ-cutover review captured in CLAUDE.md's Cryptography section is the template.

### Pass criteria
- All HIGH and MEDIUM findings either fixed or explicitly accepted with documented rationale.
- LOW findings tracked in PRIORITIES.md.

### Log
| Date | Scope | Reviewer | Verdict | Findings |
|---|---|---|---|---|
| ~v0.266.x | Full v0.262.28..HEAD PQ-cutover diff | Independent AI agent | DM crypto SOUND; 1 HIGH + 1 HIGH + 1 MED + 1 LOW found, all subsequently FIXED | HIGH-1, HIGH-2, MED-1 ✅; LOW-1 ✅ (closed v0.279.0) |
| _next_ | post-v0.283 hardening sweep | TBD | _scheduled_ | _pending_ |

## 4. Full pentest: annual (budget-gated)

When operator can fund it (~$5-15K for a reputable boutique firm), hire one. Scope:
- All published REST endpoints.
- WebSocket protocol and all known message types.
- Web client auth flow (vault, key wrapping, DM E2EE).
- Native client (process inspection, memory dumps for key extraction).
- Federation protocol.

Until that's affordable: the quarterly independent review (item 3) is the substitute.

### Log
| Date | Firm | Cost | Findings |
|---|---|---|---|
| _none_ | _budget-gated_ | _-_ | _-_ |

## 5. Backup-restore drill: quarterly

### Why
A backup you never restore from is not a backup. Drills prove the system actually works.

### Procedure
1. On a staging VPS (or local Linux VM): set up the same `humanity-relay` service config.
2. Copy the most recent `/opt/Humanity/backups/relay-PREWIPE-*.db` from production.
3. Restore: stop service → `cp backup.db data/relay.db` → start service → curl `/health`.
4. Verify: identify with a known test seed → see expected channels + messages.
5. Pass = all steps complete in < 10 minutes from backup-in-hand.

### Pass criteria
- Restore completes without manual schema fixes.
- Test identity loads successfully.
- No data corruption visible in core tables.

### Log
| Date | Backup file | Time-to-restore | Issues |
|---|---|---|---|
| _none_ | _scheduled_ | _-_ | _-_ |

## 6. Secrets rotation: annual or on suspected leak

### What to rotate
- `API_SECRET` (relay `/opt/Humanity/.env`)
- `WEBHOOK_SECRET` (relay `.env` + GitHub repo webhook config)
- `VAPID_*` keys (relay `.env`; invalidates Web Push subscriptions, users re-allow)
- SSH deploy keys (regenerate, update GitHub Secrets, remove old from `~/.ssh/authorized_keys`)
- Forgejo admin password
- Operator's GitHub access tokens (PAT in `GH_TOKEN`)
- Domain registrar password

### Procedure
For each, see BUS-FACTOR.md "Secrets, locations, not values" section. Rotate, restart, verify the affected feature still works.

### Log
| Date | Secrets rotated | Reason | Issues |
|---|---|---|---|
| _none_ | _annual_ | _baseline_ | _-_ |

## 7. Threat-model review: annual or post-architecture-change

### Procedure
Re-read `CLAUDE.md` Cryptography section + `docs/design/storage-architecture.md`. Ask:
1. What's our adversary model today? (Casual abuser? Nation-state? Operator-malicious?)
2. What's changed in the architecture since the last review?
3. Are there new attack surfaces?
4. Are the cryptographic primitives still NIST-approved / industry-standard?
5. What's the worst plausible incident in the next 12 months and what's our response?

Output: an updated `docs/threat-model.md` (or section in this file) capturing current adversary classes + their assumed capabilities + our defenses.

### Log
| Date | Trigger | Outcome |
|---|---|---|
| 2026-05-03 | PQ-cutover migration | Independent review captured in CLAUDE.md Cryptography section. |
| _next_ | Annual (2027-05) or earlier on architecture change | _scheduled_ |

## Threat model snapshot: current (2026-05-20)

### Adversary classes we defend against

1. **Casual abuser** (script kiddie, troll). Wants to spam, harass, impersonate.
   - **Mitigations**: v0.279.0 + v0.280.0 anti-abuse gates, mute/ban/kick moderation tools, name registration (first-claim-wins), Inc3b challenge-response identity proof.
   - **Residual risk**: distributed scripts could circumvent per-IP caps. Defense: Cloudflare or similar L7 protection (TIER 1 #1).

2. **Network attacker** (intercepts traffic). Wants to read messages, modify them in flight.
   - **Mitigations**: TLS for all transports; DM E2EE via ML-KEM-768 + AES-GCM (server can't decrypt); identity sigs via ML-DSA-65 prevent in-flight modification.
   - **Residual risk**: TLS cert compromise via CA compromise. Defense: Let's Encrypt's monitoring + manual cert pinning for the chat client (deferred).

3. **Compromised operator** (someone gains VPS access). Wants to read users' data, impersonate users, plant backdoors.
   - **Mitigations**: DM E2EE means even root-on-VPS can't read DMs (Kyber sealed to user's seed-derived key). Identity = Dilithium with proof-of-possession at identify (Inc3b), operator can't log in AS a user without their seed.
   - **Residual risk**: operator can rewrite the deployed binary to phish for seeds. Defense: signed release binaries (deferred, currently CI builds + raw exe; no signing). Federation gossip means other servers cache identities so a single-server compromise is less catastrophic.

4. **Quantum adversary** (future). Wants to forge identities or decrypt archived DMs.
   - **Mitigations**: identity = Dilithium3 (ML-DSA-65, NIST FIPS 204); DM = Kyber768 (ML-KEM-768, NIST FIPS 203). Both are post-quantum.
   - **Residual risk**: PQC primitives are new (< decade of cryptanalysis vs decades for Ed25519/X25519). We're in the best position the industry currently knows; that's still PQC's first generation.

5. **DDoS attacker** (wants to make the network unavailable).
   - **Mitigations**: identify rate-limit, per-IP new-identity cap, Fibonacci backoff per-key, federation rate limit, max-connections cap.
   - **Residual risk**: no L7 WAF in front. Defense: TIER 1 #1.

6. **Insider** (a future moderator turns hostile, an AI agent goes rogue).
   - **Mitigations**: admin/owner protection (a mod can't action an admin), role caps enforced server-side, name-only target gates fixed v0.247.0.
   - **Residual risk**: a malicious admin can do a lot of damage (mass-ban, delete-as-admin). Audit log review is the safety net; today there's no admin audit log. Defense: deferred but a known gap.

### Adversaries we explicitly do NOT defend against (yet)

- **Nation-state intercept with budget**: TLS + PQC are the best we have, but a sufficiently-resourced adversary breaks endpoints (compromised OS, supply-chain) before they break the wire.
- **Endpoint compromise**: stolen unlocked device = attacker has seed. This is documented in BUS-FACTOR + the INCIDENT-PLAYBOOK "Stolen seed phrase" entry.
- **Operator coercion**: legal demands from a jurisdiction. The DM E2EE design means operator can't comply with "show us their messages" for DMs; they CAN comply with "show us metadata" (who messaged whom, when). The architecture choice is to make E2E confidentiality non-circumventable even by the operator.

## Update log
- 2026-05-20, initial creation; quarterly cadences set, first independent review captured retroactively.
