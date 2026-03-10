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
- `treat_water`
- `farm_tick`
- `practice <skill>`
- `lesson`
- `set_difficulty <baby|easy|medium|hard|realistic>`
- `transition <offline|host|join|dedicated>`
- `save <path>`
- `load <path>`
- `quit`

## Future expansion

- inventory/crafting commands
- construction and energy systems commands
- multiplayer simulation test commands
- deterministic replay execution from event logs
