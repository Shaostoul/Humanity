# core-lifeform-model

Species-agnostic lifeform modeling primitives for Humanity.

## What this crate currently provides

- `SpeciesProfile` and `SpeciesClass`
- `LifeformState` with anatomy/physiology/cognition/affect/skills
- deterministic `tick(...)` behavior via `LifeformTick`
- `capability_snapshot()` for quick task-readiness estimation

## What this crate intentionally does not include

- networking
- persistence/storage
- UI
- quest scripting

## Quick test

```bash
cargo test -p core-lifeform-model
```

## Run example scenario fixture

```bash
cargo run -p core-lifeform-model --example human_livestock_crop_stress
```

## Source design spec

- `design/modules/core-lifeform-model.md`
