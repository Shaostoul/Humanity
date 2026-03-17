# Proof of Work Stamps

## Purpose
Define an optional space-configurable friction mechanism that raises the cost of spam without requiring invasive identity proofs.

This is used only when space policy enables it.

## Overview
A proof-of-work (PoW) stamp is a small computational puzzle solved by the client per message/post.
Verification must be cheap; solving must be measurably more expensive than verifying.

PoW does not replace moderation or rate limits. It is additional friction.

## Stamp fields
When PoW is enabled, relevant objects must include in their payload (or a dedicated header extension):
- pow_version: integer
- pow_difficulty: integer
- pow_nonce: bytes
- pow_hash: bytes

## Canonical challenge
The PoW challenge is derived from:
- object_id (or hash of the object with pow fields set to zero)
- space_id
- author_public_key
- pow_difficulty

Challenge bytes:
- challenge = BLAKE3("HUMANITY_POW_V1" || space_id || author_public_key || object_id || pow_difficulty)

## Condition
Client must find pow_nonce such that:
- pow_hash = BLAKE3(challenge || pow_nonce)
- pow_hash has at least pow_difficulty leading zero bits

## Verification
Verification is:
- recompute challenge
- recompute pow_hash
- check leading zero bits

## Policy controls
Space policy may set:
- require_proof_of_work: boolean
- proof_of_work_difficulty: integer
- exemptions: roles that do not require PoW (e.g., trusted members)

## Anti-abuse notes
- PoW must be combined with rate limits.
- Difficulty must be adjustable to avoid excluding low-power devices.
- Clients may precompute stamps while offline to smooth UX.
