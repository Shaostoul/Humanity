# terminology.md

## Purpose

This document defines technical terms used across design, schemas, engine behavior, documentation, UI, and AI explanations.

Words are interfaces. If terms drift, systems drift.

This file defines technical language. Human-facing moral language belongs in `accord/glossary.md`.

---

## Core Simulation Terms

### Simulation
The deterministic process that updates world state over discrete time steps based on validated inputs and system rules.

### Tick
A fixed-duration discrete unit of simulation time. All state changes occur on ticks.

### State
The complete set of variables describing the world at a given tick.

### Snapshot
A read-only view of state exposed to observers. Snapshots cannot mutate state.

### Action
An immutable request to change state. Actions are validated and applied by the simulation.

---

## Reality and Constraint Terms

### Constraint
A non-negotiable rule derived from physical, biological, or logical reality.

### Failure Case
A structured description of how and why a process can fail, including conditions, causes, and consequences.

### Conservation
The principle that matter, energy, and nutrients are neither created nor destroyed within the declared model.

### Determinism
The property that identical initial state + identical inputs + identical time progression produce identical outcomes.

---

## Data and Modeling Terms

### Definition (Data)
A canonical description of an entity or concept stored in structured data. Definitions do not contain behavior.

### Schema
A contract describing data shape, units, ranges, and invariants.

### Validation
The process of verifying that data meets schema and invariants.

### Recipe
A deterministic transformation of inputs into outputs that declares time, requirements, and byproducts.

---

## Asset Terms

### Asset
A visual or audio representation. Assets are non-authoritative and may not encode mechanics.

### GLB
A binary 3D model format used for geometry and animation. GLB files may not encode behavior.

---

## AI Terms

### AI
An optional advisory system that interprets canonical data/state and produces explanations or drafts. AI has no authority over simulation.

### Explanation
A causal account of why an outcome occurred, grounded in constraints, actions, and traces.

---

## Multiplayer and Sync Terms

### Authority
The system that determines canonical simulation outcomes. Authority resides in the deterministic simulation, not clients.

### Desync
A divergence between expected and actual simulation state. Treated as a correctness failure.

---

## Usage Rule

If a technical term appears in:
- design documents
- schemas
- code identifiers
- UI text
- AI output

and is not defined here, it must be added before widespread use.
