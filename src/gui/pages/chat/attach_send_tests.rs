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
    gs
}

/// Run `job` with an uploader that records what it was handed and answers `FAKE_URL`.
fn run_recorded(job: AttachJob) -> (Result<String, String>, Option<Upload>) {
    let mut seen = None;
    let out = run(job, |server, key, up| {
        assert_eq!((server, key), ("https://chat.example", "me"), "the job's server and identity");
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
            public_key: "me".into(),
            filename: "big.png".into(),
            mime: "image/png".into(),
            bytes: big.clone(),
            share: false,
            to: Destination::of(channel),
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
