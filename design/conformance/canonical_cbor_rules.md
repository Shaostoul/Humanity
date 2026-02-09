# Canonical CBOR Rules

## Purpose
Specify Canonical CBOR rules used by Humanity Network objects so that:
- canonical bytes are stable
- object_id is stable
- signatures remain verifiable across implementations

This document defines canonical encoding rules at the byte level.
Implementations must follow these rules exactly.

## Base standard
Use Canonical CBOR as defined by RFC 8949 (CBOR) canonicalization rules.

## Allowed major types
Objects and payloads may use:
- unsigned integers
- negative integers
- byte strings
- text strings (UTF-8)
- arrays
- maps
- booleans
- null

Floating point numbers are prohibited in canonical objects and payloads.

## Map canonicalization
- Map keys must be unique.
- Keys must be encoded in sorted order as required by Canonical CBOR:
  - primary order: shorter key encoding length first
  - secondary order: lexicographic order of encoded bytes
- Keys must be text strings unless a schema explicitly requires bytes.

## Text strings
- UTF-8 only.
- No non-normalized equivalents may be substituted by implementations.
- Implementations must not perform automatic Unicode normalization.
  - Producers are responsible for choosing a stable representation.

## Integers
- Use the shortest encoding that represents the value.

## Byte strings
- Use definite-length encoding only.
- Indefinite-length byte strings are prohibited.

## Arrays
- Use definite-length arrays only.
- Indefinite-length arrays are prohibited.

## Maps
- Use definite-length maps only.
- Indefinite-length maps are prohibited.

## Tags
- CBOR tags are prohibited unless explicitly permitted by a schema version.
- Default: no tags.

## Object header field ordering
Objects are maps and must obey canonical CBOR ordering rules.
Do not rely on human-chosen field ordering in code.
The canonical CBOR ordering rules determine final byte ordering.

## Prohibited encodings
The following must be rejected by validators:
- non-canonical integer encodings
- indefinite-length strings/arrays/maps
- duplicate map keys
- floating point numbers
- tags (unless explicitly allowed by a future schema)

## Conformance tests
A conformance suite must include:
- canonical encoding examples
- rejection examples for each prohibited encoding type
- cross-language byte-for-byte comparisons against test vectors
