# core-teaching-graph

## 1) Module identity
- **Module name:** Teaching Graph Core
- **Crate/package id:** `core-teaching-graph`
- **Domain:** teaching
- **Status:** active

## 2) Purpose
Define competency graph primitives for prerequisite validation and next-best lesson recommendations.

## 3) Scope
### In-scope
- competency nodes/edges
- prerequisite validation
- recommendation stubs based on mastery gaps

### Out-of-scope
- natural-language lesson generation
- multimedia presentation/UI

## 4) Inputs / outputs
### Inputs
- competency graph
- skill mastery snapshots

### Outputs
- unmet prerequisites
- recommended next competencies

## 5) Core simulation model
Directed acyclic graph (DAG) with cycle rejection and score-weighted recommendation ordering.

## 6) Lifeform parity requirements
Supports species-specific learning tracks through node tags while reusing shared graph logic.

## 7) Teaching design
Surface minimum-path prerequisites first, then high-impact follow-up competencies.

## 8) Gameplay hooks
- mission gating
- tutorial unlock flow
- dynamic guidance prompts

## 9) API boundary
- `CompetencyNode`
- `CompetencyEdge`
- `validate_graph(...)`
- `recommend_next(...)`

## 10) Test plan
- cycle detection tests
- prerequisite resolution tests
- recommendation ordering tests

## 11) Performance budget
Topological validation + bounded recommendation query over graph size.

## 12) Security/safety constraints
Guidance must not override policy restrictions (age/safety/role access).

## 13) Documentation contract
Include one competency DAG example and recommendation scenario.
