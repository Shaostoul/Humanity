//! The seal on a held copy (2026-11-20, docs/design/hide-and-keep.md, guarantee 2; the "Held
//! copies" row of docs/reference/cryptography.md, which names every value below).
//!
//! WHAT. A reported post that an admin hid and kept is sealed, on the relay, to the server
//! OWNER's key card: its words and each file it linked, every one its own sealed part. Only the
//! owner's device, holding the recovery phrase behind that card, can open a part; the relay, other
//! admins and anyone who breaks into the server hold ciphertext.
//!
//! NO NEW ALGORITHM. The construction of the direct-message seal: ML-KEM-768 (Kyber768)
//! encapsulation to the owner's card key, the AES key from BLAKE3 `derive_key(DOMAIN, shared
//! secret)` under a context label of its own ([`DOMAIN`]), AES-256-GCM. One encapsulation per part,
//! so every part has a fresh key. A file can be up to a gigabyte, more than a relay should hold in
//! memory, so the body is sealed in pieces of [`PIECE`] bytes, each its own AES-256-GCM seal under
//! that part's key: the nonce is four zero bytes then the piece's number (big-endian u64), never
//! repeated under one key, and the associated data binds the part ([`part_context`]), the piece's
//! number and whether it is the last ([`piece_aad`]), so a piece cannot be moved, dropped, repeated
//! or the part cut short without the opening failing. (The framing is the STREAM construction of
//! Hoang, Reyhanitabar, Rogaway and Vizar, "Online Authenticated-Encryption and its Nonce-Reuse
//! Misuse-Resistance", CRYPTO 2015, with its last-piece flag in the associated data.)
//!
//! BYTES OF A PART: [`MAGIC`] (8), the ML-KEM-768 ciphertext ([`KYBER_CIPHERTEXT_LEN`]), then the
//! pieces, each its plaintext's length plus a 16-byte tag; every piece but the last holds exactly
//! [`PIECE`] bytes of plaintext, and an empty part is one empty last piece.
//!
//! The plaintext never needs to exist anywhere but where it already is: [`seal_stream`] reads the
//! original a piece at a time and writes only ciphertext, and [`open_stream`] (the owner's device)
//! writes plaintext only where its caller says (a file the owner chose).

use std::io::{Read, Write};

use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Key, Nonce,
};

use super::pq_crypto::{encapsulate_to, KyberKeypair, KYBER_CIPHERTEXT_LEN, KYBER_EK_LEN};

/// The context label the AES key is derived under (BLAKE3 `derive_key`), and the first line of
/// every piece's associated data.
pub const DOMAIN: &str = "hum/held/v1";
/// The first bytes of every sealed part.
pub const MAGIC: &[u8; 8] = b"HUMHELD1";
/// Plaintext bytes in every piece but the last: 1 MiB.
pub const PIECE: usize = 1 << 20;
/// The bytes a piece adds to its plaintext (the AES-GCM tag).
pub const TAG: usize = 16;
/// The bytes before the first piece.
pub const HEADER: usize = MAGIC.len() + KYBER_CIPHERTEXT_LEN;

/// The version of [`Manifest`].
pub const MANIFEST_VERSION: u32 = 1;

/// Part 0 of a package, sealed like every part: the post's words and the list of the files it
/// linked, each with the part that holds it and what is needed to put it back (Restore) or to
/// describe it to a hotline (Export). Nothing of it is readable on the server.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Manifest {
    pub v: u32,
    /// The package's id, also the folder it is kept in.
    pub package: String,
    pub report_id: i64,
    /// The post's author (Dilithium3 public key, hex), its time (Unix ms) and its room.
    pub author: String,
    pub stamp: u64,
    pub channel: String,
    /// The post's words exactly as the server held them.
    pub words: String,
    pub files: Vec<HeldFile>,
    /// Who hid it, and when (Unix ms).
    pub held_by: String,
    pub held_at: i64,
}

/// One file a held post linked.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HeldFile {
    /// The part that holds it.
    pub part: usize,
    /// Its stored name, as the post linked it (`/uploads/<name>`).
    pub name: String,
    /// The folder it was in: "public", "members" or "waiting".
    pub folder: String,
    /// Who uploaded it ('' when this server had no upload row for it), on which shelf, under what
    /// name, and its size.
    pub uploader: String,
    pub shelf: i64,
    pub original_name: String,
    pub bytes: u64,
    /// The part holding a video's poster picture, when it had one beside it.
    pub poster_part: Option<usize>,
}

/// What a part is bound to: the package it belongs to (a random id the relay chose) and its
/// number in it (0 the post's words and the list of files, then one per file). A part copied into
/// another package or another place does not open.
pub fn part_context(package: &str, part: usize) -> String {
    format!("{package}\n{part}")
}

/// The associated data of piece `index` of the part `context`.
pub fn piece_aad(context: &str, index: u64, last: bool) -> String {
    format!("{DOMAIN}\n{context}\n{index}\n{}", u8::from(last))
}

fn nonce_of(index: u64) -> [u8; 12] {
    let mut n = [0u8; 12];
    n[4..].copy_from_slice(&index.to_be_bytes());
    n
}

/// Read until `buf` is full or the source ends; how many bytes were read.
fn fill(src: &mut impl Read, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut got = 0;
    while got < buf.len() {
        match src.read(&mut buf[got..]) {
            Ok(0) => break,
            Ok(n) => got += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(got)
}

/// Seal everything `src` holds, as part `context`, to the holder of the Kyber768 public key
/// `recipient`, writing the sealed part to `dst`. How many bytes were written.
pub fn seal_stream(recipient: &[u8], context: &str, mut src: impl Read, mut dst: impl Write) -> Result<u64, String> {
    if recipient.len() != KYBER_EK_LEN {
        return Err(format!("a Kyber768 public key is {KYBER_EK_LEN} bytes, not {}", recipient.len()));
    }
    let (kem_ct, ss) = encapsulate_to(recipient).map_err(|e| format!("encapsulate: {e}"))?;
    let key = blake3::derive_key(DOMAIN, &ss);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let io = |e: std::io::Error| format!("write: {e}");
    dst.write_all(MAGIC).map_err(io)?;
    dst.write_all(&kem_ct).map_err(io)?;
    let mut written = HEADER as u64;
    let (mut cur, mut next) = (vec![0u8; PIECE], vec![0u8; PIECE]);
    let mut cur_len = fill(&mut src, &mut cur).map_err(|e| format!("read: {e}"))?;
    let mut index: u64 = 0;
    loop {
        // A full piece may be followed by more: look ahead to know whether this one is the last.
        let next_len = if cur_len == PIECE { fill(&mut src, &mut next).map_err(|e| format!("read: {e}"))? } else { 0 };
        let last = next_len == 0;
        let aad = piece_aad(context, index, last);
        let body = cipher
            .encrypt(Nonce::from_slice(&nonce_of(index)), Payload { msg: &cur[..cur_len], aad: aad.as_bytes() })
            .map_err(|e| format!("seal: {e}"))?;
        dst.write_all(&body).map_err(io)?;
        written += body.len() as u64;
        if last {
            break;
        }
        std::mem::swap(&mut cur, &mut next);
        cur_len = next_len;
        index += 1;
    }
    dst.flush().map_err(io)?;
    Ok(written)
}

/// Seal `plain` (held in memory: the post's words and the list of files) as part `context`.
pub fn seal_bytes(recipient: &[u8], context: &str, plain: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(HEADER + plain.len() + TAG * (1 + plain.len() / PIECE));
    seal_stream(recipient, context, plain, &mut out)?;
    Ok(out)
}

/// Open the part `context` that [`seal_stream`] sealed to `me`, writing its plaintext to `dst`;
/// how many plaintext bytes. An error for anything not sealed to `me`, sealed as another part,
/// altered, reordered or cut short; `dst` may then hold some of the plaintext, so a caller writing
/// to a file removes it on an error.
pub fn open_stream(me: &KyberKeypair, context: &str, mut src: impl Read, mut dst: impl Write) -> Result<u64, String> {
    let mut header = [0u8; HEADER];
    if fill(&mut src, &mut header).map_err(|e| format!("read: {e}"))? != HEADER || &header[..MAGIC.len()] != MAGIC {
        return Err("this is not a sealed held part".into());
    }
    let ss = me.decapsulate(&header[MAGIC.len()..]).map_err(|_| "this part is not sealed to this key".to_string())?;
    let key = blake3::derive_key(DOMAIN, &ss);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let (mut cur, mut next) = (vec![0u8; PIECE + TAG], vec![0u8; PIECE + TAG]);
    let mut cur_len = fill(&mut src, &mut cur).map_err(|e| format!("read: {e}"))?;
    let (mut index, mut total): (u64, u64) = (0, 0);
    loop {
        if cur_len < TAG {
            return Err("this part is cut short".into());
        }
        let next_len = if cur_len == PIECE + TAG { fill(&mut src, &mut next).map_err(|e| format!("read: {e}"))? } else { 0 };
        let last = next_len == 0;
        let aad = piece_aad(context, index, last);
        let plain = cipher
            .decrypt(Nonce::from_slice(&nonce_of(index)), Payload { msg: &cur[..cur_len], aad: aad.as_bytes() })
            .map_err(|_| "this part does not open with this key (not sealed to it, sealed as another part, altered or cut short)".to_string())?;
        dst.write_all(&plain).map_err(|e| format!("write: {e}"))?;
        total += plain.len() as u64;
        if last {
            break;
        }
        std::mem::swap(&mut cur, &mut next);
        cur_len = next_len;
        index += 1;
    }
    dst.flush().map_err(|e| format!("write: {e}"))?;
    Ok(total)
}

/// Open a part held in memory.
pub fn open_bytes(me: &KyberKeypair, context: &str, sealed: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    open_stream(me, context, sealed, &mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pq_crypto::derive_kyber_seed;

    fn keys(seed: u8) -> KyberKeypair {
        KyberKeypair::from_seed(&derive_kyber_seed(&[seed; 32])).unwrap()
    }

    /// A part opens with the key it was sealed to and with no other; it carries no plaintext
    /// marker in the clear; it opens only as the part it was sealed as; and any piece altered,
    /// dropped, repeated or swapped, or the part cut at a piece's end, does not open. Sizes on
    /// and around a piece's edge (0, 1, PIECE-1, PIECE, PIECE+1, 2*PIECE+5) all round-trip.
    ///
    /// Seen red 2026-11-20 with the last-piece flag left out of `piece_aad`: "a part cut at a
    /// piece's end does not open" (the first piece alone opened as a whole part).
    #[test]
    fn a_part_opens_only_for_its_owner_whole_and_in_place() {
        let (owner, other) = (keys(9), keys(8));
        let ctx = part_context("pkg", 1);
        for len in [0usize, 1, PIECE - 1, PIECE, PIECE + 1, 2 * PIECE + 5] {
            let plain: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
            let sealed = seal_bytes(&owner.public_key(), &ctx, &plain).unwrap();
            assert_eq!(open_bytes(&owner, &ctx, &sealed).unwrap(), plain, "{len} bytes round-trip");
            assert!(open_bytes(&other, &ctx, &sealed).is_err(), "{len}: another key does not open it");
            assert!(open_bytes(&owner, &part_context("pkg", 2), &sealed).is_err(), "{len}: not as another part");
            assert!(open_bytes(&owner, &part_context("other", 1), &sealed).is_err(), "{len}: not in another package");
        }
        let marker = b"PLAINTEXT-MARKER-7f3a ".repeat(100_000); // about 2.1 MB: three pieces
        let sealed = seal_bytes(&owner.public_key(), &ctx, &marker).unwrap();
        assert!(!sealed.windows(21).any(|w| w == &marker[..21]), "no plaintext in the clear");
        let piece = PIECE + TAG;
        let body = &sealed[HEADER..];
        assert_eq!(body.len().div_ceil(piece), 3);
        // Cut at the first piece's end: the first piece was sealed as not-last.
        assert!(open_bytes(&owner, &ctx, &sealed[..HEADER + piece]).is_err(), "a part cut at a piece's end does not open");
        // The last piece dropped.
        assert!(open_bytes(&owner, &ctx, &sealed[..HEADER + 2 * piece]).is_err(), "a part missing its last piece does not open");
        // Pieces 0 and 1 swapped.
        let mut swapped = sealed[..HEADER].to_vec();
        swapped.extend_from_slice(&body[piece..2 * piece]);
        swapped.extend_from_slice(&body[..piece]);
        swapped.extend_from_slice(&body[2 * piece..]);
        assert!(open_bytes(&owner, &ctx, &swapped).is_err(), "swapped pieces do not open");
        // Piece 0 repeated in place of piece 1.
        let mut repeated = sealed[..HEADER + piece].to_vec();
        repeated.extend_from_slice(&body[..piece]);
        repeated.extend_from_slice(&body[2 * piece..]);
        assert!(open_bytes(&owner, &ctx, &repeated).is_err(), "a repeated piece does not open");
        // One bit flipped.
        let mut flipped = sealed.clone();
        flipped[HEADER + 10] ^= 1;
        assert!(open_bytes(&owner, &ctx, &flipped).is_err(), "an altered piece does not open");
        assert!(open_bytes(&owner, &ctx, b"not a part").is_err());
    }
}
