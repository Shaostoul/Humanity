// Child of msg_handlers.rs (#[path], so `use super::*` reaches the handlers): the raw task messages
// the web Tasks board sends (`task_update`, `task_create`, `task_comment`, web/pages/tasks-app.js)
// keep the rules their typed twins keep (BUG-221, 2026-11-01), through `may_edit_tasks` and
// `may_comment_on_tasks`. Beside rung 3's own test of the same fix
// (task_notice_tests.rs `the_board_asks_for_a_role_and_a_given_task_tells_its_assignee`: no role,
// no task and no change; a moderator's task notifies its assignee), these hold what that one does
// not: the raw comment path both ways, the words a refused member is told, the title and priority
// of a refused update, and an admin (not only a moderator) still running the board. The raw block in relay.rs takes
// those three message types before the typed ones are parsed, so until this fix the role checks
// in `handle_task_update_msg`, `handle_task_move`, `handle_task_create` and
// `handle_task_comment_msg` never ran for them, and anyone signed in could move, rename or
// reassign any task, add tasks and comment. Rung 3 found it reading the task routes; the warnings
// cleanup found it again the same day, because `handle_raw_task_update` never read its `my_key`
// (an unused-variable warning hidden on this PC by RUSTFLAGS=-Awarnings).
//
// Seen red 2026-11-01 with the three checks taken out again (the relay build's test run): the
// member's update changed the task, "left: (\"done\", \"Mine now\", \"critical\",
// Some(\"member_key_0123456789\"))", and the member's create made a second task, "left: 2"; the
// admin test still passed.

use super::*;

fn fresh(tag: &str) -> Arc<RelayState> {
    Arc::new(RelayState::new(Storage::open_temp(&format!("task_rules_{tag}"))))
}

fn block<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Runtime::new().expect("tokio rt").block_on(f)
}

// Keys at least eight characters long: a handler names an unnamed sender by its key's first eight.
const ADMIN: &str = "admin_key_0123456789";
const MEMBER: &str = "member_key_0123456789";
const VERIFIED: &str = "verified_key_0123456789";

/// A board with one task, made by the admin, and the roles the rules read.
fn board(tag: &str) -> (Arc<RelayState>, i64) {
    let st = fresh(tag);
    st.db.set_role(ADMIN, "admin").unwrap();
    st.db.set_role(VERIFIED, "verified").unwrap();
    let id = st.db.create_task("Plant beans", "", "backlog", "medium", None, "Ada", "[]").unwrap();
    (st, id)
}

/// Every private line sent to `to` so far.
fn told(rx: &mut tokio::sync::broadcast::Receiver<RelayMessage>, to: &str) -> Vec<String> {
    let mut out = Vec::new();
    while let Ok(m) = rx.try_recv() {
        if let RelayMessage::Private { to: who, message } = m {
            if who == to {
                out.push(message);
            }
        }
    }
    out
}

#[test]
fn a_member_cannot_move_edit_or_reassign_a_task_through_the_raw_message() {
    let (st, id) = board("member_update");
    let mut rx = st.broadcast_tx.subscribe();
    let raw = serde_json::json!({ "task_id": id, "status": "done", "title": "Mine now", "priority": "critical", "assignee": MEMBER });
    block(handle_raw_task_update(&st, MEMBER, &raw));
    let t = st.db.get_task(id).unwrap().unwrap();
    assert_eq!(
        (t.status.as_str(), t.title.as_str(), t.priority.as_str(), t.assignee.as_deref()),
        ("backlog", "Plant beans", "medium", None),
        "a member's raw task_update changed nothing"
    );
    assert_eq!(told(&mut rx, MEMBER), vec!["Only admins and mods can edit tasks.".to_string()], "and is told why");
}

#[test]
fn a_member_cannot_add_a_task_and_an_unverified_one_cannot_comment() {
    let (st, id) = board("member_create");
    let mut rx = st.broadcast_tx.subscribe();
    block(handle_raw_task_create(&st, MEMBER, &serde_json::json!({ "title": "Spam" })));
    assert_eq!(st.db.list_tasks().unwrap().len(), 1, "a member's raw task_create made no task");
    block(handle_raw_task_comment(&st, MEMBER, &serde_json::json!({ "task_id": id, "content": "hello" })));
    assert!(st.db.get_task_comments(id).unwrap().is_empty(), "nor did an unverified comment land");
    assert_eq!(
        told(&mut rx, MEMBER),
        vec!["Only admins and mods can create tasks.".to_string(), "Only verified users can comment on tasks.".to_string()]
    );
}

#[test]
fn an_admin_still_runs_the_board_and_a_verified_member_still_comments() {
    let (st, id) = board("admin");
    let raw = serde_json::json!({ "task_id": id, "status": "in_progress", "title": "Plant beans, row 2", "assignee": ADMIN });
    block(handle_raw_task_update(&st, ADMIN, &raw));
    let t = st.db.get_task(id).unwrap().unwrap();
    assert_eq!((t.status.as_str(), t.title.as_str(), t.assignee.as_deref()), ("in_progress", "Plant beans, row 2", Some(ADMIN)));
    block(handle_raw_task_create(&st, ADMIN, &serde_json::json!({ "title": "Water beans" })));
    assert_eq!(st.db.list_tasks().unwrap().len(), 2, "an admin adds a task");
    block(handle_raw_task_comment(&st, VERIFIED, &serde_json::json!({ "task_id": id, "content": "Done the first row." })));
    assert_eq!(st.db.get_task_comments(id).unwrap().len(), 1, "a verified member comments");
}
