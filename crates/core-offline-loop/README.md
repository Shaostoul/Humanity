# core-offline-loop

Deterministic offline world-loop scaffold that can be driven by CLI or future renderer adapters.

## Includes

- world snapshot state
- command parser and reducer
- session mode transitions (offline/p2p/dedicated)
- fidelity preset controls
- save/load snapshot hooks (JSON in scaffold)

## Quick test

```bash
cargo test -p core-offline-loop
```

Design references:
- `design/game/offline_world_loop_scaffold.md`
- `design/game/cli_playtest_mode.md`
