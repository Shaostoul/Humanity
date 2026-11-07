// Child of chat/attach_send.rs (#[path]): a picture that could not be cleaned is the person's to
// decide, "Don't send" first, and goes only with their signed choice to keep its details, which the
// relay checks (2026-11-06, docs/BUGS.md BUG-260; the operator: "can we have an option to send or
// refuse?"). The website's half is scripts/tests/upload-keep-web.test.js; the relay's,
// src/relay/upload_keep_tests.rs. Each test was seen red first, recorded at the test.

use super::*;
use crate::gui::pages::chat::attach_step::{answer_server_ask, sent_anyway, verdict, Verdict};
use crate::net::file_clean::Look;
use humanity_protocol::describe_metadata::describe;

/// A JPEG that says where it was taken and whose structure breaks (its EXIF segment claims to run
/// past the end of the file), so neither this app's clean nor the relay's can walk it.
fn broken_photo() -> Vec<u8> {
    let mut out = vec![0xFF, 0xD8, 0xFF, 0xE1];
    out.extend_from_slice(&0x7000u16.to_be_bytes());
    out.extend_from_slice(b"Exif\0\0GPS-LATITUDE-47.6553,-122.6870");
    out.extend(std::iter::repeat(0x55u8).take(300));
    out
}

fn public_job(bytes: Vec<u8>) -> AttachJob {
    AttachJob {
        server: "https://chat.example".into(),
        token: "tok".into(),
        filename: "garden.jpg".into(),
        mime: "image/jpeg".into(),
        body: Body::Bytes(bytes),
        share: false,
        to: Destination::Public,
        limit: ATTACH_MAX_BYTES,
        clean: true,
        allow_unchecked: false,
    }
}

fn signer(n: u8) -> (KeepSigner, String) {
    let (seed, me) = crate::relay::upload_rules::test_identity(n);
    (KeepSigner { seed, me: me.clone() }, me)
}

/// IN A PUBLIC ROOM, A FILE THAT COULD NOT BE CLEANED IS ASKED ABOUT. The attach step's verdict for
/// a file that could not be checked, the switch on, going to a public room: it may be sent, as
/// "Send anyway" (the step then shows "Don't send" first). A private conversation asks the same;
/// the protected setup sends it nowhere.
///
/// Seen red 2026-11-06 against the verdict before this (a public room never took one): "a public
/// room asks" failed (left: Verdict { can_send: false, anyway: false }).
#[test]
fn a_public_file_that_could_not_be_cleaned_is_asked_about() {
    let look: Result<Look, String> = Ok(Look { description: describe(&broken_photo(), "garden.jpg"), unchecked: true });
    assert_eq!(verdict(Some(&look), true, false, false), Verdict { can_send: true, anyway: true }, "a public room asks");
    assert_eq!(verdict(Some(&look), true, true, false), Verdict { can_send: true, anyway: true }, "so does a private conversation");
    assert_eq!(verdict(Some(&look), true, false, true), Verdict { can_send: false, anyway: false }, "the protected setup sends it nowhere");
}

/// "SEND ANYWAY" SENDS IT EXACTLY AS IT IS, WITH THE SIGNED CHOICE, AND THE RELAY STORES IT. Without
/// the person's yes the broken photo is not sent to a room; with it, the upload is the photo as
/// picked, saying `keep_metadata`. This app's signed choice for it passes the relay's own check (and
/// a choice for another file does not), and the very body this app sends, its `keep` part first,
/// is taken by the relay's upload route and stored exactly as sent.
///
/// Seen red 2026-11-06 against `run` before this: "with Send anyway it goes" failed (Err: "it could
/// not be read all the way through, ... a public place does not take such a file.").
#[tokio::test]
async fn send_anyway_sends_it_as_it_is_with_the_signed_choice_and_the_relay_stores_it() {
    let photo = broken_photo();
    let refused = run(public_job(photo.clone()), |_, _, _| Ok("/uploads/x".into()));
    assert_eq!(refused, Err(NOT_CHECKED_PUBLIC.to_string()), "without the person's yes it is not sent");
    let mut seen = None;
    let mut yes = public_job(photo.clone());
    yes.allow_unchecked = true;
    let out = run(yes, |_, _, up| {
        seen = Some(up);
        Ok("/uploads/x".into())
    });
    assert!(out.is_ok(), "with Send anyway it goes: {out:?}");
    let up = seen.unwrap();
    assert_eq!((up.bytes.as_slice(), up.keep_metadata), (photo.as_slice(), true), "exactly as picked, keeping its details");

    // The choice this app signs is one the relay accepts, for this file only.
    let (s, me) = signer(97);
    let choice = crate::relay::upload_rules::keep_choice_from(&s.choice(&photo)).expect("a keep choice");
    assert!(crate::relay::upload_rules::keep_checked(&me, &crate::relay::upload_rules::blake3_hex(&photo), Some(&choice)).is_ok());
    assert!(crate::relay::upload_rules::keep_checked(&me, &crate::relay::upload_rules::blake3_hex(b"another"), Some(&choice)).is_err());

    // The very body this app sends, through the relay's upload route: stored as sent.
    use axum::extract::FromRequest;
    use std::io::Read;
    let state = std::sync::Arc::new(crate::relay::relay::RelayState::new(crate::relay::storage::Storage::open_temp("attach_keep_e2e")));
    state.db.join_server(&me, &me).unwrap();
    state.db.set_role(&me, "verified").unwrap();
    state.upload_tokens.write().await.insert("tok".into(), me.clone());
    let d = |w: &str| humanity_core::test_temp::dir(&format!("attach_keep_e2e_{w}"));
    let (uploads, members, resume, waiting, part) = (d("uploads"), d("members"), d("resume"), d("waiting"), d("part"));
    let body = attach_body::Multipart::new("garden.jpg", "image/jpeg", photo.clone()).with_text_part_first(crate::relay::upload_rules::KEEP_FIELD, &s.choice(&photo));
    let boundary = body.boundary.clone();
    let mut bytes = Vec::new();
    body.reader().read_to_end(&mut bytes).unwrap();
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/upload")
        .header(axum::http::header::CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
        .header(axum::http::header::CONTENT_LENGTH, bytes.len().to_string())
        .body(axum::body::Body::from(bytes))
        .unwrap();
    let headers = req.headers().clone();
    let multipart = axum::extract::Multipart::from_request(req, &()).await.unwrap();
    let folders = crate::relay::files::Folders { uploads: &uploads, members: &members, resume: &resume, waiting: &waiting };
    let query = crate::relay::api::UploadQuery { token: Some("tok".into()), keep_metadata: Some(1), ..Default::default() };
    let done = crate::relay::api::upload_file_in(&state, folders, &part, query, headers, multipart).await.expect("the relay takes it");
    assert_eq!(std::fs::read(uploads.join(done.0["filename"].as_str().unwrap())).unwrap(), photo, "stored exactly as sent");
}

/// THE SERVER'S 422 ASKS, AND "SEND ANYWAY" SENDS IT AGAIN AS IT IS. A room's file sent as cleaned,
/// whose upload the server refused because it could not remove the details: the chat does not say
/// it failed, it asks (the question waits with the file, for the conversation it was for). "Don't
/// send" says it was not sent; "Send anyway" is the same file, not cleaned, so it goes keeping its
/// details with the signed choice.
///
/// Seen red 2026-11-06 against `drain` before this: "the person is asked" failed (the refusal was
/// shown as "The file was not sent: ..." and nothing waited).
#[test]
fn the_servers_422_asks_and_send_anyway_resends() {
    let mut gs = GuiState::default();
    gs.chat_active_channel = "global".into();
    let job = public_job(vec![1, 2, 3]);
    gs.files_flow.sent_public = Some(job.clone());
    let (tx, rx) = std::sync::mpsc::channel();
    tx.send(Err(format!("{CANNOT_CLEAN_TAG}{}", crate::relay::upload_rules::PICTURE_UNREADABLE))).unwrap();
    gs.clipboard_upload = Some(("global".into(), rx));
    drain(&egui::Context::default(), &mut gs);
    let (channel, waiting) = gs.files_flow.server_ask.take().expect("the person is asked");
    assert_eq!((channel.as_str(), waiting.filename.as_str()), ("global", "garden.jpg"));
    assert!(gs.pending_notices.is_empty(), "not told it failed: {:?}", gs.pending_notices);

    answer_server_ask(&mut gs, waiting.clone(), false);
    assert_eq!(gs.pending_notices, vec![humanity_core::i18n::t("upload_keep.not_sent").to_string()], "Don't send says so");
    let again = sent_anyway(waiting);
    assert!(!again.clean && again.allow_unchecked, "Send anyway: the file as picked, its details kept by signed choice");

    // Any other refusal is said as before.
    let mut gs = GuiState::default();
    gs.chat_active_channel = "global".into();
    gs.files_flow.sent_public = Some(job);
    let (tx, rx) = std::sync::mpsc::channel();
    tx.send(Err("File too large".to_string())).unwrap();
    gs.clipboard_upload = Some(("global".into(), rx));
    drain(&egui::Context::default(), &mut gs);
    assert!(gs.files_flow.server_ask.is_none());
    assert_eq!(gs.pending_notices, vec!["The file was not sent: File too large".to_string()]);
}
