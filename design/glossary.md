# glossary.md — Canonical Terms and Meanings

This document defines the **authoritative vocabulary** of Project Universe.

Words are interfaces. If terms drift, systems drift. This glossary prevents semantic collapse for humans, AI, documentation, UI, and simulation logic.

If a term is ambiguous, it must be defined here.

---

## How to read this glossary

* **Term** — canonical name
* **Definition** — exact meaning in Project Universe
* **Notes** — clarifications, exclusions, or common misconceptions

Terms defined here must be used consistently across:

* design documents
* data schemas
* code identifiers
* UI text
* AI explanations

---

## Core simulation terms

### Simulation

**Definition:** The deterministic process that updates world state over discrete ticks based on actions and systems.
**Notes:** There is only one simulation. Rendering, UI, narrative, and AI observe it.

---

### Tick

**Definition:** A fixed-duration discrete unit of simulation time.
**Notes:** All state changes occur on ticks. Wall-clock time is irrelevant.

---

### State

**Definition:** The complete set of variables describing the world at a given tick.
**Notes:** State is authoritative only inside the simulation.

---

### Snapshot

**Definition:** A read-only view of state exposed to observers.
**Notes:** Snapshots cannot mutate state.

---

### Action

**Definition:** An immutable request to change state, validated and applied by the simulation.
**Notes:** Actions are the only way state changes.

---

## Reality and constraint terms

### Constraint

**Definition:** A non-negotiable rule derived from physical, biological, or logical reality.
**Notes:** Constraints override convenience and balance.

---

### Failure Case

**Definition:** A structured description of how and why a process can fail.
**Notes:** Failure cases are mandatory and educational.

---

### Conservation

**Definition:** The principle that matter, energy, and nutrients are neither created nor destroyed.
**Notes:** Transformations must account for losses and byproducts.

---

### Determinism

**Definition:** The property that identical inputs produce identical outputs.
**Notes:** Required for replay, multiplayer, and education.

---

## Economic terms

### Labor

**Definition:** The expenditure of time and effort by an entity to perform work.
**Notes:** Labor is finite and has opportunity cost.

---

### Scarcity

**Definition:** The condition of limited availability of resources at a given time and place.
**Notes:** Scarcity is contextual, not global.

---

### Value

**Definition:** The usefulness of a thing under specific constraints.
**Notes:** Value is not inherent and is not synonymous with price.

---

### Poverty

**Definition:** Persistent inability to meet essential needs despite effort.
**Notes:** Modeled as a systemic condition, not a stat.

---

## Education terms

### Learning

**Definition:** Measurable improvement in capability under constraint.
**Notes:** Learning requires action and feedback.

---

### Skill

**Definition:** Reduced error and waste in performing a class of actions.
**Notes:** Skills do not bypass constraints.

---

### Assessment

**Definition:** A measurement of capability based on outcomes and process.
**Notes:** Memory-only tests are invalid.

---

### Apprenticeship

**Definition:** Long-duration guided practice under variable conditions.
**Notes:** Required for mastery.

---

## Data and asset terms

### Definition (Data)

**Definition:** A canonical description of an entity or concept stored in structured data.
**Notes:** Definitions do not contain logic.

---

### Practice

**Definition:** A structured method for performing an activity correctly.
**Notes:** Practices include steps, constraints, and failure modes.

---

### Recipe

**Definition:** A deterministic transformation of inputs into outputs.
**Notes:** Recipes always declare time and requirements.

---

### Asset

**Definition:** A visual or audio representation of an entity or process.
**Notes:** Assets are never authoritative.

---

### GLB

**Definition:** A binary 3D model format used for geometry and animation.
**Notes:** GLB files may not encode mechanics.

---

## AI terms

### AI (Artificial Intelligence)

**Definition:** An optional advisory system that interprets canonical data and state.
**Notes:** AI has no authority over simulation or truth.

---

### Explanation

**Definition:** A causal account of why an outcome occurred.
**Notes:** Explanations must reference constraints and actions.

---

## Multiplayer terms

### Authority

**Definition:** The system that determines the canonical simulation outcome.
**Notes:** Authority resides in the deterministic simulation, not clients.

---

### Desync

**Definition:** A divergence between expected and actual simulation state.
**Notes:** Treated as a correctness failure.

---

## Design governance terms

### Canonical

**Definition:** Official, authoritative, and binding.
**Notes:** Canonical data and design override all others.

---

### Non-authoritative

**Definition:** Informational only; cannot affect outcomes.
**Notes:** UI, narrative, assets, and AI outputs are non-authoritative.

---

## Usage rule

If a term appears in:

* code
* data
* UI
* documentation
* AI output

and is not defined here, it must be added before use.

---

## Design intent restated

Clarity of language preserves clarity of thought.

This glossary exists so Project Universe remains intelligible, teachable, and coherent over time.
