# 06-SOURCE-OF-TRUTH-MAP

This file links **vision -> design docs -> implementation status** so new humans and AIs can quickly tell what is already real vs planned.

Last updated: 2026-03-09

## 1) Canonical source hierarchy

When files disagree, use this precedence:

1. `accord/` (principles, governance, constraints)
2. `design/` (architecture/system behavior and contracts)
3. `data/` (structured runtime/canonical datasets)
4. `server/`, `engine/`, `app/`, `ui/`, `docs/website/` (implementation layers)
5. `website/` content is presentation; canonical meaning stays upstream

See also: `docs/website/README.md`.

## 2) Current implementation snapshot (reality check)

### Rust workspace (currently compiled members)

- `server/` (relay server)
- `engine/` (game engine + crates)

### Other implemented runtime surfaces

- `app/` (Tauri native wrapper)
- `ui/chat/` (web client served by relay)
- `ui/shared/` (shared front-end assets/scripts)
- `docs/website/` (public docs/presentation site)

### Important gap

Most domain systems are already documented in `design/`, but many are not yet split into dedicated Rust crates/modules.

## 3) Design-to-implementation map

## A) Ecosystem vision and product direction

- Design sources:
  - `design/product/vision.md`
  - `design/product/ecosystem_architecture.md`
  - `design/product/project_universe_integration.md`
  - `design/product/product_roadmap.md`
- Implementation now:
  - Partially reflected in relay + web experience + desktop wrapper
- Status:
  - **Documented: strong**
  - **Implemented: partial**

## B) Identity, cryptography, signed object model

- Design sources:
  - `design/architecture_decisions/client_side_identity_keys.md`
  - `design/architecture_decisions/canonical_encoding_and_hashing.md`
  - `design/network/object_format.md`
- Implementation now:
  - `engine/crates/humanity-core/src/{encoding,hash,identity,object,signing}.rs`
- Status:
  - **Documented: strong**
  - **Implemented: strong (core foundation)**

## C) Relay/server, realtime transport, API surface

- Design sources:
  - `design/network/realtime_transport.md`
  - `design/network/realtime_relay_protocol.md`
  - `design/network/server_federation.md`
- Implementation now:
  - `server/src/{main,relay,api,storage}.rs`
- Status:
  - **Documented: strong**
  - **Implemented: strong (MVP and beyond)**

## D) Systems domains (construction, farming, education loops, etc.)

- Design sources:
  - `design/systems/`
  - `design/core/`
  - `design/gameplay/`
  - `design/pages/`
- Implementation now:
  - Mostly design/spec stage
  - No dedicated Rust module crates yet for most domains
- Status:
  - **Documented: strong**
  - **Implemented: early/planned**

## E) Game/immersive integration

- Design sources:
  - `design/game/`
  - `design/game_integration/`
  - `design/engine/`
- Implementation now:
  - `engine/` has game systems and crates
  - `ui/activities/` has web-based game activities
- Status:
  - **Documented: medium-strong**
  - **Implemented: early/planned**

## F) Desktop app distribution

- Design sources:
  - `design/runtime/update_distribution_architecture.md`
- Implementation now:
  - `app/` (Tauri v2 wrapper around web app)
- Status:
  - **Documented: present**
  - **Implemented: strong**

## G) Website/docs publishing

- Design sources:
  - `design/docs/md_information_architecture_plan.md`
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

Create one pilot domain crate (example: `module-orbital` or `module-carpentry`) that:

- consumes `humanity-core` where appropriate,
- has clear inputs/outputs and tests,
- maps directly to one existing `design/systems/*` doc,
- ships with a short README proving zero-context onboarding works.
