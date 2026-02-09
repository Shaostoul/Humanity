# Backup Bundle Format

## Purpose
Define a portable encrypted backup bundle for:
- profile migration
- multi-device restore
- resilience against local corruption

## Bundle properties
- Encrypted at rest.
- Integrity-protected.
- Versioned.

## Contents (recommended)
- encrypted database snapshot
- encrypted outbox queue (optional)
- block store manifest (list of block_ids and sizes)
- identity metadata (public keys, device enrollment metadata)
- sync cursors

Do not include plaintext private keys.

## Format
- Container: a single file bundle
- Internal structure:
  - header (version, algorithms, creation time)
  - encrypted payload section
  - authentication tag / integrity section

Encryption:
- XChaCha20-Poly1305 over the payload section.
Key derivation:
- Argon2id from a user-provided passphrase or recovery secret.

## Restore procedure
1. Verify integrity.
2. Decrypt locally.
3. Restore database and outbox.
4. Restore block manifest.
5. Fetch missing blocks from server or peers as needed.
