# module-carpentry

## 1) Module identity
- **Module name:** Carpentry
- **Crate/package id:** `module-carpentry`
- **Domain:** trade
- **Status:** draft

## 2) Purpose
Teach and simulate carpentry fundamentals: measuring, cutting, joining, load considerations, and defect handling.

## 3) Scope
### In-scope
- Measurement/marking accuracy
- Join types and tradeoffs
- Material behavior for wood classes
- Tool wear and handling safety abstraction

### Out-of-scope
- Full CAD/structural engineering certification-level modeling

## 4) Inputs / outputs
### Inputs
Blueprint/task spec, wood/material profiles, tool state, worker skill.

### Outputs
Build quality, fit tolerance, time/labor cost, safety incidents.

## 5) Core simulation model
Constraint-solving for fit + probabilistic defect outcomes weighted by skill/tool condition.

## 6) Lifeform parity requirements
- Human skill/fatigue impact L2-L3
- Non-human parity not primary in this module

## 7) Teaching design
Concepts: measurement discipline, sequence planning, quality control, safety checks.

## 8) Gameplay hooks
Construction progression, workshop economy, apprenticeship quests.

## 9) API boundary
- `CarpentryTask`
- `JoinSpec`
- `execute_step(...)`
- `quality_report(...)`

## 10) Test plan
Tolerance checks, repeatability under fixed skill/tool state, defect distribution sanity.

## 11) Performance budget
Low-medium per task.

## 12) Security/safety constraints
Keep hazardous tool operation details abstract and safety-forward.

## 13) Documentation contract
Example scenario: frame wall segment with material defects.
