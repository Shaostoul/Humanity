# For contributors

This folder is for **people building HumanityOS**: code, data, docs, or design. Whether
you are fixing a typo or wiring a new game system, start here.

## Read in order

These numbered docs are a deliberate reading sequence for getting oriented:

1. **[00-START-HERE.md](00-START-HERE.md)** orient in about 10 minutes.
2. **[01-VISION.md](01-VISION.md)** the mission and the design doctrine behind it.
3. **[02-ARCHITECTURE.md](02-ARCHITECTURE.md)** how the system is put together.
4. **[03-MODULE-MAP.md](03-MODULE-MAP.md)** what each module is for, in plain language.
5. **[04-CONTRIBUTING.md](04-CONTRIBUTING.md)** expectations and the pull-request
   checklist.
6. **[06-SOURCE-OF-TRUTH-MAP.md](06-SOURCE-OF-TRUTH-MAP.md)** which file wins when docs
   disagree, and what is real versus planned.
7. **[07-MODULE-SPEC-TEMPLATE.md](07-MODULE-SPEC-TEMPLATE.md)** the template for
   specifying a new module.
8. **[08-V1-MODULE-BACKBONE.md](08-V1-MODULE-BACKBONE.md)** the first set of systems to
   build, in priority order.
9. **[09-LIFEFORM-PARITY-FRAMEWORK.md](09-LIFEFORM-PARITY-FRAMEWORK.md)** how non-human
   lifeforms are modeled.

(AI onboarding, [05-AI-ONBOARDING.md](../ai/05-AI-ONBOARDING.md), lives in
**[../ai/](../ai/)** since it is audience-specific.)

## Working references

- **[ENGINE_REFERENCE.md](ENGINE_REFERENCE.md)** the engine's architecture, module map,
  and build commands, kept current.
- **[development_loop.md](development_loop.md)** the standard continuous-development
  cycle.
- **[validate_data.md](validate_data.md)** the data-validation gate.

## The single source of truth

When this folder and the code disagree about how the system works **now**, the code and
**[../../CLAUDE.md](../../CLAUDE.md)** win. CLAUDE.md holds the architecture overview, the
file map, the canonical cryptography table, the build commands, and the non-negotiable
design rules (GUI-first, Rust-first UI, one theme source, infinite-of-X, dual-UI
parity). Read it before quoting an algorithm or touching UI, storage, or list-shaped
data.

## Where the rest lives

- **[../design/](../design/)** system and product design specs.
- **[../network/](../network/)** federation, the relay protocol, the object format.
- **[../game/](../game/)** the educational simulation's design.
- **[../reference/](../reference/)** schemas, runbooks, templates.
- **[../../docs/PRIORITIES.md](../PRIORITIES.md)** the strict-ranked backlog (what is
  next), and **[../ROADMAP.md](../ROADMAP.md)** the strategic, themed roadmap.

## Before you push

Follow **[../SOP.md](../SOP.md)** (version bump, deploy, sync) and the
**[../SYNC.md](../SYNC.md)** pre-push checklist. For Rust changes, always check the relay
build (`cargo check --features relay --no-default-features`), not just the native build,
CI deploys with the relay feature set.
