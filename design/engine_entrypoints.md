# Engine Entrypoint Constraints

## Purpose

This document defines the minimum conditions required
before implementation code may be introduced.

Code is downstream of truth.

---

## Preconditions for Code

No engine code may be written until:

1. Accord constraints are complete
2. Design constraints are internally consistent
3. Data schemas exist for all simulated domains
4. Deterministic expectations are defined
5. Failure modes are documented

Skipping these steps creates hidden authority in code.

---

## Entrypoint Definition

The engine must have a single, explicit entrypoint
that consumes:

- validated data
- validated schemas
- deterministic inputs

The engine must not invent rules.

---

## Determinism Requirement

Given the same inputs,
the engine must produce the same outputs.

Nondeterminism requires explicit declaration.

---

## Authority Boundary

The engine:
- implements constraints
- enforces invariants
- executes state transitions

The engine may not:
- redefine rules
- override canon
- encode policy implicitly

---

## Enforcement

Any implementation that violates these constraints
is invalid regardless of performance or convenience.
