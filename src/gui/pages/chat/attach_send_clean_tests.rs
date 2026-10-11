// Child of chat/attach_send.rs (#[path]): every chat file is cleaned on this device before it is
// encrypted or uploaded (2026-10-11, docs/design/media-showcase.md, the files section, parts 1 and
// 2), with the network replaced by a recorder.

use super::*;
use crate::net::dm_pq::{decrypt_attachment, file_marker_unchecked, parse_file_marker};
use crate::relay::core::describe_metadata::describe;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/describe").join(name)).unwrap()
}

fn job(to: &str, name: &str, bytes: Vec<u8>) -> AttachJob {
    AttachJob {
        server: "https://chat.example".into(),
        token: "tok".into(),
        filename: name.into(),
        mime: mime_for_filename(name).into(),
        body: Body::Bytes(bytes),
        share: false,
        to: Destination::of(to),
        limit: ATTACH_MAX_BYTES,
        clean: true,
        allow_unchecked: false,
    }
}

fn recorded(job: AttachJob) -> (Result<String, String>, Option<Upload>) {
    let mut seen = None;
    let out = run(job, |_, _, up| {
        seen = Some(up);
        Ok("/uploads/x".to_string())
    });
    (out, seen)
}

/// A phone photo sent to a DM leaves without where it was taken: what the recipient decrypts says
/// nothing, and the marker's size is the cleaned file's. In a room the plain upload is cleaned too,
/// and keeps nothing; with the switch off it goes exactly as picked, with `keep_metadata=1`.
/// Seen red 2026-10-11 with the clean in `run` taken out: "a DM's photo leaves saying nothing"
/// failed (the recipient's copy still said where it was taken).
#[test]
fn a_photo_leaves_without_where_it_was_taken_on_every_path() {
    let photo = fixture("jpeg-phone.jpg");
    let (out, seen) = recorded(job("dm:ann", "IMG_0001.JPG", photo.clone()));
    let up = seen.expect("uploaded");
    let att = parse_file_marker(&out.unwrap()).expect("a marker");
    let plain = decrypt_attachment(&up.bytes, &att.k, &att.n).unwrap();
    assert!(describe(&plain, "x.jpg").details.is_empty(), "a DM's photo leaves saying nothing");
    assert_eq!(att.size, plain.len() as u64, "the marker states the cleaned size");
    assert!(plain.len() < photo.len());

    let (_, seen) = recorded(job("general", "IMG_0001.JPG", photo.clone()));
    let up = seen.unwrap();
    assert!(describe(&up.bytes, "x.jpg").details.is_empty() && !up.keep_metadata, "a room's photo is cleaned too");

    let mut keep = job("general", "IMG_0001.JPG", photo.clone());
    keep.clean = false;
    let (_, seen) = recorded(keep);
    let up = seen.unwrap();
    assert_eq!(up.bytes, photo, "kept by choice: exactly as picked");
    assert!(up.keep_metadata, "and the relay is told to keep it");
    assert!(upload_query("t", UploadFlags { share: false, encrypted: false, keep_metadata: true }).ends_with("&keep_metadata=1"));
}

/// A file that could not be checked: not to a public place (nothing uploaded); not to a DM unless
/// the person said Send anyway; with it, its marker says `unchecked`.
/// Seen red 2026-10-11 with `run` sending such a file to a room: "nothing goes to a public place".
#[test]
fn a_file_that_could_not_be_checked() {
    let broken = fixture("mp4-broken-box.mp4");
    let out = run(job("general", "clip.mp4", broken.clone()), |_, _, _| panic!("nothing goes to a public place"));
    assert_eq!(out.unwrap_err(), NOT_CHECKED_PUBLIC);
    let out = run(job("dm:ann", "clip.mp4", broken.clone()), |_, _, _| panic!("not without Send anyway"));
    assert_eq!(out.unwrap_err(), NOT_CHECKED);
    let mut anyway = job("dm:ann", "clip.mp4", broken.clone());
    anyway.allow_unchecked = true;
    let (out, seen) = recorded(anyway);
    let text = out.unwrap();
    assert!(file_marker_unchecked(&text), "its marker says unchecked");
    assert!(seen.unwrap().encrypted);
    let (out, _) = recorded(job("dm:ann", "IMG_0001.JPG", fixture("jpeg-phone.jpg")));
    assert!(!file_marker_unchecked(&out.unwrap()), "a checked file's does not");
}
