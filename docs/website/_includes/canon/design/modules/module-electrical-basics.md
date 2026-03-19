# module-electrical-basics

## 1) Module identity
- **Module name:** Electrical Basics
- **Crate/package id:** `module-electrical-basics`
- **Domain:** trade
- **Status:** draft

## 2) Purpose
Teach practical electrical fundamentals for safe low/medium complexity systems in homestead and workshop contexts.

## 3) Scope
### In-scope
- Basic circuit behavior
- Load/power budgeting
- Fault/overload risk abstraction
- Grounding/safety concept modeling

### Out-of-scope
- Jurisdiction-specific code compliance engine (future addon)

## 4) Inputs / outputs
### Inputs
Source profile, load profile, wiring choices, protection devices, environment.

### Outputs
System stability, efficiency losses, fault events, hazard warnings.

## 5) Core simulation model
Simplified network solver with thermal/fault thresholds and protective-trip behavior.

## 6) Lifeform parity requirements
- Human injury risk and safety behavior effects L2-L3
- Infrastructure-animal interaction impacts L1

## 7) Teaching design
Concepts: voltage/current/power, safe capacity margins, fault isolation mindset.

## 8) Gameplay hooks
Powering facilities, outage quests, maintenance routines, energy economy links.

## 9) API boundary
- `CircuitGraph`
- `LoadProfile`
- `simulate_power_step(...)`
- `fault_report(...)`

## 10) Test plan
Power conservation checks, fault injection tests, protection-trip regression.

## 11) Performance budget
Low-medium; region aggregation for large grids.

## 12) Security/safety constraints
No procedural instructions for dangerous live-work operations.

## 13) Documentation contract
Example scenario: microgrid with generator + battery + variable loads.
