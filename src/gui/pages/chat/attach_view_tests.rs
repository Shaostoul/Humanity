// Child of chat/attach_view.rs (#[path]): a private file received in a DM or a P2P group, with
// the network replaced by a function that hands back the ciphertext, and frames drawn headless
// (10k of docs/design/blocking-and-safe-mode.md, 2026-10-10). Nothing here reaches a server: the
// one address the app could try to fetch is a closed loopback port. Each test was seen failing
// once on purpose, recorded at the test.

use super::*;
use crate::net::dm_pq::encrypt_attachment;
use crate::net::dm_store::{DmStore, SentPass};

/// The server every test message came through: a loopback port nothing listens on.
const SERVER: &str = "http://127.0.0.1:9";

/// A small real PNG (a gradient), the kind of picture the paste path sends.
fn png(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbaImage::from_fn(w, h, |x, y| image::Rgba([(x * 255 / w) as u8, (y * 255 / h) as u8, 90, 255]));
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .expect("encode a PNG");
    out
}

/// `bytes` encrypted the way the send path does it, with its marker's fields and the ciphertext.
fn sealed(bytes: &[u8], url: &str, name: &str, mime: &str) -> (DmAttachment, Vec<u8>) {
    let (ct, k, n) = encrypt_attachment(bytes).expect("encrypt");
    (DmAttachment { url: url.into(), k, n, name: name.into(), mime: mime.into(), size: bytes.len() as u64 }, ct)
}

/// A message in `channel` from `from` carrying `att`'s marker.
fn carrying(channel: &str, from: &str, name: &str, att: &DmAttachment) -> ChatMessage {
    ChatMessage {
        sender_name: name.into(),
        sender_key: from.into(),
        content: crate::net::dm_pq::build_file_marker(att),
        channel: channel.into(),
        server: SERVER.into(),
        ..Default::default()
    }
}

/// Signed in as "me" with a DM store in which "ann" is a friend (a mutual follow holding a pass
/// from us), "ivy" a one-way follow, and everyone else a stranger.
fn app(tag: &str) -> GuiState {
    let mut gs = GuiState::default();
    gs.profile_public_key = "me".into();
    gs.server_url = SERVER.into();
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let mut store = DmStore::load(&[9u8; 32], "me", &format!("wss://{tag}-{nanos}.example"));
    store.set_following("ann", true);
    store.set_follower("ann", true);
    store.record_pass_sent("ann", SentPass { serial: "a1".repeat(16), may: "invite,message,trade,voice_message".into() });
    store.set_following("ivy", true);
    gs.dm_store = Some(store);
    gs
}

fn tidy(gs: &GuiState) {
    gs.dm_store.as_ref().unwrap().remove_file_for_test();
}

/// Poll `cache` until the job for `key` has finished (a few seconds at most).
fn settle(cache: &mut ImageCache, ctx: &egui::Context, key: &str) -> ImageStatus {
    for _ in 0..500 {
        cache.poll(ctx);
        match cache.status(key) {
            ImageStatus::Fetching => std::thread::sleep(std::time::Duration::from_millis(10)),
            done => return done,
        }
    }
    panic!("the worker for {key} never finished");
}

/// What one headless frame of `paint` drew: every piece of text, and the texture of every mesh.
fn painted(ctx: &egui::Context, mut paint: impl FnMut(&mut egui::Ui)) -> (Vec<String>, Vec<egui::TextureId>) {
    let out = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| paint(ui));
    });
    fn walk(shape: &egui::Shape, texts: &mut Vec<String>, meshes: &mut Vec<egui::TextureId>) {
        match shape {
            egui::Shape::Text(t) => texts.push(t.galley.job.text.clone()),
            egui::Shape::Mesh(m) => meshes.push(m.texture_id),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, texts, meshes)),
            _ => {}
        }
    }
    let (mut texts, mut meshes) = (Vec::new(), Vec::new());
    for clipped in &out.shapes {
        walk(&clipped.shape, &mut texts, &mut meshes);
    }
    (texts, meshes)
}

/// One frame of `draw` for `att` as `plan` says, with a fresh viewer slot.
fn draw_frame(ctx: &egui::Context, cache: &mut ImageCache, sender: &str, att: &DmAttachment, plan: &Plan) -> (Vec<String>, Vec<egui::TextureId>) {
    let theme = crate::gui::theme::load_theme();
    let mut viewer = None;
    painted(ctx, |ui| draw(ui, &theme, cache, &mut viewer, sender, att, plan, theme.bg_primary()))
}

/// The proof list's "a marker in a group message decrypts and shows inline": the marker is
/// recognised in a P2P group message, its ciphertext opens to exactly the original picture, the
/// image cache decodes it at its own size, and the frame draws that texture under the message.
/// Seen red 2026-10-10 with `open` returning the ciphertext as fetched (no decryption): the
/// check that the opened bytes are the picture failed (they were the ciphertext).
#[test]
fn a_marker_in_a_group_message_decrypts_and_shows_inline() {
    let picture = png(64, 40);
    let (att, ct) = sealed(&picture, "/uploads/aa.enc", "trailhead.png", "image/png");
    let msg = carrying("p2pgroup:g1", "me", "Me", &att);
    let found = file_in(&msg).expect("the marker is recognised in a group message");
    assert_eq!((found.name.as_str(), found.mime.as_str()), ("trailhead.png", "image/png"));
    assert!(is_inline(&found), "a PNG is drawn inline");

    let served = ct.clone();
    let opened = open("http://127.0.0.1:9/uploads/aa.enc", &found.k, &found.n, |url, cap| {
        assert_eq!(url, "http://127.0.0.1:9/uploads/aa.enc");
        assert!(cap >= served.len() && cap <= ATTACH_MAX_BYTES as usize + 16, "the fetch is capped");
        Ok(served.clone())
    });
    assert_eq!(opened.expect("the ciphertext opens"), picture, "the opened bytes are the picture");

    let ctx = egui::Context::default();
    let mut cache = ImageCache::new();
    let key = cache_key(&found);
    let (k, n) = (found.k.clone(), found.n.clone());
    cache.request_bytes(&key, move || open("unused", &k, &n, |_, _| Ok(ct)));
    match settle(&mut cache, &ctx, &key) {
        ImageStatus::Ready { width, height } => assert_eq!((width, height), (64, 40), "decoded at its own size"),
        other => panic!("the picture did not load: {other:?}"),
    }
    let gs = app("view-inline");
    let plan = plan(&gs, &msg, &found);
    assert_eq!(plan.show, Show::Load, "our own picture loads by itself");
    let texture = cache.get_texture(&key).expect("a texture").id();
    let (texts, meshes) = draw_frame(&ctx, &mut cache, "Me", &found, &plan);
    assert!(meshes.contains(&texture), "the decrypted picture is drawn inline");
    assert!(texts.iter().any(|t| t == "trailhead.png"), "with its name under it: {texts:?}");
    tidy(&gs);
}

/// The proof list's "a tampered key or ciphertext gives the failure line": a flipped byte of
/// ciphertext, another file's key and a broken nonce each fail to open, and the row then says
/// "This file could not be opened." and nothing else: not the error, not the ciphertext.
/// Seen red 2026-10-10 with the Failed arm drawing `format!("Image failed: {err}")`, as plain
/// pictures do: "the failure line" failed.
#[test]
fn a_tampered_key_or_ciphertext_gives_the_failure_line() {
    let picture = png(16, 16);
    let (att, ct) = sealed(&picture, "/uploads/bb.enc", "photo.png", "image/png");
    let (other, _) = sealed(b"another file", "/uploads/cc.enc", "x.png", "image/png");
    let mut flipped = ct.clone();
    flipped[5] ^= 0x01;
    let fetch = |bytes: Vec<u8>| move |_: &str, _: usize| Ok::<_, String>(bytes);
    assert!(open("u", &att.k, &att.n, fetch(flipped.clone())).is_err(), "a flipped ciphertext byte");
    assert!(open("u", &other.k, &att.n, fetch(ct.clone())).is_err(), "another file's key");
    assert!(open("u", &att.k, "not-a-nonce", fetch(ct.clone())).is_err(), "a broken nonce");
    assert!(open("u", &att.k, &att.n, |_, _| Err("HTTP 404".to_string())).is_err(), "a failed download");

    let ctx = egui::Context::default();
    let mut cache = ImageCache::new();
    let key = cache_key(&att);
    let (k, n) = (att.k.clone(), att.n.clone());
    cache.request_bytes(&key, move || open("u", &k, &n, |_, _| Ok(flipped)));
    assert!(matches!(settle(&mut cache, &ctx, &key), ImageStatus::Failed(_)), "the cache records the failure");
    let plan = Plan { key: key.clone(), url: "http://127.0.0.1:9/uploads/bb.enc".into(), show: Show::Load };
    let (texts, meshes) = draw_frame(&ctx, &mut cache, "Ann", &att, &plan);
    assert_eq!(texts, vec![CANNOT_OPEN.to_string()], "the failure line, and only it");
    assert!(meshes.iter().all(|t| *t == egui::TextureId::default()), "no picture is drawn");

    // A file's Save that cannot open says the same line and writes nothing.
    let dir = std::env::temp_dir().join(format!("hos-attach-tamper-{}", std::process::id()));
    let (k, n, d) = (att.k.clone(), att.n.clone(), dir.clone());
    let mut tampered_ct = ct.clone();
    tampered_ct[0] ^= 0x80;
    cache.save_with("save-key", move || save_file("u", &k, &n, "photo.png", &d, |_, _| Ok(tampered_ct)));
    for _ in 0..500 {
        cache.poll(&ctx);
        if !cache.is_saving("save-key") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(cache.save_failed("save-key") && cache.downloaded_path("save-key").is_none(), "the save failed");
    assert!(!dir.join("photo.png").exists(), "nothing was written");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The proof list's "the click-to-load and protected-setup rules apply" to a private picture:
/// our own and a friend's load by themselves; a stranger's (a one-way follow included) wait for a
/// click and nothing is fetched until it; the click lets it load; one on another website waits
/// for the click that names it; and while the protected setup hides non-friends' pictures a
/// stranger's is left out even after a click, nothing drawn and nothing fetched, while a
/// friend's and our own still load.
/// Seen red 2026-10-10 with the `picture_friend` test left out of `plan` (every picture loading
/// by itself, the old rule for the server's own pictures): "a stranger's waits for a click"
/// failed.
#[test]
fn the_click_to_load_and_protected_setup_rules_apply_to_a_private_picture() {
    let mut gs = app("view-rules");
    let (att, _) = sealed(&png(8, 8), "/uploads/dd.enc", "map.png", "image/png");
    let show = |gs: &GuiState, channel: &str, from: &str, att: &DmAttachment| plan(gs, &carrying(channel, from, "X", att), att).show;

    assert_eq!(show(&gs, "p2pgroup:g1", "me", &att), Show::Load, "our own loads");
    assert_eq!(show(&gs, "p2pgroup:g1", "ann", &att), Show::Load, "a friend's loads");
    assert_eq!(show(&gs, "dm:ann", "ann", &att), Show::Load, "a friend's in a DM loads");
    assert_eq!(show(&gs, "p2pgroup:g1", "cy", &att), Show::AskNotFriend, "a stranger's waits for a click");
    assert_eq!(show(&gs, "dm:cy", "cy", &att), Show::AskNotFriend, "a stranger's DM too");
    assert_eq!(show(&gs, "p2pgroup:g1", "ivy", &att), Show::AskNotFriend, "a one-way follow is not a friend");

    // Waiting for the click draws the question and fetches nothing.
    let ctx = egui::Context::default();
    let stranger = carrying("p2pgroup:g1", "cy", "Cy", &att);
    let p = plan(&gs, &stranger, &att);
    let (texts, _) = draw_frame(&ctx, &mut gs.image_cache, "Cy", &att, &p);
    assert!(texts.iter().any(|t| t == "Cy is not your friend. Pictures show only when you choose."), "{texts:?}");
    assert!(texts.iter().any(|t| t == "Click to show map.png."), "{texts:?}");
    assert!(matches!(gs.image_cache.status(&p.key), ImageStatus::Idle), "nothing is fetched before the click");

    // The click lets it load.
    gs.image_cache.allow(&cache_key(&att));
    assert_eq!(show(&gs, "p2pgroup:g1", "cy", &att), Show::Load, "after the click it loads");

    // Ciphertext on another website waits for the click that names it, even from a friend.
    let (away, _) = sealed(&png(8, 8), "https://tracker.example/x.enc", "x.png", "image/png");
    assert_eq!(show(&gs, "p2pgroup:g1", "ann", &away), Show::AskOtherWebsite("tracker.example".into()));

    // The protected setup's rule comes first: a stranger's is left out, even after the click.
    gs.protected.setup.on = true;
    gs.protected.setup.pictures_hidden = true;
    assert_eq!(show(&gs, "p2pgroup:g1", "cy", &att), Show::Hidden, "a stranger's is left out, clicked or not");
    assert_eq!(show(&gs, "dm:ivy", "ivy", &att), Show::Hidden, "a one-way follow's too");
    assert_eq!(show(&gs, "p2pgroup:g1", "ann", &att), Show::Load, "a friend's still loads");
    assert_eq!(show(&gs, "p2pgroup:g1", "me", &att), Show::Load, "our own still loads");
    let (fresh, _) = sealed(&png(8, 8), "/uploads/ee.enc", "y.png", "image/png");
    let hidden = plan(&gs, &carrying("p2pgroup:g1", "cy", "Cy", &fresh), &fresh);
    let (texts, meshes) = draw_frame(&ctx, &mut gs.image_cache, "Cy", &fresh, &hidden);
    assert!(texts.is_empty() && meshes.iter().all(|t| *t == egui::TextureId::default()), "nothing drawn: {texts:?}");
    assert!(matches!(gs.image_cache.status(&hidden.key), ImageStatus::Idle), "nothing fetched");
    tidy(&gs);
}

/// A file that is not a picture is a card: its name, its size and Save, fetched and decrypted
/// only on Save, written into the folder under a safe name, never over another file.
/// Seen red 2026-10-10 with `save_file` writing the ciphertext as fetched (no `open`): "Save
/// writes the decrypted file" failed.
#[test]
fn a_file_is_a_card_whose_save_writes_the_decrypted_file() {
    let body = b"%PDF-1.4 route notes".repeat(20);
    let (att, ct) = sealed(&body, "/uploads/ff.enc", "route-notes.pdf", "application/pdf");
    assert!(!is_inline(&att), "a PDF is a card");
    let ctx = egui::Context::default();
    let mut cache = ImageCache::new();
    let plan = Plan { key: cache_key(&att), url: "http://127.0.0.1:9/uploads/ff.enc".into(), show: Show::AskNotFriend };
    let (texts, _) = draw_frame(&ctx, &mut cache, "Cy", &att, &plan);
    for want in ["route-notes.pdf", "400 bytes", "Save"] {
        assert!(texts.iter().any(|t| t == want), "the card shows {want}: {texts:?}");
    }
    assert!(matches!(cache.status(&plan.key), ImageStatus::Idle), "nothing is fetched before Save");

    let dir = std::env::temp_dir().join(format!("hos-attach-save-{}-{}", std::process::id(), line!()));
    for round in 0..2 {
        let (k, n, d, c) = (att.k.clone(), att.n.clone(), dir.clone(), ct.clone());
        let key = format!("save-{round}");
        cache.save_with(&key, move || save_file("u", &k, &n, "route-notes.pdf", &d, |_, _| Ok(c)));
        for _ in 0..500 {
            cache.poll(&ctx);
            if !cache.is_saving(&key) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let path = cache.downloaded_path(&key).expect("saved").to_path_buf();
        let want = if round == 0 { "route-notes.pdf" } else { "route-notes (2).pdf" };
        assert_eq!(path.file_name().unwrap().to_str().unwrap(), want, "never over another file");
        assert_eq!(std::fs::read(&path).unwrap(), body, "Save writes the decrypted file");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The name a sender chose cannot leave the downloads folder or make a name Windows refuses.
/// Seen red 2026-10-10 with `save_name` returning the name as given: the first case failed.
#[test]
fn a_senders_file_name_is_made_safe_to_save() {
    assert_eq!(save_name("..\\..\\Windows\\evil.exe"), "evil.exe");
    assert_eq!(save_name("../../etc/passwd"), "passwd");
    assert_eq!(save_name("a:b*c?.txt"), "a_b_c_.txt");
    assert_eq!(save_name("  notes.txt. "), "notes.txt");
    assert_eq!(save_name("CON.txt"), "attachment");
    assert_eq!(save_name("com1"), "attachment");
    assert_eq!(save_name(".."), "attachment");
    assert_eq!(save_name(""), "attachment");
    assert_eq!(save_name("photo.png"), "photo.png");
    assert_eq!(size_words(400), "400 bytes");
    assert_eq!(size_words(48 * 1024), "48 KB");
    assert_eq!(size_words(3 * 1024 * 1024 / 2), "1.5 MB");
}
