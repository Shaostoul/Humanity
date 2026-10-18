// Child of gui/drop_files.rs (#[path]): files dragged onto the window, with the frames drawn
// headless and the files given as egui's input gives them. Each test was seen failing once on
// purpose, recorded at the test.

use super::*;

/// A frame whose input carries `dropped` (let go this frame) and `over` (being dragged now).
fn input(dropped: &[PathBuf], over: &[PathBuf]) -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))),
        dropped_files: dropped.iter().map(|p| egui::DroppedFile { path: Some(p.clone()), ..Default::default() }).collect(),
        hovered_files: over.iter().map(|p| egui::HoveredFile { path: Some(p.clone()), ..Default::default() }).collect(),
        ..Default::default()
    }
}

/// Every piece of text a frame drew.
fn texts(out: &egui::FullOutput) -> String {
    fn walk(s: &egui::Shape, t: &mut Vec<String>) {
        match s {
            egui::Shape::Text(x) => t.push(x.galley.job.text.clone()),
            egui::Shape::Vec(v) => v.iter().for_each(|y| walk(y, t)),
            _ => {}
        }
    }
    let mut t = Vec::new();
    out.shapes.iter().for_each(|c| walk(&c.shape, &mut t));
    t.join("\n")
}

/// Two files and a folder on disk, for the drops; the first value keeps the folder until the test
/// ends (it is deleted when dropped).
fn files(tag: &str) -> (crate::test_temp::TempPath, PathBuf, PathBuf, PathBuf) {
    let dir = crate::test_temp::dir(tag);
    let (a, b, folder) = (dir.join("photo.jpg"), dir.join("notes.txt"), dir.join("a folder"));
    std::fs::write(&a, b"not really a jpeg").unwrap();
    std::fs::write(&b, b"some notes").unwrap();
    std::fs::create_dir_all(&folder).unwrap();
    (dir, a, b, folder)
}

/// THE MOST SPECIFIC PLACE GETS A DROP, AND ONLY IT: a window over a page takes the files, the
/// page does not; among places of one level the last drawn wins; folders are left out and named.
/// Seen red 2026-10-17 with `winner` taking the first taker: "the Edit window, not the page, got
/// the files".
#[test]
fn the_most_specific_place_gets_a_drop_and_only_it() {
    let (_dir, a, b, folder) = files("drop_most_specific");
    let ctx = egui::Context::default();
    let theme = crate::gui::theme::load_theme();
    let mut state = GuiState::default();
    let got: Arc<Mutex<Vec<(&'static str, Vec<PathBuf>)>>> = Arc::default();
    let dropped = [a.clone(), folder.clone(), b.clone()];
    let _ = ctx.run(input(&dropped, &[]), |ctx| {
        begin_frame(ctx);
        for (level, who) in [(Level::Page, "page"), (Level::Window, "first window"), (Level::Window, "Edit window")] {
            let got = got.clone();
            register(ctx, level, who, move |_s: &mut GuiState, paths| got.lock().unwrap().push((who, paths)));
        }
        finish(ctx, &theme, &mut state);
    });
    let got = got.lock().unwrap().clone();
    assert_eq!(got.len(), 1, "one place gets them: {got:?}");
    assert_eq!(got[0].0, "Edit window", "the Edit window, not the page, got the files");
    assert_eq!(got[0].1, vec![a, b], "the files, the folder left out");
    let note = note_now(&ctx).expect("a folder left out is said");
    assert!(note.starts_with(FOLDERS_LEFT_OUT) && note.contains("a folder"), "{note}");
}

/// A DROP THAT NOTHING TAKES IS SAID, NOT LOST SILENTLY: with no taker the note says where files can
/// go; a place that refuses says why; an open picker takes the drop for itself, so no page gets it.
/// Seen red 2026-10-17 with `finish` dropping untaken files without a note: "nothing here: said".
#[test]
fn a_drop_that_nothing_takes_is_said() {
    let (_dir, a, _, _) = files("drop_nothing");
    let ctx = egui::Context::default();
    let theme = crate::gui::theme::load_theme();
    let mut state = GuiState::default();
    let _ = ctx.run(input(&[a.clone()], &[]), |ctx| {
        begin_frame(ctx);
        finish(ctx, &theme, &mut state);
    });
    assert_eq!(note_now(&ctx).as_deref(), Some(NOTHING_HERE), "nothing here: said");

    let ctx = egui::Context::default();
    let _ = ctx.run(input(&[a.clone()], &[]), |ctx| {
        begin_frame(ctx);
        refuse(ctx, Level::Page, "Files cannot be dropped here: sign in first.");
        finish(ctx, &theme, &mut state);
    });
    assert_eq!(note_now(&ctx).as_deref(), Some("Files cannot be dropped here: sign in first."), "a refusal says why");

    let ctx = egui::Context::default();
    let page_got = Arc::new(Mutex::new(false));
    let mut picked = None;
    let _ = ctx.run(input(&[a.clone()], &[]), |ctx| {
        begin_frame(ctx);
        let flag = page_got.clone();
        register(ctx, Level::Page, "page", move |_s: &mut GuiState, _p| *flag.lock().unwrap() = true);
        picked = take_for_picker(ctx);
        finish(ctx, &theme, &mut state);
    });
    assert_eq!(picked, Some(vec![a]), "the open picker takes the drop as its choice");
    assert!(!*page_got.lock().unwrap() && note_now(&ctx).is_none(), "and no page gets it");
}

/// WHILE FILES ARE DRAGGED, THE WINDOW SAYS WHERE THEY WILL GO: the winning place's words over a
/// dimmed window, each dragged file with its type; with nothing to take them, where to drop them
/// instead. Seen red 2026-10-17 with the overlay drawn only for a taker: "nothing here takes them:
/// said while dragging".
#[test]
fn while_files_are_dragged_the_window_says_where_they_will_go() {
    let (_dir, a, b, _) = files("drop_overlay");
    let theme = crate::gui::theme::load_theme();
    let mut state = GuiState::default();
    let ctx = egui::Context::default();
    // Two frames: a window egui has not drawn before shows from its second frame on.
    let mut out = None;
    for _ in 0..2 {
        out = Some(ctx.run(input(&[], &[a.clone(), b.clone()]), |ctx| {
            begin_frame(ctx);
            register(ctx, Level::App, "Drop to upload to Media", |_s: &mut GuiState, _p| {});
            register(ctx, Level::Page, "Drop to send in #general", |_s: &mut GuiState, _p| {});
            finish(ctx, &theme, &mut state);
        }));
    }
    let t = texts(out.as_ref().unwrap());
    assert!(t.contains("Drop to send in #general") && !t.contains("Drop to upload to Media"), "the page's words, not the app's default: {t}");
    assert!(t.contains("photo.jpg") && t.contains("notes.txt") && t.contains("Picture"), "{t}");
    let ctx = egui::Context::default();
    for _ in 0..2 {
        out = Some(ctx.run(input(&[], &[a.clone()]), |ctx| {
            begin_frame(ctx);
            finish(ctx, &theme, &mut state);
        }));
    }
    assert!(texts(out.as_ref().unwrap()).contains(NOTHING_HERE), "nothing here takes them: said while dragging");
}

/// A FILE DROPPED ON THE CHAT GOES TO THE OPEN CONVERSATION AS ATTACH SENDS IT: a photo dropped in a
/// DM opens its attach step there (what is hidden in it, Send); the overlay names the conversation.
/// Seen red 2026-10-17 with the chat not offering itself: "the overlay names the conversation"
/// (only the page behind was drawn).
#[test]
fn a_file_dropped_on_the_chat_goes_to_the_open_conversation() {
    let photo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/describe/jpeg-phone.jpg");
    let ctx = egui::Context::default();
    let theme = crate::gui::theme::load_theme();
    let mut state = GuiState::default();
    state.server_url = "http://127.0.0.1:9".into();
    state.profile_public_key = "me".into();
    state.chat_active_channel = "dm:ann".into();
    state.chat_dms.push(crate::gui::ChatDm { user_key: "ann".into(), user_name: "Ann".into(), last_message: String::new(), timestamp: String::new(), unread: false });
    state.upload_token = Some((crate::gui::pages::chat::norm_server_url("http://127.0.0.1:9"), "tok".into()));
    let frame = |state: &mut GuiState, raw: egui::RawInput| {
        ctx.run(raw, |ctx| {
            begin_frame(ctx);
            crate::gui::pages::media::offer_drop_anywhere(ctx, state);
            crate::gui::pages::chat::draw(ctx, &theme, state);
            crate::gui::pages::media::draw_upload_window(ctx, &theme, state);
            finish(ctx, &theme, state);
        })
    };
    let _ = frame(&mut state, input(&[], &[photo.clone()]));
    let out = frame(&mut state, input(&[], &[photo.clone()]));
    assert!(texts(&out).contains("Drop to send in Ann"), "the overlay names the conversation: {}", texts(&out));
    assert!(!texts(&out).contains("Drop to upload"), "a private conversation's file is never offered to a public upload");
    let _ = frame(&mut state, input(&[photo], &[]));
    assert!(state.files_flow.step.is_some(), "the photo's attach step opens");
}

/// A FILE DROPPED ON A CHANNEL OPENS THE UPLOAD WINDOW, POSTING IN THAT CHANNEL TICKED (operator,
/// 2026-10-17: "I typically sit in #general so I'd go through the normal upload file popup path then
/// upon clicking upload it also posts the image with the title and description to the general
/// chat"): the overlay says "Drop to upload to Media and post in #general", and the drop opens Add to
/// Media with the file and "Also post in chat" ticked for #general; nothing goes until Upload. Seen
/// red 2026-10-17 with the chat sending a channel's drop straight to its Attach: the overlay said
/// "Drop to send in #general", not the upload window's words.
#[test]
fn a_file_dropped_on_a_channel_opens_the_upload_window_posting_ticked() {
    let (_dir, a, _, _) = files("drop_channel");
    let ctx = egui::Context::default();
    let theme = crate::gui::theme::load_theme();
    let mut state = GuiState::default();
    state.server_url = "http://127.0.0.1:9".into();
    state.profile_public_key = "me".into();
    state.chat_active_channel = "general".into();
    state.chat_channels.push(crate::gui::ChatChannel { id: "general".into(), name: "general".into(), ..Default::default() });
    state.upload_token = Some((crate::gui::pages::chat::norm_server_url("http://127.0.0.1:9"), "tok".into()));
    let frame = |state: &mut GuiState, raw: egui::RawInput| {
        ctx.run(raw, |ctx| {
            begin_frame(ctx);
            crate::gui::pages::media::offer_drop_anywhere(ctx, state);
            crate::gui::pages::chat::draw(ctx, &theme, state);
            crate::gui::pages::media::draw_upload_window(ctx, &theme, state);
            finish(ctx, &theme, state);
        })
    };
    let _ = frame(&mut state, input(&[], &[a.clone()]));
    let out = frame(&mut state, input(&[], &[a.clone()]));
    assert!(texts(&out).contains("Drop to upload to Media and post in #general"), "{}", texts(&out));
    let _ = frame(&mut state, input(&[a], &[]));
    let form = state.media.form.as_ref().expect("the upload window opens");
    assert!(form.post_on && form.post_to == "general", "posting in #general ticked");
    assert!(state.files_flow.step.is_none() && state.media.uploads.is_empty(), "nothing goes until Upload");
}

/// A READ-ONLY CHANNEL DOES NOT OFFER TO POST IN ITSELF (2026-10-17, the website's rule): over
/// #announcements, for someone who cannot post there, the overlay says only "Drop to upload to
/// Media" and the upload window opens with nothing ticked. Seen red 2026-10-17 with read-only
/// channels treated like any other: the overlay offered "post in #announcements".
#[test]
fn a_read_only_channel_does_not_offer_to_post_in_itself() {
    let (_dir, a, _, _) = files("drop_read_only");
    let ctx = egui::Context::default();
    let theme = crate::gui::theme::load_theme();
    let mut state = GuiState::default();
    state.server_url = "http://127.0.0.1:9".into();
    state.profile_public_key = "me".into();
    state.chat_active_channel = "announcements".into();
    state.chat_channels.push(crate::gui::ChatChannel { id: "announcements".into(), name: "announcements".into(), read_only: true, ..Default::default() });
    state.upload_token = Some((crate::gui::pages::chat::norm_server_url("http://127.0.0.1:9"), "tok".into()));
    let frame = |state: &mut GuiState, raw: egui::RawInput| {
        ctx.run(raw, |ctx| {
            begin_frame(ctx);
            crate::gui::pages::media::offer_drop_anywhere(ctx, state);
            crate::gui::pages::chat::draw(ctx, &theme, state);
            crate::gui::pages::media::draw_upload_window(ctx, &theme, state);
            finish(ctx, &theme, state);
        })
    };
    let _ = frame(&mut state, input(&[], &[a.clone()]));
    let out = frame(&mut state, input(&[], &[a.clone()]));
    let t = texts(&out);
    assert!(t.contains(crate::gui::pages::media::DROP_TO_UPLOAD) && !t.contains("post in #announcements"), "{t}");
    let _ = frame(&mut state, input(&[a], &[]));
    let form = state.media.form.as_ref().expect("the upload window opens");
    assert!(!form.post_on, "nothing ticked");
}
