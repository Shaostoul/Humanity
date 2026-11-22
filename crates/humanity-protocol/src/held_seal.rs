//! The seal on a held copy (2026-11-20, docs/design/hide-and-keep.md, guarantee 2; the "Held
//! copies" row of docs/reference/cryptography.md, which names every value below).
//!
//! WHAT. A reported post that an admin hid and kept is sealed, on the relay, to the server
//! OWNER's key card: its words and each file it linked, every one its own sealed part. Only the
//! owner's device, holding the recovery phrase behind that card, can open a part; the relay, other
//! admins and anyone who breaks into the server hold ciphertext.
//!
//! NO NEW ALGORITHM. ML-KEM-768 (Kyber768), BLAKE3 `derive_key` and AES-256-GCM, as in the
//! direct-message seal. Every part has a fresh random 32-byte PART KEY (envelope encryption):
//! - The body's AES key is BLAKE3 `derive_key(`[`DOMAIN`]`, part key)`.
//! - The header carries a KEY COMMITMENT, BLAKE3 `derive_key(`[`COMMIT_DOMAIN`]`, part key)` (32
//!   bytes, [`key_commitment`]): the one key that opens the body is the one whose commitment it is.
//! - The part key is WRAPPED once per recipient (the owner, and the successor the owner named): an
//!   ML-KEM-768 encapsulation to that recipient's card key, a wrapping key from BLAKE3
//!   `derive_key(`[`WRAP_DOMAIN`]`, shared secret)`, and the part key sealed under it by
//!   AES-256-GCM (a zero nonce, since each wrapping key seals one thing once). Its associated data
//!   ([`wrap_aad`]) binds the part, the NUMBER of recipients, the SLOT and the commitment, so a slot
//!   stripped, added or moved makes every other slot refuse, and an unwrapped key must match the
//!   commitment or it is refused ([`open_slot`]).
//! - The body is sealed in pieces of [`PIECE`] bytes (a file can be up to a gigabyte, more than a
//!   relay should hold in memory), each its own AES-256-GCM seal under the body key: the nonce is
//!   four zero bytes then the piece's number (big-endian u64), never repeated under one key, and the
//!   associated data ([`piece_aad`]) binds the part ([`part_context`]), the piece's number, whether
//!   it is the last, and the commitment, so a piece cannot be moved, dropped, repeated or the part cut
//!   short without the opening failing. (The framing is the STREAM construction of Hoang,
//!   Reyhanitabar, Rogaway and Vizar, "Online Authenticated-Encryption and its Nonce-Reuse
//!   Misuse-Resistance", CRYPTO 2015, with its last-piece flag in the associated data.) The pieces
//!   bind the commitment and NOT the list of slots, so a part's body does not depend on whom it is
//!   sealed to.
//!
//! A CHANGE OF RECIPIENTS RE-WRAPS, IT NEVER RE-ENCRYPTS (format v4, 2026-11-20, the owner
//! branch's second review; it replaces v3's re-seal, in which the owner's device opened every part
//! and sealed it again, and recipients then opened every new part to check it). For a handover or a
//! change of successor the owner's device fetches each part's HEADER only (a few kilobytes), opens
//! its own slot, wraps the same part key for the new list ([`rewrap_header`]) and sends the new
//! header. The relay keeps every body byte for byte and refuses a new header whose commitment is not
//! the old one, so no device can change what a body holds; each recipient's device opens ONLY its
//! own slot of each new header and checks the commitment ([`check_header`]), never a body.
//!
//! BYTES OF A PART: [`MAGIC`] (8), how many recipients (1 byte, 1 to [`MAX_RECIPIENTS`]), the key
//! commitment ([`COMMITMENT`] bytes), then per recipient a [`SLOT`]: the ML-KEM-768 ciphertext
//! ([`KYBER_CIPHERTEXT_LEN`]) and the wrapped part key (32 bytes and a 16-byte tag); then the
//! pieces, each its plaintext's length plus a 16-byte tag; every piece but the last holds exactly
//! [`PIECE`] bytes of plaintext, and an empty part is one empty last piece. [`header_len`] and
//! [`sealed_len`] give the sizes.
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
//! opens nothing but the plan (each part keeps its own ML-KEM key). A change of recipients never
//! sends it: part 0's body never changes.

use std::io::{Read, Write};

use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Key, Nonce,
};

use super::pq_crypto::{encapsulate_to, KyberKeypair, KYBER_CIPHERTEXT_LEN, KYBER_EK_LEN};

/// The context label a part's body key is derived under from its part key (BLAKE3 `derive_key`),
/// and the first line of every piece's associated data.
pub const DOMAIN: &str = "hum/held/v4";
/// The context label a recipient's wrapping key is derived under from their shared secret.
pub const WRAP_DOMAIN: &str = "hum/held-wrap/v4";
/// The context label a part key's commitment is derived under ([`key_commitment`]).
pub const COMMIT_DOMAIN: &str = "hum/held-commit/v4";
/// The first bytes of every sealed part.
pub const MAGIC: &[u8; 8] = b"HUMHELD4";
/// The most recipients one part is sealed to (the owner and a successor need two).
pub const MAX_RECIPIENTS: usize = 4;
/// The bytes of a part key's commitment in its header.
pub const COMMITMENT: usize = 32;
/// One recipient's bytes in a part's header: the ML-KEM-768 ciphertext and the wrapped part key.
pub const SLOT: usize = KYBER_CIPHERTEXT_LEN + 32 + 16;
/// Plaintext bytes in every piece but the last: 1 MiB.
pub const PIECE: usize = 1 << 20;
/// The bytes a piece adds to its plaintext (the AES-GCM tag).
pub const TAG: usize = 16;
/// The bytes before the first piece of a part sealed to `recipients` people: the header.
pub const fn header_len(recipients: usize) -> usize {
    MAGIC.len() + 1 + COMMITMENT + recipients * SLOT
}

/// The commitment to a part key: BLAKE3 `derive_key(`[`COMMIT_DOMAIN`]`, part key)`. It is in the
/// header and in every slot's and every piece's associated data, so the body opens only under the
/// key it names, and a slot wrapping any other key is refused even when its wrapping opens.
pub fn key_commitment(part_key: &[u8; 32]) -> [u8; COMMITMENT] {
    blake3::derive_key(COMMIT_DOMAIN, part_key)
}

/// The associated data of recipient `slot`'s wrapped key in the part `context` sealed to
/// `recipients` people under the key whose commitment is `commitment`: the count and the slot are
/// bound, so a slot stripped, added or moved makes every other slot refuse.
pub fn wrap_aad(context: &str, recipients: usize, slot: usize, commitment: &[u8]) -> String {
    format!("{WRAP_DOMAIN}\n{context}\n{recipients}\n{slot}\n{}", hex::encode(commitment))
}

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

/// The associated data of piece `index` of the part `context` whose part key has the commitment
/// `commitment`.
pub fn piece_aad(context: &str, commitment: &[u8], index: u64, last: bool) -> String {
    format!("{DOMAIN}\n{context}\n{}\n{index}\n{}", hex::encode(commitment), u8::from(last))
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

// ── A part's header: the commitment and the slots ──

/// A part's header, read ([`parse_header`]): how many recipients, the commitment, and the slots.
#[derive(Debug, Clone, Copy)]
pub struct Header<'a> {
    pub recipients: usize,
    pub commitment: [u8; COMMITMENT],
    slots: &'a [u8],
}

/// Read the header at the start of `bytes` (a whole part, or its header alone). An error when it
/// does not start as a sealed part does or is cut short.
pub fn parse_header(bytes: &[u8]) -> Result<Header<'_>, String> {
    let not_sealed = || "this is not a sealed held part".to_string();
    if bytes.len() < MAGIC.len() + 1 || &bytes[..MAGIC.len()] != MAGIC {
        return Err(not_sealed());
    }
    let n = bytes[MAGIC.len()] as usize;
    if n == 0 || n > MAX_RECIPIENTS || bytes.len() < header_len(n) {
        return Err(not_sealed());
    }
    let at = MAGIC.len() + 1;
    let mut commitment = [0u8; COMMITMENT];
    commitment.copy_from_slice(&bytes[at..at + COMMITMENT]);
    Ok(Header { recipients: n, commitment, slots: &bytes[at + COMMITMENT..header_len(n)] })
}

/// The commitment in the header at the start of `bytes`, if it is one.
pub fn header_commitment(bytes: &[u8]) -> Option<[u8; COMMITMENT]> {
    parse_header(bytes).ok().map(|h| h.commitment)
}

/// BLAKE3 (hex) of a header's bytes: what an acceptance commits to, per part.
pub fn header_hash(header: &[u8]) -> String {
    blake3::hash(header).to_hex().to_string()
}

/// The key `slot` of `h` wraps, unwrapped with `me` for the part `context`, BEFORE its commitment is
/// compared: None when the slot is not `me`'s (or was moved, or the count changed).
fn unwrap_slot(me: &KyberKeypair, context: &str, h: &Header, slot: usize) -> Option<[u8; 32]> {
    let bytes = h.slots.get(slot * SLOT..(slot + 1) * SLOT)?;
    let ss = me.decapsulate(&bytes[..KYBER_CIPHERTEXT_LEN]).ok()?;
    let wrap = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&blake3::derive_key(WRAP_DOMAIN, &ss)));
    let key = wrap
        .decrypt(Nonce::from_slice(&[0u8; 12]), Payload { msg: &bytes[KYBER_CIPHERTEXT_LEN..], aad: wrap_aad(context, h.recipients, slot, &h.commitment).as_bytes() })
        .ok()?;
    key.try_into().ok()
}

/// The words for a slot that unwrapped a key its header does not commit to.
const UNCOMMITTED: &str = "the key in this part's header is not the key its body is sealed under";

/// Unwrap the part key of the part `context` from whichever slot of `header` is `me`'s, and check
/// it against the header's commitment. An error for anything not sealed to `me`, and for a slot
/// wrapping any other key than the one the body is sealed under.
pub fn open_slot(me: &KyberKeypair, context: &str, header: &[u8]) -> Result<[u8; 32], String> {
    let h = parse_header(header)?;
    // ML-KEM's decapsulation answers every ciphertext (implicit rejection), so a slot that is not
    // `me`'s shows itself only when its wrapped key does not open.
    let key = (0..h.recipients).find_map(|slot| unwrap_slot(me, context, &h, slot)).ok_or_else(|| "this part is not sealed to this key".to_string())?;
    if key_commitment(&key) != h.commitment {
        return Err(UNCOMMITTED.into());
    }
    Ok(key)
}

/// Build the header of a part `context` whose part key is `part_key` (with commitment
/// `commitment`), wrapped for the Kyber768 public keys `recipients` (1 to [`MAX_RECIPIENTS`]) in
/// slot order.
fn wrap_for(part_key: &[u8; 32], commitment: &[u8; COMMITMENT], recipients: &[&[u8]], context: &str) -> Result<Vec<u8>, String> {
    if recipients.is_empty() || recipients.len() > MAX_RECIPIENTS {
        return Err(format!("a part is sealed to 1 to {MAX_RECIPIENTS} people, not {}", recipients.len()));
    }
    let mut out = Vec::with_capacity(header_len(recipients.len()));
    out.extend_from_slice(MAGIC);
    out.push(recipients.len() as u8);
    out.extend_from_slice(commitment);
    for (slot, recipient) in recipients.iter().enumerate() {
        if recipient.len() != KYBER_EK_LEN {
            return Err(format!("a Kyber768 public key is {KYBER_EK_LEN} bytes, not {}", recipient.len()));
        }
        let (kem_ct, ss) = encapsulate_to(recipient).map_err(|e| format!("encapsulate: {e}"))?;
        let wrap = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&blake3::derive_key(WRAP_DOMAIN, &ss)));
        let wrapped = wrap
            .encrypt(Nonce::from_slice(&[0u8; 12]), Payload { msg: part_key, aad: wrap_aad(context, recipients.len(), slot, commitment).as_bytes() })
            .map_err(|e| format!("wrap: {e}"))?;
        out.extend_from_slice(&kem_ct);
        out.extend_from_slice(&wrapped);
    }
    Ok(out)
}

/// The owner's device, in a change of recipients: open `me`'s slot of `header` (part `context`)
/// and wrap the same part key, under the same commitment, for `recipients` in slot order. The new
/// header; the body it heads is unchanged, byte for byte.
pub fn rewrap_header(me: &KyberKeypair, context: &str, header: &[u8], recipients: &[&[u8]]) -> Result<Vec<u8>, String> {
    let h = parse_header(header)?;
    let key = open_slot(me, context, header)?;
    wrap_for(&key, &h.commitment, recipients, context)
}

/// A recipient's device, in a change of recipients: check the new `header` of part `context`
/// (exactly a header, nothing after it), sealed to `count` people with `me` in slot `slot`: it
/// names that many recipients, `me`'s slot opens, and the key it wraps is the one the header
/// commits to (the relay checked that commitment against the body's, which never changes). No body
/// is read. The header's hash ([`header_hash`]), or why not, in words.
pub fn check_header(me: &KyberKeypair, context: &str, header: &[u8], count: usize, slot: usize) -> Result<String, String> {
    let h = parse_header(header)?;
    if header.len() != header_len(h.recipients) {
        return Err("this is not a part's header alone".into());
    }
    if h.recipients != count {
        return Err(format!("it is sealed to {} people, but the change names {count}", h.recipients));
    }
    let key = unwrap_slot(me, context, &h, slot).ok_or_else(|| "your place in it does not open for your key".to_string())?;
    if key_commitment(&key) != h.commitment {
        return Err(UNCOMMITTED.into());
    }
    Ok(header_hash(header))
}

/// Seal everything `src` holds, as part `context`, to the holders of the Kyber768 public keys
/// `recipients`, writing the sealed part to `dst`. How many bytes were written.
pub fn seal_stream(recipients: &[&[u8]], context: &str, src: impl Read, mut dst: impl Write) -> Result<u64, String> {
    let mut sealer = Sealer::new(recipients, context, src)?;
    let written = std::io::copy(&mut sealer, &mut dst).map_err(|e| format!("seal: {e}"))?;
    dst.flush().map_err(|e| format!("write: {e}"))?;
    Ok(written)
}

/// The sealed size of a part holding `plain` bytes sealed to `recipients` people: the header, the
/// plaintext, and a tag per piece (an empty part is one empty piece).
pub fn sealed_len(plain: u64, recipients: usize) -> u64 {
    let pieces = plain.div_ceil(PIECE as u64).max(1);
    header_len(recipients) as u64 + plain + TAG as u64 * pieces
}

/// The sealing of one part as a reader: each `read` hands out the sealed bytes of [`seal_stream`],
/// a piece at a time, so a file is sealed without ever being held whole.
pub struct Sealer<R: Read> {
    src: R,
    cipher: Aes256Gcm,
    context: String,
    /// The part key's commitment, bound into every piece.
    commitment: [u8; COMMITMENT],
    /// The bytes ready to hand out (the header, then each sealed piece in turn), and how many of
    /// them were handed out.
    out: Vec<u8>,
    pos: usize,
    cur: Vec<u8>,
    cur_len: usize,
    next: Vec<u8>,
    index: u64,
    /// The last piece is sealed into `out`.
    done: bool,
}

impl<R: Read> Sealer<R> {
    /// Start sealing `src` as part `context` to the Kyber768 public keys `recipients` (1 to
    /// [`MAX_RECIPIENTS`]): a fresh part key, committed to and wrapped for each of them.
    pub fn new(recipients: &[&[u8]], context: &str, mut src: R) -> Result<Self, String> {
        let mut part_key = [0u8; 32];
        getrandom::getrandom(&mut part_key).map_err(|e| format!("random: {e}"))?;
        let commitment = key_commitment(&part_key);
        let out = wrap_for(&part_key, &commitment, recipients, context)?;
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&blake3::derive_key(DOMAIN, &part_key)));
        let mut cur = vec![0u8; PIECE];
        let cur_len = fill(&mut src, &mut cur).map_err(|e| format!("read: {e}"))?;
        Ok(Self { src, cipher, context: context.to_string(), commitment, out, pos: 0, cur, cur_len, next: vec![0u8; PIECE], index: 0, done: false })
    }

    /// Seal the next piece into `out`.
    fn seal_next(&mut self) -> std::io::Result<()> {
        // A full piece may be followed by more: look ahead to know whether this one is the last.
        let next_len = if self.cur_len == PIECE { fill(&mut self.src, &mut self.next)? } else { 0 };
        let last = next_len == 0;
        let aad = piece_aad(&self.context, &self.commitment, self.index, last);
        self.out = self
            .cipher
            .encrypt(Nonce::from_slice(&nonce_of(self.index)), Payload { msg: &self.cur[..self.cur_len], aad: aad.as_bytes() })
            .map_err(|e| std::io::Error::other(format!("seal: {e}")))?;
        self.pos = 0;
        if last {
            self.done = true;
        } else {
            std::mem::swap(&mut self.cur, &mut self.next);
            self.cur_len = next_len;
            self.index += 1;
        }
        Ok(())
    }
}

impl<R: Read> Read for Sealer<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.pos == self.out.len() {
            if self.done {
                return Ok(0);
            }
            self.seal_next()?;
        }
        let n = buf.len().min(self.out.len() - self.pos);
        buf[..n].copy_from_slice(&self.out[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

/// Seal `plain` (held in memory: the post's words and the list of files) as part `context`.
pub fn seal_bytes(recipients: &[&[u8]], context: &str, plain: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(sealed_len(plain.len() as u64, recipients.len()) as usize);
    seal_stream(recipients, context, plain, &mut out)?;
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
    /// The commitment every piece must have been sealed with.
    commitment: [u8; COMMITMENT],
    cur: Vec<u8>,
    cur_len: usize,
    next: Vec<u8>,
    index: u64,
    out: Vec<u8>,
    out_pos: usize,
    done: bool,
}

impl<R: Read> Opener<R> {
    /// Read the part's header from `src` and unwrap its part key with `me`, from whichever slot is
    /// `me`'s ([`open_slot`]): an error for anything not sealed to `me`.
    pub fn new(me: &KyberKeypair, context: &str, mut src: R) -> Result<Self, String> {
        let not_sealed = || "this is not a sealed held part".to_string();
        let mut header = vec![0u8; MAGIC.len() + 1];
        if fill(&mut src, &mut header).map_err(|e| format!("read: {e}"))? != header.len() || &header[..MAGIC.len()] != MAGIC {
            return Err(not_sealed());
        }
        let n = header[MAGIC.len()] as usize;
        if n == 0 || n > MAX_RECIPIENTS {
            return Err(not_sealed());
        }
        header.resize(header_len(n), 0);
        let rest = &mut header[MAGIC.len() + 1..];
        if fill(&mut src, rest).map_err(|e| format!("read: {e}"))? != rest.len() {
            return Err(not_sealed());
        }
        let part_key = open_slot(me, context, &header)?;
        let commitment = key_commitment(&part_key);
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&blake3::derive_key(DOMAIN, &part_key)));
        let mut cur = vec![0u8; PIECE + TAG];
        let cur_len = fill(&mut src, &mut cur).map_err(|e| format!("read: {e}"))?;
        Ok(Self { src, cipher, context: context.to_string(), commitment, cur, cur_len, next: vec![0u8; PIECE + TAG], index: 0, out: Vec::new(), out_pos: 0, done: false })
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
        let aad = piece_aad(&self.context, &self.commitment, self.index, last);
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
    chosen: &Chosen,
    mut fetch: impl FnMut(usize) -> Result<Box<dyn Read>, String>,
    mut upload: impl FnMut(usize, &str, &mut dyn Read) -> Result<(), String>,
    finish: impl FnOnce(&str) -> Result<String, String>,
) -> Result<String, String> {
    let package = chosen.package;
    let manifest = open_manifest(kyber, package, fetch(0)?)?;
    manifest_is(&manifest, chosen)?;
    for f in &manifest.files {
        for part in std::iter::once(f.part).chain(f.poster_part) {
            let mut opener = Opener::new(kyber, &part_context(package, part), fetch(part)?)?;
            upload(part, &manifest.restore_key, &mut opener)?;
        }
    }
    finish(&manifest.restore_key)
}

/// Whether the opened `manifest` is the held item the owner CHOSE, its package `package` and its
/// report `report_id` as the list showed them (the owner branch's third review, W7, 2026-11-21:
/// Export and Restore used the relay's package for the item clicked and never compared what opened
/// with it, so a relay could hand over another item's copy under the one chosen). Why not, in words;
/// the website's twin is web/shared/held-seal.js `manifestIs`.
///
/// The post's AUTHOR and TIME are compared too (the fourth review's finding 8, W7 partial): one
/// report can hold several posts, each its own held item with the same report number, so a relay
/// could hand over a sibling post's copy under the one chosen. The list shows each item's post time
/// and author, and what opens must be that post.
pub fn manifest_is(manifest: &Manifest, chosen: &Chosen) -> Result<(), String> {
    let (package, report_id) = (chosen.package, chosen.report_id);
    if manifest.package == package && manifest.report_id == report_id && manifest.author == chosen.author && manifest.stamp == chosen.stamp {
        return Ok(());
    }
    let why = if manifest.package != package {
        " (and it names another package)"
    } else if manifest.report_id == report_id {
        " (another post of that report)"
    } else {
        ""
    };
    Err(format!(
        "the sealed copy the server sent is not the item you chose: it is for report #{}, and you chose the item of report #{report_id}{why}. Nothing was used",
        manifest.report_id
    ))
}

/// The held item the owner chose in the list, as the list showed it: its package, its report, and
/// the post's author and time ([`manifest_is`]).
#[derive(Debug, Clone, Copy)]
pub struct Chosen<'a> {
    pub package: &'a str,
    pub report_id: i64,
    pub author: &'a str,
    pub stamp: u64,
}

// ── A change of recipients (a handover, or the successor named or removed) ──
//
// The owner's device re-wraps every part's header ([`rewrap_package`]); the relay checks each new
// header's commitment against the body it keeps; each recipient's device checks its own slot of
// every new header ([`check_rewrapped_package`]) and signs an acceptance of [`acceptance_digest`],
// which the relay works out from the headers it staged (src/relay/storage/owner_handover.rs). The
// website's twins are web/shared/held-seal.js `rewrapHeader`, `checkHeader` and `acceptanceDigest`;
// scripts/tests/fixtures/held-owner-v2.json holds the two sides to each other.

/// Re-wrap every part of held item `package` (`parts` parts) for `recipients` from the owner's
/// device `me`: `fetch(part)` reads a part's current header, `upload(part, header)` sends the new
/// one. No body is fetched, opened or sent.
pub fn rewrap_package(
    me: &KyberKeypair,
    recipients: &[&[u8]],
    package: &str,
    parts: usize,
    mut fetch: impl FnMut(usize) -> Result<Vec<u8>, String>,
    mut upload: impl FnMut(usize, &[u8]) -> Result<(), String>,
) -> Result<(), String> {
    for part in 0..parts {
        let header = fetch(part)?;
        let new = rewrap_header(me, &part_context(package, part), &header, recipients).map_err(|why| format!("part {part}: {why}"))?;
        upload(part, &new)?;
    }
    Ok(())
}

/// On a recipient's device: check every new header of held item `package` (`parts` parts, each read
/// by `fetch(part)`), sealed to `count` people with this device's key `me` in slot `slot`
/// ([`check_header`]). Each header's hash in part order, or why not, in words naming the part.
pub fn check_rewrapped_package(me: &KyberKeypair, package: &str, parts: usize, count: usize, slot: usize, mut fetch: impl FnMut(usize) -> Result<Vec<u8>, String>) -> Result<Vec<String>, String> {
    let mut hashes = Vec::with_capacity(parts);
    for part in 0..parts {
        let header = fetch(part)?;
        hashes.push(check_header(me, &part_context(package, part), &header, count, slot).map_err(|why| format!("part {part}: {why}"))?);
    }
    Ok(hashes)
}

/// The context label opening the text an acceptance's digest is taken over.
pub const ACCEPT_DOMAIN: &str = "hum/held-accept/v4";

/// What a recipient's acceptance of a change commits to: BLAKE3 (hex) of a text made of
/// [`ACCEPT_DOMAIN`], the change's ROUND (a fresh id each time the change begins, so an acceptance
/// of one round never carries out another), then, for each package in order of its id, a line with
/// the package id and a line `<part> <header hash>` per part.
pub fn acceptance_digest(round: &str, items: &[(String, Vec<String>)]) -> String {
    let mut sorted: Vec<&(String, Vec<String>)> = items.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    let mut text = format!("{ACCEPT_DOMAIN}\n{round}\n");
    for (package, hashes) in sorted {
        text.push_str(package);
        text.push('\n');
        for (i, h) in hashes.iter().enumerate() {
            text.push_str(&format!("{i} {h}\n"));
        }
    }
    blake3::hash(text.as_bytes()).to_hex().to_string()
}

// ── Who asks for a part (2026-11-20, the owner review's finding 1) ──
//
// A grant (a signed action's object id) sits in the held items log and a part is sealed to the
// owner and the successor, so a grant alone never hands out a part: every request to a held part
// route carries its caller's key, the time, and a Dilithium3 signature over `purpose\ntime` (the
// relay's signed-request pattern), and the relay serves the part only to the key the grant belongs
// to. The purpose starts with its own domain line, [`CALLER_DOMAIN`] (the second review, R6): a
// chat message is signed over `content\ntime` too and is broadcast, so a signed chat line reading
// `held_part:...` must never pass for a caller.
// NOT YET CLOSED, said plainly (the owner branch's third review, S6, 2026-11-21): the domain line alone
// does not keep a signed chat line out today, because a chat message's content may hold a
// newline, so a chat line whose content is this very purpose is signed over the same
// `purpose\ntime` and broadcast to everyone, who could replay it within the five minutes. What
// closes it is the relay refusing every chat line that begins with a `hum/` domain line, which
// arrives with the signed-requests branch (src/relay/signed_request.rs, round 2); once both
// branches are merged, the held caller moves onto that request scheme. NOTE to whoever merges the
// second of the two: make that move then (it is not done here).
// The relay (src/relay/handlers/held.rs) and the
// desktop app (src/net/held.rs) share these; the website's twin is web/chat/chat-held.js
// `callerHeaders`.

/// The header carrying the caller's Dilithium3 public key (hex).
pub const CALLER_KEY: &str = "x-hos-held-key";
/// The header carrying the time the caller signed (Unix ms).
pub const CALLER_TIME: &str = "x-hos-held-time";
/// The header carrying the caller's signature (hex) over `purpose\ntime`.
pub const CALLER_SIG: &str = "x-hos-held-sig";
/// The first line of every held caller's purpose.
pub const CALLER_DOMAIN: &str = "hum/held-caller/v1";

/// What a held part route's caller signs: the domain line, then the route (`part` GET, `restore`
/// PUT, `finish` POST with part and offset 0, `header` GET and `rewrap` PUT for the owner in a
/// change, `rewrapped` GET for a recipient checking it), the grant, the item, the part and the
/// offset, so one signature is good for exactly one request.
pub fn caller_purpose(route: &str, grant: &str, id: i64, part: i64, offset: u64) -> String {
    format!("{CALLER_DOMAIN}\nheld_{route}:{grant}:{id}:{part}:{offset}")
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

    /// The header of a part sealed to one person.
    const HEADER: usize = header_len(1);
    /// Where the slots start in a header.
    const SLOTS_AT: usize = MAGIC.len() + 1 + COMMITMENT;

    fn keys(seed: u8) -> KyberKeypair {
        KyberKeypair::from_seed(&derive_kyber_seed(&[seed; 32])).unwrap()
    }

    fn manifest_of(package: &str, files: Vec<HeldFile>, restore_key: &str) -> Manifest {
        Manifest {
            v: MANIFEST_VERSION,
            package: package.into(),
            report_id: 3,
            author: "ab".into(),
            stamp: 9,
            channel: "general".into(),
            words: "the words".into(),
            files,
            held_by: "cd".into(),
            held_at: 1,
            restore_key: restore_key.into(),
        }
    }

    fn file_at(part: usize, bytes: &[u8], poster: Option<(usize, &[u8])>) -> HeldFile {
        HeldFile {
            part,
            name: format!("{part}.bin"),
            folder: "public".into(),
            uploader: String::new(),
            shelf: 0,
            original_name: String::new(),
            bytes: bytes.len() as u64,
            hash: blake3::hash(bytes).to_hex().to_string(),
            poster_part: poster.map(|(p, _)| p),
            poster_bytes: poster.map_or(0, |(_, b)| b.len() as u64),
            poster_hash: poster.map_or(String::new(), |(_, b)| blake3::hash(b).to_hex().to_string()),
        }
    }

    /// A CHANGE RE-WRAPS EVERY HEADER AND KEEPS EVERY BODY BYTE (format v4, 2026-11-20, the owner
    /// branch's second review, R1): every part of a package, its header re-wrapped from the old
    /// owner's slot for the new owner ([`rewrap_package`]), opens for the new owner as the same part
    /// with the same bytes, and no longer for the old, when the new header heads the OLD body
    /// unchanged; the new header carries the old commitment; no body is ever fetched; a key the
    /// package is not sealed to cannot re-wrap it.
    ///
    /// Seen red 2026-11-20 with `rewrap_header` wrapping a fresh key under the old commitment
    /// (`wrap_for(&[9; 32], &h.commitment, ..)`): "part 0 opens for the new owner" failed (the
    /// new owner's slot unwrapped a key the header does not commit to).
    #[test]
    fn a_change_rewraps_every_header_and_keeps_every_body_byte() {
        let (old, new) = (keys(21), keys(22));
        let package = "pkgho";
        let file: Vec<u8> = (0..PIECE + 7).map(|i| (i % 249) as u8).collect();
        let poster = b"poster bytes".to_vec();
        let json = serde_json::to_vec(&manifest_of(package, vec![file_at(1, &file, Some((2, &poster)))], &"11".repeat(32))).unwrap();
        let plains = [json, file, poster];
        let held: Vec<Vec<u8>> = plains.iter().enumerate().map(|(p, b)| seal_bytes(&[&old.public_key()], &part_context(package, p), b).unwrap()).collect();
        for (p, b) in plains.iter().enumerate() {
            assert_eq!(held[p].len() as u64, sealed_len(b.len() as u64, 1), "part {p}'s size is sealed_len");
        }
        let mut sent: Vec<Vec<u8>> = vec![Vec::new(); 3];
        let mut fetched = 0usize;
        rewrap_package(
            &old,
            &[&new.public_key()],
            package,
            3,
            |p| {
                fetched += HEADER;
                Ok(held[p][..HEADER].to_vec())
            },
            |p, h| {
                sent[p] = h.to_vec();
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(fetched, 3 * HEADER, "only the headers were fetched");
        for (p, b) in plains.iter().enumerate() {
            assert_eq!(sent[p].len(), HEADER, "part {p}'s new header is a header alone");
            assert_eq!(header_commitment(&sent[p]), header_commitment(&held[p]), "part {p}'s commitment is kept");
            let rebuilt = [&sent[p][..], &held[p][HEADER..]].concat();
            assert_eq!(&open_bytes(&new, &part_context(package, p), &rebuilt).expect("part opens for the new owner"), b, "part {p} opens for the new owner, its body unchanged");
            assert!(open_bytes(&old, &part_context(package, p), &rebuilt).is_err(), "part {p} no longer opens for the old owner");
        }
        let refused = rewrap_package(&new, &[&old.public_key()], package, 3, |p| Ok(held[p][..HEADER].to_vec()), |_, _| Ok(()));
        assert!(refused.is_err(), "a key the package is not sealed to cannot re-wrap it");
        assert_eq!(sealed_len(0, 1), (HEADER + TAG) as u64);
        assert_eq!(sealed_len(PIECE as u64, 1), (HEADER + PIECE + TAG) as u64);
        assert_eq!(sealed_len(PIECE as u64 + 1, 1), (HEADER + PIECE + 1 + 2 * TAG) as u64);
        assert_eq!(sealed_len(5, 2), (header_len(2) + 5 + TAG) as u64);
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
            let sealed = seal_bytes(&[&owner.public_key()], &ctx, &plain).unwrap();
            assert_eq!(open_bytes(&owner, &ctx, &sealed).unwrap(), plain, "{len} bytes round-trip");
            assert!(open_bytes(&other, &ctx, &sealed).is_err(), "{len}: another key does not open it");
            assert!(open_bytes(&owner, &part_context("pkg", 2), &sealed).is_err(), "{len}: not as another part");
            assert!(open_bytes(&owner, &part_context("other", 1), &sealed).is_err(), "{len}: not in another package");
        }
        let marker = b"PLAINTEXT-MARKER-7f3a ".repeat(100_000); // about 2.1 MB: three pieces
        let sealed = seal_bytes(&[&owner.public_key()], &ctx, &marker).unwrap();
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
        let sealed = seal_bytes(&[&owner.public_key()], &ctx, &plain).unwrap();
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
        assert!(Opener::new(&keys(8), &ctx, &sealed[..]).is_err(), "another key opens nothing");
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

    /// A part sealed to the owner and a named successor (2026-11-20) opens for either, as the same
    /// bytes, and for nobody else; a slot's wrapped key cannot be moved to another part; a part
    /// sealed to too many or to no one is refused.
    ///
    /// Seen red 2026-11-20 with the opening reading only the first slot: "it opens for the
    /// successor" failed ("this part is not sealed to this key").
    #[test]
    fn a_part_sealed_to_two_opens_for_either_and_no_other() {
        let (owner, successor, other) = (keys(31), keys(32), keys(33));
        let ctx = part_context("pkg2", 1);
        let plain: Vec<u8> = (0..PIECE + 9).map(|i| (i % 241) as u8).collect();
        let sealed = seal_bytes(&[&owner.public_key(), &successor.public_key()], &ctx, &plain).unwrap();
        assert_eq!(sealed.len() as u64, sealed_len(plain.len() as u64, 2));
        assert_eq!(open_bytes(&owner, &ctx, &sealed).unwrap(), plain, "it opens for the owner");
        assert_eq!(open_bytes(&successor, &ctx, &sealed).unwrap(), plain, "it opens for the successor");
        assert!(open_bytes(&other, &ctx, &sealed).is_err(), "and for nobody else");
        assert!(open_bytes(&successor, &part_context("pkg2", 2), &sealed).is_err(), "not as another part");
        let pk = owner.public_key();
        assert!(seal_bytes(&[], &ctx, b"x").is_err() && seal_bytes(&[&pk[..]; MAX_RECIPIENTS + 1], &ctx, b"x").is_err());
    }

    /// A SLOT STRIPPED, ADDED OR MOVED MAKES EVERY OTHER SLOT REFUSE (format v4 keeps the first
    /// review's finding 10 closed: each wrap binds the number of recipients and its slot): a part
    /// sealed to the owner and the successor, with the successor's slot stripped and the count set
    /// to 1, or the slots swapped, or a third added, opens for nobody. A slot REPLACED by another
    /// part's leaves the owner's slot opening (the body does not depend on the slots), and is
    /// caught by the person whose slot it was ([`check_header`]): each recipient checks their own.
    ///
    /// Seen red 2026-11-20 with the count left out of `wrap_aad`: "the successor's slot stripped"
    /// opened for the owner.
    #[test]
    fn a_slot_stripped_added_or_moved_makes_every_other_slot_refuse() {
        let (owner, successor, other) = (keys(41), keys(42), keys(43));
        let ctx = part_context("pkg3", 1);
        let plain = b"held words".to_vec();
        let sealed = seal_bytes(&[&owner.public_key(), &successor.public_key()], &ctx, &plain).unwrap();
        let body = sealed[header_len(2)..].to_vec();
        let commitment = sealed[MAGIC.len() + 1..SLOTS_AT].to_vec();
        let slot = |i: usize| sealed[SLOTS_AT + i * SLOT..SLOTS_AT + (i + 1) * SLOT].to_vec();
        let header = |count: u8, slots: &[Vec<u8>]| {
            let mut v = MAGIC.to_vec();
            v.push(count);
            v.extend_from_slice(&commitment);
            slots.iter().for_each(|s| v.extend_from_slice(s));
            v
        };
        let build = |count: u8, slots: &[Vec<u8>]| [header(count, slots), body.clone()].concat();
        assert_eq!(open_bytes(&owner, &ctx, &build(2, &[slot(0), slot(1)])).unwrap(), plain, "rebuilt unchanged, it opens");
        assert!(open_bytes(&owner, &ctx, &build(1, &[slot(0)])).is_err(), "the successor's slot stripped");
        let swapped = build(2, &[slot(1), slot(0)]);
        assert!(open_bytes(&owner, &ctx, &swapped).is_err() && open_bytes(&successor, &ctx, &swapped).is_err(), "the slots swapped");
        let third = seal_bytes(&[&other.public_key()], &ctx, b"x").unwrap();
        let added = build(3, &[slot(0), slot(1), third[SLOTS_AT..header_len(1)].to_vec()]);
        assert!(open_bytes(&owner, &ctx, &added).is_err(), "a third slot added");
        let foreign = seal_bytes(&[&owner.public_key(), &successor.public_key()], &ctx, b"another part").unwrap();
        let replaced = header(2, &[slot(0), foreign[SLOTS_AT + SLOT..header_len(2)].to_vec()]);
        assert!(check_header(&owner, &ctx, &replaced, 2, 0).is_ok(), "the owner's own slot still checks");
        assert!(check_header(&successor, &ctx, &replaced, 2, 1).is_err(), "the successor's check catches its replaced slot");
        assert!(check_header(&successor, &ctx, &header(2, &[slot(0), slot(1)]), 2, 1).is_ok(), "and passes its real one");
    }

    /// THE KEY COMMITMENT (format v4): a slot that wraps any other key than the one the body is
    /// sealed under is refused, even though its wrapping opens (built with the header's own
    /// commitment in its associated data); a header made under a fresh key and its own commitment
    /// checks out on its own but carries another commitment than the body's, which the relay
    /// refuses (src/relay/storage/owner_handover.rs), and does not open the body.
    ///
    /// Seen red 2026-11-20 with the commitment comparison taken out of `check_header`: "a slot
    /// wrapping another key under the real commitment is caught" passed the garbage.
    #[test]
    fn a_slot_wrapping_another_key_is_refused() {
        let (owner, new) = (keys(44), keys(45));
        let ctx = part_context("pkgc", 0);
        let sealed = seal_bytes(&[&owner.public_key()], &ctx, b"the words").unwrap();
        let mut real = [0u8; COMMITMENT];
        real.copy_from_slice(&sealed[MAGIC.len() + 1..SLOTS_AT]);
        let other_key = [9u8; 32];
        let forged = wrap_for(&other_key, &real, &[&new.public_key()], &ctx).unwrap();
        assert_eq!(header_commitment(&forged), Some(real), "it claims the body's commitment");
        assert!(open_slot(&new, &ctx, &forged).is_err(), "the opening refuses it");
        assert!(check_header(&new, &ctx, &forged, 1, 0).is_err(), "a slot wrapping another key under the real commitment is caught");
        let fresh = wrap_for(&other_key, &key_commitment(&other_key), &[&new.public_key()], &ctx).unwrap();
        assert!(check_header(&new, &ctx, &fresh, 1, 0).is_ok(), "a header true to itself checks out");
        assert_ne!(header_commitment(&fresh), Some(real), "but its commitment is not the body's");
        assert!(open_bytes(&new, &ctx, &[&fresh[..], &sealed[HEADER..]].concat()).is_err(), "and it does not open the body");
    }

    /// A RECIPIENT CHECKS ONLY THEIR OWN SLOT OF EVERY NEW HEADER (format v4): an honest re-wrap to
    /// the owner and a successor checks out for each in their own slot, and both arrive at the
    /// digest the relay works out from the headers alone; the wrong count, the wrong slot, a garbage
    /// slot and a body sent where a header belongs are caught; another round gives another digest.
    ///
    /// Seen red 2026-11-20 with `check_header` opening whichever slot is the caller's (not the one
    /// it was told): "the owner's key in the successor's place" checked out.
    #[test]
    fn a_recipient_checks_only_their_own_slot_of_every_new_header() {
        let (owner, successor, third) = (keys(51), keys(52), keys(53));
        let package = "pkgchk";
        let held: Vec<Vec<u8>> = (0..2).map(|p| seal_bytes(&[&owner.public_key()], &part_context(package, p), &vec![p as u8; 10 + p]).unwrap()).collect();
        let to = [owner.public_key(), successor.public_key()];
        let to: Vec<&[u8]> = to.iter().map(Vec::as_slice).collect();
        let mut new = vec![Vec::new(); 2];
        rewrap_package(&owner, &to, package, 2, |p| Ok(held[p][..HEADER].to_vec()), |p, h| {
            new[p] = h.to_vec();
            Ok(())
        })
        .unwrap();
        let check = |me: &KyberKeypair, headers: &[Vec<u8>], count: usize, slot: usize| {
            let headers = headers.to_vec();
            check_rewrapped_package(me, package, 2, count, slot, move |p| Ok(headers[p].clone()))
        };
        let mine = check(&owner, &new, 2, 0).expect("the owner's slot checks out");
        let theirs = check(&successor, &new, 2, 1).expect("the successor's slot checks out");
        assert_eq!(mine, theirs, "both see the same headers");
        let relay: Vec<String> = new.iter().map(|h| header_hash(h)).collect();
        assert_eq!(acceptance_digest("r1", &[(package.into(), mine.clone())]), acceptance_digest("r1", &[(package.into(), relay)]), "the relay's digest from the headers agrees");
        assert_ne!(acceptance_digest("r1", &[(package.into(), mine.clone())]), acceptance_digest("r2", &[(package.into(), mine)]), "another round, another digest");
        assert!(check(&successor, &new, 1, 1).is_err(), "the wrong count is caught");
        assert!(check(&owner, &new, 2, 1).is_err(), "the owner's key in the successor's place is caught");
        assert!(check(&third, &new, 2, 1).is_err(), "a third key is caught");
        let mut garbage = new.clone();
        garbage[1][SLOTS_AT + SLOT + 40] ^= 1;
        assert!(check(&successor, &garbage, 2, 1).is_err(), "a garbage slot is caught");
        let whole = vec![new[0].clone(), [&new[1][..], &held[1][HEADER..]].concat()];
        assert!(check(&successor, &whole, 2, 1).is_err(), "a body where a header belongs is caught");
    }

    fn fixture_path() -> std::path::PathBuf {
        std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("scripts/tests/fixtures/held-v4.json")
    }

    /// THE WEBSITE OPENS WHAT THE RELAY SEALS, AND THE RELAY OPENS WHAT THE WEBSITE SEALS AND
    /// RE-WRAPS (format v4). scripts/tests/fixtures/held-v4.json holds parts this crate sealed to the
    /// Kyber768 keys of two fixed test seeds (an owner and a successor): a manifest (part 0), a short
    /// file, and an empty one; scripts/tests/held-web.test.js opens them in the website's code and
    /// compares the bytes. The other way, `held-v4-web.json` (written by held-web.test.js) holds
    /// parts the website sealed and headers the website RE-WRAPPED from this fixture's parts for the
    /// successor alone; each opens here, a re-wrapped header heading this fixture's body unchanged.
    /// `HELD_FIXTURE_WRITE=1` writes a fresh fixture (the seal is random, so its bytes change; what
    /// they open to does not).
    ///
    /// Seen red 2026-11-20 with the web side's piece associated data ending in "true" for the last
    /// piece (JavaScript's own spelling of the flag) instead of 1: the website could not open the
    /// fixture's parts ("this part does not open with this key"). Seen red again for v4 with the web
    /// side's commitment written in the associated data as raw bytes, not hex: "the website's part 0
    /// opens here" failed.
    #[test]
    fn held_fixture_rust_and_the_website_agree() {
        use base64::{engine::general_purpose::STANDARD as B64, Engine};
        let (seed, second) = ([7u8; 32], [6u8; 32]);
        let me = KyberKeypair::from_seed(&derive_kyber_seed(&seed)).unwrap();
        let successor = KyberKeypair::from_seed(&derive_kyber_seed(&second)).unwrap();
        let stranger = KyberKeypair::from_seed(&derive_kyber_seed(&[5u8; 32])).unwrap();
        let package = "0123456789abcdef0123456789abcdef";
        let manifest = br#"{"v":1,"package":"0123456789abcdef0123456789abcdef","words":"fixture words"}"#.to_vec();
        let file = b"a short file, with \xe2\x9c\x93 a non-ASCII mark".to_vec();
        let wanted: Vec<(usize, Vec<u8>)> = vec![(0, manifest), (1, file), (2, Vec::new())];
        if std::env::var("HELD_FIXTURE_WRITE").is_ok() {
            let parts: Vec<serde_json::Value> = wanted
                .iter()
                .map(|(n, plain)| {
                    let sealed = seal_bytes(&[&me.public_key(), &successor.public_key()], &part_context(package, *n), plain).unwrap();
                    serde_json::json!({ "part": n, "sealed_b64": B64.encode(sealed), "plain_hex": hex::encode(plain) })
                })
                .collect();
            let fx = serde_json::json!({ "about": "Held parts (format v4) sealed by crates/humanity-protocol/src/held_seal.rs (held_fixture_rust_and_the_website_agree); opened and re-wrapped by scripts/tests/held-web.test.js.", "seed_hex": hex::encode(seed), "second_seed_hex": hex::encode(second), "package": package, "parts": parts });
            std::fs::write(fixture_path(), serde_json::to_string_pretty(&fx).unwrap() + "\n").unwrap();
        }
        let fx: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(fixture_path()).expect("the held fixture is checked in")).unwrap();
        assert_eq!(fx["seed_hex"].as_str().unwrap(), hex::encode(seed));
        assert_eq!(fx["package"].as_str().unwrap(), package);
        let parts = fx["parts"].as_array().unwrap();
        assert_eq!(parts.len(), wanted.len());
        let mut bodies = std::collections::HashMap::new();
        for (p, (n, plain)) in parts.iter().zip(&wanted) {
            assert_eq!(p["part"].as_u64().unwrap() as usize, *n);
            assert_eq!(p["plain_hex"].as_str().unwrap(), hex::encode(plain));
            let sealed = B64.decode(p["sealed_b64"].as_str().unwrap()).unwrap();
            assert_eq!(&open_bytes(&me, &part_context(package, *n), &sealed).unwrap(), plain, "part {n} opens here");
            assert_eq!(&open_bytes(&successor, &part_context(package, *n), &sealed).unwrap(), plain, "part {n} opens for the successor");
            assert!(open_bytes(&stranger, &part_context(package, *n), &sealed).is_err(), "part {n} opens for nobody else");
            bodies.insert(*n, (sealed[header_len(2)..].to_vec(), plain.clone()));
        }
        // The website's own sealing opens here, and its re-wrapping of this fixture's parts.
        let web: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(fixture_path().with_file_name("held-v4-web.json")).expect("the website's held fixture is checked in")).unwrap();
        let web_package = web["package"].as_str().unwrap();
        for p in web["parts"].as_array().unwrap() {
            let n = p["part"].as_u64().unwrap() as usize;
            let sealed = B64.decode(p["sealed_b64"].as_str().unwrap()).unwrap();
            let plain = hex::decode(p["plain_hex"].as_str().unwrap()).unwrap();
            assert_eq!(open_bytes(&me, &part_context(web_package, n), &sealed).unwrap(), plain, "the website's part {n} opens here");
            assert_eq!(open_bytes(&successor, &part_context(web_package, n), &sealed).unwrap(), plain, "and for the successor");
        }
        let rewrapped = web["rewrapped"].as_array().expect("the website's re-wrapped headers");
        assert_eq!(rewrapped.len(), wanted.len());
        for r in rewrapped {
            let n = r["part"].as_u64().unwrap() as usize;
            let header = B64.decode(r["header_b64"].as_str().unwrap()).unwrap();
            let (body, plain) = &bodies[&n];
            assert!(check_header(&successor, &part_context(package, n), &header, 1, 0).is_ok(), "the website's re-wrapped header {n} checks out here");
            let rebuilt = [&header[..], &body[..]].concat();
            assert_eq!(&open_bytes(&successor, &part_context(package, n), &rebuilt).unwrap(), plain, "and heads the same body for the successor");
            assert!(open_bytes(&me, &part_context(package, n), &rebuilt).is_err(), "and no longer for the owner");
        }
    }

    /// THE OWNER'S CHANGE MEANS THE SAME ON BOTH SIDES (format v4; the second review's R1 and R6).
    /// scripts/tests/fixtures/held-owner-v2.json holds what a held part route's caller signs
    /// ([`caller_purpose`], a line per route, each starting with [`CALLER_DOMAIN`]) and a
    /// recipient's acceptance of two packages: each part's new header (re-wrapped to the owner and
    /// the successor), its hash ([`header_hash`]), the round, and the digest ([`acceptance_digest`]).
    /// This test checks each header in its slot for both and works the hashes and the digest out
    /// here; scripts/tests/held-web.test.js checks the same headers with its `checkHeader` and works
    /// out the same hashes and digest, and its `callerPurpose` the same lines.
    /// `HELD_OWNER_FIXTURE_WRITE=1` writes it fresh (the headers are random; their hashes and the
    /// digest follow from them).
    ///
    /// Seen red 2026-11-20 with one hex digit of the checked-in digest changed: "the digest is what
    /// this crate works out" failed here, and held-web.test.js failed on the same file.
    #[test]
    fn held_owner_fixture_rust_and_the_website_agree() {
        use base64::{engine::general_purpose::STANDARD as B64, Engine};
        let (owner, successor) = (KyberKeypair::from_seed(&derive_kyber_seed(&[7u8; 32])).unwrap(), KyberKeypair::from_seed(&derive_kyber_seed(&[6u8; 32])).unwrap());
        let grant = "9f".repeat(32);
        let routes: [(&str, i64, i64, u64); 6] = [("part", 7, 0, 0), ("restore", 7, 2, 786_432), ("finish", 7, 0, 0), ("header", 12, 1, 0), ("rewrap", 12, 1, 0), ("rewrapped", 12, 3, 0)];
        let caller: Vec<serde_json::Value> = routes
            .iter()
            .map(|(route, id, part, offset)| serde_json::json!({ "route": route, "grant": grant, "id": id, "part": part, "offset": offset, "purpose": caller_purpose(route, &grant, *id, *part, *offset) }))
            .collect();
        let path = fixture_path().with_file_name("held-owner-v2.json");
        let round = "5e".repeat(16);
        // Listed out of the order of their ids, so the digest's sorting is part of what is held.
        let packages = [("c2".repeat(16), 3usize), ("a1".repeat(16), 1usize)];
        if std::env::var("HELD_OWNER_FIXTURE_WRITE").is_ok() {
            let to = [owner.public_key(), successor.public_key()];
            let to: Vec<&[u8]> = to.iter().map(Vec::as_slice).collect();
            let listed: Vec<serde_json::Value> = packages
                .iter()
                .map(|(package, parts)| {
                    let headers: Vec<String> = (0..*parts)
                        .map(|p| {
                            let held = seal_bytes(&[&owner.public_key()], &part_context(package, p), &vec![p as u8; 33]).unwrap();
                            B64.encode(rewrap_header(&owner, &part_context(package, p), &held[..HEADER], &to).unwrap())
                        })
                        .collect();
                    serde_json::json!({ "package": package, "headers_b64": headers })
                })
                .collect();
            let fx = serde_json::json!({ "about": "What a held part route's caller signs (core held_seal::caller_purpose) and a recipient's acceptance of a change of two packages (check_header, header_hash, acceptance_digest): worked out by crates/humanity-protocol/src/held_seal.rs (held_owner_fixture_rust_and_the_website_agree) and by scripts/tests/held-web.test.js (callerPurpose, checkHeader, acceptanceDigest).", "caller": caller, "round": round, "packages": listed });
            std::fs::write(&path, serde_json::to_string_pretty(&fx).unwrap() + "\n").unwrap();
        }
        let mut have: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("the owner fixture is checked in")).unwrap();
        assert_eq!(have["caller"], serde_json::Value::Array(caller), "every caller line is what this crate signs");
        assert!(have["caller"].as_array().unwrap().iter().all(|c| c["purpose"].as_str().unwrap().starts_with("hum/held-caller/v1\nheld_")), "each starts with the domain line");
        assert_eq!(have["round"].as_str(), Some(round.as_str()));
        let mut items = Vec::new();
        for p in have["packages"].as_array().unwrap() {
            let package = p["package"].as_str().unwrap().to_string();
            let headers: Vec<Vec<u8>> = p["headers_b64"].as_array().unwrap().iter().map(|h| B64.decode(h.as_str().unwrap()).unwrap()).collect();
            let fetch = |headers: Vec<Vec<u8>>| move |part: usize| Ok::<Vec<u8>, String>(headers[part].clone());
            let hashes = check_rewrapped_package(&owner, &package, headers.len(), 2, 0, fetch(headers.clone())).expect("the owner's slots check out");
            assert_eq!(check_rewrapped_package(&successor, &package, headers.len(), 2, 1, fetch(headers.clone())).expect("the successor's slots check out"), hashes);
            items.push((package, hashes));
        }
        let digest = acceptance_digest(&round, &items);
        if std::env::var("HELD_OWNER_FIXTURE_WRITE").is_ok() {
            have["digest"] = serde_json::Value::String(digest.clone());
            have["hashes"] = serde_json::json!(items.iter().map(|(p, h)| serde_json::json!({ "package": p, "hashes": h })).collect::<Vec<_>>());
            std::fs::write(&path, serde_json::to_string_pretty(&have).unwrap() + "\n").unwrap();
        }
        assert_eq!(have["digest"].as_str(), Some(digest.as_str()), "the digest is what this crate works out");
        let listed: Vec<(String, Vec<String>)> = have["hashes"].as_array().unwrap().iter().map(|h| (h["package"].as_str().unwrap().to_string(), h["hashes"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect())).collect();
        assert_eq!(listed, items, "and each header's hash");
    }
}
