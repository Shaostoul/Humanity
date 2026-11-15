//! What THIS device can do, for the screens that have to say so (Android rung A1, 2026-11-14).
//!
//! The operator, 2026-11-14: "It would be nice to have the native app on my one plus nine pro
//! instead of the PWA version." The app's pages run on a phone first; a few things a desktop does
//! are not built for a phone yet, or never can be, and CLAUDE.md's hardware-notice rule (operator,
//! 2026-11-08: "We should make sure that we clearly notice and inform users when they have a GPU or
//! whatever that can't run something we're incorporating") says each one is said in words on the
//! screen where a person meets it: what it needs, what this device has, what works instead. Never
//! silently missing.
//!
//! ONE list, here, so every screen asks the same question the same way: [`Capability`] names each
//! thing, [`has`] answers for this build, and [`notice_key`] gives the words (data/i18n/en.json,
//! `device.*`). The plan and the platform facts behind each answer: docs/design/platforms.md.
//!
//! THE PHONE PREVIEW. [`phone`] is true on an Android build, and on a desktop when a test asks
//! for it ([`set_phone_preview`]) or `HUMANITY_PHONE_PREVIEW=1` is set at start, so the phone's
//! layout and its notices can be seen and checked on the operator's PC (the snapshots render the
//! pages at the phone's size with it on). Nothing else changes: a desktop in preview still has
//! its tray and its key store; only what the pages SHOW follows the preview.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

/// An Android build: the app pages on a phone (rung A1 of docs/design/platforms.md).
pub const ANDROID: bool = cfg!(target_os = "android");

/// A test's phone preview (see the module docs).
static PREVIEW: AtomicBool = AtomicBool::new(false);

/// Lay the pages out as a phone, and show the phone's notices: an Android build, or a desktop in
/// the phone preview (the module docs).
pub fn phone() -> bool {
    static ENV: OnceLock<bool> = OnceLock::new();
    ANDROID
        || PREVIEW.load(Ordering::Relaxed)
        || *ENV.get_or_init(|| std::env::var("HUMANITY_PHONE_PREVIEW").map(|v| v == "1").unwrap_or(false))
}

/// Turn the phone preview on or off (tests and snapshots; the module docs).
pub fn set_phone_preview(on: bool) {
    PREVIEW.store(on, Ordering::Relaxed);
}

/// Something a desktop does that a phone build may not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capability {
    /// The 3D world: a later rung on a phone (A5), with its own lighter settings, ASTC textures
    /// and the renderer measured on the phone's GPU first.
    World3d,
    /// Remembering the identity in the device's key store (the desktop's Windows Credential
    /// Manager, macOS Keychain or Linux Secret Service, through the `keyring` crate, which has no
    /// Android backend). On a phone it needs the Android Keystore through JNI, not built yet.
    KeyStore,
    /// Starting other programs (the Tools page, the Games library, launchers): Android lets an
    /// app start only its own code.
    LaunchPrograms,
}

/// Whether this build has `c`. Answers for what the pages SHOW, so the phone preview counts.
pub fn has(c: Capability) -> bool {
    match c {
        Capability::World3d | Capability::KeyStore | Capability::LaunchPrograms => !phone(),
    }
}

/// The i18n key of the words that say why `c` is missing here: what it needs, what this device
/// has, what works instead (data/i18n/en.json, `device.*`).
pub fn notice_key(c: Capability) -> &'static str {
    match c {
        Capability::World3d => "device.world_3d",
        Capability::KeyStore => "device.key_store",
        Capability::LaunchPrograms => "device.launch_programs",
    }
}

/// Every capability, for the test that each has its words.
pub const ALL: [Capability; 3] = [Capability::World3d, Capability::KeyStore, Capability::LaunchPrograms];

#[cfg(test)]
mod tests {
    use super::*;

    /// A desktop build has everything until the preview asks for the phone's view, and the
    /// preview takes each away. Seen red 2026-11-14 with `has` answering `true` always:
    /// "World3d in the phone preview".
    #[test]
    fn the_preview_shows_the_phone_view_and_a_desktop_keeps_everything() {
        if ANDROID {
            return;
        }
        set_phone_preview(false);
        if phone() {
            return; // HUMANITY_PHONE_PREVIEW set for this run: nothing to compare against
        }
        for c in ALL {
            assert!(has(c), "{c:?} on a desktop");
        }
        set_phone_preview(true);
        for c in ALL {
            assert!(!has(c), "{c:?} in the phone preview");
        }
        set_phone_preview(false);
    }

    /// Every capability's notice names a key under `device.`, distinct from the others.
    #[test]
    fn each_capability_has_its_own_notice_key() {
        let keys: Vec<&str> = ALL.iter().map(|c| notice_key(*c)).collect();
        for k in &keys {
            assert!(k.starts_with("device."), "{k}");
        }
        let mut sorted = keys.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), keys.len(), "two capabilities share a notice");
    }
}
