# Snapshot+Delta Recovery Strategy (Desync-Resilient)

Goal: keep multiplayer sessions recoverable under unreliable internet with minimal hard-disconnect penalties.

## Core principles

1. Deterministic simulation where possible
2. Snapshot checkpoints at bounded intervals
3. Delta stream between snapshots
4. Rolling snapshot history for rewind/rejoin
5. Divergence detection by state hash comparison

## Recovery flow

- Client reconnects with last acknowledged snapshot tick/hash.
- Host compares against retained history.
- If exact snapshot exists: send missing deltas from that point.
- If snapshot missing but nearby exists: send nearest snapshot + deltas.
- If divergence too large: full resync snapshot.

## Failure-tolerant features

- sequence numbers and ack windows
- idempotent delta application where possible
- periodic integrity hash checks
- session resume tokens for fast rejoin

## Transport plan alignment

Phase 1:
- snapshot+delta over practical transport
- robust reconnect and recovery first

Phase 2:
- web/json control channel for session orchestration and diagnostics

Phase 3:
- optional binary high-frequency stream for scale/perf

## UX requirement

On brief disconnects, prioritize "resume in-progress session" over hard return to lobby/ship.
