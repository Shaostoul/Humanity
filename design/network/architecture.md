# data_model.md

## Purpose

This document defines how reality is represented as structured data.

Data is the **canonical description of state**.  
Design defines behavior.  
The engine executes behavior over data.

Code may interpret data.  
Code may not redefine data.

This document exists to prevent ambiguity, hidden authority, and silent corruption.

---

## Authority Relationship

The data model is constrained by:

- the Humanity Accord
- design law (`design/`)
- accord constraints (`design/accord_constraints.md`)

Data does not encode ethics or behavior.  
It encodes **facts, properties, quantities, and relationships**.

---

## Foundational Principle

If a concept cannot be represented as data, it cannot reliably exist in the system.

If a concept is represented as data, it must be:
- explicit
- inspectable
- validated
- versioned

There is no hidden state.

---

## What Data Is

Data represents:

- entities
- resources
- structures
- processes
- conditions
- events

Data describes **what exists**, not **what happens**.

---

## What Data Is Not

Data does not represent:

- intent
- ethics
- rules
- decisions
- authority
- narrative

Those belong to higher layers.

---

## Data Categories

### Entities

Entities are discrete, identifiable actors or objects.

Examples:
- humans
- animals
- plants
- machines
- tools

Entity data includes:
- identifiers
- physical properties
- capacities and limits
- current state

---

### Resources

Resources are consumable or transformable quantities.

Examples:
- water
- food
- materials
- energy
- time

Resource data includes:
- units
- availability
- constraints
- regeneration or depletion characteristics

Resources are finite unless explicitly modeled otherwise.

---

### Structures

Structures are persistent assemblies of entities and resources.

Examples:
- shelters
- machines
- infrastructure
- habitats

Structure data includes:
- components
- capacities
- maintenance requirements
- failure thresholds

---

### Processes

Processes describe potential transformations.

Examples:
- growth
- decay
- production
- repair

Processes are defined as **data describing possibility**, not execution.

Execution belongs to systems.

---

### Conditions

Conditions describe environmental or systemic context.

Examples:
- temperature
- pressure
- contamination
- damage states

Conditions influence system behavior but do not cause action directly.

---

### Events

Events record notable state transitions.

Examples:
- breakdowns
- injuries
- harvests
- depletion
- recovery

Events must be:
- timestamped
- attributable
- traceable to causes

---

## Units and Invariants

All quantitative data must specify:
- units
- valid ranges
- invariants

Unit mismatch is an error condition.

Invariants must be enforced through validation.

---

## Versioning and Evolution

Data schemas are versioned.

Changes to data structure must:
- be explicit
- preserve backward compatibility where possible
- include migration paths

Silent schema drift is forbidden.

---

## Validation

Validation occurs at multiple stages:
- authoring
- loading
- runtime (optional assertions)

Invalid data must:
- be rejected or quarantined
- produce explicit error messages
- never be silently corrected

---

## Transparency and Traceability

Every data element must be:
- human-readable
- machine-parseable
- inspectable through tools

Data changes must be attributable to:
- system execution
- validated input
- authorized modification

---

## Failure and Degradation

Data may represent failure states.

Failure must be:
- explicit
- explainable
- recoverable where possible

Irreversible failure requires justification at the system level.

---

## Security and Authority Boundaries

No system or tool may:
- inject hidden data
- modify data outside validation
- infer unrepresented state as fact

All authority over data mutation must be explicit and logged.

---

## Relationship to Knowledge

Knowledge informs data modeling.

Data is not raw knowledge.

Knowledge is:
- interpretive
- contextual
- revisable

Data is:
- explicit
- structured
- authoritative within the system

---

## Closing Statement

Data is the ground truth the system stands on.

When data is explicit, reality remains understandable.

When data is corrupted or hidden, power becomes invisible.

The data model exists to keep reality honest.
