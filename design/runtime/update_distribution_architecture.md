# Update & Distribution Architecture (GitHub-Independent)

## Goals
- Support hot reload for as many layers as safely possible.
- Keep app usable offline with bundled baseline content.
- Decouple release/update delivery from GitHub.
- Support multi-format distribution (desktop installers, portable builds, web content bundles).

---

## Update Layers

### Layer A — Hot Reload (no restart)
- markdown/docs bundles
- UI config, labels, themes
- feature flags
- non-critical content data packs

### Layer B — Warm Reload (module reload)
- plugin bundles
- renderer/layout modules
- optional integration adapters

### Layer C — Full Restart
- core runtime binaries
- security-critical components
- protocol/runtime migrations

---

## Distribution Channels

- **stable**
- **beta**
- **dev**

Each channel has separate manifests and retention policies.

---

## Source of Truth (Self-Hosted)

Primary endpoints (example):
- `https://updates.united-humanity.us/appcast/<channel>.json`
- `https://updates.united-humanity.us/packages/...`
- `https://updates.united-humanity.us/content/...`

Mirrors (optional):
- secondary object storage/CDN
- GitHub Releases (fallback only, not required)

---

## Manifest Model

```json
{
  "channel": "stable",
  "version": "1.2.3",
  "releasedAt": "2026-03-06T00:00:00Z",
  "binary": {
    "windows": { "url": "...", "sha256": "...", "sig": "..." },
    "macos":   { "url": "...", "sha256": "...", "sig": "..." },
    "linux":   { "url": "...", "sha256": "...", "sig": "..." }
  },
  "contentBundles": [
    { "name": "docs-core", "version": "2026.03.06", "url": "...", "sha256": "...", "hotReload": true }
  ],
  "minRuntime": "1.2.0",
  "rollout": { "percent": 100 }
}
```

---

## Security

- Sign manifests and packages.
- Verify signature + hash before apply/install.
- Reject downgrade unless explicitly authorized.
- Keep local rollback cache of previous known-good package.

---

## Offline-First Behavior

- Installer ships with baseline docs/content bundle.
- App runs fully in baseline mode offline.
- On reconnect: async check for updates and download diffs.
- Apply policy:
  - content hot reload immediately when safe,
  - binary updates on next restart (or user-approved).

---

## Async Update Flow

1. Startup: load local baseline.
2. Background: fetch manifest with timeout/fallback.
3. Compare versions + compatibility.
4. Download changed bundles/packages.
5. Verify signatures/hashes.
6. Apply hot-reload bundles immediately.
7. Queue binary update for restart.

---

## Packaging Targets

- Desktop installer (MSI/EXE, DMG/PKG, AppImage/DEB/RPM)
- Portable desktop zip/tar
- Web static bundle
- Optional content-only bundle packs

---

## Operations Requirements

- Release promotion pipeline: dev -> beta -> stable
- Health checks for update endpoints
- Canary rollout support (percentage-based)
- Emergency rollback switch in manifest

---

## Immediate Next Steps

1. Implement update manifest schema in code.
2. Add baseline content bundling into desktop build.
3. Add content bundle hot-reload manager.
4. Add signed package verification.
5. Switch app updater endpoints to self-hosted domain.
