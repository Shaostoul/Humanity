# CLI Playtest Mode

CLI mode is an official testing surface, not a throwaway debug script.

## Purpose

- allow full gameplay loop testing in headless environments
- make deterministic scenario replay easy
- enable AI to execute game actions and validate outcomes

## Requirements

- command REPL and scriptable command stream
- status/inspect commands expose key world state
- save/load snapshot commands
- deterministic command processing
- mode/difficulty controls surface session orchestration behavior

## Initial command set

- `help`
- `status`
- `look`
- `move <n|s|e|w>`
- `look_dir <yaw_delta> <pitch_delta>`
- `rest`
- `drink`
- `eat`
- `gather <wood|fiber|scrap|food>`
- `craft_filter`
- `treat_water`
- `farm_tick`
- `inventory`
- `objective`
- `practice <skill>`
- `lesson`
- `set_difficulty <baby|easy|medium|hard|realistic>`
- `transition <offline|host|join|dedicated>`
- `save <path>`
- `load <path>`
- `save_db <db_path> <slot>`
- `load_db <db_path> <slot>`
- `events <db_path> <slot> [limit]`
- `quit`

## Future expansion

- construction and energy systems commands
- richer quest/economy command flows
- multiplayer simulation test commands
- deterministic replay execution from event logs
