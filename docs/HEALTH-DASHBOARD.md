# HumanityOS — Health Dashboard

> **What "healthy" looks like + how we'd know if it isn't.** This file is the source of truth for SLOs (service-level objectives) and the alert criteria that fire when reality diverges from them.
>
> **Today (2026-05-20): we have ZERO active monitoring.** If the relay goes down between deploys, nobody knows until a human notices. This file commits the targets we're aiming AT; instrumentation to actually measure them is TIER 1 #2 in PRIORITIES.md.
>
> **Update rule:** SLO changes get an entry in the change log at the bottom. Adjusting an SLO is a deliberate decision, not a drift.

## Service-level objectives

### Relay availability
| Metric | Target | Current measurement |
|---|---|---|
| Uptime over 30 days | 99.5% (≤ ~3.6 hours downtime/month) | **Not measured** — TIER 1 #2 |
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
| Federated peer reconnect time after disconnect | < 5 minutes | Backoff capped at 5 min — measurable from logs |
| Cross-server message propagation latency (p95) | < 2 seconds | Not measured |
| Profile gossip rejection rate | < 0.1% (rejections = signature failures = potential abuse) | Logged but not alerted |

### Resource limits
| Metric | Target | Current measurement |
|---|---|---|
| `/opt` disk usage | < 70% | Checked by disk-guard timer (alerts at 80% by deletion, not by warning) |
| Relay process RSS memory | < 500 MB sustained | Not measured |
| SQLite DB size growth | < 100 MB/month sustained | Not measured |

## Alert criteria — what should page someone

### P0 — wake someone up at 3am
- Relay returns non-200 from `/health` for 3 consecutive minutes.
- TLS cert expires within 7 days (means autorenewal failed).
- Disk `/opt` above 90%.
- > 100 failed identify-challenge verifications in 1 hour (signals active attack on identity spoofing).

### P1 — notify next morning
- Relay uptime over last 24h below 98%.
- Backup automation hasn't fired in > 48 hours.
- > 10 `tracing::error!` events in the relay log over the last hour.
- A federation peer that was trusted (trust_tier ≥ 2) has been disconnected for > 24 hours.

### P2 — log and review at next scheduled session
- A user reports an issue via the chat-side `/report` flow.
- An admin uses the moderation actions (ban / mute / kick) — record but don't alert.
- A bot authentication fails (logged in `tracing::warn!`).
- Deploy bot reports a CI failure (already surfaces in #announcements).

## Instrumentation plan (TIER 1 #2 — not yet built)

Minimum viable:
1. **Health check loop**: a second VPS or even a free service (UptimeRobot, BetterStack, etc.) hits `https://united-humanity.us/health` every 60s. Posts to ntfy.sh / Telegram bot / email on failure. Cost: free tier viable; ~$5/mo for a paid tier with longer history.
2. **Log shipping**: `journalctl -u humanity-relay` → grepped for `ERROR`/`WARN` patterns. Aggregate count → alert on threshold. Simplest: `journalctl -p err --since "1 hour ago" | wc -l` in a cron, post if > 10.
3. **Disk + cert expiry**: weekly cron on VPS itself: `df`, `certbot certificates`, post results to a notify channel.

Better:
1. **Prometheus**: relay exposes `/metrics` (need to add — `prometheus` crate, scrape RelayState counters). Prometheus on a second host scrapes. Grafana for dashboards. Alertmanager for routing. Cost: free if self-hosted on a 2nd $5/mo VPS.
2. **Trace sampling**: `tracing-subscriber` already in deps; emit spans to a collector (OpenTelemetry → Jaeger or Tempo). Useful for diagnosing latency outliers.

Defer:
- Distributed tracing across federation peers (only useful when there are 3+ peers).
- Real-time anomaly detection (only useful at significant user count).

## Existing observability (what we have today)

- **`/health` endpoint** returns `{"status":"ok"}` with HTTP 200 (basic liveness). Returns peer count under `/api/stats`.
- **journalctl logs** for `humanity-relay.service` capture `tracing` output. `RUST_LOG=info` default, configurable in `.env`.
- **GitHub deploy log** for CI run history.
- **Deploy Bot's announcements** in `#announcements` for successful deploys — semi-passive monitoring (a missing announcement = something is wrong).

That's it. Everything else needs to be built.

## Manual health check — what an operator does today

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
- 2026-05-20 — initial creation; SLOs set as targets. Instrumentation deferred to TIER 1 #2.
