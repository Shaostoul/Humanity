# Offline Playable (No-Combat) Milestone

> **⚠️ Not built (verified 2026-06-30).** The `cargo run -p humanity-cli-game` invocation
> below references a crate from an experimental multi-crate layout that was never adopted;
> HumanityOS is one crate (`src/`) building one binary (`HumanityOS`). See
> `docs/game/cli_playtest_mode.md` for the same correction. This doc's actual gameplay-loop
> milestone (gather/craft/purify/farm/save-load, offline-first) has separately shipped for
> real in the native GUI, not via this CLI, per `docs/history/` v0.323-0.342 (the
> engine-wiring + gameplay-loop arcs). Treat this file as a historical description of an
> abandoned test-harness approach, not a live pass/fail gate.

This milestone defines "ready for human test" before combat implementation.

## Required gameplay loop

1. Move/look in first-person shell or CLI
2. Gather resources (`wood`, `fiber`, `scrap`, `food`)
3. Craft filter kit
4. Purify water
5. Consume food and water
6. Advance farming cycle (`farm_tick`)
7. Track objective progress to 3/3
8. Save/load world from SQLite in-game commands

## Pass criteria

- loop can be completed without external tools
- save/load recovery works through in-game/CLI commands
- no hard crashes on normal command paths
- deterministic command behavior in scripted replay

## Suggested scripted validation

```bash
cargo run -p humanity-cli-game -- --script "status;gather wood;gather fiber;gather scrap;craft_filter;treat_water;gather food;eat;farm_tick;objective;save_db savegame.db default;load_db savegame.db default;status;quit"
```
