# BitTorrent Seeder + Magnet URIs

> Status: live since v0.129.0 (2026-04-29).
> Step 4 of the [distribution-sovereignty](distribution-mirrors.md) plan.

The HumanityOS VPS now runs an always-on BitTorrent seeder for every
release bundle. Magnet URIs are published in the manifest at
[united-humanity.us/releases/manifest.json](https://united-humanity.us/releases/manifest.json).
Client devices that download a release become seeders themselves, so the
swarm scales with users, exactly the architecture
[`distribution-mirrors.md`](distribution-mirrors.md) describes.

## What's running

| Component | Where | Notes |
|-----------|-------|-------|
| `transmission-daemon` | systemd unit | Seeds every release bundle. RPC locked to `127.0.0.1:9091`. Runs as `debian-transmission`. |
| `/etc/transmission-daemon/settings.json` | config | RPC password generated at install, stored at `/root/.transmission-rpc-password` for scripts. |
| Peer port | `51413` (TCP + UDP) | Opened on ufw. |
| `/usr/local/bin/torrent-create-and-seed` | script | Creates a `.torrent` for a single file and adds it to transmission. Multi-tracker (5 public trackers) + WebSeed pointing back at the HTTP mirror. |
| `/usr/local/bin/regen-releases-manifest` | script | Two phases: (1) seed any unseeded `.tar.gz` in the release tree, (2) regenerate manifest.json with magnet URIs. Idempotent. |
| `.torrent` files | `/var/www/humanity/releases/<tag>/*.torrent` | Sibling to each `.tar.gz`. Downloadable via HTTPS like any other asset. |

## Public tracker layer

For coordination only, peer-to-peer transfer is direct between peers.
Trackers used (URL-encoded into every magnet URI):

- `udp://tracker.opentrackr.org:1337/announce`
- `udp://open.demonii.com:1337/announce`
- `udp://open.tracker.cl:1337/announce`
- `udp://tracker.openbittorrent.com:6969/announce`
- `udp://exodus.desync.com:6969/announce`

Plus DHT and PEX are enabled, so the swarm is resilient to any one
tracker going down. A self-hosted tracker (opentracker on the VPS) is a
sensible follow-up but not required: the **WebSeed** below makes the
critical path tracker-independent.

## WebSeed fallback

Every torrent created by `torrent-create-and-seed` includes a WebSeed
URL (`-w`) that points back at the file's HTTPS URL on the VPS mirror.
Result: even with **zero peers and every tracker down**, BitTorrent
clients fall back to plain HTTPS GETs from `united-humanity.us/releases/`
and the download still completes. BitTorrent becomes a P2P amplification
layer on top of HTTP, not a replacement.

## How the manifest exposes torrents

`manifest.json` lists every release version's assets. For each `.tar.gz`
that has a sibling `.torrent` on disk, the manifest entry includes a
`magnet` field with a one-click magnet URI:

```json
{
  "tag": "v0.128.3",
  "assets": [
    {
      "name": "HumanityOS-windows-x64.tar.gz",
      "size": 38625582,
      "url": "https://united-humanity.us/releases/v0.128.3/HumanityOS-windows-x64.tar.gz",
      "magnet": "magnet:?xt=urn:btih:60e9381fc949dfb10af4b69440e5bc0cf12afde0&dn=HumanityOS-windows-x64.tar.gz&tr=..."
    },
    {
      "name": "HumanityOS-windows-x64.tar.gz.torrent",
      "size": 24200,
      "url": "https://united-humanity.us/releases/v0.128.3/HumanityOS-windows-x64.tar.gz.torrent"
    },
    ...
  ]
}
```

Three ways for clients to fetch a release, in order of bandwidth-friendliness:

1. **Magnet URI**, paste into any BitTorrent client. Joins the swarm,
   pulls from peers + WebSeed simultaneously.
2. **`.torrent` file**, HTTPS-download the torrent file from the URL,
   load it in a BitTorrent client. Same effect as the magnet but works
   with clients that prefer files.
3. **Plain HTTPS**, direct download via the `url` field. Always works,
   no peer-to-peer involvement.

The auto-updater currently uses #3. A follow-up will let it pick #1
when a `magnet` field is present (BitTorrent for the bulk download,
HTTPS for the version manifest).

## Operations

| Action | Command |
|--------|---------|
| List active torrents | `ssh humanity-vps 'RPC=$(cat /root/.transmission-rpc-password); transmission-remote --auth "humanity:$RPC" -l'` |
| Seed status of one torrent | `transmission-remote --auth "humanity:$RPC" -t <id> -i` |
| Restart daemon | `ssh humanity-vps 'sudo systemctl restart transmission-daemon'` |
| Regenerate manifest + seed new releases | `ssh humanity-vps 'sudo /usr/local/bin/regen-releases-manifest'` |
| Create a torrent for one file by hand | `ssh humanity-vps 'sudo /usr/local/bin/torrent-create-and-seed <full-path>'` |
| Show magnet URI for a .torrent | `ssh humanity-vps 'transmission-show -m <torrent-path>'` |

## CI integration

No workflow change was needed for v0.129.0, `mirror-to-vps` already
calls `/usr/local/bin/regen-releases-manifest` after the scp upload,
and that script now does both seeding and manifest generation. New
releases automatically get torrents within seconds of the binaries
landing on the VPS.

## Backfill

v0.122.0 → v0.128.3 release bundles were seeded retroactively at the
v0.129.0 install, 36 torrents totalling ~1.4 GB, all the historical
`.tar.gz` files from the period before the BitTorrent layer existed.
Future releases extend this automatically.

## Layered architecture (Step 4.5, v0.130.0)

A single 38 MB whole-bundle is wasteful when most updates are 4 KB
data tweaks. The layered model ships three tiers per release:

| Tier | Asset | Size | Update cadence |
|------|-------|------|-----------------|
| 1. Whole-bundle | `HumanityOS-<platform>.tar.gz` | ~38 MB | First-install convenience |
| 2. Layered packages | `HumanityOS-<platform>.exe` (binary only) + `HumanityOS-data-<version>.tar.gz` (data + assets, no binary) | ~33 MB + ~25 MB | Day-2 updates: pull only the layer that changed |
| 3. File-level manifest | `data-manifest-<version>.json`, per-file SHA-256 hash + Forgejo per-file URL | ~100 KB | File-level delta sync, fetch only files whose hash differs |

The `data-manifest-<version>.json` lists every file under `data/`,
`assets/icons/`, and `assets/shaders/` with:

- relative path (POSIX)
- byte size
- SHA-256 hash (hex)
- per-file URL on the Forgejo source mirror (e.g.
  `https://git.united-humanity.us/shaostoul/humanity/raw/tag/v0.130.0/data/items.csv`)

A client comparing local file hashes to the manifest can sync only the
files that changed since their last update, typically a few KB instead
of 25 MB. The Forgejo source mirror serves each file individually as
the per-file CDN; no separate file-storage layer is required.

Generated by `scripts/gen-data-manifest.js` (Node, stdlib only, no
extra deps). Runs automatically in the `bundle-data` CI job and is
backfilled to `/var/www/humanity/releases/<tag>/` for v0.122.0 onward.

## Future work

- **Self-hosted tracker** (opentracker on the VPS) for full sovereignty
  on the coordination layer. Not blocking, the WebSeed makes the
  critical path tracker-independent.
- **Auto-updater integration**, let the in-app updater prefer the
  `magnet` URI over the plain HTTPS `url` when a torrent client
  capability is detected.
- **File-level delta sync in the auto-updater**, read
  `data-manifest-<version>.json`, compare to local files, fetch only the
  files whose hash differs.
- **Update preview UI**, `/download` page on the website and `/updates`
  page in the native app, showing exactly which files will change before
  the user clicks Apply.
- **Forgejo CI runner**, eventually seed via Forgejo's CI instead of
  GitHub Actions, so the seeder lifecycle is fully sovereign.
- **Torrent for Internet Archive** (Step 5), uploading each release
  to Internet Archive automatically generates an additional permanent
  free seeder. Adds resilience without extra VPS bandwidth.
- **Binary deltas** (bsdiff/courgette) so even the binary-layer updates
  shrink to kilobytes between releases.
