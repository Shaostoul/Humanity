# Object Format

> **Shipped.** This is the real, implemented wire format for the Humanity Network
> signed-objects substrate (`src/relay/core/object.rs`, `src/relay/api_v2_objects.rs`).
> `src/relay/core/object.rs` itself points back at this file as its canonical spec —
> keep them in sync. Consumers in production today: P2P groups
> ([docs/design/p2p-groups.md](../design/p2p-groups.md): `group_v1`, `group_member_v1`,
> `group_invite_v1`, `group_join_v1`, `group_disband_v1`, `group_epoch_key_v1`,
> `group_msg_v1`), governance proposals/votes (`proposal_v1`, `vote_v1`,
> `src/relay/storage/governance.rs`), and the credentials/trust graph (`vouch_v1`,
> `member_v1`, `revocation_v1`, `withdrawal_v1`, `dispute_v1`,
> `src/relay/storage/credentials.rs`). There is no separate forum/thread/post object
> family in code; do not reintroduce one here without shipping it first.

## Purpose
Define the canonical data formats used for:
- P2P groups, governance proposals/votes, and credentials/trust objects
- offline-first storage and synchronization
- peer-to-peer replication (P2P groups) and relay gossip (federation)
- verification and integrity checks

All shareable content is represented as immutable signed objects.
Large attachments (voice/file transfer) use separate mechanisms; see
[native_voice.md](native_voice.md) and [file_sharing.md](file_sharing.md).

## Normative standards
Canonical encoding is implemented in `src/relay/core/encoding.rs` (Rust) and
`web/shared/canonical-cbor.js` (JS), locked byte-identical by
`scripts/group-object-kat.mjs`.

Algorithms:
- Canonical encoding: Canonical CBOR
- Hash: BLAKE3
- Signatures: **ML-DSA-65 (Dilithium3)**, FIPS 204 (see CLAUDE.md "Cryptography"
  table). Ed25519 is not used for objects; it is retained only for the legacy
  migration bridge and the optional Solana wallet, neither of which are objects.

## Definitions
- Object: a signed, immutable record with structured payload.
- Block: raw bytes addressed by cryptographic hash.
- object_id: BLAKE3 hash of canonical object bytes.
- block_id: BLAKE3 hash of block bytes.
- Canonical bytes: Canonical CBOR encoding.

## Immutability
Objects are immutable.
Edits are new objects referencing prior objects.

## Object structure (logical fields)
Objects are encoded as a CBOR map with these fields (matches `Object` in
`src/relay/core/object.rs` exactly):

- protocol_version: integer
- object_type: text
- space_id: optional text (scoping field; used by governance proposals, not a
  general "community" object — no `space_create`/`space_policy` object types exist)
- channel_id: optional text
- author_public_key: bytes (1952-byte Dilithium3 public key)
- created_at: optional integer (informational only)
- references: array of text (object_id strings)
- payload_schema_version: integer
- payload_encoding: text
  - "cbor_canonical_v1" for plaintext payload
  - "xchacha20poly1305_v1" for ciphertext payload (object-level encryption; group
    messages instead use their own AES-256-GCM epoch-key scheme within the plaintext
    payload, see p2p-groups.md)
- payload: bytes
  - plaintext canonical CBOR bytes when payload_encoding is cbor_canonical_v1
  - ciphertext bytes when payload_encoding is xchacha20poly1305_v1
- signature: bytes (3309-byte Dilithium3 signature)

## Canonical bytes for hashing and signing
Canonical object bytes are:
- the Canonical CBOR encoding of the entire object map, including signature field bytes.

Object identifier:
- object_id = BLAKE3(canonical_object_bytes)

## Validation rules (as enforced by `put_signed_object`, `src/relay/storage/signed_objects.rs`)
An object is accepted only if:
1. It decodes to the fields above (`SignedObjectSubmission` in `src/relay/api_v2_objects.rs`).
2. `author_public_key` is exactly 1952 bytes (Dilithium3 public key length).
3. `payload` is at most `MAX_SIGNED_OBJECT_PAYLOAD` (256 KiB).
4. The Dilithium3 signature verifies over the canonical object bytes
   (signable bytes = canonical CBOR with the signature field zero-filled).
5. Per-`object_type` structural checks pass where they exist (e.g. `group_v1`/
   `group_member_v1` roster rules in `groups_p2p.rs`, `vote_v1`/`proposal_v1`
   tallying rules in `governance.rs`, revocation/withdrawal/dispute checks in
   `signed_objects.rs`).
6. Per-author submission rate limit is respected (30 objects / 60s, see
   `post_object` in `api_v2_objects.rs`).

There is no separate "moderation action" object type or space-authority-set
enforcement layer today; governance for the objects that exist (P2P groups,
governance votes, credentials) is described in
[docs/design/p2p-groups.md](../design/p2p-groups.md) and
[docs/design/signed_moderation_logs.md](../design/signed_moderation_logs.md).

## Ordering and duplication
- Transport is at-least-once.
- Clients must deduplicate by object_id (`put_signed_object` is INSERT-OR-IGNORE:
  resubmitting a known object returns `stored: false`, not an error).
- Timestamps (`created_at`) are informational and must not define authoritative ordering.

## Federation gossip
A newly-stored, locally-submitted object is gossiped to federated peers
(`gossip_signed_object`, `src/relay/handlers/federation.rs`). Objects received
from a peer are re-verified locally before storage; a relay never trusts a
peer's claim that an object is valid.

## Forward compatibility
- Unknown `object_type`: the signature and size checks above still apply; the
  object is stored (available via `GET /api/v2/objects`) even if no client
  currently renders it. There is no "strict vs permissive" mode switch in code.
- `payload_schema_version` is currently informational per object type; no
  central schema-version enforcement table exists.
