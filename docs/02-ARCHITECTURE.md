# 02-ARCHITECTURE

## Current workspace (today)

Current Cargo workspace members:

- `crates/humanity-core`
- `crates/humanity-relay`

## Target architecture (incremental)

We are moving toward a layered modular model.

### Layer A: Apps (orchestration)

Examples:

- `apps/humanity-app` (end-user experience)
- `apps/game-client` (game UX and runtime)
- `apps/admin-tools` (ops/moderation tooling)

### Layer B: Core domain crates (reusable logic)

Examples:

- `core-math`
- `core-physics`
- `core-materials`
- `core-sim`
- `core-progression`

These crates should not depend on app or UI crates.

### Layer C: Module crates (feature domains)

Examples:

- `module-firearms`
- `module-grenades`
- `module-orbital`
- `module-carpentry`
- `module-welding`
- `module-crochet`
- `module-stained-glass`
- `module-pottery`
- `module-swordmaking`

Each module depends on core crates as needed.

### Layer D: Game composition crates

Examples:

- `game-rules`
- `game-story`
- `game-economy`

These compose module crates for game-specific loops.

### Layer E: Platform/integration crates

Examples:

- `platform-web`
- `platform-storage`
- `platform-ssh`

These bridge domain logic with deployment and runtime systems.

## Dependency direction (strict)

Allowed direction:

`core -> modules -> (game|platform) -> apps`

Avoid reverse dependencies. If a lower layer needs higher-layer behavior, use traits/interfaces and inversion.

## Implementation strategy

1. Keep existing crates stable.
2. Extract shared logic into narrowly scoped crates.
3. Add module crates one by one with tests.
4. Wire game and platform composition after module APIs settle.
5. Update docs + ADR on each boundary decision.
