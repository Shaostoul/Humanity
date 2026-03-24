# Client-Side Identity Keys With Server Verification

## Status
Accepted

## Context
Humanity OS needs:
- offline-first operation
- cross-device use (desktop, game, web)
- secure account validity and device revocation
- resistance to impersonation
- privacy-preserving communication modes, including private spaces

If the server can decrypt user private identity keys, then:
- a server breach compromises all user identities
- server operators (or attackers) can impersonate users
- end-to-end encryption cannot be meaningfully guaranteed

Offline-first also requires that users can sign actions without server availability.

## Decision
User identity is represented by a cryptographic keypair.

- The private identity key is created and used on the client.
- The server stores only the public identity key and account status.
- The server never needs to decrypt user private identity keys.
- Clients sign actions (messages, posts, updates). Servers verify signatures.
- Each device has a device key (or equivalent enrollment proof) that can be revoked.
- Server-issued session tokens are short-lived and bind:
  - user identity
  - enrolled device
  - permissions
  - expiration

Key recovery uses encrypted backups:
- the server may store encrypted key material as a backup artifact
- the server cannot decrypt the backup artifact without client-held secrets

Preferred protection for identity keys is a non-exportable mechanism (passkeys or hardware keys).
Where not available, use an encrypted keystore unlocked locally.

## Consequences

### Positive
- Server compromise does not automatically enable universal impersonation.
- Offline-first signing works without server access.
- End-to-end encryption is compatible with the identity model.
- Device-level revocation is possible.

### Negative
- Key recovery is harder and must be designed carefully.
- Users can lose keys if recovery is poorly implemented.
- Web clients require careful local decryption and secure storage handling.

### Non-negotiable requirements created by this decision
- All user-generated actions must be verifiable by signature.
- Server endpoints must reject unsigned or invalidly signed actions.
- Device enrollment and revocation must exist.
- Recovery procedures must be documented and tested.

## Rejected alternatives

### Server-held private keys
Rejected because it allows server-side impersonation and catastrophic breach impact.

### Password-only identity without cryptographic signing
Rejected because it weakens offline-first operation and makes tamper-proof attribution harder.

### Shared single key across all devices without device revocation
Rejected because compromise of one device compromises all devices indefinitely.

## Follow-up tasks
- Define device enrollment and revocation flows.
- Define encrypted backup format and recovery steps.
- Define minimum acceptable client key storage mechanisms for each platform.
