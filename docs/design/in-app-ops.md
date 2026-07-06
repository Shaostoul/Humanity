# In-App Ops: design

> **Status:** design + living backlog (2026-05-20). Codifies the "GUI-first configurability (no-CLI-required)" non-negotiable rule from CLAUDE.md and tracks the CLI debt to pay down.

## The principle

Operator, verbatim (2026-05-20):

> "We want to make as many options/settings/configs accessible via the app. I'd prefer, for my sake and for the tech illiterate, that we avoid typing on the command line as much as possible. I don't mind if a button is effectively just running the console command. The easier it is for people to setup/use/modify the better. As an added benefit it is easier for you, Claude, to work the app since you'll be able to perfectly engineer always knowing what commands are available to you."

Three constituencies, one design:
1. **The operator**, prefers GUI; shouldn't need a terminal to run their own relay.
2. **Tech-illiterate users**, the accessibility mission (billions of people, most non-technical).
3. **AI agents**, a discoverable, structured in-app action surface lets an AI *enumerate* capabilities + invoke them correctly, instead of guessing shell commands. This is a force-multiplier for AI-assisted operation + development.

A button that just shells out to the equivalent command is acceptable. The requirement is **reachability from inside the app**, not reimplementation.

## North star: the admin action registry

Every admin/ops action is an entry in a data-driven registry (`infinite-of-x`: actions are data, not scattered hardcoded handlers). Each entry declares:

```
{ id, label, description, category, params:[...], danger_level, executor }
```

- The **GUI** (native Server Settings + web admin) renders the registry into panels/buttons/forms automatically, add an action, it appears, no per-action UI plumbing.
- An **AI agent** can fetch the registry (a read endpoint), see every action + its params + danger level, and invoke them through one authenticated "run action" endpoint. This is how an AI "perfectly knows what commands are available."
- **Danger levels** gate confirmation: `safe` (one click), `caution` (confirm), `destructive` (type-to-confirm, matches the operator's per-action-consent preference for prod-destructive ops).

This is the destination. We build toward it incrementally, each panel below is a step, and once 2-3 exist, factor out the registry so the rest fall out cheaply.

## Audit: what requires the CLI today (the debt)

Most of this is the TIER 0/1 ops work (built fast under incident pressure, all VPS-side shell/config). Each row: current CLI reality → in-app home.

| Capability | CLI today | In-app home | Notes / needs |
|---|---|---|---|
| **Alert channels** (ntfy/Discord/Telegram/webhook) | edit `data/alert-channels.secrets.json` over SSH | Server Settings → Alerts | relay GET/PUT endpoint (admin-Dilithium-authed) that reads/writes the file the watchdog consumes; UI list + add/edit/remove + a "send test alert" button. **Freshest debt, good first slice.** |
| **Backups**: status, manual backup, restore | `humanity-backup-db`, `backup-relay-from-vps.ps1`, manual `cp` restore | Server Settings → Backups (+ ties into device-mesh) | show last backup time/size/count; "Back up now" button; "Restore from…" (attended, type-to-confirm). |
| **Federation peers**: add/trust/defederate | raw `add_federated_server` / `set_server_trust_tier` storage calls | Server Settings → Federation | **NATIVE SHIPPED v0.722.0** (list + add + trust dropdown + confirmed remove + connect-all, driving the /server-* commands). Web mirror remains; spec: `docs/design/federation-activation.md` Phase 1. |
| **fail2ban**: view banned IPs, unban | `fail2ban-client status` / `set ... unbanip` over SSH | Server Settings → Security/Firewall | needs a relay→fail2ban bridge (sudo-gated helper, like the existing `scripts/sudoers.d/humanity-relay-services` pattern). Read-mostly + an unban button. |
| **Relay control**: restart, status | `systemctl restart humanity-relay` | Server Settings → System | the relay can't restart itself cleanly via its own process; route through the existing service-control sudoers helper (the `service_control`/`service_state` WS messages already exist, extend). |
| **Health/system view**: uptime, disk, version, watchdog state, cert expiry | SSH + `df`/`certbot`/`journalctl` | Server Settings → System/Health | mostly READ. **SHIPPED**: web v0.287.0; native v0.720.0 (Server Settings → admin → "System health": status / deployed build / uptime / messages / peers via the public /health + /api/stats, worker-thread fetch + Refresh). Remaining depth: disk / cert / watchdog state need the small `/api/admin/system` signed-read endpoint. |
| **Secrets rotation**: API_SECRET, WEBHOOK_SECRET, VAPID | edit `.env` over SSH + restart | Server Settings → Security | sensitive; "rotate" buttons that generate + write + restart. Destructive-confirm. WEBHOOK_SECRET also needs the GitHub-side update (can't fully automate). |
| **Admin roster**: who's admin (ADMIN_KEYS) | edit `.env` + restart, or `set_role` | Server Settings → Roles (partly exists) | the roles editor exists; surface ADMIN_KEYS sync there. |
| **Disk-guard / retention knobs** | env vars on the systemd unit | Server Settings → System | thresholds (warn/crit %, keep-counts) as editable settings. |
| **Deploy / version** | `git`/CI/`just` | Ops page (web has one) | mostly informational in-app; actual deploy stays CI (a "deploy" button would need careful auth). |

## What's ALREADY in-app (the model to follow)

Server Settings (native `src/gui/pages/server_settings.rs` + web) already does a lot the right way: roles editor, channels grid, banned-users panel, muted-users panel, server-policy matrix, PQ/sharing toggles. These prove the pattern, admin-authed WS messages (`ServerSettingsUpdate`, `set_user_role`, `BannedListRequest`/`Unban`, etc.) + a rendered panel. New ops panels extend exactly this.

Also: the theme system is the gold standard, `theme_editor_coverage` test ENFORCES that 100% of theme tokens are editable in-app. The action registry should eventually get the same kind of coverage test ("every admin action is reachable in the GUI").

## Build order (recommendation)

1. **System/Health dashboard (read-only)**, lowest risk, fastest, immediately replaces a bunch of SSH. Live uptime/disk/version/watchdog/cert in Server Settings. Proves the read path + a small `/api/admin/system` endpoint.
2. **Alert channels editor**, freshest debt; self-contained; first WRITE panel (relay reads/writes the watchdog's config file). Includes a "send test alert" button.
3. **Backups panel**, status + "back up now" + restore. Ties into device-mesh Phase B.
4. **Federation panel**, per `federation-activation.md` Phase 1.
5. **fail2ban + relay-control + secrets**, these need the sudo-gated relay→system bridge; do them together once that bridge exists.
6. **Factor out the action registry**, once 3-4 panels exist, extract the common shape so remaining actions are data entries, and add an AI-facing "list actions" + "run action" endpoint + a coverage test.

Each step is its own increment; each pays down real CLI debt and demonstrably advances the no-terminal-required goal.

## Guardrails

- **Auth**: every admin action is Dilithium-authed (the relay's `verify_dilithium_signature` path), admin/owner role required. Never expose an unauthenticated ops endpoint.
- **Danger gating**: destructive actions (restore, wipe, secret rotation, defederate) require type-to-confirm in the GUI, matches the operator's standing rule of explicit per-action consent for prod-destructive ops.
- **The CLI stays too**: GUI-first does NOT mean GUI-only. The shell scripts remain (for recovery when the relay is down, for the BUS-FACTOR successor, for automation). The GUI is the primary path; the CLI is the fallback. Both stay in sync via shared scripts where possible (e.g., a "back up now" button shells out to the same `humanity-backup-db` the timer uses).
