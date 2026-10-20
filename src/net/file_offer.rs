//! Device-to-device big files, the words (docs/design/files-showcase.md, "Device-to-device big
//! files, exact" and "Device-to-device big files: the shapes, exact", parts 1, 3 and 6; the
//! operator's answer 10 of 2026-10-11): the file pass, the offer and its answer and cancel notes,
//! the piece hashes, the receiver's question, and the record each device keeps of a transfer.
//!
//! WHY A FILE GOES DEVICE TO DEVICE. A file bigger than the person's private file limit
//! (`max_private_file_mb` for their role) never goes to the server's disk. In a DM it goes from
//! one device to the other, while both are online, once the receiver said yes, over a WebRTC data
//! channel that by default passes through the server's call forwarder, so neither learns the
//! other's network address (net/file_link.rs carries it, net/file_transfer.rs is what is said on
//! it, engine/file_transfer.rs drives it).
//!
//! THE FILE PASS. The relay lets a `dc_offer` through, and hands out forwarder credentials, only
//! with a reason. Two people who are not friends have none, so the sender mints one for this one
//! transfer: a Dilithium3 signature over
//! `hum/file-pass/v1\n{server}\n{issuer}\n{grantee}\n{id}\n{expiry}`, carried as
//! `{"v":1,"id":"<id>","expiry":<n>,"sig":"<base64>"}`. The relay checks it statelessly, rebuilding
//! the words from its own did:hum and the two keys it knows from the frame, the pattern of the
//! friendship passes and plot permits. It is good for nothing but reaching the issuer for this file.
//!
//! THE NOTES. Sealed DMs like the choice and block notes, never shown as messages:
//! `[[hum:offer:v1]]{...}`, `[[hum:offer-answer:v1]]{"id","accept"}`, `[[hum:offer-cancel:v1]]{"id"}`.
//! The offer carries the BLAKE3 of the whole file and of its list of piece hashes, never the list
//! itself (half a megabyte for a 2 GB file, far past what a DM carries): the list travels on the
//! data channel and is checked against that digest there.
//!
//! The JSON is written field by field in the spec's order with `serde_json` (no spaces), which is
//! byte for byte what the web's `JSON.stringify` writes for the same object, so both apps write the
//! same note. The shared fixture is scripts/tests/fixtures/file-offer-v1.json; until it is in this
//! tree the test pins the spec's own text (`the_offer_note_and_pass_words_are_the_spec_s`).
//!
//! No socket and no GUI here, so every rule is unit tested (file_offer_tests.rs).

use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use super::files_policy::{self, AcceptFrom};

/// The offer note's marker.
pub const CTL_OFFER: &str = "[[hum:offer:v1]]";
/// The answer note's marker: `{"id","accept"}`.
pub const CTL_OFFER_ANSWER: &str = "[[hum:offer-answer:v1]]";
/// The cancel note's marker: `{"id"}`. Either side may send it.
pub const CTL_OFFER_CANCEL: &str = "[[hum:offer-cancel:v1]]";

/// The size of every piece but the last, bytes (256 KiB).
pub const PIECE_SIZE: u64 = 262_144;

/// The first line of what a file pass signs.
pub const FILE_PASS_DOMAIN: &str = "hum/file-pass/v1";
/// The longest a file pass may run: 7 days.
pub const FILE_PASS_MAX_SECS: u64 = 7 * 86_400;
/// How much longer than [`FILE_PASS_MAX_SECS`] a check allows, as plot permits do: the issuer
/// mints on its own clock. The minting allows none.
pub const FILE_PASS_CLOCK_SLACK_SECS: u64 = 86_400;

/// The biggest file an offer may describe: 256 GiB, a million pieces, so the list of piece hashes
/// a receiver holds is at most 32 MB. An offer for more is not read. (A limit of this app's own
/// choosing, to keep that list in memory; the spec sets none.)
pub const MAX_OFFER_BYTES: u64 = 1 << 38;

/// The longest file name or type an offer may carry, in characters.
const MAX_TEXT_CHARS: usize = 255;

/// A file pass: the sender's word that the receiver may reach them for this one file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilePass {
    pub v: u32,
    pub id: String,
    pub expiry: u64,
    pub sig: String,
}

/// An offer: what the file is, in the spec's field order (which is the order it is written in).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Offer {
    /// The transfer's id: 16 random bytes, 32 lowercase hex characters.
    pub id: String,
    pub name: String,
    pub size: u64,
    #[serde(rename = "type")]
    pub mime: String,
    /// BLAKE3 of the whole file, 64 lowercase hex.
    pub blake3: String,
    /// The piece size, always [`PIECE_SIZE`].
    pub piece: u64,
    /// `ceil(size / piece)`.
    pub pieces: u64,
    /// BLAKE3 of the piece hashes (each 32 raw bytes, in order), 64 lowercase hex.
    pub hashes: String,
    pub pass: FilePass,
}

/// The receiver's answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfferAnswer {
    pub id: String,
    pub accept: bool,
}

/// Either side calling it off.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfferCancel {
    pub id: String,
}

/// One of the three notes, read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Note {
    Offer(Offer),
    Answer(OfferAnswer),
    Cancel(OfferCancel),
}

/// Does this DM text start with one of the three markers? Such a text is never shown as a message,
/// readable or not.
pub fn is_note(text: &str) -> bool {
    text.starts_with(CTL_OFFER) || text.starts_with(CTL_OFFER_ANSWER) || text.starts_with(CTL_OFFER_CANCEL)
}

/// Is `s` exactly `n` lowercase hex characters?
fn lower_hex(s: &str, n: usize) -> bool {
    s.len() == n && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Is `id` a transfer id (32 lowercase hex characters)?
pub fn is_id(id: &str) -> bool {
    lower_hex(id, 32)
}

/// How many pieces a file of `size` bytes has.
pub fn pieces_for(size: u64) -> u64 {
    size.div_ceil(PIECE_SIZE)
}

/// How long piece `index` of a file of `size` bytes is (the last may be short).
pub fn piece_len(size: u64, index: u64) -> u64 {
    let start = index.saturating_mul(PIECE_SIZE);
    size.saturating_sub(start).min(PIECE_SIZE)
}

impl Offer {
    /// The note's text: the marker, then the offer as JSON in its field order, no spaces.
    pub fn text(&self) -> String {
        format!("{CTL_OFFER}{}", serde_json::to_string(self).unwrap_or_default())
    }

    /// Whether the offer is one this app reads: a well-formed id, a name and a type within bounds, a
    /// size of at least one byte and at most [`MAX_OFFER_BYTES`], the piece size the spec fixes, the
    /// matching piece count, both digests 64 lowercase hex, and a pass for this same transfer.
    pub fn check(&self) -> Result<(), &'static str> {
        if !is_id(&self.id) {
            return Err("its id is not 32 lowercase hex characters");
        }
        if self.name.trim().is_empty() || self.name.chars().count() > MAX_TEXT_CHARS {
            return Err("its file name is empty or too long");
        }
        if self.mime.chars().count() > MAX_TEXT_CHARS {
            return Err("its file type is too long");
        }
        if self.size == 0 || self.size > MAX_OFFER_BYTES {
            return Err("its size is zero or past what this app takes");
        }
        if self.piece != PIECE_SIZE || self.pieces != pieces_for(self.size) {
            return Err("its pieces do not add up");
        }
        if !lower_hex(&self.blake3, 64) || !lower_hex(&self.hashes, 64) {
            return Err("its hashes are not 64 lowercase hex characters");
        }
        if self.pass.v != 1 || self.pass.id != self.id || self.pass.sig.is_empty() {
            return Err("its pass is not for this transfer");
        }
        Ok(())
    }
}

impl OfferAnswer {
    /// The note's text.
    pub fn text(&self) -> String {
        format!("{CTL_OFFER_ANSWER}{}", serde_json::to_string(self).unwrap_or_default())
    }
}

impl OfferCancel {
    /// The note's text.
    pub fn text(&self) -> String {
        format!("{CTL_OFFER_CANCEL}{}", serde_json::to_string(self).unwrap_or_default())
    }
}

/// Read a note, or None when the text is not a usable one (no marker, JSON that does not parse,
/// or an offer that fails [`Offer::check`]). The markers are tried longest first, since
/// `[[hum:offer:v1]]` does not prefix the other two but a careless reader might.
pub fn parse_note(text: &str) -> Option<Note> {
    if let Some(rest) = text.strip_prefix(CTL_OFFER_ANSWER) {
        let a: OfferAnswer = serde_json::from_str(rest).ok()?;
        return is_id(&a.id).then_some(Note::Answer(a));
    }
    if let Some(rest) = text.strip_prefix(CTL_OFFER_CANCEL) {
        let c: OfferCancel = serde_json::from_str(rest).ok()?;
        return is_id(&c.id).then_some(Note::Cancel(c));
    }
    let rest = text.strip_prefix(CTL_OFFER)?;
    let offer: Offer = serde_json::from_str(rest).ok()?;
    offer.check().ok()?;
    Some(Note::Offer(offer))
}

/// A fresh transfer id: 16 random bytes in lowercase hex. None if the system has no randomness to
/// give (nothing is offered then).
pub fn new_id() -> Option<String> {
    let mut b = [0u8; 16];
    getrandom::getrandom(&mut b).ok()?;
    Some(hex::encode(b))
}

// ── The file pass ───────────────────────────────────────────────────────────────────────────

/// What the sender signs: the server it is given on (that server's own did:hum), the sender's key,
/// the receiver's key, the transfer and when it runs out.
pub fn file_pass_preimage(server: &str, issuer: &str, grantee: &str, id: &str, expiry: u64) -> String {
    format!("{FILE_PASS_DOMAIN}\n{server}\n{issuer}\n{grantee}\n{id}\n{expiry}")
}

/// Why a file pass was not minted, or does not check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassError {
    /// A server, key or id that is empty, holds a line break, or an id that is not one.
    Malformed,
    /// Its end has come.
    Expired,
    /// It runs out more than 7 days ahead (plus a day of slack when checked).
    TooLong,
    /// The signature is not the issuer's over these words.
    BadSignature,
}

fn pass_field_ok(s: &str) -> bool {
    !s.is_empty() && !s.contains(['\n', '\r'])
}

fn pass_window(expiry: u64, now: u64, slack: u64) -> Result<(), PassError> {
    if expiry <= now {
        return Err(PassError::Expired);
    }
    if expiry - now > FILE_PASS_MAX_SECS + slack {
        return Err(PassError::TooLong);
    }
    Ok(())
}

/// Mint MY file pass for `grantee` to reach me for transfer `id` on the server `server` (its
/// did:hum), running out at `expiry` (Unix seconds), with the Dilithium key derived from my BIP39
/// `seed`. Refused when a check would refuse it (the minting allows no clock slack).
pub fn mint_file_pass(seed: &[u8], server: &str, issuer: &str, grantee: &str, id: &str, expiry: u64, now: u64) -> Result<FilePass, PassError> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    use crate::relay::core::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};
    if ![server, issuer, grantee].into_iter().all(pass_field_ok) || !is_id(id) {
        return Err(PassError::Malformed);
    }
    pass_window(expiry, now, 0)?;
    let kp = DilithiumKeypair::from_seed(&derive_dilithium_seed(seed));
    let sig = B64.encode(kp.sign(file_pass_preimage(server, issuer, grantee, id, expiry).as_bytes()));
    Ok(FilePass { v: 1, id: id.to_string(), expiry, sig })
}

/// Does `pass` let `grantee` reach `issuer_hex` for its transfer on the server `server`? The words
/// are rebuilt from the facts the checker knows, never the pass's claims beyond its id and end.
/// The receiver checks an offer's pass this way before asking anything, so an offer whose pass the
/// relay would refuse is never asked about.
pub fn verify_file_pass(issuer_hex: &str, server: &str, grantee: &str, pass: &FilePass, now: u64) -> Result<(), PassError> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    if pass.v != 1 || ![server, issuer_hex, grantee].into_iter().all(pass_field_ok) || !is_id(&pass.id) {
        return Err(PassError::Malformed);
    }
    pass_window(pass.expiry, now, FILE_PASS_CLOCK_SLACK_SECS)?;
    let Ok(issuer_pk) = hex::decode(issuer_hex.trim()) else { return Err(PassError::BadSignature) };
    let Ok(sig) = B64.decode(pass.sig.trim()) else { return Err(PassError::BadSignature) };
    crate::relay::core::pq_crypto::verify_dilithium(&issuer_pk, file_pass_preimage(server, issuer_hex, grantee, &pass.id, pass.expiry).as_bytes(), &sig)
        .map_err(|_| PassError::BadSignature)
}

/// Now, in Unix seconds.
pub fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

// ── The piece hashes ────────────────────────────────────────────────────────────────────────

/// BLAKE3 of the piece hashes, each 32 raw bytes, in order: the offer's `hashes`.
pub fn hashes_digest(pieces: &[[u8; 32]]) -> String {
    let mut h = blake3::Hasher::new();
    for p in pieces {
        h.update(p);
    }
    h.finalize().to_hex().to_string()
}

/// A file read for its offer: the BLAKE3 of the whole file and of each piece.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileHashes {
    pub size: u64,
    pub whole: String,
    pub pieces: Vec<[u8; 32]>,
}

/// Read the file at `path` a piece at a time for its hashes. `done` counts the bytes read so far
/// (the row's "Preparing ... (40%)"); `stop` set ends it early with an error. The whole file and
/// its pieces are hashed in one pass.
pub fn hash_file(path: &Path, done: &AtomicU64, stop: &AtomicBool) -> std::io::Result<FileHashes> {
    let mut f = std::fs::File::open(path)?;
    let mut whole = blake3::Hasher::new();
    let mut pieces = Vec::new();
    let mut buf = vec![0u8; PIECE_SIZE as usize];
    let mut size = 0u64;
    loop {
        if stop.load(Ordering::Relaxed) {
            return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "stopped"));
        }
        let n = read_full(&mut f, &mut buf)?;
        if n == 0 {
            break;
        }
        whole.update(&buf[..n]);
        pieces.push(*blake3::hash(&buf[..n]).as_bytes());
        size += n as u64;
        done.store(size, Ordering::Relaxed);
        if n < buf.len() {
            break;
        }
    }
    Ok(FileHashes { size, whole: whole.finalize().to_hex().to_string(), pieces })
}

/// Fill `buf` from `r` as far as the file goes: the number of bytes read (short only at its end).
pub fn read_full(r: &mut impl Read, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match r.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(n)
}

// ── What the person sees ────────────────────────────────────────────────────────────────────

/// A file's size in plain words, up to gigabytes ("clip.mp4 (2.3 GB)").
pub fn size_words(bytes: u64) -> String {
    const K: f64 = 1024.0;
    let b = bytes as f64;
    if bytes < 1024 {
        format!("{bytes} bytes")
    } else if b < K * K {
        format!("{} KB", bytes.div_ceil(1024))
    } else if b < K * K * K {
        format!("{:.1} MB", b / (K * K))
    } else {
        format!("{:.1} GB", b / (K * K * K))
    }
}

/// The sender's line in the attach step, for a file over their private file limit.
pub fn goes_direct_line(limit_bytes: u64, name: &str) -> String {
    format!("Files over {} MB go straight to {name}'s device instead of through the server.", limit_bytes / (1024 * 1024))
}

/// What the receiver's app asks about an offer, after "Files in messages" (part 6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Question {
    /// The protected setup leaves this person's files out: nothing is asked, the row says so.
    LeftOut,
    /// Not someone files are accepted from: the card asks first, in these words, and only its
    /// Accept leads on to the question.
    NotAccepted(String),
    /// The question itself, with Accept and Decline.
    Ask(String),
}

/// The protected setup's line for an offer it leaves out.
pub const LEFT_OUT_LINE: &str = "The protected setup leaves out files from people who are not your friends.";

/// THE ORDER (spec part 3): "Files in messages" first, then the question. Under the protected
/// setup a non-friend's offer is left out (`protected_hides`, engine/protected.rs
/// `hides_pictures_from`); someone this person does not accept files from (`accept_from`, with
/// `friend` the pictures rule's own friend test) gets the one-file card, and only its Accept
/// (`let_in`) leads on to "Ann wants to send you clip.mp4 (2.3 GB). Accept?".
pub fn question(accept_from: AcceptFrom, friend: bool, protected_hides: bool, let_in: bool, sender: &str, name: &str, size: u64) -> Question {
    if protected_hides {
        return Question::LeftOut;
    }
    if !let_in && !files_policy::accepts(accept_from, false, friend) {
        let why = match accept_from {
            AcceptFrom::Friends => "You accept files only from friends.",
            AcceptFrom::Nobody | AcceptFrom::Anyone => "You do not accept files in messages.",
        };
        return Question::NotAccepted(format!("{sender} wants to send you a file. {why} Accept this one?"));
    }
    Question::Ask(format!("{sender} wants to send you {name} ({}). Accept?", size_words(size)))
}

// ── The record each device keeps ────────────────────────────────────────────────────────────

/// Where a transfer stands, as kept in the DM store (per identity and server). The live part
/// (connecting, how many pieces are in) is the engine's and is not kept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stage {
    /// Offered, not answered.
    Offered,
    /// Accepted: it goes whenever both are online.
    Accepted,
    /// The receiver said no.
    Declined,
    /// Called off; `by_me` says which side.
    Cancelled { by_me: bool },
    /// Every piece in and the whole file's BLAKE3 matched: "Sent" / "Saved".
    Done,
    /// Ended with these words (a piece that failed three times, a list that does not match, a file
    /// that could not be read or written, an accepted transfer whose pass ran out).
    Failed(String),
    /// Never answered, and its pass ran out: the sender's row says "Ann did not answer in time".
    /// Nothing is ever sent for it (an automatic decline would tell the sender when the receiver
    /// came online, so neither app sends one).
    Expired,
}

impl Stage {
    /// No transition leaves an ended transfer.
    pub fn ended(&self) -> bool {
        !matches!(self, Stage::Offered | Stage::Accepted)
    }
}

/// What happened to a transfer, in the order notes and the channel report it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Happened {
    Accepted,
    Declined,
    Cancelled { by_me: bool },
    Done,
    Failed(String),
}

/// Where a transfer at `stage` goes when `what` happens, or None when nothing changes: an ended
/// transfer never changes again (a late note from a relay that delivers twice cannot bring back a
/// cancelled one), and an answer counts only while the offer waits for one.
///
/// RULE 9 OF THE SECOND REVIEW (files-showcase.md, "a dial is an answer"): `done` ends a transfer
/// as Done from Offered too. Only the channel reports `done`, and a channel exists only once the
/// receiver opened it, so a sender whose answer note was lost (its record still Offered) is Sent
/// when the receiver says `done`, never left waiting to end as "did not answer in time". (A
/// receiver's own record is Accepted before it opens a channel, so this changes nothing for it.)
pub fn advance(stage: &Stage, what: &Happened) -> Option<Stage> {
    match (stage, what) {
        (s, _) if s.ended() => None,
        (Stage::Offered, Happened::Accepted) => Some(Stage::Accepted),
        (Stage::Offered, Happened::Declined) => Some(Stage::Declined),
        (_, Happened::Cancelled { by_me }) => Some(Stage::Cancelled { by_me: *by_me }),
        (Stage::Accepted, Happened::Done) | (Stage::Offered, Happened::Done) => Some(Stage::Done),
        (Stage::Accepted, Happened::Failed(w)) | (Stage::Offered, Happened::Failed(w)) => Some(Stage::Failed(w.clone())),
        _ => None,
    }
}

/// One transfer as this device keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfferRecord {
    pub offer: Offer,
    /// The other person.
    pub peer: String,
    /// This person is the sender.
    pub outgoing: bool,
    /// The file is on this device (the sender's) or comes to it (the receiver's, accepted here). A
    /// record learned from another of the person's devices (a self-copy of their note) is not
    /// here: this device shows it and never connects for it. For the receiver it also means this
    /// device answered: a Decline made here is here; an answer made on another of the person's
    /// devices is not, and that row says "Answered on your other device".
    pub here: bool,
    pub stage: Stage,
    /// The sender: the file that goes (a cleaned copy when the switch removed details). The
    /// receiver: the part file while it comes, the saved file once done.
    #[serde(default)]
    pub path: String,
    /// The sender's cleaned copy, made by this app: deleted when the transfer ends.
    #[serde(default)]
    pub own_copy: bool,
    /// The receiver pressed Accept on the "Files in messages" card for this one file.
    #[serde(default)]
    pub let_in: bool,
    /// Paused on this device (kept, so a paused transfer stays paused after a restart).
    #[serde(default)]
    pub paused: bool,
    /// When it was offered (the note's signed time, ms).
    pub at: u64,
    /// The notes about this transfer that could not go when they were made (the chat was not
    /// connected, or not yet signed in), the texts in the order made: sent once the server is
    /// connected again and its mailbox read (engine/file_transfer.rs `flush_notes`), and kept here
    /// so a restart keeps them too. At most [`NOTES_KEPT`]. A cancel made offline was lost before
    /// 2026-10-12, and the other side then waited for the file until its pass ran out, up to 7
    /// days, the sender's other files with them waiting behind it.
    #[serde(default)]
    pub unsaid: Vec<String>,
}

/// The most notes one transfer keeps to send later (the website's number).
pub const NOTES_KEPT: usize = 8;

/// What [`apply_note`] did with a note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Applied {
    /// A new transfer is listed.
    Listed(String),
    /// A listed transfer moved on.
    Moved(String, Stage),
    /// Nothing (a repeat, a note about nothing listed, an offer whose pass does not check, a note
    /// that is not from or to one of the two people).
    Nothing,
}

/// Apply one verified note from `from` to `to` signed at `ts` to the records. `me` is this
/// person's key; `pass_ok` checks an offer's pass (the caller rebuilds the words from the server's
/// did:hum, the sender and this person, `verify_file_pass`). The cases:
/// - an offer from someone else to me: listed as incoming, not here until accepted here; one whose
///   pass does not check is not listed (the relay would refuse its connection anyway);
/// - an offer from me (a self-copy, from my other device or this one's own echo): listed as
///   outgoing, not here, unless this device already lists it;
/// - an answer from the receiver of one of my offers, or from me about an offer to me (my other
///   device answered): Accepted or Declined while it waits for an answer;
/// - a cancel from either of the two people: Cancelled, `by_me` when I sent it.
///
/// A note about a transfer between other people (a forged id) changes nothing.
pub fn apply_note(records: &mut Vec<OfferRecord>, me: &str, from: &str, to: &str, ts: u64, note: Note, pass_ok: impl Fn(&Offer) -> bool) -> Applied {
    let from_me = from == me;
    let peer = if from_me { to } else { from };
    if peer.is_empty() || peer == me || (!from_me && to != me) {
        return Applied::Nothing;
    }
    match note {
        Note::Offer(offer) => {
            if records.iter().any(|r| r.offer.id == offer.id) {
                return Applied::Nothing; // this device's own offer echoed back, or a repeat
            }
            if !from_me && !pass_ok(&offer) {
                log::info!("a file offer whose pass does not check was not listed");
                return Applied::Nothing;
            }
            let id = offer.id.clone();
            records.push(OfferRecord {
                offer,
                peer: peer.to_string(),
                outgoing: from_me,
                here: false,
                stage: Stage::Offered,
                path: String::new(),
                own_copy: false,
                let_in: false,
                paused: false,
                at: ts,
                unsaid: Vec::new(),
            });
            Applied::Listed(id)
        }
        Note::Answer(a) => {
            // The receiver's answer: from them about my offer, or from me about theirs.
            let Some(r) = records.iter_mut().find(|r| r.offer.id == a.id && r.peer == peer && r.outgoing != from_me) else {
                return Applied::Nothing;
            };
            let what = if a.accept { Happened::Accepted } else { Happened::Declined };
            match advance(&r.stage, &what) {
                Some(s) => {
                    r.stage = s.clone();
                    Applied::Moved(a.id, s)
                }
                None => Applied::Nothing,
            }
        }
        Note::Cancel(c) => {
            let Some(r) = records.iter_mut().find(|r| r.offer.id == c.id && r.peer == peer) else { return Applied::Nothing };
            match advance(&r.stage, &Happened::Cancelled { by_me: from_me }) {
                Some(s) => {
                    r.stage = s.clone();
                    Applied::Moved(c.id, s)
                }
                None => Applied::Nothing,
            }
        }
    }
}

#[cfg(test)]
#[path = "file_offer_tests.rs"]
mod tests;
