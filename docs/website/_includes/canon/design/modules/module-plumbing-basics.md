# module-plumbing-basics

## 1) Module identity
- **Module name:** Plumbing Basics
- **Crate/package id:** `module-plumbing-basics`
- **Domain:** trade/homestead
- **Status:** draft

## 2) Purpose
Simulate and teach safe, practical water distribution and drainage fundamentals.

## 3) Scope
### In-scope
- Pressure vs gravity-fed behaviors
- Flow restrictions and leaks
- Basic fixtures and routing
- Waste/drain interaction with sanitation model

### Out-of-scope
- City sewer network simulation

## 4) Inputs / outputs
### Inputs
Source pressure/head, pipe/material specs, layout topology, maintenance actions.

### Outputs
Flow availability, loss/leak rates, contamination risks, repair tasks.

## 5) Core simulation model
Node-edge hydraulic abstraction with pressure/flow constraints and failure thresholds.

## 6) Lifeform parity requirements
- Human sanitation/health impacts L3
- Livestock watering/waste impacts L2

## 7) Teaching design
Concepts: pressure head, trap/seal logic, leak diagnostics, preventive maintenance.

## 8) Gameplay hooks
Infrastructure reliability, disease prevention, settlement morale/productivity.

## 9) API boundary
- `PlumbingNetwork`
- `FlowState`
- `simulate_flow(...)`
- `detect_leak(...)`

## 10) Test plan
Flow consistency tests, leak propagation tests, contamination pathway checks.

## 11) Performance budget
Low-medium.

## 12) Security/safety constraints
Avoid giving high-risk DIY procedure details.

## 13) Documentation contract
Example scenario: off-grid home water + waste loop.
