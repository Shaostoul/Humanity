# engine-wgpu-shell

Custom Rust + wgpu runtime shell scaffold (first-person-style prototype, non-combat).

## Run

```bash
cargo run -p engine-wgpu-shell
```

## Movement / look

- `W/A/S/D` or arrow keys: move
- `Mouse`: look
- `Shift`: sprint

## Gameplay actions

- `E`: gather wood
- `Q`: gather fiber
- `Z`: gather scrap
- `R`: craft filter kit
- `T`: treat water
- `F`: farm tick
- `C`: eat food ration

## Menu / info

- `Esc`: toggle in-game menu overlay
- `I`: inventory summary
- `O`: objective summary
- `H` (while menu open): menu help

Window title is used as current HUD/status text in this scaffold build.
