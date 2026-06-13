# 07-MODULE-SPEC-TEMPLATE

Use this template for every gameplay/learning module.

> Goal: any human or AI can implement/extend a module with minimal ambiguity.

---

## 1) Module identity

- **Module name:**
- **Crate/package id:**
- **Domain:** (foundation | physical | life | homestead | trade | economy | governance | teaching | narrative)
- **Status:** (draft | active | stable)
- **Owners:**

## 2) Purpose

- What real-world domain this module simulates/teaches
- Why it matters for self-sustainability and civilization resilience

## 3) Scope

### In-scope
- 

### Out-of-scope
- 

## 4) Inputs / outputs

### Inputs
- data schemas used
- required upstream modules
- user actions/events

### Outputs
- state changes
- teachable events
- metrics and assessments

## 5) Core simulation model

- State variables
- Time-step behavior
- Constraints/limits
- Failure modes
- Determinism requirements

## 6) Lifeform parity requirements

For each supported lifeform type, specify whether this module models:

- Anatomy/organs
- Damage/injury
- Physiology (energy, hydration, disease, reproduction)
- Cognition/thought load
- Affective state (stress/fear/contentment)
- Skill capability and learning curves
- Social behavior/cooperation

Use levels:
- **L0** none
- **L1** abstract
- **L2** medium fidelity
- **L3** high fidelity (Dwarf Fortress-style depth)

## 7) Teaching design

- Concepts taught
- Prerequisites
- Assessment methods
- Common misconceptions
- Remediation paths

## 8) Gameplay hooks

- Quest/event integrations
- Economy hooks
- Progression hooks
- Narrative hooks

## 9) API boundary

- Public types
- Commands/events
- Query endpoints
- Serialization format

## 10) Test plan

- Unit tests
- Property tests
- Simulation regression tests
- Balance/sanity checks

## 11) Performance budget

- target tick/update cost
- memory budget
- scalability assumptions

## 12) Security/safety constraints

- misuse risks
- harmful guidance guardrails
- moderation/age constraints if needed

## 13) Documentation contract

Required files:
- module `README.md`
- one runnable example scenario
- one contributor handoff note
- links to design docs + ADRs
