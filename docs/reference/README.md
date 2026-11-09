# Reference

Two kinds of reference material live here: operational runbooks/checklists for the
live infrastructure, and a protocol/schema specification library for the federation
object model and the farming data contracts. The specs describe designed contracts
(some already implemented in `crates/humanity-protocol/src/`, some forward-looking RFCs); check
`CLAUDE.md`'s Cryptography table for what is actually shipped and activated today.

## Operations

- `ECOSYSTEM_RELEASE_CHECKLIST.md`, tick-list checklist for every feature/fix release (companion to the procedures in `docs/SOP.md` and `docs/INCIDENT-PLAYBOOK.md`)

## Dated research findings

- [`research/2026-11-08-ai-tools/`](research/2026-11-08-ai-tools/README.md), a dated scan of about 30 AI tools from one news video: what each could do here, its licence as stated, and candidate / watch / skip (a scan, not a decision: a candidate still needs its own finding).
- [`research/2026-11-08-npc-brain-voice/`](research/2026-11-08-npc-brain-voice/README.md), open language models and voices for speaking NPCs and the in-app helper, with each publisher checked against the US Entity List and the 1260H list (a scan, not a decision).
- [`findings/2026-11-09-model-runtime.md`](findings/2026-11-09-model-runtime.md), how the app runs AI models when the runtime must be a separate, optional download: llama.cpp, audio.cpp, ONNX Runtime, candle, burn, tract and our own wgpu compared; one separate AI engine program recommended. The plan that uses it, and the approved starting set, is [`../design/embeddable-ai.md`](../design/embeddable-ai.md).
- [`research/2026-11-08-asset-packs/`](research/2026-11-08-asset-packs/README.md), open 3D model, texture and HDRI libraries ranked, with the items that fill the homes' gaps (stove, oven, shower, washer, water heater) and what no library has (life-support machines); a scan, not a decision.
- [`findings/`](findings/README.md), dated research notes on legal, regulatory and licensing questions. One file per question, written the day the research was done and never quietly rewritten, so a later reader can tell whether the world changed or we were wrong. Read `findings/README.md` before adding one.

## Federation object model (protocol specs)

Canonical CBOR signed objects, replicated across federated servers. See
`docs/design/storage-architecture.md` and `docs/network/object_format.md` for how
this fits the real storage layer.

- `RFC_TEMPLATE.md`, template for new protocol RFCs
- `canonical_cbor_rules.md`, byte-level canonical CBOR encoding rules
- `reference_implementation.md`, what qualifies as the normative reference implementation
- `local_storage_layout.md`, on-disk profile layout (identity, database, blocks, cache, outbox, backups)
- `sqlite_save_backend.md`, SQLite as the persistence backend, in-game/CLI accessible
- `backup_bundle_format.md`, portable encrypted backup bundle format
- `recovery_and_backups.md`, multi-device recovery without server-side key decryption
- `retention_and_deletion_semantics.md`, what "delete" means in a replicated system (referenced directly from `cryptography.md`)
- `keys_and_sessions.md`, identity keys, device enrollment, sessions
- `private_space_key_management.md`, key hierarchy and rotation for private spaces/DMs
- `encryption_and_confidentiality.md`, what must be encrypted vs. what stays plaintext
- `security_and_privacy_architecture.md`, draft security goals and threat model overview
- `threat_model.md`, assets, adversaries, and mandatory mitigations
- `secure_communication_constraints.md`, non-negotiable truthful-E2EE and anti-censorship constraints
- `anti_spam.md`, layered friction model against spam/sybil/harassment
- `proof_of_work_stamps.md`, optional per-message PoW friction mechanism
- `governance.md`, space sovereignty and signed moderation log model
- `moderation_action_schema.md`, canonical moderation action payload schema
- `use_of_force_constraints.md`, non-negotiable constraints on force/authority (Humanity Framework)
- `voting_integrity_constraints.md`, non-negotiable constraints on collective decision-making
- `p2p_relay_continuity_rfc.md`, RFC: hybrid P2P + relay continuity when a relay goes down
- `studio_mode_rfc.md`, RFC: multi-source streaming studio mode (BRB, vtuber support)
- `constructibles.md`, schema for construction objects in `data/construction/objects.ron`
- `test_vectors/`, conformance test vectors for canonical CBOR / object hashing / signing (placeholders pending a reference implementation)
- `entities/`, `items/`, `plots/`, `resources/`, farming data schemas, verified field-for-field against the live data files (`data/entities/plants/`, `data/plots/`, etc.)
