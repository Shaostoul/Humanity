# 02-ARCHITECTURE

> **Reality note (v0.90.0+):** The "current workspace" described below is **historical**.
> HumanityOS is now a **single Rust crate at `src/`** (no Cargo workspace, no sub-crates).
> The former `crates/humanity-core` / `crates/humanity-relay` were folded into `src/`;
> the relay lives at `src/relay/`. Feature flags (`native`, `relay`, `wasm`) select what
> compiles. The layered multi-crate model below is an **aspirational design target that
> was not adopted**, kept for reference only.

## Current workspace (historical: pre-v0.90)

The pre-v0.90 Cargo workspace had these members (now consolidated into `src/`):

- `crates/humanity-core` → folded into `src/`
- `crates/humanity-relay` → now `src/relay/`

## Target architecture (incremental: aspirational, not adopted)

This section proposes a layered modular model that was explored but **not** built; the
project consolidated to a single crate instead.

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
