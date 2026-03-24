# OpenClaw Integration

This folder contains configuration, documentation, and tooling
related to integrating OpenClaw with Humanity.

OpenClaw is an interface layer.

---

## Authority Rules

OpenClaw is **not an authority**.

It may:
- read from `accord/`, `design/`, and `data/`
- summarize or present information
- assist with navigation, explanation, or interaction

It may **not**:
- redefine principles
- introduce new rules
- override constraints
- store canonical truth

Any output produced by OpenClaw is advisory unless it directly
references canonical documents.

---

## Intended Use

OpenClaw may be used to:
- explain the Humanity Accord
- help users navigate documents
- assist with learning and simulation interaction
- provide voice or text interfaces

All such use must be grounded in referenced source material.

---

## Configuration Policy

- Prompts, system messages, and adapters belong here
- Any prompt that encodes assumptions must cite the source document
- No hard-coded ideology or “creative reinterpretation”

---

## Why This Boundary Exists

Interfaces change.
Models change.
Civilizational principles must not drift because tooling did.

OpenClaw is a lens, not a lawgiver.

---

## Future Work

Additional integrations (voice, UI, assistants) should follow
this same pattern:
- tools read truth
- tools do not define truth
