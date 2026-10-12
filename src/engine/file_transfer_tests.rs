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
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
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

fn dc_offer_from(who: &str) -> Value {
    json!({ "type": "webrtc_signal", "from": who, "signal_type": "dc_offer", "data": json!({ "type": "offer", "sdp": "v=0\r\n" }).to_string() })
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

/// SIGNALS CARRY NO TRANSFER ID (the shape agreed with the web): a `dc_offer` from someone Sam has
/// live offers to is their file connection, and Sam asks for credentials for EACH of those offers
/// (newest first, at most four), since he cannot know which one Rae dialled with; a `dc_offer` from
/// anyone else, and a `dc_answer` or `dc_ice` from someone with no file connection standing or
/// being answered, are left for the app's WebRTC manager.
/// Seen red 2026-10-11 with `live_offers_to` taking only the newest offer: "credentials for each
/// live offer" got one.
#[test]
fn a_dial_is_answered_with_credentials_for_each_live_offer() {
    crate::config::keep_saves_off_disk();
    let (rae_id, sam_id) = (identity(179), identity(180));
    let mut sam = app(180, "signal-sam", &rae_id, "Rae", true);
    let mut rae = app(179, "signal-rae", &sam_id, "Sam", true);
    let older = offer_file(&mut sam, &rae.key, 300_000);
    std::thread::sleep(Duration::from_millis(5));
    let newer = offer_file(&mut sam, &rae.key, 200_000);
    deliver(&sam, &mut rae);
    assert!(!on_signal(&mut sam.gs, &dc_offer_from(&identity(181).1)), "from someone with no offer, the WebRTC manager's");
    assert!(frames(&sam.sent).is_empty());
    assert!(on_signal(&mut sam.gs, &dc_offer_from(&rae.key)), "Rae's dial is the file connection");
    assert_eq!(credentials_asked(&frames(&sam.sent)), vec![(newer.clone(), rae.key.clone()), (older.clone(), rae.key.clone())], "credentials for each live offer");
    let ice = json!({ "type": "webrtc_signal", "from": rae.key, "signal_type": "dc_ice", "data": json!({ "candidate": "" }).to_string() });
    assert!(!on_signal(&mut rae.gs, &ice), "no file connection on Rae's side: the WebRTC manager's");
    assert!(on_credentials(&mut sam.gs, &json!({ "type": "call_credentials", "file": newer, "refused": true })));
    assert!(matches!(sam.gs.files_flow.transfers.phase(&rae.key), Phase::Asking(_)), "still waiting for the other answer");
    assert!(on_credentials(&mut sam.gs, &json!({ "type": "call_credentials", "file": older, "refused": true })));
    assert!(matches!(sam.gs.files_flow.transfers.phase(&rae.key), Phase::NotSetUp(_)), "both refused: not set up");
    tidy(&[&sam, &rae]);
}

/// BOTH DIAL AT ONCE (the shape agreed with the web): Sam offered Rae a file and Rae offered Sam
/// one, both accepted, so each dials the other. When each one's `dc_offer` reaches the other, the
/// larger key (string compare of the Dilithium hex) keeps its dial and ignores the other's, and the
/// smaller drops its own attempt and answers, asking for credentials for its live offers. A
/// candidate of the dial set aside is taken by the winner and dropped (rule 3 of the first review:
/// a file connection's signal is never the call code's).
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

    assert!(on_signal(&mut rae.gs, &dc_offer_from(&sam.key)));
    assert!(on_signal(&mut sam.gs, &dc_offer_from(&rae.key)));
    let (rae_out, sam_out) = (credentials_asked(&frames(&rae.sent)), credentials_asked(&frames(&sam.sent)));
    // The one who yields asks for the credentials of its own live offers to the winner.
    let (winner, loser_out, want) = if rae.key > sam.key { ("Rae", &sam_out, (to_rae.clone(), rae.key.clone())) } else { ("Sam", &rae_out, (to_sam.clone(), sam.key.clone())) };
    let winner_out = if winner == "Rae" { &rae_out } else { &sam_out };
    assert!(winner_out.is_empty(), "the larger key ({winner}) ignores the other's dial");
    assert_eq!(loser_out, &vec![want], "exactly one side yields, asking for its own live offers' credentials");
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

/// RULE 4 OF THE FIRST REVIEW (media-showcase.md): the receiver's answer (yes or no) and its cancel
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
    assert!(on_signal(&mut sam.gs, &dc_offer_from(&rae.key)));
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

/// RULE 3, END TO END, through a forwarder on this machine (the test forwarder, which passes data
/// only to an allocation holding a permission for the sender's relayed address): Rae dials Sam,
/// her offer carries no candidate, and her one relayed candidate follows as a `dc_ice` that
/// reaches Sam before his credentials do (a browser trickles its candidates this way). Sam keeps
/// it, his connection applies it once its allocation exists, and the file crosses: Rae's copy is
/// saved whole and both records say Done. Nothing goes around the forwarder.
/// Seen red 2026-10-12 with the connection leaving `Setup::early_ice` unapplied (the loop in
/// `Link::new` filtered empty): "Rae's file came" failed after 60 seconds with the record still
/// Accepted, the forwarder refusing "data from 127.0.0.1:... without the receiver's permission"
/// again and again (the defect the review found). With `on_signal` as before the review the test
/// stops sooner, at "the candidate is the file connection's".
#[test]
fn a_dial_whose_candidate_comes_after_its_offer_connects() {
    use crate::net::webrtc::test_forwarder::{Forwarder, Trap};
    crate::config::keep_saves_off_disk();
    let (rae_id, sam_id) = (identity(191), identity(192));
    let mut sam = app(192, "trickle-sam", &rae_id, "Rae", true);
    let mut rae = app(191, "trickle-rae", &sam_id, "Sam", true);
    let len = fo::PIECE_SIZE as usize * 2 + 70_000;
    let id = offer_file(&mut sam, &rae.key, len);
    deliver(&sam, &mut rae);
    assert!(accept(&mut rae.gs, &id, &rae.dir.join("dl"), "clip.mp4"));
    deliver(&rae, &mut sam);
    assert_eq!(rec(&sam, &id).stage, Stage::Accepted);

    let traps: Vec<Trap> = (0..2).map(|_| Trap::bind()).collect();
    let fwd = Forwarder::start(traps.iter().map(|t| t.addr).collect());
    fwd.add_account("1:r", "pr", &format!("file:{id}"));
    fwd.add_account("1:s", "ps", &format!("file:{id}"));
    let grant = |user: &str, cred: &str| json!({ "type": "call_credentials", "file": id, "urls": [format!("turn:{}?transport=udp", fwd.addr)], "username": user, "credential": cred });

    pump(&mut rae.gs);
    assert_eq!(credentials_asked(&frames(&rae.sent)), vec![(id.clone(), sam.key.clone())], "Rae dials");
    assert!(on_credentials(&mut rae.gs, &grant("1:r", "pr")));

    let mut trickled = 0;
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline && !(rec(&rae, &id).stage == Stage::Done && rec(&sam, &id).stage == Stage::Done) {
        pump(&mut rae.gs);
        for f in frames(&rae.sent).into_iter().filter(|f| f["type"] == "webrtc_signal") {
            if f["signal_type"] != "dc_offer" {
                assert!(on_signal(&mut sam.gs, &f));
                continue;
            }
            // The offer goes without its candidates; they follow it, before Sam's credentials.
            let mut inner: Value = serde_json::from_str(f["data"].as_str().unwrap()).unwrap();
            let sdp = inner["sdp"].as_str().unwrap().to_string();
            let (candidates, rest): (Vec<&str>, Vec<&str>) = sdp.split("\r\n").partition(|l| l.starts_with("a=candidate:"));
            inner["sdp"] = Value::String(rest.join("\r\n"));
            let mut offer = f.clone();
            offer["data"] = Value::String(inner.to_string());
            assert!(on_signal(&mut sam.gs, &offer), "Rae's dial is Sam's file connection");
            assert_eq!(credentials_asked(&frames(&sam.sent)), vec![(id.clone(), rae.key.clone())]);
            for line in candidates {
                assert!(on_signal(&mut sam.gs, &dc_ice_from(&rae.key, line.trim_start_matches("a="))), "the candidate is the file connection's");
                trickled += 1;
            }
            assert!(on_credentials(&mut sam.gs, &grant("1:s", "ps")));
        }
        pump(&mut sam.gs);
        for f in frames(&sam.sent).into_iter().filter(|f| f["type"] == "webrtc_signal") {
            assert!(on_signal(&mut rae.gs, &f), "Sam's answer is Rae's file connection's");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(trickled, 1, "Rae's one relayed candidate went after her offer");
    assert_eq!(rec(&rae, &id).stage, Stage::Done, "Rae's file came: refused {:?}", fwd.refused());
    assert_eq!(rec(&sam, &id).stage, Stage::Done, "Sam's side says it went");
    let saved = PathBuf::from(rec(&rae, &id).path);
    assert_eq!(std::fs::read(&saved).unwrap(), (0..len).map(|i| (i % 251) as u8).collect::<Vec<u8>>(), "whole");
    assert!(fwd.delivered() > 0);
    for t in &traps {
        assert!(t.received().is_empty(), "nothing went straight to {}", t.addr);
    }
    tidy(&[&sam, &rae]);
}
