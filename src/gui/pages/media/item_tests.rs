//! Headless tests of an item's page and its Edit window (Media round 5, lane 1D;
//! docs/design/media-round-5-plan.md 3.9): the Media page drawn into an egui context with no window,
//! clicked the way a person clicks it (ui-system.md's headless pattern), and what it drew and sent
//! read back. Nothing here reaches past this machine: requests go to a closed port, or to a small
//! server on the loopback address that answers by route. Each test was seen to fail once against
//! the break its comment names, with the words it failed with.

use super::*;
use crate::gui::pages::media::api::{Category, Licences, ListAnswer};
use crate::gui::pages::media::api_items::{first_line, item_link, AttachTo, FileRole, CARD_LINE_MAX};
use crate::gui::pages::media::resumable::tests::{Fake, TOKEN};
use crate::gui::pages::media::tests::{ctx_and_theme, settled, shows, SERVER};
use crate::gui::pages::media::{edit, sync_server, uploading};
use std::io::{Read as _, Write as _};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The shared fixture's licences: the five, the game tick on CC0 and CC BY, the two warnings and
/// the hosting paragraph, as the relay sends them (scripts/tests/fixtures/media-round5-v1.json).
pub(crate) fn fixture_licences() -> Licences {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/tests/fixtures/media-round5-v1.json");
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).expect("the shared fixture")).expect("JSON");
    Licences::from_json(Some(&v["licences"]))
}

/// A 3D model with a description of two lines, an added file, counts and a Creative Commons
/// licence, put there by `by` (key, name).
pub(crate) fn rich_item(by: (&str, &str)) -> Item {
    let own = ItemFile { file: 0, filename: "7_dusk.glb".into(), original_name: "dusk.glb".into(), kind: "model".into(), size_bytes: 2 << 20, url: "/uploads/7_dusk.glb".into() };
    let added = ItemFile { file: 1, filename: "8_textures.zip".into(), original_name: "textures.zip".into(), kind: "file".into(), size_bytes: 4000, url: "/uploads/8_textures.zip".into() };
    Item {
        id: 7,
        url: "/uploads/7_dusk.glb".into(),
        stored: "7_dusk.glb".into(),
        original_name: "dusk.glb".into(),
        title: "Dusk".into(),
        description: "First line of it\nThe rest, on its own line".into(),
        kind: "model".into(),
        visibility: "public".into(),
        size_bytes: 2 << 20,
        total_bytes: (2 << 20) + 4000,
        uploader_key: by.0.into(),
        uploader_name: by.1.into(),
        category_id: 2,
        category_name: "Fire flow arts".into(),
        picks: 4,
        views: 12,
        downloads: 3,
        licence: "cc_by_4".into(),
        file_count: 2,
        files: vec![own, added],
        ..Default::default()
    }
}

/// The list's answer holding `items`, with two categories (the second with a description) and the
/// fixture's licences.
pub(crate) fn answer_of(items: Vec<Item>) -> ListAnswer {
    ListAnswer {
        items,
        members_view: true,
        media_visibility: "public".into(),
        categories: vec![
            Category { id: 1, name: "General".into(), description: String::new() },
            Category { id: 2, name: "Fire flow arts".into(), description: "Staff, poi, fans and the rest".into() },
        ],
        licences: fixture_licences(),
        ..Default::default()
    }
}

/// An app on the server at `base` with the page's first answer `answer` in, signed in as "me" with
/// the upload token `token` and an unlocked identity. Nothing a test does writes config.json.
pub(crate) fn page_on(base: &str, answer: ListAnswer, token: &str) -> GuiState {
    crate::config::keep_saves_off_disk();
    let mut state = GuiState::default();
    state.server_url = base.to_string();
    state.profile_public_key = "me".to_string();
    sync_server(&mut state);
    state.media.requested = true;
    super::super::apply_list(&mut state.media, answer, false);
    state.upload_token = Some((base.to_string(), token.to_string()));
    state.media.token_mark = blake3::hash(token.as_bytes()).to_hex()[..16].to_string();
    state.private_key_bytes = Some(vec![7u8; 32]);
    // Files go up exactly as they are here: what the cleaning does is not what these test.
    state.settings.files_in_messages.remove_details = false;
    state
}

/// Draw the Media page once with `events`.
pub(crate) fn frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, events: Vec<egui::Event>) -> egui::FullOutput {
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 1000.0))), events, ..Default::default() };
    ctx.run(input, |ctx| super::super::draw(ctx, theme, state))
}

/// Every drawn text exactly `text`, in paint order (bottom first): their rects.
pub(crate) fn every_exact(out: &egui::FullOutput, text: &str) -> Vec<egui::Rect> {
    fn walk(shape: &egui::Shape, clip: egui::Rect, text: &str, found: &mut Vec<egui::Rect>) {
        match shape {
            egui::Shape::Text(t) if t.galley.text() == text => {
                let rect = t.galley.rect.translate(t.pos.to_vec2()).intersect(clip);
                if rect.is_positive() {
                    found.push(rect);
                }
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, text, found)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    for cs in &out.shapes {
        walk(&cs.shape, cs.clip_rect, text, &mut found);
    }
    found
}

/// Press and release the pointer at `pos`; the release frame's output (where a click's effects,
/// a copy among them, are).
pub(crate) fn click_at(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, pos: egui::Pos2) -> egui::FullOutput {
    let m = egui::Modifiers::default();
    frame(ctx, theme, state, vec![egui::Event::PointerMoved(pos)]);
    frame(ctx, theme, state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
    frame(ctx, theme, state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }])
}

/// Click the topmost text that is exactly `text` (the last drawn: a window over the page, a list
/// over the window).
pub(crate) fn click_top(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, text: &str) -> egui::FullOutput {
    let out = settled(ctx, theme, state);
    let rect = *every_exact(&out, text).last().unwrap_or_else(|| panic!("{text:?} is drawn"));
    click_at(ctx, theme, state, rect.center())
}

/// Draw and read answers until `done` says so (ten seconds at most).
pub(crate) fn until(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, what: &str, done: impl Fn(&GuiState) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !done(state) {
        frame(ctx, theme, state, Vec::new());
        assert!(Instant::now() < deadline, "{what} (status: {:?}, page: {:?})", state.media.status, state.media.item.as_ref().map(|p| &p.status));
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// A server on this machine's loopback address that answers every request (up to 40) by its path:
/// the body of the first route whose prefix the path starts with, else `{"ok":true}`. Every request
/// is kept as "METHOD /path BODY". Returns its address.
pub(crate) fn serve_routes(routes: Vec<(&'static str, &'static str)>) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let keep = seen.clone();
    std::thread::spawn(move || {
        for _ in 0..40 {
            let Ok((mut stream, _)) = listener.accept() else { return };
            let mut got = Vec::new();
            let mut buf = [0u8; 8192];
            let head_end = loop {
                if let Some(i) = got.windows(4).position(|w| w == b"\r\n\r\n") {
                    break i + 4;
                }
                match stream.read(&mut buf) {
                    Ok(0) | Err(_) => break 0,
                    Ok(n) => got.extend_from_slice(&buf[..n]),
                }
            };
            if head_end == 0 {
                continue;
            }
            let head = String::from_utf8_lossy(&got[..head_end]).to_string();
            let len = head.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0))).unwrap_or(0);
            while got.len() < head_end + len {
                match stream.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => got.extend_from_slice(&buf[..n]),
                }
            }
            let first = head.lines().next().unwrap_or("").split(' ').take(2).collect::<Vec<_>>();
            let path = first.get(1).copied().unwrap_or("").to_string();
            keep.lock().unwrap().push(format!("{} {path} {}", first.first().copied().unwrap_or(""), String::from_utf8_lossy(&got[head_end..])));
            let body = routes.iter().find(|(prefix, _)| path.starts_with(prefix)).map(|(_, b)| *b).unwrap_or(r#"{"ok":true}"#);
            let _ = stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
        }
    });
    (format!("http://127.0.0.1:{port}"), seen)
}

/// A TILE OPENS THE ITEM'S PAGE WITH ITS DESCRIPTION, FILES, LICENCE AND COUNTS (3.9): a click on a
/// model's tile opens its page, which counts a view and shows the whole description (both lines),
/// "by Ann in Fire flow arts", "4 picks, 12 views, 3 downloads", the licence by name with the
/// server's own line for it, and each file with its own Download.
///
/// Seen red 2026-10-12 with the description left out of `draw_page`: "the whole description, its
/// second line too" (no shape held "The rest, on its own line").
#[test]
fn a_tile_opens_the_item_with_description_files_licence_and_counts() {
    let (ctx, theme) = ctx_and_theme();
    let mut state = page_on(SERVER, answer_of(vec![rich_item(("k2", "Ann"))]), "tok");
    super::super::tests::click(&ctx, &theme, &mut state, "3D model");
    assert_eq!(state.media.item.as_ref().map(|p| p.id), Some(7), "the click opens the item's page");
    assert!(state.media.jobs.iter().any(|(j, _)| *j == Job::Viewed { id: 7 }), "and counts a view");
    let out = settled(&ctx, &theme, &mut state);
    assert!(shows(&out, "The rest, on its own line"), "the whole description, its second line too");
    for text in ["by Ann in Fire flow arts", "4 picks, 12 views, 3 downloads", "CC BY 4.0", "Anyone may copy, share and change it", "textures.zip, file", "dusk.glb, 3d model"] {
        assert!(shows(&out, text), "the page shows {text:?}");
    }
    assert_eq!(every_exact(&out, "Download").len(), 2, "each file has its own Download");
    assert!(shows(&out, COPY_LINK) && shows(&out, "Report") && !shows(&out, "Remove item"), "a member's buttons on someone else's item");
}

/// THE REPORTS PAGE'S OPEN OPENS THE ITEM'S OWN PAGE (lane 1D's finding, 2026-10-12): Open (media.rs
/// `open_item_by_number`) opens the page a tile's click opens, never the list filtered down to the
/// item. An item the list does not hold is asked for by its number (one signed list read with
/// `id`, as the website's `openById` asks): its page opens at once, shows the item when it comes and
/// counts the view then, and the list keeps the filters the person had. An item the list holds
/// opens at once, with nothing asked.
///
/// Seen red 2026-10-12 against the old Open, which set the list's filters to `id` and opened no
/// page: "Open opens item 12's own page" (left: None, right: Some(12)).
#[test]
fn the_reports_open_opens_the_items_own_page() {
    let (ctx, theme) = ctx_and_theme();
    let one = r#"{"items":[{"id":12,"title":"Dusk at the shore","kind":"video","visibility":"public","url":"/uploads/12_dusk.webm","size_bytes":5000,"uploader_key":"k2","uploader_name":"Ann","category":{"id":2,"name":"Fire flow arts"},"picks":2,"views":5,"downloads":1}],"members_view":true}"#;
    let (base, seen) = serve_routes(vec![("/api/media/list", one)]);
    let mut state = page_on(&base, answer_of(vec![rich_item(("k2", "Ann"))]), "tok");
    // The person had picked a kind before going to the Reports page: Open must not throw it away.
    state.media.filters.kind = Some("model".into());
    let kept = state.media.filters.clone();

    crate::gui::pages::media::open_item_by_number(&mut state, 12);
    assert_eq!(state.media.item.as_ref().map(|p| p.id), Some(12), "Open opens item 12's own page");
    assert_eq!(state.active_page, crate::gui::GuiPage::Media, "on the Media page");
    assert_eq!(state.media.filters, kept, "the list keeps the filters the person had");
    assert!(state.media.jobs.iter().any(|(j, _)| *j == Job::Item { id: 12 }), "the list does not hold it: it is asked for by its number");
    // Nothing on the page yet, so its window says "Loading..." (item_view.rs `draw`) until it comes.
    assert!(state.media.item.as_ref().is_some_and(|p| p.item.is_none() && p.status.is_empty()), "the page waits for the item");

    until(&ctx, &theme, &mut state, "item 12 arrives on its page", |s| s.media.item.as_ref().and_then(|p| p.item.as_ref()).is_some());
    let out = settled(&ctx, &theme, &mut state);
    for text in ["Dusk at the shore", "by Ann in Fire flow arts", "2 picks, 5 views, 1 download"] {
        assert!(shows(&out, text), "the item's page shows {text:?}");
    }
    // Asked once, signed, by its number; and the opening counted as a view, as a tile's click is.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let asked = seen.lock().unwrap().clone();
        let reads: Vec<&String> = asked.iter().filter(|r| r.starts_with("POST /api/media/list ")).collect();
        if asked.iter().any(|r| r.starts_with("POST /api/media/viewed")) {
            assert_eq!(reads.len(), 1, "one read: {asked:?}");
            assert!(reads[0].contains(r#""id":12"#) && reads[0].contains(r#""sig":"#), "by its number, signed: {}", reads[0]);
            break;
        }
        assert!(Instant::now() < deadline, "the view was counted: {asked:?}");
        frame(&ctx, &theme, &mut state, Vec::new());
        std::thread::sleep(Duration::from_millis(5));
    }

    // An item the list holds opens at once, as it is: nothing asked for it.
    state.media.item = None;
    crate::gui::pages::media::open_item_by_number(&mut state, 7);
    assert_eq!(state.media.item.as_ref().and_then(|p| p.item.as_ref()).map(|i| i.title.as_str()), Some("Dusk"), "item 7 opens at once");
    assert!(!state.media.jobs.iter().any(|(j, _)| *j == Job::Item { id: 7 }), "nothing is asked for an item the list holds");
}

/// THE CARD SHOWS THE DESCRIPTION'S FIRST LINE AND THE FILE COUNT (3.9): at most 120 characters,
/// cut at a word with "...", never the second line (the page has that), "3 files" when it holds
/// several, the picks and downloads, and "Featured in its category" for that shelf.
///
/// Seen red 2026-10-12 with the whole description put on the card: "the second line stays on the
/// page" (the card drew "The second line stays on the page").
#[test]
fn the_card_shows_the_first_line_and_the_file_count() {
    let long = "A slow contact staff spin at dusk on the beach, with the tide coming in and the last of the light on the water behind the dancer";
    assert!(long.chars().count() > CARD_LINE_MAX);
    let cut = first_line(&format!("{long}\nThe second line stays on the page"));
    assert!(cut.chars().count() <= CARD_LINE_MAX && cut.ends_with("..."), "{cut}");
    let kept = cut.trim_end_matches("...");
    assert!(long.starts_with(kept) && long[kept.len()..].starts_with(' '), "cut at a word: {cut:?}");
    assert_eq!(first_line("  Short one\nmore"), "Short one");

    let (ctx, theme) = ctx_and_theme();
    let item = Item {
        description: format!("{long}\nThe second line stays on the page"),
        file_count: 3,
        picks: 3,
        downloads: 2,
        featured: api::Featured::Category,
        ..rich_item(("k2", "Ann"))
    };
    let mut state = page_on(SERVER, answer_of(vec![item]), "tok");
    let out = settled(&ctx, &theme, &mut state);
    assert!(!shows(&out, "The second line stays on the page"), "the second line stays on the page");
    assert!(shows(&out, &cut), "the first line, cut");
    for text in ["3 files", "3 picks, 2 downloads", crate::gui::pages::media::filters::feature_words(api::Featured::Category)] {
        assert!(shows(&out, text), "the card shows {text:?}");
    }
}

/// A CREATIVE COMMONS LICENCE SHOWS ITS WARNING BEFORE SAVE (section 10: the editor states the facts
/// plainly): on the uploader's own item, picking CC BY 4.0 in the Edit window shows that it cannot
/// be taken back for copies already taken, above Save; "All rights reserved" shows no warning; on a
/// members-only item the second fact shows too; and the game tick is there with CC BY, gone with
/// CC BY-NC.
///
/// Seen red 2026-10-12 with the warning skipped in `draw_licence_choice`: "the warning shows once
/// CC BY 4.0 is picked" (no shape held "A Creative Commons licence cannot be taken back").
#[test]
fn a_creative_commons_licence_shows_its_warning_before_save() {
    let (ctx, theme) = ctx_and_theme();
    let licences = fixture_licences();
    let item = Item { licence: "all_rights_reserved".into(), ..rich_item(("me", "Me")) };
    let mut state = page_on(SERVER, answer_of(vec![item.clone()]), "tok");
    edit::open_edit(&mut state, &item);
    let out = settled(&ctx, &theme, &mut state);
    assert!(!shows(&out, &licences.cc_warning), "All rights reserved: no warning");
    assert!(shows(&out, "Your uploads stay yours.") && shows(&out, "When you put a file on this server"), "the hosting paragraph, in the server's words");

    click_top(&ctx, &theme, &mut state, "All rights reserved");
    click_top(&ctx, &theme, &mut state, "CC BY 4.0");
    let form = state.media.item.as_ref().and_then(|p| p.edit.as_ref()).expect("the Edit window");
    assert_eq!(form.choice.licence, "cc_by_4", "CC BY 4.0 is picked");
    let out = settled(&ctx, &theme, &mut state);
    let warning = find_text(&out, &licences.cc_warning).expect("the warning shows once CC BY 4.0 is picked");
    let save = *every_exact(&out, edit::SAVE).last().expect("Save");
    assert!(warning.bottom() <= save.top(), "it shows before Save: {warning:?} above {save:?}");
    assert!(!shows(&out, &licences.cc_members_warning), "a public item: not the members' fact");
    assert!(shows(&out, &licences.game_label), "CC BY may be offered for the game");

    click_top(&ctx, &theme, &mut state, "Members only");
    let out = settled(&ctx, &theme, &mut state);
    assert!(shows(&out, &licences.cc_members_warning), "members only: every member who receives it may republish it");

    click_top(&ctx, &theme, &mut state, "CC BY 4.0");
    click_top(&ctx, &theme, &mut state, "CC BY-NC 4.0");
    let out = settled(&ctx, &theme, &mut state);
    assert!(!shows(&out, &licences.game_label), "NonCommercial: not for the game");
}

/// The first drawn text holding `text`.
pub(crate) fn find_text(out: &egui::FullOutput, text: &str) -> Option<egui::Rect> {
    crate::gui::screen_surface::find_text_in_shapes(&out.shapes, text).map(|f| f.rect)
}

/// AN ADMIN SEES THE LICENCE BUT CANNOT CHANGE IT (decision 1): an admin editing Ann's item sees
/// its licence and the game tick, read only, with "Only Ann can change these."; a press on the
/// licence opens no list, the tick does not move, and the edit sends the licence and tick as they
/// were whatever the form holds. Moderators get no Edit at all.
///
/// Seen red 2026-10-12 with the read-only rule dropped (`owner_only` passed as None): "an admin is
/// told who can change them" (no shape held "Only Ann can change these.").
#[test]
fn an_admin_sees_the_licence_but_cannot_change_it() {
    let (ctx, theme) = ctx_and_theme();
    let item = Item { project_use: true, ..rich_item(("k2", "Ann")) };
    let mut state = page_on(SERVER, answer_of(vec![item.clone()]), "tok");
    state.chat_users.push(crate::gui::ChatUser { name: "Mod".into(), public_key: "me".into(), role: "mod".into(), status: String::new() });
    assert!(!may_edit(&state, &item), "a moderator edits nothing");
    state.chat_users[0].role = "admin".into();
    assert!(may_edit(&state, &item), "an admin may edit the words");
    edit::open_edit(&mut state, &item);
    let out = settled(&ctx, &theme, &mut state);
    assert!(shows(&out, "Only Ann can change these."), "an admin is told who can change them");
    click_top(&ctx, &theme, &mut state, "CC BY 4.0");
    let out = settled(&ctx, &theme, &mut state);
    assert!(!shows(&out, "CC0 1.0"), "a press on the licence opens no list");
    click_top(&ctx, &theme, &mut state, &fixture_licences().game_label);
    let form = state.media.item.as_ref().and_then(|p| p.edit.clone()).expect("the Edit window");
    assert!(form.choice.project_use, "the tick does not move");
    let changed = edit::EditForm { choice: edit::LicenceChoice { licence: "cc0_1".into(), project_use: false }, ..form };
    let sent = changed.fields(false);
    assert_eq!((sent.licence.as_str(), sent.project_use), ("cc_by_4", true), "the edit sends them as they were");
}

/// ADD A FILE UPLOADS, THEN ATTACHES (3.9, `Of::Attach`): Add a file in the Edit window opens the
/// picker; the picked file is the same upload in pieces, a row under "Uploads in progress" naming
/// the item it goes to, and once it is up it is added to the item (`POST /api/media/files/add`,
/// signed over `media_file_add:<id>:<upload>:file`), never published as an item of its own. The
/// answer's item, now with two files, is the page's.
///
/// Seen red 2026-10-12 with `publish_entry` publishing every upload (the attach left out): "added,
/// never published: [\"POST /api/media/viewed\", \"POST /api/media/mine\", \"POST /api/media\",
/// \"POST /api/media/mine\"]" (a publish, and no files/add).
#[test]
fn add_a_file_uploads_then_attaches() {
    let (ctx, theme) = ctx_and_theme();
    let answer = r#"{"ok":true,"item":{"id":7,"title":"Dusk","url":"/uploads/7_dusk.glb","filename":"7_dusk.glb","kind":"model","visibility":"public","uploader_key":"me","file_count":2,
        "files":[{"file":0,"filename":"7_dusk.glb","original_name":"dusk.glb","kind":"model","size_bytes":2097152,"url":"/uploads/7_dusk.glb"},
                 {"file":5,"filename":"1700_clip.webm","original_name":"clip.webm","kind":"video","size_bytes":20000,"url":"/uploads/1700_clip.webm"}]}}"#;
    let (base, seen) = serve_routes(vec![("/api/media/files/add", answer)]);
    let mine = Item { files: vec![rich_item(("me", "Me")).files[0].clone()], file_count: 1, ..rich_item(("me", "Me")) };
    let mut state = page_on(&base, answer_of(vec![mine.clone()]), TOKEN);
    edit::open_edit(&mut state, &mine);
    click_top(&ctx, &theme, &mut state, edit::ADD_FILE);
    let form = state.media.item.as_ref().and_then(|p| p.edit.as_ref()).expect("the Edit window");
    assert!(matches!(form.picker, Some((FileRole::File, _))), "Add a file opens the picker");

    let file = crate::test_temp::file("media_attach", "webm");
    std::fs::write(&file, (0..20_000u32).map(|i| (i % 251) as u8).collect::<Vec<u8>>()).unwrap();
    let to = AttachTo { id: 7, filename: "7_dusk.glb".into(), role: FileRole::File };
    uploading::start_attach_with(&mut state, &file, "clip.webm", 20_000, to, Fake::new(8192)).expect("it starts");
    // Read at once: the simulated server takes 20 KB faster than a frame, so the row is soon gone.
    assert_eq!(uploading::row_heading(&state, &state.media.uploads[0].record), "clip.webm (0 MB), to add to Dusk", "its row names the item it goes to");
    until(&ctx, &theme, &mut state, "the upload is up and added", |s| s.media.uploads.is_empty() && s.media.jobs.iter().all(|(j, _)| !matches!(j, Job::Publish { .. })));

    let seen = seen.lock().unwrap().clone();
    // What was asked, without the bodies (a signature is 6 KB of hex).
    let asked: Vec<String> = seen.iter().map(|r| r.split(' ').take(2).collect::<Vec<_>>().join(" ")).collect();
    let add = seen.iter().find(|r| r.starts_with("POST /api/media/files/add ")).unwrap_or_else(|| panic!("added, never published: {asked:?}"));
    assert!(add.contains(r#""add":"1700_clip.webm""#) && add.contains(r#""as":"file""#) && add.contains(r#""filename":"7_dusk.glb""#), "{add}");
    assert!(!seen.iter().any(|r| r.starts_with("POST /api/media ")), "added, never published: {asked:?}");
    let page = state.media.item.as_ref().and_then(|p| p.item.as_ref()).expect("the page's item");
    assert_eq!(page.files.iter().map(|f| f.filename.as_str()).collect::<Vec<_>>(), vec!["7_dusk.glb", "1700_clip.webm"], "the answer's item is the page's");
    assert_eq!(state.media.status, "Added to \"Dusk\".");
}

/// COPY LINK IS THE ITEM'S PAGE ADDRESS (3.9): `https://<host>/media/<id>`, the page the relay
/// serves with its preview for social sites, never `/media?id=`; it is put on the clipboard and
/// shown under the button.
///
/// Seen red 2026-10-12 with `item_link` making the old address: "the item's own page" (left:
/// "https://united-humanity.us/media?id=12").
#[test]
fn copy_link_is_the_items_page_address() {
    assert_eq!(item_link("https://united-humanity.us", 12), "https://united-humanity.us/media/12", "the item's own page");
    let mut wss = GuiState::default();
    wss.server_url = "wss://united-humanity.us/ws".into();
    assert_eq!(item_link(&super::super::base_of(&wss), 12), "https://united-humanity.us/media/12", "from a server known by its socket");

    let (ctx, theme) = ctx_and_theme();
    let item = rich_item(("k2", "Ann"));
    let mut state = page_on(SERVER, answer_of(vec![item.clone()]), "tok");
    open(&mut state, &item);
    let out = click_top(&ctx, &theme, &mut state, COPY_LINK);
    let want = format!("{SERVER}/media/7");
    assert!(out.platform_output.commands.contains(&egui::OutputCommand::CopyText(want.clone())), "copied: {:?}", out.platform_output.commands);
    assert!(shows(&settled(&ctx, &theme, &mut state), &format!("Copied: {want}")), "and shown");
}

/// DOWNLOAD GOES THROUGH THE COUNTING ROUTE (3.5): the added file's Download asks the relay's
/// `/api/media/download/<id>?file=<n>`, never its `/uploads/` address, so the download is counted;
/// a members-only item's asks for a ticket made for that route, for that file.
///
/// Seen red 2026-10-12 with `download_origin` fetching the item's own address (`api::file_url`, as
/// before round 5): "the counted route: [\"POST /api/media/viewed {\\\"id\\\":7}\", \"GET
/// /uploads/7_dusk.glb \"]".
#[test]
fn download_goes_through_the_counting_route() {
    let (ctx, theme) = ctx_and_theme();
    let (base, seen) = serve_routes(vec![("/api/media/download/", "abc"), ("/uploads/", "abc")]);
    let item = rich_item(("k2", "Ann"));
    let mut state = page_on(&base, answer_of(vec![item.clone()]), "tok");
    open(&mut state, &item);
    let out = settled(&ctx, &theme, &mut state);
    // The Download on the added file's row.
    let row = find_text(&out, "textures.zip, file").expect("the added file's row");
    let press = every_exact(&out, "Download").into_iter().min_by(|a, b| (a.center().y - row.center().y).abs().total_cmp(&(b.center().y - row.center().y).abs())).expect("its Download");
    click_at(&ctx, &theme, &mut state, press.center());
    let deadline = Instant::now() + Duration::from_secs(10);
    while !state.media.open.as_ref().is_some_and(|o| o.saved_words(1).is_some()) && Instant::now() < deadline {
        frame(&ctx, &theme, &mut state, Vec::new());
        std::thread::sleep(Duration::from_millis(5));
    }
    let requests = seen.lock().unwrap().clone();
    assert!(requests.iter().any(|r| r.starts_with("GET /api/media/download/7?file=1 ")), "the counted route: {requests:?}");
    assert!(!requests.iter().any(|r| r.starts_with("GET /uploads/")), "never the file's own address: {requests:?}");
    let words = state.media.open.as_ref().and_then(|o| o.saved_words(1)).expect("the download ends").to_string();
    if let Some(path) = words.strip_prefix("Saved to ") {
        assert_eq!(std::fs::read(path).unwrap(), b"abc");
        let _ = std::fs::remove_file(path);
    } else {
        panic!("saved: {words}");
    }

    let members = Item { visibility: "members".into(), ..item };
    assert!(
        matches!(open::download_origin(&base, &members, 1, Some("tok")), Ok(open::Origin::Ticket { file: 1, download: true, .. })),
        "a members-only file: a ticket for the counted route, for that file"
    );
}

/// EDIT OPENS IN FRONT OF THE ITEM'S PAGE, EVERY TIME (operator, 2026-10-15: "the initial detailed
/// media window ... stays layered on top of it preventing me from being able to interact with it").
/// Every dialog asked for the top on every frame, so egui kept their earlier order, and the Edit
/// window, once opened and closed, kept a place under the page: opened again, it stayed behind.
/// Opened, cancelled and opened again from the page, the Edit window must be the layer on top where
/// the two windows overlap.
///
/// Seen red 2026-10-15 with the dialog's every-frame raise put back (widgets/dialog.rs): "the Edit
/// window opened again is in front of the item's page: where they overlap" found the page's window.
#[test]
fn edit_opens_in_front_of_the_items_page_every_time() {
    let (ctx, theme) = ctx_and_theme();
    let item = rich_item(("me", "Me"));
    let mut state = page_on(SERVER, answer_of(vec![item.clone()]), "tok");
    crate::gui::pages::media::item_view::open(&mut state, &item);
    let edit_layer = egui::LayerId::new(egui::Order::Middle, egui::Id::new(edit::EDIT_DIALOG_ID));
    let edit_open = |state: &GuiState| state.media.item.as_ref().and_then(|p| p.edit.as_ref()).is_some();
    for (time, words) in ["the Edit window is in front of the item's page", "the Edit window opened again is in front of the item's page"].iter().enumerate() {
        click_top(&ctx, &theme, &mut state, "Edit");
        assert!(edit_open(&state), "Edit opened its window (time {time})");
        let out = settled(&ctx, &theme, &mut state);
        let save = *every_exact(&out, edit::SAVE).last().expect("Save");
        assert_eq!(ctx.layer_id_at(save.center()), Some(edit_layer), "{words}: at its Save");
        // Where the two windows overlap (Save can sit below a short page, where only Edit is).
        let rect_of = |id: &str| ctx.memory(|m| m.area_rect(egui::Id::new(id))).expect("drawn");
        let both = rect_of("media_item_page").intersect(rect_of(edit::EDIT_DIALOG_ID));
        assert!(both.is_positive(), "the two windows overlap");
        assert_eq!(ctx.layer_id_at(both.center()), Some(edit_layer), "{words}: where they overlap");
        if time == 0 {
            click_top(&ctx, &theme, &mut state, "Cancel");
            assert!(!edit_open(&state), "Cancel closed the Edit window");
            assert!(state.media.item.is_some(), "the item's page is still open");
        }
    }
}
