// Child of net/file_transfer.rs (#[path]): the channel protocol against stand-in peers, each rule
// seen failing once against the break named in its doc comment. The stand-ins are written here
// from the spec's words, not from the code they test: `StandInSender` answers `hashes` and `get`
// the way part 5 says a sender does (and can damage a piece on purpose), `ask` reads a `get`.

use super::*;
use crate::net::file_offer::{hashes_digest, pieces_for, FilePass, Offer, PIECE_SIZE};
use std::time::Instant;

const ID: &str = "00112233445566778899aabbccddeeff";

/// A file of `len` bytes, its offer and its piece hashes.
fn file(len: usize) -> (Vec<u8>, Offer, Vec<[u8; 32]>) {
    let bytes: Vec<u8> = (0..len).map(|i| (i * 31 % 253) as u8).collect();
    let hashes: Vec<[u8; 32]> = bytes.chunks(PIECE_SIZE as usize).map(|c| *blake3::hash(c).as_bytes()).collect();
    let offer = Offer {
        id: ID.into(),
        name: "clip.mp4".into(),
        size: len as u64,
        mime: "video/mp4".into(),
        blake3: blake3::hash(&bytes).to_hex().to_string(),
        piece: PIECE_SIZE,
        pieces: pieces_for(len as u64),
        hashes: hashes_digest(&hashes),
        pass: FilePass { v: 1, id: ID.into(), expiry: 1, sig: "x".into() },
    };
    (bytes, offer, hashes)
}

/// A stand-in sender, from the spec's words: the list in 64 KiB messages on `hashes`; on `get`,
/// each piece in messages of at most 64 KiB, header included, each starting with the piece number
/// and the offset as u32 big-endian. `damage` names pieces to send wrong, and how many times.
struct StandInSender {
    bytes: Vec<u8>,
    hashes: Vec<[u8; 32]>,
    damage: std::collections::HashMap<u64, u32>,
    /// Data messages carry this much of the file each (to try another app's split).
    chunk: usize,
}

impl StandInSender {
    fn new(bytes: &[u8], hashes: &[[u8; 32]]) -> Self {
        StandInSender { bytes: bytes.to_vec(), hashes: hashes.to_vec(), damage: Default::default(), chunk: 65_536 - 8 }
    }

    /// What it sends back for one message from the receiver.
    fn answer(&mut self, msg: &Out) -> Vec<Vec<u8>> {
        let Out::Text(t) = msg else { return Vec::new() };
        let v: serde_json::Value = serde_json::from_str(t).unwrap();
        match v["t"].as_str().unwrap() {
            "hashes" => self.hashes.concat().chunks(65_536).map(|c| c.to_vec()).collect(),
            "get" => {
                let (from, count) = (v["from"].as_u64().unwrap(), v["count"].as_u64().unwrap());
                assert!(count >= 1 && count <= 16, "at most 16 pieces asked at once: {t}");
                let mut out = Vec::new();
                for p in from..from + count {
                    let start = (p * PIECE_SIZE) as usize;
                    let end = (start + PIECE_SIZE as usize).min(self.bytes.len());
                    let mut piece = self.bytes[start..end].to_vec();
                    if let Some(n) = self.damage.get_mut(&p).filter(|n| **n > 0) {
                        *n -= 1;
                        piece[0] ^= 0xff;
                    }
                    for (i, c) in piece.chunks(self.chunk).enumerate() {
                        let mut m = Vec::with_capacity(8 + c.len());
                        m.extend_from_slice(&(p as u32).to_be_bytes());
                        m.extend_from_slice(&((i * self.chunk) as u32).to_be_bytes());
                        m.extend_from_slice(c);
                        out.push(m);
                    }
                }
                out
            }
            _ => Vec::new(),
        }
    }
}

/// Run the receiver against the stand-in until neither has anything more to say. Returns every
/// event the receiver reported and every message it sent.
fn run(rx: &mut Receiving, tx: &mut StandInSender, part: &mut Vec<u8>, first: Vec<Out>) -> (Vec<Event>, Vec<Out>) {
    let mut events = Vec::new();
    let mut said = Vec::new();
    let mut queue: std::collections::VecDeque<Out> = first.into();
    let now = Instant::now();
    let mut turns = 0;
    while let Some(m) = queue.pop_front() {
        turns += 1;
        assert!(turns < 100_000, "the conversation ends");
        said.push(m.clone());
        for data in tx.answer(&m) {
            let step = rx.on_binary(&data, part, now);
            events.extend(step.events.clone());
            queue.extend(step.out);
        }
    }
    (events, said)
}

fn gets(said: &[Out]) -> Vec<(u64, u64)> {
    said.iter()
        .filter_map(|m| match m {
            Out::Text(t) => {
                let v: serde_json::Value = serde_json::from_str(t).ok()?;
                (v["t"] == "get").then(|| (v["from"].as_u64().unwrap(), v["count"].as_u64().unwrap()))
            }
            _ => None,
        })
        .collect()
}

fn said(out: &[Out], t: &str) -> bool {
    out.iter().any(|m| matches!(m, Out::Text(s) if s == &format!("{{\"t\":\"{t}\"}}")))
}

/// The whole conversation, in memory: the list checks, every piece arrives, and the part file holds
/// the file. Also with another app's split (a full 64 KiB of the file after each header, or a whole
/// piece per message): the receiver places each message by its header and takes up to 256 KiB.
/// Seen red 2026-10-11 with `MAX_ACCEPTED` set to `MAX_MESSAGE` (only this app's own split taken):
/// "every piece in (65536)" failed.
#[test]
fn a_whole_file_crosses_against_the_stand_in() {
    for chunk in [65_528, 65_536, 262_144] {
        let (bytes, offer, hashes) = file(PIECE_SIZE as usize * 20 + 777);
        let mut rx = Receiving::new(offer.clone(), "Ann", true, Instant::now());
        let mut tx = StandInSender::new(&bytes, &hashes);
        tx.chunk = chunk;
        let mut part = Vec::new();
        let first = rx.open(Instant::now()).out;
        assert_eq!(first, vec![Out::Text("{\"t\":\"hashes\"}".into())], "the list first");
        let (events, out) = run(&mut rx, &mut tx, &mut part, first);
        assert!(events.contains(&Event::Complete), "every piece in ({chunk})");
        assert_eq!(part, bytes, "the part file holds the file ({chunk})");
        assert!(gets(&out).iter().all(|(_, n)| *n <= WINDOW), "never more than 16 asked at once");
        assert_eq!(rx.have(), offer.pieces);
        assert_eq!(rx.finished(Ok(())).out, vec![Out::Text("{\"t\":\"done\"}".into())], "then done");
    }
}

/// THE HASH-LIST CHECK (part 5): a list whose BLAKE3 is not the offer's `hashes` ends the transfer
/// with the spec's words and says `cancel`; the right list, arriving split at 64 KiB (3,000 pieces
/// make two messages), leads straight on to asking for pieces from the first. A list longer than
/// the offer's pieces fails too.
/// Seen red 2026-10-11 with the digest comparison in `take_hashes` taken out: "a list that is not
/// the offer's ends it" got gets instead.
#[test]
fn the_hash_list_is_checked_against_the_offer() {
    let (bytes, offer, hashes) = file(PIECE_SIZE as usize * 4);
    let mut rx = Receiving::new(offer.clone(), "Ann", true, Instant::now());
    rx.open(Instant::now());
    let mut wrong = hashes.clone();
    wrong[2][0] ^= 1;
    let step = rx.on_binary(&wrong.concat(), &mut Vec::new(), Instant::now());
    assert_eq!(step.events, vec![Event::Ended(End::Failed("The file Ann offered does not match its description.".into()))], "a list that is not the offer's ends it");
    assert!(said(&step.out, "cancel"));

    let mut rx = Receiving::new(offer.clone(), "Ann", true, Instant::now());
    rx.open(Instant::now());
    let mut long = hashes.concat();
    long.extend_from_slice(&[0u8; 32]);
    assert!(matches!(rx.on_binary(&long, &mut Vec::new(), Instant::now()).events.as_slice(), [Event::Ended(End::Failed(_))]), "too long");

    // 3,000 pieces of list, split at 64 KiB: two messages.
    let many: Vec<[u8; 32]> = (0..3000u32).map(|i| *blake3::hash(&i.to_be_bytes()).as_bytes()).collect();
    let big = Offer { size: 3000 * PIECE_SIZE, pieces: 3000, hashes: hashes_digest(&many), ..offer };
    let mut rx = Receiving::new(big, "Ann", true, Instant::now());
    rx.open(Instant::now());
    let mut tx = StandInSender::new(&bytes, &many);
    let parts = tx.answer(&Out::Text("{\"t\":\"hashes\"}".into()));
    assert_eq!(parts.len(), 2, "split at 64 KiB");
    assert!(rx.on_binary(&parts[0], &mut Vec::new(), Instant::now()).out.is_empty(), "nothing asked until the whole list is in");
    let step = rx.on_binary(&parts[1], &mut Vec::new(), Instant::now());
    assert_eq!(gets(&step.out), vec![(0, 16)], "then the first window");
}

/// PIECE CHECKING (part 5): a piece that fails its hash is asked for again (alone, the window
/// refilled) and the transfer completes; one that fails three times ends it with words and
/// `cancel`, writing nothing of it.
/// Seen red 2026-10-11 with `MAX_FAILURES` checked as `> MAX_FAILURES` in `piece_failed`: "three
/// failures end it" saw a fourth `get` for piece 1 instead.
#[test]
fn a_bad_piece_is_asked_again_and_three_failures_end_it() {
    let (bytes, offer, hashes) = file(PIECE_SIZE as usize * 3);
    let mut rx = Receiving::new(offer.clone(), "Ann", true, Instant::now());
    let mut tx = StandInSender::new(&bytes, &hashes);
    tx.damage.insert(1, 2);
    let mut part = Vec::new();
    let first = rx.open(Instant::now()).out;
    let (events, out) = run(&mut rx, &mut tx, &mut part, first);
    assert!(events.contains(&Event::Complete), "two failures, then it arrives");
    assert_eq!(part, bytes);
    assert_eq!(gets(&out), vec![(0, 3), (1, 1), (1, 1)], "the bad piece asked for again, alone");

    let mut rx = Receiving::new(offer, "Ann", true, Instant::now());
    let mut tx = StandInSender::new(&bytes, &hashes);
    tx.damage.insert(1, 3);
    let mut part = Vec::new();
    let first = rx.open(Instant::now()).out;
    let (events, out) = run(&mut rx, &mut tx, &mut part, first);
    assert_eq!(
        events.iter().filter(|e| matches!(e, Event::Ended(_))).collect::<Vec<_>>(),
        vec![&Event::Ended(End::Failed("A piece of clip.mp4 arrived damaged three times, so the transfer was stopped.".into()))],
        "three failures end it"
    );
    assert_eq!(gets(&out).iter().filter(|(from, n)| *from <= 1 && 1 < from + n).count(), 3, "asked three times, no more");
    assert!(said(&out, "cancel"));
    assert!(!events.contains(&Event::Complete));
    assert!(part.len() <= 2 * PIECE_SIZE as usize || part[PIECE_SIZE as usize..2 * PIECE_SIZE as usize].iter().all(|b| *b == 0), "nothing of the bad piece written");
}

/// RESUME FROM THE FIRST MISSING PIECE (part 5): after a drop, the new connection asks straight for
/// the first piece it lacks (the list is not asked again); after a restart, a receiver over a part
/// file that already holds the first pieces asks for the list, checks the part file against it a
/// little at a time, and then asks from the first piece it lacks.
/// Seen red 2026-10-11 with `pump` starting its search at 0 instead of the first missing piece and
/// ignoring what the part file holds (`self.have[i]` dropped from the skip): "after a drop, from
/// the first missing piece" got (0, 16).
#[test]
fn a_dropped_transfer_resumes_from_the_first_missing_piece() {
    let pieces = 40usize;
    let (bytes, offer, hashes) = file(PIECE_SIZE as usize * pieces);
    let now = Instant::now();
    let mut rx = Receiving::new(offer.clone(), "Ann", true, now);
    let mut tx = StandInSender::new(&bytes, &hashes);
    let mut part = Vec::new();
    rx.open(now);
    for m in tx.answer(&Out::Text("{\"t\":\"hashes\"}".into())) {
        rx.on_binary(&m, &mut part, now);
    }
    // The first window's first ten pieces arrive, then the connection drops.
    for m in tx.answer(&get_message(0, 10)) {
        rx.on_binary(&m, &mut part, now);
    }
    assert_eq!(rx.have(), 10);
    rx.closed();
    let step = rx.open(now);
    assert!(!said(&step.out, "hashes"), "the list is in hand");
    assert_eq!(gets(&step.out), vec![(10, 16)], "after a drop, from the first missing piece");

    // A restart: a new receiver over that part file (not fresh).
    let mut rx = Receiving::new(offer.clone(), "Ann", false, now);
    let step = rx.open(now);
    assert!(said(&step.out, "hashes"));
    let mut got = Vec::new();
    for m in tx.answer(&Out::Text("{\"t\":\"hashes\"}".into())) {
        got.extend(rx.on_binary(&m, &mut part, now).out);
    }
    assert!(gets(&got).is_empty(), "nothing asked before the part file is checked");
    let mut asked = Vec::new();
    for _ in 0..pieces {
        asked.extend(rx.tick(&mut part, now).out);
    }
    assert_eq!(rx.have(), 10, "the ten pieces it holds are found");
    assert_eq!(gets(&asked), vec![(10, 16)], "after a restart, from the first missing piece");
    let (events, _) = run(&mut rx, &mut tx, &mut part, asked);
    assert!(events.contains(&Event::Complete));
    assert_eq!(part, bytes);
}

/// PAUSE AND RESUME FROM EITHER SIDE (part 5): the receiver's pause says `pause` and asks for
/// nothing more until Resume (what was already on its way still lands, since it was asked), and
/// Resume says `resume` and asks again from the first missing piece; the sender's pause says
/// `pause`, stops what it sends and drops what it owed (a `get` reaching it while paused too: the
/// receiver, told of the pause, forgets what it asked and asks again after `resume`).
/// Seen red 2026-10-11 with `Sending::next` ignoring `paused_me`: "a paused sender sends nothing"
/// got a data message. Seen red 2026-10-12 with the receiver's own pause clearing `asked` (as
/// before the first review, when what was on its way would then be ignored as not asked for):
/// "what was asked stays asked while it may still land" found nothing asked.
#[test]
fn pause_and_resume_from_either_side() {
    let (bytes, offer, hashes) = file(PIECE_SIZE as usize * 40);
    let now = Instant::now();
    let mut rx = Receiving::new(offer.clone(), "Ann", true, now);
    let mut tx = StandInSender::new(&bytes, &hashes);
    let mut part = Vec::new();
    rx.open(now);
    for m in tx.answer(&Out::Text("{\"t\":\"hashes\"}".into())) {
        rx.on_binary(&m, &mut part, now);
    }
    assert_eq!(rx.pause(), vec![Out::Text("{\"t\":\"pause\"}".into())]);
    assert!(rx.paused());
    assert_eq!(rx.asked(), (0..16).collect::<Vec<u64>>(), "what was asked stays asked while it may still land");
    for m in tx.answer(&get_message(0, 2)) {
        rx.on_binary(&m, &mut part, now);
    }
    assert_eq!(rx.have(), 2, "what was already on its way still lands");
    assert!(gets(&rx.tick(&mut part, now + STALL * 2).out).is_empty(), "nothing asked while paused, a stall neither");
    let out = rx.resume(now);
    assert_eq!(out.first(), Some(&Out::Text("{\"t\":\"resume\"}".into())));
    assert_eq!(gets(&out), vec![(2, 16)], "asked again from the first missing piece");

    // The receiver told the sender paused.
    let step = rx.on_text("{\"t\":\"pause\"}", now);
    assert_eq!(step.events, vec![Event::PausedByThem(true)]);
    assert!(rx.asked().is_empty());
    let step = rx.on_text("{\"t\":\"resume\"}", now);
    assert_eq!(step.events, vec![Event::PausedByThem(false)]);
    assert_eq!(gets(&step.out), vec![(2, 16)]);

    // The sender's own pause.
    let mut s = Sending::new(offer.clone(), Some(std::sync::Arc::new(hashes.clone())));
    let mut src = bytes.clone();
    s.open();
    s.on_text("{\"t\":\"get\",\"from\":0,\"count\":4}");
    assert_eq!(s.pause(), vec![Out::Text("{\"t\":\"pause\"}".into())]);
    s.on_text("{\"t\":\"get\",\"from\":0,\"count\":2}"); // asked before the receiver heard the pause
    assert_eq!(s.next(&mut src, 0).unwrap(), None, "a paused sender sends nothing");
    assert_eq!(s.resume(), vec![Out::Text("{\"t\":\"resume\"}".into())]);
    assert_eq!(s.next(&mut src, 0).unwrap(), None, "what was owed, and asked while paused, was dropped: the receiver asks again");
    s.on_text("{\"t\":\"get\",\"from\":0,\"count\":1}");
    assert!(matches!(s.next(&mut src, 0).unwrap(), Some((Out::Binary(_), None))));
    // Told the receiver paused, it stops too.
    s.on_text("{\"t\":\"pause\"}");
    assert_eq!(s.next(&mut src, 0).unwrap(), None);
}

/// CANCEL FROM EITHER SIDE (part 5): the side that cancels says `cancel` and ends as cancelled by
/// itself; the side that hears it ends as cancelled by them; nothing is said or asked after.
/// Seen red 2026-10-11 with `Receiving::on_text` ignoring "cancel": "the receiver hears the
/// sender's cancel" saw no event.
#[test]
fn cancel_from_either_side() {
    let (bytes, offer, hashes) = file(PIECE_SIZE as usize * 4);
    let now = Instant::now();
    let mut rx = Receiving::new(offer.clone(), "Ann", true, now);
    rx.open(now);
    let step = rx.cancel();
    assert_eq!(step.out, vec![Out::Text("{\"t\":\"cancel\"}".into())]);
    assert_eq!(step.events, vec![Event::Ended(End::Cancelled { by_them: false })]);
    assert_eq!(rx.resume(now), Vec::<Out>::new(), "nothing after");

    let mut s = Sending::new(offer.clone(), Some(std::sync::Arc::new(hashes.clone())));
    s.open();
    assert_eq!(s.on_text("{\"t\":\"cancel\"}").events, vec![Event::Ended(End::Cancelled { by_them: true })], "the sender hears the receiver's cancel");
    s.on_text("{\"t\":\"get\",\"from\":0,\"count\":1}");
    assert_eq!(s.next(&mut bytes.clone(), 0).unwrap(), None, "and sends nothing more");

    let mut s = Sending::new(offer.clone(), Some(std::sync::Arc::new(hashes)));
    s.open();
    let step = s.cancel();
    assert_eq!((step.out, step.events), (vec![Out::Text("{\"t\":\"cancel\"}".into())], vec![Event::Ended(End::Cancelled { by_them: false })]));
    let mut rx = Receiving::new(offer, "Ann", true, now);
    rx.open(now);
    assert_eq!(rx.on_text("{\"t\":\"cancel\"}", now).events, vec![Event::Ended(End::Cancelled { by_them: true })], "the receiver hears the sender's cancel");
}

/// THE SENDER (part 5): the list on `hashes` (worked out again when it was not in hand, and checked
/// against the offer); each piece in messages of at most 64 KiB, header included (the piece and
/// the offset, u32 big-endian), in the order asked; nothing while the channel's buffer is over
/// 1 MB; `done` ends it. Driven by a stand-in receiver: a real `Receiving` gets the whole file.
/// Seen red 2026-10-11 with `CHUNK` set to `MAX_MESSAGE` (the header on top of a full 64 KiB):
/// "at most 64 KiB, header included" failed at 65,544.
#[test]
fn the_sender_sends_what_is_asked_in_64_kib_messages() {
    let (bytes, offer, hashes) = file(PIECE_SIZE as usize * 3 + 5);
    let mut src = bytes.clone();
    let mut s = Sending::new(offer.clone(), None);
    s.open();
    s.on_text("{\"t\":\"hashes\"}");
    assert_eq!(s.next(&mut src, 0).unwrap(), None, "the list is not in hand yet");
    let mut wrong = hashes.clone();
    wrong[0][0] ^= 1;
    let mut s2 = Sending::new(offer.clone(), None);
    assert!(matches!(s2.set_hashes(std::sync::Arc::new(wrong)).events.as_slice(), [Event::Ended(End::Failed(_))]), "a file changed since it was offered");
    s.set_hashes(std::sync::Arc::new(hashes.clone()));
    let mut rx = Receiving::new(offer.clone(), "Ann", true, Instant::now());
    rx.open(Instant::now());
    let mut part = Vec::new();
    let mut to_sender: Vec<Out> = Vec::new();
    loop {
        for m in to_sender.drain(..) {
            if let Out::Text(t) = m {
                s.on_text(&t);
            }
        }
        assert_eq!(s.next(&mut src, HIGH_WATER).unwrap(), None, "nothing over 1 MB buffered");
        let Some((m, _)) = s.next(&mut src, 0).unwrap() else { break };
        let Out::Binary(b) = m else { panic!("data only") };
        assert!(b.len() <= MAX_MESSAGE, "at most 64 KiB, header included: {}", b.len());
        let step = rx.on_binary(&b, &mut part, Instant::now());
        to_sender.extend(step.out);
        if step.events.contains(&Event::Complete) {
            break;
        }
    }
    assert_eq!(part, bytes, "the stand-in receiver got the whole file");
    assert_eq!(s.sent(), offer.pieces);
    assert_eq!(s.on_text("{\"t\":\"done\"}").events, vec![Event::Ended(End::Done)]);
}

/// THE RENAME ONLY AFTER THE WHOLE-FILE BLAKE3 MATCHES (part 5): a part file whose whole hash is
/// the offer's gets its name (or "name (2)" beside an existing one); one whose hash is not keeps
/// no name, stays a part file, and the words say why.
/// Seen red 2026-10-11 with the whole-file comparison in `finish_part` taken out: the first
/// assertion failed, `finish_part` answering Ok (the damaged file named clip.mp4) where the words
/// were expected.
#[test]
fn the_part_file_is_named_only_after_the_whole_file_matches() {
    let dir = std::env::temp_dir().join(format!("hum_finish_part_{}_{}", std::process::id(), line!()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (bytes, offer, _) = file(PIECE_SIZE as usize + 9);
    let part = dir.join("clip.mp4.001122334455.part");
    let mut damaged = bytes.clone();
    damaged[5] ^= 1;
    std::fs::write(&part, &damaged).unwrap();
    let r = finish_part(&part, &dir, "clip.mp4", &offer.blake3, "Ann");
    assert_eq!(r, Err("clip.mp4 did not match what Ann offered, so it was not saved.".to_string()));
    assert!(!dir.join("clip.mp4").exists(), "a part file that does not match is not named");
    assert!(part.exists(), "it stays a part file (the engine deletes it)");

    std::fs::write(&part, &bytes).unwrap();
    std::fs::write(dir.join("clip.mp4"), b"someone else's").unwrap();
    let saved = finish_part(&part, &dir, "clip.mp4", &offer.blake3, "Ann").unwrap();
    assert_eq!(saved, dir.join("clip (2).mp4"), "never over an existing file");
    assert_eq!(std::fs::read(&saved).unwrap(), bytes);
    assert!(!part.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

// ── The rules settled by the first review (media-showcase.md, 2026-10-12, rule 1) ──────────────

/// The `(piece, offset)` of a data message.
fn header(m: &[u8]) -> (u64, u64) {
    (u32::from_be_bytes([m[0], m[1], m[2], m[3]]) as u64, u32::from_be_bytes([m[4], m[5], m[6], m[7]]) as u64)
}

/// A stand-in sender that follows rule 1 to the letter, written from the rule's words, not from
/// `Sending`: a `get` ADDS pieces to what it owes; it sends what it owes lowest first, each piece
/// once, from offset 0, in messages of at most 64 KiB header included; it forgets a piece once its
/// last message has gone; a piece asked for while it is being sent goes again, whole, after the
/// current one. `damage` names pieces whose next copies go out wrong (one byte flipped), and how
/// many times.
struct RuleOneSender {
    bytes: Vec<u8>,
    hashes: Vec<[u8; 32]>,
    list_out: std::collections::VecDeque<Vec<u8>>,
    owed: std::collections::BTreeSet<u64>,
    /// The piece going out, its bytes as they go (damaged or not), and the next offset.
    current: Option<(u64, Vec<u8>, usize)>,
    damage: std::collections::HashMap<u64, u32>,
}

impl RuleOneSender {
    fn new(bytes: &[u8], hashes: &[[u8; 32]]) -> Self {
        RuleOneSender { bytes: bytes.to_vec(), hashes: hashes.to_vec(), list_out: Default::default(), owed: Default::default(), current: None, damage: Default::default() }
    }

    /// A message from the receiver.
    fn hear(&mut self, msg: &Out) {
        let Out::Text(t) = msg else { panic!("the receiver sends text only") };
        let v: serde_json::Value = serde_json::from_str(t).unwrap();
        match v["t"].as_str().unwrap() {
            "hashes" => self.list_out.extend(self.hashes.concat().chunks(65_536).map(|c| c.to_vec())),
            "get" => {
                let (from, count) = (v["from"].as_u64().unwrap(), v["count"].as_u64().unwrap());
                assert!((1..=16).contains(&count), "at most 16 pieces asked at once: {t}");
                let pieces = (self.bytes.len() as u64).div_ceil(PIECE_SIZE);
                self.owed.extend(from..(from + count).min(pieces));
            }
            _ => {}
        }
    }

    /// The next data message, or None when nothing is owed.
    fn next(&mut self) -> Option<Vec<u8>> {
        if let Some(m) = self.list_out.pop_front() {
            return Some(m);
        }
        if self.current.is_none() {
            let p = self.owed.pop_first()?;
            let start = (p * PIECE_SIZE) as usize;
            let mut piece = self.bytes[start..(start + PIECE_SIZE as usize).min(self.bytes.len())].to_vec();
            if let Some(n) = self.damage.get_mut(&p).filter(|n| **n > 0) {
                *n -= 1;
                piece[0] ^= 0xff;
            }
            self.current = Some((p, piece, 0));
        }
        let (p, piece, at) = self.current.as_mut()?;
        let end = (*at + 65_528).min(piece.len());
        let mut m = Vec::with_capacity(8 + end - *at);
        m.extend_from_slice(&(*p as u32).to_be_bytes());
        m.extend_from_slice(&(*at as u32).to_be_bytes());
        m.extend_from_slice(&piece[*at..end]);
        *at = end;
        if *at >= piece.len() {
            self.current = None; // its last message has gone: forgotten
        }
        Some(m)
    }
}

/// RULE 1, THE RECEIVER (a get adds; ask each piece once, again only after a bad hash, `count` 1):
/// driven by a stand-in sender that follows rule 1 exactly, with what the receiver says reaching
/// the sender only after `lag` more data messages (so asks cross pieces in flight), the desktop
/// receiver gets the whole file with no stall (nothing here ever waits out the 30 seconds: a stall
/// would leave the file unfinished). Each piece is asked for once, plus once more per bad copy,
/// and every second ask is `count` 1; no `get` repeats the rest of a window.
/// Seen red 2026-10-12 with `piece_failed` only forgetting the bad piece (no `get`, as a receiver
/// that leaves it to the stall): "every piece in (lag 0)" failed, the conversation stopping with
/// pieces missing. Also red with `pump` asking from the first missing piece again every time
/// (`self.asked.clear()` at its top, repeating the rest of the window): "piece 1 asked once, plus
/// once per bad copy (lag 0)" counted 2 asks.
#[test]
fn a_rule_one_sender_and_this_receiver_finish_without_a_stall() {
    for lag in [0usize, 3, 7, 40, 100] {
        let (bytes, offer, hashes) = file(PIECE_SIZE as usize * 40 + 1000);
        let now = Instant::now();
        let mut rx = Receiving::new(offer.clone(), "Ann", true, now);
        let mut tx = RuleOneSender::new(&bytes, &hashes);
        tx.damage.insert(3, 1);
        tx.damage.insert(17, 2);
        tx.damage.insert(40, 1);
        let mut part = Vec::new();
        let mut events = Vec::new();
        let mut said: Vec<Out> = Vec::new();
        // What the receiver said, and after how many data messages it reaches the sender.
        let mut to_tx: std::collections::VecDeque<(usize, Out)> = std::collections::VecDeque::new();
        for m in rx.open(now).out {
            said.push(m.clone());
            to_tx.push_back((0, m));
        }
        let mut sent = 0usize;
        loop {
            while to_tx.front().is_some_and(|(due, _)| *due <= sent) {
                let (_, m) = to_tx.pop_front().unwrap();
                tx.hear(&m);
            }
            let Some(data) = tx.next() else {
                // Nothing owed: what is still on its way reaches the sender now, or it is over.
                match to_tx.pop_front() {
                    Some((_, m)) => {
                        tx.hear(&m);
                        continue;
                    }
                    None => break,
                }
            };
            sent += 1;
            let step = rx.on_binary(&data, &mut part, now);
            events.extend(step.events);
            for m in step.out {
                said.push(m.clone());
                to_tx.push_back((sent + lag, m));
            }
        }
        assert!(events.contains(&Event::Complete), "every piece in (lag {lag})");
        assert_eq!(part, bytes, "the part file holds the file (lag {lag})");
        let asks = gets(&said);
        let times = |p: u64| asks.iter().filter(|(f, n)| *f <= p && p < f + n).count();
        for p in 0..offer.pieces {
            let bad = match p {
                3 | 40 => 1,
                17 => 2,
                _ => 0,
            };
            assert_eq!(times(p), 1 + bad, "piece {p} asked once, plus once per bad copy (lag {lag}): {asks:?}");
        }
        let mut seen = std::collections::HashSet::new();
        for (from, n) in &asks {
            if (*from..from + n).any(|p| seen.contains(&p)) {
                assert_eq!(*n, 1, "asked again: one piece alone (lag {lag}): {asks:?}");
            }
            seen.extend(*from..from + n);
        }
    }
}

/// RULE 1, THE SENDER: a piece asked for while it is being sent goes again, whole, after the
/// current one; a `get` adds (pieces already owed are sent once); a piece already sent and asked
/// again goes again; what is owed goes lowest first.
/// Seen red 2026-10-12 with a `get` skipping the piece being sent (as before the review):
/// "piece 0 again, whole, after the current one" got [0, 1, 2] where [0, 0, 1, 2] was expected.
#[test]
fn a_piece_asked_again_while_it_is_being_sent_goes_again_whole() {
    let (bytes, offer, hashes) = file(PIECE_SIZE as usize * 6);
    let mut src = bytes.clone();
    let mut s = Sending::new(offer, Some(std::sync::Arc::new(hashes)));
    s.open();
    let mut sent: Vec<(u64, u64)> = Vec::new();
    let mut take = |s: &mut Sending, n: usize, sent: &mut Vec<(u64, u64)>| {
        for _ in 0..n {
            match s.next(&mut src, 0).unwrap() {
                Some((Out::Binary(m), _)) => sent.push(header(&m)),
                other => panic!("a data message: {other:?}"),
            }
        }
    };
    s.on_text("{\"t\":\"get\",\"from\":0,\"count\":3}");
    take(&mut s, 2, &mut sent); // piece 0 is going out
    s.on_text("{\"t\":\"get\",\"from\":0,\"count\":1}"); // the receiver asks for it again meanwhile
    s.on_text("{\"t\":\"get\",\"from\":1,\"count\":2}"); // and for 1 and 2, owed already
    while let Some((m, _)) = s.next(&mut src, 0).unwrap() {
        let Out::Binary(m) = m else { panic!("data only") };
        sent.push(header(&m));
    }
    let starts: Vec<u64> = sent.iter().filter(|(_, at)| *at == 0).map(|(p, _)| *p).collect();
    assert_eq!(starts, vec![0, 0, 1, 2], "piece 0 again, whole, after the current one; 1 and 2 once each");
    let whole = |from: usize| sent[from..from + 5].iter().map(|(_, at)| *at).collect::<Vec<u64>>();
    let offsets: Vec<u64> = (0..5).map(|i| i * 65_528).collect();
    assert_eq!(whole(0), offsets, "the first copy of piece 0 finished");
    assert_eq!(whole(5), offsets, "the second, from offset 0");
    assert_eq!(sent.len(), 20, "four pieces, five messages each");

    // A piece already sent, asked again, goes again; what is owed goes lowest first.
    s.on_text("{\"t\":\"get\",\"from\":4,\"count\":2}");
    s.on_text("{\"t\":\"get\",\"from\":1,\"count\":1}");
    let mut starts = Vec::new();
    while let Some((Out::Binary(m), _)) = s.next(&mut src, 0).unwrap() {
        if header(&m).1 == 0 {
            starts.push(header(&m).0);
        }
    }
    assert_eq!(starts, vec![1, 4, 5], "lowest first, each once");
}

/// A stand-in receiver with the WEBSITE'S PATTERN (web/shared/file-transfer.js `Receiver`, as it
/// was when the first review ran): it takes pieces strictly in order, asks for a window of 16 from
/// the next piece it needs, asks the next window when this one is in, ignores anything that is not
/// the next piece, and after a bad piece asks `get(next, 16)` again.
struct WebStyleReceiver {
    offer: Offer,
    hashes: Option<Vec<[u8; 32]>>,
    list: Vec<u8>,
    next: u64,
    window_end: Option<u64>,
    fill: Vec<u8>,
    part: Vec<u8>,
    failures: u32,
    done: bool,
}

impl WebStyleReceiver {
    fn new(offer: &Offer) -> Self {
        WebStyleReceiver { offer: offer.clone(), hashes: None, list: Vec::new(), next: 0, window_end: None, fill: Vec::new(), part: Vec::new(), failures: 0, done: false }
    }

    fn ask(&mut self) -> Vec<Out> {
        let count = WINDOW.min(self.offer.pieces - self.next);
        self.window_end = Some(self.next + count);
        vec![get_message(self.next, count)]
    }

    fn receive(&mut self, data: &[u8]) -> Vec<Out> {
        if self.hashes.is_none() {
            self.list.extend_from_slice(data);
            if self.list.len() < self.offer.pieces as usize * 32 {
                return Vec::new();
            }
            self.hashes = Some(self.list.chunks_exact(32).map(|c| c.try_into().unwrap()).collect());
            return self.ask();
        }
        let (piece, offset) = header(data);
        if self.window_end.is_none() || piece != self.next || (self.fill.is_empty() && offset != 0) {
            return Vec::new(); // not asked for, or left over from before a new ask
        }
        let len = piece_len(self.offer.size, piece) as usize;
        if offset as usize != self.fill.len() || self.fill.len() + data.len() - 8 > len {
            self.failures += 1;
            self.fill.clear();
            return self.ask();
        }
        self.fill.extend_from_slice(&data[8..]);
        if self.fill.len() < len {
            return Vec::new();
        }
        if blake3::hash(&self.fill).as_bytes() != &self.hashes.as_ref().unwrap()[piece as usize] {
            self.failures += 1;
            self.fill.clear();
            return self.ask(); // get(next, 16) after a bad piece
        }
        self.part.extend_from_slice(&self.fill);
        self.fill.clear();
        self.next += 1;
        if self.next == self.offer.pieces {
            self.done = true;
            return vec![say("done")];
        }
        if self.window_end.is_some_and(|end| self.next >= end) {
            return self.ask();
        }
        Vec::new()
    }
}

/// RULE 1 WITH THE WEBSITE'S RECEIVER: after a bad piece the website asks `get(next, 16)` while the
/// desktop sender is further on, mid-way through a later piece. A sender whose `get` adds sends the
/// bad piece again, then each piece after it in order, so the website's in-order receiver finishes
/// without a stall (no timer runs here: a stall would leave it unfinished).
/// Seen red 2026-10-12 with a `get` skipping the piece being sent (as before the review): "the
/// website's receiver finished (lag 7): at piece 7" failed, the receiver waiting for piece 7, the
/// one being sent when it asked, which never came again.
#[test]
fn the_websites_receiver_pattern_gets_a_bad_piece_again_without_a_stall() {
    for lag in [7usize, 12, 30] {
        let (bytes, offer, hashes) = file(PIECE_SIZE as usize * 40 + 3);
        let mut src = bytes.clone();
        let mut s = Sending::new(offer.clone(), Some(std::sync::Arc::new(hashes)));
        s.open();
        let mut rx = WebStyleReceiver::new(&offer);
        let mut to_tx: std::collections::VecDeque<(usize, Out)> = std::collections::VecDeque::new();
        to_tx.push_back((0, say("hashes")));
        let mut damaged = false;
        let mut sent = 0usize;
        let mut asks = Vec::new();
        loop {
            while to_tx.front().is_some_and(|(due, _)| *due <= sent) {
                let (_, Out::Text(t)) = to_tx.pop_front().unwrap() else { panic!("text") };
                asks.push(Out::Text(t.clone()));
                s.on_text(&t);
            }
            let Some((m, _)) = s.next(&mut src, 0).unwrap() else {
                match to_tx.pop_front() {
                    Some((_, Out::Text(t))) => {
                        asks.push(Out::Text(t.clone()));
                        s.on_text(&t);
                        continue;
                    }
                    _ => break,
                }
            };
            let Out::Binary(mut m) = m else { panic!("data only") };
            sent += 1;
            // Piece 5's first copy is damaged on the way.
            if !damaged && m.len() > 8 && header(&m) == (5, 0) {
                damaged = true;
                m[8] ^= 0xff;
            }
            for o in rx.receive(&m) {
                to_tx.push_back((sent + lag, o));
            }
        }
        assert!(rx.done, "the website's receiver finished (lag {lag}): at piece {}", rx.next);
        assert_eq!(rx.part, bytes, "with the whole file (lag {lag})");
        assert_eq!(rx.failures, 1, "the one bad copy");
        assert!(gets(&asks).contains(&(5, 16)), "it asked get(next, 16) after the bad piece (lag {lag})");
    }
}

/// RULE 1, MEMORY: a message for a piece the receiver did not ask for, or already holds, is
/// ignored and reserves nothing; partial buffers are held only for pieces asked for, so at most
/// the window's 16, whatever piece numbers a sender names.
/// Seen red 2026-10-12 with the receiver reserving a buffer for any piece at offset 0 (the
/// `asked` check taken out of `on_binary`, as before the review): "nothing reserved for pieces not
/// asked (piece 16)" found 262,144 bytes after the first message.
#[test]
fn pieces_not_asked_for_are_ignored_and_reserve_nothing() {
    let many: Vec<[u8; 32]> = (0..3000u32).map(|i| *blake3::hash(&i.to_be_bytes()).as_bytes()).collect();
    let (_, small, _) = file(PIECE_SIZE as usize);
    let offer = Offer { size: 3000 * PIECE_SIZE, pieces: 3000, hashes: hashes_digest(&many), ..small };
    let now = Instant::now();
    let mut rx = Receiving::new(offer, "Ann", true, now);
    rx.open(now);
    let mut part = Vec::new();
    let step = rx.on_binary(&many.concat(), &mut part, now);
    assert_eq!(gets(&step.out), vec![(0, 16)]);
    let start = |p: u32| {
        let mut m = Vec::with_capacity(8 + 100);
        m.extend_from_slice(&p.to_be_bytes());
        m.extend_from_slice(&0u32.to_be_bytes());
        m.extend_from_slice(&[7u8; 100]);
        m
    };
    for p in 16..3000u32 {
        let step = rx.on_binary(&start(p), &mut part, now);
        assert_eq!(step, Step::default(), "piece {p}, not asked for: ignored");
        assert_eq!(rx.partial_bytes(), 0, "nothing reserved for pieces not asked (piece {p})");
    }
    for p in 0..16u32 {
        rx.on_binary(&start(p), &mut part, now);
    }
    assert!(rx.partial_bytes() <= 16 * PIECE_SIZE as usize, "at most the window's 16 partial pieces: {}", rx.partial_bytes());
    assert_eq!(rx.asked(), (0..16).collect::<Vec<u64>>(), "and nothing asked because of them");

    // A piece already held: ignored too.
    let (bytes, offer, hashes) = file(PIECE_SIZE as usize * 2);
    let mut rx = Receiving::new(offer, "Ann", true, now);
    rx.open(now);
    let mut part = Vec::new();
    rx.on_binary(&hashes.concat(), &mut part, now);
    let mut tx = StandInSender::new(&bytes, &hashes);
    for m in tx.answer(&get_message(0, 1)) {
        rx.on_binary(&m, &mut part, now);
    }
    assert_eq!(rx.have(), 1);
    for m in tx.answer(&get_message(0, 1)) {
        assert_eq!(rx.on_binary(&m, &mut part, now), Step::default(), "piece 0 again: held already");
    }
    assert_eq!(rx.partial_bytes(), 0);
}

/// RULE 1, A STALL: when nothing asked for has arrived for 30 seconds, the receiver asks again
/// (and only then: before 30 seconds, and while paused, it does not). Messages for pieces it did
/// not ask for do not count as something arriving.
/// Seen red 2026-10-12 with the stall clock moved by every data message (asked for or not, as
/// before the first review): "a sender naming other pieces does not hold off the stall" got
/// nothing asked where (0, 16) was expected.
#[test]
fn a_stall_asks_again_and_only_a_stall() {
    let (bytes, offer, hashes) = file(PIECE_SIZE as usize * 20);
    let now = Instant::now();
    let mut rx = Receiving::new(offer, "Ann", true, now);
    rx.open(now);
    let mut part = Vec::new();
    rx.on_binary(&hashes.concat(), &mut part, now);
    assert!(gets(&rx.tick(&mut part, now + STALL / 2).out).is_empty(), "not before 30 seconds");
    // A sender naming a piece not asked for, 20 seconds in: no help.
    let mut tx = StandInSender::new(&bytes, &hashes);
    for m in tx.answer(&get_message(18, 1)) {
        rx.on_binary(&m, &mut part, now + STALL * 2 / 3);
    }
    let step = rx.tick(&mut part, now + STALL);
    assert_eq!(gets(&step.out), vec![(0, 16)], "a sender naming other pieces does not hold off the stall");
    assert!(gets(&rx.tick(&mut part, now + STALL + STALL / 2).out).is_empty(), "and the clock starts again");
}
