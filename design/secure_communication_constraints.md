# Secure Communication Constraints

## Purpose

This document defines non-negotiable constraints governing communication
systems operating under the Humanity Framework.

Its purpose is to preserve:
- freedom of expression
- privacy and confidentiality
- resistance to censorship and coercion
- truthful representation of security claims

Communication safety is a prerequisite for democratic participation,
collective coordination, and resistance to tyranny.

---

## Core Principle

If a system claims privacy, confidentiality, or end-to-end encryption,
those claims must be materially true.

False or misleading security claims constitute harm.

---

## Freedom of Expression Constraints

### 1. No Coercive Suppression

No authority may suppress expression solely due to:
- dissent
- criticism
- unpopular beliefs
- nonconformity

Restrictions on expression must be:
- explicitly defined
- harm-based
- transparent
- appealable

Silent suppression is forbidden.

---

### 2. Transparency of Moderation

If communication is moderated:
- moderation rules must be public
- enforcement actions must be visible to affected parties
- reasons must be provided
- appeal mechanisms must exist

Algorithmic moderation must not operate invisibly.

---

## Privacy and Confidentiality Constraints

### 3. Truthful End-to-End Encryption (E2EE)

A system may claim end-to-end encryption **only if**:

- plaintext is accessible exclusively to the communicating endpoints
- no intermediary possesses decryption capability
- no escrow, backdoor, or silent recovery mechanism exists

If any exception exists, the system must not claim E2EE.

---

### 4. No Deceptive Privacy Claims

Systems must not:
- claim privacy while retaining decryption capability
- imply confidentiality while harvesting content
- obscure metadata collection that materially weakens privacy

If backups, metadata, or recovery mechanisms weaken confidentiality,
those limitations must be disclosed continuously and prominently.

---

### 5. Key Ownership and Control

Cryptographic keys must:
- be generated and controlled by users
- never be silently exported
- never be substituted without user knowledge

Server-managed keys invalidate E2EE claims.

---

## Metadata and Surveillance Constraints

### 6. Metadata Minimization

Systems must minimize collection of:
- identity linkages
- social graphs
- communication timing
- behavioral fingerprints

Metadata that enables coercion or targeting
is treated as sensitive as content.

---

### 7. No Undeclared Surveillance

Surveillance capabilities must:
- be explicitly disclosed
- be narrowly scoped
- require meaningful consent
- be auditable

Undeclared surveillance is a severe safety violation.

---

## Resilience and Censorship Resistance

### 8. Resistance to Centralized Control

Communication systems must not rely on:
- single points of narrative control
- unilateral shutdown authority
- opaque ranking or suppression mechanisms

Decentralization is preferred where feasible.

---

### 9. Degradation Without Silence

When communication systems degrade:
- failure must be visible
- messages must not be silently dropped
- users must be informed of loss or delay

Silent failure is equivalent to suppression.

---

## Accountability and Auditability

### 10. Verifiable Claims

Security and privacy claims must be:
- technically verifiable
- auditable by independent parties
- reproducible in principle

Marketing language does not substitute for proof.

---

## Enforcement

Any system that violates these constraints:
- forfeits its claim to secure communication
- must not be trusted for voting, governance, or safety-critical use

Claims of E2EE or privacy that are untrue
constitute deception and harm.

---

## Relationship to Other Documents

- Ethical Principles define why expression and privacy matter.
- Safety and Responsibility define harm boundaries.
- Transparency Guarantees define observability requirements.
- Voting Integrity Constraints depend on these guarantees.

---

## Summary

Freedom of expression requires privacy.
Privacy requires truthful encryption.
Truth requires transparency.

Communication systems that lie about security
undermine democracy by design.
