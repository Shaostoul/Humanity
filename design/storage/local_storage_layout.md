# Local Storage Layout

## Purpose
Define a portable, offline-first on-disk layout suitable for:
- desktop installs
- running from removable media
- backups and restore
- minimal corruption risk

## Root folder
A single root folder per profile:
- `humanity_profile/`

## Layout
- `humanity_profile/identity/`
  - device enrollment info
  - encrypted keystore references (no plaintext private keys)
- `humanity_profile/database/`
  - encrypted SQLite database
  - write-ahead log files as required
- `humanity_profile/blocks/`
  - content-addressed blocks (attachments), encrypted when private
  - subfolders by prefix to avoid too many files in one folder
- `humanity_profile/cache/`
  - derived indexes and non-essential caches
- `humanity_profile/outbox/`
  - queued objects pending upload (encrypted at rest)
- `humanity_profile/backups/`
  - optional local backup bundles

## Portability rules
- Profile folder must be relocatable.
- Absolute paths must not be stored.
- Corruption recovery:
  - database must be recoverable from backups and server sync.

## Encryption boundaries
- Database encrypted at rest.
- Outbox encrypted at rest.
- Private blocks encrypted at rest.
- Public blocks may be stored plaintext, but must be hash-verified.

## Removable media constraints
- Minimize write amplification.
- Avoid excessive small-file churn where possible.
- Prefer batching outbox writes.
