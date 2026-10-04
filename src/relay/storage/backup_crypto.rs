//! Backup encryption at rest (privacy hardening, 2026-08-23).
//!
//! The database backups were the softest copy of everything the relay
//! knows: plain SQLite files sitting in a directory that gets rotated,
//! rsynced, and pulled to the operator's other machines. Anyone who
//! obtained one file got the whole server. Now every in-process backup
//! is sealed with AES-256-GCM under a machine-local key.
//!
//! The key lives NEXT TO THE LIVE DATABASE (`<db_dir>/backup.key`),
//! deliberately OUTSIDE the backups directory: the exposure this closes
//! is backup media wandering (a copied backups folder, a synced drive,
//! the off-box pull) — those copies are ciphertext without the key. An
//! attacker with full control of the live box has the live DB anyway;
//! at-rest backup encryption is about everywhere the backups travel.
//!
//! File format: 12-byte nonce ‖ AES-256-GCM ciphertext, extension
//! `.db.enc`. Recovery (`Storage::open_resilient`) decrypts candidates
//! transparently, and plain `.db` backups (older ones, or the VPS shell
//! script before it is updated) remain restorable.

use std::path::{Path, PathBuf};

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    AeadCore, Aes256Gcm, Key, Nonce,
};

/// Key file name, created on demand beside the live database.
const KEY_FILE: &str = "backup.key";

/// Load the 32-byte backup key from `dir`, creating it (0600 on unix)
/// on first use. None only on I/O failure (caller falls back to a
/// plain backup rather than silently having none).
pub fn load_or_create_key(dir: &Path) -> Option<[u8; 32]> {
    load_or_create_named_key(dir, KEY_FILE)
}

/// The same for any 32-byte machine-local secret kept beside the live database under the
/// file name `file`: the backup key above, and the erased-accounts fingerprint secret
/// (storage/erased_accounts.rs, 2026-10-04). One implementation, so both are created, kept
/// private (0600 on unix) and refused when damaged the same way. None when the file is
/// unusable; `load_named_key` says which case it was.
pub fn load_or_create_named_key(dir: &Path, file: &str) -> Option<[u8; 32]> {
    load_named_key(dir, file).0
}

/// What `load_named_key` found in the key file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyFile {
    /// Read from the file.
    Loaded,
    /// There was none: one was made now.
    Created,
    /// There is one but it is not a usable key (the wrong length, as a crash in the middle of
    /// a non-atomic write could leave it), or it could not be read, or a new one could not be
    /// written. A file that is there is NEVER overwritten: for the backup key that would make
    /// every backup sealed with the old key unreadable, so the operator decides. The caller
    /// reports this (the erased-accounts secret: /health and `just brief`), so it is not
    /// only a line in a log that every restart repeats.
    Unusable,
}

/// Load the key in `dir/file`, or create it when there is none. Created atomically (review
/// of BUG-135 option 2, 2026-10-04): the key is written whole to a temporary file, flushed,
/// and only then made to exist under its name, so a crash or a full disk mid-write leaves no
/// key file at all (the next start makes one) rather than a short one that every later start
/// refuses. And never over another (the second round, finding 5): every creator writes its
/// own temporary file and installs it only where there is none, then reads back whatever is
/// there, so creators racing (two relays on one folder, or the test suite, whose databases
/// share the temp folder) all end with the one key on disk.
pub fn load_named_key(dir: &Path, file: &str) -> (Option<[u8; 32]>, KeyFile) {
    let path = dir.join(file);
    if let Some(found) = read_key_file(&path, file) {
        return found;
    }
    // Generate. AeadCore::generate_nonce is the vetted entropy path this
    // crate already links; two nonces + a bit of key stretching would be
    // silly when OsRng fills arbitrary buffers directly.
    use aes_gcm::aead::rand_core::RngCore;
    let mut k = [0u8; 32];
    OsRng.fill_bytes(&mut k);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match install_key_no_clobber(&path, &k) {
        Ok(true) => {
            tracing::info!("{file} created at {}", path.display());
            (Some(k), KeyFile::Created)
        }
        // Another creator installed one first: that one is the key.
        Ok(false) => read_key_file(&path, file).unwrap_or((None, KeyFile::Unusable)),
        Err(e) => {
            tracing::error!("could not write {file} at {}: {e}", path.display());
            (None, KeyFile::Unusable)
        }
    }
}

/// What the key file at `path` holds: None when there is no such file (the caller makes one),
/// otherwise the key (Loaded) or Unusable.
fn read_key_file(path: &Path, file: &str) -> Option<(Option<[u8; 32]>, KeyFile)> {
    match std::fs::read(path) {
        Ok(bytes) if bytes.len() == 32 => {
            let mut k = [0u8; 32];
            k.copy_from_slice(&bytes);
            Some((Some(k), KeyFile::Loaded))
        }
        Ok(bytes) => {
            tracing::error!(
                "{file} at {} has wrong length ({}); refusing to overwrite it: fix or remove it manually",
                path.display(),
                bytes.len()
            );
            Some((None, KeyFile::Unusable))
        }
        // Only a file that is not there is made. Any other read error (a permission, a lock)
        // must not lead to a new key written over a good one.
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            tracing::error!("could not read {file} at {}: {e}; leaving it as it is", path.display());
            Some((None, KeyFile::Unusable))
        }
        Err(_) => None,
    }
}

/// Write `k` to a temporary file of this writer's own (`<file>.<random>.tmp`, private from the
/// start on unix), flush it to the disk, and install it as `path` only if there is none: a
/// hard link, which fails when `path` exists, so it is all 32 bytes or not there, and never
/// replaces a key another creator installed first (a shared `<file>.tmp` and `rename`, which
/// replaces, let racing creators end with different keys). The temporary file is removed
/// either way. Ok(true): installed; Ok(false): one was there first.
fn install_key_no_clobber(path: &Path, k: &[u8; 32]) -> std::io::Result<bool> {
    use std::io::Write;
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".{:016x}.tmp", rand::random::<u64>()));
    let tmp = PathBuf::from(tmp);
    let installed = (|| {
        let mut f = private_new_file().open(&tmp)?;
        f.write_all(k)?;
        f.sync_all()?;
        drop(f);
        match std::fs::hard_link(&tmp, path) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
            Err(e) => {
                // A filesystem without hard links: make the key file itself, still never over
                // another (create_new). Not atomic: a crash in these few bytes leaves a short
                // file, which every start then reports and leaves for the operator.
                tracing::warn!("no hard links for {} ({e}); writing the key file in place", path.display());
                write_key_in_place(path, k, write_and_sync)
            }
        }
    })();
    let _ = std::fs::remove_file(&tmp);
    installed
}

/// A new file opened for writing only if there is none at its path, private from the start on
/// unix (0600), so a key is never readable by others even before it is written.
fn private_new_file() -> std::fs::OpenOptions {
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    opts
}

/// The whole key written to `f` and flushed to the disk.
fn write_and_sync(f: &mut std::fs::File, k: &[u8; 32]) -> std::io::Result<()> {
    use std::io::Write;
    f.write_all(k)?;
    f.sync_all()
}

/// `install_key_no_clobber`'s fallback on a filesystem without hard links: the key file made in
/// place by `write`, never over another (create_new; Ok(false) when one is there first). When
/// the write or the flush fails (a full disk), the file is removed again: create_new proves it
/// is this call's own, and a short one left there would be read as Unusable by every later
/// start (final review of 085441749, finding 3), where none lets the next start make the key.
/// Only a crash in the middle of these few bytes can still leave a short file.
fn write_key_in_place(
    path: &Path,
    k: &[u8; 32],
    write: impl FnOnce(&mut std::fs::File, &[u8; 32]) -> std::io::Result<()>,
) -> std::io::Result<bool> {
    let mut f = match private_new_file().open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Ok(false),
        Err(e) => return Err(e),
    };
    if let Err(e) = write(&mut f, k) {
        drop(f);
        let _ = std::fs::remove_file(path);
        return Err(e);
    }
    Ok(true)
}

/// Encrypt `plain_path` into `enc_path` (nonce ‖ ciphertext).
pub fn encrypt_file(key: &[u8; 32], plain_path: &Path, enc_path: &Path) -> std::io::Result<()> {
    let plain = std::fs::read(plain_path)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ct = cipher
        .encrypt(&nonce, plain.as_slice())
        .map_err(|e| std::io::Error::other(format!("backup seal: {e}")))?;
    let mut out = Vec::with_capacity(12 + ct.len());
    out.extend_from_slice(nonce.as_slice());
    out.extend_from_slice(&ct);
    std::fs::write(enc_path, out)
}

/// Decrypt `enc_path` into `out_path`. Errors on wrong key or tampering
/// (GCM authenticates), so a corrupted backup never restores silently.
pub fn decrypt_file(key: &[u8; 32], enc_path: &Path, out_path: &Path) -> std::io::Result<()> {
    let raw = std::fs::read(enc_path)?;
    if raw.len() < 13 {
        return Err(std::io::Error::other("encrypted backup too short"));
    }
    let (nonce, ct) = raw.split_at(12);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let plain = cipher
        .decrypt(Nonce::from_slice(nonce), ct)
        .map_err(|e| std::io::Error::other(format!("backup open (wrong key / tampered): {e}")))?;
    std::fs::write(out_path, plain)
}

/// Is this backup file sealed (as opposed to a legacy plain .db)?
pub fn is_encrypted_backup(path: &Path) -> bool {
    path.to_string_lossy().ends_with(".db.enc")
}

/// The key directory for a given live-database path (its parent dir).
pub fn key_dir_for_db(db_path: &Path) -> PathBuf {
    db_path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(tag: &str) -> PathBuf {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let d = std::env::temp_dir().join(format!("hum_bkcrypt_{tag}_{pid}_{nanos}"));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn roundtrip_and_key_reuse() {
        let dir = tmp_dir("roundtrip");
        let k1 = load_or_create_key(&dir).expect("create key");
        let k2 = load_or_create_key(&dir).expect("reload key");
        assert_eq!(k1, k2, "key is stable across loads");

        let plain = dir.join("relay_test.db");
        std::fs::write(&plain, b"pretend sqlite bytes with PII inside").unwrap();
        let enc = dir.join("relay_test.db.enc");
        encrypt_file(&k1, &plain, &enc).unwrap();
        let raw = std::fs::read(&enc).unwrap();
        assert!(
            !raw.windows(3).any(|w| w == b"PII"),
            "ciphertext must not contain the plaintext"
        );

        let out = dir.join("restored.db");
        decrypt_file(&k1, &enc, &out).unwrap();
        assert_eq!(std::fs::read(&out).unwrap(), b"pretend sqlite bytes with PII inside");
    }

    /// Review of BUG-135 option 2, finding 7: a key file left short (a crash or a full disk in
    /// the middle of a plain write) made every later start log one line and carry on with a
    /// throwaway secret, so each restart forgot every remembered erase and nothing said so
    /// anywhere a person would look. Now: such a file is reported as Unusable on every start,
    /// and never overwritten (for the backup key that would strand every backup sealed with
    /// it); and a new key is written whole or not at all, with no `.tmp` left behind.
    ///
    /// Seen red 2026-10-04 with a wrong-length file returned as Loaded with zero bytes padded
    /// in: "assertion `left == right` failed: a damaged key file was not reported / left:
    /// Loaded / right: Unusable".
    #[test]
    fn a_short_key_file_is_reported_not_silently_replaced_each_start() {
        let dir = tmp_dir("shortkey");
        std::fs::write(dir.join("erased-accounts.key"), b"short").unwrap();
        for start in 0..2 {
            let (key, state) = load_named_key(&dir, "erased-accounts.key");
            assert_eq!(state, KeyFile::Unusable, "a damaged key file was not reported");
            assert!(key.is_none(), "start {start} used a key from a damaged file");
            assert_eq!(std::fs::read(dir.join("erased-accounts.key")).unwrap(), b"short", "the damaged file was overwritten");
        }
        // A missing one is created whole, atomically, and then loaded as it is.
        let fresh = tmp_dir("freshkey");
        let (made, state) = load_named_key(&fresh, "erased-accounts.key");
        assert_eq!(state, KeyFile::Created);
        assert_eq!(std::fs::read(fresh.join("erased-accounts.key")).unwrap().len(), 32);
        let tmp_left = std::fs::read_dir(&fresh).unwrap().filter_map(|e| e.ok()).any(|e| e.file_name().to_string_lossy().ends_with(".tmp"));
        assert!(!tmp_left, "the temporary file was left behind");
        let (again, state) = load_named_key(&fresh, "erased-accounts.key");
        assert_eq!((again, state), (made, KeyFile::Loaded), "the key was not kept");
        // A temporary file left by a crash mid-write does not stop the next start.
        let crashed = tmp_dir("crashedkey");
        std::fs::write(crashed.join("erased-accounts.key.tmp"), b"half").unwrap();
        let (k, state) = load_named_key(&crashed, "erased-accounts.key");
        assert!(k.is_some() && state == KeyFile::Created, "a leftover .tmp blocked creating the key");
    }

    /// Review of BUG-135 option 2, second round, finding 5: several starts creating the same
    /// key file at once (two relays sharing a folder, or the test suite, whose databases share
    /// the temp folder) all end with the ONE key that is on disk, and each reports it as kept.
    /// Every creator writes its own temporary file and installs it only where there is none;
    /// whoever finds one already there reads and uses it.
    ///
    /// Seen red 2026-10-04 on 8695b08d4 (one shared `<file>.tmp`, then `rename`, which
    /// replaces): "assertion `left == right` failed: round 0: a creator uses a key that is not
    /// the one on disk / left: Some([181, 17, 213, ...]) / right: Some([35, 169, 122, ...])".
    #[test]
    fn creators_racing_end_with_the_one_key_on_disk() {
        const CREATORS: usize = 4;
        for round in 0..40 {
            let dir = tmp_dir(&format!("race{round}"));
            let gate = std::sync::Arc::new(std::sync::Barrier::new(CREATORS));
            let threads: Vec<_> = (0..CREATORS)
                .map(|_| {
                    let (dir, gate) = (dir.clone(), gate.clone());
                    std::thread::spawn(move || {
                        gate.wait();
                        load_named_key(&dir, "erased-accounts.key")
                    })
                })
                .collect();
            let results: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
            let on_disk = std::fs::read(dir.join("erased-accounts.key")).unwrap();
            for (k, state) in &results {
                assert_ne!(*state, KeyFile::Unusable, "round {round}: a creator was left without a kept key: {results:?}");
                assert_eq!(k.map(|k| k.to_vec()), Some(on_disk.clone()), "round {round}: a creator uses a key that is not the one on disk");
            }
            let created = results.iter().filter(|(_, s)| *s == KeyFile::Created).count();
            assert_eq!(created, 1, "round {round}: {created} creators said they made the key");
            let tmp_left: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name()).filter(|n| n.to_string_lossy().ends_with(".tmp")).collect();
            assert!(tmp_left.is_empty(), "round {round}: temporary files left behind: {tmp_left:?}");
        }
    }

    /// Final review of 085441749, finding 3: on a filesystem without hard links the key file is
    /// made in place, and a write or flush that failed after the file was created (a full disk)
    /// returned the error but left the short file, which every later start then read as
    /// Unusable: the erase memory ran "this run only", and for backup.key `load_or_create_key`
    /// returned None, meaning unencrypted backups. The file is ours (create_new made it), so it
    /// is removed and the next start makes the key whole. One that was there first is never
    /// touched.
    ///
    /// Seen red 2026-10-04 with the fallback returning through `?` as before: "a short key file
    /// was left behind: every later start would read it as Unusable".
    #[test]
    fn the_in_place_fallback_leaves_no_short_key_file_when_writing_fails() {
        let dir = tmp_dir("inplacefail");
        let path = dir.join("backup.key");
        let k = [7u8; 32];
        let failed = write_key_in_place(&path, &k, |f, k| {
            use std::io::Write;
            f.write_all(&k[..5])?;
            Err(std::io::Error::other("no space left on the disk"))
        });
        assert!(failed.is_err(), "a failed write was reported as a key made");
        assert!(!path.exists(), "a short key file was left behind: every later start would read it as Unusable");
        let (made, state) = load_named_key(&dir, "backup.key");
        assert_eq!(state, KeyFile::Created, "the next start did not make the key");
        // A key file that was there first is left exactly as it is.
        let first = std::fs::read(&path).unwrap();
        let again = write_key_in_place(&path, &k, |_, _| Err(std::io::Error::other("never reached")));
        assert_eq!(again.unwrap(), false, "a key already there was not reported as there");
        assert_eq!(std::fs::read(&path).unwrap(), first, "a key already there was touched");
        assert_eq!(made.map(|m| m.to_vec()), Some(first));
    }

    #[test]
    fn wrong_key_and_tamper_fail() {
        let dir_a = tmp_dir("wrongkey_a");
        let dir_b = tmp_dir("wrongkey_b");
        let ka = load_or_create_key(&dir_a).unwrap();
        let kb = load_or_create_key(&dir_b).unwrap();
        let plain = dir_a.join("x.db");
        std::fs::write(&plain, b"secret").unwrap();
        let enc = dir_a.join("x.db.enc");
        encrypt_file(&ka, &plain, &enc).unwrap();
        // Wrong key refuses.
        assert!(decrypt_file(&kb, &enc, &dir_a.join("out1.db")).is_err());
        // Tampered ciphertext refuses (GCM tag).
        let mut raw = std::fs::read(&enc).unwrap();
        let last = raw.len() - 1;
        raw[last] ^= 0xFF;
        std::fs::write(&enc, raw).unwrap();
        assert!(decrypt_file(&ka, &enc, &dir_a.join("out2.db")).is_err());
    }
}
