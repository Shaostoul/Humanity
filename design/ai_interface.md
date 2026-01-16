# ai_interface.md — Artificial Intelligence Interaction Contract

This document defines how AI systems may interact with Project Universe.

AI is a **reader, interpreter, and educator**. AI is **never an authority**. Reality is defined by data, constraints, and simulation—not by AI output.

This contract exists to preserve determinism, low-power viability, and truth fidelity.

---

## 1. Core principles

1. **AI is optional** — the game must function fully without AI.
2. **AI is advisory** — it may explain, not decide.
3. **AI is bounded** — it may not override constraints or simulation results.
4. **AI is replaceable** — no design may depend on a specific model or vendor.
5. **AI is auditable** — inputs and outputs must be inspectable.

---

## 2. Allowed AI roles

AI systems may:

* Explain game state and outcomes using canonical data
* Translate simulation results into natural language
* Teach concepts, practices, and failure causes
* Help players plan within known constraints
* Summarize action logs and replay causality
* Assist with documentation and mod authoring (non-authoritative)

AI systems may not:

* Change simulation state
* Generate resources
* Bypass time, labor, or energy costs
* Override failure outcomes
* Introduce new rules or facts

---

## 3. AI inputs (read-only)

AI may read the following sources:

* `design/` documents
* `data/` definitions, recipes, practices, equations
* `realism_constraints.md`
* Current world state snapshots (read-only)
* Action logs and replay traces
* Validation error reports
* Localization and glossary entries

All AI inputs must be explicitly provided. AI may not infer hidden state.

---

## 4. AI outputs (non-binding)

AI outputs are **informational artifacts**, not commands.

Permitted output types:

* Explanations ("Your crop failed because…")
* Diagnoses ("Symptoms match nitrogen deficiency")
* Recommendations ("Given constraints, consider…")
* Educational summaries
* Planning suggestions with tradeoff disclosure

All recommendations must:

* Reference underlying constraints
* Acknowledge uncertainty
* Avoid certainty claims about stochastic outcomes

---

## 5. Prohibited AI behaviors

The following are forbidden:

* Hallucinating mechanics or rules
* Inferring hidden game data
* Masking failure causes
* Providing success guarantees
* Acting as an oracle or narrator of truth

If AI output contradicts simulation or data, AI output is wrong.

---

## 6. Low-power and offline requirement

* No AI system is required to run the game.
* AI integrations must degrade gracefully to:

  * static documentation
  * rule-based explanations
  * pre-authored educational content

AI must never be a hard dependency for:

* gameplay
* progression
* learning outcomes

---

## 7. Determinism and reproducibility

AI must not introduce nondeterminism into simulation.

Rules:

* AI cannot generate random values used by the simulation.
* AI suggestions must not affect authoritative state.
* Any AI-assisted planning must reference deterministic evaluation tools.

Replay of a session must produce identical results regardless of AI presence.

---

## 8. Educational integrity

AI explanations must:

* Align with `education_model.md`
* Reference real failure cases and constraints
* Explain *why* an outcome occurred

AI may not:

* Simplify reality beyond correctness
* Replace practice with narration
* Skip prerequisite knowledge

---

## 9. Interface boundaries

### 9.1 Explicit API surface

AI may interact only through:

* structured data exports (JSON/RON)
* read-only query APIs
* text-based explanation channels

AI may not:

* hook directly into engine loops
* intercept or modify actions
* bypass validation layers

---

## 10. Model neutrality

Project Universe does not endorse or require:

* a specific LLM
* cloud connectivity
* proprietary inference engines

Any AI implementation must be swappable without design change.

---

## 11. Modding and AI

Mods may:

* add AI-readable educational content
* add domain explanations

Mods may not:

* embed AI authority
* alter AI contract rules

AI behavior must remain consistent across modded and unmodded states.

---

## 12. Failure handling

If AI output is:

* missing
* incorrect
* contradictory

The game must:

* continue functioning
* fall back to canonical documentation
* surface authoritative explanations from data

AI failure must never block learning or play.

---

## 13. Compliance requirements

AI integrations must be validated to ensure:

* no write access to simulation
* no dependency on AI availability
* no contradiction of realism constraints

Non-compliant AI integrations are invalid.

---

## 14. Summary

* AI explains reality; it does not define it.
* AI assists learning; it does not replace practice.
* AI is optional, bounded, and auditable.
* Reality remains deterministic and authoritative.

AI is a lens, not a law.
