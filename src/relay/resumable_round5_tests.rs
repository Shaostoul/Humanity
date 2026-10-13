//! Media round 5 and the uploads (2026-10-12; docs/design/media-round-5-plan.md 3.1 item 5): a
//! file added to an item is never taken for an upload waiting to be published. A child of
//! resumable_tests.rs, so its rig and its file makers are at hand. Each test names the break it was
//! seen failing against ("Seen red").

use super::*;
use crate::relay::storage::media::ROLE_FILE;

/// AN ATTACHED FILE IS NEVER TAKEN AS WAITING: Ann uploads two videos in pieces, publishes one and
/// adds the other to it. Two days on, the unpublished expiry leaves the added file and its row
/// alone; it is in neither her waiting uploads nor the admins' waiting list; it cannot be removed
/// by its name as an upload waiting to be published; and a cancel of its finished upload answers
/// that it is published, keeping it.
///
/// Seen red 2026-10-12 with `NOT_ON_AN_ITEM` left out of the expiry (back to `filename NOT IN
/// (SELECT upload_filename FROM media_items)`): "the expiry took nothing / left: 1 / right: 0",
/// and the added extra.mp4 was deleted after a day.
#[tokio::test]
async fn an_attached_file_is_never_taken_as_waiting() {
    let r = rig("resume_r5_attached");
    r.person("ann", "tok_ann").await;
    r.db().join_server("ann", "Ann").unwrap();
    let main_id = r.start("tok_ann", file("main.mp4", 1500, "video/mp4")).await.unwrap();
    let main = r.send(&main_id, "tok_ann", 0, &bytes(1500)).await.unwrap()["filename"].as_str().unwrap().to_string();
    let extra_id = r.start("tok_ann", file("extra.mp4", 1200, "video/mp4")).await.unwrap();
    let extra = r.send(&extra_id, "tok_ann", 0, &bytes(1200)).await.unwrap()["filename"].as_str().unwrap().to_string();

    let s = r.db().get_server_settings().unwrap();
    let body = media::PublishBody { key: "ann".into(), filename: main.clone(), ..Default::default() };
    let item = media::publish_core(r.db(), &s, &[], r.folders(), &body).expect("published").0;
    let id = item["item"]["id"].as_i64().unwrap();
    let f = r.folders();
    let added = r.db().attach_media_file("ann", id, &extra, ROLE_FILE, 0, |members| f.place(&extra, members)).expect("added");
    assert_eq!(added.item.file_count, 2);
    assert!(r.uploads.join(&extra).is_file(), "test setup: the added file of a public item is at its public address");

    let two_days_on = crate::relay::storage::now_millis() as i64 + 2 * 24 * 60 * 60 * 1000;
    assert_eq!(r.db().expire_unpublished_media_in(r.folders(), two_days_on).unwrap(), 0, "the expiry took nothing");
    assert!(r.uploads.join(&extra).is_file(), "the added file is still there");
    assert_eq!(r.db().media_item(id).unwrap().unwrap().files.len(), 1, "and still on the item");
    assert!(r.db().media_waiting_of("ann").unwrap().is_empty(), "it is not one of her waiting uploads");
    assert!(r.db().media_usage().unwrap().waiting.is_empty(), "nor on the admins' waiting list");
    let (code, _) = media::remove_core(r.db(), r.folders(), &media::RemoveTarget::Upload(extra.clone()), "ann").unwrap_err();
    assert_eq!(code, StatusCode::NOT_FOUND, "it is not removed by its name as a waiting upload");
    assert!(r.uploads.join(&extra).is_file());

    let answer = cancel_core(&r.state, r.folders(), &extra_id, Some("tok_ann")).await.unwrap().0;
    assert_eq!(answer, json!({ "ok": true, "published": true }), "a cancel of its upload says it is published");
    assert!(r.uploads.join(&extra).is_file(), "and keeps it");
    assert_eq!(r.db().media_item_holding(&extra).unwrap().map(|(item, _)| item), Some(id));
}
