# HumanityOS: Health Dashboard

> **What "healthy" looks like + how we'd know if it isn't.** This file is the source of truth for SLOs (service-level objectives) and the alert criteria that fire when reality diverges from them.
>
> **Today (2026-05-20): we have ZERO active monitoring.** If the relay goes down between deploys, nobody knows until a human notices. This file commits the targets we're aiming AT; instrumentation to actually measure them is TIER 1 #2 in PRIORITIES.md.
>
> **Update rule:** SLO changes get an entry in the change log at the bottom. Adjusting an SLO is a deliberate decision, not a drift.

## Service-level objectives

### Relay availability
| Metric | Target | Current measurement |
|---|---|---|
| Uptime over 30 days | 99.5% (≤ ~3.6 hours downtime/month) | **Not measured**, TIER 1 #2 |
| `/health` HTTP 200 response time (p95) | < 200ms | Not measured |
| `/health` reachable from 3 geographic regions | Yes | Not measured (only operator's location verified) |

99.5% is a deliberately modest target for a single-VPS deployment. Pushing higher requires geographic redundancy (TIER 4).

### WebSocket connection health
| Metric | Target | Current measurement |
|---|---|---|
| WS handshake success rate (over last hour) | > 99% | Not measured |
| `identify` round-trip latency (p95) | < 500ms | Not measured |
| Chat-send → broadcast round-trip (p95, same channel) | < 300ms | Not measured |
| Connections rejected by anti-spam gates (per hour) | < 1% of total | Logged but not alerted on |

### DM delivery
| Metric | Target | Current measurement |
|---|---|---|
| DM end-to-end delivery latency (sender click → recipient render, both clients connected) | < 1 second p95 | Not measured |
| DM decryption success rate | 100% (anything less = crypto bug) | Not measured (silent failures would slip past today) |

### Federation
| Metric | Target | Current measurement |
|---|---|---|
| Federated peer reconnect time after disconnect | < 5 minutes | Backoff capped at 5 min, measurable from logs |
| Cross-server message propagation latency (p95) | < 2 seconds | Not measured |
| Profile gossip rejection rate | < 0.1% (rejections = signature failures = potential abuse) | Logged but not alerted |

### Resource limits
| Metric | Target | Current measurement |
|---|---|---|
| `/opt` disk usage | < 70% | Checked by disk-guard timer (alerts at 80% by deletion, not by warning) |
| Relay process RSS memory | < 500 MB sustained | Not measured |
| SQLite DB size growth | < 100 MB/month sustained | Not measured |

## Alert criteria: what should page someone

### P0: wake someone up at 3am
- Relay returns non-200 from `/health` for 3 consecutive minutes.
- TLS cert expires within 7 days (means autorenewal failed).
- Disk `/opt` above 90%.
- > 100 failed identify-challenge verifications in 1 hour (signals active attack on identity spoofing).

### P1: notify next morning
- Relay uptime over last 24h below 98%.
- Backup automation hasn't fired in > 48 hours.
- > 10 `tracing::error!` events in the relay log over the last hour.
- A federation peer that was trusted (trust_tier ≥ 2) has been disconnected for > 24 hours.

### P1: notify next morning (release signing)
- **The LATEST release is UNSIGNED (no `release-manifest.json.sig.json` asset).** The v0.421+
  desktop updater enforces signatures and offers nothing when the latest signed version lags, so
  unsigned releases silently freeze desktop auto-update (this went unnoticed for ~48 releases,
  v0.421.0 -> v0.469.0). **Check:** `just check-signing` (or `node scripts/check-release-signing.js`);
  it also runs inside `just status`. **Fix (operator only):** `export HUMANITY_SIGNING_PASSPHRASE=...
  && just sign-release vX.Y.Z`. Treat an unsigned LATEST as a launch-blocking P1 every release.

### P2: log and review at next scheduled session
- A user reports an issue via the chat-side `/report` flow.
- An admin uses the moderation actions (ban / mute / kick), record but don't alert.
- A bot authentication fails (logged in `tracing::warn!`).
- Deploy bot reports a CI failure (already surfaces in #announcements).

## Instrumentation plan: status

**DONE (v0.285.2–v0.286.x):** VPS-side detection + self-heal + configurable external alerting. See "Existing observability" + "Alert configuration" below.

**Remaining:**
1. **Off-box health monitor** (the other half of TIER 1 #2): a free service (UptimeRobot, BetterStack) or a check from the operator's PC hits `https://united-humanity.us/health` every ~60s and alerts on failure, covers whole-VPS-down, which the on-VPS watchdog can't. Can reuse the same `humanity-alert.js` channels.
2. **Log-rate alerting**: `journalctl -p err --since "1 hour ago" | wc -l` in a cron → fan out via `humanity-alert.js` if over threshold.
3. **Cert-expiry pre-warning**: weekly cron checks `certbot certificates`, alerts if < 14 days (belt-and-suspenders on top of auto-renew).

Better (future): Prometheus `/metrics` (needs a `prometheus` crate + RelayState counters) + Grafana + Alertmanager; trace sampling via the already-present `tracing-subscriber`. Defer: distributed tracing across federation peers; real-time anomaly detection (only useful at scale).

## Existing observability (what we have today)

- **`/health` endpoint** returns `{"status":"ok"}` with HTTP 200 (basic liveness). Returns peer count under `/api/stats`. Public route works as of v0.285.2 (`https://united-humanity.us/health`).
- **Relay watchdog** (`humanity-relay-watchdog.timer`, every 2 min, v0.285.2): HTTP-liveness check + self-heal restart. Detection + self-heal for "relay down/hung, VPS up."
- **Configurable external alerting** (v0.286.x, TIER 1 #2): the watchdog + disk-guard fan critical alerts out to admin-configured channels via `scripts/humanity-alert.js`. See "Alert configuration" below.
- **SQLite corruption resilience** (v0.286.0): boot-time integrity check + restore-from-healthy-backup or refuse-to-start.
- **journalctl logs** for `humanity-relay.service` capture `tracing` output. `RUST_LOG=info` default, configurable in `.env`. Watchdog logs under tag `humanity-relay-watchdog`; disk-guard under `humanity-disk-guard`.
- **GitHub deploy log** for CI run history.
- **Deploy Bot's announcements** in `#announcements` for successful deploys, semi-passive monitoring (a missing announcement = something is wrong).

## Alert configuration (per server admin)

The watchdog + disk-guard send critical alerts out through whatever channels the admin configures. **It's opt-in: no config = silent no-op.**

1. On the VPS: `cp /opt/Humanity/data/alert-channels.example.json /opt/Humanity/data/alert-channels.secrets.json` (the `.secrets.json` name is gitignored, it holds tokens/URLs).
2. Edit it; set `"enabled": true` on the channels you want. Supported types:
   - **`ntfy`**, phone push; just a topic URL (`https://ntfy.sh/<random-topic>`). Easiest.
   - **`discord`**, an incoming-webhook URL.
   - **`telegram`**, a bot token + chat id.
   - **`webhook`** / **`slack`**, POST JSON to any URL (covers Slack incoming webhooks + custom receivers).
3. Test without sending real alerts:
   `HUMANITY_ALERT_DRYRUN=1 node /opt/Humanity/scripts/humanity-alert.js "test" critical`
   then for real (drop the env var) once you trust it.

What fires an external alert (anti-spam: once on down, once on recovery, never per-cycle):
- Relay DOWN, watchdog auto-restarting → **warn**
- Relay DOWN + binary missing (restart can't fix, needs a deploy) → **critical**
- Relay RECOVERED → **info**
- Disk critical after auto-reclaim → **critical**

**Still missing, the off-box layer.** All the above runs ON the VPS, so it covers "relay down / VPS up." It does NOT cover whole-VPS-down (power/network loss), that needs an external monitor (a free uptime service like UptimeRobot/BetterStack hitting the public `/health`, or a check from the operator's PC). Wiring that is the remaining half of TIER 1 #2; it can reuse the same alert channels.

Better (future):
- **Prometheus**: relay exposes `/metrics` (need to add, `prometheus` crate, scrape RelayState counters). Prometheus on a second host scrapes. Grafana for dashboards. Alertmanager for routing.
- **Trace sampling**: `tracing-subscriber` already in deps; emit spans to a collector (OpenTelemetry → Jaeger or Tempo).

## Manual health check: what an operator does today

```bash
# Quick liveness
curl -sw "%{http_code} %{time_total}s\n" https://united-humanity.us/health

# Service state on VPS
ssh humanity-vps "
  systemctl is-active humanity-relay nginx forgejo;
  df -h /opt | tail -1;
  certbot certificates 2>&1 | grep -E 'Domain:|Expiry'
"

# Recent error rate
ssh humanity-vps "journalctl -u humanity-relay --since '1 hour ago' | grep -cE 'ERROR|WARN'"
```

If all are green: probably fine. Anything red → INCIDENT-PLAYBOOK.md.

## Change log
- 2026-05-20, initial creation; SLOs set as targets. Instrumentation deferred to TIER 1 #2.
