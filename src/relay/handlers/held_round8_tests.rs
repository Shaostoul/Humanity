// Child of handlers/held_tests.rs (#[path]): the owner branch's seventh review, L1 to L3
// (2026-11-21). Each test was seen failing once, recorded at the test (scratchpad
// ownfix10-red-rust.log).

use super::*;

fn held_rows(st: &RelayState) -> i64 {
    st.db.with_conn(|c| c.query_row("SELECT COUNT(*) FROM held_items", [], |r| r.get(0))).unwrap()
}

/// Hold the held folder's one lock on a thread of its own, as a long seal does, until the returned
/// sender says go (or 30 s pass). Returns once the lock is held.
fn hold_the_held_lock(st: &Arc<RelayState>) -> std::sync::mpsc::Sender<()> {
    let (held_tx, held_rx) = std::sync::mpsc::channel();
    let (go_tx, go_rx) = std::sync::mpsc::channel::<()>();
    let st2 = st.clone();
    std::thread::spawn(move || {
        let _swap = st2.db.held_swap.lock().unwrap_or_else(|p| p.into_inner());
        let _ = held_tx.send(());
        let _ = go_rx.recv_timeout(std::time::Duration::from_secs(30));
    });
    held_rx.recv_timeout(std::time::Duration::from_secs(10)).expect("the lock is held");
    go_tx
}

/// L1: A DESTROY DURING A LONG SEAL DOES NOT PARK THE SERVER. The held folder's lock is held (a long
/// seal); the owner's Destroy now arrives on a one-thread runtime, and a timer due in 50 ms beside it
/// still fires on time, then the destroy completes once the lock is let go.
/// Seen red 2026-11-21 with `held_destroy_in` called on the runtime (as before): the 50 ms timer
/// fired only after the lock was let go, after 1.56 s ("the runtime kept running").
#[test]
fn a_destroy_during_a_long_seal_does_not_park_the_runtime() {
    let d = dirs("r8_destroy");
    let (st, id, _rae, _tom, ada) = reported_post("r8_destroy", &d);
    let own = person(9);
    make_owner(&st, &own, true);
    decide(&st, &d, &ada, id, "hide_and_keep");
    let item = st.db.held_items().unwrap().into_iter().next().expect("a held item");
    let go = hold_the_held_lock(&st);
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let (fired, destroyed) = rt.block_on(async {
        let action = signed(&st, &own, "held_destroy", "held", item.id, 0);
        let (st2, folders) = (st.clone(), crate::relay::storage::files::OwnedFolders::of(d.folders()));
        let own_key = own.key.clone();
        let destroy = tokio::spawn(async move { handle_act_in(&st2, &own_key, &json!({ "type": "held_act", "action": action }), folders.get()).await });
        let start = std::time::Instant::now();
        let timer = tokio::spawn(async move { tokio::time::sleep(std::time::Duration::from_millis(50)).await; start.elapsed() });
        // Let the lock go 1.5 s from now, from a thread of its own.
        std::thread::spawn(move || { std::thread::sleep(std::time::Duration::from_millis(1500)); let _ = go.send(()); });
        let fired = timer.await.unwrap();
        destroy.await.unwrap();
        (fired, st.db.held_item(item.id).unwrap().is_none())
    });
    assert!(fired < std::time::Duration::from_millis(700), "the runtime kept running while the destroy waited for the lock: the 50 ms timer fired after {fired:?}");
    assert!(destroyed, "and the destroy completed once the lock was let go");
}

/// L2: A SHUTDOWN WAITS FOR A HIDE IN FLIGHT, AND A START COMPLETES ONE A STOP CUT SHORT. A hide is
/// paused mid-way; the shutdown's wait does not end while it runs (a short limit says so), and once
/// the hide goes on it ends with the signed record and the decision both written. A hide whose
/// copy was sealed but whose decision never landed (a crash) is completed at start, with words, and
/// its linked servers' delete queued again; a second start finds nothing to do.
/// Seen red 2026-11-21 with the hide task not counted (`HidingNow` left out): "the wait does not end
/// while a hide runs" failed (the wait ended at once); and before `complete_unfinished_hides`, the
/// report stayed open at start.
#[test]
fn a_shutdown_waits_for_a_hide_and_a_start_completes_one_cut_short() {
    let d = dirs("r8_shutdown");
    let (st, id, _rae, _tom, ada) = reported_post("r8_shutdown", &d);
    make_owner(&st, &person(9), true);
    let raw = json!({ "type": "report_decide", "id": id, "decision": "hide_and_keep", "note": "", "action": signed(&st, &ada, "hide_and_keep", "report", id, 0) });
    let (entered, go) = pause_hide(&d);
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let (st2, folders) = (st.clone(), crate::relay::storage::files::OwnedFolders::of(d.folders()));
        let ada_key = ada.key.clone();
        let raw2 = raw.clone();
        let decision = tokio::spawn(async move { handle_decide_in(&st2, &ada_key, &raw2, folders.get()).await });
        let started = tokio::task::spawn_blocking(move || entered.recv_timeout(std::time::Duration::from_secs(30)).is_ok()).await.unwrap();
        assert!(started, "the hide started");
        assert!(!crate::relay::handlers::owner::wait_for_hides(&st, std::time::Duration::from_millis(200)).await, "the wait does not end while a hide runs");
        go.send(()).unwrap();
        assert!(crate::relay::handlers::owner::wait_for_hides(&st, std::time::Duration::from_secs(30)).await, "it ends once the hide has");
        let recorded: i64 = st.db.with_conn(|c| c.query_row("SELECT COUNT(*) FROM held_actions WHERE action = 'hide_and_keep'", [], |r| r.get(0))).unwrap();
        assert_eq!(recorded, 1, "the signed record is written by then");
        assert_eq!(st.db.report_by_id(id).unwrap().unwrap().decision.as_deref(), Some("hide_and_keep"), "and the decision");
        let _ = decision.await;
    });

    // A crash after the seal: the post hidden and held, no record, no decision.
    let d2 = dirs("r8_cutshort");
    let (st2, id2, _rae2, tom2, ada2) = reported_post("r8_cutshort", &d2);
    make_owner(&st2, &person(9), true);
    assert!(st2.db.hide_post_in(id2, &tom2.key, 42, &ada2.key, 1, d2.folders()).unwrap().is_some(), "sealed and out of view");
    assert_eq!(st2.db.report_by_id(id2).unwrap().unwrap().decision, None, "the decision never landed");
    let done = block(crate::relay::handlers::held::complete_unfinished_hides(&st2));
    assert_eq!(done, 1, "completed at start");
    let r = st2.db.report_by_id(id2).unwrap().unwrap();
    assert_eq!(r.decision.as_deref(), Some("hide_and_keep"));
    assert_eq!(r.decision_note, crate::relay::handlers::held::UNFINISHED_HIDE_NOTE_NO_RECORD, "with words, the lost signed record among them");
    assert!(outbox_kinds(&st2).iter().any(|k| k == "delete"), "the linked servers' delete is queued: {:?}", outbox_kinds(&st2));
    assert_eq!(block(crate::relay::handlers::held::complete_unfinished_hides(&st2)), 0, "a second start finds nothing to do");
}

/// L3: TWO MODERATORS DECIDING ONE REPORT AT ONCE MAKE ONE HELD ITEM. A words-only post (no files,
/// so the second seal does not fail on a file the first took away); the held folder's lock is held
/// while both decisions start, so both reach it; once it is let go, one hide seals the post and the
/// other finds it gone and does nothing.
/// Seen red 2026-11-21 with the post read before the lock (as before): two held items (left 2,
/// right 1).
#[test]
fn two_moderators_deciding_one_report_at_once_make_one_held_item() {
    let d = dirs("r8_twice");
    let (st, id, _rae, tom, ada) = reported_post("r8_twice", &d);
    make_owner(&st, &person(9), true);
    let mo = person(4);
    st.db.set_role(&mo.key, "admin").unwrap();
    // The report is about a words-only post of Tom's.
    st.db.with_conn(|c| c.execute("INSERT INTO messages (msg_type, from_key, from_name, content, timestamp, raw_json, channel_id) VALUES ('chat', ?1, 'Tom', 'only words here', 43, '{}', 'global')", rusqlite::params![tom.key])).unwrap();
    let evidence = json!([{ "kind": "post", "from": tom.key, "timestamp": 43 }]).to_string();
    st.db.with_conn(|c| c.execute("UPDATE reports_v2 SET evidence = ?2 WHERE id = ?1", rusqlite::params![id, evidence])).unwrap();
    let go = hold_the_held_lock(&st);
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let mut tasks = Vec::new();
        for who in [&ada, &mo] {
            let raw = json!({ "type": "report_decide", "id": id, "decision": "hide_and_keep", "note": "", "action": signed(&st, who, "hide_and_keep", "report", id, 0) });
            let (st2, folders, key) = (st.clone(), crate::relay::storage::files::OwnedFolders::of(d.folders()), who.key.clone());
            tasks.push(tokio::spawn(async move { handle_decide_in(&st2, &key, &raw, folders.get()).await }));
        }
        // Both reach the lock, then it is let go.
        tokio::time::sleep(std::time::Duration::from_millis(600)).await;
        let _ = go.send(());
        for t in tasks {
            let _ = t.await;
        }
        assert!(crate::relay::handlers::owner::wait_for_hides(&st, std::time::Duration::from_secs(30)).await);
    });
    assert_eq!(held_rows(&st), 1, "one held item for the one post");
    assert!(st.db.post_for_report(&tom.key, 43).unwrap().is_none(), "the post is out of view");
    assert_eq!(st.db.report_by_id(id).unwrap().unwrap().decision.as_deref(), Some("hide_and_keep"), "the report is decided once");
}
