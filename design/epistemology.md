## Stricter Epistemology

### Definition
Stricter epistemology is a rule-set enforced by game mechanics where beliefs and procedures are not “held” by preference, status, or narrative. They are *earned* by predictive performance, provenance, replication, and resilience under changing conditions.

This exists because the default human failure mode is not ignorance; it is certainty without warranty. The game must make that failure mode expensive and visible.

### Design Goal
Convert epistemology from a speech-act into a behavior: **observe → model → predict → act → measure → update**, with auditable artifacts at every step.

### Core Enforcement Rules

1. **No-commitment, no-credit**
   - No pre-registered prediction → no epistemic reward.
   - Retrospective “I knew it” is treated as noise.

2. **High-stakes gating**
   - Actions flagged **high impact** require a complete epistemic packet before execution:
     - claim operationalization
     - prediction
     - confidence
     - falsifier
     - procedure/method
     - stopping rule
     - evidence admissibility status

3. **Provenance gating**
   - Evidence without provenance cannot justify binding decisions.
   - It may generate hypotheses only (low-stakes exploration).

4. **Calibration dominance**
   - Progression is driven primarily by calibration:
     - confidence must match hit-rate over time
   - Overconfidence is penalized more than cautious uncertainty.

5. **Replication threshold**
   - A single success cannot promote a model or procedure.
   - Promotion requires:
     - replication under similar conditions
     - at least one meaningful variation test
     - boundary conditions (where it fails) recorded

6. **Adversarial checks for governance**
   - Community-level decisions require an adversarial step:
     - counter-model or counter-test proposed
     - response recorded (accept, refute, revise)
   - Skipping adversarial review blocks policy promotion.

7. **Drift invalidates trust automatically**
   - When predictive performance decays under changed conditions, the system downgrades trust:
     - trusted → working → hypothesis
   - Re-validation is mandatory to regain status.

8. **Selective reporting is punished**
   - The system tracks:
     - discarded trials
     - unreported measurements
     - cherry-picked subsets
   - Missing data reduces admissibility and trust.

9. **Causality is privileged over correlation**
   - Correlation-only solutions may “work” short-term but degrade under shift.
   - Causal demonstrations (interventions, controls) are required for certification.

### Player-Facing Behaviors the System Forces
- State what would change your mind before you see the outcome.
- Track uncertainty explicitly instead of hiding it.
- Show the chain of custody of evidence.
- Replicate, vary, and map failure boundaries.
- Update rapidly when wrong, especially after high-confidence errors.
- Treat disagreement as an engineering problem: test design, not persuasion.

### Mechanics Required

#### Decision Gate (High-Stakes Blocker)
A state machine that blocks high-impact actions until required epistemic artifacts exist and are admissible.

#### Prediction Ledger (Anti-Hindsight Log)
An immutable log of predictions committed prior to outcome reveal, with timestamps and confidence. Outcomes are linked only after resolution.

#### Evidence Provenance Graph (DAG)
Evidence objects form a directed acyclic graph of transformations:
- raw measurement → cleaned → aggregated → derived metric
Missing links downgrade admissibility.

#### Model Promotion Pipeline
Models and procedures have explicit trust states:
- `Hypothesis` → `Working` → `Trusted` → `Certified`

Promotion requires explicit tests, replication, and variation. Downgrades occur automatically under drift or audit failure.

#### Adversarial Review System
For governance or high-stakes claims, an adversarial actor (NPC/system/other players) must propose a counter-test or counter-model. The claimant must respond with a recorded update.

#### Drift Monitor
Sliding-window evaluation of predictive accuracy by context. If performance degrades past thresholds, trust is revoked automatically.

### Canonical Fields (Required for `.csv`, `.ron`, `.rs`)

Add these fields across your epistemic record types (`Claim`, `Prediction`, `Test`, `Measurement`, `Evidence`, `Model`, `ModelEvaluation`, `Postmortem`):

- `stake_level`: `low | medium | high`
- `admissibility`: `admissible | downgraded | inadmissible`
- `preregistered`: `bool`
- `prediction_value`: numeric/category/struct
- `confidence`: `[0.0..1.0]` (or calibrated discrete levels)
- `error_bounds`: interval or distribution
- `falsifier`: structured condition
- `stopping_rule`: structured condition
- `procedure_id` and `procedure_version`
- `tool_id` and `tool_calibration_state`
- `context_snapshot_id` (environmental conditions)
- `replication_count`: `u32`
- `variation_count`: `u32`
- `boundary_conditions`: references or structs
- `audit_required`: `bool`
- `review_mode`: `none | peer | adversarial | governance`
- `drift_status`: `stable | degrading | invalid`
- `evidence_provenance_links`: list of IDs (DAG edges)
- `discarded_trials_count`: `u32`
- `missing_data_flags`: list/bitset
- `postmortem_required`: `bool`
- `postmortem_id`: optional ID

### Scoring Priorities
- Calibration and correction speed dominate.
- Replication and admissibility dominate high-stakes progression.
- Overconfident failure costs more than cautious uncertainty.
- Transparent uncertainty costs less than concealed uncertainty.

### Non-Negotiables
- The game never treats confidence as virtue by itself.
- The game never allows binding decisions without admissible evidence.
- The game never allows a single success to become doctrine.
- The game always makes epistemic shortcuts visible, measurable, and costly.