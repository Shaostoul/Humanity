# Design

This folder defines the **authoritative technical law** of Humanity.

Where the Humanity Accord defines human principles, this folder defines how those principles are enforced through structure, constraint, and determinism.

Documents here are written for builders, maintainers, and systems.
They are not aspirational. They are binding.

---

## How to Use This Folder

- If you are deciding **what must exist**, read the root design documents.
- If you are defining **how a system behaves**, read `systems/`.
- If you are defining **what shape data must take**, read `schemas/`.

No system, data file, or implementation may contradict the documents listed here.

---

## Canonical Design Documents

The following documents constitute the **minimum required design corpus**.
All are expected to exist and remain consistent.

### Foundational Design Law (root)

These documents apply to *all* systems.

- **DESIGN.md**  
  High-level design intent and non-negotiable principles.

- **architecture.md**  
  Module boundaries, authority separation, and structural layout.

- **simulation_laws.md**  
  Determinism, causality, time progression, and conservation rules.

- **realism_constraints.md**  
  Reality-first constraints and acceptable abstraction limits.

- **data_model.md**  
  Rules governing how reality is represented as structured data.

- **economy_model.md**  
  Material, energy, labor, and time flow constraints.

- **education_model.md**  
  How learning is represented and validated at a system level.

- **ai_interface.md**  
  AI authority limits, access rules, and failure handling.

- **asset_rules.md**  
  Constraints on models, textures, audio, and other assets.

- **testing_philosophy.md**  
  What correctness means and how it is verified.

---

### System Specifications (`systems/`)

Each file here defines a **bounded system**.
Systems must obey all foundational design law.

Expected systems include (not exhaustive):

- **construction.md**
- **farming.md**
- **health.md**
- **energy.md**
- **storage.md**
- **transport.md**
- **ecology.md**
- **population.md**

Each system document should define:
- purpose
- inputs and outputs
- constraints
- failure modes
- interactions with other systems

---

### Data Schemas (`schemas/`)

Schemas define **data shape contracts** consumed by systems.

Expected schema categories include:

- **entities** (people, animals, plants)
- **resources** (materials, energy, food)
- **structures** (buildings, machines)
- **processes** (growth, decay, production)
- **events** (failures, transitions)

Schemas define structure, units, and invariants.
They do not define behavior.

---

## Relationship to Other Folders

- `accord/` defines human principles and ethics
- `design/` defines enforceable system law
- `knowledge/` provides real-world reference material
- `data/` contains canonical structured truth
- `engine/` executes design deterministically

Design translates meaning into machinery.

---

## Status and Evolution

This folder is expected to grow, but not drift.

When adding a new design document, ask:
- Does this define global law or a specific system?
- Does it introduce new constraints?
- Does it conflict with existing documents?

Unnecessary design is worse than missing design.

---

## Closing Note

Good design makes failure visible and abuse difficult.

Design is not about enabling everything.
It is about preventing the wrong things from becoming easy.
