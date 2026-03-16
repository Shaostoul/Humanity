# Recovery and Backups

## Purpose
Define encrypted backup artifacts, multi-device restore, and recovery without server-side decryption of identity keys.

## Recovery goals
- Users can use multiple devices.
- Users can recover from device loss.
- Server compromise does not automatically expose identity private keys.
- Loss conditions are explicit and understood.

## Key materials
- Identity private key (signing): must remain client-side in plaintext.
- Local data encryption key: used to encrypt local databases and cached content.
- Device keys: per device, revocable.

## Backup artifacts
### Encrypted key backup artifact
Contains:
- encrypted identity private key (only if identity key is exportable in chosen model)
- metadata (version, key algorithm, creation time)
- integrity checks

Rules:
- encrypted with a user-held recovery secret
- server may store the encrypted artifact
- server cannot decrypt without recovery secret

Preferred model:
- Use passkeys / non-exportable identity keys.
- In that model, backup artifact stores:
  - device enrollment recovery methods
  - encrypted local data key backups
  - not the non-exportable identity key itself

### Encrypted local data backups
Contains:
- encrypted SQLite database snapshot
- encrypted attachment block store index (optional)
- sync cursors and pending outbound queue (optional)
- integrity checks

## Recovery secrets
At least one must exist:
- passkey / platform secure recovery
- hardware security key
- recovery phrase (high-entropy)
- printed recovery codes

Recovery secrets must not be stored unencrypted on the same thumb drive as the backup artifact.

## Restore flow (multi-device)
1. User authenticates to account validity service.
2. User retrieves encrypted backup artifact (from server or other storage).
3. User unlocks artifact locally with recovery secret.
4. User enrolls new device key and links it to the account.
5. Client restores local data backups if present.
6. Client syncs from server to fill gaps.

## Loss conditions
Unrecoverable outcomes occur if:
- identity key is exportable and the only copy is lost and there is no encrypted backup artifact
- recovery secret is lost and the only backup artifact is encrypted under that secret
- all enrolled devices are lost/revoked and no recovery method exists

These are design constraints, not bugs.

## Thumb drive considerations
- A thumb drive is storage, not a secure element.
- Storing raw private keys on a thumb drive is prohibited.
- Storing encrypted backup artifacts on a thumb drive is allowed.
- Unlocking requires a separate secret not stored on the same drive in plaintext.

## Rotation
If compromise is suspected:
- revoke compromised devices
- rotate space encryption keys for private spaces where feasible
- issue new recovery artifacts
