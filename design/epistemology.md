# Epistemology Through Gameplay

## Purpose

Define how Humanity teaches epistemology (how reliable knowledge is produced) through gameplay. Epistemology is treated as competence under uncertainty, not trivia.

---

## Core Definitions

**Reality**  
That which produces outcomes independent of preference.

**Model**  
A compressed representation used to predict outcomes. Always incomplete.

**Claim**  
A statement that can be operationalized into predictions.

**Evidence**  
Observed outcomes produced by interaction with the world, recorded with provenance.

**Uncertainty**  
A quantified description of what is not known (ranges, confidence, error bounds).

**Truth-tracking**  
A process that converges toward better predictions across varied conditions.

---

## Design Constraints

1. **No magical knowledge transfer.** Explanations may not bypass learning.
2. **Knowledge must survive contact with reality.** Competence is demonstrated by repeatable outcomes and transfer.
3. **Failure is informational.** Failures must be explainable and retriable; failure without explanation is harm.
4. **Models are not reality.** UI and systems must continually distinguish map vs territory.
5. **Transparency is mandatory.** Learners must be able to see why outcomes occurred, what rules applied, and what data mattered.
6. **Principles are enforced mechanically.** Determinism, constraints, audit trails, and structured records implement the teaching.

---

## What “Learning Epistemology” Means In-Game

Represented by:
- improved prediction accuracy across domains
- improved calibration (confidence matches hit-rate)
- reduced reliance on single sources and untested narratives
- better experimental design and controls
- faster error detection and correction
- resilience under drift and novelty

Not represented by:
- unlocking “knowledge perks”
- passing dialogue quizzes as proof of understanding
- irreversible progression from symbolic completion

---

## Canonical Gameplay Loop

### Observe → Model → Predict → Act → Measure → Update

1. **Observe**
   - gather measurements through tools, sensors, inspections, audits
   - capture context (time, place, tool state, conditions)

2. **Model**
   - select or construct a model (heuristic, causal diagram, simulation recipe, statistical fit)
   - list assumptions explicitly

3. **Predict**
   - commit to a prediction before the outcome resolves
   - include confidence and acceptable error bounds
   - define a falsifier (what would change the model)

4. **Act**
   - perform the intervention or test procedure

5. **Measure**
   - record outcomes, measurement error, and anomalies
   - preserve the full causal trace (inputs → transformations → outputs)

6. **Update**
   - compare prediction vs result
   - revise parameters, revise assumptions, or discard the model
   - require a short postmortem after high-confidence failures

---

## Teaching Mechanics

### 1) Prediction Commitments (anti-hindsight)
Before a test resolves, the player must record:
- predicted value/category
- confidence level
- falsifier condition

Scoring emphasizes:
- calibration
- honest uncertainty
- learning rate under new evidence

### 2) Provenance-First Evidence
Every evidence object carries:
- origin (who/what generated it)
- method (tool + procedure)
- environment (conditions)
- chain-of-custody (transforms, aggregations)
- tamper-evidence markers (system audit trail)

Evidence without provenance is downgraded and cannot justify high-impact actions.

### 3) Replication and Variation
Single success does not certify understanding. Certification requires:
- replication under similar conditions
- variation across conditions
- boundary condition identification (where it stops working)

### 4) Competing Models
For many problems multiple models exist:
- simple heuristic
- mechanistic causal model
- data-fit model
- hybrid model

The game tracks:
- predictive accuracy
- cost (time/materials)
- fragility (performance under drift)
- interpretability (debuggability)

### 5) Confounders and Controls
Challenges introduce confounders (hidden variables). Players must learn:
- variable isolation
- control design
- randomization where appropriate
- preventing leakage and selection effects
- detecting confounding via diagnostics

### 6) Measurement Error as First-Class
Every measurement has:
- resolution
- bias
- noise
- calibration state

Players can:
- calibrate tools
- upgrade instrumentation
- trade precision for cost/time

### 7) Bias Pressure Tests (belief hygiene)
Scenarios are constructed to trigger:
- confirmation bias
- motivated reasoning
- availability bias
- sunk cost
- authority bias

Counterplay is mechanical:
- adversarial checks required for high-stakes claims
- enforced disconfirming search quotas
- rewards for early reversal when evidence shifts
- penalties for selective reporting

### 8) Causality Over Correlation
Some quests allow correlation shortcuts that later fail under shift. The system rewards:
- interventions that demonstrate causality
- models stable under distribution shift

---

## Competence Catalog

This is not a perk tree. It is a set of measurable competencies.

1. **Observation**
   - sampling strategy
   - instrumentation choice
   - anomaly detection

2. **Measurement**
   - calibration routines
   - error budgeting
   - signal/noise separation

3. **Modeling**
   - assumption listing
   - boundary mapping
   - causal graph construction

4. **Prediction**
   - probabilistic forecasts
   - interval forecasts
   - base-rate use

5. **Experimentation**
   - control design
   - isolation and intervention
   - replication planning

6. **Updating**
   - postmortems
   - parameter refinement
   - model replacement

7. **Communication**
   - claim operationalization
   - evidence presentation
   - uncertainty disclosure

8. **Governance (high-stakes epistemics)**
   - auditability requirements
   - adversarial review
   - preventing consensus-by-force

---

## Systems That Must Integrate Epistemology

### Crafting / Engineering / Construction
- material claims must be testable (load tests, fatigue tests)
- wear introduces drift; calibration restores trust
- “worked once” cannot certify safety

### Farming / Ecology / Health
- causal complexity forces careful experimentation
- interventions carry risk; uncertainty must be explicit
- high-confidence failures require postmortems

### Economy / Planning
- forecasts drive allocations
- overconfidence produces shortages and waste
- robust plans outperform fragile “optimal” plans under volatility

### Social / Knowledge Systems
- authority is not evidence
- trust is earned via track record, replication, and provenance
- non-auditable consensus is flagged as unsafe for binding decisions

---

## UI Requirements

### Claim Card
- statement
- operationalization (predictions)
- assumptions
- status: supported / uncertain / refuted
- links to evidence objects and tests

### Evidence Viewer
- provenance chain
- method and conditions
- transformations
- uncertainty representation and error bounds

### Prediction Ledger
- timestamped commitments
- outcomes
- calibration visualization
- drift detection (accuracy decay)

### Postmortem Form
- what was predicted
- what happened
- which assumptions failed
- which evidence was ignored
- what changes now follow

Transparency is mandatory for all failures and high-impact decisions.

---

## Validation and Scoring

### Metrics
- calibration score (confidence vs hit-rate)
- probabilistic forecast score (Brier-like)
- replication rate
- drift resilience score
- audit completeness score (missing provenance penalties)
- time-to-correction after disconfirming evidence

### Failure Handling
Failure must:
- preserve causal trace
- show violated assumptions
- allow retry with altered design
- avoid punitive lockout loops

---

## Data Outputs Required

Every epistemic interaction must serialize cleanly into structured records suitable for `.csv`, `.ron`, and `.rs` ingestion.

### Canonical Record Types
- `Claim`
- `Assumption`
- `Prediction`
- `Test`
- `Measurement`
- `Evidence`
- `Model`
- `ModelEvaluation`
- `Postmortem`
- `ProvenanceLink`

### Cross-Cutting Required Fields
- stable IDs
- timestamps (game-time; optionally wall-time)
- actor (player/tool/system)
- location/context snapshot
- inputs/outputs
- uncertainty representation
- rule/procedure version
- reproducibility metadata (seed, procedure ID)
- references (IDs) linking claims ↔ predictions ↔ tests ↔ evidence ↔ models

---

## Non-Negotiables

- Epistemology is taught by doing, not by being told.
- High-impact decisions require evidence with provenance.
- Honest uncertainty and rapid correction are rewarded.
- Confident falsehoods are penalized more than cautious ambiguity.
- “Understanding” is competence under unfamiliar conditions.