//! Resumable uploads, round 4 (2026-10-11; docs/design/media-showcase.md, "Round 4: fixes from the
//! round 3 review"): a cancel leaves nothing stored (R4-2), a piece counted and never stored gives
//! its bytes back at start (R4-3), moving a pieces file back never copies under the room lock
//! (R4-4), and the expiry pass and the account erase remove a staged pieces file with its session
//! (R4-5). A child of resumable_tests.rs, so its rig and its file makers are at hand. Each test
//! names the break it was seen failing against ("Seen red").

use super::*;
use crate::relay::storage::upload_counts::{Span, DAY_MS, HOUR_MS};
use crate::relay::storage::upload_sessions::staged_part_name;

/// Wait, a little at a time, until `gate` has been reached by the store it holds.
async fn reached(gate: &media::hold::Gate) {
    for _ in 0..400 {
        if gate.reached() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    gate.open();
    panic!("the store never reached the gate");
}

/// Ann publishes the stored upload `name` (from the waiting folder) as a public item.
fn publish(r: &Rig, name: &str) {
    let s = r.db().get_server_settings().unwrap();
    let body = media::PublishBody { key: "ann".into(), filename: name.into(), ..Default::default() };
    media::publish_core(r.db(), &s, &[], r.folders(), &body).expect("published");
}

/// R4-2. A CANCEL MEANS NOTHING STAYS, EVEN WHEN IT RACES THE FINISH: Ann cancels while the last
/// piece's finish is being written (held here once the file is staged): 409 `{busy: true}`, and
/// nothing changes. The finish then stores the file, and the cancel asked again removes that stored
/// file from the waiting folder with its upload row and the session: `{ok: true, removed_stored:
/// true}`. An upload already published is left on the Media page (her own choice):
/// `{ok: true, published: true}`, the item and its file kept, the session gone. A session still
/// arriving answers `{ok: true}` as before.
///
/// Seen red 2026-10-11 with `cancel_session` removing only a finished session's row and answering
/// `{ok: true}`, as round 3 did: "the cancel after the finish removes the stored file too / left:
/// Object {\"ok\": Bool(true)} / right: Object {\"ok\": Bool(true), \"removed_stored\": Bool(true)}".
#[tokio::test]
async fn a_cancel_that_races_the_finish_leaves_nothing_stored() {
    let r = rig("resume_r42");
    r.person("ann", "tok_ann").await;
    r.db().join_server("ann", "Ann").unwrap();
    let data = bytes(2000);
    let gate = media::hold::gate("held-r42.mp4");
    let id = r.start("tok_ann", file("held-r42.mp4", 2000, "video/mp4")).await.unwrap();

    let watch = async {
        reached(&gate).await;
        let during = cancel_core(&r.state, r.folders(), &id, Some("tok_ann")).await;
        gate.open();
        during
    };
    let (done, during) = tokio::join!(r.send(&id, "tok_ann", 0, &data), watch);
    let during = during.expect_err("a cancel while the finish is written is refused");
    assert_eq!(during.status(), StatusCode::CONFLICT);
    assert!(matches!(&during, Refused::Json(_, body) if body["busy"] == true), "{during:?}");
    let name = done.expect("the finish stores the file")["filename"].as_str().unwrap().to_string();
    assert!(r.waiting.join(&name).is_file(), "test setup: the file is stored, waiting to be published");

    let answer = cancel_core(&r.state, r.folders(), &id, Some("tok_ann")).await.unwrap().0;
    assert_eq!(answer, json!({ "ok": true, "removed_stored": true }), "the cancel after the finish removes the stored file too");
    assert!(!r.waiting.join(&name).exists(), "the stored file is gone from the waiting folder");
    assert!(r.db().media_waiting_of("ann").unwrap().is_empty(), "and its upload row");
    assert!(r.session(&id).is_none(), "and the session");
    assert!(Rig::files_in(&r.waiting).is_empty() && Rig::files_in(&r.resume).is_empty(), "nothing stays anywhere");

    let kept = r.start("tok_ann", file("kept.mp4", 1500, "video/mp4")).await.unwrap();
    let name = r.send(&kept, "tok_ann", 0, &bytes(1500)).await.unwrap()["filename"].as_str().unwrap().to_string();
    publish(&r, &name);
    let answer = cancel_core(&r.state, r.folders(), &kept, Some("tok_ann")).await.unwrap().0;
    assert_eq!(answer, json!({ "ok": true, "published": true }), "a published upload is said to be so");
    assert!(r.db().media_item_for_upload(&name).unwrap().is_some(), "and stays on the Media page");
    assert!(r.uploads.join(&name).is_file(), "with its file");
    assert!(r.session(&kept).is_none(), "the session is gone");

    let arriving = r.start("tok_ann", file("arriving.mp4", 3000, "video/mp4")).await.unwrap();
    r.send(&arriving, "tok_ann", 0, &bytes(1000)).await.unwrap();
    assert_eq!(cancel_core(&r.state, r.folders(), &arriving, Some("tok_ann")).await.unwrap().0, json!({ "ok": true }));
    assert!(r.session(&arriving).is_none() && !r.part(&arriving).exists(), "a session still arriving goes, file and all");
}

/// The relay's clock, for the allowances, standing still at `at` (Unix ms).
fn clock_at(r: &Rig, at: i64) {
    assert!(r.state.upload_clock.set(Box::new(move || at)).is_ok(), "a new relay has no test clock yet");
}

/// R4-3. A PIECE COUNTED AND THEN CUT BACK AT START GIVES ITS BYTES BACK, TO THE DAY IT WAS COUNTED
/// IN: Ann sends 1,000 bytes, then the next 1,000 are counted and written and the relay stops
/// before their offset is stored. The next start cuts the file back to 1,000 bytes and gives the
/// 1,000 counted bytes back; she sends that piece again and the rest, and the day's count is the
/// file's 3,000 bytes, not 4,000. The relay's clock is set to a day long past, so a give-back made
/// to today's window (by the wall clock) instead of the window the piece was counted in would
/// leave that day's count where it was.
///
/// Seen red 2026-10-11 with the give-back taken out of `reconcile_upload_sessions_keeping`:
/// "Settled { ... cut_back: 1, offset_lowered: 0, given_back: 0 } } / left: (1, 0) / right: (1, 1)"
/// (cut back, and nothing given back).
#[tokio::test]
async fn a_piece_counted_and_cut_back_at_start_gives_its_bytes_back() {
    let r = rig("resume_r43");
    let at = 20_000 * DAY_MS + 22 * HOUR_MS;
    clock_at(&r, at);
    r.person("ann", "tok_ann").await;
    let data = bytes(3000);
    let id = r.start("tok_ann", file("stop-r43.mp4", 3000, "video/mp4")).await.unwrap();
    r.send(&id, "tok_ann", 0, &data[..1000]).await.unwrap();

    media::hold::stop_at("written:stop-r43.mp4");
    r.send(&id, "tok_ann", 1000, &data[1000..2000]).await.expect_err("the relay stops after the write");
    assert_eq!(std::fs::metadata(r.part(&id)).unwrap().len(), 2000, "test setup: written");
    assert_eq!(r.session(&id).unwrap().offset, 1000, "test setup: its offset not stored");
    assert_eq!(r.db().upload_count("ann", Span::Day, at), 2000, "test setup: counted");

    let settled = r.db().settle_uploads_at_start(r.folders()).unwrap();
    assert_eq!((settled.reconciled.cut_back, settled.reconciled.given_back), (1, 1), "{settled:?}");
    assert_eq!(std::fs::metadata(r.part(&id)).unwrap().len(), 1000, "cut back to the offset");
    assert_eq!(r.db().upload_count("ann", Span::Day, at), 1000, "the counted bytes are given back at start");

    r.send(&id, "tok_ann", 1000, &data[1000..2000]).await.expect("sent again");
    let done = r.send(&id, "tok_ann", 2000, &data[2000..]).await.expect("finished");
    assert_eq!(std::fs::read(r.waiting.join(done["filename"].as_str().unwrap())).unwrap(), data);
    assert_eq!(r.db().upload_count("ann", Span::Day, at), 3000, "the day counts the file once");
    assert_eq!(r.db().settle_uploads_at_start(r.folders()).unwrap().reconciled.given_back, 0, "nothing is given back twice");
}

/// R4-4. A PIECES FILE THAT CANNOT BE RENAMED BACK IS NEVER COPIED UNDER THE ROOM LOCK: the finish
/// stages the pieces file, then (held here) Ann's share fills, so the room check refuses (507),
/// and the staged file is held open by another program in a way that stops a rename but not a
/// read. Round 3 then copied it back (up to 64 GiB, with every other upload and publish waiting on
/// the room lock); now it stays staged under its session's name, whole, and the empty piece at its
/// end brings it back and stores it once it can be moved. (Windows only: a file open without
/// delete sharing is how a rename fails there while a copy still works.)
///
/// Seen red 2026-10-11 with `unstage` calling `move_file` again (a rename, else a copy): "nothing
/// was copied back under the room lock".
#[cfg(windows)]
#[tokio::test]
async fn a_pieces_file_that_cannot_be_renamed_back_is_never_copied() {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_SHARE_READ: u32 = 0x1;
    let r = rig("resume_r44");
    r.person("ann", "tok_ann").await;
    let data = bytes(3000);
    let gate = media::hold::gate("held-r44.mp4");
    let id = r.start("tok_ann", file("held-r44.mp4", 3000, "video/mp4")).await.unwrap();
    let staged = r.waiting.join(staged_part_name(&id));
    r.settings(|s| s.media_per_person_mb = 1);

    let watch = async {
        reached(&gate).await;
        r.db().record_upload("ann", "9_filler.mp4", 100, MEDIA_SHELF, "filler.mp4", (MIB - 100) as i64).unwrap();
        let held = std::fs::OpenOptions::new().read(true).share_mode(FILE_SHARE_READ).open(&staged).expect("the staged file is there");
        gate.open();
        held
    };
    let (refused, held) = tokio::join!(r.send(&id, "tok_ann", 0, &data), watch);
    let refused = refused.expect_err("no room at the finish");
    assert_eq!(refused.status(), StatusCode::INSUFFICIENT_STORAGE, "{}", refused.words());
    assert!(!r.part(&id).exists(), "nothing was copied back under the room lock");
    assert_eq!(std::fs::read(&staged).unwrap(), data, "the pieces file stays staged, whole");
    assert!(r.session(&id).is_some(), "with its session");

    drop(held);
    r.db().with_conn(|c| c.execute("DELETE FROM user_uploads WHERE filename = '9_filler.mp4'", [])).unwrap();
    let done = r.send(&id, "tok_ann", 3000, &[]).await.expect("brought back and stored");
    assert_eq!(std::fs::read(r.waiting.join(done["filename"].as_str().unwrap())).unwrap(), data);
    assert!(!staged.exists(), "nothing left staged");
}

/// R4-5. THE EXPIRY PASS AND THE ACCOUNT ERASE REMOVE A STAGED PIECES FILE WITH ITS SESSION: a
/// finish that could not move a pieces file back keeps it staged under its session's name (R3-5),
/// in the waiting folder or the public one. When that session goes idle for a day, the expiry pass
/// removes it there, not only in the resume folder; when the person erases their account, the
/// erase does the same. Neither waits for the next restart.
///
/// Seen red 2026-10-11 with `expire_upload_sessions_in` removing only the resume folder's file, as
/// round 3 did: "the idle session's staged pieces file went with it".
#[tokio::test]
async fn the_expiry_and_the_erase_remove_a_staged_pieces_file() {
    let r = rig("resume_r45");
    r.person("ann", "tok_ann").await;
    let idle = r.start("tok_ann", file("idle.mp4", 3000, "video/mp4")).await.unwrap();
    r.send(&idle, "tok_ann", 0, &bytes(1000)).await.unwrap();
    let staged_idle = r.waiting.join(staged_part_name(&idle));
    std::fs::rename(r.part(&idle), &staged_idle).unwrap();
    r.db().with_conn(|c| c.execute("UPDATE upload_sessions SET touched_at = 0 WHERE id = ?1", [&idle])).unwrap();

    r.db().run_expiry_sweeps_in(r.folders());
    assert!(r.session(&idle).is_none(), "the idle session goes");
    assert!(!staged_idle.exists(), "the idle session's staged pieces file went with it");

    let erased = r.start("tok_ann", file("erased.mp4", 3000, "video/mp4")).await.unwrap();
    r.send(&erased, "tok_ann", 0, &bytes(1000)).await.unwrap();
    let staged_erased = r.uploads.join(staged_part_name(&erased));
    std::fs::rename(r.part(&erased), &staged_erased).unwrap();
    let receipt = r.db().delete_account_in("ann", "ann", r.folders());
    assert!(!staged_erased.exists(), "the erase removes a staged pieces file too");
    assert!(receipt.contains(&("upload_session_files_removed".to_string(), 1)), "{receipt:?}");
    assert!(r.session(&erased).is_none(), "with its session");
}
