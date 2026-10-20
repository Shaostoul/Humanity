// Child of reach_tests.rs (#[path], so `use super::*` reaches its helpers and, through them,
// everything reach.rs has): a direct-connection offer let through by a FILE PASS (device-to-device
// big files, 2026-10-11, docs/design/files-showcase.md, "Device-to-device big files: the shapes,
// exact", part 2 and its Proof list). The receiver of a file dials its sender; the relay lets the
// `dc_offer` through when the pass it carries checks with the sender (the offer's target) as
// issuer and the receiver (the signed-in socket) as grantee, on this relay, now. A file of its own
// so the step B tests above stay readable.

use super::*;
use crate::relay::core::pq_crypto::{build_file_pass, file_pass_preimage};

/// Now, Unix seconds: `file_pass_of` checks on the relay's own clock.
fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
}

/// The file pass `issuer` gives `grantee` on this relay for transfer `id`, running out a day from
/// now, minted by the shipped builder: the JSON object a frame carries.
fn file_pass_from(st: &Arc<RelayState>, issuer: &Person, grantee: &str, id: &str) -> serde_json::Value {
    let server = st.db.server_did().expect("the relay's own did:hum");
    let now = unix_now();
    let text = build_file_pass(&issuer.seed, &server, &issuer.key, grantee, id, now + 86_400, now).expect("a pass");
    serde_json::from_str(&text).unwrap()
}

/// A pass signed by hand with `issuer`'s key over the words for `server`, ending at `expiry`: what
/// a modified app could send, which the shipped builder would refuse to mint.
fn signed_pass(issuer: &Person, server: &str, grantee: &str, id: &str, expiry: u64) -> serde_json::Value {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    let sig = B64.encode(issuer.keypair().sign(file_pass_preimage(server, &issuer.key, grantee, id, expiry).as_bytes()));
    serde_json::json!({ "v": 1, "id": id, "expiry": expiry, "sig": sig })
}

const ID: &str = "00112233445566778899aabbccddeeff";

/// THE RECEIVER OF A FILE DIALS ITS SENDER WITH THE SENDER'S FILE PASS, AND ONLY SO. Ann (the
/// sender, whose app minted the pass) and Bo (the receiver) are strangers to each other: no
/// friendship pass, no shared group or voice room, and Ann's settings at their defaults. Bo's
/// `dc_offer` to Ann carrying Ann's pass for him reaches her, without the pass (its wire form has
/// no such field). Refused, silently: a forged pass (Ann's words, someone else's signature), the
/// wrong way round (a pass Bo minted for Ann, presented by Bo; and Ann's pass for Bo presented by
/// Ann to Bo), a pass for another file (its id changed after signing), one run out, one longer
/// than a week, one given on another server, one Ann gave someone else, and none at all. A file
/// pass opens no call: Bo's call `offer` with it is dropped.
///
/// Seen red 2026-10-11 with `dc_offer_may_pass` checking the pass with the sender as issuer
/// (`file_pass_of(state, sender, to, ..)`): "Ann's pass for Bo lets Bo's offer reach her: heard
/// [], reached []".
#[test]
fn a_direct_connection_offer_with_a_file_pass_only_from_its_grantee() {
    let st = fresh_state("dc_offer_file_pass");
    let (ann, bo, cy) = (person(191), person(192), person(193));
    for p in [&ann, &bo, &cy] {
        connect(&st, &p.key);
    }
    let server = st.db.server_did().unwrap();
    let now = unix_now();
    let anns = file_pass_from(&st, &ann, &bo.key, ID);

    // What a dc_offer from `from` to `to` carrying `pass` did: (what `from` heard, what `to` got).
    let offer = |from: &Person, to: &Person, signal: &str, pass: Option<serde_json::Value>| -> (Vec<String>, Vec<RelayMessage>) {
        let mut rx = st.broadcast_tx.subscribe();
        block(handle_webrtc_signal(&st, &from.key, to.key.clone(), signal.to_string(), serde_json::json!({ "sdp": "x" }), None, pass));
        let mut all = Vec::new();
        while let Ok(m) = rx.try_recv() {
            all.push(m);
        }
        let reached = all.iter().filter(|m| matches!(m, RelayMessage::WebrtcSignal { to: t, .. } if *t == to.key)).cloned().collect();
        (heard_in(&all, &from.key), reached)
    };

    let (heard, reached) = offer(&bo, &ann, "dc_offer", Some(anns.clone()));
    assert_eq!(reached.len(), 1, "Ann's pass for Bo lets Bo's offer reach her: heard {heard:?}, reached {reached:?}");
    let RelayMessage::WebrtcSignal { from, signal_type, file_pass, friend_cert, .. } = &reached[0] else { unreachable!() };
    assert_eq!((from.as_str(), signal_type.as_str()), (bo.key.as_str(), "dc_offer"));
    assert!(file_pass.is_none() && friend_cert.is_none(), "the offer reaches Ann without the pass");
    assert!(!serde_json::to_string(&reached[0]).unwrap().contains("file_pass"), "and its wire form has no such field");

    let dropped = |what: &str, from: &Person, to: &Person, signal: &str, pass: Option<serde_json::Value>| {
        let (heard, reached) = offer(from, to, signal, pass);
        assert!(heard.is_empty() && reached.is_empty(), "{what}: heard {heard:?}, reached {reached:?}");
    };
    let mut forged = anns.clone();
    forged["sig"] = signed_pass(&cy, &server, &bo.key, ID, now + 86_400)["sig"].clone();
    dropped("a forged pass", &bo, &ann, "dc_offer", Some(forged));
    dropped("a pass Bo minted for Ann", &bo, &ann, "dc_offer", Some(file_pass_from(&st, &bo, &ann.key, ID)));
    dropped("Ann's pass presented by Ann", &ann, &bo, "dc_offer", Some(anns.clone()));
    let mut other_file = anns.clone();
    other_file["id"] = "ffeeddccbbaa99887766554433221100".into();
    dropped("a pass for another file", &bo, &ann, "dc_offer", Some(other_file));
    dropped("a pass run out", &bo, &ann, "dc_offer", Some(signed_pass(&ann, &server, &bo.key, ID, now - 1)));
    dropped("a pass of thirty days", &bo, &ann, "dc_offer", Some(signed_pass(&ann, &server, &bo.key, ID, now + 30 * 86_400)));
    dropped("a pass given on another server", &bo, &ann, "dc_offer", Some(signed_pass(&ann, "did:hum:another", &bo.key, ID, now + 86_400)));
    dropped("Ann's pass for Cy, presented by Bo", &bo, &ann, "dc_offer", Some(file_pass_from(&st, &ann, &cy.key, ID)));
    dropped("no pass", &bo, &ann, "dc_offer", None);
    dropped("a pass as text", &bo, &ann, "dc_offer", Some(serde_json::Value::String(anns.to_string())));
    dropped("a call offer with a file pass", &bo, &ann, "offer", Some(anns.clone()));
    assert!(!call_is_open(&st, &ann.key, &bo.key), "a file pass opens no call");

    // The frame as an app sends it: `file_pass` is the object, read off the wire.
    let frame: RelayMessage = serde_json::from_value(serde_json::json!({
        "type": "webrtc_signal", "to": ann.key, "signal_type": "dc_offer", "data": { "sdp": "x" }, "file_pass": anns,
    }))
    .unwrap();
    assert!(matches!(frame, RelayMessage::WebrtcSignal { file_pass: Some(ref p), .. } if p["id"] == ID), "the frame carries the pass to the relay");
}

/// THE RECEIVER'S ANSWER REACHES A SENDER WHO TAKES MESSAGES ONLY FROM FRIENDS, WITH THE PASS (the
/// review's rule 4, 2026-10-12, found in a real browser run: Ben's "yes" never reached Ann). Ann
/// (the sender) takes messages from friends only, the default; Bo (the receiver) is a stranger.
/// Bo's DM to Ann is refused without the pass and delivered with Ann's pass for him, at a
/// stranger's knock; a pass Bo minted himself, a pass Ann gave Cy, and any pass when Ann takes
/// messages from nobody, are refused.
///
/// Seen red 2026-10-12 with `by_file_pass` left out of `dm_gate`'s let-in: "with Ann's pass for
/// him, delivered" failed.
#[test]
fn an_answer_with_the_file_pass_reaches_a_sender_who_takes_messages_from_friends_only() {
    let st = fresh_state("dm_put_file_pass");
    let (ann, bo, cy) = (person(201), person(202), person(203));
    for p in [&ann, &bo, &cy] {
        connect(&st, &p.key);
    }
    let anns = file_pass_from(&st, &ann, &bo.key, ID);
    let put = |pass: Option<serde_json::Value>| -> bool {
        let before = st.db.mailbox_fetch(&ann.key, 0, 1_000).unwrap().len();
        block(async {
            st.dm_rate.forget(&bo.key);
            handle_dm_put(&st, &bo.key, ann.key.clone(), envelope(), None, DmAsk::Ordinary, None, pass).await;
        });
        st.db.mailbox_fetch(&ann.key, 0, 1_000).unwrap().len() > before
    };
    assert!(!put(None), "without a pass, refused (Ann takes messages from friends only)");
    assert!(put(Some(anns.clone())), "with Ann's pass for him, delivered");
    assert!(!put(Some(file_pass_from(&st, &bo, &ann.key, ID))), "a pass Bo minted for Ann is not Ann's consent");
    assert!(!put(Some(file_pass_from(&st, &ann, &cy.key, ID))), "a pass Ann gave Cy is not for Bo");
    set(&st, &ann.key, Kind::Message, Audience::Nobody);
    assert!(!put(Some(anns)), "Ann takes messages from nobody: her choice stands");
}

/// A DIAL LET IN BY A FILE PASS TELLS ITS TARGET WHICH TRANSFER IT IS FOR (the second review of
/// 2026-10-12): the forwarded `dc_offer` carries the pass's id as `file`, so the file's sender asks
/// the forwarder for that one transfer instead of every offer it has open. A dial let in another way
/// (a friendship pass here: Ann's own other device) carries none, nor does any other signal, and a
/// `file` a client puts on its frame is never passed on.
///
/// Seen red 2026-10-12 with `file` always None in `handle_webrtc_signal`: "the dial names its
/// transfer" failed.
#[test]
fn a_dial_let_in_by_a_file_pass_names_its_transfer() {
    let st = fresh_state("dc_offer_names_file");
    let (ann, bo) = (person(211), person(212));
    for p in [&ann, &bo] {
        connect(&st, &p.key);
    }
    let anns = file_pass_from(&st, &ann, &bo.key, ID);
    let forwarded = |from: &Person, to: &Person, signal: &str, pass: Option<serde_json::Value>| -> Vec<serde_json::Value> {
        let mut rx = st.broadcast_tx.subscribe();
        block(handle_webrtc_signal(&st, &from.key, to.key.clone(), signal.to_string(), serde_json::json!({ "sdp": "x" }), None, pass));
        let mut out = Vec::new();
        while let Ok(m) = rx.try_recv() {
            if matches!(m, RelayMessage::WebrtcSignal { .. }) {
                out.push(serde_json::to_value(&m).unwrap());
            }
        }
        out
    };
    let dial = forwarded(&bo, &ann, "dc_offer", Some(anns.clone()));
    assert_eq!(dial.len(), 1);
    assert_eq!(dial[0]["file"], ID, "the dial names its transfer: {}", dial[0]);
    let ice = forwarded(&bo, &ann, "dc_ice", Some(anns.clone()));
    assert!(ice[0].get("file").is_none(), "a candidate names none: {}", ice[0]);
    // A dial let in another way (Ann's own key: her other device) is forwarded, naming none.
    let own = forwarded(&ann, &ann, "dc_offer", None);
    assert_eq!(own.len(), 1, "her own device's dial is forwarded");
    assert!(own[0].get("file").is_none(), "a dial let in another way names none: {}", own[0]);
    // A client's own `file` on its frame never reaches the handler: features.rs
    // `a_clients_own_file_on_a_signal_is_never_passed_on` proves it through the real socket path.
}
