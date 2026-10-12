// Child of net/file_link.rs (#[path]): transfers end to end on this machine, without the game.

use super::*;
use crate::net::file_offer::{hashes_digest, pieces_for, FilePass, Offer, PIECE_SIZE};
use crate::net::file_transfer::{End, Receiving, Sending};
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

/// A receiving transfer of `offer`, its part file made at full size in `dir`.
fn receive(dir: &std::path::Path, offer: &Offer, sender: &str) -> Transfer {
    let part = dir.join(format!("{}.{}.part", offer.name, &offer.id[..12]));
    std::fs::OpenOptions::new().write(true).create(true).truncate(true).open(&part).unwrap().set_len(offer.size).unwrap();
    Transfer {
        id: offer.id.clone(),
        side: Side::Receive { proto: Receiving::new(offer.clone(), sender, true, Instant::now()), part, dir: dir.to_path_buf(), name: offer.name.clone(), sender: sender.into() },
    }
}

fn send(path: &std::path::Path, offer: &Offer) -> Transfer {
    Transfer { id: offer.id.clone(), side: Side::Send { proto: Sending::new(offer.clone(), None), path: path.to_path_buf() } }
}

/// ONE CONNECTION PER PAIR, ONLY THROUGH THE FORWARDER (parts 4 and 5, the shape agreed with the
/// web): Sam offered Rae two files (X and Y) and Rae offered Sam one (Z). Rae dials Sam with X's
/// credentials; her `dc_offer` carries X's pass and no transfer id. Sam, dialled, cannot know which
/// of his two offers she chose, so he answers with credentials for both: two allocations on two
/// sockets, two relayed candidates; the forwarder joins only the pair in X's scope. On the one
/// connection each receiver opens its transfer's channel (Rae X and Y, Sam Z, on the connection
/// Rae dialled), the sender tells them apart by the label, and all three files arrive whole, each
/// named only once its whole BLAKE3 matched. Every candidate is a relayed address the forwarder
/// gave out, and those addresses are `Trap` sockets: anything sent around the forwarder is caught.
/// Seen red 2026-10-11 with `drain` sending each transmit through the LAST allocation whatever its
/// source (`allocs.last_mut()` for the source match): Sam's packets left from his Y-scope
/// allocation, which the forwarder will not join with Rae's, so no channel opened ("Rae saved
/// all" failed, each side having seen only RelayReady, its one signal and Closed).
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

    let traps: Vec<Trap> = (0..3).map(|_| Trap::bind()).collect();
    let fwd = Forwarder::start(traps.iter().map(|t| t.addr).collect());
    fwd.add_account("1:rx", "prx", &format!("file:{X}"));
    fwd.add_account("1:sx", "psx", &format!("file:{X}"));
    fwd.add_account("1:sy", "psy", &format!("file:{Y}"));
    let relay = |user: &str, cred: &str| Relay { server: fwd.addr.to_string(), username: user.into(), credential: cred.into() };

    let rae = FileLink::start(Setup {
        me: "rr".into(),
        peer: "ss".into(),
        dial_pass: Some(ox.pass.clone()),
        their_offer: None,
        relays: vec![relay("1:rx", "prx")],
        transfers: vec![receive(&rae_dir, &ox, "Sam"), receive(&rae_dir, &oy, "Sam"), send(&pz, &oz)],
        bind: "127.0.0.1:0".into(),
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
                assert!(inner.get("file").is_none(), "signalling carries no transfer id");
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
                        relays: vec![relay("1:sx", "psx"), relay("1:sy", "psy")],
                        transfers: vec![send(&px, &ox), send(&py, &oy), receive(&sam_dir, &oz, "Rae")],
                        bind: "127.0.0.1:0".into(),
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
    assert_eq!(candidates(&sdps[1]).len(), 2, "the answerer: one per credential");
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

/// A transfer's channel is told by its label (signals carry no transfer id).
/// Seen red 2026-10-11 with `label_id` taking anything after "file:": "file:not-an-id" read as
/// a transfer.
#[test]
fn a_channel_label_names_its_transfer() {
    assert_eq!(label(X), format!("file:{X}"));
    assert_eq!(label_id(&label(X)), Some(X));
    assert_eq!(label_id("hum-data"), None, "the app's own data channel");
    assert_eq!(label_id("file:not-an-id"), None);
}
