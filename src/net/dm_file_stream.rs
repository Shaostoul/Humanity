//! A private file's Save, decrypted from disk to disk a megabyte at a time (2026-10-11).
//!
//! WHY: each server now sets how large a file in a private conversation may be, per role, up to
//! 1 GB (the relay's `max_private_file_mb`). The app used to fetch the whole ciphertext into
//! memory and decrypt it there (`dm_pq::decrypt_attachment`), which for such a file means two
//! copies of a gigabyte in memory. This is the same AES-256-GCM, done in two passes over a file:
//!
//! 1. The tag. GHASH over the whole ciphertext, read a piece at a time, then the lengths block;
//!    the tag is that XOR the key's encryption of the first counter block (J0). A wrong key or any
//!    changed byte is refused here, before a single byte of plaintext is written.
//! 2. The plaintext. AES in counter mode from the second counter block, a piece at a time, each
//!    written as it is made.
//!
//! The format is exactly what `dm_pq::encrypt_attachment` (and the web's `pqEncryptFile`) makes:
//! a 96-bit nonce, no associated data, the ciphertext followed by its 16-byte tag. The tests hold
//! it to the one-shot `decrypt_attachment` on the same bytes.
//!
//! The ciphertext file is the app's own (a hidden file it downloaded into the downloads folder a
//! moment before), so it is read twice without being checked for changes between the passes.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use aes_gcm::aes::cipher::BlockEncrypt;
use aes_gcm::aes::Aes256;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ctr::cipher::{KeyIvInit, StreamCipher};
use ghash::universal_hash::{KeyInit, UniversalHash};
use ghash::GHash;

/// AES-GCM's tag, at the end of the ciphertext.
const TAG_LEN: u64 = 16;
/// How much is read, hashed or decrypted at a time (a multiple of the 16-byte block, so only the
/// last piece is ever padded for GHASH).
const PIECE: usize = 1 << 20;

/// What a file that is not this key's, or has been changed, is refused with.
pub const NOT_THIS_KEY: &str = "attachment open (wrong key / tampered)";

/// Decrypt the ciphertext in `src` (as `dm_pq::encrypt_attachment` made it) into `out`, with the
/// key and nonce from the file's marker. Nothing is written unless the whole file's tag holds.
/// Returns the plaintext's length.
pub fn decrypt_file(src: &Path, out: &mut dyn Write, key_b64: &str, nonce_b64: &str) -> Result<u64, String> {
    let key = B64.decode(key_b64.trim()).map_err(|e| format!("key b64: {e}"))?;
    let nonce = B64.decode(nonce_b64.trim()).map_err(|e| format!("nonce b64: {e}"))?;
    if key.len() != 32 || nonce.len() != 12 {
        return Err("bad key/nonce length".into());
    }
    let mut f = std::fs::File::open(src).map_err(|e| format!("read: {e}"))?;
    let len = f.metadata().map_err(|e| format!("read: {e}"))?.len();
    if len < TAG_LEN {
        return Err(NOT_THIS_KEY.into());
    }
    let ct_len = len - TAG_LEN;
    let aes = Aes256::new_from_slice(&key).map_err(|_| "bad key length".to_string())?;

    // H, the hash key: the block of zeros encrypted. J0, the first counter block: the nonce and 1.
    let mut h = aes_gcm::aes::Block::default();
    aes.encrypt_block(&mut h);
    let mut j0 = [0u8; 16];
    j0[..12].copy_from_slice(&nonce);
    j0[15] = 1;
    let mut ek_j0 = aes_gcm::aes::Block::clone_from_slice(&j0);
    aes.encrypt_block(&mut ek_j0);

    // 1. The tag, over the whole ciphertext.
    let mut ghash = GHash::new_from_slice(&h).map_err(|_| "hash key".to_string())?;
    let mut buf = vec![0u8; PIECE];
    let mut left = ct_len;
    while left > 0 {
        let n = (left as usize).min(PIECE);
        f.read_exact(&mut buf[..n]).map_err(|e| format!("read: {e}"))?;
        ghash.update_padded(&buf[..n]);
        left -= n as u64;
    }
    let mut tag = [0u8; TAG_LEN as usize];
    f.read_exact(&mut tag).map_err(|e| format!("read: {e}"))?;
    // The lengths block: no associated data, then the ciphertext's length in bits.
    let mut lengths = [0u8; 16];
    lengths[8..].copy_from_slice(&(ct_len.wrapping_mul(8)).to_be_bytes());
    ghash.update_padded(&lengths);
    let s = ghash.finalize();
    // Compared in constant time: every byte is looked at, whatever the first difference.
    let diff = (0..16).fold(0u8, |acc, i| acc | (s[i] ^ ek_j0[i] ^ tag[i]));
    if diff != 0 {
        return Err(NOT_THIS_KEY.into());
    }

    // 2. The plaintext, from the second counter block.
    let mut iv = j0;
    iv[15] = 2;
    let mut keystream = ctr::Ctr32BE::<Aes256>::new_from_slices(&key, &iv).map_err(|_| "counter".to_string())?;
    f.seek(SeekFrom::Start(0)).map_err(|e| format!("read: {e}"))?;
    let mut left = ct_len;
    while left > 0 {
        let n = (left as usize).min(PIECE);
        f.read_exact(&mut buf[..n]).map_err(|e| format!("read: {e}"))?;
        keystream.apply_keystream(&mut buf[..n]);
        out.write_all(&buf[..n]).map_err(|e| format!("write: {e}"))?;
        left -= n as u64;
    }
    out.flush().map_err(|e| format!("write: {e}"))?;
    Ok(ct_len)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::dm_pq::{decrypt_attachment, encrypt_attachment};

    fn sealed_file(tag: &str, plain: &[u8]) -> (crate::test_temp::TempPath, String, String, Vec<u8>) {
        let (ct, k, n) = encrypt_attachment(plain).expect("seal");
        let path = crate::test_temp::file(tag, "enc");
        std::fs::write(&path, &ct).unwrap();
        (path, k, n, ct)
    }

    /// THE STREAMED DECRYPT IS THE ONE-SHOT DECRYPT: for files across the piece boundary and an
    /// empty one, the plaintext matches `dm_pq::decrypt_attachment` on the same bytes; a wrong key
    /// or one changed byte anywhere (the first, the middle, the tag) is refused, and nothing is
    /// written.
    ///
    /// Seen red 2026-10-11 with the counter started at J0 instead of J0 + 1 (`iv[15] = 1`): "the
    /// streamed plaintext is the one-shot plaintext (1 bytes)" (the chat's Save test failed with
    /// it too: "Save writes the decrypted file"); and with the lengths block left out of GHASH:
    /// every file was refused, "it opens: attachment open (wrong key / tampered)".
    #[test]
    fn the_streamed_decrypt_is_the_one_shot_decrypt() {
        for len in [0usize, 1, 15, 16, 17, PIECE - 1, PIECE, PIECE + 1, 3 * PIECE + 37] {
            let plain: Vec<u8> = (0..len).map(|i| (i * 31 % 251) as u8).collect();
            let (path, k, n, ct) = sealed_file(&format!("dm_stream_{len}"), &plain);
            let mut out = Vec::new();
            let got = decrypt_file(&path, &mut out, &k, &n).unwrap_or_else(|e| panic!("it opens: {e}"));
            assert_eq!(got, len as u64);
            assert!(out == plain && out == decrypt_attachment(&ct, &k, &n).unwrap(), "the streamed plaintext is the one-shot plaintext ({len} bytes)");
        }

        let plain = vec![9u8; PIECE + 100];
        let (path, k, n, ct) = sealed_file("dm_stream_tamper", &plain);
        for at in [0, PIECE / 2, ct.len() - 1] {
            let mut bad = ct.clone();
            bad[at] ^= 1;
            std::fs::write(&path, &bad).unwrap();
            let mut out = Vec::new();
            assert_eq!(decrypt_file(&path, &mut out, &k, &n), Err(NOT_THIS_KEY.to_string()), "a changed byte at {at} is refused");
            assert!(out.is_empty(), "and nothing is written");
        }
        std::fs::write(&path, &ct).unwrap();
        let (_, other_key, _) = encrypt_attachment(b"x").unwrap();
        let mut out = Vec::new();
        assert!(decrypt_file(&path, &mut out, &other_key, &n).is_err() && out.is_empty(), "another key is refused");
    }
}
