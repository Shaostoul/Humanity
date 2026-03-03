## Stricter Epistemology

### Purpose
Convert “knowing” into enforceable behavior. Every belief that influences outcomes must be:
1) operationalized into predictions,  
2) committed before observation,  
3) backed by admissible evidence with provenance,  
4) validated by replication and variation,  
5) continuously monitored for drift,  
6) revocable on failure.

No exceptions for status, narrative, authority, or convenience.

---

## Hard Rules

### R0 — Definitions are operational
A claim exists only if it produces:
- a measurable target
- a measurement method
- a time window
- an expected range/distribution
- a falsifier

Anything else is labeled **non-operational** and cannot drive decisions.

### R1 — No pre-registration, no effect
If a belief is not recorded *before* outcome reveal, it cannot:
- unlock progression
- grant certifications
- justify high-impact actions
- become a procedure
- update community doctrine

Retrospective reasoning is treated as noise.

### R2 — High-stakes actions are blocked by default
All actions are classified: `low | medium | high` stakes.

Any **high** stake action is **hard-blocked** until the player provides an Epistemic Packet containing:
- claim ID (operational)
- prediction value(s)
- confidence / uncertainty
- falsifier condition(s)
- method/procedure ID + version
- stopping rule
- required controls (if applicable)
- admissible evidence IDs (or a plan to generate them)

If the packet is incomplete or inadmissible, the action does not execute.

### R3 — Evidence without provenance is inadmissible
Evidence must include:
- source actor/tool
- procedure ID + version
- context snapshot (conditions)
- calibration state
- raw data hash / tamper-evidence marker
- transformation chain (DAG)

Missing any required provenance fields → **inadmissible** for binding decisions.

### R4 — Calibration is the primary fitness function
Progression and trust are based on calibration, not success count.

Rule:
- overconfidence is punished superlinearly
- underconfidence is mildly punished
- correct uncertainty is rewarded

A player with honest 60% confidence and 60% hit-rate outranks a player claiming 95% confidence with 70% hit-rate.

### R5 — Single-shot success cannot promote trust
A model/procedure cannot be promoted beyond `Hypothesis` without:
- `N` replications (default 3)
- `M` variation tests (default 1)
- recorded boundary conditions

Promotion ladder:
- `Hypothesis` → `Working` requires: 1 success + admissible provenance
- `Working` → `Trusted` requires: N replications + calibration above threshold
- `Trusted` → `Certified` requires: M variations + adversarial check + audit completeness

### R6 — Drift revokes trust automatically
Any `Trusted` or `Certified` model is continuously evaluated in a sliding window.

If performance drops below thresholds:
- trust is downgraded automatically
- further high-stakes use is blocked
- re-validation is required for restoration

No manual override.

### R7 — Adversarial review is mandatory for governance
Any claim affecting:
- shared systems
- communal policy
- safety-critical procedures

requires an adversarial step:
- counter-model or counter-test must exist
- claimant must respond with: accept/reject + recorded rationale + new predictions

Without adversarial artifacts → policy cannot promote.

### R8 — Selective reporting is penalized as falsification
The system tracks:
- discarded trials
- missing measurements
- unreported outcomes
- cherry-picked subsets

Rules:
- unexplained missingness downgrades admissibility
- repeated missingness triggers audit lockout
- selective reporting blocks certification

### R9 — Correlation cannot certify causality
Correlation-only models may be used for low stakes if calibration is good, but they cannot be `Certified`.

Certification requires at least one:
- controlled intervention
- randomized assignment (when feasible)
- causal identification strategy logged in the procedure

### R10 — Postmortems are compulsory on high-confidence failure
If confidence ≥ threshold (default 0.8) and prediction fails:
- a postmortem record is required before further progress in that domain
- the postmortem must name:
  - violated assumptions
  - ignored evidence
  - proposed model change
  - next test plan

No postmortem → no continuation in that track.

---

## Epistemic Packet (Required Structure)

### Packet Fields
- `claim_id`
- `stake_level`
- `prediction`
- `confidence`
- `uncertainty_representation` (interval/distribution)
- `falsifier`
- `stopping_rule`
- `procedure_id`
- `procedure_version`
- `required_controls`
- `context_snapshot_id`
- `evidence_ids` (optional if generating evidence; required before execution for high stakes)
- `admissibility_status`

### Validity Conditions
Packet is valid only if:
- claim is operational (R0)
- prediction is committed before outcome (R1)
- evidence is admissible for the stake level (R3)
- procedure is versioned and reproducible (R3)
- controls are specified when confounding risk is nontrivial (R2/R9)

---

## System Mechanics (Non-Optional)

### Decision Gate
Hard-blocker enforcing R1–R3 and R2.

### Prediction Ledger
Immutable pre-commit log; outcomes link only after resolution.

### Provenance DAG
All derived evidence must link back to raw measurements; missing edge = inadmissible.

### Trust State Machine
`Hypothesis → Working → Trusted → Certified → Revoked`
Transitions are rule-based; revocation is automatic under drift, audit failure, or selective reporting.

### Drift Monitor
Context-aware sliding window evaluation; triggers automatic downgrade.

### Adversarial Harness
Generates or requires counter-tests; blocks governance promotion without them.

### Audit Lockout
If a player repeatedly produces inadmissible evidence or selective reporting patterns, the system:
- blocks certification attempts
- forces remediation tasks (instrumentation, logging discipline, replication)

---

## Required Data Fields (Minimum)

Across record types:
- IDs and timestamps
- stake level
- procedure/version
- tool + calibration
- context snapshot
- raw data hashes / tamper markers
- provenance DAG links
- prediction + confidence + uncertainty
- falsifier + stopping rule
- replication/variation counts
- trust state
- drift metrics
- missingness/selective reporting flags
- adversarial review artifacts
- postmortem link (when triggered)

---

## Non-Negotiables
- If it cannot be pre-registered, it cannot matter.
- If it cannot be audited, it cannot govern.
- If it cannot replicate, it cannot be trusted.
- If it drifts, it is revoked.
- If it is selectively reported, it is treated as falsification.