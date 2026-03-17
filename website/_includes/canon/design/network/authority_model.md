# Authority Model

## Purpose
Define which state is authoritative locally versus on a server-shard, and how offline-first interacts with shared online worlds.

## Definitions
- Local timeline: client-owned event log and snapshots.
- Shard timeline: server-owned event log and snapshots for a specific shared world (MMO-like shard).
- Merge: server acceptance of a subset of local events into shard timeline.

## State buckets

### Bucket A: Local-only
- Never merges into shard.
- Examples: private notes, offline-only story branches, local sandbox builds.

### Bucket B: Mergeable
- May merge under deterministic server rules.
- Examples: non-competitive achievements, lore discovery, cosmetic unlocks approved by rules.

### Bucket C: Shard-only
- Cannot be advanced offline for shard authority.
- Examples: currency, trading, competitive rank, shared permissions, guild roles, shared resource extraction.

## Requirements
- Every state item must be classified into A, B, or C.
- Server must not trust client timestamps for shard ordering.
- Server assigns authoritative ordering for shard-visible effects.

## Behavior on connect
1. Client fetches shard snapshot and shard log head.
2. Client proposes mergeable local events since last accepted shard head.
3. Server validates and accepts/rejects deterministically.
4. Accepted events become shard events; rejected remain local-only outcomes.
5. Client renders shard state as the online truth while preserving local timeline separately.

## Rollback meaning
“Rollback” is not deletion of local progress.
It is the choice to display shard-authoritative state for online participation.
Local progress remains preserved in local timeline.

## Multi-device
- Local timeline may be backed up and restored between devices (encrypted).
- Shard timeline is obtained from server snapshots and shard logs.
