// Child of api_v2_recovery.rs: the routes, called the way axum calls them. Each test was seen
// failing once; what it saw red is at the test.

use super::*;
use crate::relay::api_v2_objects::{SignedObjectSubmission, post_object};
use crate::relay::core::pq_crypto::{DilithiumKeypair, KyberKeypair, derive_dilithium_seed, derive_kyber_seed};
use crate::relay::core::recovery::{
    self as proto, OneTimeAsk, TrustedPerson, build_setup, object_json, open_answer, open_piece, seal_answer, served_piece,
};
use crate::relay::storage::Storage;

struct Person {
    secret: [u8; 32],
    dil: DilithiumKeypair,
    kyber: KyberKeypair,
    did: String,
}

fn person(b: u8) -> Person {
    let secret = [b; 32];
    let dil = DilithiumKeypair::from_seed(&derive_dilithium_seed(&secret));
    let kyber = KyberKeypair::from_seed(&derive_kyber_seed(&secret)).unwrap();
    let did = did_for_pubkey(&dil.public_key());
    Person { secret, dil, kyber, did }
}

fn state(tag: &str) -> Arc<RelayState> {
    Arc::new(RelayState::new(Storage::open_temp(&format!("recovery_api_{tag}"))))
}

async fn body(res: Response) -> (u16, serde_json::Value) {
    let code = res.status().as_u16();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 22).await.unwrap();
    (code, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

async fn post(st: &Arc<RelayState>, o: &crate::relay::core::object::Object) -> (u16, serde_json::Value) {
    let sub: SignedObjectSubmission = serde_json::from_value(object_json(o)).unwrap();
    body(post_object(State(st.clone()), Json(sub)).await.into_response()).await
}

/// `purpose\ntimestamp` signed the way both apps sign it (`pq_sign_chat`, `getPqSignedAuth`).
fn signed(p: &Person, purpose: &str, ts: u64) -> SignedBody {
    let sig = p.dil.sign(format!("{purpose}\n{ts}").as_bytes());
    SignedBody { key: hex::encode(p.dil.public_key()), timestamp: ts, sig: hex::encode(sig) }
}

/// THE WHOLE WAY THROUGH THE ROUTES: a setup posted through POST /api/v2/objects stands and is
/// served; a new device asks without any signature; a trusted person reads the ask by the code
/// as typed (lower case, dashes), checks the key against the code, and answers with their token;
/// the new device reads the answers and rebuilds the account; then it closes the ask.
///
/// Seen red 2026-10-21 with `get_ask` not reading the code through `normalize_code`: "the code is
/// read as typed: 404".
#[tokio::test]
async fn the_whole_way_through_the_routes() {
    let st = state("whole");
    let (holder, a, b, c) = (person(0x71), person(0x72), person(0x73), person(0x74));
    let trusted: Vec<TrustedPerson> = [&a, &b, &c]
        .iter()
        .map(|p| TrustedPerson { did: p.did.clone(), name: "Friend".into(), kyber_public: p.kyber.public_key() })
        .collect();
    let (_, objects) = build_setup(&holder.secret, &holder.dil, &holder.kyber.public_key(), &trusted, 2, now_ms()).unwrap();
    for o in &objects {
        let (code, v) = post(&st, o).await;
        assert_eq!(code, 200, "{v}");
    }
    let (code, setup) = body(get_recovery_setup(State(st.clone()), Path(holder.did.clone())).await).await;
    assert_eq!(code, 200);
    assert_eq!(setup["active"]["needed"], 2);
    let pieces: Vec<_> = setup["active"]["pieces"].as_array().unwrap().iter().map(|p| served_piece(&p["object"]).unwrap()).collect();

    // The new device asks.
    let ask = OneTimeAsk::new().unwrap();
    let ask_body = AskBody {
        holder_did: holder.did.clone(),
        one_time_key_b64: B64.encode(&ask.public),
        close_hash: proto::close_hash(&ask.close_secret),
    };
    let (code, opened) = body(open_ask(State(st.clone()), Json(ask_body)).await).await;
    assert_eq!(code, 200, "{opened}");
    let the_code = opened["code"].as_str().unwrap().to_string();
    assert_eq!(the_code, proto::ask_code(&holder.did, &ask.public));

    // A trusted person reads it by the code as typed, checks the key, answers.
    let typed = proto::show_code(&the_code).to_lowercase();
    let (code, seen) = body(get_ask(State(st.clone()), Path(typed)).await).await;
    assert_eq!(code, 200, "the code is read as typed: {code}");
    let key = B64.decode(seen["one_time_key_b64"].as_str().unwrap()).unwrap();
    assert_eq!(proto::ask_code(seen["holder_did"].as_str().unwrap(), &key), the_code, "the key matches the code");
    for (slot, who) in [(0usize, &a), (2, &c)] {
        let o = open_piece(&who.kyber, &pieces[slot]).unwrap();
        let sealed = seal_answer(&key, &the_code, &holder.did, slot, &o.share).unwrap();
        let ans = AnswerBody { code: the_code.clone(), slot, token_b64: B64.encode(o.token), sealed_b64: B64.encode(&sealed) };
        let (code, v) = body(answer(State(st.clone()), Json(ans)).await).await;
        assert_eq!(code, 200, "{v}");
    }
    let wrong = AnswerBody { code: the_code.clone(), slot: 1, token_b64: B64.encode([0u8; 32]), sealed_b64: B64.encode([0u8; proto::ANSWER_LEN]) };
    assert_eq!(body(answer(State(st.clone()), Json(wrong)).await).await.0, 403, "no token, no answer");

    // The new device rebuilds.
    let (_, seen) = body(get_ask(State(st.clone()), Path(the_code.clone())).await).await;
    let back: Vec<_> = seen["answers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let slot = r["slot"].as_u64().unwrap() as usize;
            open_answer(&ask.keys, &the_code, &holder.did, slot, &B64.decode(r["sealed_b64"].as_str().unwrap()).unwrap()).unwrap()
        })
        .collect();
    assert_eq!(proto::rebuild(&back, 2, &holder.did), Some(holder.secret));

    // The list shows the ask, then closing it with the device's secret takes it away.
    let (_, list) = body(list_asks(State(st.clone())).await).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert!(!list.to_string().contains(&the_code), "the list shows no code");
    let close_body = CloseBody { code: the_code.clone(), close_secret_b64: B64.encode(ask.close_secret) };
    assert_eq!(body(close(State(st.clone()), Json(close_body)).await).await.0, 200);
    assert_eq!(body(get_ask(State(st.clone()), Path(the_code)).await).await.0, 404);
}

/// THE OBJECT DOOR REFUSES WHAT IS NOT A GOOD PIECE: an altered signature is a 401, a malformed
/// payload a 400 with words, and neither is kept.
///
/// Seen red 2026-10-21 with "invalid recovery_share_v1" left out of `post_object`'s classes: "a
/// malformed piece is a 400, not a 500".
#[tokio::test]
async fn the_object_door_refuses_what_is_not_a_good_piece() {
    let st = state("door");
    let (holder, a, b) = (person(0x81), person(0x82), person(0x83));
    let trusted: Vec<TrustedPerson> =
        [&a, &b].iter().map(|p| TrustedPerson { did: p.did.clone(), name: "F".into(), kyber_public: p.kyber.public_key() }).collect();
    let (_, objects) = build_setup(&holder.secret, &holder.dil, &holder.kyber.public_key(), &trusted, 2, now_ms()).unwrap();
    let mut unsigned = objects[0].clone();
    unsigned.signature[0] ^= 0xff;
    assert_eq!(post(&st, &unsigned).await.0, 401, "an altered signature");
    let mut p = proto::SharePayload::parse(&objects[0].payload).unwrap();
    p.note.truncate(10);
    let bad = crate::relay::core::object::ObjectBuilder::new(proto::SHARE_OBJECT_TYPE)
        .created_at(now_ms())
        .payload_cbor(&p.to_cbor())
        .unwrap()
        .sign(&holder.dil)
        .unwrap();
    let (code, v) = post(&st, &bad).await;
    assert_eq!(code, 400, "a malformed piece is a 400, not a 500: {v}");
    assert!(v["error"].as_str().unwrap().contains("note"), "{v}");
    assert_eq!(body(get_recovery_setup(State(st.clone()), Path(holder.did.clone())).await).await.0, 404, "nothing was kept");
}

/// TAKING IT BACK AND "THAT WAS NOT ME" NEED THE PERSON'S OWN SIGNATURE: a withdrawal signed by
/// someone else, or replayed, is refused and leaves the setup standing; the person's own removes
/// it. Requests naming an account are closed by that account's signature, and by no one else's.
///
/// Seen red 2026-10-21 with the signature check taken out of `withdraw`: the forged withdrawal
/// was answered 200, not 401.
#[tokio::test]
async fn taking_it_back_needs_the_persons_own_signature() {
    let st = state("withdraw");
    let (holder, a, b, stranger) = (person(0x91), person(0x92), person(0x93), person(0x94));
    let trusted: Vec<TrustedPerson> =
        [&a, &b].iter().map(|p| TrustedPerson { did: p.did.clone(), name: "F".into(), kyber_public: p.kyber.public_key() }).collect();
    let (_, objects) = build_setup(&holder.secret, &holder.dil, &holder.kyber.public_key(), &trusted, 2, now_ms()).unwrap();
    for o in &objects {
        assert_eq!(post(&st, o).await.0, 200);
    }

    // "This was not me" from the account it names.
    let ask = OneTimeAsk::new().unwrap();
    let ask_body = AskBody { holder_did: holder.did.clone(), one_time_key_b64: B64.encode(&ask.public), close_hash: proto::close_hash(&ask.close_secret) };
    let (_, opened) = body(open_ask(State(st.clone()), Json(ask_body)).await).await;
    let code = opened["code"].as_str().unwrap().to_string();
    let (status, v) = body(not_me(State(st.clone()), Json(signed(&stranger, NOT_ME_PURPOSE, now_ms()))).await).await;
    assert_eq!((status, v["closed"].as_u64()), (200, Some(0)), "a stranger closes nothing of it");
    assert_eq!(body(get_ask(State(st.clone()), Path(code.clone())).await).await.0, 200);
    let mut forged = signed(&holder, NOT_ME_PURPOSE, now_ms());
    forged.key = hex::encode(stranger.dil.public_key());
    assert_eq!(body(not_me(State(st.clone()), Json(forged)).await).await.0, 401, "nor with the person's signature on their key");
    let (status, v) = body(not_me(State(st.clone()), Json(signed(&holder, NOT_ME_PURPOSE, now_ms()))).await).await;
    assert_eq!((status, v["closed"].as_u64()), (200, Some(1)), "the account it names can");
    assert_eq!(body(get_ask(State(st.clone()), Path(code)).await).await.0, 404);

    // Withdrawal.
    let mut forged = signed(&holder, WITHDRAW_PURPOSE, now_ms());
    forged.key = hex::encode(stranger.dil.public_key());
    assert_eq!(body(withdraw(State(st.clone()), Json(forged)).await).await.0, 401);
    let theirs = signed(&stranger, WITHDRAW_PURPOSE, now_ms());
    assert_eq!(body(withdraw(State(st.clone()), Json(theirs)).await).await.0, 200, "a stranger withdraws only their own (nothing)");
    assert_eq!(body(get_recovery_setup(State(st.clone()), Path(holder.did.clone())).await).await.0, 200, "a stranger's withdrawal leaves the setup standing");
    let mine = signed(&holder, WITHDRAW_PURPOSE, now_ms());
    let replay = SignedBody { key: mine.key.clone(), timestamp: mine.timestamp, sig: mine.sig.clone() };
    let (code, v) = body(withdraw(State(st.clone()), Json(mine)).await).await;
    assert_eq!((code, v["removed"].as_u64()), (200, Some(2)));
    assert_eq!(body(withdraw(State(st.clone()), Json(replay)).await).await.0, 409, "a replay is refused");
    assert_eq!(body(get_recovery_setup(State(st.clone()), Path(holder.did.clone())).await).await.0, 404, "taken back");
    assert_eq!(post(&st, &objects[0]).await.0, 400, "and it cannot be posted again");
}

/// MALFORMED REQUESTS ARE 400s WITH WORDS: a bad account id, a key of the wrong length, a code
/// that is not one, base64 that is not.
#[tokio::test]
async fn malformed_requests_are_400s() {
    let st = state("malformed");
    assert_eq!(body(get_recovery_setup(State(st.clone()), Path("bob".into())).await).await.0, 400);
    let short = AskBody { holder_did: person(0xa1).did, one_time_key_b64: B64.encode([1u8; 10]), close_hash: "00".repeat(32) };
    assert_eq!(body(open_ask(State(st.clone()), Json(short)).await).await.0, 400);
    let notb64 = AskBody { holder_did: person(0xa1).did, one_time_key_b64: "!!".into(), close_hash: "00".repeat(32) };
    assert_eq!(body(open_ask(State(st.clone()), Json(notb64)).await).await.0, 400);
    assert_eq!(body(get_ask(State(st.clone()), Path("U".repeat(16))).await).await.0, 400);
    let k = OneTimeAsk::new().unwrap();
    let nobody = AskBody { holder_did: person(0xa2).did, one_time_key_b64: B64.encode(&k.public), close_hash: "00".repeat(32) };
    assert_eq!(body(open_ask(State(st.clone()), Json(nobody)).await).await.0, 404, "no setup, no ask");
}
