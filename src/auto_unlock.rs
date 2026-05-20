//! Auto-unlock: let the user opt into silent startup without typing the
//! full BIP39-derived passphrase every launch.
//!
//! **Threat model.** The seed (32 bytes that re-derive Ed25519 + Dilithium +
//! Kyber) is the master secret. The user's passphrase-encrypted vault
//! (`AppConfig::encrypted_private_key`, PBKDF2-SHA256 600k iters → AES-GCM)
//! is the "cold" backup, always present, always usable. This module adds
//! TWO additional unlock paths that consume the same seed:
//!
//! - **Keychain** — the OS keychain (Windows DPAPI / macOS Keychain Services
//!   / Linux Secret Service) stores the raw 32-byte seed. The OS encrypts it
//!   at rest, scoped to the logged-in user account. Auto-unlocks silently.
//! - **KeychainPin** — the keychain stores a randomly-generated 32-byte
//!   *device key*; the seed lives in `AppConfig` encrypted via
//!   `AES-GCM(key = PBKDF2(pin ‖ device_key, salt, 600k iters))`. Cold-disk
//!   theft of the config without the keychain is useless; cold-theft of the
//!   keychain entry without the config is useless. Combined theft requires
//!   the user to crack the PIN against 600k PBKDF2 iters per guess.
//!
//! **Why not bake the OS keychain into one mode?** Keychain alone is the
//! lowest-friction option but ALSO the lowest barrier — anyone who can
//! unlock the user's OS session (sleep timer, spousal access, etc.) opens
//! the app silently. The PIN tier exists for the middle ground: still
//! short, still no 64-char passphrase, but a tiny barrier against
//! opportunistic access. Both opt-in, default is `AlwaysPrompt`.
//!
//! **Failure mode parity.** Every call site that loads from the keychain
//! tolerates a missing entry (user cleared their Credential Manager,
//! reinstalled OS, etc.) — the caller MUST fall back to the passphrase
//! prompt. The passphrase is the only path that has to keep working;
//! losing keychain access is annoying, never destructive.

#![cfg(feature = "native")]

use serde::{Deserialize, Serialize};

/// How the user wants the seed unlocked on app launch.
///
/// `AlwaysPrompt` is the only safe default for a fresh install: it preserves
/// the pre-v0.278.0 behavior so no existing user is surprised by an auto-
/// unlock they didn't opt into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AutoUnlockMode {
    /// Type the full BIP39 passphrase every time. Most secure for shared /
    /// public machines and the only mode pre-v0.278.0 supported.
    #[default]
    AlwaysPrompt,
    /// Seed sits in the OS keychain. App reads it on launch with no UX.
    /// Tied to the OS user account. Recommended for personal machines.
    Keychain,
    /// Short PIN (4-6 digits) gates a keychain-stored device key. Less
    /// friction than the full passphrase, more friction than zero.
    KeychainPin,
}

impl AutoUnlockMode {
    /// True when this mode wants startup code to TRY to load silently
    /// (or via short PIN) rather than show the full passphrase modal.
    pub fn wants_startup_unlock(&self) -> bool {
        !matches!(self, AutoUnlockMode::AlwaysPrompt)
    }

    /// Human-readable name for Settings UI.
    pub fn label(&self) -> &'static str {
        match self {
            AutoUnlockMode::AlwaysPrompt => "Always ask for passphrase",
            AutoUnlockMode::Keychain => "Remember on this device",
            AutoUnlockMode::KeychainPin => "Quick PIN (4-6 digits)",
        }
    }
}

// ── Keychain backend ─────────────────────────────────────────────────────

/// Service name used for all HumanityOS keychain entries. The OS keychain
/// browser (Windows Credential Manager UI, macOS Keychain Access) groups
/// entries by service, so all our entries appear together.
const KEYCHAIN_SERVICE: &str = "HumanityOS";

/// Build the per-identity keychain account name. Different identities on
/// the same OS account get distinct slots, so switching identities does
/// not require clearing the keychain manually.
fn keychain_account(slot: KeychainSlot, public_key_hex: &str) -> String {
    // Short prefix of the pubkey hex (32 chars) is enough to disambiguate
    // identities; full Dilithium hex is 3904 chars which OS keychains
    // sometimes truncate / reject as the account name.
    let short = if public_key_hex.len() > 32 {
        &public_key_hex[..32]
    } else {
        public_key_hex
    };
    format!("{}/{}", slot.tag(), short)
}

/// Distinct keychain slots HumanityOS uses. Adding a new tag is forward-
/// compatible; rename means clearing existing entries.
#[derive(Debug, Clone, Copy)]
pub enum KeychainSlot {
    /// The raw 32-byte seed (Keychain mode).
    Seed,
    /// The 32-byte device key (KeychainPin mode). Useless without the
    /// PIN-encrypted seed in `AppConfig`.
    DeviceKey,
}

impl KeychainSlot {
    fn tag(self) -> &'static str {
        match self {
            KeychainSlot::Seed => "seed",
            KeychainSlot::DeviceKey => "device-key",
        }
    }
}

/// Stash 32 bytes in the OS keychain under the given slot+identity.
/// Returns Err(_) on platform failure (no keyring backend available,
/// permissions denied, etc.) — caller MUST handle by leaving the user on
/// the passphrase path.
pub fn keychain_stash(slot: KeychainSlot, public_key_hex: &str, bytes: &[u8; 32]) -> Result<(), String> {
    let account = keychain_account(slot, public_key_hex);
    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, &account)
        .map_err(|e| format!("keychain entry build: {}", e))?;
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD as B64;
    // The keyring crate's password API is string-typed across all OSes
    // (DPAPI accepts arbitrary bytes but Secret Service / Keychain go
    // through string APIs at one layer or another). Base64 keeps the
    // bytes intact across all three backends without UTF-8 worries.
    entry.set_password(&B64.encode(bytes)).map_err(|e| format!("keychain set: {}", e))?;
    Ok(())
}

/// Load 32 bytes from the OS keychain. Returns Ok(None) when the entry
/// is genuinely absent (user cleared it, new device, etc.) so the caller
/// can route to the passphrase prompt without treating a missing entry as
/// an error. Returns Err for actual platform failures.
pub fn keychain_load(slot: KeychainSlot, public_key_hex: &str) -> Result<Option<[u8; 32]>, String> {
    let account = keychain_account(slot, public_key_hex);
    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, &account)
        .map_err(|e| format!("keychain entry build: {}", e))?;
    let password = match entry.get_password() {
        Ok(p) => p,
        Err(keyring::Error::NoEntry) => return Ok(None),
        Err(e) => return Err(format!("keychain get: {}", e)),
    };
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD as B64;
    let bytes = B64.decode(&password).map_err(|e| format!("keychain b64 decode: {}", e))?;
    if bytes.len() != 32 {
        return Err(format!("keychain entry malformed: expected 32 bytes, got {}", bytes.len()));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(Some(out))
}

/// Remove a keychain entry. Used by the Settings "Forget this device"
/// button and when switching modes. Missing entries are NOT an error
/// (idempotent).
pub fn keychain_clear(slot: KeychainSlot, public_key_hex: &str) -> Result<(), String> {
    let account = keychain_account(slot, public_key_hex);
    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, &account)
        .map_err(|e| format!("keychain entry build: {}", e))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("keychain delete: {}", e)),
    }
}

// ── PIN crypto ───────────────────────────────────────────────────────────

/// PBKDF2 iteration count for the PIN flow. Matches the passphrase vault
/// (v0.277.0's `PBKDF2_ITERATIONS_NEW`). The PIN's low entropy is the
/// brute-force concern, but combined-cold-theft (config + keychain) still
/// has to chew through 600_000 iters per guess against a 10^4..10^6
/// keyspace — feasible on a GPU but not trivial, and the device key in
/// the keychain bumps the keyspace from "PIN-only" to "PIN ‖ 32 random
/// bytes you don't have."
pub const PIN_PBKDF2_ITERS: u32 = 600_000;

/// Encrypt the seed using PIN + device_key. The device_key MUST already
/// be stored in the OS keychain by the caller — losing it makes the
/// returned `(encrypted, salt)` blob unrecoverable except via the
/// passphrase-encrypted backup.
///
/// Returns `(encrypted_base64, salt_base64)`. Caller stashes those in
/// `AppConfig.pin_encrypted_seed` + `pin_salt`.
pub fn encrypt_seed_with_pin(seed: &[u8; 32], pin: &str, device_key: &[u8; 32]) -> Result<(String, String), String> {
    use aes_gcm::{Aes256Gcm, KeyInit, aead::Aead};
    use aes_gcm::aead::generic_array::GenericArray;
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD as B64;

    if pin.is_empty() {
        return Err("PIN cannot be empty".to_string());
    }

    let mut salt = [0u8; 16];
    getrandom::getrandom(&mut salt).map_err(|e| format!("RNG failed: {}", e))?;

    // Key material: PIN bytes ‖ device_key. The device_key half is
    // adversary-unknown without keychain access, so even a cracked PIN
    // alone doesn't yield the AES key.
    let mut key_input = Vec::with_capacity(pin.len() + 32);
    key_input.extend_from_slice(pin.as_bytes());
    key_input.extend_from_slice(device_key);

    let mut derived_key = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(&key_input, &salt, PIN_PBKDF2_ITERS, &mut derived_key);

    let mut iv = [0u8; 12];
    getrandom::getrandom(&mut iv).map_err(|e| format!("RNG failed: {}", e))?;

    let cipher = Aes256Gcm::new(GenericArray::from_slice(&derived_key));
    let nonce = GenericArray::from_slice(&iv);
    let ciphertext = cipher.encrypt(nonce, seed.as_slice())
        .map_err(|e| format!("PIN encrypt failed: {}", e))?;

    let mut combined = Vec::with_capacity(12 + ciphertext.len());
    combined.extend_from_slice(&iv);
    combined.extend_from_slice(&ciphertext);

    Ok((B64.encode(&combined), B64.encode(&salt)))
}

/// Decrypt the seed using PIN + device_key. Wrong PIN, missing/wrong
/// device_key, and corrupted blob all surface as the same "Wrong PIN"
/// error — AES-GCM auth failure is the only signal and it's deliberately
/// ambiguous to avoid leaking which input was bad.
pub fn decrypt_seed_with_pin(encrypted: &str, salt: &str, pin: &str, device_key: &[u8; 32]) -> Result<[u8; 32], String> {
    use aes_gcm::{Aes256Gcm, KeyInit, aead::Aead};
    use aes_gcm::aead::generic_array::GenericArray;
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD as B64;

    let combined = B64.decode(encrypted)
        .map_err(|e| format!("PIN blob b64 decode: {}", e))?;
    if combined.len() < 12 + 16 {
        return Err("PIN blob too short".to_string());
    }
    let (iv, ciphertext) = combined.split_at(12);

    let salt_bytes = B64.decode(salt)
        .map_err(|e| format!("PIN salt b64 decode: {}", e))?;

    let mut key_input = Vec::with_capacity(pin.len() + 32);
    key_input.extend_from_slice(pin.as_bytes());
    key_input.extend_from_slice(device_key);

    let mut derived_key = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(&key_input, &salt_bytes, PIN_PBKDF2_ITERS, &mut derived_key);

    let cipher = Aes256Gcm::new(GenericArray::from_slice(&derived_key));
    let nonce = GenericArray::from_slice(iv);
    let plaintext = cipher.decrypt(nonce, ciphertext)
        .map_err(|_| "Wrong PIN or corrupted data".to_string())?;

    if plaintext.len() != 32 {
        return Err(format!("Decrypted seed has wrong length: expected 32, got {}", plaintext.len()));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&plaintext);
    Ok(out)
}

/// Generate a fresh 32-byte device key for the KeychainPin mode. Called
/// once per identity when the user first sets up a PIN.
pub fn generate_device_key() -> Result<[u8; 32], String> {
    let mut k = [0u8; 32];
    getrandom::getrandom(&mut k).map_err(|e| format!("RNG failed: {}", e))?;
    Ok(k)
}

// ── PIN validation helpers ───────────────────────────────────────────────

/// Minimum PIN length we accept at setup time. Below 4 is too low to be
/// even a token barrier; the OS keychain still gives single-credential
/// protection in that case but we shouldn't pretend the PIN is doing work.
pub const PIN_MIN_LEN: usize = 4;

/// Maximum PIN length. Above ~12 the user might as well use a passphrase;
/// we cap to keep the input feeling like a PIN rather than a password.
pub const PIN_MAX_LEN: usize = 12;

/// Validate a candidate PIN. Returns Ok(()) when it passes, Err with a
/// user-friendly message otherwise. Enforces length and digit-only
/// (digits-only is what users expect from "PIN"; allowing punctuation
/// would be silently letting them set a short weak passphrase).
pub fn validate_pin(pin: &str) -> Result<(), String> {
    if pin.len() < PIN_MIN_LEN {
        return Err(format!("PIN must be at least {} digits.", PIN_MIN_LEN));
    }
    if pin.len() > PIN_MAX_LEN {
        return Err(format!("PIN must be at most {} digits.", PIN_MAX_LEN));
    }
    if !pin.chars().all(|c| c.is_ascii_digit()) {
        return Err("PIN must be digits only (0-9).".to_string());
    }
    Ok(())
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_round_trip() {
        let seed = [42u8; 32];
        let dk = [1u8; 32];
        let (enc, salt) = encrypt_seed_with_pin(&seed, "1234", &dk).unwrap();
        let decrypted = decrypt_seed_with_pin(&enc, &salt, "1234", &dk).unwrap();
        assert_eq!(decrypted, seed);
    }

    #[test]
    fn wrong_pin_rejects() {
        let seed = [7u8; 32];
        let dk = [9u8; 32];
        let (enc, salt) = encrypt_seed_with_pin(&seed, "111111", &dk).unwrap();
        assert!(decrypt_seed_with_pin(&enc, &salt, "222222", &dk).is_err());
    }

    #[test]
    fn wrong_device_key_rejects() {
        // The core security claim: a cracked PIN without the device key
        // does NOT yield the seed. This is what makes the PIN tier
        // meaningfully safer than "short passphrase" alone.
        let seed = [3u8; 32];
        let dk = [4u8; 32];
        let attacker_dk = [5u8; 32]; // attacker has no keychain access
        let (enc, salt) = encrypt_seed_with_pin(&seed, "9999", &dk).unwrap();
        assert!(decrypt_seed_with_pin(&enc, &salt, "9999", &attacker_dk).is_err());
    }

    #[test]
    fn salts_differ_per_encrypt() {
        // Each encrypt rolls a fresh salt. Same PIN + device_key + seed
        // must NOT produce identical ciphertext — would leak that the
        // identity didn't change across re-encrypts.
        let seed = [11u8; 32];
        let dk = [22u8; 32];
        let (_enc1, salt1) = encrypt_seed_with_pin(&seed, "0000", &dk).unwrap();
        let (_enc2, salt2) = encrypt_seed_with_pin(&seed, "0000", &dk).unwrap();
        assert_ne!(salt1, salt2);
    }

    #[test]
    fn validate_pin_accepts_4_to_12_digits() {
        assert!(validate_pin("1234").is_ok());
        assert!(validate_pin("123456789012").is_ok());
    }

    #[test]
    fn validate_pin_rejects_short() {
        assert!(validate_pin("123").is_err());
        assert!(validate_pin("").is_err());
    }

    #[test]
    fn validate_pin_rejects_long() {
        assert!(validate_pin("1234567890123").is_err());
    }

    #[test]
    fn validate_pin_rejects_non_digit() {
        assert!(validate_pin("12ab").is_err());
        assert!(validate_pin("1234-5678").is_err());
    }

    #[test]
    fn empty_pin_rejected_at_encrypt() {
        let dk = [0u8; 32];
        let seed = [0u8; 32];
        assert!(encrypt_seed_with_pin(&seed, "", &dk).is_err());
    }

    #[test]
    fn mode_label_strings_stable() {
        // Settings UI relies on these strings — changing them breaks the
        // existing user's chosen mode if it's stored by label rather than
        // by enum tag. We store by enum tag (serde) so labels are free
        // to change, but the test keeps them grep-able.
        assert_eq!(AutoUnlockMode::AlwaysPrompt.label(), "Always ask for passphrase");
        assert_eq!(AutoUnlockMode::Keychain.label(), "Remember on this device");
        assert_eq!(AutoUnlockMode::KeychainPin.label(), "Quick PIN (4-6 digits)");
    }

    #[test]
    fn wants_startup_unlock_only_for_opted_in_modes() {
        assert!(!AutoUnlockMode::AlwaysPrompt.wants_startup_unlock());
        assert!(AutoUnlockMode::Keychain.wants_startup_unlock());
        assert!(AutoUnlockMode::KeychainPin.wants_startup_unlock());
    }

    #[test]
    fn default_is_always_prompt() {
        // Pre-v0.278.0 configs serialize without the field; serde default
        // gives them AlwaysPrompt so no existing user gets auto-unlocked
        // without explicit opt-in.
        let mode = AutoUnlockMode::default();
        assert_eq!(mode, AutoUnlockMode::AlwaysPrompt);
    }
}
