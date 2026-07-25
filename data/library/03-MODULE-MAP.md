# 03-MODULE-MAP

This file defines feature/content domain intent in plain language. "Module" here
means a coherent feature area inside the single `src/` crate (a systems module under
`src/systems/`, or a data domain under `data/`), NOT a separate Cargo crate. See
`00-START-HERE.md`'s note on this, an earlier draft of this doc described real crate
boundaries with dependency-isolation rules; that was never built and per the operator
(2026-06-30) is not the current direction.

## Shared foundations

Cross-cutting concerns most domains touch, currently implemented inline where needed
rather than as their own module, promote to a shared module if duplication becomes a
real problem:

- **Math**: units, formulas, conversions, vectors, interpolation
- **Physics**: kinematics, dynamics, trajectories, orbital calculations (`src/physics/`, rapier3d)
- **Materials**: properties, constraints, process behavior (`data/chemistry/`, `data/materials.csv`)
- **Progression**: skill levels, mastery, prerequisites (`src/systems/skills.rs`, `data/skills/skills.csv`)

## Domain areas

Each domain area, built or planned, should have:

- purpose
- inputs/outputs
- which `src/systems/` module or `data/` file it lives in (or would live in)
- a test scenario
- how it surfaces in-game and in the real-world/platform side

Combat and crafting content domains (planned/partial, not all built yet, check
`docs/STATUS.md` and `tests/engine_wiring_lint.rs::DEFERRED_SYSTEMS` for what's
actually registered and ticking before assuming any of these exist):

- **firearms**: internal/external ballistics and handling logic
- **grenades**: throw dynamics, timing/fuse behavior, blast modeling
- **orbital**: spacecraft and planetary orbital mechanics (`src/terrain/`, celestial data)
- **carpentry**: measuring, cutting/joining, structure planning (ties to the real
  Construction/Build Editor, see `docs/STATUS.md`'s Construction section)
- **welding**: weld process parameters, materials joining outcomes
- **crochet**, **stained-glass**, **pottery**: creative/craft content (`data/creative_arts.ron`)
- **swordmaking**: forging flow, geometry, material treatment sequencing (ties to the
  real ore-to-ingot-to-alloy crafting chain, `src/systems/crafting.rs`)

## Naming conventions

- Rust modules: `snake_case` (e.g. `src/systems/farming.rs`)
- Public types: `PascalCase`
- Data files: match the domain name, `kebab-case` or `snake_case` depending on
  existing sibling files in that `data/` subfolder, follow local convention.
- Keep APIs explicit; avoid hidden global state.
