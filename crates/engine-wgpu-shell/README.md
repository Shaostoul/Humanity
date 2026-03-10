# engine-wgpu-shell

Custom Rust + wgpu runtime shell scaffold (first-person-style prototype, non-combat).

## Run

```bash
cargo run -p engine-wgpu-shell
```

## Movement / look

- `W/A/S/D` or arrow keys: move
- `Mouse`: look/turn camera
- `Shift`: sprint

## Gameplay actions

- `1`: gather wood
- `2`: gather fiber
- `3`: gather scrap
- `4`: craft filter kit
- `5`: treat water
- `6`: farm tick
- `7`: eat food ration

## Menu / info

- `Esc`: toggle in-game menu overlay
- `I`: inventory summary
- `O`: objective summary
- `H` (while menu open): menu help

Window title is used as current HUD/status text in this scaffold build.
