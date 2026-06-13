# Device Mesh: design

> **Status:** design (2026-05-20). The "immediate backup" stopgap is SHIPPED (`scripts/backup-relay-from-vps.ps1` + a Windows scheduled task pulling the relay DB to the operator's PC every 6h). The full feature below is TIER 2, phased, not yet built.

## Origin

Operator framing (2026-05-20), verbatim intent:

> "My chosen devices with my key are configurably backups for each other and the relay. Technically it is *my* relay so it makes sense I'd back up my stuff on my devices. I'd like to review my local devices' system info (hardware, storage capacity) and my other devices from one device. My PC is my battle station. My phone is an accessory. The VPS is my static public-facing relay with the internet that doesn't require my home PC or phone to be one."

This is sovereignty tooling: your identity key already binds your devices into a trust set; the mesh makes that set *useful*, mutual backup, unified health/inventory view, no third-party cloud. It's the kind of feature the platform should give **every** user, not just the operator. Build it generic.

## The core architectural constraint: NAT

- Home devices (PC, phone) sit behind NAT, no public IP, no inbound reachability.
- The VPS is public + always-on.
- **Therefore traffic flows device -> VPS only. The VPS never initiates to a home device.**

This isn't a limitation to work around; it's the shape of the design:

- **The VPS is the rendezvous.** Every device reports its state *up* to the VPS. Any device reads *all* devices' state *from* the VPS. That's the "review all my devices from one device" dashboard, and it works precisely because the VPS doesn't depend on any home device being online (the operator's explicit requirement).
- **Backups are pull-based from home devices.** A home device pulls what it wants to back up from the VPS when it's online. Device-to-device backup (PC <- phone) routes through the VPS as a store-and-forward relay, or happens directly over LAN when the two devices are co-located.

This maps cleanly onto the existing federation model: the VPS is the hub, devices are spokes that sync through it.

## Device roles (data-driven, per infinite-of-x)

Roles are a data file, not hardcoded enums. Suggested archetypes (operator-editable):

| Role | Example | Characteristics | Default behaviors |
|------|---------|-----------------|-------------------|
| `battle-station` | Desktop PC | Always-capable, high storage, primary workstation | Backup target for relay + all other devices; full dashboard; can trigger restores |
| `accessory` | Phone, tablet | Intermittent, limited storage, companion | Reports health; light backup target (recent data only); read-only dashboard |
| `relay` | VPS | Always-on, public, no GPU/UI, headless | Rendezvous + aggregation point; source-of-truth for live data; backup SOURCE, not target |
| `archive` | NAS, old laptop | Rarely online, huge storage | Deep-history backup target; keeps more retention than battle-station |

A device's role drives sensible defaults but every behavior is individually overridable (which devices it backs up, retention depth, what it reports).

## What exists today

- `DeviceInfo { public_key, label, registered_at, is_current, is_online }`, thin registry (`src/relay/relay.rs`).
- `device_list` / `device_label` / `device_revoke` wire messages + the link-code flow (`link_codes` table, `redeem_link_code`) to bind a new device to an identity.
- Local-first storage per CLAUDE.md: native client uses an OS data dir with a `backups/` subtree.
- **SHIPPED stopgap:** `scripts/backup-relay-from-vps.ps1` + `humanity-backup-db.sh` (VPS-side) + the Windows scheduled task. This is the 3-2-1 backup the mesh's Phase B will eventually subsume.

## Phased roadmap

### Phase A: system-info reporting + "My Devices" dashboard
The smallest standalone-useful slice.

- Add a `sysinfo` crate dependency (native feature only). Read: OS + version, CPU model + core count, total/used RAM, per-disk total/free capacity, app version, uptime, last-boot.
- New wire message `device_report { public_key, role, hostname, sysinfo_json, reported_at }`, device pushes its state to the relay on connect + every N minutes.
- Relay stores the latest report per device (new `device_reports` table, keyed by public_key, last-write-wins like signed_profiles).
- Extend `DeviceInfo` with `role`, `last_report`, `sysinfo` fields.
- New native page **"My Devices"** (`src/gui/pages/devices.rs`) + web mirror: a card per device showing role, online status, hardware, storage bars, app version, last-seen. Viewable from any device because the relay aggregates.
- Web parity: `web/pages/devices.html`.

Acceptance: from the phone, the operator sees the PC's disk capacity + the VPS's uptime. From the PC, sees the phone's battery/storage. All without either home device being reachable from the other directly.

### Phase B: backup designation + execution + encryption
Subsumes the shipped stopgap into the app.

- Per-device config: "back up [relay] [device-X] [device-Y]" with retention depth + cadence.
- Pull mechanism in the native client (replaces the external PowerShell script): when a backup-target device is online, it pulls the designated sources' latest snapshots from the VPS over the authenticated WS/REST channel.
- **Encryption-at-rest**: snapshots are encrypted with a key derived from the operator's seed before they leave the source. A stolen backup device yields nothing without the seed. (The shipped stopgap relies on BitLocker; Phase B makes it explicit + portable.)
- Backup status surfaced on the My Devices dashboard: last backup time, size, which device holds which copy, staleness warnings.
- The VPS-side `humanity-backup-db.sh` + timer stay as the relay's own local snapshot layer (defense in depth).

### Phase C: restore flow
- From any device: "restore [relay | device-X] from [backup source]".
- For the relay: the chosen backup is shipped back to the VPS, verified (schema + row sanity), staged, and applied via an attended cutover (same care as `pq-wipe.sh`).
- For a device: pull + decrypt + apply locally.
- Dry-run / verify mode that checks a backup is restorable WITHOUT applying it (turns the SECURITY-CADENCE quarterly backup-restore drill into a one-click action).

### Phase D: stretch
- Device-to-device direct sync over LAN (mDNS discovery when co-located; skips the VPS round-trip for big transfers).
- Mobile clients (Android needs a keyring backend + JNI; iOS rides the existing `keyring` crate). Mobile devices become first-class mesh members.
- Remote wipe: revoke + remote-wipe a lost device's local data (best-effort; only fires when the lost device next connects).

## Security model

- **Membership = the identity key.** A device is in your mesh iff it holds your seed (or a link-code-bound sub-device). Inc3b proof-of-possession at identify already gates this.
- **Reports are authenticated.** `device_report` is signed like every other identity-keyed message (Dilithium). The relay rejects reports for a public_key the socket hasn't proven possession of.
- **Backups are encrypted at rest** (Phase B). DMs inside them are already E2EE (Kyber); Phase B adds a wrapping layer so profiles/messages/etc. are also protected on a backup device.
- **The VPS sees metadata, not secrets.** It aggregates device reports (hardware, storage) + relays encrypted backup blobs. It cannot read backup contents (encrypted) or DM plaintext (E2EE). Consistent with the operator-can't-read-DMs posture in SECURITY-CADENCE.
- **Revocation** (`device_revoke`) drops a device from the mesh; Phase D adds best-effort remote wipe.

## Open questions for the operator (resolve before Phase B)

1. Encryption key for backups: derive from the seed (zero extra UX, but a seed compromise exposes all backups) vs. a separate backup passphrase (more secure, more friction)?
2. Retention defaults per role, how many snapshots does a battle-station keep vs. an archive device?
3. Should the dashboard show *real-time* device health (requires devices to stay connected + stream) or *last-report* (cheaper, devices report-then-disconnect)? Last-report is the cheaper default; real-time is a Phase A+ toggle.

## Relationship to the shipped stopgap

`scripts/backup-relay-from-vps.ps1` is Phase B's job done by hand outside the app. When Phase B lands, the native client does the pull internally (authenticated, encrypted, dashboard-visible) and the PowerShell script + scheduled task can be retired. Until then, the script IS the off-site backup and should be kept working (it's in-repo + version-controlled for that reason).
