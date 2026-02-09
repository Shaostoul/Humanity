# Conformance Test Vectors

## Purpose
Provide byte-level test vectors that all implementations must pass.
These vectors prevent protocol drift and ensure long-term interoperability.

## What must be tested
Every implementation must be able to:
- produce canonical CBOR bytes for an object
- compute object_id using BLAKE3
- verify Ed25519 signatures on objects
- compute block_id using BLAKE3
- reject non-canonical encodings that would hash differently

## Rules
- Test vectors are normative.
- If a vector is wrong, add a corrected vector and mark the old one as deprecated, do not delete history.
- Any protocol change requires a new protocol version boundary and a new vector set.

## Files
- object_hash_and_signature_vectors.md
  - canonical CBOR examples
  - object_id expected values
  - signature expected values
  - negative cases

- block_hash_vectors.md
  - raw bytes examples
  - block_id expected values
