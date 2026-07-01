# ADR-0001: Modular boundaries for Humanity ecosystem

> **Archived 2026-06-30. NEVER ADOPTED / SUPERSEDED.** This ADR proposed a multi-crate
> `core -> modules -> (game|platform) -> apps` layered workspace. The operator confirmed
> (2026-06-30) the real, current, and intended architecture is a SINGLE Rust crate at
> `src/` with feature flags (`native`/`relay`/`wasm`) — no workspace, no `crates/` directory,
> ever. Moved from `docs/design/` to `docs/history/` as a rejected architectural proposal.

- Status: Accepted (superseded — see banner above)
- Date: 2026-03-09

## Context

The project spans platform software, learning modules, and game systems. Without strict boundaries, feature work becomes coupled and hard to maintain.

## Decision

Adopt a layered architecture with dependency flow:

`core -> modules -> (game|platform) -> apps`

Core crates are UI-agnostic and reusable. Module crates compose core crates. App/game/platform crates orchestrate modules for specific runtime contexts.

## Consequences

### Positive

- Better reuse across platform and game
- Easier onboarding for humans and AIs
- Lower blast radius for feature changes

### Trade-offs

- More crate boundaries to maintain
- Requires discipline around interfaces and docs

## Follow-up

- Create module README templates
- Add first extracted module crate with tests
- Keep architecture docs synchronized with actual workspace
