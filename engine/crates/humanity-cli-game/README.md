# humanity-cli-game

CLI-playable harness for the current Humanity world-loop scaffold.

## Run interactive

```bash
cargo run -p humanity-cli-game
```

## Run scripted playtest

```bash
cargo run -p humanity-cli-game -- --script "status;gather wood;gather fiber;gather scrap;craft_filter;treat_water;gather food;eat;farm_tick;objective;status;quit"
```

DB commands (in CLI):

```bash
help_db
save_db savegame.db default
load_db savegame.db default
events savegame.db default 20
```

This mode exists so AI and headless environments can play/test core gameplay loops without GUI requirements.
