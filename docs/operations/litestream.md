# Litestream Replication for HumanityOS Relay

> **Status:** v0.110.0 — supported configuration documented; relay code is
> Litestream-ready (WAL mode enabled in `Storage::open`). Replication is the
> operator's responsibility to configure outside the binary.

The HumanityOS relay stores everything in a single SQLite database
(`/opt/Humanity/data/relay.db` on the standard VPS deploy). Litestream provides
async, S3-compatible streaming replication of that file with **zero application
changes** — the relay continues to use SQLite normally; Litestream watches the
WAL file and ships changes to remote storage every few seconds.

## Why Litestream

Per the strategic plan (decision 5): chosen over LiteFS or Turso for v1 because:
- **Zero app-code change** — works against the existing rusqlite usage.
- **No vendor lock-in** — S3-compatible target (Backblaze B2, Cloudflare R2,
  MinIO, AWS S3, etc.).
- **Degrades gracefully** — if the replica destination is unreachable, the
  relay keeps running; replication catches up when network returns.
- **Operator-friendly** — single binary, single config file, transparent.

## Pre-requisites already in place

The relay calls `PRAGMA journal_mode=WAL;` on every connection
(`src/relay/storage/mod.rs` in `Storage::open`). WAL is required for Litestream
and incurs no measurable overhead for our workload.

## Install Litestream

On the VPS (Debian/Ubuntu):
```bash
wget https://github.com/benbjohnson/litestream/releases/download/v0.3.13/litestream-v0.3.13-linux-amd64.deb
sudo dpkg -i litestream-v0.3.13-linux-amd64.deb
```

## Configure replication

Create `/etc/litestream.yml`:

```yaml
dbs:
  - path: /opt/Humanity/data/relay.db
    replicas:
      - type: s3
        bucket: humanity-relay-replica
        path: relay
        endpoint: https://s3.us-east-005.backblazeb2.com  # or your provider
        access-key-id: ${LITESTREAM_ACCESS_KEY_ID}
        secret-access-key: ${LITESTREAM_SECRET_ACCESS_KEY}
        # Snapshot frequency — full DB copy. Daily is fine for our scale.
        snapshot-interval: 24h
        # Retention — keep 30 days of history for point-in-time restore.
        retention: 720h
```

Set the credentials in `/etc/default/litestream` (env file, owner `root:root`,
mode `0600`):
```bash
LITESTREAM_ACCESS_KEY_ID=...
LITESTREAM_SECRET_ACCESS_KEY=...
```

## Run as a systemd service

Create `/etc/systemd/system/litestream.service`:

```ini
[Unit]
Description=Litestream replication for HumanityOS relay
After=network.target humanity-relay.service
Requires=humanity-relay.service

[Service]
Type=simple
EnvironmentFile=/etc/default/litestream
ExecStart=/usr/bin/litestream replicate -config /etc/litestream.yml
Restart=always
RestartSec=5
User=root

[Install]
WantedBy=multi-user.target
```

Then:
```bash
sudo systemctl daemon-reload
sudo systemctl enable --now litestream
sudo systemctl status litestream
```

## Verify replication

```bash
litestream snapshots /opt/Humanity/data/relay.db
litestream wal /opt/Humanity/data/relay.db
```

## Restore from replica (disaster recovery)

If the VPS dies and you're starting from a fresh server:

```bash
# Stop the relay
sudo systemctl stop humanity-relay

# Restore the latest snapshot + WAL
sudo litestream restore -o /opt/Humanity/data/relay.db \
  s3://humanity-relay-replica/relay

# Verify integrity
sqlite3 /opt/Humanity/data/relay.db "PRAGMA integrity_check;"

# Restart the relay
sudo systemctl start humanity-relay
```

Recovery time objective is dominated by S3 download time — typically under
60 seconds for our DB size (currently a few MB; will grow with adoption).

## What Litestream does NOT do

- **Multi-master writes.** Only one process writes to the DB at a time. For
  multi-master, look at LiteFS or a real distributed DB. Phase 7a v2 if/when
  needed.
- **Synchronous replication.** Updates are async, typically <1 second behind.
  In a hard failure scenario, you may lose the last second of writes.
- **Replicate hot WAL during snapshot.** A small (sub-second) write blip is
  possible when a snapshot completes. Our workload tolerates this.

## Monitoring

Litestream exposes Prometheus metrics on `:9090`:
```bash
curl http://localhost:9090/metrics | grep litestream
```

Key metrics to alert on:
- `litestream_replica_validation_total{status="failed"}` increasing
- `litestream_replica_lag_seconds` rising above 30s
- `litestream_db_disk_size_bytes` growing wildly (rare)

## See also

- Plan trade-off 5: `~/.claude/plans/okay-claude-here-s-a-floating-wozniak.md`
- Litestream docs: https://litestream.io
