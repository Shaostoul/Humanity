# humanity-cli-game

CLI-playable harness for the current Humanity world-loop scaffold.

## Run interactive

```bash
cargo run -p humanity-cli-game
```

## Run scripted playtest

```bash
cargo run -p humanity-cli-game -- --script "status;practice water;lesson;move n;drink;farm_tick;status;quit"
```

This mode exists so AI and headless environments can play/test core gameplay loops without GUI requirements.
