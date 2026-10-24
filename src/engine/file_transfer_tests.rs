// Child of engine/file_transfer.rs (#[path]): the app's half of device-to-device big files, with
// two apps' state and recording sockets, no screen and no network. What one app sends is opened
// with the other's seed and handed to it through its own DM path, as the relay would. Each rule
// seen failing once against the break named in its doc comment.

use super::*;
use crate::net::dm_store::{DmStore, SentPass};
use crate::relay::core::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};
use serde_json::{json, Value};
use std::sync::mpsc::Receiver;

const SERVER: &str = "did:hum:4dQe1bVHyiHm1Vh8rWbx2F";

fn identity(n: u8) -> (Vec<u8>, String) {
    let seed = vec![n; 32];
    (seed.clone(), hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&seed)).public_key()))
}

struct App {
    gs: GuiState,
    sent: Receiver<String>,
    seed: Vec<u8>,
    key: String,
    dir: PathBuf,
}

/// A signed-in app on a server of its own with a recording socket, for identity `n`; `peer` is
/// known (their DM key and name), and both count as friends (each holds a pass from the other), so
/// a DM from them is admitted. `peer_online` lists them as online.
fn app(n: u8, tag: &str, peer: &(Vec<u8>, String), peer_name: &str, peer_online: bool) -> App {
    let (seed, key) = identity(n);
    let mut gs = GuiState::default();
    gs.profile_public_key = key.clone();
    gs.private_key_bytes = Some(seed.clone());
    // The process id too: two test programs running at once (two checkouts verifying) share the
    // temporary folder, so a name from the clock alone can be another program's. With three copies
    // of this test at once, one failed on 2026-10-22 reading its saved file ("The system cannot
    // find the path specified"), its folder apparently gone; the id rules that out.
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let nanos = format!("{}_{nanos}", std::process::id());
    gs.server_url = format!("https://{tag}-{nanos}.example");
    let mut store = DmStore::load(&seed, &key, &crate::gui::pages::chat::norm_server_url(&gs.server_url));
    store.set_pass_server(SERVER);
    store.set_following(&peer.1, true);
    store.set_follower(&peer.1, true);
    store.record_pass_sent(&peer.1, SentPass { serial: "11".repeat(16), may: "invite,message".into() });
    gs.dm_store = Some(store);
    gs.block_list = Some(crate::net::block_list::BlockList::in_temp(&seed, &key, tag));
    gs.peer_kyber_keys.insert(peer.1.clone(), crate::net::dm_pq::DmPqKeypair::from_bip39_seed(&peer.0).unwrap().public_base64());
    if peer_online {
        gs.chat_users.push(crate::gui::ChatUser { name: peer_name.into(), public_key: peer.1.clone(), role: String::new(), status: "online".into() });
    } else {
        // Their name is known from the conversation, not from the list of people online.
        gs.chat_dms.push(crate::gui::ChatDm { user_name: peer_name.into(), user_key: peer.1.clone(), last_message: String::new(), timestamp: String::new(), unread: false });
    }
    let (client, sent) = crate::net::ws_client::WsClient::recording();
    gs.ws_client = Some(client);
    gs.dm_fetch = crate::net::mailbox_fetch::MailboxFetch::already_read();
    gs.files_flow.transfers.bind = Some("127.0.0.1:0".into());
    let dir = std::env::temp_dir().join(format!("hum_engine_transfer_{tag}_{nanos}"));
    std::fs::create_dir_all(&dir).unwrap();
    App { gs, sent, seed, key, dir }
}

fn tidy(apps: &[&App]) {
    for a in apps {
        a.gs.dm_store.as_ref().unwrap().remove_file_for_test();
        a.gs.block_list.as_ref().unwrap().remove_file_for_test();
        let _ = std::fs::remove_dir_all(&a.dir);
    }
}

fn frames(sent: &Receiver<String>) -> Vec<Value> {
    sent.try_iter().map(|f| serde_json::from_str(&f).unwrap()).collect()
}

/// Open a `dm_put` with its recipient's seed.
fn open(put: &Value, seed: &[u8]) -> DmInner {
    let kp = crate::net::dm_pq::DmPqKeypair::from_bip39_seed(seed).unwrap();
    crate::net::dm_pq::parse_verify_inner(&crate::net::dm_pq::open_v2(&kp, put["content"].as_str().unwrap()).unwrap()).unwrap()
}

/// The `dm_put`s among `frames` addressed to `to`.
fn puts_to(frames: &[Value], to: &str) -> Vec<Value> {
    frames.iter().filter(|v| v["type"] == "dm_put" && v["to"] == to).cloned().collect()
}

/// Hand `out` (what some app sent) to `to`: the puts addressed to its identity, opened with its
/// seed, through the app's own DM path (`engine::dm::ingest_dm`).
fn hand(out: &[Value], to: &mut App) {
    for put in puts_to(out, &to.key) {
        let inner = open(&put, &to.seed);
        crate::engine::dm::ingest_dm(&mut to.gs, &inner);
    }
}

/// Everything `from` sent since the last look, handed to `to`. Returns it.
fn deliver(from: &App, to: &mut App) -> Vec<Value> {
    let out = frames(&from.sent);
    hand(&out, to);
    out
}

fn rec(a: &App, id: &str) -> OfferRecord {
    record(&a.gs, id).cloned().expect("a record")
}

fn words(a: &App, id: &str, name: &str) -> String {
    let r = rec(a, id);
    let online = peer_online(&a.gs, &r.peer);
    status_words(&r, a.gs.files_flow.transfers.live.get(id), a.gs.files_flow.transfers.phase(&r.peer), name, online)
}

/// `from` offers `to` a file of `len` bytes: wait for it to be read, return its id. The frames are
/// left unread.
fn offer_file(from: &mut App, to: &str, len: usize) -> String {
    let path = from.dir.join(format!("clip-{len}.mp4"));
    std::fs::write(&path, (0..len).map(|i| (i % 251) as u8).collect::<Vec<u8>>()).unwrap();
    offer(&mut from.gs, to, &path, "clip.mp4", "video/mp4", false, false);
    let id = from.gs.files_flow.transfers.preparing.last().unwrap().id.clone();
    let until = Instant::now() + Duration::from_secs(20);
    while !from.gs.files_flow.transfers.preparing.is_empty() && Instant::now() < until {
        pump(&mut from.gs);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(from.gs.files_flow.transfers.preparing.is_empty(), "the file was read");
    id
}

fn credentials_asked(out: &[Value]) -> Vec<(String, String)> {
    out.iter().filter(|v| v["type"] == "call_credentials").map(|v| (v["file"].as_str().unwrap().to_string(), v["peer"].as_str().unwrap().to_string())).collect()
}

/// A `dc_offer` from `who` as the relay hands it on: let in by the file pass of transfer `file`,
/// which it names (rule 7 of the second review).
fn dc_offer_from(who: &str, file: &str) -> Value {
    json!({ "type": "webrtc_signal", "from": who, "signal_type": "dc_offer", "data": json!({ "type": "offer", "sdp": "v=0\r\n" }).to_string(), "file": file })
}

/// What the relay hands on of a `webrtc_signal` a client sent (msg_handlers.rs
/// `handle_webrtc_signal`): the frame without its passes, and a `dc_offer` let in by a file pass
/// naming that pass's transfer as `file` (rule 7).
fn via_relay(sent: &Value) -> Value {
    let mut out = sent.clone();
    let map = out.as_object_mut().unwrap();
    let pass = map.remove("file_pass");
    map.remove("friend_cert");
    if sent["signal_type"] == "dc_offer" {
        if let Some(id) = pass.as_ref().and_then(|p| p.get("id")).cloned() {
            map.insert("file".into(), id);
        }
    }
    out
}

/// AN OFFER IS A SEALED NOTE, WITH ITS SELF-COPY, WHOSE PASS CHECKS (parts 1 and 3). Sam offers Rae
/// a 600 KB file: one `dm_put` to Rae and one to Sam's own mailbox, both the offer note; its pass
/// checks for Sam to Rae on this server; Sam's record waits for an answer. Rae's app takes the note
/// through its DM path, never as a message, lists it and says so.
/// Seen red 2026-10-11 with `screens_dm` taken out of `ingest_dm` (engine/dm.rs, `if false &&`):
/// Rae's app listed nothing, `rec` failing with "a record".
#[test]
fn an_offer_goes_as_a_sealed_note_and_is_listed_not_shown() {
    crate::config::keep_saves_off_disk();
    let (rae_id, sam_id) = (identity(171), identity(172));
    let mut sam = app(172, "offer-sam", &rae_id, "Rae", true);
    let mut rae = app(171, "offer-rae", &sam_id, "Sam", true);
    let id = offer_file(&mut sam, &rae.key, 600_000);
    let out = deliver(&sam, &mut rae);
    let to_rae = puts_to(&out, &rae.key);
    let to_me = puts_to(&out, &sam.key);
    assert_eq!((to_rae.len(), to_me.len()), (1, 1), "the note and its self-copy");
    let inner = open(&to_rae[0], &rae.seed);
    let Some(fo::Note::Offer(offer)) = fo::parse_note(&inner.text) else { panic!("an offer note") };
    assert_eq!(open(&to_me[0], &sam.seed).text, inner.text, "the same note to my other devices");
    assert_eq!((offer.id.as_str(), offer.name.as_str(), offer.size, offer.pieces), (id.as_str(), "clip.mp4", 600_000, 3));
    assert_eq!(fo::verify_file_pass(&sam.key, SERVER, &rae.key, &offer.pass, fo::now_secs()), Ok(()), "the pass checks");
    let r = rec(&sam, &id);
    assert!(r.outgoing && r.here && r.stage == Stage::Offered && r.path.ends_with("clip-600000.mp4"));
    assert_eq!(words(&sam, &id, "Rae"), "Waiting for Rae to accept");

    let theirs = rec(&rae, &id);
    assert!(!theirs.outgoing && !theirs.here && theirs.stage == Stage::Offered);
    assert!(rae.gs.dm_store.as_ref().unwrap().conversation(&sam.key).is_empty(), "never shown as a message");
    assert!(rae.gs.pending_notices.iter().any(|n| n.starts_with("Sam wants to send you clip.mp4")), "{:?}", rae.gs.pending_notices);
    assert!(rae.gs.chat_dms.iter().any(|d| d.user_key == sam.key), "their conversation is listed, to be opened");
    tidy(&[&sam, &rae]);
}

/// DECLINE (part 3): Rae's Decline sends `accept: false` to Sam and to her own mailbox; Sam's row
/// says "Rae declined"; Rae's says "You declined"; her other device, given the self-copy, says
/// "Answered on your other device". An answered offer cannot be answered again.
/// Seen red 2026-10-11 with `decline` sending `accept: true`: the answer Sam opened read
/// `accept: true` where `accept: false` was expected.
#[test]
fn decline_answers_no_and_both_sides_see_it() {
    crate::config::keep_saves_off_disk();
    let (rae_id, sam_id) = (identity(173), identity(174));
    let mut sam = app(174, "decline-sam", &rae_id, "Rae", true);
    let mut rae = app(173, "decline-rae", &sam_id, "Sam", true);
    let mut rae2 = app(173, "decline-rae2", &sam_id, "Sam", true);
    let id = offer_file(&mut sam, &rae.key, 300_000);
    let out = deliver(&sam, &mut rae);
    hand(&out, &mut rae2);
    assert!(decline(&mut rae.gs, &id));
    let out = deliver(&rae, &mut sam);
    hand(&out, &mut rae2);
    let answer = open(&puts_to(&out, &sam.key)[0], &sam.seed);
    assert_eq!(fo::parse_note(&answer.text), Some(fo::Note::Answer(OfferAnswer { id: id.clone(), accept: false })));
    assert_eq!(rec(&sam, &id).stage, Stage::Declined);
    assert_eq!(words(&sam, &id, "Rae"), "Rae declined", "Sam's row says Rae declined");
    assert_eq!(words(&rae, &id, "Sam"), "You declined");
    assert_eq!(words(&rae2, &id, "Sam"), ANSWERED_ELSEWHERE, "her other device");
    assert!(!decline(&mut rae.gs, &id) && !accept(&mut rae.gs, &id, &rae.dir.clone(), "clip.mp4"), "an answered offer is answered");
    tidy(&[&sam, &rae, &rae2]);
}

/// NOTHING IS SENT FOR AN OFFER LEFT UNANSWERED (the shape agreed with the web): once its pass runs
/// out, Sam's row says "Rae did not answer in time" and Rae's "This offer ran out", and neither app
/// sends anything (an automatic decline would tell Sam when Rae came online).
/// Seen red 2026-10-11 with `pump` ending an unanswered offer as `Failed(RAN_OUT)` like an accepted
/// one: Sam's record was `Failed(..)` where `Expired` was expected.
#[test]
fn an_unanswered_offer_runs_out_with_nothing_sent() {
    crate::config::keep_saves_off_disk();
    let (rae_id, sam_id) = (identity(183), identity(184));
    let mut sam = app(184, "expire-sam", &rae_id, "Rae", true);
    let mut rae = app(183, "expire-rae", &sam_id, "Sam", true);
    let id = offer_file(&mut sam, &rae.key, 300_000);
    deliver(&sam, &mut rae);
    for a in [&mut sam, &mut rae] {
        with_record(&mut a.gs, &id, |r| r.offer.pass.expiry = fo::now_secs() - 1);
        pump(&mut a.gs);
    }
    assert_eq!(rec(&sam, &id).stage, Stage::Expired);
    assert_eq!(words(&sam, &id, "Rae"), "Rae did not answer in time", "Sam's row");
    assert_eq!(words(&rae, &id, "Sam"), "This offer ran out");
    assert!(frames(&sam.sent).is_empty() && frames(&rae.sent).is_empty(), "nothing sent either way");
    tidy(&[&sam, &rae]);
}

/// CANCEL FROM EITHER SIDE (part 5): Sam cancels an accepted transfer; the cancel note reaches Rae,
/// whose row says "Sam cancelled" and whose part file is gone. Then Rae cancels another; Sam's row
/// says "Rae cancelled". A cancel needs no connection: the note carries it.
/// Seen red 2026-10-11 with `cancel` not sending the note (the channel only): "the note reaches
/// Rae" found no `dm_put`.
#[test]
fn cancel_from_either_side_reaches_the_other() {
    crate::config::keep_saves_off_disk();
    let (rae_id, sam_id) = (identity(175), identity(176));
    let mut sam = app(176, "cancel-sam", &rae_id, "Rae", true);
    let mut rae = app(175, "cancel-rae", &sam_id, "Sam", true);
    let id = offer_file(&mut sam, &rae.key, 300_000);
    deliver(&sam, &mut rae);
    let downloads = rae.dir.join("downloads");
    assert!(accept(&mut rae.gs, &id, &downloads, "clip.mp4"));
    let part = part_path(&downloads, "clip.mp4", &id);
    assert_eq!(std::fs::metadata(&part).map(|m| m.len()).ok(), Some(300_000), "the part file, at the file's size");
    deliver(&rae, &mut sam);
    assert_eq!(rec(&sam, &id).stage, Stage::Accepted);

    cancel(&mut sam.gs, &id);
    let out = deliver(&sam, &mut rae);
    assert_eq!(puts_to(&out, &rae.key).len(), 1, "the note reaches Rae");
    assert_eq!(rec(&rae, &id).stage, Stage::Cancelled { by_me: false });
    assert_eq!(words(&rae, &id, "Sam"), "Sam cancelled");
    assert_eq!(words(&sam, &id, "Rae"), "Cancelled");
    assert!(!part.exists(), "Rae's part file is gone");

    let id2 = offer_file(&mut sam, &rae.key, 200_000);
    deliver(&sam, &mut rae);
    assert!(accept(&mut rae.gs, &id2, &downloads, "clip.mp4"));
    deliver(&rae, &mut sam);
    cancel(&mut rae.gs, &id2);
    deliver(&rae, &mut sam);
    assert_eq!(rec(&sam, &id2).stage, Stage::Cancelled { by_me: false });
    assert_eq!(words(&sam, &id2, "Rae"), "Rae cancelled");
    assert!(!part_path(&downloads, "clip.mp4", &id2).exists());
    tidy(&[&sam, &rae]);
}

/// DIALLING (part 4): Rae's accepted transfer is dialled only while Sam is online and it is not
/// paused, by asking for the forwarder's credentials with the pass
/// (`{"type":"call_credentials","file","peer","file_pass"}`); a refusal says the server is not set
/// up and is not asked again for a minute; a pause is kept and nothing is dialled while it holds;
/// Resume tries again at once.
/// Seen red 2026-10-11 with the `peer_online` condition taken out of `pump`: "nothing while Sam is
/// away" found a `call_credentials` request.
#[test]
fn an_accepted_transfer_is_dialled_only_while_the_sender_is_online_and_not_paused() {
    crate::config::keep_saves_off_disk();
    let (rae_id, sam_id) = (identity(177), identity(178));
    let mut sam = app(178, "dial-sam", &rae_id, "Rae", true);
    let mut rae = app(177, "dial-rae", &sam_id, "Sam", false);
    let id = offer_file(&mut sam, &rae.key, 300_000);
    deliver(&sam, &mut rae);
    assert!(accept(&mut rae.gs, &id, &rae.dir.join("dl"), "clip.mp4"));
    frames(&rae.sent);
    pump(&mut rae.gs);
    assert!(frames(&rae.sent).is_empty(), "nothing while Sam is away");
    assert_eq!(words(&rae, &id, "Sam"), "Waiting for Sam to come online");

    rae.gs.chat_users.push(crate::gui::ChatUser { name: "Sam".into(), public_key: sam.key.clone(), role: String::new(), status: "online".into() });
    pump(&mut rae.gs);
    let asked = frames(&rae.sent);
    let pass = rec(&rae, &id).offer.pass;
    assert_eq!(asked, vec![json!({ "type": "call_credentials", "file": id, "peer": sam.key, "file_pass": pass })], "the credentials, asked with the pass");
    assert_eq!(words(&rae, &id, "Sam"), "Connecting through the server's call relay");

    assert!(on_credentials(&mut rae.gs, &json!({ "type": "call_credentials", "file": id, "refused": true })));
    assert_eq!(words(&rae, &id, "Sam"), NOT_SET_UP);
    pump(&mut rae.gs);
    assert!(frames(&rae.sent).is_empty(), "not asked again at once");

    pause(&mut rae.gs, &id);
    assert!(rec(&rae, &id).paused, "a pause is kept");
    assert_eq!(words(&rae, &id, "Sam"), "Paused");
    resume(&mut rae.gs, &id);
    pause(&mut rae.gs, &id);
    pump(&mut rae.gs);
    assert!(frames(&rae.sent).is_empty(), "nothing dialled while paused");
    resume(&mut rae.gs, &id);
    pump(&mut rae.gs);
    assert_eq!(credentials_asked(&frames(&rae.sent)), vec![(id.clone(), sam.key.clone())], "Resume tries again at once");
    assert!(!on_credentials(&mut rae.gs, &json!({ "type": "call_credentials", "room": "r1", "refused": true })), "a call's answer is not a transfer's");
    tidy(&[&sam, &rae]);
}

/// RULE 7 OF THE SECOND REVIEW, A DIAL NAMES ITS TRANSFER: Sam has five live offers to Rae, and
/// her dial names the third (`file`, which the relay sets on a `dc_offer` it let in by a file
/// pass). Sam asks for that one transfer's credentials and no other's: one allocation for the
/// connection. A dial naming a transfer that is not one of Sam's live offers to its sender (one
/// to someone else, one not offered, one that ended) is taken and refused quietly: nothing sent,
/// nothing asked, and not handed to the WebRTC manager. A `dc_offer` naming no transfer is not a
/// file dial and is the manager's (a friend's direct connection), as is a candidate from someone
/// with no file connection standing or being answered.
///
/// THE BACKSTOP OF RULE 8 (the review of the merged code, 2026-10-12): a dial naming an offer of
/// Sam's to Rae that he cancelled is not answered either, but it means his cancel note never
/// reached her (she dials with her oldest accepted transfer, so her other files from him wait
/// behind it): the cancel note goes again, exactly one, and her row ends. A second dial for it in
/// the same run sends nothing, and a dial naming one that was sent (done) sends nothing.
/// Seen red 2026-10-12 with `on_signal` answering a dial with the newest live offer whatever it
/// named (as if it named none): "only the transfer the dial names" got the fifth offer. Seen red
/// 2026-10-12 with `on_signal` not saying the cancel again (the `say_cancel_again` call taken
/// out): "the cancel note goes again, exactly one" failed, 0 against 1. Seen red the same day with
/// `say_cancel_again` not keeping its once-a-run mark (`said_again` not consulted): "a second dial
/// for it sends nothing" failed.
#[test]
fn a_dial_is_answered_for_the_transfer_it_names() {
    crate::config::keep_saves_off_disk();
    let (rae_id, sam_id) = (identity(179), identity(180));
    let mut sam = app(180, "signal-sam", &rae_id, "Rae", true);
    let mut rae = app(179, "signal-rae", &sam_id, "Sam", true);
    let offers: Vec<String> = (0..5)
        .map(|n| {
            std::thread::sleep(Duration::from_millis(3));
            offer_file(&mut sam, &rae.key, 100_000 + n * 1_000)
        })
        .collect();
    deliver(&sam, &mut rae);
    let stranger = identity(181).1;
    let mut unnamed = dc_offer_from(&rae.key, "");
    unnamed.as_object_mut().unwrap().remove("file");
    assert!(!on_signal(&mut sam.gs, &unnamed), "a dial naming no transfer: the WebRTC manager's");
    assert!(on_signal(&mut sam.gs, &dc_offer_from(&stranger, &offers[2])), "naming an offer to someone else: taken");
    assert!(on_signal(&mut sam.gs, &dc_offer_from(&rae.key, &"ab".repeat(16))), "naming nothing offered: taken");
    assert!(frames(&sam.sent).is_empty(), "and refused quietly");
    assert!(matches!(sam.gs.files_flow.transfers.phase(&rae.key), Phase::Idle));

    assert!(on_signal(&mut sam.gs, &dc_offer_from(&rae.key, &offers[2])), "Rae's dial is the file connection");
    assert_eq!(credentials_asked(&frames(&sam.sent)), vec![(offers[2].clone(), rae.key.clone())], "only the transfer the dial names");
    let ice = json!({ "type": "webrtc_signal", "from": rae.key, "signal_type": "dc_ice", "data": json!({ "candidate": "" }).to_string() });
    assert!(!on_signal(&mut rae.gs, &ice), "no file connection on Rae's side: the WebRTC manager's");
    assert!(on_credentials(&mut sam.gs, &json!({ "type": "call_credentials", "file": offers[2], "refused": true })));
    assert!(matches!(sam.gs.files_flow.transfers.phase(&rae.key), Phase::NotSetUp(_)), "refused: not set up");

    cancel(&mut sam.gs, &offers[4]);
    frames(&sam.sent); // the cancel note, lost on its way to Rae
    assert!(on_signal(&mut sam.gs, &dc_offer_from(&rae.key, &offers[4])), "naming one that ended: taken");
    let again = frames(&sam.sent);
    assert!(credentials_asked(&again).is_empty(), "not answered");
    let to_rae = puts_to(&again, &rae.key);
    assert_eq!(to_rae.len(), 1, "the cancel note goes again, exactly one");
    assert_eq!(fo::parse_note(&open(&to_rae[0], &rae.seed).text), Some(fo::Note::Cancel(OfferCancel { id: offers[4].clone() })));
    hand(&again, &mut rae);
    assert_eq!(rec(&rae, &offers[4]).stage, Stage::Cancelled { by_me: false }, "and Rae's row ends");
    assert!(on_signal(&mut sam.gs, &dc_offer_from(&rae.key, &offers[4])));
    assert!(frames(&sam.sent).is_empty(), "a second dial for it sends nothing (once in this run)");
    with_record(&mut sam.gs, &offers[3], |r| r.stage = Stage::Done);
    assert!(on_signal(&mut sam.gs, &dc_offer_from(&rae.key, &offers[3])), "naming one that was sent: taken");
    assert!(frames(&sam.sent).is_empty(), "and nothing said: no cancel note went for it");
    tidy(&[&sam, &rae]);
}

/// BOTH DIAL AT ONCE (the shape agreed with the web): Sam offered Rae a file and Rae offered Sam
/// one, both accepted, so each dials the other. When each one's `dc_offer` reaches the other, the
/// larger key (string compare of the Dilithium hex) keeps its dial and ignores the other's, and the
/// smaller drops its own attempt and answers, asking for the credentials of its offer the dial
/// names. A candidate of the dial set aside is taken by the winner and dropped (rule 3 of the
/// first review: a file connection's signal is never the call code's).
/// Seen red 2026-10-11 with the key comparison in `on_signal` reversed (`me < from`): "exactly one
/// side yields" found the larger key answering. Seen red 2026-10-12 with `on_signal` as before the
/// review (no connection standing: false): "a candidate of the dial set aside is taken" failed.
#[test]
fn when_both_dial_the_larger_key_wins() {
    crate::config::keep_saves_off_disk();
    let (rae_id, sam_id) = (identity(185), identity(186));
    let mut sam = app(186, "glare-sam", &rae_id, "Rae", true);
    let mut rae = app(185, "glare-rae", &sam_id, "Sam", true);
    let to_rae = offer_file(&mut sam, &rae.key, 300_000);
    deliver(&sam, &mut rae);
    let to_sam = offer_file(&mut rae, &sam.key, 200_000);
    deliver(&rae, &mut sam);
    assert!(accept(&mut rae.gs, &to_rae, &rae.dir.join("dl"), "clip.mp4"));
    assert!(accept(&mut sam.gs, &to_sam, &sam.dir.join("dl"), "clip.mp4"));
    deliver(&rae, &mut sam);
    deliver(&sam, &mut rae);
    pump(&mut rae.gs);
    pump(&mut sam.gs);
    assert_eq!(credentials_asked(&frames(&rae.sent)), vec![(to_rae.clone(), sam.key.clone())], "Rae dials");
    assert_eq!(credentials_asked(&frames(&sam.sent)), vec![(to_sam.clone(), rae.key.clone())], "and so does Sam");

    // Each dial names the transfer it dials with: one of the other side's own offers.
    assert!(on_signal(&mut rae.gs, &dc_offer_from(&sam.key, &to_sam)));
    assert!(on_signal(&mut sam.gs, &dc_offer_from(&rae.key, &to_rae)));
    let (rae_out, sam_out) = (credentials_asked(&frames(&rae.sent)), credentials_asked(&frames(&sam.sent)));
    // The one who yields asks for the credentials of its own offer the winner's dial names.
    let (winner, loser_out, want) = if rae.key > sam.key { ("Rae", &sam_out, (to_rae.clone(), rae.key.clone())) } else { ("Sam", &rae_out, (to_sam.clone(), sam.key.clone())) };
    let winner_out = if winner == "Rae" { &rae_out } else { &sam_out };
    assert!(winner_out.is_empty(), "the larger key ({winner}) ignores the other's dial");
    assert_eq!(loser_out, &vec![want], "exactly one side yields, asking for the credentials of its offer the dial names");
    // A candidate of the dial set aside reaches the winner, still asking for its own credentials:
    // the file connection's, so taken here and not given to the call code, and not kept for the
    // winner's own connection either (rule 3).
    let (winner_gs, loser_key) = if rae.key > sam.key { (&mut rae.gs, sam.key.clone()) } else { (&mut sam.gs, rae.key.clone()) };
    let ice = json!({ "type": "webrtc_signal", "from": loser_key, "signal_type": "dc_ice", "data": json!({ "candidate": "candidate:1 1 udp 16777215 127.0.0.1 40001 typ relay raddr 0.0.0.0 rport 0" }).to_string() });
    assert!(on_signal(winner_gs, &ice), "a candidate of the dial set aside is taken");
    assert!(winner_gs.files_flow.transfers.pair(&loser_key).early_ice.is_empty(), "and not kept for the winner's connection");
    tidy(&[&sam, &rae]);
}

/// `{"type":"webrtc_signal","signal_type":"dc_ice"}` from `from`, carrying `line` the way a browser
/// sends a candidate (`JSON.stringify(event.candidate)`).
fn dc_ice_from(from: &str, line: &str) -> Value {
    json!({ "type": "webrtc_signal", "from": from, "signal_type": "dc_ice", "data": json!({ "candidate": line, "sdpMid": "0", "sdpMLineIndex": 0 }).to_string() })
}

/// RULE 4 OF THE FIRST REVIEW (files-showcase.md): the receiver's answer (yes or no) and its cancel
/// go with the offer's file pass on the `dm_put` to the sender (`file_pass`, the same object the
/// offer carried), so the relay lets them through to a sender who takes messages only from
/// friends; the self-copies carry none, and neither do the sender's own notes (the pass names the
/// receiver as its grantee, so it is no use to the receiver's own gate).
/// Seen red 2026-10-12 with `send_note` leaving the pass off: "the answer's put carries the
/// offer's pass" found `null`.
#[test]
fn the_receivers_answer_and_cancel_carry_the_file_pass() {
    crate::config::keep_saves_off_disk();
    let (rae_id, sam_id) = (identity(187), identity(188));
    let mut sam = app(188, "pass-sam", &rae_id, "Rae", true);
    let mut rae = app(187, "pass-rae", &sam_id, "Sam", true);
    let id = offer_file(&mut sam, &rae.key, 300_000);
    let out = deliver(&sam, &mut rae);
    assert!(out.iter().all(|v| v.get("file_pass").is_none()), "the offer's put carries none (the pass is inside the note)");
    let pass = serde_json::to_value(&rec(&rae, &id).offer.pass).unwrap();
    assert!(accept(&mut rae.gs, &id, &rae.dir.join("dl"), "clip.mp4"));
    let out = deliver(&rae, &mut sam);
    let to_sam = puts_to(&out, &sam.key);
    assert_eq!(to_sam.len(), 1);
    assert_eq!(to_sam[0]["file_pass"], pass, "the answer's put carries the offer's pass");
    assert!(puts_to(&out, &rae.key).iter().all(|v| v.get("file_pass").is_none()), "its self-copy carries none");
    assert_eq!(rec(&sam, &id).stage, Stage::Accepted, "and Sam heard it");

    cancel(&mut rae.gs, &id);
    let out = deliver(&rae, &mut sam);
    assert_eq!(puts_to(&out, &sam.key)[0]["file_pass"], pass, "the receiver's cancel too");
    assert_eq!(rec(&sam, &id).stage, Stage::Cancelled { by_me: false });

    let id2 = offer_file(&mut sam, &rae.key, 200_000);
    deliver(&sam, &mut rae);
    let pass2 = serde_json::to_value(&rec(&rae, &id2).offer.pass).unwrap();
    assert!(decline(&mut rae.gs, &id2));
    let out = deliver(&rae, &mut sam);
    assert_eq!(puts_to(&out, &sam.key)[0]["file_pass"], pass2, "a decline too");

    let id3 = offer_file(&mut sam, &rae.key, 200_000);
    deliver(&sam, &mut rae);
    cancel(&mut sam.gs, &id3);
    let out = deliver(&sam, &mut rae);
    let to_rae = puts_to(&out, &rae.key);
    assert_eq!(to_rae.len(), 1, "the sender's cancel goes");
    assert!(to_rae[0].get("file_pass").is_none(), "carrying no pass");
    tidy(&[&sam, &rae]);
}

/// RULE 3 OF THE FIRST REVIEW: Rae dials Sam, and her candidates come as `dc_ice` after her offer,
/// while Sam is still waiting for his credentials. Each is taken by the file connection (true, so
/// never handed to the call code) and kept, in order; when the credentials come the connection
/// starts with them; once it stands, a candidate goes straight to it. Without a dial or a file
/// connection with them, a candidate is still the WebRTC manager's.
/// Seen red 2026-10-12 with `on_signal` as before the review (no connection standing: false):
/// "taken while Sam waits for his credentials" failed.
#[test]
fn a_candidate_that_comes_before_the_credentials_is_kept_for_the_connection() {
    crate::config::keep_saves_off_disk();
    let (rae_id, sam_id) = (identity(189), identity(190));
    let mut sam = app(190, "ice-sam", &rae_id, "Rae", true);
    let rae = app(189, "ice-rae", &sam_id, "Sam", true);
    let id = offer_file(&mut sam, &rae.key, 300_000);
    frames(&sam.sent);
    let one = "candidate:1 1 udp 16777215 127.0.0.1 40001 typ relay raddr 0.0.0.0 rport 0";
    let two = "candidate:2 1 udp 16777215 127.0.0.1 40002 typ relay raddr 0.0.0.0 rport 0";
    assert!(!on_signal(&mut sam.gs, &dc_ice_from(&rae.key, one)), "no dial yet: the WebRTC manager's");
    assert!(on_signal(&mut sam.gs, &dc_offer_from(&rae.key, &id)));
    assert_eq!(credentials_asked(&frames(&sam.sent)), vec![(id.clone(), rae.key.clone())]);
    assert!(on_signal(&mut sam.gs, &dc_ice_from(&rae.key, one)), "taken while Sam waits for his credentials");
    assert!(on_signal(&mut sam.gs, &dc_ice_from(&rae.key, two)));
    let kept: Vec<String> = sam.gs.files_flow.transfers.pair(&rae.key).early_ice.clone();
    assert_eq!(kept.len(), 2, "both kept");
    assert!(kept[0].contains("40001") && kept[1].contains("40002"), "in order: {kept:?}");

    // The credentials come (a forwarder address nothing answers on: only the hand-over is looked
    // at here; the test below runs a whole transfer this way).
    let granted = json!({ "type": "call_credentials", "file": id, "urls": ["turn:127.0.0.1:9?transport=udp"], "username": "1:s", "credential": "ps" });
    assert!(on_credentials(&mut sam.gs, &granted));
    let pair = sam.gs.files_flow.transfers.pair(&rae.key);
    assert!(pair.link.is_some(), "the connection starts");
    assert!(pair.early_ice.is_empty(), "with the candidates handed to it");
    assert!(on_signal(&mut sam.gs, &dc_ice_from(&rae.key, two)), "once it stands, straight to it");
    assert!(sam.gs.files_flow.transfers.pair(&rae.key).early_ice.is_empty());
    tidy(&[&sam, &rae]);
}

/// END TO END through a forwarder on this machine (the test forwarder, which passes data only to
/// an allocation holding a permission for the sender's relayed address, and here holds exactly two
/// relayed addresses, one a side):
/// - RULE 7 OF THE SECOND REVIEW, one dial, one allocation, every file. Sam has five live offers
///   to Rae; she accepted two. She dials with one of them, the relay names it on her `dc_offer`,
///   and Sam asks for that transfer's credentials alone: one allocation, one relayed candidate in
///   his answer. The other accepted file rides the same connection as its own channel, and both
///   arrive whole.
/// - RULE 9, a dial is an answer: Rae's answer note for one of the two never reached Sam. His
///   record of it went from Offered to Done all the same (the channel opening counted as Accept,
///   `done` as Sent).
/// - RULE 3 OF THE FIRST REVIEW: Rae's offer carries no candidate, and her one relayed candidate
///   follows as a `dc_ice` that reaches Sam before his credentials do (a browser trickles its
///   candidates this way). Sam keeps it and his connection applies it once its allocation exists.
///
/// Nothing goes around the forwarder.
/// Seen red 2026-10-12 with the connection leaving `Setup::early_ice` unapplied (the loop in
/// `Link::new` filtered empty): "Rae's files came" failed after 60 seconds, the forwarder refusing
/// "data from 127.0.0.1:... without the receiver's permission" again and again (the defect the
/// first review found). Seen red 2026-10-12 with `on_signal` answering a dial with the newest live
/// offer whatever the dial named (as if it named none, the second review's defect): "Sam asks for
/// the transfer the dial names, and no other" got the fifth offer.
#[test]
fn one_dial_one_allocation_carries_every_file_with_a_late_candidate() {
    use crate::net::webrtc::test_forwarder::{Forwarder, Trap};
    crate::config::keep_saves_off_disk();
    let (rae_id, sam_id) = (identity(191), identity(192));
    let mut sam = app(192, "trickle-sam", &rae_id, "Rae", true);
    let mut rae = app(191, "trickle-rae", &sam_id, "Sam", true);
    let lens = [100_000, fo::PIECE_SIZE as usize * 2 + 70_000, 150_000, fo::PIECE_SIZE as usize + 9_000, 120_000];
    let offers: Vec<String> = lens
        .iter()
        .map(|&len| {
            std::thread::sleep(Duration::from_millis(3));
            offer_file(&mut sam, &rae.key, len)
        })
        .collect();
    deliver(&sam, &mut rae);
    let (one, two) = (offers[1].clone(), offers[3].clone());
    assert!(accept(&mut rae.gs, &one, &rae.dir.join("dl"), "one.mp4"));
    deliver(&rae, &mut sam);
    assert!(accept(&mut rae.gs, &two, &rae.dir.join("dl"), "two.mp4"));
    frames(&rae.sent); // the answer for `two` is lost on its way to Sam
    assert_eq!((rec(&sam, &one).stage, rec(&sam, &two).stage), (Stage::Accepted, Stage::Offered));

    pump(&mut rae.gs);
    let asked = credentials_asked(&frames(&rae.sent));
    assert_eq!(asked.len(), 1, "Rae dials with one credential: {asked:?}");
    let dialled = asked[0].0.clone();
    assert!(dialled == one || dialled == two, "with one of the two she accepted");
    let traps: Vec<Trap> = (0..2).map(|_| Trap::bind()).collect();
    let fwd = Forwarder::start(traps.iter().map(|t| t.addr).collect());
    fwd.add_account("1:r", "pr", &format!("file:{dialled}"));
    fwd.add_account("1:s", "ps", &format!("file:{dialled}"));
    let grant = |user: &str, cred: &str| json!({ "type": "call_credentials", "file": dialled, "urls": [format!("turn:{}?transport=udp", fwd.addr)], "username": user, "credential": cred });
    assert!(on_credentials(&mut rae.gs, &grant("1:r", "pr")));

    let mut trickled = 0;
    let mut sdps: Vec<String> = Vec::new();
    let all_done = |rae: &App, sam: &App| [&one, &two].iter().all(|id| rec(rae, id).stage == Stage::Done && rec(sam, id).stage == Stage::Done);
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline && !all_done(&rae, &sam) {
        pump(&mut rae.gs);
        for f in frames(&rae.sent).into_iter().filter(|f| f["type"] == "webrtc_signal").map(|f| via_relay(&f)) {
            if f["signal_type"] != "dc_offer" {
                assert!(on_signal(&mut sam.gs, &f));
                continue;
            }
            assert_eq!(f["file"], dialled.as_str(), "the relay names the dial's transfer");
            // The offer goes without its candidates; they follow it, before Sam's credentials.
            let mut inner: Value = serde_json::from_str(f["data"].as_str().unwrap()).unwrap();
            let sdp = inner["sdp"].as_str().unwrap().to_string();
            let (candidates, rest): (Vec<&str>, Vec<&str>) = sdp.split("\r\n").partition(|l| l.starts_with("a=candidate:"));
            inner["sdp"] = Value::String(rest.join("\r\n"));
            sdps.push(sdp.clone());
            let mut offer = f.clone();
            offer["data"] = Value::String(inner.to_string());
            assert!(on_signal(&mut sam.gs, &offer), "Rae's dial is Sam's file connection");
            assert_eq!(credentials_asked(&frames(&sam.sent)), vec![(dialled.clone(), rae.key.clone())], "Sam asks for the transfer the dial names, and no other");
            for line in candidates {
                assert!(on_signal(&mut sam.gs, &dc_ice_from(&rae.key, line.trim_start_matches("a="))), "the candidate is the file connection's");
                trickled += 1;
            }
            assert!(on_credentials(&mut sam.gs, &grant("1:s", "ps")));
        }
        pump(&mut sam.gs);
        for f in frames(&sam.sent).into_iter().filter(|f| f["type"] == "webrtc_signal").map(|f| via_relay(&f)) {
            if let Some(sdp) = serde_json::from_str::<Value>(f["data"].as_str().unwrap()).ok().and_then(|v| v["sdp"].as_str().map(str::to_string)) {
                sdps.push(sdp);
            }
            assert!(on_signal(&mut rae.gs, &f), "Sam's answer is Rae's file connection's");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(trickled, 1, "Rae's one relayed candidate went after her offer");
    for (id, len) in [(&one, lens[1]), (&two, lens[3])] {
        assert_eq!(rec(&rae, id).stage, Stage::Done, "Rae's files came: refused {:?}", fwd.refused());
        let saved = PathBuf::from(rec(&rae, id).path);
        assert_eq!(std::fs::read(&saved).unwrap(), (0..len).map(|i| (i % 251) as u8).collect::<Vec<u8>>(), "whole");
    }
    assert_eq!(rec(&sam, &one).stage, Stage::Done, "Sam's side says it went");
    assert_eq!(rec(&sam, &two).stage, Stage::Done, "and the one whose answer was lost too (rule 9)");
    assert_eq!(words(&sam, &two, "Rae"), "Sent");
    for id in [&offers[0], &offers[2], &offers[4]] {
        assert_eq!(rec(&sam, id).stage, Stage::Offered, "the offers Rae did not take wait on");
    }
    assert_eq!(sdps.len(), 2, "one offer and one answer for both files: {sdps:?}");
    let candidates = |sdp: &str| sdp.lines().filter(|l| l.starts_with("a=candidate:")).count();
    assert_eq!((candidates(&sdps[0]), candidates(&sdps[1])), (1, 1), "one relayed address each side: one allocation");
    assert!(fwd.delivered() > 0);
    for t in &traps {
        assert!(t.received().is_empty(), "nothing went straight to {}", t.addr);
    }
    tidy(&[&sam, &rae]);
}

/// RULE 9 OF THE SECOND REVIEW, A DIAL IS AN ANSWER, and the engine's half of RULE 8, through a
/// stand-in connection whose news the test plays. Sam offered Rae two files and her answer notes
/// never reached him (both still Offered).
/// - The first one's channel opening is her answer: the record goes to Accepted and the row says
///   it is sending, not waiting. A pause she held shows, and her closing the channel clears it and
///   the row stops saying it is sending. `done` makes it Sent.
/// - The second one's channel opens, then she closes it and stays: the connection ends it with
///   "Rae's side stopped it", which the row says, and the cancel note goes to Rae (carrying no
///   pass: Sam is its issuer), so her row ends too ("Sam cancelled").
///
/// Seen red 2026-10-12 with `pump_link` taking `TransferOpen` as before the review (open only, the
/// record left Offered): "the channel opening is the answer" failed, the row still saying "Waiting
/// for Rae to accept".
#[test]
fn a_sender_still_waiting_for_the_answer_takes_the_channel_as_it() {
    crate::config::keep_saves_off_disk();
    let (rae_id, sam_id) = (identity(193), identity(194));
    let mut sam = app(194, "answer-sam", &rae_id, "Rae", true);
    let mut rae = app(193, "answer-rae", &sam_id, "Sam", true);
    let one = offer_file(&mut sam, &rae.key, 300_000);
    let two = offer_file(&mut sam, &rae.key, 200_000);
    deliver(&sam, &mut rae);
    for id in [&one, &two] {
        assert!(accept(&mut rae.gs, id, &rae.dir.join("dl"), "clip.mp4"));
    }
    frames(&rae.sent); // both answers lost
    assert_eq!(words(&sam, &one, "Rae"), "Waiting for Rae to accept");

    let (link, _cmds, news) = file_link::FileLink::stand_in();
    let pair = sam.gs.files_flow.transfers.pair(&rae.key);
    pair.link = Some(link);
    pair.on_link = [one.clone(), two.clone()].into_iter().collect();
    pair.phase = Phase::Connecting;
    let play = |sam: &mut App, ev: LinkEvent| {
        news.send(ev).unwrap();
        pump(&mut sam.gs);
    };
    play(&mut sam, LinkEvent::Open);
    play(&mut sam, LinkEvent::TransferOpen(one.clone()));
    assert_eq!(rec(&sam, &one).stage, Stage::Accepted, "the channel opening is the answer");
    assert_eq!(words(&sam, &one, "Rae"), "Sending through the server's call relay");
    play(&mut sam, LinkEvent::PausedByThem(one.clone(), true));
    assert_eq!(words(&sam, &one, "Rae"), "Rae paused it");
    play(&mut sam, LinkEvent::TransferClosed(one.clone()));
    assert_eq!(words(&sam, &one, "Rae"), "Connecting through the server's call relay", "the pause she held is cleared with the channel");
    play(&mut sam, LinkEvent::TransferOpen(one.clone()));
    play(&mut sam, LinkEvent::Ended(one.clone(), End::Done));
    assert_eq!(rec(&sam, &one).stage, Stage::Done);
    assert_eq!(words(&sam, &one, "Rae"), "Sent");
    assert!(puts_to(&frames(&sam.sent), &rae.key).is_empty(), "done needs no note");

    play(&mut sam, LinkEvent::TransferOpen(two.clone()));
    play(&mut sam, LinkEvent::TransferClosed(two.clone()));
    play(&mut sam, LinkEvent::Ended(two.clone(), End::Failed(crate::net::file_transfer::stopped_line("Rae"))));
    assert_eq!(words(&sam, &two, "Rae"), "Rae's side stopped it");
    let out = deliver(&sam, &mut rae);
    let to_rae = puts_to(&out, &rae.key);
    assert_eq!(to_rae.len(), 1, "an end is always said: the cancel note goes");
    assert!(to_rae[0].get("file_pass").is_none(), "carrying no pass");
    assert_eq!(fo::parse_note(&open(&to_rae[0], &rae.seed).text), Some(fo::Note::Cancel(OfferCancel { id: two.clone() })));
    assert_eq!(words(&rae, &two, "Sam"), "Sam cancelled", "Rae's row ends too");
    tidy(&[&sam, &rae]);
}

/// RULE 8 OFFLINE (the review of the merged code, 2026-10-12): a cancel or failure note that cannot
/// go because the chat is not connected is kept with its transfer's record, is still there in the
/// DM store read back from disk (a restart keeps it), and goes once the server is connected and
/// signed in again: nothing while the new connection's mailbox is still being read, then each note
/// once, carrying what it would have carried (the receiver's, the offer's pass). Sam cancels one
/// transfer and another fails on its channel while he is offline; Rae, offline too, cancels a
/// third. Each side's rows end on the other's. Remove keeps a row whose note has not gone, saying
/// why, and takes it off once it has.
/// Seen red 2026-10-12 with `cancel` sending its note as before (`let _ = send_note(...)`, nothing
/// kept): "the cancel is kept with the record" failed, the list empty.
#[test]
fn a_note_that_cannot_go_is_kept_and_sent_once_connected_again() {
    use crate::net::mailbox_fetch::MailboxFetch;
    crate::config::keep_saves_off_disk();
    let (rae_id, sam_id) = (identity(195), identity(196));
    let mut sam = app(196, "unsaid-sam", &rae_id, "Rae", true);
    let mut rae = app(195, "unsaid-rae", &sam_id, "Sam", true);
    let one = offer_file(&mut sam, &rae.key, 300_000);
    let two = offer_file(&mut sam, &rae.key, 200_000);
    let three = offer_file(&mut sam, &rae.key, 100_000);
    deliver(&sam, &mut rae);
    for id in [&one, &two, &three] {
        assert!(accept(&mut rae.gs, id, &rae.dir.join("dl"), "clip.mp4"));
    }
    deliver(&rae, &mut sam);

    // Both chats go offline.
    let (sam_socket, rae_socket) = (sam.gs.ws_client.take(), rae.gs.ws_client.take());
    cancel(&mut sam.gs, &one);
    on_end(&mut sam.gs, &two, End::Failed("clip.mp4 could not be read on this device any more.".into()));
    cancel(&mut rae.gs, &three);
    assert_eq!(rec(&sam, &one).stage, Stage::Cancelled { by_me: true });
    assert_eq!(rec(&sam, &one).unsaid, vec![OfferCancel { id: one.clone() }.text()], "the cancel is kept with the record");
    assert_eq!(rec(&sam, &two).unsaid, vec![OfferCancel { id: two.clone() }.text()], "and the failure's note");
    assert_eq!(rec(&rae, &three).unsaid, vec![OfferCancel { id: three.clone() }.text()], "and the receiver's");
    let server = crate::gui::pages::chat::norm_server_url(&sam.gs.server_url);
    let reread = DmStore::load(&sam.seed, &sam.key, &server);
    let kept: Vec<usize> = [&one, &two].iter().map(|id| reread.file_offers().iter().find(|r| &r.offer.id == *id).map(|r| r.unsaid.len()).unwrap_or(0)).collect();
    assert_eq!(kept, vec![1, 1], "kept on disk, so a restart keeps them");
    remove(&mut sam.gs, &one);
    assert!(record(&sam.gs, &one).is_some(), "Remove keeps a row whose note waits");
    assert!(sam.gs.pending_notices.iter().any(|n| n == "clip.mp4 stays on the list until this server is connected again, so Rae hears that it ended."), "{:?}", sam.gs.pending_notices);
    pump(&mut sam.gs);

    // Connected again, the new connection's mailbox not yet read: nothing yet.
    sam.gs.ws_client = sam_socket;
    rae.gs.ws_client = rae_socket;
    sam.gs.dm_fetch = MailboxFetch::default();
    pump(&mut sam.gs);
    assert!(frames(&sam.sent).is_empty(), "nothing before the mailbox is read");

    sam.gs.dm_fetch = MailboxFetch::already_read();
    pump(&mut sam.gs);
    pump(&mut rae.gs);
    let out = deliver(&sam, &mut rae);
    let to_rae = puts_to(&out, &rae.key);
    let notes: Vec<Option<fo::Note>> = to_rae.iter().map(|p| fo::parse_note(&open(p, &rae.seed).text)).collect();
    assert_eq!(notes, vec![Some(fo::Note::Cancel(OfferCancel { id: one.clone() })), Some(fo::Note::Cancel(OfferCancel { id: two.clone() }))], "each note once");
    assert_eq!(puts_to(&out, &sam.key).len(), 2, "with their self-copies");
    assert!(to_rae.iter().all(|p| p.get("file_pass").is_none()), "the sender's carry no pass");
    let back = deliver(&rae, &mut sam);
    let to_sam = puts_to(&back, &sam.key);
    assert_eq!(to_sam.len(), 1, "Rae's kept cancel goes");
    assert_eq!(to_sam[0]["file_pass"], serde_json::to_value(&rec(&rae, &three).offer.pass).unwrap(), "carrying the offer's pass, as it would have");
    for id in [&one, &two, &three] {
        assert!(rec(&sam, id).unsaid.is_empty() && rec(&rae, id).unsaid.is_empty(), "nothing left kept");
    }
    assert_eq!(words(&rae, &one, "Sam"), "Sam cancelled", "Rae's rows end");
    assert_eq!(rec(&rae, &two).stage, Stage::Cancelled { by_me: false });
    assert_eq!(words(&sam, &three, "Rae"), "Rae cancelled", "and Sam's");
    pump(&mut sam.gs);
    pump(&mut rae.gs);
    assert!(frames(&sam.sent).is_empty() && frames(&rae.sent).is_empty(), "and only once");
    remove(&mut sam.gs, &one);
    assert!(record(&sam.gs, &one).is_none(), "its note gone, Remove takes the row off");
    tidy(&[&sam, &rae]);
}

/// RULE 8 ON A CONNECTION THAT STANDS (the review of the merged code, 2026-10-12), through a
/// stand-in connection: it carries one of Sam's transfers to Rae when Rae opens the channel of
/// another that Sam cancelled (his cancel note lost). The connection asks (`Wanted`), and Sam's app
/// hands that transfer over as already ended (`Cmd::AddEnded`, so the connection writes `cancel`
/// on the channel) and sends the cancel note again: Rae's row ends. A channel she opens for it
/// again is answered the same way with no second note (once in this run), and one opened for a
/// transfer that ended done gets nothing (nothing to say).
/// Seen red 2026-10-12 with `pump_link` taking `Wanted` as before (an ended record ignored): "the
/// connection is told to answer it with cancel" failed.
#[test]
fn a_channel_opened_for_an_ended_transfer_is_answered_with_cancel() {
    crate::config::keep_saves_off_disk();
    let (rae_id, sam_id) = (identity(197), identity(198));
    let mut sam = app(198, "ended-sam", &rae_id, "Rae", true);
    let mut rae = app(197, "ended-rae", &sam_id, "Sam", true);
    let one = offer_file(&mut sam, &rae.key, 300_000);
    let two = offer_file(&mut sam, &rae.key, 200_000);
    let three = offer_file(&mut sam, &rae.key, 100_000);
    deliver(&sam, &mut rae);
    for id in [&one, &two, &three] {
        assert!(accept(&mut rae.gs, id, &rae.dir.join("dl"), "clip.mp4"));
    }
    deliver(&rae, &mut sam);
    cancel(&mut sam.gs, &two);
    frames(&sam.sent); // the cancel note, lost on its way to Rae
    with_record(&mut sam.gs, &three, |r| r.stage = Stage::Done);

    let (link, cmds, news) = file_link::FileLink::stand_in();
    let pair = sam.gs.files_flow.transfers.pair(&rae.key);
    pair.link = Some(link);
    pair.on_link = [one.clone()].into_iter().collect();
    pair.phase = Phase::Open;
    let play = |sam: &mut App, ev: LinkEvent| {
        news.send(ev).unwrap();
        pump(&mut sam.gs);
    };
    play(&mut sam, LinkEvent::Wanted(two.clone()));
    match cmds.try_recv() {
        Ok(Cmd::AddEnded(t)) => assert_eq!(t.id, two, "for the transfer it names"),
        _ => panic!("the connection is told to answer it with cancel"),
    }
    let out = deliver(&sam, &mut rae);
    let to_rae = puts_to(&out, &rae.key);
    assert_eq!(to_rae.len(), 1, "the cancel note goes again");
    assert_eq!(fo::parse_note(&open(&to_rae[0], &rae.seed).text), Some(fo::Note::Cancel(OfferCancel { id: two.clone() })));
    assert_eq!(words(&rae, &two, "Sam"), "Sam cancelled", "Rae's row ends");

    play(&mut sam, LinkEvent::Wanted(two.clone()));
    assert!(matches!(cmds.try_recv(), Ok(Cmd::AddEnded(_))), "a channel opened for it again is answered again");
    assert!(frames(&sam.sent).is_empty(), "with no second note in this run");
    play(&mut sam, LinkEvent::Wanted(three.clone()));
    assert!(cmds.try_recv().is_err(), "a transfer that ended done: nothing to say");
    assert!(frames(&sam.sent).is_empty());
    assert_eq!(rec(&sam, &one).stage, Stage::Accepted, "the transfer on the connection goes on");
    tidy(&[&sam, &rae]);
}
