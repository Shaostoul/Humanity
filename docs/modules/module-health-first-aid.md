# module-health-first-aid

## 1) Module identity
- **Module name:** Health + First Aid
- **Crate/package id:** `module-health-first-aid`
- **Domain:** human capability
- **Status:** draft

## 2) Purpose
Teach prevention, triage logic, and stabilization principles in emergency and everyday health scenarios.

## 3) Scope
### In-scope
- Injury categories and severity abstraction
- Triage prioritization logic
- Stabilization/prevention concepts
- Recovery and follow-up state changes

### Out-of-scope
- Professional medical procedure simulation requiring licensure-level detail

## 4) Inputs / outputs
### Inputs
Incident events, lifeform state, available resources, responder skill.

### Outputs
Stabilization outcome probabilities, risk trajectories, teaching feedback.

## 5) Core simulation model
State-transition model with urgency tiers and time-sensitive intervention windows.

## 6) Lifeform parity requirements
- Humans: L3
- Livestock/animals: L2 (caretaker-oriented health response)

## 7) Teaching design
Concepts: scene safety, triage priorities, prevention, when to escalate to professionals.

## 8) Gameplay hooks
Emergency events, settlement health burden, role specialization pathways.

## 9) API boundary
- `HealthIncident`
- `TriageLevel`
- `apply_first_aid(...)`
- `recovery_projection(...)`

## 10) Test plan
Triage ordering tests, intervention timing sensitivity tests, recovery consistency tests.

## 11) Performance budget
Low per incident; aggregate by population when distant.

## 12) Security/safety constraints
Educational and safety-forward only; avoid harmful step-by-step content.

## 13) Documentation contract
Example scenario: workshop injury with delayed response tradeoffs.
