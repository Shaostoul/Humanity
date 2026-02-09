# Object Format

## Purpose
Define the canonical data formats used for:
- posts, messages, threads, reactions, governance, and moderation
- offline-first storage and synchronization
- optional peer-to-peer replication
- verification and integrity checks

All shareable content is represented as immutable signed objects.
All large data (attachments) is represented as content-addressed blocks.

## Normative standards
This document is bound to:
- design/architecture_decisions/0005_canonical_encoding_and_hashing.md

Algorithms:
- Canonical encoding: Canonical CBOR
- Hash: BLAKE3
- Signatures: Ed25519

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
Objects are encoded as a CBOR map with these fields:

- protocol_version: integer
- object_type: text
- space_id: optional text
- channel_id: optional text
- author_public_key: bytes (Ed25519 public key)
- created_at: optional integer (informational only)
- references: array of text (object_id strings)
- payload_schema_version: integer
- payload_encoding: text
  - "cbor_canonical_v1" for plaintext payload
  - "xchacha20poly1305_v1" for ciphertext payload
- payload: bytes
  - plaintext canonical CBOR bytes when payload_encoding is cbor_canonical_v1
  - ciphertext bytes when payload_encoding is xchacha20poly1305_v1
- signature: bytes (Ed25519 signature)

## Canonical bytes for hashing and signing
Canonical object bytes are:
- the Canonical CBOR encoding of the entire object map, including signature field bytes.

Object identifier:
- object_id = BLAKE3(canonical_object_bytes)

## Validation rules
An object is valid only if:
1. It is canonical CBOR and decodes to a map.
2. protocol_version is supported.
3. object_type is recognized (or policy allows storage-only).
4. payload_schema_version is supported per object type rules.
5. author_public_key is well-formed.
6. signature verifies over canonical object bytes.
7. Object type payload validation passes (for plaintext payloads).
8. If payload is ciphertext, encryption framing validation passes.
9. Space and channel scoping rules are satisfied.
10. Object is not prohibited by effective signed moderation actions.

## Ordering and duplication
- Transport is at-least-once.
- Clients must deduplicate by object_id.
- Timestamps are informational and must not define authoritative ordering.

## Blocks (attachments)
- block_id = BLAKE3(block_bytes)
- In private spaces, blocks are encrypted before hashing; block_id is computed over ciphertext.

## Forward compatibility
- Unknown object_type:
  - must not affect governance, membership, or moderation
  - may be stored if policy allows, but not displayed by default
- Unknown schema versions:
  - reject for governance/membership/moderation
  - optional store-only for benign content, per policy

## Required documents
- Object type payload schemas: design/network/09_object_type_schemas.md
- Moderation payload schema: design/moderation/01_moderation_action_schema.md
- Encryption framing and AAD rules: design/security/01_encryption_and_confidentiality.md
