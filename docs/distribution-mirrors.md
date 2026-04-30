# Distribution Mirrors and Sovereignty

> **Goal:** decouple HumanityOS from any single host. The project's source
> code, release binaries, and installable packages should survive the loss
> of any one platform — including GitHub. This matches the federation
> philosophy already baked into the relay.

The strategy has three layers, ordered from most-aligned with HumanityOS's
P2P design to least:

1. **Sovereignty layer** — you own and run it. No vendor can take it down.
2. **Community layer** — non-profit or mission-aligned hosts that mirror
   the work and add discoverability without acting as a single point of
   failure.
3. **Distribution layer** — package managers and app stores that get
   HumanityOS in front of users wherever they live.

Sites the project should keep using or adopt are listed below. **If a
platform isn't listed, it isn't recommended.** GitLab, SourceForge,
Bitbucket, NotABug, Chocolatey, and similar were considered and dropped
because they don't add anything HumanityOS doesn't already get from the
recommended set.

---

## Cost summary

| # | Platform | Layer | Signup | Cost |
|---|----------|-------|--------|------|
| 1 | [Forgejo / Gitea](#1-forgejo--gitea--self-hosted-git-on-the-vps) | Sovereignty | No (self-hosted) | **$0** (VPS already paid) |
| 2 | [Self-hosted BitTorrent tracker + seeder](#2-self-hosted-bittorrent-tracker--seeder) | Sovereignty | No (self-hosted) | **$0** (VPS bandwidth) |
| 3 | [Self-hosted IPFS node](#3-self-hosted-ipfs-node) | Sovereignty | No (self-hosted) | **$0** (VPS storage) |
| 4 | [Codeberg](#4-codeberg) | Community | Yes | **$0** forever |
| 5 | [Software Heritage](#5-software-heritage) | Community | No (passive) | **$0** forever |
| 6 | [Internet Archive](#6-internet-archive) | Community | Yes (free account) | **$0** forever |
| 7 | [Radicle](#7-radicle) | Sovereignty (P2P) | Generate identity (no website) | **$0** forever |
| 8 | [GitHub](#8-github-keep-it-but-stop-relying-on-it) | Community | Already done | **$0** for OSS |
| 9 | [F-Droid](#9-f-droid-android) | Distribution | Yes | **$0** forever |
| 10 | [Flathub](#10-flathub-linux) | Distribution | Yes | **$0** forever |
| 11 | [Snap Store](#11-snap-store-linux) | Distribution | Yes | **$0** forever |
| 12 | [WinGet](#12-winget-windows) | Distribution | Yes (Microsoft account) | **$0** forever |
| 13 | [Homebrew](#13-homebrew-macos) | Distribution | No (PR to repo) | **$0** forever |
| 14 | [Pinata (IPFS pinning)](#14-pinata-ipfs-pinning) | Sovereignty backstop | Yes | Free 1 GB; **$20/mo** for 50 GB |
| 15 | [web3.storage / Filecoin](#15-web3storage--filecoin) | Sovereignty backstop | Yes | Free 5 GB; pay-per-use beyond |

---

## 1. Forgejo / Gitea — self-hosted git on the VPS

[forgejo.org](https://forgejo.org) · [gitea.io](https://gitea.io) ·
**Cost: $0** (no signup; runs on your existing VPS)

A single Go binary, ~50 MB, drops next to the relay on
`server1.shaostoul.com` and serves a GitHub-like UI at
`git.united-humanity.us`. Free software, AGPL, community-governed
(Forgejo is the community-controlled fork after Gitea's 2022 governance
shift).

**Why it fits.** This is the cleanest match for the project's ethos. You
already pay for the VPS, you already have nginx in front of it, the relay
already runs there. Adding Forgejo gives you a primary source-of-truth
remote that no third party can take down. `just ship` becomes
multi-remote — push to GitHub for visibility, push to your own Forgejo
for sovereignty. If GitHub disappears tomorrow, your repo, issues, and CI
configuration are intact.

Forgejo also supports **ForgeFed** (the federation protocol for git
forges, ActivityPub-based). Once that's production-grade, your Forgejo
instance can federate with other operators' Forgejo instances — exactly
the same shape as the HumanityOS relay federation. That's the
philosophically right end state.

---

## 2. Self-hosted BitTorrent tracker + seeder

[opentracker](https://erdgeist.org/arts/software/opentracker/) ·
[transmission-daemon](https://transmissionbt.com) ·
**Cost: $0** (VPS bandwidth only)

Lightweight BitTorrent tracker (opentracker, written in C, single
binary) plus a torrent seeder (transmission-daemon or libtorrent-based).
Generate `.torrent` and `magnet:` URIs for each release; the VPS is the
always-on initial seeder; client devices become seeders too once they've
downloaded.

**Why it fits.** This is exactly the architecture you described — exe is
the small entry point, on first run it pulls `data/` and `assets/` via
the swarm with the VPS as guaranteed seed. Bandwidth scales horizontally
with users; the VPS's bandwidth budget caps download speed only when the
swarm is small. The torrent file's info-hash is a content-addressable
checksum, so binaries can't be silently swapped. Magnet links can be
included on the website and in onboarding without depending on any
external service.

Pair with [WebTorrent](https://webtorrent.io) (BitTorrent over WebRTC)
to let browser users join the swarm without a native client.

---

## 3. Self-hosted IPFS node

[ipfs.tech](https://ipfs.tech) · [Kubo (IPFS daemon)](https://github.com/ipfs/kubo) ·
**Cost: $0** (VPS storage)

Run an IPFS node on the VPS, pin every release. Each release becomes a
content-addressable hash (CID). Anyone with the CID can fetch the
release from any IPFS node that has it pinned — yours, a community
member's, a public gateway, or a paid pinning service.

**Why it fits.** Complementary to BitTorrent. Where torrents are great
for big-file swarms, IPFS is great for granular file addressing — you
can pin individual data files (`data/items.csv`, `data/recipes.csv`,
etc.) and clients can fetch only what they need. CIDs go in
`server-config.json`; clients can resolve them via HTTP gateways
(`https://ipfs.io/ipfs/<cid>`) without an IPFS client, or speak IPFS
natively if available.

Most natural fit for federating *signed-object payloads* across HOS
relays: the `signed_objects` substrate already content-addresses by
BLAKE3 hash; IPFS is a built-out CID-based distribution layer with the
same model.

---

## 4. Codeberg

[codeberg.org](https://codeberg.org) · **Cost: $0** forever

Non-profit (Codeberg e.V., Berlin), runs Forgejo, ~150k repos, no ads,
no tracking, EU jurisdiction. Has integrated CI via Woodpecker. Stable,
mature, organisationally accountable to its members rather than to
shareholders.

**Why it fits.** The mission match is obvious: Codeberg is what GitHub
*should* be — community-run, non-commercial, mission-aligned. It's the
strongest external mirror you can pick. EU jurisdiction adds robustness:
even if a US legal action targeted GitHub's hosting of HumanityOS,
Codeberg would be unaffected. Setting up a mirror is one signup + one
remote add. Use Woodpecker CI as a backup pipeline that runs the same
build the GitHub Actions workflow runs.

---

## 5. Software Heritage

[softwareheritage.org](https://softwareheritage.org) · **Cost: $0** forever

Non-profit (UNESCO partnership) running the universal source-code
archive. They harvest from GitHub, GitLab, Codeberg, etc. continuously.
Stored content gets a permanent SWHID identifier that survives every
host on Earth disappearing.

**Why it fits.** Passive insurance. You don't have to do daily work —
they harvest from any git host they're already crawling. You can also
trigger an explicit save with the [Save Code Now](https://archive.softwareheritage.org/save/)
form once you set up Forgejo or Codeberg. **No active failure mode** —
this is the layer that says "even if every other host vanishes, the
source survives."

---

## 6. Internet Archive

[archive.org](https://archive.org) · **Cost: $0** forever

The big public archive. Accepts arbitrary uploads. Critically, it
**generates a torrent automatically** for every uploaded item and acts
as a permanent seeder. So uploading each release to Internet Archive
gives you a free, durable BitTorrent seed that supplements your own
VPS-hosted seeder.

**Why it fits.** Reinforces the torrent-distribution layer (#2). If your
VPS is briefly down, the Internet Archive's seeder keeps the swarm
alive. If the VPS is permanently lost, the binaries are still
downloadable. Also useful for archiving non-code artifacts: docs PDFs,
release notes, talks, screenshots. Bonus: anything uploaded gets indexed
by the Wayback Machine.

---

## 7. Radicle

[radicle.xyz](https://radicle.xyz) · **Cost: $0** forever

Peer-to-peer git over a custom gossip protocol. No central server. You
run a Radicle node, push your repo to it, and it replicates to other
nodes that follow you. Has issue tracking, patches (PR equivalent), and
a CLI. Identity is a self-generated keypair — same shape as your HOS
identity.

**Why it fits.** Philosophically the closest match to HumanityOS's
federation model: signed objects gossip between peers, no central
authority, identity is a public key. The mental model maps directly onto
the existing PQ substrate. The ecosystem is small but maturing; treat
this as a **pilot** for now (run a node, mirror the repo, watch how it
develops). Long-term, Radicle is what HumanityOS's source distribution
*should* look like once the ecosystem catches up.

---

## 8. GitHub — keep it, but stop relying on it

[github.com](https://github.com) · **Cost: $0** for OSS · already done

The largest discoverability channel for open-source code. Most
contributors will find HumanityOS through GitHub first.

**Why it stays.** Visibility, contributor convenience, mature CI/CD
infrastructure, low effort to maintain. The project doesn't *replace*
GitHub — it adds layers underneath so a GitHub takedown becomes a
nuisance rather than an existential threat. GitHub is the front door,
not the foundation.

---

## 9. F-Droid (Android)

[f-droid.org](https://f-droid.org) · **Cost: $0** forever

Community-run repository of free and open-source Android apps. Builds
each app from source on F-Droid's own infrastructure, so you publish by
submitting metadata, not a binary. Strict on FOSS-only dependencies.

**Why it fits.** When the HOS mobile app exists, F-Droid is the
philosophy-aligned default for Android distribution. No Google Play
account needed (and no Google Play fees), no proprietary blob in the
build chain, signed by F-Droid's reproducible-build pipeline. This is
the most natural Android channel for a mission-driven open project.

---

## 10. Flathub (Linux)

[flathub.org](https://flathub.org) · **Cost: $0** forever

Cross-distro Linux app store using Flatpak (sandboxed, runtime-shared).
The dominant way Linux desktop users install apps in 2025+.

**Why it fits.** Once the desktop client is polished, Flatpak is the
distribution that reaches the most Linux users with the least
distro-specific work. One submission covers Ubuntu, Fedora, Arch,
Debian, openSUSE, etc.

---

## 11. Snap Store (Linux)

[snapcraft.io](https://snapcraft.io) · **Cost: $0** forever

Canonical's app store for Linux. Snaps are confined and bundle their
runtime; default on Ubuntu and broadly available elsewhere.

**Why it fits.** Reaches Ubuntu users who don't have Flatpak installed
(still a sizable group). Covers a different audience than Flathub
without replacing it. Submission is fairly low-effort once you have a
working build.

---

## 12. WinGet (Windows)

[github.com/microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs) ·
**Cost: $0** forever (Microsoft account for the PR)

Microsoft's Windows package manager, ships with Windows 10/11. Adding
HumanityOS means a single PR with a YAML manifest pointing at the
release `.exe`. Users then install with `winget install HumanityOS`.

**Why it fits.** Lowest-friction Windows distribution path. No publisher
fees, no certificate-signing requirement, no review queue beyond a
manifest validation. Reaches every Windows 11 user out of the box.
Caveat: the manifest still points at GitHub Releases by default —
update it to point at your VPS release mirror when that's live.

---

## 13. Homebrew (macOS)

[brew.sh](https://brew.sh) · **Cost: $0** forever (no account; PR to a repo)

The de facto macOS package manager. Add HumanityOS as a "cask" (binary
distribution) by opening a PR to `homebrew-cask` with a Ruby formula
pointing at the release binary.

**Why it fits.** Same shape as WinGet: a PR-based distribution channel
with no fees and broad reach. Mac users who use Homebrew will install
HumanityOS with `brew install --cask humanityos`. Caveat: Homebrew casks
require a stable URL — point it at your VPS release mirror so the link
doesn't depend on GitHub.

---

## 14. Pinata (IPFS pinning)

[pinata.cloud](https://pinata.cloud) · **Cost: free 1 GB / $20/mo for 50 GB**

Managed IPFS pinning service. If your self-hosted IPFS node (#3) is
unavailable, Pinata's pin keeps your content discoverable on the IPFS
network. Free tier is small; paid tier at $20/month covers a lot of
release archive.

**Why it fits.** Backstop for the IPFS layer. Only worth subscribing if
you're committing to IPFS as a primary distribution channel and the
self-hosted node alone isn't sufficient redundancy. Defer until usage
demands it.

---

## 15. web3.storage / Filecoin

[web3.storage](https://web3.storage) · **Cost: free 5 GB / pay-per-use beyond**

Filecoin-backed storage. Generous free tier (5 GB), pay-per-use beyond.
Decentralized in a different way from IPFS-pinning: the data lives in
the Filecoin storage network with cryptographic proofs of retrievability.

**Why it fits.** Alternative IPFS pinning backstop with a generous free
tier and a decentralized backing layer. Lower friction than Pinata for
small-scale usage.

---

## Adoption order (recommended)

The path that gets the most decoupling per unit of effort:

1. **Self-hosted Forgejo on the VPS** ✅ shipped v0.127.0 — see
   [`forgejo-setup.md`](forgejo-setup.md).
2. **Codeberg mirror** — non-profit external mirror, signup +
   `git remote add`. ~30 min.
3. **VPS release mirror** ✅ shipped v0.128.0 — live at
   <https://united-humanity.us/releases/>. Backfilled v0.122.0–v0.127.0;
   CI mirrors every future tagged release via `appleboy/scp-action`,
   and the manifest at `/releases/manifest.json` is regenerated
   automatically by `/usr/local/bin/regen-releases-manifest` on the VPS.
   `latest` is a symlink to the newest `vX.Y.Z` directory. The
   auto-updater (BUG-034 fix in v0.124.0) will gain this as a fallback
   URL in a follow-up — for now it's a working mirror that anyone can
   `wget` from independently of GitHub.
4. **BitTorrent seeder + magnet URIs** ✅ shipped v0.129.0 — see
   [`torrent-infrastructure.md`](torrent-infrastructure.md). Live on the
   VPS via transmission-daemon, seeding 36 release bundles
   (v0.122.0–v0.128.3). Magnet URIs in `/releases/manifest.json`. Public
   trackers (opentrackr, demonii, etc.) for coordination + WebSeed back
   at `united-humanity.us/releases/` so the critical path is
   tracker-independent. Self-hosted tracker is a follow-up (not blocking
   because of the WebSeed). Auto-updater integration follows.
5. **Internet Archive uploads** — manual at first; automated later. Adds
   a permanent free seeder to every torrent.
6. **Software Heritage Save Code Now** — one form, then it's automatic
   forever. ~10 min.
7. **WinGet manifest PR** — distribution to every Windows user. ~1 hour.
8. **Self-hosted IPFS node** — when there's a use case for granular
   content-addressed assets. ~half a day.
9. **F-Droid / Flathub / Snap / Homebrew submissions** — once the
   desktop and mobile clients are stable enough to commit to the
   release cadence. Each is ~1 hour of metadata work.
10. **Radicle node** — pilot. Mirror the repo, evaluate the workflow.
    Track the ecosystem; revisit when ForgeFed lands in Forgejo.
11. **Pinata / web3.storage** — only if IPFS becomes a primary channel
    and the self-hosted node alone isn't enough redundancy.

Steps 1–4 are the meaningful sovereignty work. Steps 5–7 are
low-effort, high-resilience additions. Steps 8+ are scope-driven —
pick them up as the project grows.
