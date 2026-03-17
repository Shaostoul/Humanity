# Encryption and Confidentiality

## Purpose
Define confidentiality requirements and encryption rules for private spaces and private messaging.

This document specifies what must be encrypted, what may remain plaintext, and what is explicitly not promised.

## Scope
Applies to:
- private spaces (forums and chat)
- direct messages
- private attachments
Does not apply to:
- public spaces where content is intended to be publicly readable

## Confidentiality goals
- Only authorized members of a private space can read private content.
- Servers and relays should not be able to read private content payloads.
- Offline-first storage encrypts private content at rest.
- Leakage is minimized where feasible.

## Non-goals (explicit)
- Perfect metadata secrecy.
- Guaranteed deletion after replication.
- Protection against a fully compromised client device that has decryption keys.

## What is encrypted

### Payload encryption
For private spaces and direct messages:
- object payload bytes are encrypted
- attachment blocks are encrypted

### What remains unencrypted
Certain header fields remain visible to enable routing and enforcement:
- version
- object_type
- space_id or channel_id
- references (object identifiers)
- author_public_key
- payload_encoding indicator (must indicate ciphertext type, not plaintext type)

Header fields must be minimal to reduce metadata leakage.

## Algorithms
- Symmetric encryption: XChaCha20-Poly1305
- Key derivation (when needed): Argon2id or HKDF (depending on source material)
- Randomness: cryptographically secure RNG per platform

## Nonce rules
- Nonces must never repeat for the same key.
- For XChaCha20-Poly1305, use 192-bit nonce generated randomly.
- Nonces are included with the ciphertext.

## Associated data
Encryption must bind key context by using associated data (AAD) including:
- space_id or channel_id
- object_type
- references (or a hash of them)
- protocol version
This prevents ciphertext reuse across contexts.

## Integrity and authenticity
- XChaCha20-Poly1305 provides ciphertext integrity.
- Objects remain signed at the object layer.
- The object signature covers the ciphertext bytes and relevant header metadata.

## Attachments
Attachment blocks in private spaces must be encrypted before block hashing.
Block identifier is computed over ciphertext bytes.
This prevents plaintext fingerprinting by block_id.

## Key separation
Never reuse the same key for:
- payload encryption
- attachment encryption
- local database encryption
Derive separate keys using a key derivation function.

## Local at-rest encryption
Clients must encrypt:
- local databases containing private space content
- local block stores containing encrypted attachments
Key management for local storage is defined in key management documents.

## Exposure controls
- Clients must not automatically render quarantined private content.
- Clients must not leak decrypted payloads into logs or crash reports.
