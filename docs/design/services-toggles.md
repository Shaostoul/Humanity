# Server → Services (operator feature + daemon toggles)

**Status:** BUILT v0.262.16
**Affects:** `src/relay/services.rs` (new — the privilege bridge),
`src/relay/relay.rs` (`ServiceControl`/`ServiceState` WS + handler),
`src/relay/storage/server_settings.rs` + `storage/mod.rs`
(`p2p_distribution_enabled` column + migration),
`src/gui/pages/server_settings.rs` (Services panel),
`src/gui/mod.rs` + `src/lib.rs` (`service_state` cache + WS parse),
`scripts/sudoers.d/humanity-relay-services` (new),
`.github/workflows/deploy.yml` + `Justfile` (self-install).

---

## 1. Why

Operator (2026-05-17): *"The transmission thing is part of HOS,
right? Could a toggle be added to the server? Like, just click to
disable that feature… I don't know where the limits are from our app
to needing to be OS side."*

Some HumanityOS features are backed by a **separate OS daemon**
(coturn = the WebRTC voice/video TURN relay; a future
BitTorrent-based model-distribution feature → `transmission-daemon`).
Before this, those could only be turned off via `systemctl` over SSH.
The operator wants one-click control in the native Server Settings
page, consistent with the master∧capability model already shipped
(`docs/design/roles-system.md`).

## 2. The boundary — two layers, one panel

| Layer | What | Persists? | Mechanism |
|-------|------|-----------|-----------|
| **Soft** (app) | The relay stops *offering* the feature instantly — no restart, broadcast to clients. | **Yes** (DB) | a `server_settings` bool the relay reads at runtime |
| **Hard** (OS) | Start/stop the backing daemon to reclaim RAM. | **No (v1)** — see §5 | the allowlisted privilege bridge → `sudo systemctl` |

**Effective = soft gate ON.** The hard layer is a resource
optimisation, not the feature decision. A stopped-then-rebooted
daemon is harmless because the soft gate still holds the feature off.

### coturn ↔ `voice_channels_enabled` boundary (decision)

`voice_channels_enabled` already exists as voice's **soft** gate (it
is the Voice column of the Server-master row, v0.262.6). We did **not**
add a second voice bool. The Services panel **reuses**
`voice_channels_enabled` as voice's soft gate and adds only the
**daemon control** for `coturn.service`. Rationale: a duplicate bool
would be exactly the "two detached toggles for the same thing" the
operator rejected during the roles-table consolidation. One field,
one shared `send_server_settings_update` builder, two entry points
(Server-master row + Services panel) — coherent, not duplicated logic.

For P2P there was no existing gate, so v0.262.16 adds
`server_settings.p2p_distribution_enabled` (**default OFF**). The P2P
feature itself is unbuilt; this is the plumbing + the documented
**no-op gate contract**: *any* future torrent/magnet-generation code
MUST check `server_settings.p2p_distribution_enabled` (and may consult
the `transmission-daemon` status) before doing anything.

## 3. Security model (the hard part)

The relay runs as the **non-root** user `humanity`. The bridge
(`src/relay/services.rs`) is the entire trust boundary; defence in
depth, every layer independently blocks escalation:

1. **Authorization** — the WS handler verifies the caller is
   `admin`/`owner` (`db.get_role(&my_key_for_recv)`, identical to
   `ServerSettingsUpdate`) *before* the privileged call; `control()`
   re-checks the role.
2. **Allowlist** — `ALLOWLIST` is a compile-time table. `resolve()`
   does **exact-equality** lookup of the client `service` string. The
   unit + systemctl subcommand handed onward are ALWAYS `&'static`
   constants from that table — **never** a client-derived string.
   `ServiceAction::parse` accepts only `"start"`/`"stop"`.
3. **No shell** — `std::process::Command` with an argv vector; `sudo`
   / `systemctl` exec'd directly. No `sh -c`, no interpolation →
   injection is structurally impossible.
4. **`sudo -n`** — non-interactive: a missing/wrong sudoers rule fails
   immediately instead of hanging a relay thread (fails closed).
5. **Kernel/sudoers** — `scripts/sudoers.d/humanity-relay-services`
   grants `humanity` NOPASSWD for EXACTLY four commands
   (`/usr/bin/systemctl start|stop` × `coturn.service` |
   `transmission-daemon.service`) — no wildcards, no
   enable/disable/mask. Installed only after `visudo -cf` validation,
   mode `0440 root:root`, with a fail-safe "not installed" branch
   (a malformed drop-in can break ALL sudo, so validate-first).

Status queries (`is-active`/`is-enabled`) need no privilege → no sudo.
`ServiceState` is delivered ONLY to the requesting admin (same
targeted-delivery filter as `banned_list`; never broadcast).

Audited by the `security-review` skill at v0.262.16: **no HIGH/MEDIUM
findings**. The review noted a *pre-existing, relay-wide* property
(WS session identity is client-asserted; every admin handler shares
it) — out of scope for this feature, queued separately as
"Harden relay WS session identity (signed-nonce auth)".

To add a service: one row in `ALLOWLIST` **and** the matching two
lines in the sudoers drop-in, then re-run the security review.

## 4. Protocol

- `service_control` (admin → server): `{ service, action }`.
  `action` ∈ `start` | `stop` | (anything else ⇒ refresh only).
- `service_state` (server → admin, targeted): `{ services: [{ id,
  label, soft_enabled, daemon_active, daemon_enabled }], target }`.
  Built by `services::snapshot(&server_settings)` — the soft-gate ↔
  service mapping lives there (one place).

## 5. Known v1 limitation + roadmap

- **Daemon stop is not reboot-persistent.** v1 uses `start`/`stop`
  only (minimal sudoers surface). A stopped daemon restarts on VPS
  reboot; the **soft gate still holds the feature off**, so this is a
  resource-reclaim gap, not a feature-control gap. v2 may add
  `enable`/`disable` (two more sudoers lines + allowlist) or a
  relay-startup reconcile (`if !soft_gate { stop daemon }`).
- **P2P feature unbuilt.** `p2p_distribution_enabled` is plumbing +
  the gate contract; the torrent/seed/magnet feature is a separate
  build that must honour the contract in §2.
- **Signed-session hardening** (queued, relay-wide): bind the WS
  session to a verified key so admin handlers can't be reached by
  asserting a known admin public key. Pre-existing; tracked as its
  own task.

## 6. Migration safety

`p2p_distribution_enabled` follows the `require_pq_signatures`
precedent exactly: `CREATE TABLE` default `0` for fresh DBs +
idempotent guarded `ALTER` for existing DBs, run before any SELECT of
the column. The `server_settings` seed is `INSERT … (id) VALUES (1)`
(never names columns), so the 2026-05-17 seed-names-new-column
incident class cannot occur here. Regression test
`upgrade_from_pre_p2p_server_settings_schema_does_not_panic` rewinds
the schema (drops the column), reopens `Storage`, and asserts: no
panic, column defaults OFF, **operator's pre-migration tuned values
preserved** (non-destructive).
