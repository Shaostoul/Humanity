// Child of reach_tests.rs (#[path], so `use super::*` reaches its helpers and, through them,
// everything reach.rs has): a direct-connection offer let through by a FILE PASS (device-to-device
// big files, 2026-10-11, docs/design/media-showcase.md, "Device-to-device big files: the shapes,
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
