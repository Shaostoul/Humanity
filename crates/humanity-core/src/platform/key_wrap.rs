//! The phone's remembered key, as bytes on disk (Android rung A1, 2026-11-20;
//! docs/design/platforms.md, docs/reference/cryptography.md "Remembering the key on a phone").
//!
//! The operator, 2026-11-20, on the phone remembering its key: "We should add that." A desktop
//! remembers through its own key store (Windows Credential Manager, macOS Keychain, the Linux
//! Secret Service, through the `keyring` crate, `auto_unlock.rs`), which holds the 32 bytes
//! itself. Android's key store holds KEYS, not secrets: it makes a key that never leaves it (kept
//! in the phone's secure hardware when the phone has some) and encrypts with it on request. So on
//! a phone the same 32 bytes the desktop's key store keeps (the seed, or the Quick PIN's device
//! key) are SEALED by such a key, and the sealed bytes live in the app's private folder, one file
//! per remembered secret. Copying the file off the phone gives nothing: the key that opens it
//! cannot be copied. The JNI half that asks Android's key store is `android_jni.rs`; everything
//! here is plain Rust so the host tests can run it.
//!
//! THE FILE (version 1), `<storage root>/key-store/<entry>.key`, 68 bytes:
//!
//! | bytes  | what |
//! |--------|------|
//! | 0..4   | `HOSK` |
//! | 4      | version, 1 |
//! | 5      | where the key store said it keeps the sealing key ([`Protection`]), for the words in Settings only |
//! | 6      | which of the entry's two sealing keys sealed it, 0 or 1 ([`key_alias`]) |
//! | 7      | 1 when the sealing key carries Android's unlocked-only rule, else 0 ([`KeyInfo`]), for the words in Settings only |
//! | 8..20  | the 12-byte AES-GCM nonce Android's key store chose (it refuses a caller's nonce) |
//! | 20..68 | the 32-byte secret sealed with AES-256-GCM, its 16-byte tag last |
//!
//! The sealing binds the file to its place: the associated data is `HOSK`, the version and the
//! entry's name ([`aad`]), so a seed's file moved into the device key's slot (or another
//! identity's) fails to open instead of opening as the wrong secret. Each entry has two sealing
//! key names that take turns ([`key_alias`]): every remember makes a fresh key under the one the
//! file in place does NOT use, seals and writes the new file, and only then deletes the old key,
//! so a remember that fails leaves the secret remembered before opening as it did (the second
//! review of rung A1, 2026-11-20). Forgetting deletes both keys FIRST and then the file, so a copy
//! of the file made before is useless afterwards, and a key it could not delete leaves the file
//! as the sign that something is still remembered. A file whose key this phone's key store does
//! not hold (one that arrived from another phone) reads as nothing remembered and is deleted
//! ([`KeyFileStore::read`]). [`KeyFileStore`] is the whole store, over a [`Sealer`] (Android's key
//! store on a phone, a stand-in in the tests).

use std::path::{Path, PathBuf};

/// The file's first four bytes.
pub const MAGIC: [u8; 4] = *b"HOSK";
/// The format this code writes and reads.
pub const VERSION: u8 = 1;
/// AES-GCM's nonce, as Android's key store makes it.
pub const NONCE_LEN: usize = 12;
/// The secret a desktop's key store keeps: the seed, or the Quick PIN's device key.
pub const SECRET_LEN: usize = 32;
/// AES-GCM's tag, the last bytes of the sealed secret.
pub const TAG_LEN: usize = 16;
/// Magic, version, protection, which sealing key, the unlocked-only rule.
pub const HEADER_LEN: usize = 8;
/// The two sealing key names of an entry, which take turns ([`key_alias`]).
pub const KEYS: [u8; 2] = [0, 1];
/// The whole file.
pub const FILE_LEN: usize = HEADER_LEN + NONCE_LEN + SECRET_LEN + TAG_LEN;
/// The folder under the storage root that holds the files.
pub const DIR: &str = "key-store";

/// Where Android's key store said it keeps a sealing key (`KeyInfo.getSecurityLevel`, or
/// `isInsideSecureHardware` before Android 12). Shown in Settings so a person knows what their
/// phone actually gives them; it does not change how the key is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protection {
    /// No secure hardware: Android keeps the key in software, so someone with full control of
    /// the phone could use it (still not copy it off the phone through Android's own interfaces).
    Software,
    /// The phone's secure hardware (its trusted execution environment): the key cannot leave it.
    SecureHardware,
    /// A separate security chip (StrongBox), tried first.
    StrongBox,
    /// Android did not say.
    Unknown,
}

impl Protection {
    /// The byte the file keeps.
    pub fn to_byte(self) -> u8 {
        match self {
            Protection::Software => 0,
            Protection::SecureHardware => 1,
            Protection::StrongBox => 2,
            Protection::Unknown => 255,
        }
    }

    /// The protection a file's byte (or the Java side's answer, the same numbers) names; anything
    /// else is unknown.
    pub fn from_code(code: i32) -> Protection {
        match code {
            0 => Protection::Software,
            1 => Protection::SecureHardware,
            2 => Protection::StrongBox,
            _ => Protection::Unknown,
        }
    }

    /// The words under "Remember on this device" on a phone that remembered (data/i18n/en.json).
    pub fn words_key(self) -> &'static str {
        match self {
            Protection::Software => "unlock.protection_software",
            Protection::SecureHardware => "unlock.protection_hardware",
            Protection::StrongBox => "unlock.protection_strongbox",
            Protection::Unknown => "unlock.protection_unknown",
        }
    }

    /// Every protection, for the test that each has its words.
    pub const ALL: [Protection; 4] = [Protection::Software, Protection::SecureHardware, Protection::StrongBox, Protection::Unknown];
}

/// The bit `KeyStoreBridge.create` adds to its answer when the key it made carries Android's
/// unlocked-only rule (`setUnlockedDeviceRequired`, Android 9 and later). The low byte is the
/// [`Protection`] code.
pub const UNLOCKED_ONLY: i32 = 0x100;

/// What Android's key store said about a sealing key it made, kept in the file for the words in
/// Settings: where it lives, and whether Android refuses it while the phone is locked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyInfo {
    pub protection: Protection,
    /// The key carries the unlocked-only rule. Android 8.0 and 8.1 have no such rule, and a phone
    /// that refuses it gets a key without it (KeyStoreBridge falls back), so this is said per
    /// phone, never promised.
    pub unlocked_only: bool,
}

impl KeyInfo {
    /// The answer of `KeyStoreBridge.create`: the protection code, and [`UNLOCKED_ONLY`].
    pub fn from_code(code: i32) -> KeyInfo {
        KeyInfo { protection: Protection::from_code(code & 0xFF), unlocked_only: code & UNLOCKED_ONLY != 0 }
    }

    /// The words under a remembering mode on a phone that remembered, about the locked-phone rule
    /// (data/i18n/en.json).
    pub fn rule_words_key(self) -> &'static str {
        if self.unlocked_only { "unlock.locked_rule_on" } else { "unlock.locked_rule_off" }
    }
}

/// One remembered secret, sealed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wrapped {
    /// Where the sealing key lives and whether it carries the unlocked-only rule.
    pub info: KeyInfo,
    /// Which of the entry's two sealing keys sealed it ([`KEYS`]).
    pub key: u8,
    pub nonce: [u8; NONCE_LEN],
    /// The sealed secret, its tag last: [`SECRET_LEN`] + [`TAG_LEN`] bytes.
    pub sealed: Vec<u8>,
}

/// The file's bytes for `w`. Refuses a sealed secret of the wrong length, so a bad answer from
/// the key store is never written down as though it were a key.
pub fn encode(w: &Wrapped) -> Result<Vec<u8>, String> {
    if w.sealed.len() != SECRET_LEN + TAG_LEN {
        return Err(format!("the sealed key is {} bytes, not {}", w.sealed.len(), SECRET_LEN + TAG_LEN));
    }
    if !KEYS.contains(&w.key) {
        return Err(format!("sealing key {} is not one of an entry's two", w.key));
    }
    let mut out = Vec::with_capacity(FILE_LEN);
    out.extend_from_slice(&MAGIC);
    out.push(VERSION);
    out.push(w.info.protection.to_byte());
    out.push(w.key);
    out.push(u8::from(w.info.unlocked_only));
    out.extend_from_slice(&w.nonce);
    out.extend_from_slice(&w.sealed);
    Ok(out)
}

/// The sealed secret in a file's bytes, or why they are not one.
pub fn decode(bytes: &[u8]) -> Result<Wrapped, String> {
    if bytes.len() != FILE_LEN {
        return Err(format!("the remembered-key file is {} bytes, not {FILE_LEN}", bytes.len()));
    }
    if bytes[..4] != MAGIC {
        return Err("the remembered-key file does not start with HOSK".into());
    }
    if bytes[4] != VERSION {
        return Err(format!("the remembered-key file is version {}, this app reads version {VERSION}", bytes[4]));
    }
    if !KEYS.contains(&bytes[6]) {
        return Err(format!("the remembered-key file names sealing key {}, not 0 or 1", bytes[6]));
    }
    let mut nonce = [0u8; NONCE_LEN];
    nonce.copy_from_slice(&bytes[HEADER_LEN..HEADER_LEN + NONCE_LEN]);
    if bytes[7] > 1 {
        return Err(format!("the remembered-key file's rule byte is {}, not 0 or 1", bytes[7]));
    }
    Ok(Wrapped {
        info: KeyInfo { protection: Protection::from_code(bytes[5] as i32), unlocked_only: bytes[7] == 1 },
        key: bytes[6],
        nonce,
        sealed: bytes[HEADER_LEN + NONCE_LEN..].to_vec(),
    })
}

/// The entry's name for a slot (`seed`, `device-key`) and an identity: the slot and the first 32
/// hex characters of the public key, the way the desktop names its key-store accounts
/// (`auto_unlock::keychain_account`), with `-` for its `/` since this is a file name. Only hex is
/// accepted for the identity and only `a-z` and `-` for the slot, so the name can never leave the
/// folder.
pub fn entry_name(slot_tag: &str, public_key_hex: &str) -> Result<String, String> {
    if slot_tag.is_empty() || !slot_tag.bytes().all(|b| b.is_ascii_lowercase() || b == b'-') {
        return Err(format!("not a key-store slot: {slot_tag:?}"));
    }
    // The bytes are checked BEFORE the cut, so a hand-edited key with a character wider than a
    // byte is refused rather than sliced through the middle of it (a panic).
    let short = &public_key_hex.as_bytes()[..public_key_hex.len().min(32)];
    if short.is_empty() || !short.iter().all(|b| b.is_ascii_hexdigit()) {
        return Err("the identity is not a hex public key".into());
    }
    Ok(format!("{slot_tag}-{}", String::from_utf8_lossy(short).to_ascii_lowercase()))
}

/// The name inside Android's key store of the entry's sealing key `key` (0 or 1, [`KEYS`]). Two
/// names that take turns, so a new key is made and used before the key in use is deleted
/// ([`KeyFileStore::keep`]).
pub fn key_alias(entry: &str, key: u8) -> String {
    format!("humanityos/{entry}/{key}")
}

/// The associated data the sealing binds: `HOSK`, the version, the entry's name.
pub fn aad(entry: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(5 + entry.len());
    out.extend_from_slice(&MAGIC);
    out.push(VERSION);
    out.extend_from_slice(entry.as_bytes());
    out
}

/// The entry's file in the folder `dir` (`<storage root>/key-store`).
pub fn file_path(dir: &Path, entry: &str) -> PathBuf {
    dir.join(format!("{entry}.key"))
}

/// Overwrite `bytes` with zeros in a way the compiler cannot drop as a dead store, so a plaintext
/// copy of a remembered secret does not linger in memory once used (the review of rungs A1 and
/// A4, 2026-11-20). No crate for it: a volatile write per byte and a fence, the way `zeroize`
/// does it.
pub fn wipe(bytes: &mut [u8]) {
    for b in bytes.iter_mut() {
        // SAFETY: `b` is a valid, aligned, exclusive reference to one byte.
        unsafe { std::ptr::write_volatile(b, 0) };
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
}

/// [`wipe`] for a `String` (a PIN or a passphrase typed into a box): its bytes zeroed, then
/// emptied. Copies a String left behind when it grew are out of reach; this wipes the one it has.
pub fn wipe_string(s: &mut String) {
    // SAFETY: zero bytes are valid UTF-8, so the String is valid throughout.
    wipe(unsafe { s.as_bytes_mut() });
    s.clear();
}

// ── The store: the files and the key store's keys together ────────────────

/// What seals and opens the 32 bytes. On a phone it is Android's key store through the app's Java
/// (`auto_unlock`'s `JniSealer`, `platform::android_jni`); in the tests a stand-in. Everything
/// around it ([`KeyFileStore`]) is the same for both, so the phone's file handling runs in the
/// host tests.
pub trait Sealer {
    /// Make a fresh sealing key under `alias`, replacing any: where the key store put it, and
    /// whether it carries the unlocked-only rule.
    fn create(&mut self, alias: &str) -> Result<KeyInfo, String>;
    /// Seal `plain` with `alias`'s key, binding `aad`: the 12-byte nonce the key store chose, then
    /// the sealed bytes with the tag last.
    fn seal(&mut self, alias: &str, plain: &[u8], aad: &[u8]) -> Result<Vec<u8>, String>;
    /// Open `sealed`: `None` when the key store holds no key under `alias`, an error when the bytes
    /// or the associated data do not match the key it holds.
    fn unseal(&mut self, alias: &str, nonce: &[u8], sealed: &[u8], aad: &[u8]) -> Result<Option<Vec<u8>>, String>;
    /// Delete `alias`'s key; nothing when there is none.
    fn forget(&mut self, alias: &str) -> Result<(), String>;
}

/// The remembered secrets of one phone: a file per entry in `dir` (`<storage root>/key-store`),
/// each sealed by its own key in the key store `sealer` asks.
pub struct KeyFileStore<S> {
    pub dir: PathBuf,
    pub sealer: S,
}

impl<S: Sealer> KeyFileStore<S> {
    /// Remember `secret` under the slot `slot_tag` (`seed`, `device-key`) for the identity
    /// `public_key_hex`, replacing what the slot held only once the new secret is safely kept: a
    /// fresh sealing key under the entry's key name the file in place does NOT use, the sealed
    /// bytes written whole beside the file and renamed over it (a crash never leaves half a
    /// file), and only then the old key deleted. Should any step fail, the new key is deleted
    /// again and the secret remembered before still opens (`auto_unlock::remember`'s "nothing
    /// changes"). Returns what the key store said about the new key.
    pub fn keep(&mut self, slot_tag: &str, public_key_hex: &str, secret: &[u8; SECRET_LEN]) -> Result<KeyInfo, String> {
        let entry = entry_name(slot_tag, public_key_hex)?;
        let path = file_path(&self.dir, &entry);
        // The key the file in place was sealed by, when there is one that reads: the new key goes
        // under the other name.
        let in_use = std::fs::read(&path).ok().and_then(|b| decode(&b).ok()).map(|w| w.key);
        let key = if in_use == Some(0) { 1 } else { 0 };
        match self.seal_into(&entry, &path, key, secret) {
            Ok(info) => {
                // The new file is in place: the other key (the old one, or one a remember cut
                // short left) goes now. Should it stay, the next remember or forget deletes it.
                if let Err(e) = self.sealer.forget(&key_alias(&entry, 1 - key)) {
                    log::warn!("key store: the key {entry} was sealed by before could not be deleted ({e}); the next remember or forget tries again");
                }
                Ok(info)
            }
            Err(e) => {
                if let Err(e2) = self.sealer.forget(&key_alias(&entry, key)) {
                    log::warn!("key store: the new key for {entry} could not be deleted after a failed remember ({e2})");
                }
                Err(e)
            }
        }
    }

    /// The steps of [`KeyFileStore::keep`] that may fail: make the key `key`, seal, write.
    fn seal_into(&mut self, entry: &str, path: &Path, key: u8, secret: &[u8; SECRET_LEN]) -> Result<KeyInfo, String> {
        let alias = key_alias(entry, key);
        let info = self.sealer.create(&alias)?;
        let out = self.sealer.seal(&alias, secret, &aad(entry))?;
        if out.len() != NONCE_LEN + SECRET_LEN + TAG_LEN {
            return Err(format!("the key store sealed {} bytes, not {}", out.len(), NONCE_LEN + SECRET_LEN + TAG_LEN));
        }
        let mut nonce = [0u8; NONCE_LEN];
        nonce.copy_from_slice(&out[..NONCE_LEN]);
        let file = encode(&Wrapped { info, key, nonce, sealed: out[NONCE_LEN..].to_vec() })?;
        std::fs::create_dir_all(&self.dir).map_err(|e| format!("{}: {e}", self.dir.display()))?;
        let tmp = path.with_extension("key.tmp");
        let written = std::fs::write(&tmp, &file)
            .map_err(|e| format!("{}: {e}", tmp.display()))
            .and_then(|_| std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display())));
        if written.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        written.map(|_| info)
    }

    /// The secret remembered under the slot for the identity: `None` when nothing is.
    pub fn read(&mut self, slot_tag: &str, public_key_hex: &str) -> Result<Option<[u8; SECRET_LEN]>, String> {
        let entry = entry_name(slot_tag, public_key_hex)?;
        let path = file_path(&self.dir, &entry);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        let w = decode(&bytes)?;
        let Some(mut plain) = self.sealer.unseal(&key_alias(&entry, w.key), &w.nonce, &w.sealed, &aad(&entry))? else {
            // A file whose sealing key this phone's key store does not hold: one that arrived from
            // another phone (a device transfer or a restored backup despite the manifest's rules)
            // or outlived its key. It can never open here, so nothing is remembered: the file goes,
            // and start-up treats the mode as Always ask, so Settings offers Remember again in one
            // click instead of claiming a key it cannot use (the review's finding 3, 2026-11-20).
            remove(&path)?;
            log::info!("key store: {entry} was sealed by a key this phone's key store does not hold; removed it");
            return Ok(None);
        };
        let out = <[u8; SECRET_LEN]>::try_from(plain.as_slice()).map(Some).map_err(|_| format!("the remembered secret is {} bytes, not {SECRET_LEN}", plain.len()));
        wipe(&mut plain);
        out
    }

    /// Forget the slot's secret for the identity: its sealing keys FIRST (both names, whichever the
    /// file uses, so a key a remember cut short left goes too), so the file is useless whatever
    /// happens to it, then the file. A key that could not be deleted is reported and leaves the
    /// file in place: the file is how the store knows something is still remembered
    /// ([`KeyFileStore::holds`]), so the next forget (Settings' Try again, or start-up,
    /// `auto_unlock::start_up`) finishes the job. A file that could not be deleted is reported too.
    pub fn forget(&mut self, slot_tag: &str, public_key_hex: &str) -> Result<(), String> {
        let entry = entry_name(slot_tag, public_key_hex)?;
        let failed: Vec<String> = KEYS.iter().filter_map(|&k| self.sealer.forget(&key_alias(&entry, k)).err()).collect();
        if !failed.is_empty() {
            return Err(failed.join("; "));
        }
        remove(&file_path(&self.dir, &entry))
    }

    /// Whether the slot holds a remembered secret for the identity: its file is there (a file
    /// whose key is gone counts until a read or a forget removes it). Answers without opening the
    /// secret, so start-up can look for another mode's leftovers without unsealing them.
    pub fn holds(&self, slot_tag: &str, public_key_hex: &str) -> Result<bool, String> {
        let entry = entry_name(slot_tag, public_key_hex)?;
        Ok(file_path(&self.dir, &entry).exists())
    }

    /// What the key store said about the slot's sealing key when it was made, from its file (for
    /// the words in Settings): `None` when nothing is remembered.
    pub fn key_info(&self, slot_tag: &str, public_key_hex: &str) -> Option<KeyInfo> {
        let entry = entry_name(slot_tag, public_key_hex).ok()?;
        let bytes = std::fs::read(file_path(&self.dir, &entry)).ok()?;
        decode(&bytes).ok().map(|w| w.info)
    }
}

/// Delete a file; one already gone is fine.
fn remove(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// A stand-in for Android's key store in the tests: one key byte per alias, "sealing" by XOR
/// with it and a tag over the associated data, so a wrong slot, a forgotten key and a missing key
/// all behave as the real one's do.
#[cfg(test)]
#[derive(Debug, Default)]
pub(crate) struct FakeSealer {
    pub keys: std::collections::BTreeMap<String, u8>,
    next: u8,
    /// Refuse every seal, as a key store that made a key and then cannot use it does.
    pub fail_seal: bool,
    /// Refuse every key delete, leaving the key in place.
    pub fail_forget: bool,
}

#[cfg(test)]
impl FakeSealer {
    fn tag(key: u8, aad: &[u8]) -> [u8; TAG_LEN] {
        let mut tag = [key; TAG_LEN];
        for (i, b) in aad.iter().enumerate() {
            tag[i % TAG_LEN] ^= b.wrapping_add(i as u8);
        }
        tag
    }
}

#[cfg(test)]
impl Sealer for FakeSealer {
    fn create(&mut self, alias: &str) -> Result<KeyInfo, String> {
        self.next = self.next.wrapping_add(1).max(1);
        self.keys.insert(alias.to_string(), self.next);
        Ok(KeyInfo { protection: Protection::SecureHardware, unlocked_only: true })
    }

    fn seal(&mut self, alias: &str, plain: &[u8], aad: &[u8]) -> Result<Vec<u8>, String> {
        if self.fail_seal {
            return Err("the key store refused to seal".into());
        }
        let key = *self.keys.get(alias).ok_or_else(|| format!("no key {alias}"))?;
        let mut out = vec![key; NONCE_LEN];
        out.extend(plain.iter().map(|b| b ^ key));
        out.extend_from_slice(&Self::tag(key, aad));
        Ok(out)
    }

    fn unseal(&mut self, alias: &str, _nonce: &[u8], sealed: &[u8], aad: &[u8]) -> Result<Option<Vec<u8>>, String> {
        let Some(&key) = self.keys.get(alias) else { return Ok(None) };
        let (body, tag) = sealed.split_at(sealed.len().saturating_sub(TAG_LEN));
        if tag != Self::tag(key, aad) {
            return Err("the sealed bytes do not open with this key".into());
        }
        Ok(Some(body.iter().map(|b| b ^ key).collect()))
    }

    fn forget(&mut self, alias: &str) -> Result<(), String> {
        if self.fail_forget {
            return Err("the key store refused to delete a key".into());
        }
        self.keys.remove(alias);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Wrapped {
        Wrapped { info: KeyInfo { protection: Protection::SecureHardware, unlocked_only: true }, key: 1, nonce: [7u8; NONCE_LEN], sealed: (0..48u8).collect() }
    }

    /// The file round-trips, is exactly 68 bytes and starts with its header. Seen red 2026-11-20
    /// with `encode` leaving out the protection byte: the length assertion failed (65, not 66, the
    /// length then), and `a_wrong_file_is_refused` with it (a short file is refused for its length
    /// first).
    #[test]
    fn a_wrapped_key_round_trips_through_its_file() {
        let bytes = encode(&sample()).unwrap();
        assert_eq!(bytes.len(), FILE_LEN);
        assert_eq!(FILE_LEN, 68);
        assert_eq!(&bytes[..8], b"HOSK\x01\x01\x01\x01");
        let without = Wrapped { info: KeyInfo { unlocked_only: false, ..sample().info }, ..sample() };
        assert_eq!(decode(&encode(&without).unwrap()).unwrap(), without, "a key without the rule reads back without it");
        assert_eq!(decode(&bytes).unwrap(), sample());
    }

    /// Every way a file can be wrong is refused in words, and a sealed secret of the wrong length
    /// is never written.
    #[test]
    fn a_wrong_file_is_refused() {
        let good = encode(&sample()).unwrap();
        assert!(decode(&good[..67]).unwrap_err().contains("67 bytes"));
        let mut magic = good.clone();
        magic[0] = b'X';
        assert!(decode(&magic).unwrap_err().contains("HOSK"));
        let mut version = good.clone();
        version[4] = 2;
        assert!(decode(&version).unwrap_err().contains("version 2"));
        let mut key = good.clone();
        key[6] = 2;
        assert!(decode(&key).unwrap_err().contains("sealing key 2"));
        assert!(encode(&Wrapped { key: 2, ..sample() }).is_err(), "only an entry's two sealing keys");
        let mut rule = good.clone();
        rule[7] = 2;
        assert!(decode(&rule).unwrap_err().contains("rule byte is 2"));
        let short = Wrapped { sealed: vec![0; 32], ..sample() };
        assert!(encode(&short).is_err(), "a sealed secret without its tag");
    }

    /// The protection byte keeps what the key store said, and an unknown byte reads as unknown.
    #[test]
    fn the_protection_byte_names_where_the_key_lives() {
        for p in Protection::ALL {
            assert_eq!(Protection::from_code(p.to_byte() as i32), p, "{p:?}");
        }
        assert_eq!(Protection::from_code(9), Protection::Unknown);
        assert_eq!(Protection::from_code(-1), Protection::Unknown);
        // KeyStoreBridge.create's answer: the code, and the unlocked-only bit above it.
        assert_eq!(KeyInfo::from_code(1 | UNLOCKED_ONLY), KeyInfo { protection: Protection::SecureHardware, unlocked_only: true });
        assert_eq!(KeyInfo::from_code(2), KeyInfo { protection: Protection::StrongBox, unlocked_only: false });
        assert_eq!(KeyInfo::from_code(255 | UNLOCKED_ONLY), KeyInfo { protection: Protection::Unknown, unlocked_only: true });
    }

    /// Entry names are the slot and the identity's first 32 hex characters, lower case; anything
    /// that could leave the folder is refused.
    #[test]
    fn entry_names_stay_inside_the_folder() {
        let pk = "ABCDEF0123456789abcdef0123456789ffffffffffff";
        assert_eq!(entry_name("seed", pk).unwrap(), "seed-abcdef0123456789abcdef0123456789");
        assert_eq!(entry_name("device-key", "00ff").unwrap(), "device-key-00ff");
        assert!(entry_name("seed", "../etc").is_err());
        assert!(entry_name("seed", "").is_err());
        assert!(entry_name("../seed", "00ff").is_err());
        assert!(entry_name("Seed", "00ff").is_err());
        assert_eq!(file_path(Path::new("ks"), "seed-00ff"), Path::new("ks").join("seed-00ff.key"));
        assert_eq!(key_alias("seed-00ff", 1), "humanityos/seed-00ff/1");
    }

    /// The associated data differs per entry, so one entry's file never opens as another's.
    #[test]
    fn the_sealing_is_bound_to_its_entry() {
        assert_ne!(aad("seed-00ff"), aad("device-key-00ff"));
        assert_ne!(aad("seed-00ff"), aad("seed-00fe"));
        assert_eq!(&aad("seed-00ff")[..5], b"HOSK\x01");
    }

    /// A hand-edited identity with a character wider than a byte where the 32-character cut falls
    /// is refused in words, never a panic (the review's finding 8). Seen red 2026-11-20 with the
    /// slice taken before the hex check: "byte index 32 is not a char boundary; it is inside 'é'".
    #[test]
    fn a_non_ascii_identity_is_refused_not_a_panic() {
        let pk = format!("{}é{}", "a".repeat(31), "b".repeat(10));
        assert!(entry_name("seed", &pk).is_err());
        assert!(entry_name("seed", "ééé").is_err());
    }

    fn store(name: &str) -> KeyFileStore<FakeSealer> {
        let dir = std::env::temp_dir().join(format!("hos-key-wrap-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        KeyFileStore { dir, sealer: FakeSealer::default() }
    }

    const PK: &str = "00ff00ff00ff00ff00ff00ff00ff00ff00ff";

    /// Keep, read, forget: the phone's store round-trips a secret, and forgetting removes the file
    /// and the key, so nothing reads afterwards.
    #[test]
    fn the_store_keeps_reads_and_forgets() {
        let mut s = store("round");
        assert_eq!(s.read("seed", PK).unwrap(), None, "nothing remembered yet");
        assert_eq!(s.keep("seed", PK, &[9u8; 32]).unwrap().protection, Protection::SecureHardware);
        assert_eq!(s.read("seed", PK).unwrap(), Some([9u8; 32]));
        assert_eq!(s.key_info("seed", PK), Some(KeyInfo { protection: Protection::SecureHardware, unlocked_only: true }));
        s.forget("seed", PK).unwrap();
        assert_eq!(s.read("seed", PK).unwrap(), None);
        assert!(s.sealer.keys.is_empty(), "the sealing key went with the file");
        s.forget("seed", PK).unwrap();
        let _ = std::fs::remove_dir_all(&s.dir);
    }

    /// A remembered file that arrives on a phone whose key store lacks its key (a device transfer,
    /// a restored backup) can never open there: it reads as nothing remembered and is removed, so
    /// Settings shows the mode as not remembered and one click remembers again (the review's
    /// finding 3). Seen red 2026-11-20 with `read` answering an error for it ("the key store no
    /// longer holds the key that sealed it") and leaving the file in place.
    #[test]
    fn a_file_from_another_phone_reads_as_nothing_remembered_and_is_removed() {
        let mut s = store("arrived");
        s.keep("seed", PK, &[3u8; 32]).unwrap();
        s.sealer.keys.clear(); // this phone's key store never made that key
        assert_eq!(s.read("seed", PK).unwrap(), None);
        let entry = entry_name("seed", PK).unwrap();
        assert!(!file_path(&s.dir, &entry).exists(), "the file that can never open here was removed");
        assert_eq!(s.key_info("seed", PK), None, "Settings says nothing is remembered");
        let _ = std::fs::remove_dir_all(&s.dir);
    }

    /// Forgetting deletes the sealing key FIRST and the file only once the key is gone, so a copy
    /// of the file made before is useless afterwards, and a key that could not be deleted leaves
    /// the file in place: the store still says it holds the secret, and the next forget
    /// (Settings' Try again, or start-up) finishes the job. A file it could not delete is
    /// reported, never dropped (the review's finding 8). This pins the ORDER (the second review's
    /// F: the test before it passed whichever came first). Seen red 2026-11-20 against the forget
    /// that attempted both whatever happened: "a key that could not be deleted must leave the
    /// file in place".
    #[test]
    fn forgetting_deletes_the_key_first() {
        let mut s = store("order");
        s.keep("seed", PK, &[5u8; 32]).unwrap();
        let entry = entry_name("seed", PK).unwrap();
        let path = file_path(&s.dir, &entry);
        s.sealer.fail_forget = true;
        assert!(s.forget("seed", PK).is_err(), "a key it could not delete is reported");
        assert!(path.exists(), "a key that could not be deleted must leave the file in place");
        assert_eq!(s.holds("seed", PK), Ok(true), "the store still says it holds the secret");
        s.sealer.fail_forget = false;
        s.forget("seed", PK).unwrap();
        assert!(!path.exists() && s.sealer.keys.is_empty(), "the retry forgets the key and the file");
        assert_eq!(s.holds("seed", PK), Ok(false));

        // A folder where the file was: remove_file refuses it on every platform.
        s.keep("seed", PK, &[5u8; 32]).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir_all(path.join("stuck")).unwrap();
        assert!(s.forget("seed", PK).is_err(), "a file it could not delete is reported");
        assert!(s.sealer.keys.is_empty(), "the sealing key must be gone even when the file stays");
        let _ = std::fs::remove_dir_all(&s.dir);
    }

    /// Remembering again over a remembered secret makes the new key under the entry's OTHER alias,
    /// seals and writes the new file, and only then deletes the old key, so a re-remember that
    /// fails at any step leaves the old secret opening as before (the second review's B: the old
    /// `create` deleted the alias in use first, so a Quick PIN set up again that failed broke the
    /// PIN that worked). Seen red 2026-11-20 against the one-alias keep: "called
    /// `Result::unwrap()` on an `Err` value: \"the sealed bytes do not open with this key\"" (the
    /// failed seal had already replaced the key the old file was sealed by).
    #[test]
    fn a_failed_re_remember_leaves_the_old_secret_working() {
        let mut s = store("again");
        s.keep("device-key", PK, &[1u8; 32]).unwrap();
        s.sealer.fail_seal = true;
        assert!(s.keep("device-key", PK, &[2u8; 32]).is_err(), "the failed seal is reported");
        s.sealer.fail_seal = false;
        assert_eq!(s.read("device-key", PK).unwrap(), Some([1u8; 32]), "a failed seal broke the old secret");
        assert_eq!(s.sealer.keys.len(), 1, "the key made for the failed seal is deleted again: {:?}", s.sealer.keys);

        // The write fails: a folder where the new file is written first.
        let entry = entry_name("device-key", PK).unwrap();
        let tmp = file_path(&s.dir, &entry).with_extension("key.tmp");
        std::fs::create_dir_all(tmp.join("stuck")).unwrap();
        assert!(s.keep("device-key", PK, &[3u8; 32]).is_err(), "the failed write is reported");
        assert_eq!(s.read("device-key", PK).unwrap(), Some([1u8; 32]), "a failed write broke the old secret");
        assert_eq!(s.sealer.keys.len(), 1, "the key made for the failed write is deleted again: {:?}", s.sealer.keys);
        std::fs::remove_dir_all(&tmp).unwrap();

        // A re-remember that works replaces the secret and leaves one key, the new one.
        s.keep("device-key", PK, &[4u8; 32]).unwrap();
        assert_eq!(s.read("device-key", PK).unwrap(), Some([4u8; 32]));
        assert_eq!(s.sealer.keys.len(), 1, "the old key is deleted once the new file is written: {:?}", s.sealer.keys);
        s.keep("device-key", PK, &[5u8; 32]).unwrap();
        assert_eq!(s.read("device-key", PK).unwrap(), Some([5u8; 32]), "the aliases take turns");
        let _ = std::fs::remove_dir_all(&s.dir);
    }

    /// The wipe leaves only zeros, and is made of the writes the optimiser may not drop. What a
    /// test can and cannot prove, said honestly: the zeros after the call are measured here, but a
    /// plain `fill(0)` passes that half too, because a buffer read afterwards is exactly the case
    /// in which every compiler must keep the writes. The writes it may drop are the ones nobody
    /// reads (a key about to go out of scope), and no test can read memory after that without
    /// undefined behaviour. So the second half pins the mechanism in the source: `wipe` writes
    /// through `write_volatile` and ends with a `compiler_fence`, which is what keeps those writes
    /// (the second review's C: the test before passed with or without them). Seen red 2026-11-20
    /// with the body replaced by `bytes.fill(0)`: "wipe must write through write_volatile and end
    /// with a compiler_fence".
    #[test]
    fn the_wipe_leaves_zeros() {
        let mut secret = [0xA5u8; 32];
        wipe(&mut secret);
        assert_eq!(secret, [0u8; 32]);
        let mut pin = String::from("482916");
        let (at, len) = (pin.as_ptr(), pin.len());
        wipe_string(&mut pin);
        assert!(pin.is_empty());
        // SAFETY: `clear` keeps the buffer, whose bytes were written, so reading them is defined.
        let left = unsafe { std::slice::from_raw_parts(at, len) };
        assert_eq!(left, [0u8; 6], "the PIN's bytes stayed in its buffer");
        let src = include_str!("key_wrap.rs");
        let body = src.split("pub fn wipe(bytes: &mut [u8]) {").nth(1).and_then(|r| r.split("\n}").next()).unwrap_or_default();
        assert!(body.contains("write_volatile(") && body.contains("compiler_fence("), "wipe must write through write_volatile and end with a compiler_fence:\n{body}");
    }
}
