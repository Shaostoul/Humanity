# Tiered AI System Design (Public Draft)

_Status: Proposed_

## 1) Purpose

This document proposes a **tiered AI architecture** for Humanity that combines local models and cloud models to balance:

- quality,
- cost,
- privacy,
- reliability,
- and throughput.

The design is public/open-source friendly and intended to evolve with the project.

---

## 2) Problem Statement

A single-model strategy is simple, but creates tradeoffs:

- premium cloud models are expensive for high-volume routine work,
- local models are cheaper/private but weaker on complex reasoning,
- sensitive data may need tighter control than default cloud routing,
- outages/limits can block all AI features if there is no fallback layer.

Humanity needs a routing strategy that chooses the right model tier for each task.

---

## 3) Design Goals

1. **Quality-first where it matters**
   - Critical product, security, and architectural reasoning should use strongest models.

2. **Cost-aware by default**
   - Low-value repetitive tasks should be offloaded to local tiers.

3. **Privacy-aware routing**
   - Sensitive content should prefer local processing unless explicitly escalated.

4. **Observable and explainable**
   - Every routed task should record why a tier/model was selected.

5. **Graceful degradation**
   - If one tier fails, system should fall back safely instead of hard failing.

---

## 4) Tier Model

### Tier 0 — Local Utility Model (small/fast)

**Role:** high-volume utility operations.

Typical tasks:
- classification,
- tagging,
- keyword extraction,
- lightweight summarization,
- queue triage,
- format conversion,
- pre-filtering logs/events.

Properties:
- lowest cost,
- fastest response,
- lowest reasoning depth.

---

### Tier 1 — Local Reasoning Model (mid-size local)

**Role:** medium-complexity drafting/synthesis.

Typical tasks:
- structured summaries,
- draft release notes,
- first-pass troubleshooting syntheses,
- long-context condensation before cloud escalation.

Properties:
- low incremental cost,
- private by default,
- better quality than Tier 0,
- still weaker than flagship cloud models.

---

### Tier 2 — Cloud Flagship Model

**Role:** critical reasoning and final authority.

Typical tasks:
- architecture decisions,
- security-sensitive analysis,
- code changes with broad impact,
- ambiguous/high-risk decisions,
- user-facing “final answer” responses.

Properties:
- best quality,
- highest cost,
- should be used selectively but deliberately.

---

## 5) Routing Policy

Routing should consider five factors:

1. **Task criticality** (low/medium/high)
2. **Data sensitivity** (public/internal/restricted)
3. **Complexity estimate** (simple/compound/ambiguous)
4. **Latency target** (real-time/normal/background)
5. **Budget state** (normal/constrained)

### Baseline policy

- Low critical + low complexity → Tier 0
- Medium complexity + non-critical → Tier 1
- High critical or ambiguous/high-risk → Tier 2
- Restricted data → prefer local tier, escalate only with explicit policy/consent

### Escalation policy

- Tier 0 confidence below threshold → Tier 1
- Tier 1 confidence below threshold or conflicting outputs → Tier 2
- Tier failure/timeouts → fallback to next valid tier

---

## 6) Privacy and Safety Boundaries

1. **No secret leakage across tiers**
   - redact tokens/keys/PII before cloud escalation unless explicitly required.

2. **Minimum necessary context**
   - send only the smallest context needed for task completion.

3. **Policy-enforced redaction hooks**
   - pre-routing scrubber for credentials, private identifiers, and infrastructure secrets.

4. **Auditability**
   - log route decisions and redaction actions for debugging and governance.

---

## 7) Cost Control Strategy

1. **Cloud budget gate**
   - configurable token/cost envelope.

2. **Compression before escalation**
   - Tier 0/1 condense large logs/docs before Tier 2 handoff.

3. **Batching background work**
   - periodic low-priority tasks run on local tiers.

4. **Task-level limits**
   - max context and retry caps per route.

---

## 8) Observability Requirements

For each AI task, track:

- selected tier/model,
- route reason,
- fallback/escalation steps,
- latency,
- token/cost metrics (if available),
- success/failure,
- confidence/quality signal.

This enables optimization and transparent open-source iteration.

---

## 9) Example Task Mapping

- **Deploy log triage** → Tier 0 first, Tier 1 summarize anomalies, Tier 2 only for root-cause ambiguity.
- **Security incident investigation** → Tier 2 primary, Tier 1 for preprocessing artifacts.
- **Daily summary digest** → Tier 1 local by default.
- **Channel moderation queue labeling** → Tier 0 local.
- **Architecture RFC drafting** → Tier 1 draft + Tier 2 final review.

---

## 10) Open Questions

1. What confidence rubric should each tier expose (probability, heuristic, verifier-based)?
2. Which local model baseline should be mandatory for contributors?
3. How should route decisions be represented in public telemetry without exposing private data?
4. What is the default privacy mode for self-hosted deployments?
5. Should users be able to force a tier per conversation/thread?

---

## 11) Initial Implementation Plan (Phased)

### Phase A — Policy + Instrumentation
- Define route schema and policy engine.
- Add per-task routing logs.
- Implement redaction pre-processor.

### Phase B — Tier 0/1 Local Pipeline
- Connect local utility + local reasoning models.
- Add escalation thresholds.

### Phase C — Tier 2 Integration and Budget Guard
- Integrate flagship cloud routing with budget gates.
- Add fallback and failure recovery.

### Phase D — Public Metrics + Optimization
- Publish aggregate route performance stats.
- Tune policies based on real workload.

---

## 12) Non-Goals (for now)

- Full autonomous model self-selection without policy constraints.
- Replacing Tier 2 entirely with local models.
- Perfect one-size-fits-all routing from day one.

---

## 13) Summary

The tiered system is designed to make Humanity:

- more cost-efficient,
- more privacy-aware,
- more resilient,
- and more transparent.

Cloud flagship models remain essential for difficult reasoning, while local tiers absorb the repetitive load and sensitive preprocessing.
