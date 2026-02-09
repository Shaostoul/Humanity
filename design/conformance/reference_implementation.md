# Reference Implementation

## Purpose
Define what qualifies as the normative reference implementation for:
- Canonical CBOR encoding
- Object hashing (BLAKE3)
- Object signing and verification (Ed25519)
- Block hashing (BLAKE3)
- Test vector generation

This document exists to prevent protocol drift and to allow multiple independent implementations to remain interoperable over long time horizons.

## Definition
The reference implementation is the smallest implementation that can:
1. Encode objects using Canonical CBOR exactly as specified.
2. Produce the canonical byte sequence used for hashing and signing.
3. Compute object_id and block_id using BLAKE3.
4. Produce and verify Ed25519 signatures on canonical bytes.
5. Generate conformance test vectors.

The reference implementation is normative for:
- canonical byte generation
- hashing inputs and outputs
- signature inputs and outputs

## Location
The reference implementation must live in the Humanity repository as source code, not as a build artifact.

Recommended path:
- `crates/humanity_network_reference/`

The reference implementation must expose a command-line interface that:
- reads a canonical JSON description of an object
- outputs:
  - canonical CBOR bytes (hex)
  - object_id (hex)
  - signature (hex) given a private key (for vector generation)
  - validation results

## Output stability
The reference implementation must:
- pin dependency versions
- include regression tests
- include a reproducible build configuration

## Compatibility policy
If the reference implementation is discovered to be wrong:
- do not silently change outputs
- add a new protocol version or add an explicit correction note with deprecation
- preserve historical test vectors and document supersession

## Non-negotiable requirements
- Deterministic output given the same inputs.
- No locale- or platform-dependent behavior.
- Explicit byte-level tests that run in continuous integration.
