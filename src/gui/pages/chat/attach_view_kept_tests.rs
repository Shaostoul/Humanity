// Child of chat/attach_view.rs (#[path]): the files section's receiving side (2026-10-11,
// docs/design/media-showcase.md): the recipient's line under a file its sender could not check
// (part 2), and an opened private file kept on this device so it opens after the server's copy is
// gone (part 4). The network is a function handing back the ciphertext, or failing as a server
// that removed the file would.

use super::*;
use crate::net::dm_pq::{build_file_marker_unchecked, encrypt_attachment};

fn sealed(bytes: &[u8], url: &str, name: &str, mime: &str) -> (DmAttachment, Vec<u8>) {
    let (ct, k, n) = encrypt_attachment(bytes).expect("encrypt");
    (DmAttachment { url: url.into(), k, n, name: name.into(), mime: mime.into(), size: bytes.len() as u64 }, ct)
}

fn painted_texts(ctx: &egui::Context, cache: &mut ImageCache, att: &DmAttachment, plan: &Plan) -> Vec<String> {
    let theme = crate::gui::theme::load_theme();
    let mut viewer = None;
    let mut texts = Vec::new();
    let out = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| draw(ui, &theme, cache, &mut viewer, "Ann", att, plan, theme.bg_primary()));
    });
    for clipped in &out.shapes {
        if let egui::Shape::Text(t) = &clipped.shape {
            texts.push(t.galley.text().to_string());
        }
    }
    texts
}

/// The recipient sees one line under a file whose marker says `unchecked`, and none under a checked
/// one; the plan reads it from the message's own marker. Seen red 2026-10-11 with `draw` not
/// drawing the line: "the unchecked line is under it" failed.
#[test]
fn a_file_its_sender_could_not_check_says_so() {
    let (att, _) = sealed(b"%PDF-1.4 notes", "/uploads/u.enc", "clip.mp4", "video/mp4");
    let mut gs = GuiState::default();
    gs.profile_public_key = "me".into();
    gs.server_url = "http://127.0.0.1:9".into();
    gs.settings.files_in_messages.accept_from = crate::net::files_policy::AcceptFrom::Anyone;
    let msg = ChatMessage { sender_name: "Ann".into(), sender_key: "ann".into(), content: build_file_marker_unchecked(&att), channel: "dm:ann".into(), ..Default::default() };
    let p = plan(&gs, &msg, &att);
    assert!(p.unchecked, "the plan reads the marker's unchecked");
    let ctx = egui::Context::default();
    let texts = painted_texts(&ctx, &mut gs.image_cache, &att, &p);
    assert!(texts.iter().any(|t| t == crate::net::files_policy::UNCHECKED_LINE), "the unchecked line is under it: {texts:?}");
    let checked = Plan { unchecked: false, ..p.clone() };
    let texts = painted_texts(&ctx, &mut gs.image_cache, &att, &checked);
    assert!(!texts.iter().any(|t| t == crate::net::files_policy::UNCHECKED_LINE), "none under a checked file");
}

/// A picture opened once opens again after the server's copy is gone (from the kept copy), and so
/// does a file saved once; a kept copy that does not open is forgotten and the server asked.
/// Seen red 2026-10-11 with `open_kept` not keeping what it fetched: "it opens from the kept copy"
/// failed (the second open asked the server, which no longer had it).
#[test]
fn an_opened_file_still_opens_after_the_server_removed_it() {
    let dir = crate::test_temp::dir("view_kept");
    let kept = KeptFiles::in_dir(dir.to_path_buf(), &[5u8; 32], 1 << 30);
    let picture = b"\x89PNG not really, but private".to_vec();
    let (att, ct) = sealed(&picture, "/uploads/p.enc", "p.png", "image/png");
    let gone = |_: &str, _: usize| -> Result<Vec<u8>, String> { Err("404: removed after 30 days".into()) };

    let first = open_kept("http://s/uploads/p.enc", &att.k, &att.n, Some(&kept), |_, _| Ok(ct.clone())).unwrap();
    assert_eq!(first, picture);
    assert_eq!(open_kept("http://s/uploads/p.enc", &att.k, &att.n, Some(&kept), gone).unwrap(), picture, "it opens from the kept copy");
    assert!(open_kept("http://s/uploads/p.enc", &att.k, &att.n, None, gone).is_err(), "without the kept copy it would not");

    // A kept copy that does not open (here: a different key) is forgotten, and the server is asked.
    let (other, other_ct) = sealed(b"another file", "/uploads/p.enc", "p.png", "image/png");
    kept.put("http://s/uploads/q.enc", &other.k, &other.n, &ct).unwrap(); // the wrong ciphertext under q
    assert_eq!(open_kept("http://s/uploads/q.enc", &other.k, &other.n, Some(&kept), |_, _| Ok(other_ct.clone())).unwrap(), b"another file");

    // Save: once from the server, then again with the server's copy gone.
    let body = b"long ride notes ".repeat(64);
    let (doc, doc_ct) = sealed(&body, "/uploads/d.enc", "notes.txt", "text/plain");
    let out = dir.join("downloads");
    let write = |f: &mut std::fs::File, b: &[u8]| -> Result<u64, String> {
        use std::io::Write;
        f.write_all(b).map(|_| b.len() as u64).map_err(|e| e.to_string())
    };
    let saved = save_file("http://s/uploads/d.enc", &doc.k, &doc.n, "notes.txt", &out, Some(&kept), |_, _, f| write(f, &doc_ct)).unwrap();
    assert_eq!(std::fs::read(&saved).unwrap(), body);
    let again = save_file("http://s/uploads/d.enc", &doc.k, &doc.n, "notes.txt", &out, Some(&kept), |_, _, _| Err("404: removed".to_string()))
        .expect("saved again from the kept copy");
    assert_eq!(std::fs::read(&again).unwrap(), body);
    let left: Vec<String> = std::fs::read_dir(&out).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
    assert_eq!(left.len(), 2, "the two saved files and no part files: {left:?}");
}
