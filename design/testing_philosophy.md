# testing_philosophy.md

## Purpose

This document defines what must be tested, why it must be tested, and what failure means.

Testing is not primarily for “bugs.”  
Testing is for **truth preservation**.

If a system teaches something false, hides causality, violates constraints, or breaks determinism, the build must fail.

---

## Authority

Testing enforces the authority chain:

`accord/` → `design/` → `data/` → `engine/`

Testing must enforce `design/accord_constraints.md`, `design/simulation_laws.md`, and `design/realism_constraints.md`.

---

## Testing Principles

1. **Correctness outranks convenience**  
   Convenient falsehood is still falsehood.

2. **Determinism is mandatory**  
   Nondeterministic outcomes are treated as defects unless explicitly modeled and bounded.

3. **Constraints are law**  
   Any violation of realism constraints or conservation is a hard failure.

4. **Explanations must match causes**  
   If the system explains an outcome, the explanation must match the causal trace.

5. **Regression is unacceptable**  
   Once a truth is encoded and tested, it cannot silently change.

---

## Test Categories

### Unit Tests
Validate small deterministic rules.

Examples:
- unit conversions
- nutrient accounting functions
- spoilage functions

---

### Property-Based Tests
Verify invariants across wide input spaces.

Examples:
- conservation across transformations
- monotonic decay functions
- bounds on fatigue/error relationships

---

### Simulation Replay Tests
Prove determinism.

Method:
- run simulation with seed + action log
- replay
- assert identical state hashes at milestones

---

### Data Validation Tests
Ensure data is lawful.

Fail the build if:
- missing units
- invalid ranges
- unresolved references
- broken schema invariants
- impossible values

---

### Integration Tests
Verify domain interactions.

Examples:
- growth depends on soil + water + temperature
- fatigue increases error rate
- preservation trades time/energy for reduced spoilage

---

### Explanation Correctness Tests
Ensure explanations match reality.

Requirements:
- every reported failure reason maps to:
  - a constraint
  - a violated requirement
  - a defined failure case
- the system can always say:
  - what happened
  - why it happened
  - what would prevent it

---

### Performance Tests
Ensure low-power viability.

Rules:
- complexity scales ~linearly with active entities
- performance improvements must not change outcomes

---

## Always-Test Requirements

### Determinism
- identical inputs ⇒ identical outputs
- no cross-platform drift within supported configurations

### Conservation
- matter, energy, nutrients must balance within declared models

### Time Cost
- every process declares time
- no zero-time production

### Failure Cases
- every major process must fail in defined ways
- failures must be explainable

---

## Test Artifacts

### Golden Replays
Canonical seed + action logs for reference scenarios. Treated as constitutional fixtures.

Examples:
- basic week scenario
- nutrient stress scenario
- maintenance neglect scenario

### Scenario Packs
Curated small scenarios representing common real-world conditions. Used for regression and demonstration.

---

## Failure Policy

A test failure means:
- truth was violated
- determinism was broken
- constraints were bypassed
- explanations no longer match causality

Response:
- fix the defect
- or explicitly update design + data + tests together

Silent changes are forbidden.

---

## Balance Policy

Balance is evaluated only after correctness.

Rules:
- difficulty caused by reality constraints is not a defect
- ease caused by reality constraints is not a defect

Any tuning must:
- preserve constraints
- remain explainable

---

## Collaboration Stance

Open collaboration increases the need for strong tests.

Tests replace trust by making violations unmergeable.

---

## Minimal Required Suite

Must run on every change:
- schema/unit validation
- conservation properties
- replay determinism
- explanation mapping

Recommended:
- performance regressions
