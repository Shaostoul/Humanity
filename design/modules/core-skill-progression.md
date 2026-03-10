# core-skill-progression

## 1) Module identity
- **Module name:** Skill Progression Core
- **Crate/package id:** `core-skill-progression`
- **Domain:** foundation / human capability
- **Status:** active

## 2) Purpose
Provide deterministic XP/mastery progression primitives shared by all simulation modules and teaching systems.

## 3) Scope
### In-scope
- Skill XP accumulation
- Level/mastery thresholds
- Difficulty-aware XP scaling
- Capability score derivation

### Out-of-scope
- Narrative quest rewards
- UI leveling effects

## 4) Inputs / outputs
### Inputs
- practice events
- challenge difficulty
- fidelity/difficulty preset

### Outputs
- updated skill records
- level-up events
- capability deltas

## 5) Core simulation model
Deterministic XP addition with bounded multipliers and explicit threshold progression table.

## 6) Lifeform parity requirements
- Humans: L3-ready support
- Non-human lifeforms: optional skill tracks via same primitives

## 7) Teaching design
Expose weak-skill identification and prerequisite gaps for lesson recommendation.

## 8) Gameplay hooks
- profession unlocks
- construction/crafting quality modifiers
- survival competency checks

## 9) API boundary
- `SkillRecord`
- `ProgressionProfile`
- `award_xp(...)`
- `capability_index(...)`

## 10) Test plan
- threshold regression tests
- deterministic XP path tests
- multiplier boundary tests

## 11) Performance budget
O(number of tracked skills) updates with low constant cost.

## 12) Security/safety constraints
No client-trusted progression updates in closed-profile server mode.

## 13) Documentation contract
Include level threshold table and at least one scenario example.
