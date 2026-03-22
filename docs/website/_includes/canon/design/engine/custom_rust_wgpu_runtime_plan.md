# Custom Rust + wgpu Runtime Plan (Scaffold)

## Objective
Build a custom engine runtime in Rust with wgpu, keeping simulation authoritative in shared core crates.

## Architecture split

1. `core-*` crates: deterministic simulation law (authoritative)
2. `engine-*` crates: rendering/input/audio/runtime adapters
3. `native/game` crates: gameplay composition and UX

## Initial milestones

1. first-person controller core (headless deterministic)
2. fixed-step world loop (done scaffold)
3. renderer contracts (camera, scene graph abstraction)
4. wgpu shell bootstrap (window + input + camera)
5. bridge controller/world loop into renderer tick

## Performance principles

- fixed-step simulation, variable render interpolation
- minimize per-frame allocations
- explicit data-oriented update batches
- deterministic state snapshots for multiplayer consistency

## Testing principle

All critical gameplay systems must remain CLI/headless-testable independently from renderer.
