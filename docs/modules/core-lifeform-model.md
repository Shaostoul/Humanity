# core-lifeform-model

## 1) Module identity
- **Module name:** Lifeform Core Model
- **Crate/package id:** `core-lifeform-model`
- **Domain:** life + foundation
- **Status:** draft
- **Owners:** core simulation

## 2) Purpose
Define a shared lifeform interface for humans and non-human species with parity-ready support for anatomy, injury, physiology, cognition, affect, skills, and social behavior.

## 3) Scope
### In-scope
- Species-agnostic lifeform traits/interfaces
- Body/organ schema interface
- Vital-state, injury, recovery, and disease hooks
- Cognition/affect integration points
- Skill capability interface

### Out-of-scope
- Species-specific balancing data
- UI presentation details
- Quest scripting

## 4) Inputs / outputs
### Inputs
- Time ticks
- Environmental conditions
- Damage/health events
- Resource intake (food, water, rest)

### Outputs
- Updated lifeform state
- Health incidents and teachable events
- Capability modifiers to task systems

## 5) Core simulation model
- Deterministic state transitions where possible
- Organ-level state as structured sub-components
- Tiered fidelity support (active/regional/distant)
- Failure modes: death, incapacitation, chronic decline, behavioral instability

## 6) Lifeform parity requirements
Humans: L3 across all categories.
Initial non-human targets (livestock + pollinators): anatomy L2, physiology L3, cognition L1-L2, affect L1-L2, skills L1, social L1-L2.

## 7) Teaching design
- Teach cause-and-effect from care decisions
- Show how environment and treatment affect outcomes
- Surface preventive best practices

## 8) Gameplay hooks
- NPC/creature behavior modifiers
- Settlement stability and productivity impacts
- Survival and ethics-driven decision consequences

## 9) API boundary
- `LifeformState`
- `SpeciesProfile`
- `apply_event(...)`
- `tick(...)`
- `capability_snapshot(...)`

## 10) Test plan
- Trait conformance tests
- Deterministic tick regression tests
- Injury/recovery scenario tests
- Tier transition continuity tests

## 11) Performance budget
- O(active entities) high-fidelity ticks
- bounded memory per lifeform profile

## 12) Security/safety constraints
- Avoid exploit loops that reward cruelty
- Keep educational framing for injury content

## 13) Documentation contract
- README with species parity table
- Example scenario: human + livestock + crop stress chain
- Link: `docs/09-LIFEFORM-PARITY-FRAMEWORK.md`
