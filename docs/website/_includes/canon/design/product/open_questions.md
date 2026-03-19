# Humanity Open Questions & Decision Log (Draft)

Use this file to capture unresolved architecture/product decisions.
Each item should end in: **Decision**, **Owner**, **Due Date**, **Status**.

---

## A) Identity & Account

### Q1. What is the recovery model?
- Options:
- Recovery phrase only
- Trusted contacts
- Multi-device quorum
- Optional custodial fallback
- Concern:
- Recovery convenience vs cryptographic guarantees

**Decision:** TBD
**Owner:**
**Due Date:**
**Status:** Open

### Q2. Are pseudonymous and verified identities both first-class at launch?
- If yes, what are trust signals for each?

**Decision:** TBD
**Owner:**
**Due Date:**
**Status:** Open

---

## B) Messaging Security

### Q3. Which protocol stack for E2EE messaging?
- Build custom protocol vs adopt proven primitives/frameworks
- Need formal reviewability + implementation practicality in Rust

**Decision:** TBD
**Owner:**
**Due Date:**
**Status:** Open

### Q4. How long is server-side metadata retained?
- Need operational debugging window without surveillance creep
- Define hard deletion/retention schedule

**Decision:** TBD
**Owner:**
**Due Date:**
**Status:** Open

---

## C) Network Architecture

### Q5. Relay strategy: centralized first vs federated-ready from day one?
- Faster launch with centralized
- Better resilience with federation planning early

**Decision:** TBD
**Owner:**
**Due Date:**
**Status:** Open

### Q6. NAT traversal approach and fallback priority?
- STUN/TURN-like strategy equivalents
- Mobile network edge-case behavior

**Decision:** TBD
**Owner:**
**Due Date:**
**Status:** Open

---

## D) Product Scope

### Q7. What is explicitly in MVP vs Phase 2?
- Keep MVP tight:
- identity
- secure chat
- contacts/presence
- Move market/game integration later unless critical pilot needs it

**Decision:** TBD
**Owner:**
**Due Date:**
**Status:** Open

### Q8. Web client capability ceiling?
- How much secure functionality in browser vs native-only?

**Decision:** TBD
**Owner:**
**Due Date:**
**Status:** Open

---

## E) Marketplace / Economy

### Q9. How to separate real-money systems from in-game progression?
- Prevent pay-to-win
- Preserve fairness and trust

**Decision:** TBD
**Owner:**
**Due Date:**
**Status:** Open

### Q10. Partner model governance
- Listing standards
- Data-sharing limitations
- Fee/commission ethics policy

**Decision:** TBD
**Owner:**
**Due Date:**
**Status:** Open

---

## F) Governance & Trust/Safety

### Q11. Moderation model for private vs public spaces
- Private E2EE DMs: user controls + reports
- Public spaces: stronger policy controls

**Decision:** TBD
**Owner:**
**Due Date:**
**Status:** Open

### Q12. Legal/process framework
- Jurisdiction
- Lawful request handling
- Transparency reporting cadence

**Decision:** TBD
**Owner:**
**Due Date:**
**Status:** Open

---

## G) Project Universe Integration

### Q13. Which ecosystem actions must be available in-game at first integration milestone?
- Suggested first set:
- chat
- contacts/presence
- service browsing
- event participation

**Decision:** TBD
**Owner:**
**Due Date:**
**Status:** Open

### Q14. How narrative-driven should functional UX be?
- Pure utility mode vs lore-immersive mode toggle

**Decision:** TBD
**Owner:**
**Due Date:**
**Status:** Open

---

## H) Technical Foundation

### Q15. Monorepo vs multirepo structure?
- Impacts build pipelines, release coordination, and contributor onboarding

**Decision:** TBD
**Owner:**
**Due Date:**
**Status:** Open

### Q16. Telemetry model
- Privacy-preserving operational metrics without invasive tracking

**Decision:** TBD
**Owner:**
**Due Date:**
**Status:** Open

---

## Next Review Cadence

- Weekly architecture review (60 min)
- Every review must close at least 2 open questions
- Record finalized decisions back into:
- `02-ecosystem-architecture.md`
- `03-security-and-privacy-architecture.md`
- `04-product-roadmap.md`