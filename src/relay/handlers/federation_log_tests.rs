//! Tests for federation's numbered log (federation_log.rs) with fake links: a link registered the
//! way a real one registers itself, whose channel the test reads where its writer would.

use super::*;
use crate::relay::relay::FederatedConnection;
use crate::relay::storage::Storage;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

async fn fake_link(state: &RelayState, id: &str) -> UnboundedReceiver<String> {
    let (tx, rx) = unbounded_channel();
    state.federation_connections.write().await.insert(
        id.to_string(),
        FederatedConnection { tx, server_id: id.to_string(), server_name: id.to_string(), trust_tier: 2, connected_at: Instant::now() },
    );
    rx
}

fn frames(rx: &mut UnboundedReceiver<String>) -> Vec<Value> {
    let mut out = Vec::new();
    while let Ok(text) = rx.try_recv() {
        out.push(serde_json::from_str(&text).expect("a frame is JSON"));
    }
    out
}

/// A LINK THAT IS UP BUT HAS NOT SAID WHERE IT IS GETS NOTHING LIVE, THEN EXACTLY WHAT IT MISSED:
/// three events published while the link waits are not sent live; its request from 1 is answered
/// with numbers 2 and 3 and the end of the batch, and from then on it is sent live. A frame passed
/// on from that very link is sent to it as a skip.
///
/// Seen red 2026-10-22 with `serve_resume` reading rows after `from + 1` (an off-by-one): "the
/// catch-up from 1 is 2 and 3 / left: [3] / right: [2, 3]".
#[tokio::test]
async fn a_waiting_link_gets_nothing_live_then_exactly_what_it_missed() {
    let state = Arc::new(RelayState::new(Storage::open_temp("fedlog_catch_up")));
    let mut rx = fake_link(&state, "peer-b").await;
    let (tx, _keep) = unbounded_channel::<String>();
    let _queued = link_up(&state, "peer-b", &tx).await;
    for i in 1..=3 {
        publish(&state, "chat", "ann", i, "", &json!({ "type": "federated_chat", "n": i })).await;
    }
    assert!(frames(&mut rx).is_empty(), "a waiting link was sent frames live");

    let (reply_tx, mut reply) = unbounded_channel::<String>();
    serve_resume(&state, "peer-b", &reply_tx, &log_id(&state), 1, false).await;
    let got = frames(&mut reply);
    let seqs: Vec<i64> = got.iter().filter(|f| f["type"] == "fed_event").map(|f| f["seq"].as_i64().unwrap()).collect();
    assert_eq!(seqs, vec![2, 3], "the catch-up from 1 is 2 and 3");
    let end = got.iter().find(|f| f["type"] == "fed_batch_end").expect("the batch ends");
    assert_eq!((end["upto"].as_i64(), end["more"].as_bool()), (Some(3), Some(false)));

    // Caught up: the next one goes live, and one from this very link is a skip.
    publish(&state, "profile", "", 0, "peer-b", &json!({ "type": "profile_gossip" })).await;
    let live = frames(&mut rx);
    assert_eq!(live.len(), 1);
    assert_eq!((live[0]["type"].as_str(), live[0]["seq"].as_i64()), (Some("fed_skip"), Some(4)));
}

/// A REQUEST FROM ANOTHER LOG, OR FROM BEFORE WHAT THE OUTBOX KEEPS, STARTS FROM WHAT IS THERE: a
/// peer that knows another log id starts from nothing; one whose place was pruned away is told the
/// gap first.
///
/// Seen red 2026-10-22 with the same off-by-one in `serve_resume`: "left: [5] / right: [4, 5]".
#[tokio::test]
async fn a_request_from_before_the_outbox_is_told_the_gap() {
    let state = Arc::new(RelayState::new(Storage::open_temp("fedlog_gap")));
    for i in 1..=5 {
        publish(&state, "chat", "ann", i, "", &json!({ "type": "federated_chat", "n": i })).await;
    }
    state.db.with_conn(|c| c.execute("DELETE FROM federation_outbox WHERE seq <= 3", [])).unwrap();
    let (tx, mut rx) = unbounded_channel::<String>();
    serve_resume(&state, "peer-b", &tx, &log_id(&state), 1, false).await;
    let got = frames(&mut rx);
    assert_eq!((got[0]["type"].as_str(), got[0]["upto"].as_i64()), (Some("fed_gap"), Some(3)), "{got:?}");
    let seqs: Vec<i64> = got.iter().filter(|f| f["type"] == "fed_event").map(|f| f["seq"].as_i64().unwrap()).collect();
    assert_eq!(seqs, vec![4, 5]);
    serve_resume(&state, "peer-b", &tx, "another-log", 4, false).await;
    let seqs: Vec<i64> = frames(&mut rx).iter().filter(|f| f["type"] == "fed_event").map(|f| f["seq"].as_i64().unwrap()).collect();
    assert_eq!(seqs, vec![4, 5], "another log: from the start of what is kept (after the gap)");
}

/// NUMBERED FRAMES APPLY ONCE AND IN ORDER: the next number moves the place on (and is stored),
/// a repeat is ignored, and a gap asks for the catch-up once (not again while it is unanswered).
///
/// Seen red 2026-10-22 with the repeat check removed from `numbered` (`seq <= pin.last` treated
/// as next): "a repeat moved the place / left: 1 / right: 2".
#[tokio::test]
async fn numbered_frames_apply_once_and_in_order() {
    let state = Arc::new(RelayState::new(Storage::open_temp("fedlog_numbered")));
    let (tx, mut asks) = unbounded_channel::<String>();
    let skip = |seq: i64| json!({ "type": "fed_skip", "log": "L", "seq": seq }).to_string();
    ingest(&state, "peer-b", &tx, &skip(1)).await;
    ingest(&state, "peer-b", &tx, &skip(2)).await;
    assert_eq!(state.db.fed_cursor("peer-b").unwrap(), Some(("L".to_string(), 2)));
    ingest(&state, "peer-b", &tx, &skip(1)).await;
    assert_eq!(state.db.fed_cursor("peer-b").unwrap().unwrap().1, 2, "a repeat moved the place");
    assert!(frames(&mut asks).is_empty());
    ingest(&state, "peer-b", &tx, &skip(5)).await;
    ingest(&state, "peer-b", &tx, &skip(6)).await;
    let asked = frames(&mut asks);
    assert_eq!(asked.len(), 1, "a gap asks once while unanswered: {asked:?}");
    assert_eq!((asked[0]["type"].as_str(), asked[0]["from"].as_i64(), asked[0]["log"].as_str()), (Some("fed_resume"), Some(2), Some("L")));
    assert_eq!(state.db.fed_cursor("peer-b").unwrap().unwrap().1, 2, "nothing past the gap applied");
}
