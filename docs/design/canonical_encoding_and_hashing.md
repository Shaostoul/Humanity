# Canonical Encoding and Hashing Standards

## Status
Accepted

## Context
Humanity Network uses immutable signed objects and content-addressed blocks.
Object identifiers and block identifiers must be stable across:
- platforms
- programming languages
- time

If serialization or hashing changes, all identifiers change, breaking:
- synchronization
- deduplication
- replication
- verification
- long-term archives

The system requires a single canonical byte representation for:
- hashing
- signing
- verification

## Decision
Adopt the following fixed standards:

- Canonical encoding for objects: Canonical CBOR
- Hash function for object and block identifiers: BLAKE3
- Signature algorithm for object signatures: Ed25519

Object identifier:
- object_id = BLAKE3(canonical_object_bytes)

Block identifier:
- block_id = BLAKE3(block_bytes)

The canonical bytes for an object are defined as:
- the CBOR-encoded map with a fixed field order and canonical CBOR rules
- the "signature" field is included as bytes
- the canonical encoding rules are the only allowed representation for hashing and signing

No alternative encodings are permitted for object identifiers.

## Consequences
### Positive
- Fast hashing and verification.
- Stable identifiers across languages and time.
- Deterministic canonicalization reduces ambiguity.

### Negative
- Requires strict canonical CBOR implementation and test vectors.
- Any client must implement canonical CBOR exactly.

### Non-negotiable requirements created by this decision
- Provide canonical CBOR test vectors for every object type.
- Provide cross-language conformance tests for hashing and signing.
- Never change these standards; superseding requires a new versioned protocol.

## Rejected alternatives
### JSON-based canonicalization
Rejected due to ambiguity risk, whitespace/ordering pitfalls, and poor binary efficiency.

### Protocol Buffers deterministic encoding
Rejected due to unknown-field behavior, schema evolution complexity, and language divergence risk.

### SHA-256 instead of BLAKE3
Rejected due to performance and chunking advantages of BLAKE3, with no required compatibility constraint.

## Follow-up tasks
- Add a conformance folder with test vectors.
- Update Object Format document to remove "Open selections."
