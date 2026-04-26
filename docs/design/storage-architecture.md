# Storage Architecture

> **Last updated:** v0.115 (2026-04-25)
>
> The single source of truth for how data lives, replicates, and scales across
> client / server / game / federation. If you're integrating a new feature, read
> this first to know which layer your data belongs to.

---

## Three storage layers

### 1. Server (relay) — SQLite at `/opt/Humanity/data/relay.db`

- 38 storage modules in `src/relay/storage/`
- Single SQLite file with WAL mode (`PRAGMA journal_mode=WAL` set in `Storage::open`)
- Litestream-ready for async S3-compatible replication (see `docs/operations/litestream.md`)
- Stores both:
  - **Substrate**: the generic `signed_objects` table — every higher-level
    domain projects from this
  - **Projections**: vouches, credentials, governance, trust scores, recovery
    shares, AI status, etc. — populated automatically by side-effects on
    `put_signed_object`

### 2. Web client — browser-local

- `localStorage` for small preferences and cached display data
- `IndexedDB` for larger blobs (image cache, message history)
- Encrypted vault stored as opaque ciphertext on the relay's `vault_blobs`
  table — keys never leave the browser
- All PQ private key material derived from the BIP39 seed phrase, kept
  client-side and re-derived on each session

### 3. Native client — `%APPDATA%/HumanityOS/`

```
%APPDATA%/HumanityOS/
  identity/      — encrypted Dilithium3 keys (passphrase-locked via Argon2id)
  saves/         — full ECS world state as JSON (src/persistence.rs)
  settings/      — preferences, sync config, display state
  cache/         — offline messages, avatars, manifests
  backups/       — auto-rotated, last 5
```

A native install can also run the relay (`HumanityOS --headless`) so it has
both a client store AND a server store on the same disk.

---

## Authority model: signed objects, not server rows

The relay is **not** the source of truth. **Signed objects are.**

Every meaningful piece of state is a canonical-CBOR object signed by its
author's Dilithium3 key:

| Object type | What it represents |
|-------------|--------------------|
| `signed_profile_v1` | User's display profile |
| `vouch_v1` | One DID vouching for another |
| Various VC schemas | Verifiable Credentials (graduation, employment, role, member, account_age, skill_endorsement, …) |
| `revocation_v1` / `withdrawal_v1` | VC revocation by issuer / withdrawal by subject |
| `proposal_v1` / `vote_v1` | Governance |
| `recovery_share_v1` / `recovery_request_v1` / `recovery_approval_v1` | Social key recovery |
| `dispute_v1` | Dispute against an issuer or VC |
| `subject_class_v1` / `controlled_by_v1` / `ai_introduction_v1` | AI-as-citizen declarations |
| `key_rotation_v1` | Identity key rotation |

Servers are **caches + gossip nodes**. They store the canonical bytes,
validate the signature on insert, and auto-update derived projections.
Latest-timestamp signed object wins for any (subject, schema) pair.

**Identity isn't a server row.** It's the public key. Lose every server
that's ever cached your data, and your seed phrase still rebuilds you.

---

## End-to-end flow for one operation

Alice vouches for Bob:

```
Alice's client
  → signs vouch_v1{subject_did: did:hum:bob..., kind: "skill", ...}
    with Alice's Dilithium3 key
  → POST /api/v2/objects to her server

Alice's server
  → verifies Dilithium3 signature
  → INSERT OR IGNORE into signed_objects (idempotent on object_id)
  → auto-indexes: skill_verifications row, bumps Alice's issuer_trust good_count,
    invalidates Bob's cached trust_score so next read recomputes
  → fires gossip_signed_object → SignedObjectGossip RelayMessage to peers

Federation
  → each peer re-verifies signature, INSERT OR IGNORE
  → if newly inserted, re-gossips to ITS peers (multi-hop)
  → cycles broken by INSERT OR IGNORE dedup on object_id

Bob's server (somewhere in the mesh)
  → has the vouch in its signed_objects
  → Bob's trust score has it as a vouching_graph input

Bob's client
  → GET /api/v2/credentials?subject=did:hum:bob...
  → sees Alice's vouch + the trust sub-score it contributes
```

---

## Scaling story

| Scenario | What happens |
|---|---|
| **1 user, offline** | Native client. Identity in `%APPDATA%`, world in `saves/`. Zero network. Works on a plane. |
| **1 user, online** | Native or web client connects to ONE relay for chat + lookups. Optional Solana balance proxied through `/api/v2/solana/balance`. |
| **A few hundred users on one server** | One SQLite file handles it easily. WAL mode = concurrent reads. |
| **Many servers, many users** | Federation: each server is independent, gossips signed_objects, maintains its OWN per-issuer trust matrix from its OWN observations. No global view required. |
| **Disaster / no-internet zone** | Phase 7b LoRa transport: high-priority objects (recovery_request, vouch, vote, proposal) propagate over radio at ~50kbps. Mesh syncs to internet when it returns. |
| **Billions of users** | Sharding on `did[:2]` (256 buckets) is built-in for hot tables. Litestream replication for durability. Federation = no single bottleneck. |

---

## P2P paths (no server needed)

- **First meet via QR**: signed profile bytes encoded as QR. Both parties verify the Dilithium3 signature locally; no server in the loop.
- **WebRTC**: `web/chat/chat-p2p.js` — direct browser-to-browser data channels for chat, voice, video, screen share.
- **Recovery**: guardian sends decrypted Shamir share to holder out-of-band — typically through a friend's DM, but could be in-person, paper, or any side channel.

---

## "No home server" — what it actually means

When you connect to a NEW server you've never used before:

1. It looks up your DID (`did:hum:abc...`) — your DID resolves anywhere because the BLAKE3 fingerprint is deterministic from your pubkey
2. It pulls your signed profile via federation gossip if any peer has cached it
3. Your VCs follow you — they're signed by the issuer, not server-bound
4. Your trust score recomputes locally from the signed objects this server has observed

Different servers may show slightly different trust scores depending on what
they've seen. The inputs are always exposed (Accord transparency), so you can
always audit why a given server's score differs from another's.

Lose every server you've ever used? Boot a fresh one, log in with your
24-word seed phrase, your DID still works, and any VCs other servers cached
for you propagate back when you start interacting.

---

## Game data vs. social data

Two separate concerns living side-by-side in the same binary:

- **Social/identity/governance** — flows through the v2 substrate above. Federated, signed, replicated.
- **Game state** (ECS world, inventory, terrain, ship layouts, NPCs) — lives in:
  - `data/` next to the exe — hot-reloadable canonical content (CSV/RON/TOML/JSON)
  - `%APPDATA%/HumanityOS/saves/` — per-user world state (JSON, written by `src/persistence.rs`)
  - In-memory `hecs::World` at runtime
- The two intersect at the **inventory** and **marketplace** boundaries — your game-mode marketplace listings can carry trust scores from the social layer.

---

## Search performance at scale

Common question: *if we list 1 billion items (foods, products, anything), how
fast can we search?* Concrete numbers, all on a single mid-range VPS (4 CPU,
16GB RAM, NVMe SSD), assuming SQLite WAL mode + appropriate indexes:

| Query type | Latency at 1B rows | Notes |
|---|---|---|
| Lookup by primary key (`object_id`, `did`, item `id`) | **<1 ms** | B-tree index, O(log n). Same as 1M rows. |
| Lookup by indexed secondary column (e.g. `author_fp`, `category`) | **<5 ms** | B-tree on a smaller key |
| Range scan by indexed timestamp + LIMIT 100 | **5–20 ms** | Index seek + small forward scan |
| Full-text search via FTS5 (e.g. "find all items containing 'caffeine'") | **20–100 ms** | FTS5 inverted index; index file is ~5 GB at 1B rows |
| Multi-column filter without composite index | **seconds to minutes** | Avoid this — add the right composite index, or denormalize |
| Aggregate scan ("count all items by category") | **seconds to minutes** | Use a pre-computed materialized table updated by triggers |

### What 1 billion items actually looks like

- **Storage**: ~1KB per item row + indexes ≈ **2–3 TB** raw. Use Litestream
  to replicate to S3-compatible storage for durability.
- **Single SQLite file**: handles up to 2^63-1 rows in theory, ~140 TB max
  size. We never approach that.
- **Per-server practical ceiling**: ~100M rows on commodity hardware before
  sharding is worth the complexity.
- **Federation does the sharding**: when one server's load gets high, spin up
  another, federate, gossip the relevant subsets. Per-server load drops
  proportionally.

### Federated sharding (built-in)

For hot tables (signed_objects, credentials, items):
- Shard key: first 2 hex chars of `did` or `object_id` → 256 buckets
- Each bucket holds 1/256th of the load → at 1B items each shard handles ~4M
  rows (trivial)
- A user's data tends to land on a small number of shards (whichever DIDs
  they've interacted with), so latency stays bounded by the active set, not
  the total set.
- The relay code is shard-agnostic; the routing layer (or a future deployment
  with multiple relays + an LB) handles bucket → server mapping.

### What about "infinite" items?

There is no actual infinite. The architecture treats data as
**federated and content-addressable**:

- Every signed object is BLAKE3-hashed → globally unique address
- Lookup by hash = sub-millisecond on the server that holds it
- If the hash isn't local, federation gossip resolves via peer query
- The bottleneck becomes network latency between peers, not database scan
  time. At ~50ms RTT between peers + dedup cache, even cross-federation
  lookups are sub-second.

### AI-assisted search

The relay is just an HTTP API; an AI agent calling `GET /api/v2/objects?...`
sees the same sub-100ms latency as any client. For semantic search ("find
items similar to Oreos"), the recommended pattern is:
- Compute embeddings client-side (no need to ship them to the relay)
- Store as a small extension table with an HNSW index
- Combine with FTS5 for hybrid keyword + semantic queries

We do not currently ship semantic search; it is a Phase 9+ candidate.

## Real-world items: ingredient-list versioning

Real-world consumer items (Oreos, Coca Cola, household cleaners) **do
change** their ingredient lists over time. Oreos in 1990 ≠ Oreos in 2026.
The schema in `data/items/foods/SCHEMA.md` (v0.117.0+) accommodates this:

- **Items** carry only their identity + a sorted `history: [{as_of, note,
  ingredients}]` array
- **Ingredients** live in their own RON sidecar files with toxicology data
- **Toxicology is derived** at query time by aggregating the current
  ingredient list — never stored on the item
- Query: "what's in Oreos today?" → latest history entry. "What was in Oreos
  in 1985?" → entry with as_of ≤ 1985-01-01.
- Historical formulations stay queryable for medical exposure investigations

This separation means: improving the toxicology data for "caffeine" once
automatically improves the verdict for every item that contains caffeine,
without touching any item file.

## Related docs

- `docs/network/object_format.md` — canonical CBOR signed object format
- `docs/network/hybrid_replication.md` — what replicates P2P vs centralized
- `docs/network/server_federation.md` — federation protocol
- `docs/operations/litestream.md` — replication ops
- `docs/design/identity.md` — DID + key rotation
- `docs/design/credentials.md` — VC schema details (when written)
- `docs/design/multi-agent-development.md` — how AI agents coordinate without nuking each other's work
