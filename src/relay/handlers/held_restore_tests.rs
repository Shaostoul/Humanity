// Child of held_restore.rs (#[path]): Restore driven end to end against a real database, real
// Dilithium3 and Kyber768 identities and a test's own upload folders: the owner's device opens the
// sealed parts as the app does (net/held.rs's opening) and sends each file back in pieces.

use super::super::tests::{act, dirs, drain, one_held, public_status, signed, told, upload_rows, Dirs, MARKER};
use super::*;
use crate::relay::handlers::held::{export_part_path, handle_list};
use crate::relay::storage::held::restore::RESTORE_PIECE_MAX;
use humanity_protocol::held_seal::{open_bytes, part_context, Manifest};
use humanity_protocol::pq_crypto::{derive_kyber_seed, KyberKeypair};

fn grant_of(action: &serde_json::Value) -> String {
    let sub: crate::relay::api_v2_objects::SignedObjectSubmission = serde_json::from_value(action.clone()).unwrap();
    sub.to_object().unwrap().object_id().unwrap().to_hex()
}

/// What `who`'s device opens of part `part` (the bytes on disk read under the grant).
fn opened(st: &RelayState, d: &Dirs, kp: &KyberKeypair, row: &HeldRow, grant: &str, part: usize) -> Vec<u8> {
    let owner = st.db.owner_keys().unwrap().pop().unwrap();
    let path = export_part_path(st, row.id, part as i64, grant, &owner, d.folders(), now_ms()).expect("the restore grant reaches the part");
    open_bytes(kp, &part_context(&row.package, part), &std::fs::read(path).unwrap()).unwrap()
}

fn status(r: &Response) -> u16 {
    r.status().as_u16()
}

/// RESTORE PUTS BACK EXACTLY WHAT WAS HELD, AND ONLY THAT (rung 5). The owner signs Restore; their
/// device opens each sealed part with their own key and sends every file back in pieces; the relay
/// takes nothing without the key part 0 carries, nothing in the wrong place or under an export's
/// grant, and no byte that differs from what it held; then the post is back in its room with its
/// words, every file is back in its own folder (the members-only file in the members' folder, a
/// video's poster picture beside it) with its upload row, every client is sent the post again, and
/// the held copy and its row are gone, its restore counted. An admin who is not the owner cannot
/// restore, and only the owner's list carries the package id.
///
/// Seen red 2026-11-20 with `take_piece`'s hash check taken out: "a file that differs is not taken"
/// (the altered file was taken whole: left 200, right 400).
#[test]
fn restore_puts_back_exactly_what_was_held_and_only_that() {
    let d = dirs("restore");
    let (st, _id, ada, own, row) = one_held("restore", &d);
    let kp = KyberKeypair::from_seed(&derive_kyber_seed(&own.seed)).unwrap();

    // Only the owner's list carries the package id the owner's device needs.
    let mut rx = st.broadcast_tx.subscribe();
    handle_list(&st, &own.key);
    handle_list(&st, &ada.key);
    let lists: Vec<(String, String)> = drain(&mut rx)
        .into_iter()
        .filter_map(|m| match &m {
            RelayMessage::HeldItems { to, .. } => Some((to.clone(), serde_json::to_string(&m).unwrap())),
            _ => None,
        })
        .collect();
    assert!(lists.iter().any(|(to, l)| *to == own.key && l.contains(&row.package)), "the owner's list carries the package id");
    assert!(lists.iter().any(|(to, l)| *to == ada.key && !l.contains(&row.package) && !l.contains("package")), "an admin's does not");

    // An admin who is not the owner cannot restore.
    let all = act(&st, &d, &ada, signed(&st, &ada, "held_restore", "held", row.id, 0));
    assert!(told(&all, &ada.key).contains("only this server's owner"), "{}", told(&all, &ada.key));

    // An export's grant opens the parts but takes nothing back.
    let export = signed(&st, &own, "held_export", "held", row.id, 0);
    act(&st, &d, &own, export.clone());
    let export_grant = grant_of(&export);

    let restore = signed(&st, &own, "held_restore", "held", row.id, 0);
    let all = act(&st, &d, &own, restore.clone());
    assert!(told(&all, &own.key).contains("Restore of held item"), "{}", told(&all, &own.key));
    let grant = grant_of(&restore);

    let m: Manifest = serde_json::from_slice(&opened(&st, &d, &kp, &row, &grant, 0)).unwrap();
    let key = m.restore_key.clone();
    assert_eq!(key.len(), 64, "part 0 carries the restore key");
    let now = now_ms();
    let put = |part: usize, offset: u64, body: &[u8], key: &str, grant: &str| piece_in(&st, row.id, part as i64, &PieceQuery { grant: grant.into(), offset }, &own.key, key, body, d.folders(), now);

    let pic = m.files.iter().find(|f| f.name == "pic_1.png").unwrap();
    let pic_bytes = opened(&st, &d, &kp, &row, &grant, pic.part);
    assert_eq!(status(&put(pic.part, 0, &pic_bytes, &key, &export_grant)), 404, "an export's grant takes nothing back");
    assert_eq!(status(&put(pic.part, 0, &pic_bytes, &"00".repeat(32), &grant)), 403, "nothing without the key part 0 carries");
    assert_eq!(status(&put(m.files.len() + 9, 0, b"x", &key, &grant)), 404, "nothing for a part the plan does not hold");
    assert_eq!(status(&finish_in(&st, row.id, &grant, &own.key, &key, d.folders(), now)), 409, "nothing is put back before every file arrived");
    // The owner review's finding 1: the grant and the restore key are not enough. An admin holding
    // both (from the log and a part they could open as successor) is refused, piece and finish.
    let file_part = m.files[0].part as i64;
    assert_eq!(status(&piece_in(&st, row.id, file_part, &PieceQuery { grant: grant.clone(), offset: 0 }, &ada.key, &key, b"x", d.folders(), now)), 404, "an admin's piece under the owner's grant");
    assert_eq!(status(&finish_in(&st, row.id, &grant, &ada.key, &key, d.folders(), now)), 404, "an admin's finish under the owner's grant");

    let mut altered = pic_bytes.clone();
    altered[0] ^= 1;
    assert_eq!(status(&put(pic.part, 0, &altered, &key, &grant)), 400, "a file that differs is not taken");
    assert_eq!(status(&put(pic.part, 0, &vec![0u8; RESTORE_PIECE_MAX + 1], &key, &grant)), 413, "a piece larger than a piece may be is refused");

    // Every file and poster picture, each in two pieces where it has two bytes or more.
    for f in &m.files {
        for part in std::iter::once(f.part).chain(f.poster_part) {
            let bytes = opened(&st, &d, &kp, &row, &grant, part);
            let cut = bytes.len() / 2;
            if cut > 0 {
                assert_eq!(status(&put(part, 0, &bytes[..cut], &key, &grant)), 200);
                assert_eq!(status(&put(part, 0, &bytes[cut..], &key, &grant)), 409, "a piece that does not start where the file ends is refused");
            }
            assert_eq!(status(&put(part, cut as u64, &bytes[cut..], &key, &grant)), 200, "{} part {part}", f.name);
        }
    }
    assert!(st.db.post_for_report(&row.author_key, 42).unwrap().is_none(), "the post is still hidden until the finish");

    let mut rx = st.broadcast_tx.subscribe();
    let r = finish_in(&st, row.id, &grant, &own.key, &key, d.folders(), now);
    assert_eq!(status(&r), 200);
    let all = drain(&mut rx);
    let (content, channel) = st.db.post_for_report(&row.author_key, 42).unwrap().expect("the post is back");
    assert!(content.starts_with(MARKER) && channel == "global", "with its words, in its room: {content} {channel}");
    assert!(
        all.iter().any(|m| matches!(m, RelayMessage::Chat { from, timestamp: 42, channel, .. } if *from == row.author_key && channel == "global")),
        "every client is sent the post again"
    );
    assert!(told(&all, &own.key).contains("is restored"), "{}", told(&all, &own.key));
    assert_eq!(public_status(&d, "pic_1.png"), 200, "the picture is served again");
    assert_eq!(std::fs::read(d.uploads.join("pic_1.png")).unwrap(), format!("{MARKER} picture").into_bytes(), "byte for byte");
    assert!(d.uploads.join("vid_2.mp4.poster.jpg").is_file(), "the poster picture beside its video");
    assert!(d.members.join("doc_3.pdf").is_file() && !d.uploads.join("doc_3.pdf").exists(), "the members-only file in the members' folder only");
    assert_eq!(upload_rows(&st), 3, "each file's upload row is back");
    assert!(st.db.held_item(row.id).unwrap().is_none(), "the held row is gone");
    assert!(!crate::relay::storage::held::held_dir_beside(d.folders()).join(&row.package).exists(), "and its sealed copy");
    assert_eq!(st.db.held_counts().unwrap().restored, 1, "the restore is counted");
    assert_eq!(status(&finish_in(&st, row.id, &grant, &own.key, &key, d.folders(), now)), 404, "and cannot be done twice");
}

/// THE APPS' RESTORE ROUND-TRIPS A SEALED POST ON A TEMP RELAY (rung 5): the protocol's
/// `held_seal::restore_with` (the order the desktop app's Restore runs, engine/held.rs) opens part
/// 0 and every file with the owner's key and sends each file in pieces (`send_in_pieces`, cut small
/// here so every file takes several) to the relay's own piece and finish handlers; the post and its
/// files are back, byte for byte. It lives in the protocol so this relay test reaches it without
/// reaching the client (scripts/crate-layers.json), and so it runs in the relay's battery too.
///
/// Seen red 2026-11-20 with `restore_with` skipping the poster parts (`.chain(f.poster_part)` taken
/// out): the finish refused with the poster missing (left `Err("finish refused: 409 Conflict")`,
/// right `Ok("restored")`).
#[test]
fn the_apps_restore_round_trips_a_sealed_post() {
    use std::io::Read;
    let d = dirs("restore_app");
    let (st, _id, _ada, own, row) = one_held("restore_app", &d);
    let kp = KyberKeypair::from_seed(&derive_kyber_seed(&own.seed)).unwrap();
    let restore = signed(&st, &own, "held_restore", "held", row.id, 0);
    act(&st, &d, &own, restore.clone());
    let grant = grant_of(&restore);
    let now = now_ms();
    let fetch = |part: usize| -> Result<Box<dyn Read>, String> {
        let path = export_part_path(&st, row.id, part as i64, &grant, &own.key, d.folders(), now).ok_or("no such part")?;
        Ok(Box::new(std::fs::File::open(path).map_err(|e| e.to_string())?))
    };
    let upload = |part: usize, key: &str, src: &mut dyn Read| {
        humanity_protocol::held_seal::send_in_pieces(src, 7, |offset, piece| {
            let r = piece_in(&st, row.id, part as i64, &PieceQuery { grant: grant.clone(), offset }, &own.key, key, piece, d.folders(), now);
            if r.status().is_success() { Ok(()) } else { Err(format!("piece refused: {}", r.status())) }
        })
    };
    let finish = |key: &str| {
        let r = finish_in(&st, row.id, &grant, &own.key, key, d.folders(), now);
        if r.status().is_success() { Ok("restored".to_string()) } else { Err(format!("finish refused: {}", r.status())) }
    };
    // W7 (the third review): an item the owner did not choose (another report than the one the list
    // showed) is refused before anything is uploaded. Seen red 2026-11-21 before restore_with compared
    // the manifest: the restore of the wrong report went through ("restored").
    // The fourth review's finding 8: a sibling post of the same report (the post time differs) is
    // refused too. Seen red 2026-11-21 (scratchpad ownfix7-batch1.log, mutant P-2 of
    // ownfix6-rust-mutants.json) with manifest_is comparing package and report alone: "the sibling's
    // restore is refused" failed with Ok("restored").
    use humanity_protocol::held_seal::Chosen;
    let chosen = Chosen { package: &row.package, report_id: row.report_id, author: &row.author_key, stamp: row.stamp as u64 };
    let uploads = std::cell::Cell::new(0);
    for wrong in [Chosen { report_id: row.report_id + 1, ..chosen }, Chosen { stamp: row.stamp as u64 + 1, ..chosen }] {
        let refused = humanity_protocol::held_seal::restore_with(&kp, &wrong, fetch, |_, _, _| { uploads.set(uploads.get() + 1); Ok(()) }, |_| Ok("restored".to_string()));
        assert!(refused.as_ref().is_err_and(|w| w.contains("not the item you chose")), "the sibling's restore is refused: {refused:?}");
    }
    assert_eq!(uploads.get(), 0, "nothing was uploaded");
    assert_eq!(humanity_protocol::held_seal::restore_with(&kp, &chosen, fetch, upload, finish), Ok("restored".to_string()));
    let (content, _) = st.db.post_for_report(&row.author_key, 42).unwrap().expect("the post is back");
    assert!(content.starts_with(MARKER));
    assert_eq!(std::fs::read(d.uploads.join("vid_2.mp4")).unwrap(), format!("{MARKER} video").into_bytes());
    assert_eq!(std::fs::read(d.uploads.join("vid_2.mp4.poster.jpg")).unwrap(), format!("{MARKER} poster").into_bytes());
    assert_eq!(std::fs::read(d.members.join("doc_3.pdf")).unwrap(), format!("{MARKER} members").into_bytes());
    assert!(st.db.held_item(row.id).unwrap().is_none());
}
