// Child of chat/attach_send.rs (#[path]): what a file sent into the chat is uploaded as, for each
// kind of conversation, with the network replaced by a recorder (10k of
// docs/design/blocking-and-safe-mode.md, 2026-10-10). Each test was seen failing once on purpose,
// recorded at the test.

use super::*;
use crate::net::dm_pq::{decrypt_attachment, parse_file_marker};

const FAKE_URL: &str = "/uploads/0123abcd.enc";

/// A signed-in app viewing `channel`, with no seed (so the phrase guard has nothing to match).
fn viewing(channel: &str) -> GuiState {
    let mut gs = GuiState::default();
    gs.server_url = "https://chat.example".into();
    gs.profile_public_key = "me".into();
    gs.chat_active_channel = channel.into();
    // Signed in: the server sent this connection its upload token (2026-10-11).
    gs.upload_token = Some(("https://chat.example".into(), "tok-chat-example".into()));
    gs
}

/// Run `job` with an uploader that records what it was handed and answers `FAKE_URL`.
fn run_recorded(job: AttachJob) -> (Result<String, String>, Option<Upload>) {
    let mut seen = None;
    let out = run(job, |server, key, up| {
        assert_eq!((server, key), ("https://chat.example", "tok-chat-example"), "the job's server and this connection's upload token");
        seen = Some(up);
        Ok(FAKE_URL.to_string())
    });
    (out, seen)
}

/// A file's bytes that cannot appear by chance in ciphertext or framing.
fn file_bytes() -> Vec<u8> {
    b"PRIVATE-PICTURE-BYTES ".repeat(40)
}

/// A private upload: ciphertext under a neutral name and type, `encrypted=1`, never shared, and
/// the text sent is a marker whose key opens exactly that ciphertext back to the file.
fn assert_private(label: &str, out: Result<String, String>, seen: Option<Upload>, plain: &[u8], name: &str, mime: &str) {
    let up = seen.unwrap_or_else(|| panic!("{label}: nothing was uploaded"));
    assert!(up.encrypted, "{label}: uploaded with encrypted=1");
    assert!(!up.share, "{label}: a private file is never published to Shared Files");
    assert_eq!((up.filename.as_str(), up.mime.as_str()), ("attachment.enc", "application/octet-stream"), "{label}: neutral name and type");
    assert_ne!(up.bytes, plain, "{label}: the uploader was handed the file's own bytes");
    assert!(!up.bytes.windows(21).any(|w| w == &plain[..21]), "{label}: the file's bytes appear in the upload");
    let text = out.unwrap_or_else(|e| panic!("{label}: {e}"));
    assert!(!text.contains(FAKE_URL), "{label}: a plain URL was sent instead of a marker");
    let att = parse_file_marker(&text).unwrap_or_else(|| panic!("{label}: the text sent is not a marker: {text}"));
    assert_eq!((att.url.as_str(), att.name.as_str(), att.mime.as_str(), att.size), (FAKE_URL, name, mime, plain.len() as u64), "{label}");
    assert_eq!(decrypt_attachment(&up.bytes, &att.k, &att.n).expect("the marker's key opens the upload"), plain, "{label}");
}

/// Which conversations encrypt the file. Seen red 2026-10-10 with `encrypts_file` answering
/// `Self::DirectMessage` only (the old `is_dm`): "a group encrypts" failed.
#[test]
fn each_kind_of_conversation_says_whether_it_encrypts() {
    assert_eq!(Destination::of("dm:abc"), Destination::DirectMessage);
    assert_eq!(Destination::of("p2pgroup:g1"), Destination::Group);
    assert_eq!(Destination::of("scratchpad"), Destination::Scratchpad);
    for public in ["general", "announcements", "commons:general", ""] {
        assert_eq!(Destination::of(public), Destination::Public, "{public}");
    }
    assert!(Destination::DirectMessage.encrypts_file(), "a DM encrypts");
    assert!(Destination::Group.encrypts_file(), "a group encrypts");
    assert!(Destination::Scratchpad.encrypts_file(), "the scratchpad encrypts");
    assert!(!Destination::Public.encrypts_file(), "a public channel does not");
}

/// THE SCRATCHPAD IS PRIVATE (the 2026-10-10 review, item 10): its label says local-only, so a
/// file put there goes up encrypted like a DM's (ciphertext, `encrypted=1`, never shared, a 3D
/// model included), and its marker, with the key, stays in the scratchpad on this device: nothing
/// is sent over the socket.
/// Seen red 2026-10-10 with `Destination::of` answering Public for "scratchpad" (the old rule):
/// "scratchpad, picked: uploaded with encrypted=1" failed.
#[test]
fn a_file_in_the_scratchpad_is_encrypted_and_its_key_stays_on_this_device() {
    let mut gs = viewing("scratchpad");
    let picked = attach_job(&mut gs, "trail map.jpg", file_bytes()).expect("a picked file");
    let (out, seen) = run_recorded(picked);
    assert_private("scratchpad, picked", out.clone(), seen, &file_bytes(), "trail map.jpg", "image/jpeg");
    let pasted = paste_job(&gs, file_bytes()).expect("a pasted image");
    let (pasted_out, pasted_seen) = run_recorded(pasted);
    assert_private("scratchpad, pasted", pasted_out, pasted_seen, &file_bytes(), "pasted-image.png", "image/png");
    let model = attach_job(&mut gs, "bridge.stl", vec![1, 2, 3]).unwrap();
    assert!(!model.share, "a model there is never shared to the public library");

    let (client, sent) = crate::net::ws_client::WsClient::recording();
    gs.ws_client = Some(client);
    let (tx, rx) = std::sync::mpsc::channel();
    tx.send(out).unwrap();
    gs.clipboard_upload = Some(("scratchpad".to_string(), rx));
    drain(&egui::Context::default(), &mut gs);
    assert_eq!(gs.chat_messages.len(), 1, "the marker is kept in the scratchpad");
    assert!(parse_file_marker(&gs.chat_messages[0].content).is_some() && gs.chat_messages[0].channel == "scratchpad");
    assert!(sent.try_iter().next().is_none(), "and nothing is sent to the server");
}

/// The proof list's three private cases and the DM's picked file: a pasted image in a DM, a
/// picked file in a group, a pasted image in a group (and a picked file in a DM) each upload
/// ciphertext with encrypted=1 and send a marker, never a plain URL.
/// Seen red 2026-10-10 with `attach_job` and `paste_job` building `to: Destination::Public`
/// (the plain upload): "dm, picked: uploaded with encrypted=1" failed.
#[test]
fn a_picked_or_pasted_file_in_a_dm_or_a_group_goes_up_encrypted_with_a_marker() {
    for channel in ["dm:abc", "p2pgroup:g1"] {
        let mut gs = viewing(channel);
        let picked = attach_job(&mut gs, "trail map.jpg", file_bytes()).expect("a picked file under the cap");
        let (out, seen) = run_recorded(picked);
        assert_private(&format!("{channel}, picked"), out, seen, &file_bytes(), "trail map.jpg", "image/jpeg");

        let pasted = paste_job(&gs, file_bytes()).expect("a pasted image under the cap");
        let (out, seen) = run_recorded(pasted);
        assert_private(&format!("{channel}, pasted"), out, seen, &file_bytes(), "pasted-image.png", "image/png");
    }
}

/// A public channel keeps the plain upload: the file's own bytes and name, no encrypted flag, and
/// the text sent is the URL. A 3D model there still goes to Shared Files; in a DM it does not.
/// Seen red 2026-10-10 with `encrypts_file` answering true for every kind: "public, picked: the
/// file's own bytes" failed.
#[test]
fn a_public_channel_uploads_the_plain_file_and_sends_its_url() {
    let mut gs = viewing("general");
    let (out, seen) = run_recorded(attach_job(&mut gs, "notes.pdf", file_bytes()).unwrap());
    let up = seen.unwrap();
    assert_eq!(up.bytes, file_bytes(), "public, picked: the file's own bytes");
    assert!(!up.encrypted && up.filename == "notes.pdf" && up.mime == "application/pdf");
    assert_eq!(out.unwrap(), FAKE_URL, "public, picked: the URL is sent");

    let (out, seen) = run_recorded(paste_job(&gs, file_bytes()).unwrap());
    assert!(!seen.unwrap().encrypted, "public, pasted");
    assert_eq!(out.unwrap(), FAKE_URL);

    assert!(attach_job(&mut gs, "bridge.stl", vec![1, 2, 3]).unwrap().share, "a model in public is shared");
    let mut dm = viewing("dm:abc");
    assert!(!attach_job(&mut dm, "bridge.stl", vec![1, 2, 3]).unwrap().share, "a model in a DM is not");
}

/// The size cap holds on every path: a picked file and a pasted image over it are refused before
/// any job exists, and `run` itself refuses one without calling the uploader.
/// Seen red 2026-10-10 with the cap removed from `paste_job` (as the paste path had none
/// before): "a pasted image over the cap" failed.
#[test]
fn nothing_over_the_size_cap_is_uploaded_on_any_path() {
    let big = vec![7u8; ATTACH_MAX_BYTES as usize + 1];
    for channel in ["dm:abc", "p2pgroup:g1", "general"] {
        let mut gs = viewing(channel);
        assert!(attach_job(&mut gs, "big.png", big.clone()).is_none(), "{channel}: a picked file over the cap");
        assert!(paste_job(&gs, big.clone()).is_none(), "{channel}: a pasted image over the cap");
        let job = AttachJob {
            server: gs.server_url.clone(),
            token: "tok".into(),
            filename: "big.png".into(),
            mime: "image/png".into(),
            body: Body::Bytes(big.clone()),
            share: false,
            to: Destination::of(channel),
            limit: ATTACH_MAX_BYTES,
        };
        let out = run(job, |_, _, _| panic!("{channel}: the uploader was called over the cap"));
        assert!(out.is_err(), "{channel}");
    }
}

/// Step F's guard on a file's NAME: the name travels inside the marker, where the guard on the
/// message text cannot read it, so a picked file whose name holds four or more of the person's
/// own recovery-phrase words in order is not sent, with the guard's line.
/// Seen red 2026-10-10 with the `guard_stops` call left out of `attach_job`: "a name holding
/// the phrase is not sent" failed.
#[test]
fn a_file_name_holding_the_recovery_phrase_is_not_sent() {
    let seed = [5u8; 32];
    let phrase = crate::net::identity::mnemonic_from_seed(&seed).expect("a phrase seed");
    let words: Vec<&str> = phrase.split(' ').collect();
    for channel in ["dm:abc", "p2pgroup:g1", "general"] {
        let mut gs = viewing(channel);
        gs.private_key_bytes = Some(seed.to_vec());
        let name = format!("backup {} {} {} {} {}.txt", words[2], words[3], words[4], words[5], words[6]);
        assert!(attach_job(&mut gs, &name, b"x".to_vec()).is_none(), "{channel}: a name holding the phrase is not sent");
        assert!(gs.pending_notices.iter().any(|n| n == crate::net::warnings::GUARD_LINE), "{channel}: the guard's line");
        assert!(attach_job(&mut gs, "holiday.txt", b"x".to_vec()).is_some(), "{channel}: an ordinary name goes");
    }
}

/// A finished upload's marker goes only to the conversation it was made for: after moving to
/// another view it is not sent anywhere (its key would open the file for whoever reads it there),
/// and the person is told. A plain URL keeps the old behaviour (sent to the view on screen; the
/// scratchpad here, which keeps it on this device).
/// Seen red 2026-10-10 with the conversation check left out of `drain`: "the marker is not
/// sent into another view" failed (it landed in the scratchpad).
#[test]
fn a_private_files_marker_is_sent_only_to_the_conversation_it_was_made_for() {
    let ctx = egui::Context::default();
    let att = crate::net::dm_pq::DmAttachment {
        url: FAKE_URL.into(),
        k: "k".into(),
        n: "n".into(),
        name: "a.png".into(),
        mime: "image/png".into(),
        size: 3,
    };
    let marker = crate::net::dm_pq::build_file_marker(&att);
    let finished = |made_for: &str, text: &str| {
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(Ok(text.to_string())).unwrap();
        Some((made_for.to_string(), rx))
    };

    let mut gs = viewing("scratchpad");
    gs.clipboard_upload = finished("p2pgroup:g1", &marker);
    drain(&ctx, &mut gs);
    assert!(gs.clipboard_upload.is_none(), "the result is taken");
    assert!(gs.chat_messages.is_empty(), "the marker is not sent into another view");
    assert!(gs.pending_notices.iter().any(|n| n == LEFT_BEFORE_UPLOAD), "the person is told");

    gs.clipboard_upload = finished("scratchpad", &marker);
    drain(&ctx, &mut gs);
    assert_eq!(gs.chat_messages.len(), 1, "the same view: sent");

    gs.clipboard_upload = finished("general", FAKE_URL);
    drain(&ctx, &mut gs);
    assert_eq!(gs.chat_messages.len(), 2, "a plain URL still goes to the view on screen");
    assert_eq!(gs.chat_messages[1].content, FAKE_URL);
}

/// EACH FILE GETS THE LIMIT FOR WHERE IT IS GOING: in a server's room the person's own limit on
/// that server (`upload_limit_mb`, 2026-10-10); in a private conversation their own private-file
/// limit there (`private_limit_mb`, 2026-10-11: the role's `max_private_file_mb`, which the relay
/// holds an encrypted upload to instead of the room limit); each 6 MB until the server has said,
/// and a private one never past what the receiving side opens. Only their own entry of the peer
/// list is read. A file over the limit is refused with a line saying why.
/// Seen red 2026-10-10 with `limit_for` returning the server's limit for private conversations
/// too: "a DM keeps the 6 MB limit". Seen red 2026-10-11 with `note_upload_limit` not reading
/// `private_limit_mb`: "my private limit is read from my own entry" (left: None / right:
/// Some(20971520)).
#[test]
fn each_file_gets_the_limit_for_where_it_is_going() {
    let mb = 1024 * 1024;
    let room = |n: u64| Limits { room: Some(n * mb), private: None };
    assert_eq!(limit_for(room(300), Destination::Public), 300 * mb, "a room: my limit on this server");
    assert_eq!(limit_for(Limits::default(), Destination::Public), ATTACH_MAX_BYTES, "until the server says, the fallback");
    for private in [Destination::DirectMessage, Destination::Group, Destination::Scratchpad] {
        assert_eq!(limit_for(room(300), private), ATTACH_MAX_BYTES, "until the server says, 6 MB in a DM ({private:?})");
        let both = Limits { room: Some(2 * mb), private: Some(20 * mb) };
        assert_eq!(limit_for(both, private), 20 * mb, "the private limit, not the room's ({private:?})");
    }
    let huge = Limits { room: None, private: Some(1 << 50) };
    assert_eq!(limit_for(huge, Destination::DirectMessage), private_ceiling(), "never past what the receiving side opens");
    assert_eq!(private_ceiling(), 1024 * mb, "the relay's own top of the range, 1 GB");

    let mut gs = viewing("general");
    gs.profile_public_key = "me".into();
    note_upload_limit(&mut gs, &serde_json::json!({ "type": "peer_list", "peers": [
        { "public_key": "someone", "upload_limit_mb": 999, "private_limit_mb": 999 },
        { "public_key": "me", "upload_limit_mb": 300, "private_limit_mb": 20 },
    ] }));
    assert_eq!(Limits::of(&gs).room, Some(300 * mb), "only my own entry is read");
    assert_eq!(Limits::of(&gs).private, Some(20 * mb), "my private limit is read from my own entry");
    note_upload_limit(&mut gs, &serde_json::json!({ "type": "peer_list", "peers": [{ "public_key": "me" }] }));
    assert_eq!(Limits::of(&gs), Limits { room: Some(300 * mb), private: Some(20 * mb) }, "a list without them changes nothing");

    let ten_mb = vec![7u8; 10 * mb as usize];
    assert!(attach_job(&mut gs, "clip.mp4", ten_mb.clone()).is_some(), "a 10 MB video goes into a room");
    let mut dm = viewing("dm:abc");
    note_upload_limit(&mut dm, &serde_json::json!({ "peers": [{ "public_key": "me", "upload_limit_mb": 300 }] }));
    assert!(attach_job(&mut dm, "clip.mp4", ten_mb.clone()).is_none(), "but not into a DM before the server says");
    assert!(dm.ws_status.contains("private conversation can be up to 6 MB"), "and the person is told why: {}", dm.ws_status);
    note_upload_limit(&mut dm, &serde_json::json!({ "peers": [{ "public_key": "me", "private_limit_mb": 20 }] }));
    assert!(attach_job(&mut dm, "clip.mp4", ten_mb).is_some(), "and into a DM once the server allows 20 MB");
}

/// THE LIMITS BELONG TO ONE SERVER AND ONE IDENTITY (round 3 of docs/design/media-showcase.md, the
/// review's finding 10): a private-file limit one server told this person was kept after a switch
/// to another server, or a restore from a recovery phrase, and let a file past what that server or
/// person may send. Now they count only where they were told, the 6 MB fallback elsewhere, and a
/// list from another server or identity starts them again.
///
/// Seen red 2026-10-11 with `Limits::of` reading the limits whoever and wherever they were told
/// (as the two bare fields of round 2 did): "another server: the fallback" (left: Some(20971520) /
/// right: None).
#[test]
fn the_limits_belong_to_one_server_and_one_identity() {
    let mb = 1024 * 1024;
    let mut gs = viewing("dm:abc");
    note_upload_limit(&mut gs, &serde_json::json!({ "peers": [{ "public_key": "me", "upload_limit_mb": 300, "private_limit_mb": 20 }] }));
    assert_eq!(Limits::of(&gs).private, Some(20 * mb), "told here");

    gs.server_url = "https://other.example".into();
    assert_eq!(Limits::of(&gs).private, None, "another server: the fallback");
    assert_eq!(room_limit_here(&gs), None);
    note_upload_limit(&mut gs, &serde_json::json!({ "peers": [{ "public_key": "me", "upload_limit_mb": 50 }] }));
    assert_eq!(Limits::of(&gs), Limits { room: Some(50 * mb), private: None }, "its own list starts again: nothing of the first server's");

    gs.server_url = "https://chat.example".into();
    note_upload_limit(&mut gs, &serde_json::json!({ "peers": [{ "public_key": "me", "private_limit_mb": 20 }] }));
    gs.profile_public_key = "restored".into();
    assert_eq!(Limits::of(&gs), Limits::default(), "another identity: the fallback");
}

/// A PICKED FILE IS READ BY THE WORKER, NEVER ON THE FRAME, AND KEPT TO ONE COPY (round 3, the
/// review's finding 8: a 1 GB private file was read on the UI thread, then held as plaintext,
/// ciphertext and the upload's own copy at once). The job holds the path and size; `run` reads it;
/// the encryption is done in that same buffer; and the multipart body streams its framing around
/// the buffer instead of copying it.
///
/// Seen red 2026-10-11, once for each copy it guards: with `attach_file_job` reading the file as
/// round 2's `start_attach` did (`std::fs::read` on the frame), "the job holds the path, not the
/// bytes" (left: Bytes([102, 105, 114, ...]), the file read already); with
/// `encrypt_attachment_in_place` making its ciphertext in a new buffer, "the ciphertext is the
/// file's own buffer" (left: 0x2bfb5f1ead0 / right: 0x2bfb5f1eaa0); and with `Multipart::new`
/// copying the bytes, "never copied into a second buffer with its framing" (two addresses).
#[test]
fn a_picked_file_is_read_by_the_worker_and_kept_to_one_copy() {
    let mut gs = viewing("dm:abc");
    let path = crate::test_temp::file("attach_worker_read", "txt");
    std::fs::write(&path, b"first version").unwrap();
    let job = attach_file_job(&mut gs, &path).expect("a job");
    assert_eq!(job.body, Body::File { path: path.to_path_buf(), len: 13 }, "the job holds the path, not the bytes");
    std::fs::write(&path, b"second, later").unwrap();
    let (out, seen) = run_recorded(job);
    let up = seen.expect("uploaded");
    let att = parse_file_marker(&out.unwrap()).expect("a marker");
    assert_eq!(decrypt_attachment(&up.bytes, &att.k, &att.n).unwrap(), b"second, later", "the worker reads what is there when it runs");

    // In place: the ciphertext is the very buffer the file was read into.
    let mut buf = attach_body::read(Body::File { path: path.to_path_buf(), len: 13 }, 1000).unwrap();
    let before = buf.as_ptr();
    let (k, n) = crate::net::dm_pq::encrypt_attachment_in_place(&mut buf).unwrap();
    assert_eq!(buf.as_ptr(), before, "the ciphertext is the file's own buffer");
    assert_eq!(decrypt_attachment(&buf, &k, &n).unwrap(), b"second, later");

    // Streamed: the body is the framing around that same buffer, and reads as the whole request.
    let ptr = buf.as_ptr();
    let body = attach_body::Multipart::new("a.enc", "application/octet-stream", buf);
    assert_eq!(body.file.get_ref().as_ptr(), ptr, "never copied into a second buffer with its framing");
    let (len, boundary) = (body.len(), body.boundary.clone());
    let mut sent = Vec::new();
    std::io::Read::read_to_end(&mut body.reader(), &mut sent).unwrap();
    assert_eq!(sent.len() as u64, len, "Content-Length is the body's length");
    assert!(sent.starts_with(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a.enc\"").as_bytes()));
    assert!(sent.ends_with(format!("\r\n--{boundary}--\r\n").as_bytes()));

    // A file that grew past the cap since it was picked is refused, not read whole.
    std::fs::write(&path, vec![1u8; 2000]).unwrap();
    assert!(attach_body::read(Body::File { path: path.to_path_buf(), len: 13 }, 1000).is_err());
}

/// EVERY UPLOAD CARRIES THIS CONNECTION'S TOKEN, never the person's key (the upload review,
/// 2026-10-11: a bare `?key=` proved nothing about the sender). The token comes from this person's
/// own entry of the server's peer list and is kept with that server; on another server, or before
/// it arrives, nothing is sent and the person is told why. Seen red 2026-10-11 with
/// `upload_token_here` ignoring the server: "on another server the token does not count".
#[test]
fn every_upload_carries_this_connections_token_for_this_server() {
    let mut gs = viewing("general");
    gs.upload_token = None;
    note_upload_limit(&mut gs, &serde_json::json!({ "type": "peer_list", "peers": [
        { "public_key": "someone", "upload_token": "theirs" },
        { "public_key": "me", "upload_token": "mine", "upload_limit_mb": 2 },
    ] }));
    assert_eq!(upload_token_here(&gs).as_deref(), Some("mine"), "my own entry's token");
    let job = attach_job(&mut gs, "note.txt", b"hello".to_vec()).expect("a small file goes");
    assert_eq!(job.token, "mine", "carrying my token");

    gs.server_url = "https://other.example".into();
    assert_eq!(upload_token_here(&gs), None, "on another server the token does not count");
    assert!(attach_job(&mut gs, "note.txt", b"hello".to_vec()).is_none(), "so nothing is sent there");
    assert_eq!(gs.ws_status, NOT_SIGNED_IN_YET, "and the person is told why");
}

/// NO UPLOAD TOKEN IN WHAT A FAILED UPLOAD SAYS (the 2026-10-11 review of the Media page): an
/// upload carries the token in its address (`?token=`), ureq writes the whole address into its
/// error words, and those words reach the screen and run.log. The words keep the address, with
/// the token taken out.
/// Seen red 2026-10-11 with `upload_error_words` returning ureq's words as they were: "the token
/// is not in the words" (they held "token=SECRET-TOKEN-1234&encrypted=1").
#[test]
fn no_upload_token_in_what_a_failed_upload_says() {
    // A closed port on this machine: the upload fails at once with a transport error.
    let err = upload_file_blocking_ext("http://127.0.0.1:9", "SECRET-TOKEN-1234", "a.png", "image/png", vec![1, 2, 3], false, true)
        .expect_err("nothing listens there");
    assert!(!err.contains("SECRET-TOKEN-1234"), "the token is not in the words: {err}");
    assert!(err.contains("could not be reached"), "they still say what went wrong: {err}");
    // And the rule itself, on the shapes it meets: a query, a token last, two in one text.
    use crate::gui::widgets::image_cache::hide_token;
    assert_eq!(hide_token("http://h/api/upload?token=abc&share=1"), "http://h/api/upload?token=(hidden)&share=1");
    assert_eq!(hide_token("GET http://h/x?token=abc: refused"), "GET http://h/x?token=(hidden) refused");
    assert_eq!(hide_token("token=a token=b"), "token=(hidden) token=(hidden)");
    assert_eq!(hide_token("no secrets here"), "no secrets here");
}
