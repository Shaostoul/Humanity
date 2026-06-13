# 03-MODULE-MAP

This file defines module intent in plain language.

## Shared foundations

- **Math**: units, formulas, conversions, vectors, interpolation
- **Physics**: kinematics, dynamics, trajectories, orbital calculations
- **Materials**: properties, constraints, process behavior
- **Progression**: skill levels, mastery, prerequisites, assessment

## Domain modules

Each module should include:

- purpose
- inputs/outputs
- required core crates
- standalone test scenario
- game integration hook
- platform integration hook

### module-firearms

- Covers internal/external ballistics and handling logic
- Depends on: math, physics, materials, progression

### module-grenades

- Covers throw dynamics, timing/fuse behavior, blast modeling
- Depends on: math, physics, materials, progression

### module-orbital

- Covers spacecraft and planetary orbital mechanics
- Depends on: math, physics, sim, progression

### module-carpentry

- Covers measuring, cutting/joining, structure planning
- Depends on: math, materials, progression

### module-welding

- Covers weld process parameters, materials joining outcomes
- Depends on: math, materials, progression

### module-crochet

- Covers pattern logic, counting, tension abstraction
- Depends on: math, progression

### module-stained-glass

- Covers template geometry, material fit, assembly sequencing
- Depends on: math, materials, progression

### module-pottery

- Covers shaping process states, material behavior, kiln workflow
- Depends on: materials, progression

### module-swordmaking

- Covers forging flow, geometry, material treatment sequencing
- Depends on: math, materials, progression

## Naming conventions

- Crate names: `kebab-case`
- Rust modules: `snake_case`
- Public types: `PascalCase`
- Keep APIs explicit; avoid hidden global state.
