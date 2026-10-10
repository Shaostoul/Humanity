//! Where the protected setup (step G of docs/design/blocking-and-safe-mode.md, 10h) is kept on
//! this device: a file of its own, `protected_setup.json`, beside config.json.
//!
//! WHY NOT IN config.json, where it was kept first (found in the 2026-10-10 batch review):
//! - an unreadable config.json loads as the defaults (config.rs `load_if_exists`), and the
//!   default setup is OFF, so damage to that one file (a crash mid-write, a bad edit, a disk
//!   error) turned the lock off together with every other setting;
//! - config.json was written in place, so a crash mid-write could leave exactly that damage;
//! - a field-by-field read of the setup was impossible from inside one big struct read.
//!
//! So here: a MISSING file means off (nobody ever turned it on on this device), and a file that
//! exists but cannot be read, wholly or in part, counts as ON (`ProtectedSetup::from_stored`
//! reads each field on its own and fails closed: no PIN that matches, the rules on), so only the
//! recovery phrase can then set a new PIN. The file is written whole or not at all: to a temp
//! file first, then renamed over the old one (persistence.rs `write_atomic`), so a crash leaves
//! the old state or the new one, never half.
//!
//! What remains, said plainly as the spec does: deleting the file deletes the lock, the same as
//! deleting the app's settings does (the preset's sentence about "this app on this device").
//!
//! Nothing here is ever sent anywhere: this file is not in the vault, the self-sync notes or any
//! export (10h, "Per device, never synced").

use std::path::{Path, PathBuf};

use super::protected::ProtectedSetup;

/// The file's name, in the same folder as config.json.
pub const FILE_NAME: &str = "protected_setup.json";

/// The setup's file beside `config_path` (config.rs `AppConfig::config_path`, which honours
/// HUMANITY_DATA_DIR and portable mode, so a second profile or a portable install keeps its own).
pub fn path_beside(config_path: &Path) -> PathBuf {
    config_path.with_file_name(FILE_NAME)
}

/// The setup as `path` holds it. No file: off. A file that cannot be read (an I/O error, text
/// that is not JSON, or any field that does not read): on, failing closed field by field.
pub fn load(path: &Path) -> ProtectedSetup {
    match std::fs::read(path) {
        Ok(bytes) => {
            let value = serde_json::from_slice::<serde_json::Value>(&bytes).unwrap_or_else(|e| {
                log::warn!("The protected setup's file {} is not readable ({e}); it counts as on, with no PIN that matches", path.display());
                serde_json::Value::String(String::new())
            });
            ProtectedSetup::from_stored(value)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => ProtectedSetup::default(),
        Err(e) => {
            log::warn!("The protected setup's file {} could not be read ({e}); it counts as on, with no PIN that matches", path.display());
            ProtectedSetup::from_stored(serde_json::Value::String(String::new()))
        }
    }
}

/// Write `setup` to `path`, whole or not at all. A setup that is off, where no file exists,
/// writes nothing: a device where it was never turned on keeps no file for it. An off setup over
/// an existing file is written as off (not deleted), so a delete that fails can never leave the
/// old "on" behind looking current.
pub fn save(path: &Path, setup: &ProtectedSetup) -> Result<(), String> {
    if *setup == ProtectedSetup::default() && !path.exists() {
        return Ok(());
    }
    let json = serde_json::to_vec_pretty(setup).map_err(|e| format!("the protected setup did not serialize: {e}"))?;
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    }
    crate::persistence::write_atomic(path, &json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::protected::PinVerifier;

    /// A scratch folder of this test's own.
    fn scratch(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("protected-store-{tag}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A verifier as this app stores one (600,000 iterations), made without running them: these
    /// tests only read and write the file, they never check a PIN.
    fn stored_verifier() -> PinVerifier {
        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD;
        PinVerifier { salt: b64.encode([1u8; 16]), hash: b64.encode([2u8; 32]), iterations: 600_000 }
    }

    fn on_setup() -> ProtectedSetup {
        ProtectedSetup {
            on: true,
            pin: Some(stored_verifier()),
            identity: "ab12".into(),
            approved: vec!["ann".into(), "ben".into()],
            warnings_on_friends: true,
            pictures_hidden: true,
            public_rooms_hidden: false,
            wrong_tries: 1,
            wait_until: 0,
        }
    }

    /// ITS OWN FILE, WRITTEN WHOLE (the 2026-10-10 review, item 7): the setup goes to
    /// `protected_setup.json` beside config.json and reads back as itself; no file is off; a
    /// device that never turned it on gets no file; turning it off over an existing file writes
    /// "off" rather than deleting; the write goes through a temp file, so none is left behind.
    /// Seen red 2026-10-10 with `load` reading the whole struct at once and falling back to the
    /// defaults (what config.json's reader did): "a half-written file still counts as on, with no
    /// PIN that matches" failed.
    #[test]
    fn the_setup_has_a_file_of_its_own_written_whole() {
        let dir = scratch("own");
        let config = dir.join("config.json");
        let path = path_beside(&config);
        assert_eq!(path, dir.join("protected_setup.json"), "beside config.json");
        assert_eq!(load(&path), ProtectedSetup::default(), "no file: off");
        save(&path, &ProtectedSetup::default()).unwrap();
        assert!(!path.exists(), "never turned on: no file is made");

        save(&path, &on_setup()).unwrap();
        assert_eq!(load(&path), on_setup(), "it reads back as itself");
        let leftovers: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name()).collect();
        assert_eq!(leftovers, [std::ffi::OsString::from("protected_setup.json")], "no temp file is left behind");

        // A crash in the middle of an old-style write: half the bytes. Still on.
        let whole = std::fs::read(&path).unwrap();
        std::fs::write(&path, &whole[..whole.len() / 2]).unwrap();
        let half = load(&path);
        assert!(half.on && half.pin.is_none(), "a half-written file still counts as on, with no PIN that matches");

        save(&path, &ProtectedSetup::default()).unwrap();
        assert!(path.exists(), "turned off over a file: written as off, not deleted");
        assert!(!load(&path).on, "and reads as off");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// ONE BAD FIELD DOES NOT LOSE THE OTHERS (the review, item 7; the web reads each field the
    /// same way): a damaged `approved`, `wait_until` or rule keeps the PIN and the rest; a
    /// damaged PIN keeps the approved friends; text that is not JSON at all, or a file that
    /// cannot be read, counts as on with no PIN.
    /// Seen red 2026-10-10 with `from_stored` reading the whole struct at once (as before): "a
    /// bad approved list keeps the PIN" failed (the PIN was gone).
    #[test]
    fn one_damaged_field_does_not_lose_the_others() {
        let dir = scratch("fields");
        let path = dir.join(FILE_NAME);
        let good = serde_json::to_value(on_setup()).unwrap();
        let damaged = |field: &str, value: serde_json::Value| {
            let mut v = good.clone();
            v[field] = value;
            std::fs::write(&path, v.to_string()).unwrap();
            load(&path)
        };

        let s = damaged("approved", serde_json::json!("not a list"));
        assert!(s.on && s.pin == Some(stored_verifier()), "a bad approved list keeps the PIN");
        assert!(s.approved.is_empty() && !s.public_rooms_hidden && s.wrong_tries == 1, "and the other fields, the approved list emptied");

        let s = damaged("wait_until", serde_json::json!(-5));
        assert_eq!((s.pin.clone(), s.approved.clone(), s.wait_until), (Some(stored_verifier()), on_setup().approved, 0), "a bad wait keeps the PIN and the friends");

        let s = damaged("public_rooms_hidden", serde_json::json!("no"));
        assert!(s.public_rooms_hidden && s.pin.is_some(), "a bad rule is its safe value, the rest kept");

        let s = damaged("pin", serde_json::json!({ "salt": 5 }));
        assert!(s.on && s.pin.is_none() && s.approved == on_setup().approved, "a bad PIN is no PIN, the friends kept");

        std::fs::write(&path, b"{ this is not json").unwrap();
        let s = load(&path);
        assert!(s.on && s.pin.is_none() && s.pictures_hidden && s.warnings_on_friends, "not JSON at all: on, no PIN, the rules on");

        // A folder where the file should be: it exists but cannot be read as a file.
        let blocked = dir.join("blocked");
        std::fs::create_dir_all(blocked.join(FILE_NAME)).unwrap();
        let s = load(&blocked.join(FILE_NAME));
        assert!(s.on && s.pin.is_none(), "a file that cannot be read: on, no PIN");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
