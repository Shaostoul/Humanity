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

## Related docs

- `docs/network/object_format.md` — canonical CBOR signed object format
- `docs/network/hybrid_replication.md` — what replicates P2P vs centralized
- `docs/network/server_federation.md` — federation protocol
- `docs/operations/litestream.md` — replication ops
- `docs/design/identity.md` — DID + key rotation
- `docs/design/credentials.md` — VC schema details (when written)
- `docs/design/multi-agent-development.md` — how AI agents coordinate without nuking each other's work
