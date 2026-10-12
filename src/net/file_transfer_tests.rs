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
/// nothing more until Resume, which says `resume` and asks again from the first missing piece; the
/// sender's pause says `pause`, stops what it sends, and the receiver, told so, asks for nothing
/// until it hears `resume`.
/// Seen red 2026-10-11 with `Sending::next` ignoring `paused_me`: "a paused sender sends nothing"
/// got a data message.
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
    assert!(rx.asked().is_empty(), "what was asked is forgotten");
    for m in tx.answer(&get_message(0, 2)) {
        rx.on_binary(&m, &mut part, now); // what was already on its way still lands
    }
    assert_eq!(rx.have(), 2);
    assert!(gets(&rx.tick(&mut part, now).out).is_empty(), "nothing asked while paused");
    let out = rx.resume();
    assert_eq!(out.first(), Some(&Out::Text("{\"t\":\"resume\"}".into())));
    assert_eq!(gets(&out), vec![(2, 16)], "asked again from the first missing piece");

    // The receiver told the sender paused.
    let step = rx.on_text("{\"t\":\"pause\"}");
    assert_eq!(step.events, vec![Event::PausedByThem(true)]);
    assert!(rx.asked().is_empty());
    let step = rx.on_text("{\"t\":\"resume\"}");
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
    while s.next(&mut src, 0).unwrap().is_some() {} // what was asked while paused goes now
    assert_eq!(s.next(&mut src, 0).unwrap(), None);
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
    assert_eq!(rx.resume(), Vec::<Out>::new(), "nothing after");

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
    assert_eq!(rx.on_text("{\"t\":\"cancel\"}").events, vec![Event::Ended(End::Cancelled { by_them: true })], "the receiver hears the sender's cancel");
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
