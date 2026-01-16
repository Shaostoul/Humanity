# DESIGN.md — Project Universe Design Authority

This directory contains the **authoritative design corpus** for Project Universe.

Design is law. Code is an implementation detail.

If implementation conflicts with design, implementation is wrong.

---

## Purpose of this directory

The `design/` directory exists to:

* Declare **what must be true** about Project Universe
* Prevent architectural drift over time
* Provide a stable source of truth for:

  * human contributors
  * automated tooling
  * low-power AI systems
  * future maintainers

This directory is intentionally:

* human-readable
* machine-ingestible
* stable
* version-controlled

---

## Authority hierarchy

From highest to lowest authority:

1. **philosophy/** (intent and ethics)
2. **design/** (constraints and structure)
3. **data/** (canonical facts)
4. **engine/** (universal mechanics)
5. **domain systems** (world, life, society, industry, etc.)
6. **ui / narrative / assets** (representation only)

Nothing below may violate anything above.

---

## Design corpus overview

Each document in this directory exists to prevent a specific class of failure.

### game_design.md

* Defines *what exists* in the game and *why*
* Maps real-world domains to game systems
* Explains repository structure and responsibilities

### architecture.md

* Defines system boundaries and allowed dependencies
* Establishes deterministic simulation and action pipeline
* Prevents logic leakage into UI, narrative, or assets

### data_model.md

* Defines canonical schemas, units, and invariants
* Establishes data as the source of truth
* Enables AI reasoning and educational traceability

### realism_constraints.md

* Declares non-negotiable physical, biological, and labor limits
* Highest enforcement authority

### simulation_laws.md

* Defines tick order, causality, determinism, replay
* Required for multiplayer correctness and verification

### education_model.md

* Formalizes learning, mastery, decay, and assessment
* Prevents grind-based or fictional progression

### economy_model.md

* Models scarcity, labor value, distribution, and failure
* Encodes poverty as a solvable systemic problem

### ai_interface.md

* Defines how AI may read and explain the game
* Explicitly forbids AI authority over truth

### modding_contract.md

* Defines how mods may extend the game
* Prevents corruption of canonical reality

### asset_rules.md

* Declares assets as representation only
* Prevents mechanics from hiding in models or textures

### testing_philosophy.md

* Defines what must be tested and why
* Enforces educational correctness

### glossary.md

* Defines shared language for humans and machines
* Prevents semantic drift

---

## Rules for contributors and tools

* No gameplay system may be added without a corresponding design reference.
* No data schema may be changed without updating `data_model.md`.
* No mechanic may violate `realism_constraints.md`.
* UI, narrative, and assets may not encode authoritative rules.
* Mods must comply with `modding_contract.md`.

Automated checks are expected to enforce these rules.

---

## Stability and change

Design documents are intentionally slow to change.

* Changes require justification in terms of reality, education, or determinism.
* Refactors must preserve meaning.
* Version history must remain intelligible.

---

## Final statement

Project Universe is not a game about abstraction or power fantasy.

It is a simulation intended to:

* teach reality
* preserve knowledge
* enable cooperation
* scale from a single human to a united civilization

This directory exists to ensure that purpose is never lost.
