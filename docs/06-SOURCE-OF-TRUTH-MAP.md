# 06-SOURCE-OF-TRUTH-MAP

This file links **vision -> design docs -> implementation status** so new humans and AIs can quickly tell what is already real vs planned.

Last updated: 2026-05-15

## 1) Canonical source hierarchy

When files disagree, use this precedence:

1. `docs/accord/` (principles, governance, constraints)
2. `docs/design/` (architecture/system behavior and contracts)
3. `data/` (structured runtime/canonical datasets)
4. `src/`, `web/` (implementation layers — single Rust crate since the v0.90 unified-binary restructure; `server/` + `crates/` no longer exist)
5. `docs/website/` content is presentation; canonical meaning stays upstream

See also: `docs/website/README.md`.

## 2) Current implementation snapshot (reality check)

### Rust crate (single unified binary, post-v0.90)

- `src/` — one crate, all features behind flags. `src/relay/` is the
  axum relay server (was `server/`); the rest is the game engine +
  systems. Build modes: `HumanityOS` (native desktop) /
  `HumanityOS --headless` (relay). No workspace, no sub-crates.

### Other implemented runtime surfaces
- `web/chat/` (web client served by relay)
- `web/shared/` (shared front-end assets/scripts)
- `docs/website/` (public docs/presentation site)

### Important gap

Most domain systems are already documented in `docs/design/`, but many are not yet split into dedicated Rust modules under `src/`.

## 3) Design-to-implementation map

## A) Ecosystem vision and product direction

- Design sources:
  - `docs/design/product/vision.md`
  - `docs/design/product/ecosystem_architecture.md`
  - `docs/design/product/project_universe_integration.md`
  - `docs/design/product/product_roadmap.md`
- Implementation now:
  - Partially reflected in relay + web experience + desktop wrapper
- Status:
  - **Documented: strong**
  - **Implemented: partial**

## B) Identity, cryptography, signed object model

- Design sources:
  - `docs/design/architecture_decisions/client_side_identity_keys.md`
  - `docs/design/architecture_decisions/canonical_encoding_and_hashing.md`
  - `docs/design/network/object_format.md`
- Implementation now:
  - `src/relay/core/{encoding,hashing,identity,signing}.rs` (+ `pq_crypto.rs`, `did.rs`, `kdf.rs`)
- Status:
  - **Documented: strong**
  - **Implemented: strong (core foundation)**

## C) Relay/server, realtime transport, API surface

- Design sources:
  - `docs/design/network/realtime_transport.md`
  - `docs/design/network/realtime_relay_protocol.md`
  - `docs/design/network/server_federation.md`
- Implementation now:
  - `src/relay/{relay,api,mod}.rs` + `src/relay/storage/*.rs` + `src/relay/handlers/*.rs` (entry point `src/main.rs --headless`)
- Status:
  - **Documented: strong**
  - **Implemented: strong (MVP and beyond)**

## D) Systems domains (construction, farming, education loops, etc.)

- Design sources:
  - `docs/design/systems/`
  - `docs/design/core/`
  - `docs/design/gameplay/`
  - `docs/design/pages/`
- Implementation now:
  - Mostly design/spec stage
  - No dedicated Rust modules yet for most domains (all live in the single `src/` crate)
- Status:
  - **Documented: strong**
  - **Implemented: early/planned**

## E) Game/immersive integration

- Design sources:
  - `docs/design/game/`
  - `docs/design/game_integration/`
  - `docs/design/engine/`
- Implementation now:
  - `web/activities/` has web assets/pages
  - `src/` contains the Rust game engine and systems (single crate)
- Status:
  - **Documented: medium-strong**
  - **Implemented: early/planned**

## F) Desktop app distribution

- Design sources:
  - `docs/design/runtime/update_distribution_architecture.md`
- Implementation now:
  - Tauri desktop app (deprecated; native binary replaces it)
- Status:
  - **Documented: present**
  - **Implemented: deprecated**

## G) Website/docs publishing

- Design sources:
  - `docs/design/docs/md_information_architecture_plan.md`
- Implementation now:
  - `docs/website/` + mirrored/public docs strategy
- Status:
  - **Documented: strong**
  - **Implemented: strong (presentation layer)**

## 4) Where new contributors should start

1. `docs/00-START-HERE.md`
2. `docs/01-VISION.md`
3. `docs/02-ARCHITECTURE.md`
4. `docs/03-MODULE-MAP.md`
5. This file (`docs/06-SOURCE-OF-TRUTH-MAP.md`)

## 5) Next execution step (recommended)

> Updated 2026-05-15: the v0.90 unified-binary restructure eliminated
> the workspace and all sub-crates. There is no `humanity-core` crate
> and new domains must NOT be split into separate crates. Add a domain
> as a **module** inside the single `src/` crate.

Add one pilot domain module (example: `src/systems/orbital.rs` or
`src/systems/carpentry.rs`) that:

- reuses shared primitives from `src/relay/core/` (crypto/encoding) and
  the ECS in `src/ecs/` where appropriate,
- has clear inputs/outputs and `#[cfg(test)]` unit tests,
- maps directly to one existing `docs/design/systems/*` doc,
- registers with the `SystemRunner` if it ticks per-frame,
- keeps domain content data-driven (`data/*.csv|toml|ron|json`) per the
  infinite-of-X rule — no hardcoded arrays of domain objects.
