# Library: federated file/media catalog + curated directories

> **Status:** design (2026-05-26). Operator-driven design conversation. The
> **Library** is HumanityOS's single "free public access to resources" home: a
> federated catalog of files hosted *on* the network (art, 3D models, media)
> plus the curated directories that point *out* (open-source software, web
> links). One nav entry; tabbed by kind. Replaces/absorbs the separate **Tools**
> and **Browser**/**Resources** pages.

## Why this exists

The web chat already lets people attach files, and the relay already has an
asset-library backend (`assets.rs`) + an upload path (`uploads.rs`). But there's
no **browsable, searchable, federated catalog** of shared files, and no way to
host the things that are "cool but not required", concept art, `.stl`/`.blend`
3D models, reference media, without unbounded disk growth. The Library adds
that, and in doing so unifies three overlapping "resources" surfaces into one.

Seed content: the 187 media files (~264 MB of 2020-era concept art/renders)
archived from the retired Project Universe site (`docs/history/project-universe-site/`,
staged in gitignored `_pu-archive/media/`) are the Library's first upload.

## Information architecture: one page, tabbed by consume-mode

The organizing principle that keeps a merged page un-confusing is **how you
consume the thing**:

| Tab | What it is | Interaction | Source today |
|-----|-----------|-------------|--------------|
| **Files** | Content hosted *on the federation* | download **in**, upload, pin | NEW (this doc) |
| **Software** | Curated open-source apps | opens the project's site | `tools.rs` / `data/tools/catalog.json` |
| **Web** | Curated websites + knowledge | opens browser (in-app browser later) | `browser.rs` + `resources.rs` |

That *hosted-here vs. pointer-out* line is the tab boundary, so each tab gets
the UI that fits it. A real library maps cleanly: media you borrow/take **here**,
plus a reference desk pointing **elsewhere**.

**Consolidation:** `tools.rs` → **Software** tab; `browser.rs` (curated link
list) + `resources.rs` → **Web** tab. Those pages retire into the Library
(phased, see below). The eventual in-app **browser engine** stays a separate
capability that the Web tab's links *launch into*; the Library is the directory,
not the renderer. `files.rs` (the `data/`-dir text editor, dev-tier) is
unrelated and stays as-is.

**Provenance labelling (trust clarity):** the Files tab is *user-uploaded +
federated*; Software/Web are *curated by HumanityOS*. Every Files item shows its
**source server**; curated items show "curated." Trust levels never blur.

## The Files engine: bounded disk by construction

A **trust-tiered LRU cache (ephemeral) + a curated permanent tier (pins)**. This
bounds disk *physically* (the 91 GB cascade can't recur) while creating an
incentive gradient: *verify → more space; make something worth keeping → it gets
pinned forever.*

### Trust-tier quotas

Tiers map onto the existing role/trust system (`roles.rs`, `trust_score.rs`,
`credentials.rs`). Each tier has a storage budget, configurable per server admin
as **GB or % of disk** (% auto-scales as the disk grows; GB is predictable).

| Tier | Pool model | Rationale |
|------|-----------|-----------|
| **Unverified** | *Shared* tier pool, LRU, **+ per-user sub-cap** (no one unverified user holds > ~10–20% of the pool) | No per-user guarantee *is* the incentive to verify. A Sybil spammer churns only this tier, never the disk. |
| **Verified** | *Per-user* quota (earned guarantee), own LRU when exceeded | They've earned a guarantee. |
| **Mod / Admin / custom roles** | Larger per-user quota; `pin_files` permission | Data-driven roles (infinite-of-X). |

### LRU eviction

When a pool is full and a new upload arrives, evict the **oldest** file in that
pool. For the shared unverified pool, **evict the heaviest user's oldest first**
, a flooder evicts *themselves*, not their neighbours (anti-grief). Combine with
the existing per-key rate limiting.

### Pin → permanent → durable

- A `pin_files`-roled user (admin/mod/custom) pins a file → it becomes
  **permanent** (exempt from LRU) **and is subtracted from the poster's quota**
  (refund, incentive to post pin-worthy things).
- **Unpin** → returns to the ephemeral pool with a fresh timestamp, re-counts
  against the owner.
- Permanent still costs relay disk, so "infinite pins" is bounded by routing
  permanent items to the **existing BitTorrent seeder** (`torrent-create-and-seed`,
  `docs/admin/torrent-infrastructure.md`) + eventually **Internet Archive** (a free
  permanent seeder, already on the torrent roadmap). Permanent tier = P2P/IA-backed,
  not just relay disk. *This is what makes "infinite" actually hold.*

### Per-file size cap

Per tier, so a 300 MB `.blend` can't evict 3,000 pictures from the unverified
pool. Large files require verified+ (or a separate large-file budget).

## File identity & dedup: content, never name

Files are identified by a **SHA-256 of their bytes** (`content_hash` column),
not their filename (rename `cat.png` → `dog.png`, the hash is unchanged). The
hash does triple duty: dedup key, store-once mechanism, and a stable content
address (the good part of IPFS without running IPFS).

Two notions of "same":

1. **Byte-identical** → same SHA-256. Provably identical regardless of name.
   **Auto-link** the new name to the one stored blob; no decision needed (at
   most a quiet "you already have this" FYI). No preview dialog, they're the
   same bytes.
2. **Looks-the-same, different bytes** (re-encoded / resized / cropped /
   screenshotted) → different SHA-256. Caught by a **perceptual hash (pHash)**
   for *images*, which yields a *similarity score*. Because it's probabilistic,
   this is where the **preview-confirmation dialog** fires:

   > "This looks ~94% like a file already here, [new | existing, side by side]
   >, **Same** (use existing) / **Different** (keep both)."

   The human decides; near-dupes are never silently merged.

**Honest caveat:** perceptual matching is an *image* technique. For 3D models
(`.stl`/`.blend`/`.obj`) and other binaries, the realistic mechanism is
exact-hash only, a re-exported model with different bytes reads as new.
Mesh-geometry fingerprinting is a later enhancement, not v1. So: **images get
exact + perceptual + the dialog; everything else gets exact-hash dedup.**

## Storage & transport: the layered model you already run

Storage (bytes) and Catalog (searchable index) are **separate concerns**. The
catalog is lightweight metadata; bytes are fetched per-file from wherever the
record points.

- **Bytes:** relay disk → HTTPS (nginx), layered with the existing torrent
  seeder. `torrent-create-and-seed` adds a **WebSeed** pointing at the HTTPS
  URL, so BitTorrent is a P2P *amplification layer on top of HTTP*, cold files
  fall back to plain HTTPS (always works), popular/large files get P2P offload.
  Auto-torrent files above a threshold (e.g. ≥ 25 MB).
- **Deferred:** object storage (S3/R2/B2), infinite scale but **costs money**
  (against the budget) + external dependency; revisit only under disk pressure
  (R2's zero egress would be the pick). IPFS, elegant content-addressing but
  run-a-node complexity; the torrent layer already delivers ~the same benefit.

## Federation: catalog aggregation, not byte-shipping

**Rule: ephemeral content is server-local; only pinned/permanent content
federates.** (Otherwise the global search fills with entries that vanish on
eviction.) The federated Library aggregates **lightweight metadata** across
`/api/federation/servers`, filename, type, tags, size, `content_hash`, `url`,
`magnet`, source server, and the Files tab **groups results by source server**
(like the left-rail server grouping). Bytes are fetched per-file via
`url`/`magnet`. This is how "infinite files × users × servers" stays tractable:
the index is cheap, the bytes are pulled on demand.

## Web vs native

Both platforms get the Library page (dual-UI parity; both have Tools/Browser
today). Difference in the Files tab:
- **Native** manages a real local files dir → a true **"On this device" vs
  "Available"** split.
- **Web** can't see the OS downloads folder → "downloaded" is best-effort
  (record what you grabbed + mark "yours/uploaded by you"; can't verify it's on
  disk). Shape the web view around *"Available + Yours."*

## Schema / code touchpoints (extend, don't rebuild)

| Concern | Existing | Add |
|---------|----------|-----|
| Catalog | `assets.rs` (`get_assets` w/ category/type/tags/text search) | columns: `content_hash`, `perceptual_hash`, `magnet`, `pinned`, `permanent`, `source_server`, `tier`; quota accounting |
| Upload | `uploads.rs`, `/api/upload` (10 MB cap) | tiered size caps; hash-on-upload; dedup check + dialog handshake |
| Tiers | `roles.rs`, `trust_score.rs`, `credentials.rs` | `pin_files` permission; per-tier quota config |
| Pins | `pins.rs` (message pins) | file pins → permanent + quota refund + torrent/IA handoff |
| Config | `server_settings.rs` | per-tier GB/%; per-file caps; auto-torrent threshold (GUI-first) |
| Distribution | `docs/admin/torrent-infrastructure.md` seeder | per-file `torrent-create-and-seed` on pin/large-file |

## Phased build plan

1. **Files engine (relay):** schema, hash-on-upload + exact dedup, tiered quotas
   + LRU eviction, per-file caps. The hard part.
2. **Library page → Files tab (web first, then native):** browse/search,
   group-by-source, upload, "Available vs Yours/On-device."
3. **Pin → permanent → torrent** handoff; admin pin UI; quota refund.
4. **Perceptual hash + preview-confirmation dialog** (images).
5. **Federation aggregation** (pinned-only) + per-source grouping across peers.
6. **Consolidate:** `tools.rs` → Software tab, `browser.rs`+`resources.rs` → Web
   tab; retire those nav entries (web + native).
7. **Internet Archive** permanent-seeder handoff; object-storage path only if
   disk pressure forces it.

## Moderation / abuse

Ephemeral content auto-expires (good, limits exposure window). Need: active
moderation, hard-delete, **hash-ban** (block re-upload of known-bad
`content_hash`), and pin-as-trust-action (only roled users make content
permanent + federated, exactly where human judgment belongs).

## Guardrails

- **Infinite-of-X:** tiers, quotas, categories, curated lists are all data, 
  no hardcoded arrays.
- **GUI-first:** every quota/cap/threshold/role-permission is a server-admin
  setting in `server_settings`, rendered in-app (and AI-enumerable).
- **Theme tokens only** in the Library UI (web + native).
- **Don't regress** Tools/Browser/Resources content when folding them in, 
  same data files, new home.
