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
    let path = dir.join(KEY_FILE);
    if let Ok(bytes) = std::fs::read(&path) {
        if bytes.len() == 32 {
            let mut k = [0u8; 32];
            k.copy_from_slice(&bytes);
            return Some(k);
        }
        tracing::error!(
            "backup.key at {} has wrong length ({}); refusing to overwrite it — fix or remove it manually",
            path.display(),
            bytes.len()
        );
        return None;
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
    if let Err(e) = std::fs::write(&path, k) {
        tracing::error!("could not write backup key {}: {e}", path.display());
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    tracing::info!("backup encryption key created at {}", path.display());
    Some(k)
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
