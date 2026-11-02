// Child of task_notice.rs (#[path]): what a task's change means for its assignee, who is told,
// and the board's role check on the raw messages the website's Tasks page sends (BUG-221).

use super::*;
use crate::relay::handlers::msg_handlers::{handle_raw_task_create, handle_raw_task_update};

fn task(assignee: Option<&str>, status: &str, title: &str) -> TaskRecord {
    TaskRecord {
        id: 7,
        title: title.into(),
        description: String::new(),
        status: status.into(),
        priority: "medium".into(),
        assignee: assignee.map(str::to_string),
        created_by: "k_mod".into(),
        created_at: 0,
        updated_at: 0,
        position: 0,
        labels: "[]".into(),
        project: "default".into(),
    }
}

/// A task is newly given to someone (made with them as its assignee, or moved to them from
/// nobody or someone else), or theirs changed in what a notice is about; anything else tells
/// nobody. Seen red 2026-11-01 with the assignee compared as typed: " bea " after "Bea" was
/// "given again" (Assigned, not Changed).
#[test]
fn a_change_is_assigned_changed_or_nothing() {
    let bea = task(Some("Bea"), "backlog", "Fix the pump");
    assert_eq!(change(None, &bea), Some(("Bea".into(), Change::Assigned)), "made with an assignee");
    assert_eq!(change(None, &task(None, "backlog", "x")), None, "made with nobody");
    assert_eq!(change(Some(&task(None, "backlog", "Fix the pump")), &bea), Some(("Bea".into(), Change::Assigned)), "given to Bea");
    assert_eq!(change(Some(&task(Some("Cy"), "backlog", "Fix the pump")), &bea), Some(("Bea".into(), Change::Assigned)), "moved to Bea");
    let moved = task(Some(" bea "), "in_progress", "Fix the pump");
    assert_eq!(change(Some(&bea), &moved), Some(("bea".into(), Change::Changed)), "hers, its status changed");
    let retitled = task(Some("Bea"), "backlog", "Fix the pump today");
    assert_eq!(change(Some(&bea), &retitled).map(|c| c.1), Some(Change::Changed));
    let mut labelled = bea.clone();
    labelled.labels = "[\"water\"]".into();
    labelled.project = "home".into();
    assert_eq!(change(Some(&bea), &labelled), None, "the labels and the project are not a notice");
    assert_eq!(change(Some(&bea), &task(None, "backlog", "Fix the pump")), None, "taken off her: nobody to tell");
    assert_eq!(change(Some(&bea), &bea.clone()), None, "nothing changed");
}

/// The recipients are the keys registered under the assignee's name here, never the person who
/// made the change. Seen red 2026-11-01 with the `retain` removed: a moderator who gave a task to
/// a name they also hold was told about their own change.
#[test]
fn the_assignee_is_told_never_the_actor() {
    let db = Storage::open_temp("task_notice_recipients");
    db.register_name("Bea", "k_bea").unwrap();
    db.register_name("Mo", "k_mod").unwrap();
    assert_eq!(recipients(&db, "bea", "k_mod"), vec!["k_bea".to_string()], "the name any case");
    assert!(recipients(&db, "Mo", "k_mod").is_empty(), "my own change tells me nothing");
    assert!(recipients(&db, "Nobody", "k_mod").is_empty());
}

fn fresh_state(tag: &str) -> Arc<RelayState> {
    Arc::new(RelayState::new(Storage::open_temp(tag)))
}

fn block<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Runtime::new().expect("tokio rt").block_on(f)
}

/// Every `task_notice` on the broadcast channel, as (target, change).
fn notices(rx: &mut tokio::sync::broadcast::Receiver<RelayMessage>) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    while let Ok(m) = rx.try_recv() {
        if let RelayMessage::TaskNotice { target, change, by_key, .. } = m {
            out.push((target.unwrap_or_default(), change, by_key));
        }
    }
    out
}

/// BUG-221: the raw `task_create` and `task_update` (what the website's Tasks page sends) ask for
/// a moderator or an admin, as their enum twins always did, so a person with no role can neither
/// add a task nor give one to anyone (which, from rung 3, would page them). A moderator's change
/// that gives a task to Bea tells Bea, on her sockets only. Seen red 2026-11-01 with the role
/// check taken out of `handle_raw_task_update`: "no role: the task is unchanged" failed (a person
/// with no role gave the task to Cy and marked it done).
#[test]
fn the_board_asks_for_a_role_and_a_given_task_tells_its_assignee() {
    let st = fresh_state("task_notice_flow");
    st.db.register_name("Bea", "k_bea").unwrap();
    st.db.set_role("k_mod", "mod").unwrap();
    let mut rx = st.broadcast_tx.subscribe();

    block(handle_raw_task_create(&st, "k_anyone", &serde_json::json!({ "type": "task_create", "title": "Spam", "assignee": "Bea" })));
    assert!(st.db.list_tasks().unwrap_or_default().is_empty(), "no role: no task");
    assert!(notices(&mut rx).is_empty(), "and nobody paged");

    block(handle_raw_task_create(&st, "k_mod", &serde_json::json!({ "type": "task_create", "title": "Fix the pump", "assignee": "Bea", "project": "home" })));
    let made = st.db.list_tasks().unwrap_or_default();
    assert_eq!(made.len(), 1, "a moderator's task");
    assert_eq!((made[0].assignee.as_deref(), made[0].project.as_str()), (Some("Bea"), "home"), "the assignee and project the form sent are kept");
    assert_eq!(notices(&mut rx), vec![("k_bea".into(), "assigned".into(), "k_mod".into())], "given to Bea at its creation");

    let id = made[0].id;
    block(handle_raw_task_update(&st, "k_anyone", &serde_json::json!({ "type": "task_update", "task_id": id, "assignee": "Cy", "status": "done" })));
    let t = st.db.get_task(id).unwrap().unwrap();
    assert_eq!((t.assignee.as_deref(), t.status.as_str()), (Some("Bea"), "backlog"), "no role: the task is unchanged");
    assert!(notices(&mut rx).is_empty());

    block(handle_raw_task_update(&st, "k_mod", &serde_json::json!({ "type": "task_update", "task_id": id, "status": "in_progress" })));
    assert_eq!(notices(&mut rx), vec![("k_bea".into(), "changed".into(), "k_mod".into())], "hers changed");
}
