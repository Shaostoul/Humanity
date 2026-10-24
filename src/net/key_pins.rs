//! The key cards this app has checked and pinned (2026-10-22, docs/design/key-cards.md). No socket
//! and no GUI, so every rule has a unit test; the half that acts on the app is
//! src/engine/key_cards.rs, and the card itself is src/relay/core/key_card.rs.
//!
//! WHAT A PIN IS. For each person, the Kyber768 key from the first good card this app saw for them
//! (a card signed by their own identity key, checked here, whatever server delivered it) and that
//! card's time. Everything this app seals to them is sealed to that key. A later good card:
//! - with the same key changes nothing (its time is noted, so it is not fetched again);
//! - with a different key and a newer time is a CHANGE: kept beside the pin, and nothing is sealed
//!   to them until the person using the app accepts it ("safety number changed" in other apps);
//! - with a different key and an older or equal time is ignored (an old card sent again).
//! A change waiting is dropped when a card newer than it brings the pinned key back.
//!
//! WHERE IT LIVES. One encrypted file per identity on this device, beside the block list
//! (`%APPDATA%/HumanityOS/dms/`), NOT one per server: a key is the same person on every server,
//! and a card is good from any server. AES-256-GCM under a key derived from the seed (its own
//! domain), so a copied file names nobody without the seed; its name is a hash. The server never
//! holds any of it: who you have sealed things to is your social graph.

use std::collections::HashMap;
use std::path::PathBuf;

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng as AesOsRng},
    AeadCore, Aes256Gcm, Key, Nonce,
};
use serde::{Deserialize, Serialize};

use crate::relay::core::key_card::{compare, KeyCard, Seen};

/// BLAKE3 domain for the file's encryption key. Distinct from every other seed-derived key.
const PINS_KEY_DOMAIN: &str = "hum/key-pins/v1";

/// A pinned key, or a changed one waiting: the Kyber768 key (base64, as the card carries it) and
/// the time on the card it came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pin {
    pub kyber: String,
    pub at: u64,
}

/// What is encrypted into the file.
#[derive(Debug, Default, Serialize, Deserialize)]
struct PinsBody {
    /// person -> the key sealed to.
    #[serde(default)]
    pins: HashMap<String, Pin>,
    /// person -> a newer different key, waiting to be accepted.
    #[serde(default)]
    changes: HashMap<String, Pin>,
}

/// The pinned key cards of one identity on this device.
pub struct KeyPins {
    path: PathBuf,
    key: [u8; 32],
    me: String,
    body: PinsBody,
}

impl KeyPins {
    /// Load (or start empty) the pins of `identity_hex`, keyed by its seed.
    pub fn load(seed: &[u8], identity_hex: &str) -> Self {
        let tag = blake3::hash(format!("key-pins\n{identity_hex}").as_bytes());
        let path = super::dm_store::DmStore::store_dir().join(format!("{}.keypins", &tag.to_hex()[..24]));
        Self::load_at(seed, identity_hex, path)
    }

    fn load_at(seed: &[u8], identity_hex: &str, path: PathBuf) -> Self {
        let mut pins = Self { path, key: blake3::derive_key(PINS_KEY_DOMAIN, seed), me: identity_hex.to_string(), body: PinsBody::default() };
        if let Ok(raw) = std::fs::read(&pins.path) {
            if raw.len() > 12 {
                let (nonce, ct) = raw.split_at(12);
                let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&pins.key));
                // A file that does not open (damaged, or another seed's) starts empty: every card is
                // then checked and pinned afresh, which is safe, and the file is replaced on save.
                if let Ok(plain) = cipher.decrypt(Nonce::from_slice(nonce), ct) {
                    if let Ok(body) = serde_json::from_slice::<PinsBody>(&plain) {
                        pins.body = body;
                    }
                }
            }
        }
        pins
    }

    /// Pins in their own file under the temp directory, for tests (never the real dms folder).
    #[cfg(test)]
    pub(crate) fn in_temp(seed: &[u8], identity_hex: &str, tag: &str) -> Self {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        Self::load_at(seed, identity_hex, std::env::temp_dir().join(format!("hum-keypins-test-{tag}-{nanos}.keypins")))
    }

    /// The same file loaded again (a restart), for tests.
    #[cfg(test)]
    pub(crate) fn reloaded(&self, seed: &[u8]) -> Self {
        Self::load_at(seed, &self.me, self.path.clone())
    }

    /// Persist (encrypt, write to a temp file, rename over the old one).
    pub fn save(&self) {
        let Ok(json) = serde_json::to_vec(&self.body) else { return };
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));
        let nonce = Aes256Gcm::generate_nonce(&mut AesOsRng);
        let Ok(ct) = cipher.encrypt(&nonce, json.as_slice()) else { return };
        let mut out = Vec::with_capacity(12 + ct.len());
        out.extend_from_slice(nonce.as_slice());
        out.extend_from_slice(&ct);
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = self.path.with_extension("tmp");
        if std::fs::write(&tmp, &out).is_ok() {
            let _ = std::fs::rename(&tmp, &self.path);
        }
    }

    /// Whose pins these are.
    pub fn identity(&self) -> &str {
        &self.me
    }

    /// Take a good card (the caller checked it: `key_card::card_for(card, &card.key)`) by the pin
    /// rule, saving when anything changed. Returns what it meant.
    pub fn take(&mut self, card: &KeyCard) -> Seen {
        let person = card.key.as_str();
        let seen = compare(self.body.pins.get(person).map(|p| (p.kyber.as_str(), p.at)), &card.kyber, card.at);
        let changed = match seen {
            Seen::First => {
                self.body.pins.insert(person.to_string(), Pin { kyber: card.kyber.clone(), at: card.at });
                true
            }
            Seen::Same => {
                let mut changed = false;
                if let Some(pin) = self.body.pins.get_mut(person) {
                    if card.at > pin.at {
                        pin.at = card.at;
                        changed = true;
                    }
                }
                // The newest card brings the pinned key back: a change waiting is moot.
                if self.body.changes.get(person).is_some_and(|c| card.at > c.at) {
                    self.body.changes.remove(person);
                    changed = true;
                }
                changed
            }
            Seen::Changed => {
                let newer = self.body.changes.get(person).is_none_or(|c| card.at > c.at);
                if newer {
                    self.body.changes.insert(person.to_string(), Pin { kyber: card.kyber.clone(), at: card.at });
                }
                newer
            }
            Seen::Stale => false,
        };
        if changed {
            self.save();
        }
        seen
    }

    /// The key to seal to `person`: their pinned key, unless a change is waiting (then none until
    /// it is accepted), or none when nothing is pinned.
    pub fn sealing_key(&self, person: &str) -> Option<&str> {
        if self.body.changes.contains_key(person) {
            return None;
        }
        self.body.pins.get(person).map(|p| p.kyber.as_str())
    }

    /// The pin for `person`, if any.
    pub fn pinned(&self, person: &str) -> Option<&Pin> {
        self.body.pins.get(person)
    }

    /// The changed key waiting to be accepted for `person`, if any.
    pub fn changed(&self, person: &str) -> Option<&Pin> {
        self.body.changes.get(person)
    }

    /// Everyone whose key changed and is waiting to be accepted.
    pub fn changed_people(&self) -> Vec<String> {
        let mut v: Vec<String> = self.body.changes.keys().cloned().collect();
        v.sort();
        v
    }

    /// Accept `person`'s changed key: it becomes the pin. Returns the new key, None when no change
    /// was waiting.
    pub fn accept(&mut self, person: &str) -> Option<String> {
        let change = self.body.changes.remove(person)?;
        let kyber = change.kyber.clone();
        self.body.pins.insert(person.to_string(), change);
        self.save();
        Some(kyber)
    }

    /// Every person with a key to seal to now (pinned, no change waiting) and that key.
    pub fn sealable(&self) -> Vec<(String, String)> {
        self.body
            .pins
            .iter()
            .filter(|(k, _)| !self.body.changes.contains_key(*k))
            .map(|(k, p)| (k.clone(), p.kyber.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::core::key_card::KeyCard;

    fn card(person: &str, kyber: &str, at: u64) -> KeyCard {
        KeyCard { v: 1, key: person.into(), kyber: kyber.into(), at, sig: String::new() }
    }

    /// A CHANGED KEY RAISES THE CHANGE NOTICE: the first good card for Ann is pinned and sealed to;
    /// a newer card with another key is a change, and nothing is sealed to her until it is
    /// accepted; then the new key is the one sealed to. An older card with another key changes
    /// nothing, and the same key again only notes its time.
    ///
    /// Seen red 2026-10-23 with `sealing_key` reading the pin whatever `changes` held: "with a
    /// change waiting, nothing is sealed to Ann" failed (left `Some("K1")`, right `None`).
    #[test]
    fn a_changed_key_raises_the_change_notice() {
        let mut pins = KeyPins::in_temp(&[61; 32], "me", "change");
        assert_eq!(pins.take(&card("ann", "K1", 100)), Seen::First);
        assert_eq!(pins.sealing_key("ann"), Some("K1"));
        assert_eq!(pins.take(&card("ann", "K1", 150)), Seen::Same);
        assert_eq!(pins.pinned("ann").map(|p| p.at), Some(150), "the newer time is noted");
        assert_eq!(pins.take(&card("ann", "K0", 120)), Seen::Stale, "an older card with another key");
        assert_eq!(pins.sealing_key("ann"), Some("K1"), "and it changed nothing");

        assert_eq!(pins.take(&card("ann", "K2", 200)), Seen::Changed);
        assert_eq!(pins.changed("ann").map(|p| p.kyber.as_str()), Some("K2"));
        assert_eq!(pins.sealing_key("ann"), None, "with a change waiting, nothing is sealed to Ann");
        assert!(pins.sealable().is_empty());
        assert_eq!(pins.changed_people(), ["ann"]);

        assert_eq!(pins.accept("ann").as_deref(), Some("K2"));
        assert_eq!(pins.sealing_key("ann"), Some("K2"), "the accepted key is sealed to");
        assert!(pins.changed("ann").is_none() && pins.accept("ann").is_none());
    }

    /// A change is dropped when a card newer than it brings the pinned key back; an older change
    /// never replaces a newer one waiting.
    #[test]
    fn a_change_waiting_follows_the_newest_card() {
        let mut pins = KeyPins::in_temp(&[62; 32], "me", "follow");
        pins.take(&card("bo", "K1", 100));
        assert_eq!(pins.take(&card("bo", "K3", 300)), Seen::Changed);
        assert_eq!(pins.take(&card("bo", "K2", 200)), Seen::Changed);
        assert_eq!(pins.changed("bo").map(|p| p.kyber.as_str()), Some("K3"), "the newer change stays");
        assert_eq!(pins.take(&card("bo", "K1", 400)), Seen::Same);
        assert!(pins.changed("bo").is_none(), "the pinned key came back in the newest card");
        assert_eq!(pins.sealing_key("bo"), Some("K1"));
    }

    /// The pins and a change waiting survive a restart, and only the identity's own seed opens the
    /// file.
    #[test]
    fn pins_survive_a_restart_for_their_own_seed_only() {
        let seed = [63u8; 32];
        let mut pins = KeyPins::in_temp(&seed, "me", "restart");
        pins.take(&card("ann", "K1", 100));
        pins.take(&card("ann", "K2", 200));
        let again = pins.reloaded(&seed);
        assert_eq!(again.pinned("ann").map(|p| p.kyber.as_str()), Some("K1"));
        assert_eq!(again.changed("ann").map(|p| p.kyber.as_str()), Some("K2"));
        let other = pins.reloaded(&[64u8; 32]);
        assert!(other.pinned("ann").is_none(), "another seed opened the file");
        let _ = std::fs::remove_file(&pins.path);
    }
}
