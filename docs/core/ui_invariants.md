# UI Invariants

## Purpose

This document defines non-negotiable invariants for human-facing interfaces
operating under the Humanity Framework.

UI invariants ensure that consent, control, and transparency remain intact
regardless of platform, device, engine, or visual style.

An interface that violates these invariants is unsafe by design.

---

## Authority

These invariants are binding on all interfaces.

No implementation detail, artistic choice, or usability preference
may override them.

If an interface conflicts with these invariants,
the interface must be redesigned.

---

## Core Principle

Interfaces exist to preserve human agency.

Any interface that obscures understanding,
impedes control,
or manipulates behavior
violates this principle.

---

## Visibility Invariants

### 1. Active System Visibility

Users must always be able to see:
- what systems are active
- what authority those systems possess
- what actions they are taking or preparing to take

No system may operate invisibly.

---

### 2. Continuous Transparency

Transparency surfaces must:
- remain visible during normal operation
- remain visible during failure or degradation
- update in real time

Transparency must not be hidden behind menus, modes, or toggles.

---

## Consent Invariants

### 3. Consent Before Action

No action affecting a user may occur without prior consent.

Consent must be:
- explicit
- informed
- scoped
- revocable

Pre-selected options, silence, or continued use
do not constitute consent.

---

### 4. Consent Scope Clarity

Interfaces must clearly communicate:
- what actions are authorized
- what data is accessed
- what authority is delegated
- how long consent persists

Ambiguous consent is invalid consent.

---

## Control Invariants

### 5. Immediate Revocation

Users must be able to:
- revoke consent instantly
- halt automated actions immediately
- regain manual control without penalty

Revocation controls must be:
- visible
- reachable in one step
- functional under stress

---

### 6. Override Priority

Human intervention must always take precedence.

No interface may:
- delay overrides
- require confirmation to stop harm
- hide emergency controls

Stopping action must be easier than initiating it.

---

## Interface Integrity

### 7. No Deceptive Design

Interfaces must not:
- disguise consequences
- use dark patterns
- nudge users into consent
- bury critical information

Clarity overrides persuasion.

---

### 8. No Silent Defaults

Defaults must:
- be visible
- be changeable
- be explained

Hidden defaults are treated as concealed authority.

---

## Stress and Failure Conditions

### 9. Usability Under Stress

Interfaces must remain usable when users are:
- overloaded
- fatigued
- distressed
- time-constrained

Critical controls must not degrade during stress conditions.

---

### 10. Failure Visibility

When systems fail:
- failure must be explicit
- degraded capability must be visible
- users must be informed immediately

Silent failure is a safety violation.

---

## Accessibility and Comprehension

### 11. Human Legibility

Interfaces must be:
- understandable without specialized training
- readable without decoding hidden meaning
- layered for depth without obscuring basics

Expert access does not replace general legibility.

---

## Validation Criteria

An interface is compliant only if:

- users can see what is happening at all times
- users understand what authority they have granted
- users can stop action immediately
- users are not manipulated into consent
- failure never hides behavior

Passing usability tests without meeting these criteria
does not constitute compliance.

---

## Relationship to Other Documents

- Transparency Design Hooks define system-level observability.
- Safety and Responsibility define harm constraints.
- Consent and Control define legitimacy of authority.
- Governance Models define enforcement responsibility.

UI invariants bind these principles to human interaction.

---

## Summary

Interfaces are where power meets people.

If people cannot see, understand, or stop what affects them,
the system has failed.

Design accordingly.
