# Systems

This folder contains **bounded system specifications**.

A system is a coherent set of rules that governs how a specific domain of reality behaves over time under constraint.

Systems are not features.
Systems are not content.
Systems are not implementations.

Systems define behavior.

---

## Purpose of Systems

Systems exist to:
- model reality or constrained abstraction
- enforce causality and consequence
- expose tradeoffs and failure modes
- interact predictably with other systems

Every system exists to answer the question:
“What happens if…?”

---

## What Belongs in This Folder

A system document belongs here if it:

- defines rules of behavior
- operates over time
- consumes structured data
- produces observable outcomes
- interacts with other systems

Examples include:
- construction
- farming
- health
- energy
- ecology
- transport
- storage
- population

Each system must be internally coherent and externally compatible.

---

## What Does NOT Belong Here

Do not place the following in this folder:

- ethics or human principles  
  (these belong in `accord/`)

- real-world reference material  
  (these belong in `knowledge/`)

- data shape definitions  
  (these belong in `schemas/`)

- engine implementation details  
  (these belong in `engine/`)

- speculative ideas without constraints  

Systems are law, not exploration.

---

## Required Structure of a System Document

Each system document must include, at minimum:

1. **Purpose**  
   What problem this system exists to model or regulate.

2. **Scope**  
   What is included and explicitly excluded.

3. **Inputs**  
   Data consumed by the system (referencing schemas).

4. **Outputs**  
   State changes or effects produced by the system.

5. **Constraints**  
   Limits imposed by design law (time, energy, materials, realism).

6. **Failure Modes**  
   How the system degrades, breaks, or produces harm.

7. **Interactions**  
   How this system affects and is affected by other systems.

8. **Abstractions**  
   Any intentional simplifications and why they exist.

---

## System Boundaries

Systems must be:
- narrowly scoped
- composable
- predictable under repetition

When a system grows beyond a single responsibility, it must be split.

Hidden coupling between systems is a design failure.

---

## Authority and Change

System documents are authoritative.

Changes to a system must:
- preserve determinism
- document tradeoffs
- list affected systems
- respect the Humanity Accord

System evolution is expected.
Silent change is forbidden.

---

## Relationship to Schemas

Systems consume schemas.

Systems may:
- validate schema invariants
- reject invalid data
- produce new data instances

Systems may not:
- redefine schema structure
- embed schema logic in prose

Structure and behavior must remain separate.

---

## Relationship to Implementation

Systems define behavior.
Engines implement behavior.

Implementation details must not leak back into system definitions.

A system that requires a specific implementation is improperly designed.

---

## Closing Note

Systems are the engines of consequence.

Good systems make tradeoffs visible.
Bad systems hide them.

Design systems as if future humans will depend on their honesty.
