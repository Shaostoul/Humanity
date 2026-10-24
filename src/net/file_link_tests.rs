// Child of net/file_link.rs (#[path]): transfers end to end on this machine, without the game.

use super::*;
use crate::net::file_offer::{hashes_digest, pieces_for, FilePass, Offer, PIECE_SIZE};
use crate::net::file_transfer::{stopped_line, End, Receiving, Sending};
use crate::net::webrtc::test_forwarder::{Forwarder, Trap};

const X: &str = "a0112233445566778899aabbccddeeff";
const Y: &str = "b0112233445566778899aabbccddeeff";
const Z: &str = "c0112233445566778899aabbccddeeff";

/// One file on disk (`len` bytes, a pattern of its own per `seed`) and its offer.
fn file(dir: &std::path::Path, id: &str, len: usize, seed: usize) -> (PathBuf, Vec<u8>, Offer) {
    let bytes: Vec<u8> = (0..len).map(|i| ((i * 13 + seed) % 241) as u8).collect();
    let path = dir.join(format!("{id}.src"));
    std::fs::write(&path, &bytes).unwrap();
    let hashes: Vec<[u8; 32]> = bytes.chunks(PIECE_SIZE as usize).map(|c| *blake3::hash(c).as_bytes()).collect();
    let offer = Offer {
        id: id.into(),
        name: format!("{id}.mp4"),
        size: len as u64,
        mime: "video/mp4".into(),
        blake3: blake3::hash(&bytes).to_hex().to_string(),
        piece: PIECE_SIZE,
        pieces: pieces_for(len as u64),
        hashes: hashes_digest(&hashes),
        pass: FilePass { v: 1, id: id.into(), expiry: 4_000_000_000, sig: format!("sig-{id}") },
    };
    (path, bytes, offer)
}

/// A receiving transfer of `offer`, its part file made at full size in `dir`; `paused` as if
/// paused here before it started.
fn receive_as(dir: &std::path::Path, offer: &Offer, sender: &str, paused: bool) -> Transfer {
    let part = dir.join(format!("{}.{}.part", offer.name, &offer.id[..12]));
    std::fs::OpenOptions::new().write(true).create(true).truncate(true).open(&part).unwrap().set_len(offer.size).unwrap();
    let mut proto = Receiving::new(offer.clone(), sender, true, Instant::now());
    proto.set_paused(paused);
    Transfer { id: offer.id.clone(), side: Side::Receive { proto, part, dir: dir.to_path_buf(), name: offer.name.clone(), sender: sender.into() } }
}

fn receive(dir: &std::path::Path, offer: &Offer, sender: &str) -> Transfer {
    receive_as(dir, offer, sender, false)
}

/// A sending transfer of `offer` to `receiver` (their name); `paused` as if paused here before.
fn send_as(path: &std::path::Path, offer: &Offer, receiver: &str, paused: bool) -> Transfer {
    let mut proto = Sending::new(offer.clone(), None);
    proto.set_paused(paused);
    Transfer { id: offer.id.clone(), side: Side::Send { proto, path: path.to_path_buf(), receiver: receiver.into() } }
}

fn send(path: &std::path::Path, offer: &Offer) -> Transfer {
    send_as(path, offer, "Rae", false)
}

/// ONE CONNECTION PER PAIR, ONE ALLOCATION EACH SIDE, ONLY THROUGH THE FORWARDER (parts 4 and 5,
/// and rule 7 of the second review): Sam offered Rae two files (X and Y) and Rae offered Sam one
/// (Z). Rae dials Sam with X's credentials; her `dc_offer` carries X's pass. The dial names X (the
/// relay's doing), so Sam answers with X's credentials alone: one allocation on one socket, one
/// relayed candidate each side, and the forwarder joins the two in X's scope. On the one
/// connection each receiver opens its transfer's channel (Rae X and Y, Sam Z, on the connection
/// Rae dialled), the sender tells them apart by the label, and all three files arrive whole, each
/// named only once its whole BLAKE3 matched: Y and Z need no allocation of their own. Every
/// candidate is a relayed address the forwarder gave out, and those addresses are `Trap` sockets:
/// anything sent around the forwarder is caught.
/// Seen red 2026-10-11 (two allocations on Sam's side then) with `drain` sending each transmit
/// through the last allocation whatever its source. Seen red 2026-10-12, one allocation each side,
/// with `connect` adding the socket's own address as a host candidate beside the relayed one:
/// "the dialler: one relayed address" found two candidates.
#[test]
fn a_pair_s_files_cross_one_connection_only_through_the_forwarder() {
    let dir = std::env::temp_dir().join(format!("hum_file_link_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (sam_dir, rae_dir) = (dir.join("sam"), dir.join("rae"));
    std::fs::create_dir_all(&sam_dir).unwrap();
    std::fs::create_dir_all(&rae_dir).unwrap();
    let (px, bx, ox) = file(&sam_dir, X, PIECE_SIZE as usize * 2 + 70_000, 1);
    let (py, by, oy) = file(&sam_dir, Y, 300_000, 2);
    let (pz, bz, oz) = file(&rae_dir, Z, 400_000, 3);

    // Two relayed addresses in all: one allocation each side.
    let traps: Vec<Trap> = (0..2).map(|_| Trap::bind()).collect();
    let fwd = Forwarder::start(traps.iter().map(|t| t.addr).collect());
    fwd.add_account("1:rx", "prx", &format!("file:{X}"));
    fwd.add_account("1:sx", "psx", &format!("file:{X}"));
    let relay = |user: &str, cred: &str| Relay { server: fwd.addr.to_string(), username: user.into(), credential: cred.into() };

    let rae = FileLink::start(Setup {
        me: "rr".into(),
        peer: "ss".into(),
        dial_pass: Some(ox.pass.clone()),
        their_offer: None,
        relay: relay("1:rx", "prx"),
        transfers: vec![receive(&rae_dir, &ox, "Sam"), receive(&rae_dir, &oy, "Sam"), send(&pz, &oz)],
        bind: "127.0.0.1:0".into(),
        early_ice: Vec::new(),
    });

    let (mut er, mut es) = (Vec::new(), Vec::new());
    let mut sam: Option<FileLink> = None;
    let mut sdps = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(60);
    let saved = |ev: &[LinkEvent], id: &str| ev.iter().any(|e| matches!(e, LinkEvent::Saved(i, _) if i == id));
    let done = |ev: &[LinkEvent], id: &str| ev.contains(&LinkEvent::Ended(id.into(), End::Done));
    while Instant::now() < deadline && !(saved(&er, X) && saved(&er, Y) && saved(&es, Z) && done(&es, X) && done(&es, Y) && done(&er, Z)) {
        for ev in rae.poll() {
            if let LinkEvent::Signal(frame) = &ev {
                let v: Value = serde_json::from_str(frame).unwrap();
                assert_eq!((v["type"].as_str(), v["to"].as_str()), (Some("webrtc_signal"), Some("ss")));
                let inner: Value = serde_json::from_str(v["data"].as_str().expect("data is a JSON string")).unwrap();
                assert!(inner.get("file").is_none(), "the transfer is named by the relay, never inside the signal");
                if let Some(sdp) = inner["sdp"].as_str() {
                    sdps.push(sdp.to_string());
                }
                if v["signal_type"] == "dc_offer" {
                    assert_eq!(v["file_pass"], serde_json::to_value(&ox.pass).unwrap(), "the dialler's offer carries the pass of the credentials it used");
                    sam = Some(FileLink::start(Setup {
                        me: "ss".into(),
                        peer: "rr".into(),
                        dial_pass: None,
                        their_offer: Some(signal_text(&v["data"])),
                        relay: relay("1:sx", "psx"),
                        transfers: vec![send(&px, &ox), send(&py, &oy), receive(&sam_dir, &oz, "Rae")],
                        bind: "127.0.0.1:0".into(),
                        early_ice: Vec::new(),
                    }));
                } else if let Some(s) = &sam {
                    s.send(Cmd::Signal { signal_type: v["signal_type"].as_str().unwrap().into(), data: signal_text(&v["data"]) });
                }
            }
            er.push(ev);
        }
        if let Some(s) = &sam {
            for ev in s.poll() {
                if let LinkEvent::Signal(frame) = &ev {
                    let v: Value = serde_json::from_str(frame).unwrap();
                    assert!(v.get("file_pass").is_none(), "only the dialler's offer carries a pass");
                    let inner: Value = serde_json::from_str(v["data"].as_str().unwrap()).unwrap();
                    if let Some(sdp) = inner["sdp"].as_str() {
                        sdps.push(sdp.to_string());
                    }
                    rae.send(Cmd::Signal { signal_type: v["signal_type"].as_str().unwrap().into(), data: signal_text(&v["data"]) });
                }
                es.push(ev);
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }

    assert!(saved(&er, X) && saved(&er, Y), "Rae saved all: {er:?} / {es:?}");
    assert!(saved(&es, Z), "Sam saved his: {es:?}");
    assert_eq!(std::fs::read(rae_dir.join(&ox.name)).unwrap(), bx, "X arrived whole");
    assert_eq!(std::fs::read(rae_dir.join(&oy.name)).unwrap(), by, "Y arrived whole");
    assert_eq!(std::fs::read(sam_dir.join(&oz.name)).unwrap(), bz, "Z, the other way, arrived whole");
    for id in [X, Y] {
        assert!(er.contains(&LinkEvent::Ended(id.into(), End::Done)) && es.contains(&LinkEvent::Ended(id.into(), End::Done)), "{id}: both ends say done");
    }
    assert!(er.contains(&LinkEvent::Progress(X.into(), ox.pieces)), "every piece counted");
    assert_eq!(sdps.len(), 2, "one offer and one answer for the three files: {sdps:?}");
    let candidates = |sdp: &str| sdp.lines().filter(|l| l.starts_with("a=candidate:")).map(str::to_string).collect::<Vec<_>>();
    assert_eq!(candidates(&sdps[0]).len(), 1, "the dialler: one relayed address");
    assert_eq!(candidates(&sdps[1]).len(), 1, "the answerer: one relayed address");
    for sdp in &sdps {
        for line in candidates(sdp) {
            assert!(traps.iter().any(|t| line.contains(&format!(" {} typ relay", t.addr.port()))), "a relayed address: {line}");
            // (The test forwarder's relayed addresses are 127.0.0.1 sockets too, so the address
            // itself cannot tell; the related address is where a device's own would show.)
            assert!(line.contains(" typ relay raddr 0.0.0.0 rport 0"), "nothing of either device's address: {line}");
        }
    }
    assert!(fwd.delivered() > 0);
    for t in &traps {
        assert!(t.received().is_empty(), "nothing went straight to {}", t.addr);
    }
    drop(sam);
    drop(rae);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Two connections on this machine through `fwd`: Rae dials with `rae_transfers`, Sam answers
/// with `sam_transfers`, each with one credential in X's scope; their signals are carried between
/// them as the relay would carry them, and what each reports is kept with the moment it came.
struct TwoApps {
    rae: FileLink,
    sam: Option<FileLink>,
    sam_setup: Option<(Relay, Vec<Transfer>)>,
    er: Vec<(Instant, LinkEvent)>,
    es: Vec<(Instant, LinkEvent)>,
}

impl TwoApps {
    fn start(fwd: &Forwarder, rae_transfers: Vec<Transfer>, sam_transfers: Vec<Transfer>, pass: &FilePass) -> TwoApps {
        fwd.add_account("1:r", "pr", &format!("file:{X}"));
        fwd.add_account("1:s", "ps", &format!("file:{X}"));
        let relay = |user: &str, cred: &str| Relay { server: fwd.addr.to_string(), username: user.into(), credential: cred.into() };
        let rae = FileLink::start(Setup {
            me: "rr".into(),
            peer: "ss".into(),
            dial_pass: Some(pass.clone()),
            their_offer: None,
            relay: relay("1:r", "pr"),
            transfers: rae_transfers,
            bind: "127.0.0.1:0".into(),
            early_ice: Vec::new(),
        });
        TwoApps { rae, sam: None, sam_setup: Some((relay("1:s", "ps"), sam_transfers)), er: Vec::new(), es: Vec::new() }
    }

    /// One turn: each side's news taken, its signals handed to the other.
    fn turn(&mut self) {
        for ev in self.rae.poll() {
            if let LinkEvent::Signal(frame) = &ev {
                let v: Value = serde_json::from_str(frame).unwrap();
                let (kind, data) = (v["signal_type"].as_str().unwrap().to_string(), signal_text(&v["data"]));
                if kind == "dc_offer" {
                    if let Some((relay, transfers)) = self.sam_setup.take() {
                        self.sam = Some(FileLink::start(Setup { me: "ss".into(), peer: "rr".into(), dial_pass: None, their_offer: Some(data), relay, transfers, bind: "127.0.0.1:0".into(), early_ice: Vec::new() }));
                    }
                } else if let Some(s) = &self.sam {
                    s.send(Cmd::Signal { signal_type: kind, data });
                }
            }
            self.er.push((Instant::now(), ev));
        }
        if let Some(s) = &self.sam {
            for ev in s.poll() {
                if let LinkEvent::Signal(frame) = &ev {
                    let v: Value = serde_json::from_str(frame).unwrap();
                    self.rae.send(Cmd::Signal { signal_type: v["signal_type"].as_str().unwrap().into(), data: signal_text(&v["data"]) });
                }
                self.es.push((Instant::now(), ev));
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }

    /// Turns until `done(rae's news, sam's news)` or `secs` go by; whether it came.
    fn until(&mut self, secs: u64, done: impl Fn(&[LinkEvent], &[LinkEvent]) -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(secs);
        while Instant::now() < deadline {
            self.turn();
            if done(&self.rae_news(), &self.sam_news()) {
                return true;
            }
        }
        false
    }

    fn rae_news(&self) -> Vec<LinkEvent> {
        self.er.iter().map(|(_, e)| e.clone()).collect()
    }

    fn sam_news(&self) -> Vec<LinkEvent> {
        self.es.iter().map(|(_, e)| e.clone()).collect()
    }

    /// When `ev` first came from a side's news, if it did.
    fn when(news: &[(Instant, LinkEvent)], ev: &LinkEvent) -> Option<Instant> {
        news.iter().find(|(_, e)| e == ev).map(|(t, _)| *t)
    }
}

/// RULE 8 OF THE SECOND REVIEW, THE SENDER, ON A REAL CONNECTION: a channel the receiver closes
/// before `done`, without a word, ends a sending transfer with words ("Rae's side stopped it") once
/// [`STOPPED_WAIT`] (15 seconds, as the website) has gone by with the connection still there (ICE
/// checks the path every 3 seconds at most, so something comes from Rae) and the channel not opened
/// again; never at the close itself. Sam hears `TransferClosed` at once and the transfer's end
/// after. Rae's channel is closed without a word here (str0m tells the other side nothing of its
/// own close, so Sam's half of it is `TheyClosed`, what str0m reports when a browser closes one).
/// Seen red 2026-10-12 with `channel_closed` not marking a sending transfer (`closed_by_them` left
/// None, as before the review: the leg only forgot its channel): "Sam's side ended X with words"
/// failed after 40 seconds, Sam's news ending at `TransferClosed`.
#[test]
fn a_channel_the_other_side_closes_ends_a_sending_transfer_with_words() {
    let dir = std::env::temp_dir().join(format!("hum_file_link_stopped_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (px, _, ox) = file(&dir, X, PIECE_SIZE as usize * 3, 4);
    let traps: Vec<Trap> = (0..2).map(|_| Trap::bind()).collect();
    let fwd = Forwarder::start(traps.iter().map(|t| t.addr).collect());
    // Rae starts paused, so nothing flows while the channel is open.
    let mut apps = TwoApps::start(&fwd, vec![receive_as(&dir, &ox, "Sam", true)], vec![send_as(&px, &ox, "Rae", false)], &ox.pass);
    let open = LinkEvent::TransferOpen(X.into());
    assert!(apps.until(30, |r, s| r.contains(&open) && s.contains(&open)), "X's channel opened: {:?} / {:?}", apps.rae_news(), apps.sam_news());

    let closed_at = Instant::now();
    apps.rae.send(Cmd::CloseChannel(X.into()));
    apps.sam.as_ref().unwrap().send(Cmd::TheyClosed(X.into()));
    let stopped = LinkEvent::Ended(X.into(), End::Failed(stopped_line("Rae")));
    assert!(apps.until(40, |_, s| s.contains(&stopped)), "Sam's side ended X with words: {:?}", apps.sam_news());
    let said_closed = TwoApps::when(&apps.es, &LinkEvent::TransferClosed(X.into())).expect("Sam heard the close at once");
    let ended = TwoApps::when(&apps.es, &stopped).unwrap();
    assert!(said_closed <= ended, "the close is told before the end");
    assert!(ended.duration_since(closed_at) >= STOPPED_WAIT, "not at the close itself: only after the wait");
    let sam_news = apps.sam_news();
    let before_end = &sam_news[..sam_news.iter().position(|e| *e == stopped).unwrap()];
    assert!(!before_end.contains(&LinkEvent::Closed), "the connection did not drop first");
    assert!(!apps.rae_news().iter().any(|e| matches!(e, LinkEvent::Ended(..))), "Rae's side ended nothing itself");
    drop(apps);
    let _ = std::fs::remove_dir_all(&dir);
}

/// RULE 8 OF THE SECOND REVIEW, THE RECEIVER: a channel the sender closes before `done` is opened
/// again by the receiver on the same connection after a short wait ([`REOPEN_AFTER`]), not when the
/// whole connection drops, and a pause the sender held is cleared with it. Sam starts paused (Rae
/// hears his pause), closes X's channel without a word (as a website sender does with a channel
/// it will not serve) and resumes while it is closed; Rae hears `TransferClosed`, opens X again
/// two seconds later, and the file comes whole on the one connection: her news holds X opened
/// twice and no `Closed` before it is saved.
/// Seen red 2026-10-12 with `channel_closed` not setting `reopen_at` (as before the review):
/// "Rae saved X on the same connection" failed after 30 seconds, X opened once. Seen red the same
/// day with a channel str0m closed before it opened not asked for again (`Link::opening` ignored):
/// the same failure, since here the reopened channel took the stream id the association still held
/// (the two sides' closes are str0m's own, which reset nothing) and str0m closed it unopened.
#[test]
fn a_receiving_transfer_opens_its_channel_again_when_the_other_side_closed_it() {
    let dir = std::env::temp_dir().join(format!("hum_file_link_reopen_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (sam_dir, rae_dir) = (dir.join("sam"), dir.join("rae"));
    std::fs::create_dir_all(&sam_dir).unwrap();
    std::fs::create_dir_all(&rae_dir).unwrap();
    let (px, bx, ox) = file(&sam_dir, X, PIECE_SIZE as usize * 2 + 9_000, 5);
    let traps: Vec<Trap> = (0..2).map(|_| Trap::bind()).collect();
    let fwd = Forwarder::start(traps.iter().map(|t| t.addr).collect());
    let mut apps = TwoApps::start(&fwd, vec![receive(&rae_dir, &ox, "Sam")], vec![send_as(&px, &ox, "Rae", true)], &ox.pass);
    let paused = LinkEvent::PausedByThem(X.into(), true);
    assert!(apps.until(30, |r, _| r.contains(&paused)), "Sam's pause reached Rae: {:?} / {:?}", apps.rae_news(), apps.sam_news());

    let closed_at = Instant::now();
    let sam = apps.sam.as_ref().unwrap();
    sam.send(Cmd::CloseChannel(X.into()));
    sam.send(Cmd::Resume(X.into()));
    apps.rae.send(Cmd::TheyClosed(X.into()));
    let saved = |r: &[LinkEvent]| r.iter().any(|e| matches!(e, LinkEvent::Saved(i, _) if i == X));
    let done = LinkEvent::Ended(X.into(), End::Done);
    assert!(apps.until(30, |r, s| saved(r) && s.contains(&done)), "Rae saved X on the same connection: {:?} / {:?}", apps.rae_news(), apps.sam_news());
    assert_eq!(std::fs::read(rae_dir.join(&ox.name)).unwrap(), bx, "X arrived whole");

    let rae_news = apps.rae_news();
    let open = LinkEvent::TransferOpen(X.into());
    assert_eq!(rae_news.iter().filter(|e| **e == open).count(), 2, "X opened twice: {rae_news:?}");
    let at = |ev: &LinkEvent| rae_news.iter().position(|e| e == ev).unwrap();
    assert!(at(&paused) < at(&LinkEvent::TransferClosed(X.into())), "the close told after the pause it clears");
    let reopened = apps.er.iter().filter(|(_, e)| *e == open).nth(1).map(|(t, _)| *t).unwrap();
    assert!(reopened.duration_since(closed_at) >= REOPEN_AFTER, "after the short wait");
    let saved_at = rae_news.iter().position(|e| matches!(e, LinkEvent::Saved(..))).unwrap();
    assert!(!rae_news[..saved_at].contains(&LinkEvent::Closed), "on the connection that stood: {rae_news:?}");
    drop(apps);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Rule 8's two clocks, as rules. A sending transfer whose channel the other side closed counts as
/// stopped by them only once 15 seconds have gone by AND something came from them a second or more
/// after the close (the connection is still there): before the wait it is not yet, and with
/// nothing heard, or only a datagram at the close itself (the close's own exchange), it is a drop.
/// A receiving transfer waits two seconds before opening its channel again, doubling each time it
/// is closed again before a piece comes, at most a minute.
/// Seen red 2026-10-12 with `stop_due` leaving out `they_stopped` (the wait alone): "nothing heard
/// after the wait: a drop" failed. Seen red the same day with `they_stopped` taking any datagram
/// after the close (no [`STOP_PROOF`]): "a datagram at the close proves nothing" failed.
#[test]
fn a_stop_needs_the_wait_and_them_still_there_and_reopening_backs_off() {
    let t = Instant::now();
    let later = |s: u64| t + Duration::from_secs(s);
    assert!(!they_stopped(t, None), "nothing heard: the connection went");
    assert!(!they_stopped(t, Some(t)), "a datagram at the close proves nothing");
    assert!(!they_stopped(t, Some(t + Duration::from_millis(400))));
    assert!(they_stopped(t, Some(t + STOP_PROOF)), "something a second later: they are there");
    assert!(!they_stopped(later(5), Some(t)), "heard before the close only");

    assert!(!stop_due(t, Some(later(3)), later(14)), "still within the wait");
    assert!(stop_due(t, Some(later(3)), later(15)), "the wait over, them still there: stopped");
    assert!(!stop_due(t, None, later(15)), "nothing heard after the wait: a drop");
    assert!(!stop_due(t, Some(t), later(60)), "only the close's own exchange heard: a drop");
    assert_eq!(STOPPED_WAIT, Duration::from_secs(15), "the website's wait");

    let waits: Vec<u64> = (0..8).map(|n| reopen_wait(n).as_secs()).collect();
    assert_eq!(waits, vec![2, 4, 8, 16, 32, 60, 60, 60]);
}

/// A connection with no thread and no network (its forwarder an address nothing answers on), for
/// its rules: Sam's side with `transfers`, the Rae of `Side::Send`. Its news comes to the receiver.
fn bare_link(transfers: Vec<Transfer>) -> (Link, Receiver<LinkEvent>, Sender<Cmd>) {
    let (tx_cmd, rx_cmd) = mpsc::channel();
    let (tx_ev, rx_ev) = mpsc::channel();
    let relay = Relay { server: "127.0.0.1:9".into(), username: "1:s".into(), credential: "ps".into() };
    let udp = UdpSocket::bind("127.0.0.1:0").unwrap();
    let setup = Setup { me: "ss".into(), peer: "rr".into(), dial_pass: None, their_offer: None, relay: relay.clone(), transfers, bind: "127.0.0.1:0".into(), early_ice: Vec::new() };
    (Link::new(setup, Alloc::new(udp, relay), rx_cmd, tx_ev), rx_ev, tx_cmd)
}

/// Channel ids as str0m hands them out (they cannot be made any other way).
fn channel_ids(n: usize) -> Vec<ChannelId> {
    let mut rtc = Rtc::builder().build(Instant::now());
    (0..n).map(|i| rtc.direct_api().create_data_channel(str0m::channel::ChannelConfig { label: format!("t{i}"), ..Default::default() })).collect()
}

/// RULE 8 OF THE SECOND REVIEW, THE SENDER, EACH BRANCH (the coordinator's ruling of 2026-10-12,
/// the website's `ftSendChannelGone`): Sam sends three files and Rae closes each one's channel
/// without a word. Each is told to the engine at once (`TransferClosed`). Then, the connection's
/// clock run on to past [`STOPPED_WAIT`]:
/// - STOP: X, with something heard from Rae after its close and its channel not opened again,
///   ends "Rae's side stopped it";
/// - DROP: with nothing heard from Rae since the closes (a receiver that restarted: its channels
///   close one at a time while the connection still reads connected, then nothing), none ends;
///   the connection keeps them for the next dial;
/// - OPENED AGAIN: Y's channel, opened again within the wait, is served and never ends;
/// - NOT YET: Z, closed only five seconds ago, does not end before its wait is over.
///
/// Seen red 2026-10-12 with `stop_due` leaving out `they_stopped` (ending at 15 seconds whether or
/// not the connection is still there): "a drop ends nothing" failed with X ended. Seen red the same
/// day with `closed_channels` judging a sending transfer whose channel is open again (both guards
/// taken out: its `cid.is_none()` test and `bind` clearing `closed_by_them`; either one alone
/// keeps Y going): "only X ends, with words" failed with Y ended too.
#[test]
fn a_closed_sending_channel_ends_as_a_stop_only_with_them_still_there_after_the_wait() {
    let dir = std::env::temp_dir().join(format!("hum_file_link_branches_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (px, _, ox) = file(&dir, X, 1000, 1);
    let (py, _, oy) = file(&dir, Y, 1000, 2);
    let (pz, _, oz) = file(&dir, Z, 1000, 3);
    let (mut link, news, _cmds) = bare_link(vec![send(&px, &ox), send(&py, &oy), send(&pz, &oz)]);
    let cids = channel_ids(4);
    for (id, cid) in [(X, cids[0]), (Y, cids[1]), (Z, cids[2])] {
        link.bind(id, cid);
    }
    for cid in &cids[..3] {
        link.channel_closed(*cid);
    }
    let told: Vec<LinkEvent> = news.try_iter().collect();
    for id in [X, Y, Z] {
        assert!(told.contains(&LinkEvent::TransferOpen(id.into())) && told.contains(&LinkEvent::TransferClosed(id.into())), "{id}: told at once: {told:?}");
    }
    // The closes happened 16 seconds ago, except Z's, five.
    let ago = |s: u64| Instant::now().checked_sub(Duration::from_secs(s)).unwrap();
    let closed = ago(16);
    for id in [X, Y] {
        link.legs.get_mut(id).unwrap().closed_by_them = Some(closed);
    }
    link.legs.get_mut(Z).unwrap().closed_by_them = Some(ago(5));
    link.bind(Y, cids[3]); // Y opened again within the wait

    // DROP: nothing heard from Rae since.
    link.heard_at = Some(closed - Duration::from_millis(1));
    link.closed_channels(Instant::now());
    let ended = |news: &[LinkEvent]| news.iter().filter_map(|e| if let LinkEvent::Ended(id, end) = e { Some((id.clone(), end.clone())) } else { None }).collect::<Vec<_>>();
    let drop_news: Vec<LinkEvent> = news.try_iter().collect();
    assert!(ended(&drop_news).is_empty(), "a drop ends nothing: {drop_news:?}");
    assert!(!link.legs[X].ended && !link.legs[Z].ended, "kept for the next dial");

    // STOP: Rae is still there.
    link.heard_at = Some(closed + Duration::from_secs(4));
    link.closed_channels(Instant::now());
    let stop_news: Vec<LinkEvent> = news.try_iter().collect();
    assert_eq!(ended(&stop_news), vec![(X.to_string(), End::Failed(stopped_line("Rae")))], "only X ends, with words: {stop_news:?}");
    assert!(!link.legs[Y].ended, "opened again within the wait: served");
    assert!(link.legs[Y].cid == Some(cids[3]));
    assert!(!link.legs[Z].ended, "Z's wait is not over");
    let _ = std::fs::remove_dir_all(&dir);
}

/// RULE 2 OF THE FIRST REVIEW: the connection closes when its last channel ends. A receiving leg
/// is a channel (it opens one), a sending leg is one once the receiver opened it; one leg ending
/// leaves the rest, and an offer the other person never opened a channel for does not keep the
/// connection up.
/// Seen red 2026-10-12 with the close rule as before the review (every leg ended, channel or not):
/// "an offer nobody opened does not keep it up" failed.
#[test]
fn the_connection_closes_when_its_last_channel_ends() {
    let dir = std::env::temp_dir().join(format!("hum_file_link_close_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (px, _, ox) = file(&dir, X, 1000, 1);
    let (_, _, oz) = file(&dir, Z, 1000, 3);
    let leg = |t: Transfer, ended: bool, had_channel: bool| {
        let mut l = Leg::new(t.side, Disk::None);
        l.ended = ended;
        l.had_channel = had_channel;
        l
    };
    let sent_done = leg(send(&px, &ox), true, true);
    let offered = leg(send(&px, &oz), false, false);
    assert!(last_channel_ended([&sent_done, &offered].into_iter()), "an offer nobody opened does not keep it up");
    let receiving = leg(receive(&dir, &oz, "Rae"), false, false);
    assert!(!last_channel_ended([&sent_done, &receiving].into_iter()), "a receiving leg is a channel, still going");
    let opened = leg(send(&px, &oz), false, true);
    assert!(!last_channel_ended([&sent_done, &opened].into_iter()), "one ended leaves the other going");
    assert!(!last_channel_ended([&offered].into_iter()), "no channel yet: nothing has ended");
    assert!(!last_channel_ended(std::iter::empty::<&Leg>()));
    let _ = std::fs::remove_dir_all(&dir);
}

/// RULE 8, A CHANNEL OPENED FOR A TRANSFER THAT ENDED HERE BEFORE THIS CONNECTION (the review of the
/// merged code, 2026-10-12), on a connection with no thread: it carries X when Rae opens Y's
/// channel, which it was not given, and asks the engine (`Wanted`). She opens Y's again before the
/// answer: the first is closed and the second kept. The engine hands Y over as ended
/// (`Cmd::AddEnded`): `cancel` is written on that channel, and Y is not served (its file, deleted
/// already, never opened; no `TransferOpen`, no end reported). A channel she opens for Y once more
/// is answered `cancel` again, the last one closed.
/// Seen red 2026-10-12 with `Cmd::AddEnded` doing nothing: "cancel is said on Y's channel" failed
/// (no leg for Y). Seen red the same day with `ChannelOpen` keeping a second unbound channel
/// without closing the first (as before the review): "the replaced channel is closed" failed.
#[test]
fn a_channel_for_a_transfer_that_ended_here_is_answered_with_cancel() {
    let dir = std::env::temp_dir().join(format!("hum_file_link_ended_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (px, _, ox) = file(&dir, X, 1000, 1);
    let (py, _, oy) = file(&dir, Y, 1000, 2);
    let (mut link, news, _cmds) = bare_link(vec![send(&px, &ox)]);
    let cids = channel_ids(4);
    link.bind(X, cids[0]);
    link.rtc_event(RtcEvent::ChannelOpen(cids[1], label(Y)));
    link.rtc_event(RtcEvent::ChannelOpen(cids[2], label(Y)));
    let told: Vec<LinkEvent> = news.try_iter().collect();
    assert_eq!(told.iter().filter(|e| **e == LinkEvent::Wanted(Y.into())).count(), 2, "the engine asked each time: {told:?}");
    assert_eq!(link.closed_here, vec![cids[1]], "the replaced channel is closed");
    assert_eq!(link.unbound.get(Y), Some(&cids[2]), "the newer one kept");

    std::fs::remove_file(&py).unwrap(); // a cleaned copy goes when its transfer ends
    link.command(Cmd::AddEnded(send(&py, &oy)));
    let said: Vec<Out> = link.legs.get(Y).map(|l| l.outbox.iter().cloned().collect()).unwrap_or_default();
    assert_eq!(said, vec![cancel_message()], "cancel is said on Y's channel");
    let leg = &link.legs[Y];
    assert!(leg.ended && !leg.done && leg.cid == Some(cids[2]), "held as ended, on that channel");
    assert!(link.unbound.is_empty());
    let told: Vec<LinkEvent> = news.try_iter().collect();
    assert!(told.is_empty(), "not served, nothing reported: {told:?}");
    assert!(!link.legs[X].ended, "X goes on");

    link.rtc_event(RtcEvent::ChannelOpen(cids[3], label(Y)));
    assert_eq!(link.legs[Y].cid, Some(cids[3]));
    assert_eq!(link.legs[Y].outbox.len(), 2, "a channel opened for Y again is answered cancel again");
    assert_eq!(link.closed_here, vec![cids[1], cids[2]], "the last one closed");
    let told: Vec<LinkEvent> = news.try_iter().collect();
    assert!(told.is_empty(), "the connection holds Y now: nothing asked: {told:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// THE LAST MESSAGES GET THEIR ACKNOWLEDGEMENT (the review of the merged code, 2026-10-12): once its
/// last channel ended, a connection stays until what it wrote was acknowledged, at most 15 seconds,
/// so a receiver's `done` lost on its way is sent again by SCTP (after one second, then two more,
/// then four more: at 1, 3 and 7 seconds) before the connection closes; acknowledged, it closes
/// at once.
/// Seen red 2026-10-12 with `LINGER` back at 2 seconds: "a done lost twice is sent again at 3
/// seconds: still there" failed.
#[test]
fn the_connection_stays_until_its_last_messages_are_acknowledged_or_15_seconds() {
    let t = Instant::now();
    let at = |ms: u64| t + Duration::from_millis(ms);
    assert!(may_close(t, true, t), "acknowledged: at once");
    assert!(may_close(t, true, at(5_000)));
    assert!(!may_close(t, false, at(3_000)), "a done lost twice is sent again at 3 seconds: still there");
    assert!(!may_close(t, false, at(7_000)), "lost three times, at 7 seconds: still there");
    assert!(!may_close(t, false, at(14_999)));
    assert!(may_close(t, false, at(15_000)), "15 seconds: it goes, whatever is left");
    assert_eq!(LINGER, Duration::from_secs(15));
}

/// A transfer's channel is told by its label.
/// Seen red 2026-10-11 with `label_id` taking anything after "file:": "file:not-an-id" read as
/// a transfer.
#[test]
fn a_channel_label_names_its_transfer() {
    assert_eq!(label(X), format!("file:{X}"));
    assert_eq!(label_id(&label(X)), Some(X));
    assert_eq!(label_id("hum-data"), None, "the app's own data channel");
    assert_eq!(label_id("file:not-an-id"), None);
}

/// THE LOOP NEVER WAITS INSIDE A RECEIVE (BUG-201, 2026-10-22): when a turn moved nothing it
/// sleeps (a millisecond at first, longer as the quiet goes on, at most 20 ms, never past the
/// Rtc's next deadline), and the next turn reads what came meanwhile from a nonblocking socket. A
/// receive with a timeout on Windows can lose a datagram arriving as it times out; on a loaded
/// machine the link lost a receiver's `done` and all its resends that way (`Link::run`, step 7).
/// The source is read too, so a timed receive put back anywhere in the link fails here.
/// Seen red 2026-10-22 with the timed receive put back in step 7: "the link never receives with
/// a timeout".
#[test]
fn the_loop_sleeps_and_never_receives_with_a_timeout() {
    let ms = Duration::from_millis;
    assert_eq!(idle_sleep(ms(0), None), ms(1), "a millisecond right after something moved");
    assert_eq!(idle_sleep(ms(80), None), ms(10), "longer as the quiet goes on");
    assert_eq!(idle_sleep(Duration::from_secs(60), None), IDLE_WAIT, "at most 20 ms");
    assert_eq!(idle_sleep(Duration::from_secs(60), Some(ms(3))), ms(3), "never past the Rtc's deadline");
    assert_eq!(idle_sleep(Duration::from_secs(60), Some(ms(0))), ms(1), "but at least a millisecond");
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/net/file_link.rs")).unwrap();
    let code: String = source.lines().filter(|l| !l.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n");
    assert!(!code.contains("set_read_timeout"), "the link never receives with a timeout");
    assert!(code.contains("set_nonblocking(true)"), "its socket is nonblocking");
    assert!(!code.contains("set_nonblocking(false)"), "for good");
}
