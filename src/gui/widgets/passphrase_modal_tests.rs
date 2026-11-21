// Child of widgets/passphrase_modal.rs (#[path]): Unlock on App Launch through the GUI's own
// switch (the second review of the Android key store fixes, 2026-11-20). Each test runs the
// function a button calls, with a stand-in key store and saves kept off disk, and measures what
// it leaves in the state and the store, never what it was told.
//
// RED FIRST, 2026-11-20, each against the code as the review found it (moved behind these
// functions unchanged):
// - always_ask_with_no_identity_switches: "Always ask with no identity is refused:
//   Err(\"unlock.stash_failed\")" (set_unlock_mode's own check refused it before anything).
// - a_failed_forget_is_said_and_try_again_finishes_it: "a retry that fails again says so: left:
//   Ok(())" (no retry existed: an already-selected Always ask did nothing; run against a Try
//   again that did nothing).
// - skip_drops_the_remember_tick: "Skip left the Remember tick on, so a later Reveal recovery
//   phrase would remember the seed unasked".
// - a_failed_remember_after_unlock_is_said_in_words: "the unlock dialog's failed Remember is only
//   a log line: []".

use super::*;
use humanity_core::auto_unlock::{AutoUnlockMode, KeychainSlot, SecretStore};

/// A key store that can refuse to keep or to forget.
#[derive(Default)]
struct Store {
    kept: std::collections::HashMap<String, [u8; 32]>,
    refuse_stash: bool,
    refuse_clear: bool,
}

fn entry(slot: KeychainSlot, pk: &str) -> String {
    format!("{slot:?}/{pk}")
}

impl SecretStore for Store {
    fn stash(&mut self, slot: KeychainSlot, pk: &str, secret: &[u8; 32]) -> Result<(), String> {
        if self.refuse_stash {
            return Err("java.security.ProviderException: Keystore operation failed".into());
        }
        self.kept.insert(entry(slot, pk), *secret);
        Ok(())
    }
    fn load(&mut self, slot: KeychainSlot, pk: &str) -> Result<Option<[u8; 32]>, String> {
        Ok(self.kept.get(&entry(slot, pk)).copied())
    }
    fn clear(&mut self, slot: KeychainSlot, pk: &str) -> Result<(), String> {
        if self.refuse_clear {
            return Err("the keyring is locked".into());
        }
        self.kept.remove(&entry(slot, pk));
        Ok(())
    }
}

const PK: &str = "abcdef0123456789abcdef0123456789abcdef";

fn state(pk: &str) -> GuiState {
    crate::config::keep_saves_off_disk();
    let mut s = GuiState::default();
    s.profile_public_key = pk.into();
    s
}

/// Switching to Always ask always works: with no identity loaded there is nothing to forget, and
/// the PIN blob still goes.
#[test]
fn always_ask_with_no_identity_switches() {
    let mut state = state("");
    state.auto_unlock_mode = AutoUnlockMode::KeychainPin;
    state.pin_encrypted_seed = "blob".into();
    state.pin_salt = "salt".into();
    let got = set_unlock_mode_in(&mut Store::default(), &mut state, AutoUnlockMode::AlwaysPrompt, None, None);
    assert_eq!(got, Ok(()), "Always ask with no identity is refused: {got:?}");
    assert_eq!(state.auto_unlock_mode, AutoUnlockMode::AlwaysPrompt);
    assert!(state.pin_encrypted_seed.is_empty() && state.pin_salt.is_empty(), "the PIN blob went with the mode");
    assert_eq!(state.unlock_alert, None);
}

/// A forget that fails is said (Settings' alert), the switch still happens, and Try again
/// finishes it with no secret needed, taking the alert away.
#[test]
fn a_failed_forget_is_said_and_try_again_finishes_it() {
    let mut store = Store::default();
    let mut state = state(PK);
    set_unlock_mode_in(&mut store, &mut state, AutoUnlockMode::Keychain, Some(&[1u8; 32]), None).unwrap();
    store.refuse_clear = true;
    assert_eq!(set_unlock_mode_in(&mut store, &mut state, AutoUnlockMode::AlwaysPrompt, None, None), Err("unlock.forget_failed"));
    assert_eq!(state.auto_unlock_mode, AutoUnlockMode::AlwaysPrompt, "the switch still happens");
    assert_eq!(state.unlock_alert, Some("unlock.forget_failed"), "Settings says so");
    assert_eq!(forget_other_modes_in(&mut store, &mut state), Err("unlock.forget_failed"), "a retry that fails again says so");
    store.refuse_clear = false;
    assert_eq!(forget_other_modes_in(&mut store, &mut state), Ok(()));
    assert_eq!(store.load(KeychainSlot::Seed, PK).unwrap(), None, "Try again left the seed Remember kept");
    assert_eq!(state.unlock_alert, None, "the alert goes once it is forgotten");
}

/// Skipping the unlock dialog drops its Remember tick, so the next unlock that is not the
/// dialog's (Settings, Account's Reveal recovery phrase) cannot remember the seed unasked.
#[test]
fn skip_drops_the_remember_tick() {
    let mut state = state(PK);
    state.passphrase_needed = true;
    state.remember_on_device = true;
    state.passphrase_input = "correct horse".into();
    skip_unlock(&mut state);
    assert!(!state.remember_on_device, "Skip left the Remember tick on, so a later Reveal recovery phrase would remember the seed unasked");
    assert!(!state.passphrase_needed && state.passphrase_input.is_empty());
}

/// The unlock dialog's Remember tick that fails says so in words (a notice now, and Settings'
/// alert), never the store's own error, and changes nothing.
#[test]
fn a_failed_remember_after_unlock_is_said_in_words() {
    let mut store = Store { refuse_stash: true, ..Default::default() };
    let mut state = state(PK);
    remember_after_unlock(&mut store, &mut state, &[3u8; 32], 1.0);
    let words = humanity_core::i18n::t("unlock.stash_failed");
    let shown: Vec<&str> = state.toasts.iter().map(|t| t.text.as_str()).collect();
    assert!(shown.contains(&words), "the unlock dialog's failed Remember is only a log line: {shown:?}");
    assert!(!shown.iter().any(|t| t.contains("Exception")), "the store's own error reached the screen: {shown:?}");
    assert_eq!(state.unlock_alert, Some("unlock.stash_failed"));
    assert_eq!(state.auto_unlock_mode, AutoUnlockMode::AlwaysPrompt, "nothing changed");
    assert!(store.kept.is_empty());
}
