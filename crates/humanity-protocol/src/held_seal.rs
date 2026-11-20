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
//! writes plaintext only where its caller says (a file the owner chose). [`Opener`] is the same
//! opening as a reader, a piece at a time, so the owner's Restore can stream a file from the
//! server, through the opening, straight back up without holding it whole or writing it to disk.
//!
//! THE RESTORE PLAN (rung 5, 2026-11-20). Restore puts a post back from what the owner's device
//! opened, so the relay must be able to tell that what comes back is exactly what it held, without
//! being able to read the package before the owner chooses Restore. When it seals a package the
//! relay makes a fresh random 32-byte RESTORE KEY, puts it inside part 0 (the [`Manifest`], sealed
//! to the owner), and keeps beside the parts a copy of the manifest (the post's words, each file's
//! name, folder, uploader and BLAKE3 hash) sealed under that key alone ([`seal_restore_plan`]:
//! AES-256-GCM, key BLAKE3 `derive_key(`[`RESTORE_DOMAIN`]`, restore key)`, a zero nonce since each
//! key seals exactly one plan, associated data the domain and the package), then forgets the key.
//! Only the owner's device, opening part 0, learns it; it sends the key with Restore, and the relay
//! opens the plan and takes nothing else from the device but each file's bytes, which must match
//! the plan's hash and size. So a restore cannot change a word, a name or a folder, and the key
//! opens nothing but the plan (each part keeps its own ML-KEM key).

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
    /// The key the restore plan is sealed under (32 bytes, hex; the module's header). Only the
    /// owner's Restore sends it to the relay; an export leaves it out of the file it writes.
    pub restore_key: String,
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
    /// BLAKE3 of its bytes, hex: what a restore must give back.
    pub hash: String,
    /// The part holding a video's poster picture, when it had one beside it, with that picture's
    /// size and BLAKE3 hash.
    pub poster_part: Option<usize>,
    pub poster_bytes: u64,
    pub poster_hash: String,
}

/// The context label of the restore plan's key (BLAKE3 `derive_key`).
pub const RESTORE_DOMAIN: &str = "hum/held-restore/v1";
/// The file a package's restore plan is kept in, beside its parts.
pub const RESTORE_PLAN_FILE: &str = "restore.plan";

fn restore_cipher(restore_key: &[u8; 32]) -> Aes256Gcm {
    Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&blake3::derive_key(RESTORE_DOMAIN, restore_key)))
}

fn restore_aad(package: &str) -> String {
    format!("{RESTORE_DOMAIN}\n{package}")
}

/// Seal the restore plan `plain` (the manifest's JSON) of `package` under `restore_key`, a key
/// made for this one plan (the module's header).
pub fn seal_restore_plan(restore_key: &[u8; 32], package: &str, plain: &[u8]) -> Result<Vec<u8>, String> {
    restore_cipher(restore_key)
        .encrypt(Nonce::from_slice(&[0u8; 12]), Payload { msg: plain, aad: restore_aad(package).as_bytes() })
        .map_err(|e| format!("seal: {e}"))
}

/// Open the restore plan of `package` with the key the owner's device sent: an error for another
/// key, another package or an altered plan.
pub fn open_restore_plan(restore_key: &[u8; 32], package: &str, sealed: &[u8]) -> Result<Vec<u8>, String> {
    restore_cipher(restore_key)
        .decrypt(Nonce::from_slice(&[0u8; 12]), Payload { msg: sealed, aad: restore_aad(package).as_bytes() })
        .map_err(|_| "the restore key does not open this package's plan".to_string())
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
pub fn open_stream(me: &KyberKeypair, context: &str, src: impl Read, mut dst: impl Write) -> Result<u64, String> {
    let mut opener = Opener::new(me, context, src)?;
    let mut total = 0u64;
    while let Some(plain) = opener.next_piece()? {
        dst.write_all(plain).map_err(|e| format!("write: {e}"))?;
        total += plain.len() as u64;
    }
    dst.flush().map_err(|e| format!("write: {e}"))?;
    Ok(total)
}

/// The opening of one part as a reader: each `read` hands out plaintext a piece at a time, opening
/// the next piece only when the last is used up, so no more than one piece of plaintext is ever
/// held. A piece that does not open is an `InvalidData` error, and the reader then stops.
pub struct Opener<R: Read> {
    src: R,
    cipher: Aes256Gcm,
    context: String,
    cur: Vec<u8>,
    cur_len: usize,
    next: Vec<u8>,
    index: u64,
    out: Vec<u8>,
    out_pos: usize,
    done: bool,
}

impl<R: Read> Opener<R> {
    /// Read the part's header from `src` and take its key with `me`: an error for anything not
    /// sealed to `me`.
    pub fn new(me: &KyberKeypair, context: &str, mut src: R) -> Result<Self, String> {
        let mut header = [0u8; HEADER];
        if fill(&mut src, &mut header).map_err(|e| format!("read: {e}"))? != HEADER || &header[..MAGIC.len()] != MAGIC {
            return Err("this is not a sealed held part".into());
        }
        let ss = me.decapsulate(&header[MAGIC.len()..]).map_err(|_| "this part is not sealed to this key".to_string())?;
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&blake3::derive_key(DOMAIN, &ss)));
        let mut cur = vec![0u8; PIECE + TAG];
        let cur_len = fill(&mut src, &mut cur).map_err(|e| format!("read: {e}"))?;
        Ok(Self { src, cipher, context: context.to_string(), cur, cur_len, next: vec![0u8; PIECE + TAG], index: 0, out: Vec::new(), out_pos: 0, done: false })
    }

    /// Open the next piece: its plaintext, or None once the last piece was opened.
    pub fn next_piece(&mut self) -> Result<Option<&[u8]>, String> {
        if self.done {
            return Ok(None);
        }
        if self.cur_len < TAG {
            self.done = true;
            return Err("this part is cut short".into());
        }
        let next_len = if self.cur_len == PIECE + TAG { fill(&mut self.src, &mut self.next).map_err(|e| format!("read: {e}"))? } else { 0 };
        let last = next_len == 0;
        let aad = piece_aad(&self.context, self.index, last);
        let opened = self.cipher.decrypt(Nonce::from_slice(&nonce_of(self.index)), Payload { msg: &self.cur[..self.cur_len], aad: aad.as_bytes() });
        let Ok(plain) = opened else {
            self.done = true;
            return Err("this part does not open with this key (not sealed to it, sealed as another part, altered or cut short)".into());
        };
        self.out = plain;
        self.out_pos = 0;
        if last {
            self.done = true;
        } else {
            std::mem::swap(&mut self.cur, &mut self.next);
            self.cur_len = next_len;
            self.index += 1;
        }
        Ok(Some(&self.out))
    }
}

impl<R: Read> Read for Opener<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        while self.out_pos == self.out.len() {
            match self.next_piece() {
                Ok(Some(_)) => {}
                Ok(None) => return Ok(0),
                Err(e) => return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
            }
        }
        let n = buf.len().min(self.out.len() - self.out_pos);
        buf[..n].copy_from_slice(&self.out[self.out_pos..self.out_pos + n]);
        self.out_pos += n;
        Ok(n)
    }
}

/// Open a part held in memory.
pub fn open_bytes(me: &KyberKeypair, context: &str, sealed: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    open_stream(me, context, sealed, &mut out)?;
    Ok(out)
}

// ── The owner's device: part 0, and Restore's order (rung 5) ──
//
// The desktop app's Export and Restore (src/net/held.rs) and the relay's own round-trip test
// (src/relay/handlers/held_restore_tests.rs) run these, so they live here, under both: the client
// and the relay may not reach each other (scripts/crate-layers.json). The website's twin is
// web/shared/held-seal.js. IO comes in as closures; nothing here touches a socket or a file.

/// Open part 0 of `package` (read whole from `src`: it is the post's words and its list of files,
/// a few kilobytes): the manifest.
pub fn open_manifest(kyber: &KyberKeypair, package: &str, src: impl Read) -> Result<Manifest, String> {
    let mut json = Vec::new();
    open_stream(kyber, &part_context(package, 0), src, &mut json)?;
    let m: Manifest = serde_json::from_slice(&json).map_err(|_| "the list of files could not be read".to_string())?;
    if m.package != package {
        return Err("the list of files is another package's".into());
    }
    Ok(m)
}

/// Restore held item `package` from this device: open part 0 for the restore key, then stream each
/// file and poster part through the opening to `upload(part, key, reader)`, then `finish(key)`.
/// The relay checks every byte against what it held. The words the relay answered with.
pub fn restore_with(
    kyber: &KyberKeypair,
    package: &str,
    mut fetch: impl FnMut(usize) -> Result<Box<dyn Read>, String>,
    mut upload: impl FnMut(usize, &str, &mut dyn Read) -> Result<(), String>,
    finish: impl FnOnce(&str) -> Result<String, String>,
) -> Result<String, String> {
    let manifest = open_manifest(kyber, package, fetch(0)?)?;
    for f in &manifest.files {
        for part in std::iter::once(f.part).chain(f.poster_part) {
            let mut opener = Opener::new(kyber, &part_context(package, part), fetch(part)?)?;
            upload(part, &manifest.restore_key, &mut opener)?;
        }
    }
    finish(&manifest.restore_key)
}

/// Read everything `src` holds and hand it to `put(offset, piece)` in pieces of at most `max`
/// bytes, each starting where the last ended; an empty source still sends one empty piece.
pub fn send_in_pieces(src: &mut dyn Read, max: usize, mut put: impl FnMut(u64, &[u8]) -> Result<(), String>) -> Result<(), String> {
    let (mut buf, mut offset, mut sent_one) = (vec![0u8; max], 0u64, false);
    loop {
        let mut got = 0;
        while got < max {
            match src.read(&mut buf[got..]) {
                Ok(0) => break,
                Ok(n) => got += n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => return Err(format!("the file could not be opened ({e})")),
            }
        }
        if got == 0 && sent_one {
            return Ok(());
        }
        put(offset, &buf[..got])?;
        sent_one = true;
        offset += got as u64;
        if got < max {
            return Ok(());
        }
    }
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

    /// THE OPENER READS WHAT OPEN_BYTES OPENS, A FEW BYTES AT A TIME, AND STOPS AT A BAD PIECE
    /// (rung 5, the owner's Restore streams a file through it): read in 7-byte gulps across a piece's
    /// edge it gives back exactly the plaintext, and a part with its second piece altered errors
    /// with InvalidData once the first piece is used up, never handing out the altered piece.
    ///
    /// Seen red 2026-11-20 with `out_pos` left unmoved in `read` (the first seven bytes repeated
    /// forever: the test's 3 MB cap tripped, "read gives back the plaintext").
    #[test]
    fn the_opener_streams_a_part_and_stops_at_a_bad_piece() {
        let owner = keys(9);
        let ctx = part_context("pkg", 3);
        let plain: Vec<u8> = (0..PIECE + 5000).map(|i| (i % 253) as u8).collect();
        let sealed = seal_bytes(&owner.public_key(), &ctx, &plain).unwrap();
        let mut r = Opener::new(&owner, &ctx, &sealed[..]).unwrap();
        let (mut got, mut buf) = (Vec::new(), [0u8; 7]);
        loop {
            let n = r.read(&mut buf).unwrap();
            if n == 0 || got.len() > 3_000_000 {
                break;
            }
            got.extend_from_slice(&buf[..n]);
        }
        assert_eq!(got, plain, "read gives back the plaintext");
        let mut bad = sealed.clone();
        let at = HEADER + PIECE + TAG + 3;
        bad[at] ^= 1;
        let mut r = Opener::new(&owner, &ctx, &bad[..]).unwrap();
        let mut out = Vec::new();
        let err = std::io::copy(&mut r, &mut out).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(out, plain[..PIECE], "only the first, good piece was handed out");
        // ML-KEM decapsulates any ciphertext (implicit rejection), so another key fails at the
        // first piece rather than at the header.
        let other = Opener::new(&keys(8), &ctx, &sealed[..]).map_or(true, |mut r| std::io::copy(&mut r, &mut Vec::new()).is_err());
        assert!(other, "another key opens nothing");
    }

    /// RESTORE SENDS EACH FILE IN PIECES SMALL ENOUGH FOR THE SERVER, AND AT LEAST ONE FOR AN EMPTY
    /// FILE: every piece starts where the last ended and none is larger than asked. (Moved here
    /// from src/net/held_tests.rs with `send_in_pieces` on 2026-11-20.)
    ///
    /// Seen red 2026-11-20 with `send_in_pieces` returning before its first piece on an empty file:
    /// "an empty file still sends one piece" (no piece at all for the 0-byte file).
    #[test]
    fn restore_sends_each_file_in_pieces_from_where_the_last_ended() {
        for len in [0usize, 1, 9, 10, 11, 35] {
            let data: Vec<u8> = (0..len).map(|i| i as u8).collect();
            let mut got: Vec<(u64, Vec<u8>)> = Vec::new();
            send_in_pieces(&mut std::io::Cursor::new(data.clone()), 10, |offset, piece| {
                got.push((offset, piece.to_vec()));
                Ok(())
            })
            .unwrap();
            assert!(!got.is_empty(), "an empty file still sends one piece");
            let mut at = 0u64;
            for (offset, piece) in &got {
                assert_eq!(*offset, at, "{len}: each piece starts where the last ended");
                assert!(piece.len() <= 10);
                at += piece.len() as u64;
            }
            assert_eq!(got.iter().flat_map(|(_, p)| p.clone()).collect::<Vec<u8>>(), data, "{len}: whole");
        }
    }

    /// THE RESTORE PLAN OPENS ONLY WITH ITS KEY AND ONLY AS ITS PACKAGE'S: another key, another
    /// package or one bit changed refuses.
    ///
    /// Seen red 2026-11-20 with `restore_aad` returning the domain alone: "not as another package's"
    /// (the plan opened under package "other").
    #[test]
    fn the_restore_plan_opens_only_with_its_key_for_its_package() {
        let key = [5u8; 32];
        let plan = b"{\"words\":\"hello\"}";
        let sealed = seal_restore_plan(&key, "pkg", plan).unwrap();
        assert!(!sealed.windows(5).any(|w| w == b"hello"), "nothing readable");
        assert_eq!(open_restore_plan(&key, "pkg", &sealed).unwrap(), plan);
        assert!(open_restore_plan(&[6u8; 32], "pkg", &sealed).is_err(), "not with another key");
        assert!(open_restore_plan(&key, "other", &sealed).is_err(), "not as another package's");
        let mut flipped = sealed.clone();
        flipped[0] ^= 1;
        assert!(open_restore_plan(&key, "pkg", &flipped).is_err(), "not altered");
    }

    fn fixture_path() -> std::path::PathBuf {
        std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("scripts/tests/fixtures/held-v1.json")
    }

    /// THE WEBSITE OPENS WHAT THE RELAY SEALS (rung 5: the owner's Export and Restore in a browser,
    /// web/shared/held-seal.js). scripts/tests/fixtures/held-v1.json holds parts this crate sealed
    /// to the Kyber768 key of a fixed test seed: a manifest (part 0), a short file, and an empty
    /// one. This test opens every part of the checked-in fixture here; scripts/tests/held-web.test.js
    /// opens the same parts in the website's code and compares the bytes. `HELD_FIXTURE_WRITE=1`
    /// writes a fresh fixture (the seal is random, so its bytes change; what they open to does not).
    ///
    /// Seen red 2026-11-20 with the web side's piece associated data ending in "true" for the last
    /// piece (JavaScript's own spelling of the flag) instead of 1: the website could not open the
    /// fixture's parts ("this part does not open with this key").
    #[test]
    fn held_fixture_rust_and_the_website_agree() {
        let seed = [7u8; 32];
        let me = KyberKeypair::from_seed(&derive_kyber_seed(&seed)).unwrap();
        let package = "0123456789abcdef0123456789abcdef";
        let manifest = br#"{"v":1,"package":"0123456789abcdef0123456789abcdef","words":"fixture words"}"#.to_vec();
        let file = b"a short file, with \xe2\x9c\x93 a non-ASCII mark".to_vec();
        let wanted: Vec<(usize, Vec<u8>)> = vec![(0, manifest), (1, file), (2, Vec::new())];
        if std::env::var("HELD_FIXTURE_WRITE").is_ok() {
            use base64::{engine::general_purpose::STANDARD as B64, Engine};
            let parts: Vec<serde_json::Value> = wanted
                .iter()
                .map(|(n, plain)| {
                    let sealed = seal_bytes(&me.public_key(), &part_context(package, *n), plain).unwrap();
                    serde_json::json!({ "part": n, "sealed_b64": B64.encode(sealed), "plain_hex": hex::encode(plain) })
                })
                .collect();
            let fx = serde_json::json!({ "about": "Held parts sealed by crates/humanity-protocol/src/held_seal.rs (held_fixture_rust_and_the_website_agree); opened by scripts/tests/held-web.test.js.", "seed_hex": hex::encode(seed), "package": package, "parts": parts });
            std::fs::write(fixture_path(), serde_json::to_string_pretty(&fx).unwrap() + "\n").unwrap();
        }
        let fx: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(fixture_path()).expect("the held fixture is checked in")).unwrap();
        assert_eq!(fx["seed_hex"].as_str().unwrap(), hex::encode(seed));
        assert_eq!(fx["package"].as_str().unwrap(), package);
        let parts = fx["parts"].as_array().unwrap();
        assert_eq!(parts.len(), wanted.len());
        for (p, (n, plain)) in parts.iter().zip(&wanted) {
            use base64::{engine::general_purpose::STANDARD as B64, Engine};
            assert_eq!(p["part"].as_u64().unwrap() as usize, *n);
            assert_eq!(p["plain_hex"].as_str().unwrap(), hex::encode(plain));
            let sealed = B64.decode(p["sealed_b64"].as_str().unwrap()).unwrap();
            assert_eq!(&open_bytes(&me, &part_context(package, *n), &sealed).unwrap(), plain, "part {n} opens here");
        }
    }
}
