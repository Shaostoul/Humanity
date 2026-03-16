# Offline Playable (No-Combat) Milestone

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
