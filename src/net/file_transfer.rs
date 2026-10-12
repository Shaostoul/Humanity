//! Device-to-device big files, what is said on the data channel (docs/design/media-showcase.md,
//! "Device-to-device big files: the shapes, exact", part 5). Both halves as plain state machines:
//! [`Receiving`] and [`Sending`] take what arrived and say what to send, with the file behind a
//! small trait, so each rule runs in a test against a stand-in peer and the real connection
//! (net/file_link.rs) only moves bytes.
//!
//! THE CONVERSATION, on one ordered, reliable channel labelled `file:<id>`:
//! - text messages are JSON `{"t":...}`; data messages are binary, at most 64 KiB each, header
//!   included (browsers cap a data channel message near 256 KiB);
//! - the receiver asks `{"t":"hashes"}`; the sender answers with the piece hashes as binary
//!   messages (32 bytes per piece, in order, split at 64 KiB); the receiver checks that BLAKE3 of
//!   all of them is the offer's `hashes`, or the transfer ends ("The file Ann offered does not
//!   match its description.");
//! - the receiver asks `{"t":"get","from":<piece>,"count":<n>}`, n at most 16 (4 MB in flight);
//!   the sender sends each piece as binary messages starting with an 8-byte header (the piece
//!   number and the offset within the piece, both u32 big-endian), holding back while the
//!   channel's buffer is over 1 MB;
//! - the receiver checks each whole piece against its hash and writes it; a piece that fails is
//!   asked for again, and three failures of one piece end the transfer with words;
//! - after a drop, on a new connection, the receiver asks again from the first piece it lacks;
//! - when every piece is in and the whole file's BLAKE3 matches (net/file_transfer.rs
//!   `finish_part`, run by the link), the receiver sends `{"t":"done"}`;
//! - `{"t":"pause"}`, `{"t":"resume"}` and `{"t":"cancel"}` from either side.
//!
//! ONE CHOICE THE SPEC LEFT OPEN, made here and kept liberal: "64 KiB binary messages, each
//! starting with an 8-byte header" is read as 65,536 bytes per message HEADER INCLUDED (65,528
//! bytes of the file), the bound the spec states for every data message. A message from another
//! app that carries a full 64 KiB after its header, or any other split, is taken all the same: the
//! receiver places each message by its own header and accepts up to 256 KiB per message.
//!
//! A pause from either side stops the flow both ways: the sender drops what it was asked for, the
//! receiver forgets what it asked, and a resume starts asking again from the first missing piece.

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::file_offer::{hashes_digest, piece_len, Offer, PIECE_SIZE};

/// The most one data message may be, header included.
pub const MAX_MESSAGE: usize = 65_536;
/// The piece number and the offset within it, both u32 big-endian.
pub const HEADER_LEN: usize = 8;
/// The bytes of the file in one full data message.
pub const CHUNK: usize = MAX_MESSAGE - HEADER_LEN;
/// The biggest data message accepted from another app (a browser's own cap).
pub const MAX_ACCEPTED: usize = 262_144 + HEADER_LEN;
/// The most pieces asked for at once (4 MB in flight).
pub const WINDOW: u64 = 16;
/// The sender holds back while the channel's buffer is over this.
pub const HIGH_WATER: usize = 1 << 20;
/// Failures of one piece that end the transfer.
pub const MAX_FAILURES: u8 = 3;
/// Nothing arriving for this long while pieces are asked for: ask again.
pub const STALL: Duration = Duration::from_secs(30);
/// Pieces of an existing part file checked per turn when a transfer resumes after a restart, so a
/// big part file is read a little at a time and the connection keeps being served.
pub const SCAN_PER_TICK: u64 = 16;
/// Hashes per binary message of the list (64 KiB of them).
const HASHES_PER_MESSAGE: usize = MAX_MESSAGE / 32;

/// One message to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Out {
    Text(String),
    Binary(Vec<u8>),
}

fn say(t: &str) -> Out {
    Out::Text(serde_json::json!({ "t": t }).to_string())
}

/// `{"t":"get","from":<from>,"count":<count>}`.
pub fn get_message(from: u64, count: u64) -> Out {
    Out::Text(serde_json::json!({ "t": "get", "from": from, "count": count }).to_string())
}

/// How a transfer ended on the channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum End {
    /// Every piece in, the whole file matched, `done` said.
    Done,
    /// Ended with these words.
    Failed(String),
    /// Called off; `by_them` when the other side said `cancel`.
    Cancelled { by_them: bool },
}

/// What a step reports, besides what to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Pieces in (the receiver) or sent at least once (the sender), of the offer's `pieces`.
    Progress(u64),
    /// The receiver has every piece: the link runs the whole-file check (`finish_part`) and hands
    /// the answer to [`Receiving::finished`].
    Complete,
    /// The other side paused (true) or resumed (false).
    PausedByThem(bool),
    Ended(End),
}

/// What one step produced.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Step {
    pub out: Vec<Out>,
    pub events: Vec<Event>,
}

impl Step {
    fn out(out: Vec<Out>) -> Self {
        Step { out, events: Vec::new() }
    }
}

/// The words when the list of piece hashes is not the one the offer described.
pub fn mismatch_line(sender: &str) -> String {
    format!("The file {sender} offered does not match its description.")
}

/// The words when one piece failed its check three times.
pub fn damaged_line(name: &str) -> String {
    format!("A piece of {name} arrived damaged three times, so the transfer was stopped.")
}

/// The words when the whole file did not match the offer.
pub fn whole_mismatch_line(name: &str, sender: &str) -> String {
    format!("{name} did not match what {sender} offered, so it was not saved.")
}

/// Where a file's bytes are read and written: the part file (receiver), the file sent (sender).
pub trait FileAt {
    /// Read up to `buf.len()` bytes at `at`; the count read (short only at the end of the file).
    fn read_at(&mut self, at: u64, buf: &mut [u8]) -> std::io::Result<usize>;
    /// Write `data` at `at`.
    fn write_at(&mut self, at: u64, data: &[u8]) -> std::io::Result<()>;
}

impl FileAt for std::fs::File {
    fn read_at(&mut self, at: u64, buf: &mut [u8]) -> std::io::Result<usize> {
        use std::io::{Seek, SeekFrom};
        self.seek(SeekFrom::Start(at))?;
        super::file_offer::read_full(self, buf)
    }
    fn write_at(&mut self, at: u64, data: &[u8]) -> std::io::Result<()> {
        use std::io::{Seek, SeekFrom, Write};
        self.seek(SeekFrom::Start(at))?;
        self.write_all(data)
    }
}

/// A file in memory (the tests' stand-in).
impl FileAt for Vec<u8> {
    fn read_at(&mut self, at: u64, buf: &mut [u8]) -> std::io::Result<usize> {
        let start = (at as usize).min(self.len());
        let n = buf.len().min(self.len() - start);
        buf[..n].copy_from_slice(&self[start..start + n]);
        Ok(n)
    }
    fn write_at(&mut self, at: u64, data: &[u8]) -> std::io::Result<()> {
        let end = at as usize + data.len();
        if self.len() < end {
            self.resize(end, 0);
        }
        self[at as usize..end].copy_from_slice(data);
        Ok(())
    }
}

/// The text message's `t`, and the message.
fn read_text(text: &str) -> Option<(String, serde_json::Value)> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let t = v.get("t")?.as_str()?.to_string();
    Some((t, v))
}

// ── The receiver ────────────────────────────────────────────────────────────────────────────

/// One piece being put together from its messages.
struct Building {
    buf: Vec<u8>,
}

/// The receiver's half of one transfer.
pub struct Receiving {
    pub offer: Offer,
    /// The sender's name, for the words.
    sender: String,
    hashes: Option<Vec<[u8; 32]>>,
    hash_buf: Vec<u8>,
    have: Vec<bool>,
    have_count: u64,
    /// Which pieces the part file holds is known: it was made fresh, or has been checked against
    /// the hashes (`tick` does that a little at a time after a restart).
    known: bool,
    scan_at: u64,
    /// No piece before this one is missing.
    low: u64,
    building: HashMap<u64, Building>,
    asked: BTreeSet<u64>,
    failures: HashMap<u64, u8>,
    paused_me: bool,
    paused_them: bool,
    open: bool,
    last_data: Instant,
    ended: bool,
    complete_said: bool,
}

impl Receiving {
    /// The receiver of `offer` from `sender` (their name). `fresh`: the part file was just made, so
    /// it holds nothing; otherwise it may hold pieces from before (a restart), which are checked
    /// against the hashes once they arrive.
    pub fn new(offer: Offer, sender: &str, fresh: bool, now: Instant) -> Self {
        let pieces = offer.pieces as usize;
        Receiving {
            offer,
            sender: sender.to_string(),
            hashes: None,
            hash_buf: Vec::new(),
            have: vec![false; pieces],
            have_count: 0,
            known: fresh,
            scan_at: 0,
            low: 0,
            building: HashMap::new(),
            asked: BTreeSet::new(),
            failures: HashMap::new(),
            paused_me: false,
            paused_them: false,
            open: false,
            last_data: now,
            ended: false,
            complete_said: false,
        }
    }

    /// Pieces in.
    pub fn have(&self) -> u64 {
        self.have_count
    }

    /// The pieces asked for and not yet in (the tests read it).
    pub fn asked(&self) -> Vec<u64> {
        self.asked.iter().copied().collect()
    }

    /// Paused on this device (it is not dialled while paused).
    pub fn paused(&self) -> bool {
        self.paused_me
    }

    /// Start paused (a transfer paused before a restart).
    pub fn set_paused(&mut self, paused: bool) {
        self.paused_me = paused;
    }

    fn fail(&mut self, words: String) -> Step {
        self.ended = true;
        Step { out: vec![say("cancel")], events: vec![Event::Ended(End::Failed(words))] }
    }

    /// A connection's channel opened: ask for the list (unless it is in), or straight for pieces.
    /// What was asked on an earlier connection is forgotten: it went with it.
    pub fn open(&mut self, now: Instant) -> Step {
        self.open = true;
        self.asked.clear();
        self.building.clear();
        self.paused_them = false;
        self.last_data = now;
        let mut out = Vec::new();
        if self.ended {
            return Step::default();
        }
        if self.paused_me {
            out.push(say("pause"));
        }
        if self.hashes.is_none() {
            self.hash_buf.clear();
            out.push(say("hashes"));
        } else {
            out.extend(self.pump());
        }
        Step::out(out)
    }

    /// The connection dropped.
    pub fn closed(&mut self) {
        self.open = false;
        self.asked.clear();
        self.building.clear();
        self.paused_them = false;
    }

    /// Ask for what is missing, up to the window: the first missing pieces not asked for yet, as
    /// one `get` per run of consecutive pieces.
    fn pump(&mut self) -> Vec<Out> {
        if !self.open || self.ended || self.paused_me || self.paused_them || self.hashes.is_none() || !self.known {
            return Vec::new();
        }
        let pieces = self.offer.pieces;
        while self.low < pieces && self.have[self.low as usize] {
            self.low += 1;
        }
        let mut out = Vec::new();
        let mut room = WINDOW.saturating_sub(self.asked.len() as u64);
        let mut i = self.low;
        while room > 0 && i < pieces {
            while i < pieces && (self.have[i as usize] || self.asked.contains(&i)) {
                i += 1;
            }
            if i >= pieces {
                break;
            }
            let start = i;
            let mut n = 0;
            while i < pieces && n < room && !self.have[i as usize] && !self.asked.contains(&i) {
                self.asked.insert(i);
                i += 1;
                n += 1;
            }
            room -= n;
            out.push(get_message(start, n));
        }
        out
    }

    /// Pause here: the sender is told, and nothing more is asked until Resume.
    pub fn pause(&mut self) -> Vec<Out> {
        if self.paused_me || self.ended {
            return Vec::new();
        }
        self.paused_me = true;
        self.asked.clear();
        if self.open { vec![say("pause")] } else { Vec::new() }
    }

    /// Resume here: the sender is told, and the missing pieces are asked for again.
    pub fn resume(&mut self) -> Vec<Out> {
        if !self.paused_me || self.ended {
            return Vec::new();
        }
        self.paused_me = false;
        let mut out = if self.open { vec![say("resume")] } else { Vec::new() };
        out.extend(self.pump());
        out
    }

    /// Cancel here: `cancel` is said (the engine also sends the cancel note).
    pub fn cancel(&mut self) -> Step {
        if self.ended {
            return Step::default();
        }
        self.ended = true;
        Step { out: if self.open { vec![say("cancel")] } else { Vec::new() }, events: vec![Event::Ended(End::Cancelled { by_them: false })] }
    }

    /// A text message from the sender.
    pub fn on_text(&mut self, text: &str) -> Step {
        if self.ended {
            return Step::default();
        }
        let Some((t, _)) = read_text(text) else { return Step::default() };
        match t.as_str() {
            "pause" => {
                self.paused_them = true;
                self.asked.clear(); // the sender dropped what it was asked for
                Step { out: Vec::new(), events: vec![Event::PausedByThem(true)] }
            }
            "resume" => {
                self.paused_them = false;
                Step { out: self.pump(), events: vec![Event::PausedByThem(false)] }
            }
            "cancel" => {
                self.ended = true;
                Step { out: Vec::new(), events: vec![Event::Ended(End::Cancelled { by_them: true })] }
            }
            _ => Step::default(),
        }
    }

    /// A data message from the sender: part of the list of hashes while it is coming, else part
    /// of a piece, placed by its header and written once the whole piece checks.
    pub fn on_binary(&mut self, data: &[u8], part: &mut impl FileAt, now: Instant) -> Step {
        if self.ended || data.len() > MAX_ACCEPTED {
            return Step::default();
        }
        self.last_data = now;
        if self.hashes.is_none() {
            return self.take_hashes(data);
        }
        if data.len() < HEADER_LEN {
            return Step::default();
        }
        let piece = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as u64;
        let offset = u32::from_be_bytes([data[4], data[5], data[6], data[7]]) as u64;
        let payload = &data[HEADER_LEN..];
        if piece >= self.offer.pieces || self.have[piece as usize] {
            return Step::default(); // nothing we need (a repeat, or a piece already in)
        }
        let plen = piece_len(self.offer.size, piece);
        if offset + payload.len() as u64 > plen {
            return self.piece_failed(piece);
        }
        if offset == 0 {
            self.building.insert(piece, Building { buf: Vec::with_capacity(plen as usize) });
        }
        let Some(b) = self.building.get_mut(&piece) else { return Step::default() };
        if b.buf.len() as u64 != offset {
            // Out of order (a piece cut off by a pause): forgotten, asked again in its turn.
            self.building.remove(&piece);
            return Step::default();
        }
        b.buf.extend_from_slice(payload);
        if (b.buf.len() as u64) < plen {
            return Step::default();
        }
        let Some(b) = self.building.remove(&piece) else { return Step::default() };
        let ok = self.hashes.as_ref().is_some_and(|h| blake3::hash(&b.buf).as_bytes() == &h[piece as usize]);
        if !ok {
            return self.piece_failed(piece);
        }
        if let Err(e) = part.write_at(piece * PIECE_SIZE, &b.buf) {
            return self.fail(format!("{} could not be written on this device ({e}).", self.offer.name));
        }
        self.have[piece as usize] = true;
        self.have_count += 1;
        self.asked.remove(&piece);
        self.failures.remove(&piece);
        let mut step = Step { out: self.pump(), events: vec![Event::Progress(self.have_count)] };
        self.check_complete(&mut step);
        step
    }

    /// A piece that did not check: asked for again, unless it has failed three times.
    fn piece_failed(&mut self, piece: u64) -> Step {
        self.building.remove(&piece);
        self.asked.remove(&piece);
        let n = self.failures.entry(piece).or_insert(0);
        *n += 1;
        if *n >= MAX_FAILURES {
            return self.fail(damaged_line(&self.offer.name));
        }
        Step::out(self.pump())
    }

    fn take_hashes(&mut self, data: &[u8]) -> Step {
        let expected = self.offer.pieces as usize * 32;
        self.hash_buf.extend_from_slice(data);
        if self.hash_buf.len() > expected {
            return self.fail(mismatch_line(&self.sender));
        }
        if self.hash_buf.len() < expected {
            return Step::default();
        }
        let list: Vec<[u8; 32]> = self.hash_buf.chunks_exact(32).map(|c| c.try_into().unwrap_or([0u8; 32])).collect();
        self.hash_buf = Vec::new();
        if hashes_digest(&list) != self.offer.hashes {
            return self.fail(mismatch_line(&self.sender));
        }
        self.hashes = Some(list);
        let mut step = Step::out(self.pump());
        self.check_complete(&mut step);
        step
    }

    fn check_complete(&mut self, step: &mut Step) {
        if self.known && self.have_count == self.offer.pieces && !self.complete_said {
            self.complete_said = true;
            step.events.push(Event::Complete);
        }
    }

    /// Once a turn: after a restart, check a few pieces of the part file against their hashes
    /// (a piece already right is not asked for again); and when nothing has arrived for
    /// [`STALL`] while pieces are asked for, ask again.
    pub fn tick(&mut self, part: &mut impl FileAt, now: Instant) -> Step {
        if self.ended {
            return Step::default();
        }
        let mut step = Step::default();
        if let (false, Some(hashes)) = (self.known, self.hashes.as_ref()) {
            let end = (self.scan_at + SCAN_PER_TICK).min(self.offer.pieces);
            let mut buf = vec![0u8; PIECE_SIZE as usize];
            for i in self.scan_at..end {
                let plen = piece_len(self.offer.size, i) as usize;
                let ok = matches!(part.read_at(i * PIECE_SIZE, &mut buf[..plen]), Ok(n) if n == plen)
                    && blake3::hash(&buf[..plen]).as_bytes() == &hashes[i as usize];
                if ok && !self.have[i as usize] {
                    self.have[i as usize] = true;
                    self.have_count += 1;
                }
            }
            self.scan_at = end;
            if self.scan_at >= self.offer.pieces {
                self.known = true;
                step.events.push(Event::Progress(self.have_count));
                step.out.extend(self.pump());
                self.check_complete(&mut step);
            }
            return step;
        }
        if self.open && !self.asked.is_empty() && now.duration_since(self.last_data) >= STALL {
            self.asked.clear();
            self.building.clear();
            self.last_data = now;
            step.out.extend(self.pump());
        }
        step
    }

    /// The whole-file check came back (`finish_part`): `done` is said and the transfer is done,
    /// or it ended with the check's words.
    pub fn finished(&mut self, result: Result<(), String>) -> Step {
        if self.ended {
            return Step::default();
        }
        match result {
            Ok(()) => {
                self.ended = true;
                Step { out: vec![say("done")], events: vec![Event::Ended(End::Done)] }
            }
            Err(words) => self.fail(words),
        }
    }
}

// ── The sender ──────────────────────────────────────────────────────────────────────────────

/// The sender's half of one transfer.
pub struct Sending {
    pub offer: Offer,
    hashes: Option<Arc<Vec<[u8; 32]>>>,
    /// The receiver asked for the list before it was ready.
    hashes_wanted: bool,
    /// The list's messages waiting to go.
    list_out: VecDeque<Out>,
    queue: VecDeque<u64>,
    /// The piece going out and the next offset in it.
    current: Option<(u64, u64)>,
    sent: Vec<bool>,
    sent_count: u64,
    paused_me: bool,
    paused_them: bool,
    open: bool,
    ended: bool,
}

impl Sending {
    /// The sender of `offer`, with its piece hashes when this device still has them (they are
    /// worked out again from the file otherwise, `set_hashes`).
    pub fn new(offer: Offer, hashes: Option<Arc<Vec<[u8; 32]>>>) -> Self {
        let pieces = offer.pieces as usize;
        Sending {
            offer,
            hashes,
            hashes_wanted: false,
            list_out: VecDeque::new(),
            queue: VecDeque::new(),
            current: None,
            sent: vec![false; pieces],
            sent_count: 0,
            paused_me: false,
            paused_them: false,
            open: false,
            ended: false,
        }
    }

    /// Whether the list of piece hashes is in hand.
    pub fn has_hashes(&self) -> bool {
        self.hashes.is_some()
    }

    /// Pieces sent at least once.
    pub fn sent(&self) -> u64 {
        self.sent_count
    }

    /// Start paused (a transfer paused before a restart).
    pub fn set_paused(&mut self, paused: bool) {
        self.paused_me = paused;
    }

    /// The list, worked out again from the file. It must be the one the offer described: a file
    /// changed since it was offered ends the transfer here, in plain words, rather than at the
    /// receiver's check.
    pub fn set_hashes(&mut self, list: Arc<Vec<[u8; 32]>>) -> Step {
        if hashes_digest(&list) != self.offer.hashes || list.len() as u64 != self.offer.pieces {
            self.ended = true;
            let words = format!("{} changed on this device since it was offered, so it was not sent.", self.offer.name);
            return Step { out: vec![say("cancel")], events: vec![Event::Ended(End::Failed(words))] };
        }
        self.hashes = Some(list);
        if std::mem::take(&mut self.hashes_wanted) {
            self.queue_list();
        }
        Step::default()
    }

    fn queue_list(&mut self) {
        let Some(list) = self.hashes.clone() else { return };
        for chunk in list.chunks(HASHES_PER_MESSAGE) {
            let mut m = Vec::with_capacity(chunk.len() * 32);
            for h in chunk {
                m.extend_from_slice(h);
            }
            self.list_out.push_back(Out::Binary(m));
        }
    }

    /// A connection's channel opened. What was asked on an earlier connection went with it.
    pub fn open(&mut self) -> Vec<Out> {
        self.open = true;
        self.queue.clear();
        self.current = None;
        self.list_out.clear();
        self.hashes_wanted = false;
        self.paused_them = false;
        if self.paused_me && !self.ended { vec![say("pause")] } else { Vec::new() }
    }

    /// The connection dropped.
    pub fn closed(&mut self) {
        self.open = false;
        self.queue.clear();
        self.current = None;
        self.list_out.clear();
        self.paused_them = false;
    }

    /// Pause here: nothing more goes, and the receiver is told.
    pub fn pause(&mut self) -> Vec<Out> {
        if self.paused_me || self.ended {
            return Vec::new();
        }
        self.paused_me = true;
        self.queue.clear();
        self.current = None;
        if self.open { vec![say("pause")] } else { Vec::new() }
    }

    /// Resume here: the receiver is told and asks again.
    pub fn resume(&mut self) -> Vec<Out> {
        if !self.paused_me || self.ended {
            return Vec::new();
        }
        self.paused_me = false;
        if self.open { vec![say("resume")] } else { Vec::new() }
    }

    /// Cancel here.
    pub fn cancel(&mut self) -> Step {
        if self.ended {
            return Step::default();
        }
        self.ended = true;
        Step { out: if self.open { vec![say("cancel")] } else { Vec::new() }, events: vec![Event::Ended(End::Cancelled { by_them: false })] }
    }

    /// A text message from the receiver.
    pub fn on_text(&mut self, text: &str) -> Step {
        if self.ended {
            return Step::default();
        }
        let Some((t, v)) = read_text(text) else { return Step::default() };
        match t.as_str() {
            "hashes" => {
                self.list_out.clear();
                if self.hashes.is_some() {
                    self.queue_list();
                } else {
                    self.hashes_wanted = true;
                }
                Step::default()
            }
            "get" => {
                let from = v.get("from").and_then(|x| x.as_u64()).unwrap_or(u64::MAX);
                let count = v.get("count").and_then(|x| x.as_u64()).unwrap_or(0).min(WINDOW);
                let end = from.saturating_add(count).min(self.offer.pieces);
                for p in from..end {
                    if !self.queue.contains(&p) && self.current.map(|(c, _)| c) != Some(p) {
                        self.queue.push_back(p);
                    }
                }
                Step::default()
            }
            "pause" => {
                self.paused_them = true;
                self.queue.clear();
                self.current = None;
                Step { out: Vec::new(), events: vec![Event::PausedByThem(true)] }
            }
            "resume" => {
                self.paused_them = false;
                Step { out: Vec::new(), events: vec![Event::PausedByThem(false)] }
            }
            "cancel" => {
                self.ended = true;
                Step { out: Vec::new(), events: vec![Event::Ended(End::Cancelled { by_them: true })] }
            }
            "done" => {
                self.ended = true;
                Step { out: Vec::new(), events: vec![Event::Ended(End::Done)] }
            }
            _ => Step::default(),
        }
    }

    /// The next message to write, when one may go now: the list first (it is small and the
    /// receiver waits on it), then the pieces asked for, one data message at a time, held back
    /// while paused either side or while `buffered` (the channel's buffer) is over [`HIGH_WATER`].
    /// Err ends the transfer: the file could not be read as it was offered.
    pub fn next(&mut self, file: &mut impl FileAt, buffered: usize) -> Result<Option<(Out, Option<Event>)>, String> {
        if self.ended || !self.open {
            return Ok(None);
        }
        if buffered >= HIGH_WATER {
            return Ok(None);
        }
        if let Some(m) = self.list_out.pop_front() {
            return Ok(Some((m, None)));
        }
        if self.paused_me || self.paused_them {
            return Ok(None);
        }
        let (piece, offset) = match self.current {
            Some(c) => c,
            None => match self.queue.pop_front() {
                Some(p) => (p, 0),
                None => return Ok(None),
            },
        };
        let plen = piece_len(self.offer.size, piece);
        let len = (plen - offset).min(CHUNK as u64) as usize;
        let mut msg = vec![0u8; HEADER_LEN + len];
        msg[..4].copy_from_slice(&(piece as u32).to_be_bytes());
        msg[4..8].copy_from_slice(&(offset as u32).to_be_bytes());
        // On Err the caller cancels (which says `cancel` to the receiver) with these words.
        let read = file.read_at(piece * PIECE_SIZE + offset, &mut msg[HEADER_LEN..]).map_err(|e| format!("{} could not be read on this device any more ({e}).", self.offer.name))?;
        if read != len {
            return Err(format!("{} changed on this device since it was offered, so it was not sent.", self.offer.name));
        }
        let next = offset + len as u64;
        let mut event = None;
        if next >= plen {
            self.current = None;
            if !self.sent[piece as usize] {
                self.sent[piece as usize] = true;
                self.sent_count += 1;
                event = Some(Event::Progress(self.sent_count));
            }
        } else {
            self.current = Some((piece, next));
        }
        Ok(Some((Out::Binary(msg), event)))
    }

    /// Whether anything waits to be written (the link loops quickly while so).
    pub fn busy(&self) -> bool {
        self.open && !self.ended && (!self.list_out.is_empty() || (!self.paused_me && !self.paused_them && (self.current.is_some() || !self.queue.is_empty())))
    }
}

// ── The end ─────────────────────────────────────────────────────────────────────────────────

/// THE WHOLE-FILE CHECK, then the rename: the part file at `part` is read whole for its BLAKE3,
/// and only when that is the offer's `whole` is it given its name in `dir` (`name`, already made
/// safe, or "name (2)" and so on when taken, claimed so no other writer takes it between). On a
/// mismatch nothing is renamed and the part file stays where it is (the caller deletes it).
pub fn finish_part(part: &std::path::Path, dir: &std::path::Path, name: &str, whole: &str, sender: &str) -> Result<std::path::PathBuf, String> {
    let mut f = std::fs::File::open(part).map_err(|e| format!("{name} could not be read back on this device ({e})."))?;
    let mut h = blake3::Hasher::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = super::file_offer::read_full(&mut f, &mut buf).map_err(|e| format!("{name} could not be read back on this device ({e})."))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        if n < buf.len() {
            break;
        }
    }
    drop(f);
    if h.finalize().to_hex().as_str() != whole {
        return Err(whole_mismatch_line(name, sender));
    }
    crate::gui::widgets::part_file::claim_and_move(part, dir, name)
}

#[cfg(test)]
#[path = "file_transfer_tests.rs"]
mod tests;
