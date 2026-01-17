# architecture.md

## Purpose

This document defines the technical architecture of Humanity as a **constraint-driven, inspectable, deterministic world system**.

It exists to:
- preserve authority separation (Accord → Design → Data → Execution)
- prevent hidden power and hidden logic
- keep systems composable and testable
- ensure the system remains usable on low-power hardware
- ensure understanding remains human-accessible

This document describes architecture as **law**, not as a product plan.

---

## Authority Chain

Highest to lowest authority:

1. `accord/`  
   Human principles, ethics, harm boundaries, and civilizational constraints.

2. `design/`  
   Enforceable technical law: determinism, constraints, schemas, system behavior.

3. `data/`  
   Canonical structured definitions of entities, resources, and world state.

4. `engine/`  
   Deterministic execution of design over data across time.

5. `ui/` and `tools/`  
   Presentation and authoring utilities; they do not redefine reality.

No lower layer may override a higher layer.

---

## Architectural Goals

### Determinism
Given the same initial state, inputs, and time steps, the system produces the same outcomes.

### Inspectability
Humans must be able to see:
- why something happened
- what rules applied
- what data drove the outcome

### Composability
Systems are bounded modules that interact through explicit interfaces.

### Correctness Under Constraint
The architecture prioritizes stable behavior under limited compute and without reliance on remote services.

### Non-Domination by Design
The architecture prevents hidden authority through:
- explicit scopes
- reversible states where possible
- transparent rules
- bounded interfaces

---

## Core Layers

### Accord Layer (`accord/`)
Defines human-facing principles and constraints:
- dignity, agency, consent
- non-domination
- harm minimization and repair
- epistemic integrity

This layer never contains implementation detail.

---

### Design Layer (`design/`)
Defines enforceable technical law:
- simulation laws and time rules
- realism constraints and abstraction boundaries
- schemas and invariants
- system specifications
- testing philosophy

Design documents must reference Accord constraints as the source of moral boundaries.

---

### Data Layer (`data/`)
Data is the canonical source of truth for:
- entities (humans, fauna, flora)
- resources (materials, energy, water, food)
- structures (machines, habitats, tools)
- processes (growth, decay, production)
- events (failures, transitions)

Data must be:
- versioned
- validated against schemas
- unit-consistent
- auditable

---

### Engine Layer (`engine/`)
The engine executes:
- discrete time progression
- system updates as pure transformations of state
- validation and invariant checks
- event generation and logging

The engine must:
- be deterministic by default
- record traceable causes
- degrade safely when optional components are absent

---

### Interface Layers (`ui/`, `tools/`)
Interfaces may:
- visualize state
- explain causality
- assist authoring and validation

Interfaces may not:
- invent truth
- conceal rule application
- bypass validation

---

## System Architecture

### System Definition
A system is a bounded behavioral module that:
- consumes validated data/state
- applies rules over time
- produces state changes and events

Systems live in `design/systems/` and are implemented in `engine/`.

Systems must not embed schema definitions; they reference `design/schemas/`.

---

## Schema and Validation Architecture

Schemas define:
- required fields
- types, units, ranges
- invariants and relationships

Validation occurs:
- at authoring time (tools)
- at load time (engine)
- optionally at runtime (assertions and tests)

Invalid data must be rejected or quarantined with explicit error reporting.

---

## Time and Causality

Time progression is explicit and inspectable.

Engine updates occur in ordered phases:
1. Input ingestion (human actions, external events)
2. System updates (bounded, deterministic)
3. Conflict resolution between systems (explicit rules)
4. Validation and invariant checks
5. Event logging and explanation trace

Causality must be reconstructible from logs and state.

---

## State, Events, and Logs

### State
State is the complete snapshot of the world at a time step.

### Events
Events represent notable transitions:
- failures
- thresholds crossed
- injuries
- harvests
- breakdowns
- discoveries (modeled, not mystical)

### Logs
Logs must support:
- human-readable explanations
- machine-readable traces
- reproducibility for tests

---

## Failure and Repair

Failure is expected.

Architecture must support:
- graceful degradation
- partial functionality when components are missing
- repair paths rather than irreversible dead ends where possible

Irreversible outcomes require explicit justification in system specs.

---

## Security and Boundaries

No component is permitted to silently extend its authority.

All external inputs must be:
- explicit
- validated
- logged

Any mechanism that introduces hidden control is invalid by design.

---

## Extensibility Without Drift

Extensions are allowed only when:
- schemas exist or are updated
- system specs define behavior
- tests define expected outcomes
- Accord constraints remain satisfied

The architecture is designed to grow without losing coherence.

---

## Closing Statement

Architecture is the discipline that preserves truth under growth.

Humanity’s architecture exists to keep reality understandable, power bounded, and consequence honest.
