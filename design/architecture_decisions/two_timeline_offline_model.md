# Two-Timeline Offline Model With Explicit Merge Rules

## Status
Accepted

## Context
Humanity OS includes both:
- offline play and learning experiences
- online shared-world experiences (MMO-like shards)
- communication and community systems used in and out of the game

Offline-first must allow continued progress without connectivity.
Online shared-world integrity requires a canonical authority for state that affects other people:
- economy
- trading
- competitive ranking
- shared world resources and permissions

If offline progress is blindly merged into online shared state, the system becomes exploitable.
If offline progress is discarded, offline-first loses value and user trust.

## Decision
Adopt a two-timeline model:

- Local timeline: always writable, offline-first, preserves user progress.
- Shard timeline (server): canonical for shared-world state.

Partition state into three buckets:
- Local-only: never merges into shard.
- Mergeable: can be proposed and accepted by deterministic rules.
- Shard-only: cannot be advanced offline; offline actions become drafts or local-only outcomes.

Synchronization uses event logs:
- clients produce signed local events
- server validates mergeable events against shard rules
- accepted events become shard events
- rejected events remain in local timeline only

The online experience is defined as the shard snapshot plus shard event history, not the local timeline.

## Consequences

### Positive
- Offline-first retains continuity without corrupting shared state.
- Shared-world integrity is preserved.
- Rollback is replaced by explicit acceptance/rejection outcomes.
- Multi-device becomes feasible through replicated event logs and snapshots.

### Negative
- Requires careful bucket classification for all game and social state.
- Adds complexity to progression design.
- Users must understand that some offline actions do not carry into online shards.

### Non-negotiable requirements created by this decision
- Every state item must be classified into Local-only, Mergeable, or Shard-only.
- Merge rules must be deterministic and testable.
- Server must never trust client ordering or timestamps for shard-authoritative effects.

## Rejected alternatives

### Full client authority with server reconciliation
Rejected due to exploitability and competitive/economic integrity failures.

### Always-server-authoritative, offline disabled
Rejected because offline-first is a core requirement and usability constraint.

### Snapshot-only sync without event logs
Rejected due to poor auditability, weak conflict handling, and brittle recovery.

## Follow-up tasks
- Define the initial bucket classifications for character progression and economy.
- Define merge rule tests and validation rules.
- Define player-facing UI rules for “local timeline” vs “shard timeline.”
