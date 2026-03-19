# Voting Integrity Constraints

## Purpose

This document defines non-negotiable constraints for systems that support
collective decision-making under the Humanity Framework.

Voting is treated as a safety-critical function.
Failure of voting integrity constitutes civilizational harm.

---

## Core Principle

Participation must be:
- equal
- private
- verifiable
- resistant to coercion
- resistant to tampering
- resilient to failure

Trust is insufficient.
Correctness must be provable.

---

## Fundamental Guarantees

### 1. One Human, One Vote

Each eligible human may:
- cast at most one vote per decision
- verify that their vote was counted
- verify that no additional votes were substituted

No system may amplify, suppress, or duplicate participation.

---

### 2. Identity Separation

Voting systems must strictly separate:
- **eligibility verification**
- **ballot casting**
- **vote counting**

No component may link:
- voter identity to ballot content
- eligibility credentials to final tally

Linkability after validation is forbidden.

---

## Privacy and Anonymity

### 3. Ballot Secrecy

No authority may:
- determine how an individual voted
- infer voting behavior through metadata
- reconstruct ballots after casting

Privacy must persist:
- during voting
- after voting
- indefinitely

---

### 4. Resistance to Coercion

Systems must prevent:
- proof-of-vote generation
- vote selling
- forced disclosure

If a voter can prove how they voted,
the system is unsafe.

---

## Verifiability

### 5. End-to-End Verifiability

Voting systems must allow:
- voters to verify inclusion of their ballot
- observers to verify correctness of the tally
- auditors to verify process integrity

Verification must not compromise voter anonymity.

---

### 6. Public Auditability

Vote counting must be:
- transparent
- reproducible in principle
- independently auditable

Black-box tallying is forbidden.

---

## Integrity and Tamper Resistance

### 7. Tamper Evidence

All voting artifacts must be:
- immutable once cast
- tamper-evident
- traceable through public logs

Undetectable modification constitutes failure.

---

### 8. Determinism and Reproducibility

Vote counting must be:
- deterministic
- reproducible from published artifacts
- independent of hidden state

If two honest auditors cannot reproduce results,
the system is invalid.

---

## Infrastructure Constraints

### 9. Minimal Trusted Computing Base

Voting systems must:
- minimize complexity
- minimize code surface
- minimize dependencies

Complexity increases attack surface.
Simplicity increases trust.

---

### 10. Network Isolation During Voting

Voting infrastructure must not:
- depend on live network access
- allow remote code execution
- allow dynamic updates during voting

Connectivity increases risk.

---

## Failure Handling

### 11. Fail-Safe Behavior

On failure, systems must:
- halt voting
- preserve cast ballots
- prevent partial or corrupted tallies
- make failure visible immediately

Silent degradation is forbidden.

---

## Accountability

### 12. Responsibility for Integrity

Responsibility for voting integrity lies with:
- system designers
- system deployers
- system operators

Voters are never responsible for:
- system compromise
- hidden failure modes
- misleading assurances

---

## Enforcement

Any system that violates these constraints:
- must not be used for governance
- must not be trusted for collective decision-making
- must be withdrawn until corrected

Efficiency does not excuse risk.
Convenience does not excuse harm.

---

## Relationship to Other Documents

- Secure Communication Constraints define confidentiality requirements.
- Safety and Responsibility define harm minimization.
- Transparency Guarantees define observability.
- Governance Models define scope and enforcement.

---

## Summary

Democracy cannot rely on trust alone.

A voting system that cannot be verified
cannot be legitimate.

Participation without integrity
is coercion by other means.
