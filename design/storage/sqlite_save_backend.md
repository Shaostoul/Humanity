# SQLite Save Backend (In-Game Accessible)

## Goal
Use SQLite as the first persistence backend while keeping save management accessible from inside the game/CLI.

## Why
- transactional integrity for world data
- no dependency on external save editors
- in-game controlled save/load/event inspection

## Current scaffold

- crate: `crates/persistence-sqlite`
- snapshots table with JSON world blob + tick metadata
- append-only events table for audit/replay preparation

## In-game/CLI accessibility

CLI commands:
- `save_db <db_path> <slot>`
- `load_db <db_path> <slot>`
- `events <db_path> <slot> [limit]`

## Security direction

- closed-profile dedicated servers should keep authoritative DB server-side
- clients may view selected profile metadata via game UI, not direct DB file edits

## Next migration target
Hybrid persistence:
- SQLite snapshot/checkpoint store
- CBOR event log for deterministic replay/audit
