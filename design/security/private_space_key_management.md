# Private Space Key Management

## Purpose
Define how encryption keys are created, distributed, stored, rotated, and revoked for private spaces and direct messages.

## Scope
Applies to:
- private spaces (space-scoped content)
- private channels within spaces
- direct messages

## Roles
- Space owner and moderators govern membership.
- Key distribution follows membership decisions.

## Key types

### Space master key (SMK)
- Root key for a private space.
- Used only to derive subordinate keys.
- Not used directly to encrypt content.

### Channel encryption keys (CEK)
- Derived from SMK for each channel, or created independently per channel.
- Used to encrypt message payloads.

### Attachment encryption keys (AEK)
- Derived per attachment or per channel.
- Used to encrypt attachment blocks.

### Key encryption keys (KEK)
- Used to encrypt and distribute SMK/CEK/AEK to members.
- Per recipient or per device, depending on model.

## Distribution model

### Preferred model: per-device distribution
- Each enrolled device has a device public key suitable for encryption.
- Space keys are wrapped (encrypted) to each device.

This allows device revocation without rotating the entire space immediately.

### Acceptable model: per-identity distribution
- Keys are wrapped to a user's identity encryption key.
- Device revocation requires rotating keys more often.

## Cryptographic wrapping
Keys are distributed using authenticated encryption to recipients.
Acceptable approaches:
- X25519 key agreement + symmetric wrap
- or libsodium sealed boxes

Exact scheme must be implemented consistently and tested.

## Membership changes

### Adding a member
- New member devices receive wrapped SMK (or current CEKs) after approval.
- New member cannot read history unless policy allows and keys for history are provided.

### Removing a member
Removal requires one of:

1. Security-first rotation:
- rotate SMK and all derived keys
- rewrap and redistribute to remaining members
- future content is protected from removed member

2. Practical rotation:
- rotate CEKs for active channels only
- do not guarantee protection of old history

The system must support security-first rotation as an option.

## Compromise response
If compromise is suspected:
- revoke compromised devices
- rotate affected keys
- rewrap to remaining devices
- mark a compromise event in governance logs for audit

## Storage on clients
- Space keys are stored encrypted at rest.
- Decryption requires the client’s local unlock mechanism (passkey / secure storage / encrypted keystore).

## Offline-first constraints
- A member device must retain keys locally to read and write offline.
- Key distribution events are synced and applied when online.
- Writes performed offline must queue ciphertext objects that can be decrypted by recipients once delivered.

## Direct messages
Direct messages use per-conversation keys:
- derive a shared key via key agreement
- rotate periodically or per message batch
- store and protect keys like CEKs

## Non-negotiable requirements
- Server and relay must not require plaintext access to private payloads.
- Key distribution must be auditable in space governance logs.
- Key rotation must be supported.
