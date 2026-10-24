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
A deterministic transformation of inputs into outputs that declares time, requirements, and byproducts. A how-to that makes something names its recipe in its `makes` block (2026-10-23; the recipe moves into the how-to's file in increment 4 of docs/design/how-to.md).

---

## How-to Terms (2026-10-23, docs/design/how-to.md)

### How-to
One procedure as data (`data/how_to/<top category>/<id>.json`, schemas/how_to.toml): its purpose, who it is for, what you need, its steps, how to tell it worked, what goes wrong, who can help, and its sources with dates. On screen "How-to" (the page), "a how-to", "how-tos"; in code `how_to`.

### Step
One thing to do in a how-to, with a stable id: a tick, a timer, a check (Yes or No), a value to write down, a choice, another how-to, a page to open, a command. A run's state is keyed by the step's id.

### Run
One time a person does a how-to: when it started, which steps are done, what they wrote down, how it ended. Kept on their own device only.

### Repeat
A person's choice to do a how-to every so often (every 90 days); the Calendar shows when it is next due (increment 3).

### Variant
A how-to adapted to a place or a kind of equipment, stating only what differs from the how-to it adapts (increment 7).

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
