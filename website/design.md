---
layout: default
title: Design
permalink: /design
---

# Design

Binding technical constraints and specifications. These documents define what implementations must satisfy.

---

## Foundational Constraints

- [Data Model](/Humanity/design/data-model) — how reality is represented as structured data
- [Testing Philosophy](/Humanity/design/testing) — what correctness means
- [Asset Rules](/Humanity/design/assets) — constraints on content and media

---

## Interface Constraints

- [UI Invariants](/Humanity/design/ui-invariants) — consent, control, and visibility requirements
- [Transparency Hooks](/Humanity/design/transparency-hooks) — structural observability requirements

---

## Security Constraints

- [Secure Communication Constraints](/Humanity/design/secure-communication) — privacy, encryption, and truthful claims
- [Voting Integrity Constraints](/Humanity/design/voting-integrity) — one human, one vote, verifiable
- [Use of Force Constraints](/Humanity/design/use-of-force) — limits on coercive authority

---

## Network Architecture

The full network specification lives in the repository under `design/network/`:

- Object format and encoding (CBOR, BLAKE3, Ed25519)
- Membership, roles, and moderation schemas
- Encryption and key management
- Offline-first synchronization
- Hybrid transport with relay fallback

→ [View on GitHub](https://github.com/Shaostoul/Humanity/tree/main/design/network)

---

## Architecture Decisions

Key decisions are recorded as Architecture Decision Records:

→ [View ADRs on GitHub](https://github.com/Shaostoul/Humanity/tree/main/design/architecture_decisions)

---

*Design defines what must be true. Implementations make it real.*
