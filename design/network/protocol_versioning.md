# Protocol Versioning

## Purpose
Define how Humanity Network evolves without breaking old data, archives, and clients.

## Definitions
- Protocol version: the version of object format, canonical encoding rules, hashing, and signature rules.
- Schema version: the version of a specific object_type payload schema.
- Compatibility: whether a client can safely parse, validate, and enforce rules for data produced by another client.

## Rules
1. Protocol version changes are rare and treated as breaking changes.
2. Schema versions may evolve more often but must remain compatible within a protocol version.
3. Objects must include:
   - protocol_version (in header)
   - object_schema_version (in payload or header, per object type)

## Compatibility levels
- Strict: rejects unknown object types and unknown schema versions.
- Permissive: stores unknown types but does not display or execute effects.
Default for safety: strict for governance and moderation; permissive optional for benign public content.

## Required behavior for unknown objects
- Unknown object_type:
  - must not affect membership, moderation, or authority
  - must not be displayed unless the user explicitly opts in
- Unknown schema version for a known type:
  - reject for moderation/governance/membership
  - store but do not display for forum/chat content, unless policy allows

## Deprecation policy
- Never change the meaning of an existing field.
- To change meaning, introduce a new field or new schema version.
- To remove a field, deprecate and ignore it; do not break canonical encoding rules.

## Version boundaries
A new protocol version is required if any of these change:
- canonical encoding rules
- hash algorithm
- signature algorithm
- object identifier computation
- encryption framing rules that alter verification semantics

## Test requirements
- Conformance vectors must exist per protocol version.
- Each new schema version must include:
  - positive validation vectors
  - negative validation vectors
