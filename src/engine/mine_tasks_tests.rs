// Child of engine/mine_tasks.rs (#[path]): rung 1's gates on the desktop (docs/design/tasks.md
// section 11), with the app state and a recording socket, no screen. Each test was seen failing
// once on purpose; what it saw red is at the test.

use super::*;
use crate::engine::put_answer::tests::{app, frames, identity, open};
use crate::gui::pages::tasks::{add_chain_task, take_board_list, Chain};
use crate::net::dm_pq::{build_signed_inner, open_v2, parse_verify_inner, DmPqKeypair};
use crate::gui::mine_tasks::MineTasks;
use serde_json::json;

/// A signed-in app (put_answer_tests' `app`, a recording socket on a server of its own) whose own
/// tasks are kept in a file of their own under the temp directory.
fn app_with_mine(seed: &[u8], me: &str, tag: &str) -> (GuiState, std::sync::mpsc::Receiver<String>) {
    let (mut gs, sent) = app(seed, me, tag, &[]);
    gs.mine_tasks = Some(MineTasks::in_temp(seed, me, tag));
    (gs, sent)
}

fn tidy_all(gs: &GuiState) {
    crate::engine::put_answer::tests::tidy(gs);
    gs.mine_tasks.as_ref().unwrap().remove_file_for_test();
}

fn mine(gs: &GuiState) -> &MineTasks {
    gs.mine_tasks.as_ref().expect("my tasks are loaded")
}

fn water() -> Chain {
    Chain { id: "water".into(), title: "Water Independence".into(), purpose: "Know where your water comes from.".into(), steps: Vec::new() }
}

/// BUG-217'S GATE: a task of my own, made with Add to my tasks or typed in, survives the server's
/// task list arriving (what a reconnect sends) and a restart, and the board's tasks sit in their
/// own list beside it. Seen red 2026-11-02 with `take_board_list` doing what the frame loop did
/// before the fix (the server's list put in place of the tasks made here): "the server's list left
/// my own tasks alone" failed, both gone, which is BUG-217.
#[test]
fn a_mine_task_survives_the_servers_list_and_a_restart() {
    let (seed, me) = identity(31);
    let (mut gs, _sent) = app_with_mine(&seed, &me, "mine-survives");
    let from_card = add_chain_task(&mut gs, &water()).expect("Add to my tasks makes one of my own");
    assert!(add_chain_task(&mut gs, &water()).is_none(), "never twice");
    let typed = add(&mut gs, "Fix the gate", "", TaskPriority::High, vec![], None).expect("a new task of my own");

    // The server's board arrives, as on every connection.
    take_board_list(&mut gs, &[json!({ "id": 3, "title": "Translate the Library", "status": "todo", "labels": "[]" })]);
    let mut ids: Vec<&str> = mine(&gs).tasks().iter().map(|t| t.id.as_str()).collect();
    ids.sort();
    let mut want = vec![from_card.as_str(), typed.as_str()];
    want.sort();
    assert_eq!(ids, want, "the server's list left my own tasks alone");
    assert_eq!(gs.board_tasks.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["3"], "and is the board's own list");
    assert_eq!(mine(&gs).get(&from_card).and_then(|t| t.chain.as_deref()), Some("water"), "linked to its chain");

    // A restart reads them back from the file.
    let again = mine(&gs).reopened(&seed);
    assert_eq!(again.tasks().len(), 2, "kept across a restart");
    assert!(again.has_chain("water"), "so the suggestion stays away after a restart too");
    assert_eq!(again.get(&typed).map(|t| t.priority), Some(TaskPriority::High));
    tidy_all(&gs);
}

/// THE WORDS NEVER REACH A SERVER: making, moving and deleting a task of my own sends only sealed
/// `dm_put`s to my own mailbox, and no frame holds any word of it; opened with my own key they are
/// my task notes. With no server connected nothing is sent, and the change goes on the next
/// connection. Seen red 2026-11-02 with `send` adding the note's text to the `dm_put` beside the
/// sealed envelope: "a frame to the server carries insulin".
#[test]
fn a_mine_tasks_words_never_reach_a_server() {
    let (seed, me) = identity(32);
    let (mut gs, sent) = app_with_mine(&seed, &me, "mine-words");
    let id = add(&mut gs, "Pick up insulin for Mum", "From the chemist on Elm Street", TaskPriority::Critical, vec!["health".into()], None).unwrap();
    change(&mut gs, &id, Some(TaskStatus::InProgress), None);
    let out = frames(&sent);
    assert_eq!(out.len(), 2, "one note for each change: {out:?}");
    for f in &out {
        let text = f.to_string();
        // Words of six letters or more: a shorter one turns up in base64 by chance.
        for word in ["insulin", "chemist", "Street", "health", "in_progress", "critical"] {
            assert!(!text.contains(word), "a frame to the server carries {word}");
        }
        assert_eq!(f["type"], "dm_put", "only sealed notes go out");
        assert_eq!(f["to"].as_str(), Some(me.as_str()), "to my own mailbox only");
        let env: serde_json::Value = serde_json::from_str(f["content"].as_str().unwrap()).unwrap();
        assert_eq!(env["v"], 2, "a sealed v2 envelope");
    }
    let inner = open(&out[1], &seed);
    assert_eq!((inner.from.as_str(), inner.to.as_str()), (me.as_str(), me.as_str()), "from me, to me");
    let records = mine_tasks::parse_note(&inner.text);
    assert_eq!(records.len(), 1);
    assert_eq!((records[0].title.as_str(), records[0].status.as_str()), ("Pick up insulin for Mum", "in_progress"));

    // No server connected: kept, and sent on the next connection.
    gs.ws_client = None;
    let later = add(&mut gs, "Book the boiler service", "", TaskPriority::Medium, vec![], None).unwrap();
    assert_eq!(mine(&gs).unsent(), [later.clone()], "kept for the next connection");
    let (client, sent2) = crate::net::ws_client::WsClient::recording();
    gs.ws_client = Some(client);
    flush(&mut gs);
    let out2 = frames(&sent2);
    assert_eq!(out2.len(), 1, "it went on the next connection");
    assert!(!out2[0].to_string().contains("boiler"));
    assert_eq!(mine_tasks::parse_note(&open(&out2[0], &seed).text)[0].id, later);
    assert!(mine(&gs).unsent().is_empty());
    tidy_all(&gs);
}

/// MY OTHER DEVICE, AND NOBODY ELSE'S: a task made on my laptop reaches my phone (the same
/// recovery phrase) through my mailbox, and so does its deletion; neither is ever stored or shown
/// as a message. Someone else cannot open the note, a task note they write to me is dropped
/// unread, and they cannot sign one as me. Seen red 2026-11-02 with `screens_dm` applying a note
/// from anyone: "a task note from someone else changes nothing" failed (2 records, not 1: Eve's
/// task was on my list).
#[test]
fn a_task_made_on_one_device_arrives_on_my_other_device_and_never_on_anyone_elses() {
    let (seed, me) = identity(33);
    let (mut laptop, sent) = app_with_mine(&seed, &me, "mine-laptop");
    let (mut phone, _phone_sent) = app_with_mine(&seed, &me, "mine-phone");
    let (eve_seed, eve) = identity(34);

    let id = add(&mut laptop, "Renew the passport", "", TaskPriority::High, vec![], None).unwrap();
    let put = frames(&sent).remove(0);
    let inner = open(&put, &seed);
    assert!(!crate::engine::dm::ingest_dm(&mut phone, &inner), "a task note is never a message");
    assert_eq!(mine(&phone).get(&id).map(|t| t.title.as_str()), Some("Renew the passport"), "my phone has it");
    assert!(phone.dm_store.as_ref().unwrap().conversation(&me).is_empty(), "and did not store it as a message");

    delete(&mut laptop, &id);
    let gone = open(&frames(&sent).remove(0), &seed);
    crate::engine::dm::ingest_dm(&mut phone, &gone);
    assert!(mine(&phone).get(&id).is_none(), "the deletion reached my phone too");

    // Eve cannot open my note: it is sealed to my key.
    let eve_kp = DmPqKeypair::from_bip39_seed(&eve_seed).unwrap();
    assert!(open_v2(&eve_kp, put["content"].as_str().unwrap()).is_err(), "someone else cannot open it");
    // A task note Eve writes to me is dropped unread.
    let text = mine_tasks::note_texts(&[mine_tasks::Record::gone(&mine_tasks::new_id(), 1)]).remove(0).replace("\"deleted\":true", "\"title\":\"Planted by Eve\"");
    let from_eve = parse_verify_inner(&build_signed_inner(&eve_seed, &eve, &me, 9, &text).unwrap()).unwrap();
    let before = mine(&laptop).records().len();
    assert!(!crate::engine::dm::ingest_dm(&mut laptop, &from_eve), "never shown either");
    assert_eq!(mine(&laptop).records().len(), before, "a task note from someone else changes nothing");
    // And Eve cannot sign a note as me: the signature check refuses it before anything reads it.
    assert!(parse_verify_inner(&build_signed_inner(&eve_seed, &me, &me, 10, &text).unwrap()).is_err());
    tidy_all(&laptop);
    tidy_all(&phone);
}

/// MY CHANGES STILL GO OUT WHEN THE FILE COULD NOT BE READ (the review of rung 1, 2026-11-02): the
/// session saves nothing over the file, and a task made now still reaches my other devices as a
/// note. Seen red 2026-11-02 with `send_or_keep` returning before sending when the file could not
/// be saved: "my change still went to my own mailbox" failed (0 frames).
#[test]
fn an_unreadable_file_still_sends_my_changes_to_my_other_devices() {
    let (seed, me) = identity(35);
    let (mut gs, sent) = app(&seed, &me, "mine-unreadable", &[]);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let folder = std::env::temp_dir().join(format!("hum-tasks-test-locked-{nanos}.mine"));
    std::fs::create_dir_all(&folder).unwrap();
    gs.mine_tasks = Some(MineTasks::at_path_for_test(&seed, &me, folder.clone()));
    assert!(matches!(mine(&gs).problem(), Some(crate::gui::mine_tasks::OpenProblem::Unreadable(_))));
    let id = add(&mut gs, "Water the seedlings", "", TaskPriority::Medium, vec![], None).unwrap();
    let out = frames(&sent);
    assert_eq!(out.len(), 1, "my change still went to my own mailbox");
    assert_eq!(mine_tasks::parse_note(&open(&out[0], &seed).text)[0].id, id);
    assert!(folder.is_dir(), "and nothing was saved over the file");
    crate::engine::put_answer::tests::tidy(&gs);
    let _ = std::fs::remove_dir_all(&folder);
}
