---
layout: default
title: Status
---

# Project Status

**Last updated:** February 2026

---

## Current Phase: Specification

We're in the **spec-first, data-first** phase. This means:

- ✅ Documentation defines what must be true
- ✅ Schemas define what data must look like
- 🔄 Reference implementation is next
- ⏳ Game engine comes after the foundation is solid

---

## What's Complete

### The Humanity Accord
The civilizational framework is documented and stable:
- Core charter and ethical principles
- Rights, responsibilities, and prohibitions
- Governance models and conflict resolution
- Transparency and consent requirements

→ [Read the Accord](/Humanity/accord)

### Technical Design
System constraints and specifications are defined:
- Network architecture (hybrid P2P + relay)
- Object format (CBOR, BLAKE3, Ed25519)
- Identity and encryption model
- Moderation and governance schemas
- Security threat model

→ [View Design](/Humanity/design)

---

## What's In Progress

### Reference Implementation
Building the core Rust crates to validate the spec:
- `humanity-core` — object encoding, hashing, signatures
- `humanity-storage` — local persistence
- `humanity-cli` — command-line tools

### Test Vectors
Generating canonical test cases for:
- Object encoding/decoding
- Hash computation
- Signature verification

---

## What's Planned

### Network MVP
- Basic relay server
- Web client prototype
- Desktop client

### Game Integration
- Simulation engine hooks
- World state synchronization
- Multiplayer foundation

---

## How to Help

This is an open project. Contributions welcome at every level:

- **Writers** — improve clarity and accessibility
- **Developers** — Rust implementation work
- **Reviewers** — find gaps, inconsistencies, edge cases
- **Translators** — make this accessible worldwide

→ [Get Involved](/Humanity/get-involved)

---

*The future is constructed by those who show up.*
