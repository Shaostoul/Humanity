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
//! **On a phone** (Android rung A1, 2026-11-20; operator: "We should add that.") both modes keep
//! the same secret, but Android's key store holds keys rather than secrets: the 32 bytes are
//! sealed with AES-256-GCM by a key it makes and never lets out (in the phone's secure hardware
//! when it has some), and the sealed file sits in the app's private folder ([`KeyStoreKind`],
//! `platform::key_wrap`, docs/reference/cryptography.md).
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
#[cfg(not(target_os = "android"))]
const KEYCHAIN_SERVICE: &str = "HumanityOS";

/// Build the per-identity keychain account name. Different identities on
/// the same OS account get distinct slots, so switching identities does
/// not require clearing the keychain manually.
/// (A phone names its entries the same way, as files: `platform::key_wrap::entry_name`.)
#[cfg(not(target_os = "android"))]
fn keychain_account(slot: KeychainSlot, public_key_hex: &str) -> String {
    // Short prefix of the pubkey hex (32 chars) is enough to disambiguate
    // identities; full Dilithium hex is 3904 chars which OS keychains
    // sometimes truncate / reject as the account name. Cut at a character, not a byte, so a
    // hand-edited key with a wide character there is never a panic (2026-11-20; the same 32 bytes
    // as before for every real, hex, key).
    let end = public_key_hex.char_indices().nth(32).map_or(public_key_hex.len(), |(i, _)| i);
    format!("{}/{}", slot.tag(), &public_key_hex[..end])
}

/// Distinct keychain slots HumanityOS uses. Adding a new tag is forward-
/// compatible; rename means clearing existing entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

    /// Every slot, so a switch can forget the ones the new mode does not use.
    pub const ALL: [KeychainSlot; 2] = [KeychainSlot::Seed, KeychainSlot::DeviceKey];
}

// ── Switching modes: the one way every switch goes ───────────────────────

/// Where the remembering modes keep their 32 bytes: [`ThisDevice`] is this build's own key store
/// (the computer's, or Android's through `platform::key_wrap::KeyFileStore`); the tests use
/// stand-ins, so the rules below run on every PC, the phone's file store included.
pub trait SecretStore {
    fn stash(&mut self, slot: KeychainSlot, public_key_hex: &str, secret: &[u8; 32]) -> Result<(), String>;
    fn load(&mut self, slot: KeychainSlot, public_key_hex: &str) -> Result<Option<[u8; 32]>, String>;
    fn clear(&mut self, slot: KeychainSlot, public_key_hex: &str) -> Result<(), String>;
    /// Whether the slot holds a secret for the identity. This default opens it to see and wipes
    /// the copy; a store that can tell without opening it (the phone's: is the file there) does.
    fn holds(&mut self, slot: KeychainSlot, public_key_hex: &str) -> Result<bool, String> {
        Ok(self.load(slot, public_key_hex)?.map(|mut s| crate::platform::key_wrap::wipe(&mut s)).is_some())
    }
}

/// This build's key store: [`keychain_stash`], [`keychain_load`] and [`keychain_clear`].
pub struct ThisDevice;

impl SecretStore for ThisDevice {
    fn stash(&mut self, slot: KeychainSlot, public_key_hex: &str, secret: &[u8; 32]) -> Result<(), String> {
        keychain_stash(slot, public_key_hex, secret)
    }
    fn load(&mut self, slot: KeychainSlot, public_key_hex: &str) -> Result<Option<[u8; 32]>, String> {
        keychain_load(slot, public_key_hex)
    }
    fn clear(&mut self, slot: KeychainSlot, public_key_hex: &str) -> Result<(), String> {
        keychain_clear(slot, public_key_hex)
    }
}

/// The phone's store answers the same three calls.
impl<S: crate::platform::key_wrap::Sealer> SecretStore for crate::platform::key_wrap::KeyFileStore<S> {
    fn stash(&mut self, slot: KeychainSlot, public_key_hex: &str, secret: &[u8; 32]) -> Result<(), String> {
        self.keep(slot.tag(), public_key_hex, secret).map(|_| ())
    }
    fn load(&mut self, slot: KeychainSlot, public_key_hex: &str) -> Result<Option<[u8; 32]>, String> {
        self.read(slot.tag(), public_key_hex)
    }
    fn clear(&mut self, slot: KeychainSlot, public_key_hex: &str) -> Result<(), String> {
        self.forget(slot.tag(), public_key_hex)
    }
    fn holds(&mut self, slot: KeychainSlot, public_key_hex: &str) -> Result<bool, String> {
        crate::platform::key_wrap::KeyFileStore::holds(self, slot.tag(), public_key_hex)
    }
}

/// Why a switch did not fully happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RememberError {
    /// The new mode's secret could not be kept: nothing changed, the old mode still works.
    NotKept(String),
    /// The new mode's secret is kept, but what another mode kept could not be forgotten.
    NotForgotten(String),
}

impl RememberError {
    /// The sentence the screen shows (data/i18n/en.json), never the error's own text: on a phone
    /// that can be a Java stack trace, which goes to the log only (the review's finding 6).
    pub fn words_key(&self) -> &'static str {
        match self {
            RememberError::NotKept(_) => "unlock.stash_failed",
            RememberError::NotForgotten(_) => "unlock.forget_failed",
        }
    }

    /// The details, for the log.
    pub fn details(&self) -> &str {
        match self {
            RememberError::NotKept(e) | RememberError::NotForgotten(e) => e,
        }
    }
}

/// Switch the key store to `mode` for the identity `public_key_hex`: keep `secret` in the mode's
/// slot (the seed for Remember on this device, the device key for Quick PIN; `None` for Always
/// ask), then forget every other slot. Every switch goes through here, on a desktop and on a
/// phone, from Settings and from the unlock and PIN dialogs, so no switch leaves another mode's
/// secret behind (BUG-335: Remember, then Quick PIN, left the seed remembered).
pub fn remember(store: &mut impl SecretStore, mode: AutoUnlockMode, public_key_hex: &str, secret: Option<&[u8; 32]>) -> Result<(), RememberError> {
    let keep = remembered_slot(mode);
    if public_key_hex.is_empty() {
        // No identity: nothing can be remembered, and nothing was (every entry is kept under an
        // identity), so a mode that keeps nothing is reached with nothing to forget, which is not
        // a failure (the second review: Always ask was refused before anything was looked at).
        return match keep {
            None => Ok(()),
            Some(_) => Err(RememberError::NotKept("no identity loaded".into())),
        };
    }
    if let Some(slot) = keep {
        let secret = secret.ok_or_else(|| RememberError::NotKept(format!("no secret given for {mode:?}")))?;
        store.stash(slot, public_key_hex, secret).map_err(RememberError::NotKept)?;
    }
    forget_others(store, mode, public_key_hex)
}

/// Forget every slot `mode` does not use, for the identity: the second half of [`remember`], and
/// on its own Settings' Try again after a forget that failed (nothing new is kept, so no secret is
/// needed). Every slot is asked to forget, held or not, so a store that cannot be asked says so.
pub fn forget_others(store: &mut impl SecretStore, mode: AutoUnlockMode, public_key_hex: &str) -> Result<(), RememberError> {
    forget_slots(store, mode, public_key_hex, false)
}

/// Forget the slots `mode` does not use; with `only_held`, only those the store says hold a
/// secret, and a store that cannot say is left alone (start-up, which must not raise an alert on
/// every launch of a computer that has no key store at all).
fn forget_slots(store: &mut impl SecretStore, mode: AutoUnlockMode, public_key_hex: &str, only_held: bool) -> Result<(), RememberError> {
    if public_key_hex.is_empty() {
        return Ok(());
    }
    let keep = remembered_slot(mode);
    let mut failed = Vec::new();
    for slot in KeychainSlot::ALL {
        if Some(slot) == keep {
            continue;
        }
        if only_held {
            match store.holds(slot, public_key_hex) {
                Ok(true) => {}
                Ok(false) => continue,
                Err(e) => {
                    log::info!("unlock on launch: could not look for a leftover {}: {e}", slot.tag());
                    continue;
                }
            }
        }
        match store.clear(slot, public_key_hex) {
            Ok(()) if only_held => log::info!("unlock on launch: forgot the {} another way to unlock left behind", slot.tag()),
            Ok(()) => {}
            Err(e) => failed.push(format!("{}: {e}", slot.tag())),
        }
    }
    if failed.is_empty() { Ok(()) } else { Err(RememberError::NotForgotten(failed.join("; "))) }
}

/// What start-up finds in the key store for the saved `mode`.
#[derive(Debug, PartialEq, Eq)]
pub enum StartUp {
    /// Always ask: nothing to look for.
    Nothing,
    /// The mode's secret (the seed, or the Quick PIN's device key).
    Loaded([u8; 32]),
    /// The store holds nothing for the mode (cleared, a new device, a file that arrived from
    /// another phone): the mode is not remembered any more, so the app treats it as Always ask and
    /// Settings offers to remember again in one click.
    Forgotten,
    /// The store could not be asked (no backend, a locked keyring): the mode stands, and the
    /// passphrase unlocks.
    Failed(String),
}

/// What start-up found for the saved mode, and what it did about another mode's leftovers.
#[derive(Debug, PartialEq, Eq)]
pub struct StartUpReport {
    pub found: StartUp,
    /// The slots the saved mode does not use, each looked at and, when it still held a secret (a
    /// forget that failed before, a switch cut short), forgotten: `Ok` when none held one or each
    /// is forgotten now, [`RememberError::NotForgotten`] when one still could not be, which the
    /// screen says (Settings, with Try again). So a failed forget does not end with the app.
    pub leftovers: Result<(), RememberError>,
}

/// Ask the store at start-up what it holds for `mode`, and forget what it still holds for the
/// other modes.
pub fn start_up(store: &mut impl SecretStore, mode: AutoUnlockMode, public_key_hex: &str) -> StartUpReport {
    let leftovers = forget_slots(store, mode, public_key_hex, true);
    let found = match remembered_slot(mode) {
        None => StartUp::Nothing,
        Some(slot) => match store.load(slot, public_key_hex) {
            Ok(Some(secret)) => StartUp::Loaded(secret),
            Ok(None) => StartUp::Forgotten,
            Err(e) => StartUp::Failed(e),
        },
    };
    StartUpReport { found, leftovers }
}

// ── Which key store, and what each mode keeps in it ──────────────────────

/// The key store this build remembers with. The two remembering modes keep the SAME secret in
/// either ([`remembered_slot`]); only where it is kept, and so the words Settings shows, differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyStoreKind {
    /// The computer's own: Windows Credential Manager, macOS Keychain or the Linux Secret
    /// Service, through the `keyring` crate, which keeps the 32 bytes itself.
    Desktop,
    /// Android's key store (rung A1, 2026-11-20): it keeps a sealing key that never leaves the
    /// phone, and the 32 bytes are sealed by it into the app's private folder
    /// (`platform::key_wrap`, `platform::android_jni`).
    Android,
}

impl KeyStoreKind {
    /// This build's key store.
    pub fn this_build() -> KeyStoreKind {
        if cfg!(target_os = "android") {
            KeyStoreKind::Android
        } else {
            KeyStoreKind::Desktop
        }
    }

    /// The key store whose words Settings shows: a phone's when the pages show a phone's view
    /// (`platform::device::phone()`, which the phone preview turns on for a desktop), else this
    /// build's.
    pub fn for_view(phone_view: bool) -> KeyStoreKind {
        if phone_view { KeyStoreKind::Android } else { KeyStoreKind::this_build() }
    }
}

/// What a mode keeps in the key store: the seed (Remember on this device), the device key the
/// Quick PIN needs, or nothing. One answer for every platform: this is the contract the Android
/// key store matches.
pub fn remembered_slot(mode: AutoUnlockMode) -> Option<KeychainSlot> {
    match mode {
        AutoUnlockMode::AlwaysPrompt => None,
        AutoUnlockMode::Keychain => Some(KeychainSlot::Seed),
        AutoUnlockMode::KeychainPin => Some(KeychainSlot::DeviceKey),
    }
}

/// The words under a remembering mode's choice in Settings, for the key store they go to
/// (data/i18n/en.json); `None` for the mode that remembers nothing. Both modes are matched on a
/// phone: Android's key store does what a desktop's does for them, by sealing instead of keeping.
/// Where a phone gives less than its words promise (a key store without secure hardware), the
/// protection words say so ([`crate::platform::key_wrap::Protection::words_key`]).
pub fn mode_words(mode: AutoUnlockMode, kind: KeyStoreKind) -> Option<&'static str> {
    match (remembered_slot(mode)?, kind) {
        (KeychainSlot::Seed, KeyStoreKind::Desktop) => Some("unlock.remember_desktop"),
        (KeychainSlot::Seed, KeyStoreKind::Android) => Some("unlock.remember_android"),
        (KeychainSlot::DeviceKey, KeyStoreKind::Desktop) => Some("unlock.pin_desktop"),
        (KeychainSlot::DeviceKey, KeyStoreKind::Android) => Some("unlock.pin_android"),
    }
}

/// Every mode, for the tests.
pub const ALL_MODES: [AutoUnlockMode; 3] = [AutoUnlockMode::AlwaysPrompt, AutoUnlockMode::Keychain, AutoUnlockMode::KeychainPin];

// ── Android's key store (rung A1, 2026-11-20) ────────────────────────────

/// The phone's backend for the same three calls the desktop's key store answers. The `keyring`
/// crate has no Android backend (its in-memory mock would forget the key when the app closed), so
/// on a phone the 32 bytes are SEALED by a key Android's key store makes and keeps, and the sealed
/// bytes live in `<storage root>/key-store/` (docs/reference/cryptography.md has the row). All of
/// the file handling is `platform::key_wrap::KeyFileStore`, which the host tests run with a
/// stand-in key store; only the four calls into Android's key store (`JniSealer`) are phone-only.
/// A load that finds no file, or a file this phone's key store has no key for, is `Ok(None)`, as a
/// desktop's missing entry is; anything else that goes wrong is `Err`, and the caller falls back
/// to the passphrase, which always works.
#[cfg(target_os = "android")]
mod android_key_store {
    use super::KeychainSlot;
    use crate::platform::{android_jni as jni, key_wrap as kw};

    /// Android's key store, through the app's Java (`KeyStoreBridge`).
    struct JniSealer;

    impl kw::Sealer for JniSealer {
        fn create(&mut self, alias: &str) -> Result<kw::KeyInfo, String> {
            jni::key_create(alias).map(kw::KeyInfo::from_code)
        }
        fn seal(&mut self, alias: &str, plain: &[u8], aad: &[u8]) -> Result<Vec<u8>, String> {
            jni::key_seal(alias, plain, aad)
        }
        fn unseal(&mut self, alias: &str, nonce: &[u8], sealed: &[u8], aad: &[u8]) -> Result<Option<Vec<u8>>, String> {
            jni::key_unseal(alias, nonce, sealed, aad)
        }
        fn forget(&mut self, alias: &str) -> Result<(), String> {
            jni::key_forget(alias)
        }
    }

    fn store() -> Result<kw::KeyFileStore<JniSealer>, String> {
        let dir = crate::storage::os_root().map(|r| r.join(kw::DIR)).ok_or_else(|| "the app has no private folder".to_string())?;
        Ok(kw::KeyFileStore { dir, sealer: JniSealer })
    }

    /// What the key store said about the sealing key of the last remember on this phone, from the
    /// entry's file (for the words in Settings; `None` when nothing is remembered).
    pub fn key_info(slot: KeychainSlot, public_key_hex: &str) -> Option<kw::KeyInfo> {
        store().ok()?.key_info(slot.tag(), public_key_hex)
    }

    pub fn keychain_stash(slot: KeychainSlot, public_key_hex: &str, bytes: &[u8; 32]) -> Result<(), String> {
        let info = store()?.keep(slot.tag(), public_key_hex, bytes)?;
        log::info!("key store: remembered the {} ({info:?})", slot.tag());
        Ok(())
    }

    pub fn keychain_load(slot: KeychainSlot, public_key_hex: &str) -> Result<Option<[u8; 32]>, String> {
        store()?.read(slot.tag(), public_key_hex)
    }

    pub fn keychain_clear(slot: KeychainSlot, public_key_hex: &str) -> Result<(), String> {
        store()?.forget(slot.tag(), public_key_hex)
    }
}
#[cfg(target_os = "android")]
pub use android_key_store::{keychain_clear, keychain_load, keychain_stash};

/// Where this phone keeps the sealing key of `slot`'s remembered secret, and whether Android
/// refuses it while the phone is locked, for the words in Settings: `None` on a desktop, or when
/// nothing is remembered.
pub fn key_info(slot: KeychainSlot, public_key_hex: &str) -> Option<crate::platform::key_wrap::KeyInfo> {
    #[cfg(target_os = "android")]
    {
        android_key_store::key_info(slot, public_key_hex)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (slot, public_key_hex);
        None
    }
}

/// Stash 32 bytes in the OS keychain under the given slot+identity.
/// Returns Err(_) on platform failure (no keyring backend available,
/// permissions denied, etc.) — caller MUST handle by leaving the user on
/// the passphrase path.
#[cfg(not(target_os = "android"))]
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
    let encoded = B64.encode(bytes);
    let set = entry.set_password(&encoded);
    // The base64 copy of the secret is wiped once handed over (2026-11-20).
    crate::platform::key_wrap::wipe(&mut encoded.into_bytes());
    set.map_err(|e| format!("keychain set: {}", e))
}

/// Load 32 bytes from the OS keychain. Returns Ok(None) when the entry
/// is genuinely absent (user cleared it, new device, etc.) so the caller
/// can route to the passphrase prompt without treating a missing entry as
/// an error. Returns Err for actual platform failures.
#[cfg(not(target_os = "android"))]
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
    // Both plaintext copies (the base64 text and the decoded bytes) are wiped once the secret is
    // out of them (2026-11-20).
    let mut password = password.into_bytes();
    let decoded = B64.decode(&password);
    crate::platform::key_wrap::wipe(&mut password);
    let mut bytes = decoded.map_err(|e| format!("keychain b64 decode: {}", e))?;
    let out = <[u8; 32]>::try_from(bytes.as_slice()).map_err(|_| format!("keychain entry malformed: expected 32 bytes, got {}", bytes.len()));
    crate::platform::key_wrap::wipe(&mut bytes);
    out.map(Some)
}

/// Remove a keychain entry. Used by the Settings "Forget this device"
/// button and when switching modes. Missing entries are NOT an error
/// (idempotent).
#[cfg(not(target_os = "android"))]
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
    // The PIN with the device key, and the key made from them, are wiped once used (2026-11-20).
    crate::platform::key_wrap::wipe(&mut key_input);

    let mut iv = [0u8; 12];
    getrandom::getrandom(&mut iv).map_err(|e| format!("RNG failed: {}", e))?;

    let cipher = Aes256Gcm::new(GenericArray::from_slice(&derived_key));
    crate::platform::key_wrap::wipe(&mut derived_key);
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
    crate::platform::key_wrap::wipe(&mut key_input);

    let cipher = Aes256Gcm::new(GenericArray::from_slice(&derived_key));
    crate::platform::key_wrap::wipe(&mut derived_key);
    let nonce = GenericArray::from_slice(iv);
    let mut plaintext = cipher.decrypt(nonce, ciphertext)
        .map_err(|_| "Wrong PIN or corrupted data".to_string())?;

    let out = <[u8; 32]>::try_from(plaintext.as_slice())
        .map_err(|_| format!("Decrypted seed has wrong length: expected 32, got {}", plaintext.len()));
    crate::platform::key_wrap::wipe(&mut plaintext);
    out
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

    /// The phone matches each desktop mode: the same secret goes to its key store for the same
    /// mode, and each remembering mode has its own words for each key store, present in
    /// data/i18n/en.json, as do the protection words a phone adds. Seen red 2026-11-20 with
    /// `remembered_slot` giving the Quick PIN the seed's slot ("KeychainPin keeps
    /// Some(Seed), the device key expected") and before the words were in en.json
    /// ("unlock.remember_desktop is not in data/i18n/en.json").
    #[test]
    fn a_phone_remembers_what_a_desktop_remembers_and_says_where() {
        assert_eq!(remembered_slot(AutoUnlockMode::AlwaysPrompt), None);
        assert_eq!(remembered_slot(AutoUnlockMode::Keychain), Some(KeychainSlot::Seed), "Keychain keeps the seed");
        assert_eq!(
            remembered_slot(AutoUnlockMode::KeychainPin),
            Some(KeychainSlot::DeviceKey),
            "KeychainPin keeps {:?}, the device key expected",
            remembered_slot(AutoUnlockMode::KeychainPin)
        );
        let en: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data/i18n/en.json")).unwrap(),
        )
        .unwrap();
        let has = |key: &str| en.pointer(&format!("/{}", key.replace('.', "/"))).and_then(|v| v.as_str()).is_some_and(|s| !s.is_empty());
        let mut seen = Vec::new();
        for mode in ALL_MODES {
            for kind in [KeyStoreKind::Desktop, KeyStoreKind::Android] {
                let words = mode_words(mode, kind);
                assert_eq!(words.is_some(), mode.wants_startup_unlock(), "{mode:?} on {kind:?}");
                if let Some(key) = words {
                    assert!(has(key), "{key} is not in data/i18n/en.json");
                    assert!(!seen.contains(&key), "{key} is used twice");
                    seen.push(key);
                }
            }
        }
        for p in crate::platform::key_wrap::Protection::ALL {
            assert!(has(p.words_key()), "{} is not in data/i18n/en.json", p.words_key());
        }
    }

    /// Settings shows a phone's words in a phone's view (the phone preview on a desktop included)
    /// and this build's otherwise. Replaces an assertion the review found unable to fail
    /// (`this_build()` compared with the very `cfg!` it is written as): this one compares with the
    /// answer a desktop must give. Seen red 2026-11-20 with `this_build` answering Android
    /// everywhere: "a desktop build shows the desktop's key store words".
    #[test]
    fn settings_shows_the_key_store_of_the_view() {
        assert_eq!(KeyStoreKind::for_view(true), KeyStoreKind::Android, "the phone preview shows a phone's words");
        #[cfg(not(target_os = "android"))]
        assert_eq!(KeyStoreKind::for_view(false), KeyStoreKind::Desktop, "a desktop build shows the desktop's key store words");
    }

    /// The host's stand-in for a desktop key store: the keyring crate's own mock forgets an entry
    /// between two `Entry::new` calls, so it cannot show what a switch leaves behind.
    #[derive(Default)]
    struct MockStore {
        kept: std::collections::BTreeMap<(&'static str, String), [u8; 32]>,
        /// Refuse every clear, as a locked keyring does.
        refuse_clear: bool,
    }

    impl SecretStore for MockStore {
        fn stash(&mut self, slot: KeychainSlot, pk: &str, secret: &[u8; 32]) -> Result<(), String> {
            self.kept.insert((slot.tag(), pk.to_string()), *secret);
            Ok(())
        }
        fn load(&mut self, slot: KeychainSlot, pk: &str) -> Result<Option<[u8; 32]>, String> {
            Ok(self.kept.get(&(slot.tag(), pk.to_string())).copied())
        }
        fn clear(&mut self, slot: KeychainSlot, pk: &str) -> Result<(), String> {
            if self.refuse_clear {
                return Err("the keyring is locked".into());
            }
            self.kept.remove(&(slot.tag(), pk.to_string()));
            Ok(())
        }
    }

    const PK: &str = "abcdef0123456789abcdef0123456789abcdef";

    /// Remember on this device, then Quick PIN, leaves no seed in the store, on a desktop's store
    /// and through the phone's file store; and every other switch keeps exactly the new mode's
    /// secret (BUG-335). Seen red 2026-11-20 with `remember` doing what the PIN dialog did before
    /// (keep the device key, leave the seed): "Remember -> Quick PIN left the seed in the store
    /// (MockStore)" (the stand-in desktop store is checked first; the file store runs the same
    /// `remember`).
    #[test]
    fn switching_from_remember_to_quick_pin_forgets_the_seed() {
        fn check(store: &mut impl SecretStore, which: &str) {
            let (seed, device_key) = ([1u8; 32], [2u8; 32]);
            remember(store, AutoUnlockMode::Keychain, PK, Some(&seed)).unwrap();
            assert_eq!(store.load(KeychainSlot::Seed, PK).unwrap(), Some(seed), "{which}: Remember keeps the seed");
            remember(store, AutoUnlockMode::KeychainPin, PK, Some(&device_key)).unwrap();
            assert_eq!(store.load(KeychainSlot::Seed, PK).unwrap(), None, "Remember -> Quick PIN left the seed in the store ({which})");
            assert_eq!(store.load(KeychainSlot::DeviceKey, PK).unwrap(), Some(device_key), "{which}: the PIN's device key is kept");
            remember(store, AutoUnlockMode::Keychain, PK, Some(&seed)).unwrap();
            assert_eq!(store.load(KeychainSlot::DeviceKey, PK).unwrap(), None, "Quick PIN -> Remember left the device key ({which})");
            remember(store, AutoUnlockMode::AlwaysPrompt, PK, None).unwrap();
            for slot in KeychainSlot::ALL {
                assert_eq!(store.load(slot, PK).unwrap(), None, "Always ask left {slot:?} ({which})");
            }
            assert!(matches!(remember(store, AutoUnlockMode::Keychain, PK, None), Err(RememberError::NotKept(_))), "{which}: no seed, nothing kept");
        }
        check(&mut MockStore::default(), "MockStore");
        let dir = std::env::temp_dir().join(format!("hos-auto-unlock-switch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut files = crate::platform::key_wrap::KeyFileStore { dir: dir.clone(), sealer: crate::platform::key_wrap::FakeSealer::default() };
        check(&mut files, "the phone's file store");
        assert!(files.sealer.keys.is_empty(), "Always ask left a sealing key in the key store: {:?}", files.sealer.keys);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Switching to Always ask always works, an identity loaded or not: with none there is nothing
    /// to forget, which is not a failure. A remembering mode still needs an identity to remember
    /// under. Seen red 2026-11-20 with the check `set_unlock_mode` had (an empty identity is
    /// NotKept before anything is looked at), moved here unchanged: "Always ask with no identity
    /// is refused: Err(NotKept(\"no identity loaded\"))".
    #[test]
    fn switching_to_always_ask_with_no_identity_is_not_a_failure() {
        let mut store = MockStore::default();
        let got = remember(&mut store, AutoUnlockMode::AlwaysPrompt, "", None);
        assert_eq!(got, Ok(()), "Always ask with no identity is refused: {got:?}");
        assert!(matches!(remember(&mut store, AutoUnlockMode::Keychain, "", Some(&[1u8; 32])), Err(RememberError::NotKept(_))), "nothing is remembered under no identity");
        assert!(store.kept.is_empty(), "nothing was written under an empty identity: {:?}", store.kept.keys());
    }

    /// Start-up treats a mode whose secret is gone as not remembered, and a store it could not ask
    /// as a reason to leave the mode alone. Through the phone's file store, a remembered file whose
    /// key this phone's key store lacks is such a gone secret (the review's finding 3).
    #[test]
    fn start_up_says_when_a_remembered_mode_is_gone() {
        let mut store = MockStore::default();
        assert_eq!(start_up(&mut store, AutoUnlockMode::AlwaysPrompt, PK).found, StartUp::Nothing);
        assert_eq!(start_up(&mut store, AutoUnlockMode::Keychain, PK).found, StartUp::Forgotten);
        store.stash(KeychainSlot::Seed, PK, &[4u8; 32]).unwrap();
        assert_eq!(start_up(&mut store, AutoUnlockMode::Keychain, PK).found, StartUp::Loaded([4u8; 32]));
        assert_eq!(start_up(&mut store, AutoUnlockMode::KeychainPin, PK).found, StartUp::Forgotten, "the PIN's device key is not there");

        let dir = std::env::temp_dir().join(format!("hos-auto-unlock-start-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut files = crate::platform::key_wrap::KeyFileStore { dir: dir.clone(), sealer: crate::platform::key_wrap::FakeSealer::default() };
        remember(&mut files, AutoUnlockMode::Keychain, PK, Some(&[6u8; 32])).unwrap();
        files.sealer.keys.clear(); // the file arrived on a phone whose key store never made its key
        assert_eq!(start_up(&mut files, AutoUnlockMode::Keychain, PK).found, StartUp::Forgotten);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Start-up forgets what another mode left in the store, so a forget that failed does not end
    /// with the app (the second review's A: the alert promised a retry that a restart lost), and
    /// says so when it still cannot. A slot that holds nothing is not touched, so a store that
    /// refuses every clear raises nothing on a launch with nothing to forget. Seen red 2026-11-20
    /// with start-up not looking at the other slots (`leftovers` always `Ok`): "a leftover it could
    /// not forget is said: Ok(())".
    #[test]
    fn start_up_forgets_another_modes_leftover_and_says_when_it_cannot() {
        let mut store = MockStore::default();
        store.stash(KeychainSlot::Seed, PK, &[7u8; 32]).unwrap();
        store.refuse_clear = true;
        let report = start_up(&mut store, AutoUnlockMode::AlwaysPrompt, PK);
        assert_eq!(report.found, StartUp::Nothing);
        assert!(matches!(report.leftovers, Err(RememberError::NotForgotten(_))), "a leftover it could not forget is said: {:?}", report.leftovers);
        store.refuse_clear = false;
        assert_eq!(start_up(&mut store, AutoUnlockMode::AlwaysPrompt, PK).leftovers, Ok(()));
        assert_eq!(store.load(KeychainSlot::Seed, PK).unwrap(), None, "start-up left the seed Remember kept while the mode is Always ask");

        // The saved mode's own secret stays; only the other mode's goes.
        store.stash(KeychainSlot::DeviceKey, PK, &[8u8; 32]).unwrap();
        store.stash(KeychainSlot::Seed, PK, &[9u8; 32]).unwrap();
        assert_eq!(start_up(&mut store, AutoUnlockMode::KeychainPin, PK).found, StartUp::Loaded([8u8; 32]));
        assert_eq!(store.load(KeychainSlot::Seed, PK).unwrap(), None, "Quick PIN at start-up still had the seed");
        store.refuse_clear = true;
        assert_eq!(start_up(&mut store, AutoUnlockMode::KeychainPin, PK).leftovers, Ok(()), "nothing held is nothing to say");
        assert_eq!(start_up(&mut store, AutoUnlockMode::AlwaysPrompt, "").leftovers, Ok(()), "no identity, nothing looked at");

        // The phone's store: a key it could not delete leaves the file, and the next start-up
        // finishes the job.
        let dir = std::env::temp_dir().join(format!("hos-auto-unlock-leftover-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut files = crate::platform::key_wrap::KeyFileStore { dir: dir.clone(), sealer: crate::platform::key_wrap::FakeSealer::default() };
        remember(&mut files, AutoUnlockMode::Keychain, PK, Some(&[6u8; 32])).unwrap();
        files.sealer.fail_forget = true;
        assert!(matches!(remember(&mut files, AutoUnlockMode::AlwaysPrompt, PK, None), Err(RememberError::NotForgotten(_))));
        files.sealer.fail_forget = false;
        assert_eq!(start_up(&mut files, AutoUnlockMode::AlwaysPrompt, PK).leftovers, Ok(()));
        assert_eq!(files.load(KeychainSlot::Seed, PK).unwrap(), None, "the next start-up forgot the seed");
        assert!(files.sealer.keys.is_empty(), "and its sealing key: {:?}", files.sealer.keys);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Try again after a failed forget needs no secret: it forgets the slots the mode does not
    /// use and keeps the one it does.
    #[test]
    fn forget_others_keeps_the_modes_own_secret() {
        let mut store = MockStore::default();
        store.stash(KeychainSlot::Seed, PK, &[1u8; 32]).unwrap();
        store.stash(KeychainSlot::DeviceKey, PK, &[2u8; 32]).unwrap();
        forget_others(&mut store, AutoUnlockMode::Keychain, PK).unwrap();
        assert_eq!(store.load(KeychainSlot::Seed, PK).unwrap(), Some([1u8; 32]));
        assert_eq!(store.load(KeychainSlot::DeviceKey, PK).unwrap(), None);
    }

    /// The screen's sentence for a failed switch is words by key with no place for the error's
    /// text, so a Java stack trace never reaches it (the review's finding 6). Seen red 2026-11-20
    /// against the old words: "unlock.stash_failed shows the error's text: Your key could not be
    /// remembered: {{why}}. Your passphrase still unlocks it."
    #[test]
    fn a_failed_switch_says_a_sentence_not_the_error() {
        let en: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data/i18n/en.json")).unwrap(),
        )
        .unwrap();
        let trace = "java.security.ProviderException: Keystore operation failed\n0: android.security.KeyStore2.getKeyEntry";
        for e in [RememberError::NotKept(trace.into()), RememberError::NotForgotten(trace.into())] {
            let key = e.words_key();
            let words = en.pointer(&format!("/{}", key.replace('.', "/"))).and_then(|v| v.as_str()).unwrap_or_else(|| panic!("{key} is not in en.json"));
            assert!(!words.contains("{{"), "{key} shows the error's text: {words}");
            assert!(words.contains("log"), "{key} must say the details are in the app's log: {words}");
            assert_eq!(e.details(), trace, "the details still reach the log");
        }
    }

    /// The phone's Quick PIN words say plainly what the PIN does: it is checked by HumanityOS
    /// (PBKDF2, the key store sets no user-authentication rule) (the review's finding 4). They
    /// promise nothing about a locked phone, because Android's unlocked-only rule
    /// (`setUnlockedDeviceRequired`) exists from Android 9 only (the app runs from 8.0) and a phone
    /// may refuse it, when KeyStoreBridge falls back to a key without it; instead Settings shows
    /// what THIS phone's key carries, from the file ([`crate::platform::key_wrap::KeyInfo`]), and
    /// the words for "carries it" say it needs a screen lock (PIN, pattern or password) (the second
    /// review's D). Seen red 2026-11-20 against the words that promised the rule to every phone:
    /// "the PIN words promise Android's locked-phone rule, which Android 8 and a phone that fell
    /// back do not give".
    #[test]
    fn the_phone_pin_words_say_what_android_does_and_does_not() {
        let en: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data/i18n/en.json")).unwrap(),
        )
        .unwrap();
        let word = |key: &str| en.pointer(&format!("/{}", key.replace('.', "/"))).and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let words = word("unlock.pin_android");
        assert!(words.contains("checked by HumanityOS") && words.contains("not by Android"), "the PIN words must say the PIN is checked by HumanityOS, not by Android: {words}");
        assert!(!words.contains("phone is locked"), "the PIN words promise Android's locked-phone rule, which Android 8 and a phone that fell back do not give: {words}");
        use crate::platform::key_wrap::KeyInfo;
        let on = word(KeyInfo { protection: crate::platform::key_wrap::Protection::SecureHardware, unlocked_only: true }.rule_words_key());
        assert!(on.contains("PIN, pattern or password") && on.contains("swipe"), "the rule's words must say it needs a screen lock: {on}");
        let off = word(KeyInfo { protection: crate::platform::key_wrap::Protection::SecureHardware, unlocked_only: false }.rule_words_key());
        assert!(off.contains("Android 9"), "the words for a key without the rule must say why: {off}");
        let java = std::fs::read_to_string(std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("android/java/us/united_humanity/humanityos/KeyStoreBridge.java")).unwrap();
        assert!(java.contains("setUnlockedDeviceRequired(true)"), "KeyStoreBridge must try the unlocked-only rule");
        assert!(
            java.contains(&format!("UNLOCKED_ONLY = 0x{:x};", crate::platform::key_wrap::UNLOCKED_ONLY)),
            "KeyStoreBridge must report the rule with the bit key_wrap reads (UNLOCKED_ONLY)"
        );
    }

    /// A hand-edited identity with a wide character where the desktop cuts its account name at 32
    /// characters is never a panic (the review's finding 8). Seen red 2026-11-20 with the byte
    /// slice `[..32]`: "byte index 32 is not a char boundary".
    #[cfg(not(target_os = "android"))]
    #[test]
    fn the_desktop_account_name_never_panics() {
        let pk = format!("{}é{}", "a".repeat(31), "b".repeat(10));
        assert_eq!(keychain_account(KeychainSlot::Seed, &pk), format!("seed/{}é", "a".repeat(31)));
        assert_eq!(keychain_account(KeychainSlot::DeviceKey, PK), "device-key/abcdef0123456789abcdef0123456789");
        assert_eq!(keychain_account(KeychainSlot::Seed, "00ff"), "seed/00ff");
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
