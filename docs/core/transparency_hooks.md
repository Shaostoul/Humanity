# Transparency Design Hooks

## Purpose

This document defines mandatory design requirements that enforce
the Transparency Guarantees established by the Humanity Accord.

These hooks exist to ensure that transparency is not optional,
configurable, or dependent on implementation choices.

Any system that cannot satisfy these hooks
must not be considered compliant with Humanity.

---

## Design Authority

This document is binding on all system designs.

No implementation, optimization, interface, or tool
may override or bypass these requirements.

If a design conflicts with a transparency hook,
the design is invalid.

---

## Core Design Principle

Transparency must be structurally unavoidable.

A system must be incapable of operating
without exposing its state, actions, and reasoning.

---

## Mandatory System Surfaces

All systems must provide the following observable surfaces:

### 1. State Surface

The system must expose:
- its current operational state
- relevant internal variables
- active modes or configurations

State visibility must:
- be continuous
- reflect real state, not summaries
- update in real time

Hidden state is forbidden.

---

### 2. Action Surface

The system must expose:
- actions already taken
- actions currently in progress
- actions scheduled or queued

Actions must be visible:
- before execution
- during execution
- after execution

Post-hoc visibility alone is insufficient.

---

### 3. Reasoning Surface

The system must expose:
- why an action was chosen
- what inputs contributed
- what rules or models were applied
- what alternatives were considered, if applicable

This surface must distinguish:
- fact from inference
- rule from assumption
- certainty from estimation

---

### 4. Uncertainty Surface

The system must expose:
- confidence levels
- known unknowns
- assumptions
- approximation bounds

Uncertainty must never be suppressed
to appear authoritative or decisive.

False certainty is a transparency violation.

---

## Persistence and Traceability

### 5. Persistent Records

Systems must produce records that:
- exist by default
- are tamper-evident
- persist long enough for meaningful review

Records must link:
- inputs → decisions → actions → outcomes

Loss of records constitutes a system failure.

---

### 6. Real-Time Auditability

Auditability must be possible:
- during operation
- without halting the system
- without special access

Transparency that exists only after failure
is considered insufficient.

---

## Prohibited Design Patterns

The following patterns are explicitly forbidden:

- silent background automation
- hidden default behaviors
- non-inspectable inference pipelines
- “expert-only” transparency
- configuration flags that disable transparency
- performance optimizations that remove observability
- abstraction layers that obscure causality

If a design requires opacity to function,
the design must be rejected.

---

## Failure and Degradation Rules

### 7. Transparency Under Failure

When a system degrades or fails:
- transparency must persist
- state must remain visible
- failure must be explicit, not silent

If transparency cannot be maintained,
the system must halt safely.

---

## Human Legibility Requirement

Transparency must be:
- readable by non-experts at a high level
- explorable in detail when desired

This does not require simplification of truth.
It requires layered presentation.

---

## Validation Criteria

A design satisfies transparency hooks only if:

- transparency is present without configuration
- transparency cannot be disabled
- transparency survives failure
- transparency is visible before harm occurs

Passing tests without satisfying these criteria
does not constitute compliance.

---

## Relationship to Other Documents

- Transparency Guarantees define the civilizational requirement.
- Safety and Responsibility define harm constraints.
- Consent and Control define legitimacy of action.
- UI Invariants define how transparency is presented to humans.

This document binds those principles into system design.

---

## Summary

Transparency must not be added.
It must be unavoidable.

If a system can act without being seen,
it cannot be trusted.

Design accordingly.
