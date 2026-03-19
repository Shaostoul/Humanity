# Keys and Sessions

## Purpose
Define identity keys, device enrollment, sessions, and recovery without server-side private key decryption.

## Key types

### User identity key
- Algorithm: Ed25519 (or equivalent modern signature scheme).
- Private key: client-side only.
- Public key: stored by server and used for verification.

### Device key
- Per device keypair or equivalent proof bound to the device.
- Used to support device revocation without revoking the whole account.

### Space keys (private spaces)
- Space encryption keys used to encrypt private content.
- Distributed only to members authorized by space governance.
- Rotated when necessary (member removal, compromise).

## Authentication model
- Login establishes a session.
- Server issues short-lived session tokens bound to:
  - user identity
  - enrolled device
  - permissions
  - expiration
- Critical state changes require:
  - valid session token
  - valid signature on request payload

## Signing requirements
- Messages, posts, and moderation actions are signed objects.
- Server verifies signature before acceptance.
- Client verifies signatures on receipt before display and storage.

## Recovery model
- Recovery uses encrypted backup artifacts.
- Server may store encrypted backup artifacts.
- Server cannot decrypt backups without client-held secrets.
- Loss of both primary key access and recovery secrets is unrecoverable by design.

## Client key storage
Preferred:
- Passkeys / platform secure storage / hardware security keys (non-exportable).

Fallback:
- Encrypted keystore file unlocked locally by passphrase (optionally combined with device-bound secret).

## Device revocation
- Server maintains a list of enrolled devices.
- Revocation immediately prevents token issuance and invalidates active sessions for that device.
