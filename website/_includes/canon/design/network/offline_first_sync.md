# Offline-First Synchronization

## Purpose
Define how clients store, queue, sync, and verify community and messaging data under intermittent connectivity.

## Local storage
- Encrypted SQLite database for:
  - joined spaces and memberships
  - cached threads and messages
  - moderation logs
  - outbound queue
- Content-addressed block store for attachments and replicated objects.

## Data model: immutable signed objects
All shareable content is an immutable object:
- object_id is derived from a cryptographic hash of canonical bytes
- payload includes:
  - type
  - space_id (if applicable)
  - author public key
  - timestamp (informational only)
  - references to previous objects (optional)
  - signature

Edits are new objects referencing old objects.

## Outbound queue
- Offline creation places objects into an outbound queue.
- Queue items remain until server acknowledges acceptance.
- Failed items retain error state and remain local; they do not silently disappear.

## Server synchronization
- Server provides:
  - snapshots for fast bootstrap
  - append-only event feed per space/channel (or per shard)
  - merge endpoints for mergeable events

Client sync cycle:
1. Pull: fetch new server events since last cursor.
2. Verify: validate signatures and moderation eligibility before storage.
3. Apply: store objects and update local indexes.
4. Push: submit outbound queue items with signatures.
5. Acknowledge: mark accepted items committed; retain rejected items as local-only.

## Conflict rules
- Immutable objects avoid write conflicts.
- Conflicts occur only in:
  - membership state
  - moderation state
  - derived indexes
These are resolved by authoritative signed logs and declared policies.

## Privacy
- Private space payloads are encrypted.
- Objects remain signed even when encrypted (signature covers ciphertext + metadata).
