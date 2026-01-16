# testing_philosophy.md — Verification as Truth Enforcement

This document defines **what must be tested, why it must be tested, and what failure means** in Project Universe.

Testing is not primarily for bugs. Testing is for **truth preservation**.

If a system teaches something false, the build must fail.

---

## 1. Testing principles

1. **Correctness outranks balance** — balanced falsehood is still falsehood.
2. **Determinism is mandatory** — nondeterministic failures are treated as defects.
3. **Constraints are law** — any violation of `realism_constraints.md` is a hard failure.
4. **Education is testable** — explanations must match causes.
5. **Regression is unacceptable** — once a truth is encoded, it cannot silently change.

---

## 2. Test categories

### 2.1 Unit tests

Purpose:

* Validate small deterministic rules.

Examples:

* unit conversions
* nutrient accounting
* spoilage rate functions

---

### 2.2 Property-based tests

Purpose:

* Verify invariants across large input space.

Examples:

* conservation of mass across recipes
* calories in/out accounting
* monotonic decay functions

---

### 2.3 Simulation replay tests

Purpose:

* Prove determinism.

Method:

* run a simulation with seed + action log
* replay
* assert identical state hashes at milestones

---

### 2.4 Data validation tests

Purpose:

* Ensure data is lawful.

Fail build if:

* missing units
* unresolved references
* missing failure cases
* impossible values

---

### 2.5 Integration tests

Purpose:

* Verify domain interactions.

Examples:

* crop growth depends on soil + water + temperature
* fatigue increases error rate
* preservation trades time/energy for reduced spoilage

---

### 2.6 Education correctness tests

Purpose:

* Ensure teaching matches reality.

Requirements:

* every failure reason returned by validation must map to:

  * a constraint
  * a practice mistake
  * a failure case

The game must be able to say:

* what happened
* why it happened
* what could prevent it

---

### 2.7 Performance tests

Purpose:

* Ensure low-power viability.

Rules:

* complexity should scale linearly with active entities
* performance improvements must not change outcomes

---

## 3. What must always be tested

### 3.1 Determinism

* identical inputs ⇒ identical outputs
* no floating drift between supported platforms

---

### 3.2 Conservation

* mass conservation across transformations
* energy conservation in energy systems
* nutrition conservation in diets and labor

---

### 3.3 Time cost

* every process declares time
* no zero-time production

---

### 3.4 Failure cases

* every major system must fail in defined ways
* failure must produce causal explanations

---

## 4. Test artifacts and fixtures

### 4.1 Golden replays

* Store canonical seed + action logs for reference scenarios.
* These are treated as constitutional fixtures.

Examples:

* basic homestead week
* crop cycle with nutrient stress
* machine maintenance neglect scenario

---

### 4.2 Scenario packs

* Small, curated scenarios representing common real-world situations.
* Used for education, regression, and demonstration.

---

## 5. Failure policy

A test failure means:

* truth was violated
* determinism was broken
* educational explanation no longer matches causality

Response is to:

* fix the bug
* or explicitly update design + data + tests together

Silent changes are forbidden.

---

## 6. Balance policy

Balance is measured *after* correctness.

Rules:

* If realism makes something hard, difficulty is not a bug.
* If realism makes something too easy, ease is not a bug.

Balance changes must:

* preserve constraints
* remain explainable

---

## 7. Multiplayer test stance

Even if multiplayer is optional:

* simulation must remain deterministic under synchronized actions
* desync is treated as a correctness failure

---

## 8. Mod stance (open source)

Project Universe is open source. Testing therefore serves as the immune system.

Rules:

* forks may do anything, but the mainline remains constraint-correct
* contributions that violate constraints are rejected by tests

Testing replaces trust.

---

## 9. Minimal required test suite

The following suite must run on every change:

* schema/unit validation
* conservation properties
* replay determinism
* failure explanation mapping

Optional but recommended:

* performance regressions

---

## 10. Design intent restated

Project Universe must remain:

* deterministic
* explainable
* educationally correct
* grounded in reality

Testing is how reality remains enforceable.
