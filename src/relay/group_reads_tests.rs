// Child of api_v2_objects.rs (#[path]). BUG-284 (2026-11-07): a P2P group's name, member list and
// who posted when were served to anyone: the object list (`GET /api/v2/objects`, group creation and
// membership included), one object by its id, `GET /api/v2/groups?pubkey=` for anyone's key, and
// the group's members, messages and epoch keys by its id. Now the list never carries a group's
// objects, and everything else of a group answers only a signed read by one of its members.
//
// Seen red on 2026-11-07 against the routes before the fix (the handlers as they were, read the
// way a stranger reads them): the list carried the group_v1 and its message object ("a stranger's
// list has no group objects" failed), and the group's id gave the group object to anyone.

use super::*;
use ciborium::Value;
use humanity_protocol::object::ObjectBuilder;
use humanity_protocol::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};

fn person(seed_byte: u8) -> (DilithiumKeypair, String) {
    let dil = DilithiumKeypair::from_seed(&derive_dilithium_seed(&[seed_byte; 32]));
    let key = hex::encode(dil.public_key());
    (dil, key)
}

fn block<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Runtime::new().unwrap().block_on(f)
}

fn signed(dil: &DilithiumKeypair, key: &str) -> GroupReadRequest {
    let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;
    GroupReadRequest { key: key.to_string(), timestamp: ts, sig: hex::encode(dil.sign(format!("{GROUP_READ_PURPOSE}\n{ts}").as_bytes())) }
}

/// The answer's status and its JSON body.
fn answer(r: impl IntoResponse) -> (StatusCode, serde_json::Value) {
    let res = r.into_response();
    let code = res.status();
    let bytes = block(axum::body::to_bytes(res.into_body(), 1 << 22)).unwrap();
    (code, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

/// A group "Book club" started by `creator`, with one message of theirs, and a public post.
fn setup(st: &Arc<RelayState>, creator: &DilithiumKeypair) -> String {
    let group = ObjectBuilder::new("group_v1")
        .created_at(1000)
        .payload_cbor(&Value::Map(vec![(Value::Text("name".into()), Value::Text("Book club".into()))]))
        .unwrap()
        .sign(creator)
        .unwrap();
    let gid = group.object_id().unwrap().to_hex();
    assert!(st.db.put_signed_object(&group, None).unwrap());
    let msg = ObjectBuilder::new("group_msg_v1")
        .reference(&gid)
        .created_at(2001)
        .payload_cbor(&Value::Map(vec![
            (Value::Text("epoch".into()), Value::Integer(0.into())),
            (Value::Text("ct".into()), Value::Bytes(vec![1, 2, 3, 4])),
        ]))
        .unwrap()
        .sign(creator)
        .unwrap();
    st.db.put_signed_object(&msg, None).unwrap();
    let post = ObjectBuilder::new("post")
        .created_at(3000)
        .payload_cbor(&Value::Map(vec![(Value::Text("text".into()), Value::Text("hello world".into()))]))
        .unwrap()
        .sign(creator)
        .unwrap();
    st.db.put_signed_object(&post, None).unwrap();
    gid
}

fn fresh() -> Arc<RelayState> {
    Arc::new(RelayState::new(crate::relay::storage::Storage::open_temp("group_reads")))
}

#[test]
fn a_strangers_list_and_lookups_carry_nothing_of_a_group() {
    let st = fresh();
    let (ada, ada_key) = person(0x41);
    let gid = setup(&st, &ada);

    let q = ListObjectsQuery { object_type: None, space_id: None, author_fp: None, since_received: None, limit: None };
    let (code, list) = answer(block(list_objects(State(st.clone()), Query(q))));
    assert_eq!(code, StatusCode::OK);
    let types: Vec<&str> = list.as_array().unwrap().iter().map(|o| o["object_type"].as_str().unwrap()).collect();
    assert!(types.contains(&"post"), "a public object is still listed: {types:?}");
    assert!(!types.iter().any(|t| t.starts_with("group_")), "a stranger's list has no group objects: {types:?}");
    let by_type = ListObjectsQuery { object_type: Some("group_v1".into()), space_id: None, author_fp: None, since_received: None, limit: None };
    assert_eq!(answer(block(list_objects(State(st.clone()), Query(by_type)))).1, serde_json::json!([]), "nor when asked for by type");
    let by_author = ListObjectsQuery { object_type: None, space_id: None, author_fp: Some(crate::relay::storage::author_fingerprint(&hex::decode(&ada_key).unwrap())), since_received: None, limit: None };
    let mine: Vec<String> = answer(block(list_objects(State(st.clone()), Query(by_author)))).1.as_array().unwrap().iter().map(|o| o["object_type"].as_str().unwrap().to_string()).collect();
    assert_eq!(mine, vec!["post".to_string()], "nor when asked for by its starter's fingerprint");

    assert_eq!(answer(block(get_object_by_id(State(st.clone()), Path(gid.clone())))).0, StatusCode::UNAUTHORIZED, "the group's id gives nothing unsigned");
    assert_eq!(answer(block(count_objects(State(st.clone()), Query(CountObjectsQuery { object_type: Some("group_v1".into()) })))).0, StatusCode::FORBIDDEN);

    // A stranger's signed reads are refused, every one.
    let (bo, bo_key) = person(0x42);
    assert_eq!(answer(block(read_object_signed(State(st.clone()), Path(gid.clone()), Json(signed(&bo, &bo_key))))).0, StatusCode::FORBIDDEN);
    assert_eq!(answer(block(group_member_keys(State(st.clone()), Path(gid.clone()), Json(signed(&bo, &bo_key))))).0, StatusCode::FORBIDDEN);
    assert_eq!(answer(block(group_messages(State(st.clone()), Path(gid.clone()), Json(signed(&bo, &bo_key))))).0, StatusCode::FORBIDDEN);
    assert_eq!(answer(block(group_epoch_key(State(st.clone()), Path(gid.clone()), Json(signed(&bo, &bo_key))))).0, StatusCode::FORBIDDEN);
    assert_eq!(answer(block(group_epoch_keys(State(st.clone()), Path(gid.clone()), Json(signed(&bo, &bo_key))))).0, StatusCode::FORBIDDEN);
    // Asking for someone else's groups with their key and my signature fails the signature.
    assert_eq!(answer(block(my_p2p_groups(State(st.clone()), Json(signed(&bo, &ada_key))))).0, StatusCode::UNAUTHORIZED);
    let (_, bo_groups) = answer(block(my_p2p_groups(State(st.clone()), Json(signed(&bo, &bo_key)))));
    assert_eq!(bo_groups["groups"], serde_json::json!([]), "and my own list holds no group of Ada's");
    // A public object still reads signed or not.
    let post_id = answer(block(list_objects(State(st.clone()), Query(ListObjectsQuery { object_type: Some("post".into()), space_id: None, author_fp: None, since_received: None, limit: None })))).1[0]["object_id"].as_str().unwrap().to_string();
    assert_eq!(answer(block(read_object_signed(State(st.clone()), Path(post_id.clone()), Json(signed(&bo, &bo_key))))).0, StatusCode::OK);
    assert_eq!(answer(block(get_object_by_id(State(st.clone()), Path(post_id)))).0, StatusCode::OK);
}

#[test]
fn a_member_reads_their_group_signed() {
    let st = fresh();
    let (ada, ada_key) = person(0x41);
    let gid = setup(&st, &ada);
    let (code, obj) = answer(block(read_object_signed(State(st.clone()), Path(gid.clone()), Json(signed(&ada, &ada_key)))));
    assert_eq!(code, StatusCode::OK);
    assert_eq!(obj["object_type"], "group_v1");
    let (code, groups) = answer(block(my_p2p_groups(State(st.clone()), Json(signed(&ada, &ada_key)))));
    assert_eq!(code, StatusCode::OK);
    assert_eq!(groups["groups"][0]["name"], "Book club");
    assert_eq!(groups["groups"][0]["is_creator"], true);
    let (code, members) = answer(block(group_member_keys(State(st.clone()), Path(gid.clone()), Json(signed(&ada, &ada_key)))));
    assert_eq!(code, StatusCode::OK);
    assert_eq!(members["members"].as_array().unwrap().len(), 1);
    let (code, msgs) = answer(block(group_messages(State(st.clone()), Path(gid.clone()), Json(signed(&ada, &ada_key)))));
    assert_eq!(code, StatusCode::OK);
    assert_eq!(msgs["messages"].as_array().unwrap().len(), 1);
    // A signature too old is refused.
    let mut stale = signed(&ada, &ada_key);
    stale.timestamp -= 10 * 60 * 1000;
    assert_eq!(answer(block(group_messages(State(st.clone()), Path(gid), Json(stale)))).0, StatusCode::BAD_REQUEST);
}
