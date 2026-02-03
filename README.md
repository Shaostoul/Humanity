# README.md

# Humanity

Humanity is a shared world focused on cooperation, learning, and long-term survival.

It explores how individuals and communities can live well together, scale peacefully, and build a future without domination, exploitation, or violence as default tools. Peace is treated as infrastructure. Education is treated as capability. Realism is treated as a foundation for understanding, not a limitation on imagination.

Humanity is open, revisable, and built to endure change without losing coherence.

---

## What this is

Humanity is:
- a living world
- an educational environment
- a cooperative simulation
- an experiment in civilizational design

It is not a manifesto, an ideology, or a belief system. It does not require faith, allegiance, or conformity.

---

## Repository structure

This project is structured by authority and purpose:

- `accord/`  
  Human-facing civilizational principles: how humans choose to live together, resolve conflict, and scale peacefully.

- `design/`  
  Binding technical law: architecture, constraints, systems specifications, schemas, and test philosophy.

- `data/`  
  Canonical structured truth: definitions and instance state that must validate against schemas.

- `engine/`  
  Deterministic simulation implementation (planned; may be empty during spec/data-first phases).

Other directories may contain world content, tools, and interfaces. No lower layer overrides or redefines a higher layer.

---

## Authority model (read before contributing)

This repository is organized by a strict authority stack.

From highest authority to lowest:

1. `accord/` — human-facing civilizational principles  
2. `design/` — technical constraints, system laws, schemas  
3. `data/` — concrete instances that must validate against schemas  
4. `engine/`, `tools/`, `website/`, `assets/` — implementations and presentations

Rules:

- Lower layers may not contradict higher layers.
- Presentation layers may not redefine meaning.
- Tools and interfaces may explain, render, or assist, but never override.
- If two files disagree, the higher layer is correct.

This structure exists to prevent silent drift over time.

---

## Current phase: spec-first, data-first

The repository is intentionally valid without implementation code.

- Design documents define what must be true.
- Schemas define what data must look like.
- Data slices instantiate a minimal lawful world.
- Replays define deterministic expectations.

Implementation is added only after the above chain is coherent.

---

## Implementation stance

Primary implementation language target: Rust.

`Cargo.toml` is intentionally present even if no `.rs` files exist yet. It marks the future engine/tooling entrypoint and keeps the project oriented toward deterministic, high-performance execution.

Python (or other tooling) may be used for authoring/validation utilities, but must not become an authority layer.

---

## The Humanity Accord

The ethical and civilizational principles guiding this world are defined in the Humanity Accord.

Start here:
- `accord/humanity_accord.md`

---

## Public Domain

This work is released into the public domain under the Creative Commons Zero (CC0) dedication, for the benefit of humanity—present and future.

No permission or attribution is required.

---

## Openness and revision

No generation is infallible. Understanding evolves.

This project is designed to change without collapsing, correct itself without denial, and grow without losing its core.

The future is not guaranteed.

It is constructed—by those who choose to have humanity.
