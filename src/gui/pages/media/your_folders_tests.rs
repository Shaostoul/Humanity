// Child of media/your_folders.rs (#[path]): headless tests of the Media page's Your folders tab,
// drawn through the whole page (media.rs `draw`) over real folders written for each test, with no
// window and nothing sent anywhere except, in the marks test, to a server on this machine's
// loopback address. Each was seen failing once on purpose; what it saw red is at the test.

use super::*;
use crate::gui::pages::media::tests::{click, ctx_and_theme, demo_answer, page_state, settled, shows};
use crate::gui::pages::media::uploads_tests::http_server;
use crate::gui::pages::media::{add, poll, sync_server, uploading};
use crate::gui::screen_surface::find_text_in_shapes;
use std::time::Duration;

/// A signed-in member on the test server, on the Your folders tab.
fn on_the_tab() -> GuiState {
    let mut state = page_state(demo_answer(), true);
    state.media.tab = MediaTab::YourFolders;
    state
}

/// A folder holding a sub-folder (with one of its own), a picture, a video, a text file and a model.
fn folder(tag: &str) -> crate::test_temp::TempPath {
    let dir = crate::test_temp::dir(tag);
    std::fs::create_dir_all(dir.join("trips").join("day one")).unwrap();
    image::RgbaImage::from_fn(64, 40, |x, _| image::Rgba([(x * 3) as u8, 90, 160, 255])).save(dir.join("dawn.png")).unwrap();
    std::fs::write(dir.join("clip.mp4"), vec![0u8; 3000]).unwrap();
    std::fs::write(dir.join("notes.txt"), b"a few words").unwrap();
    std::fs::write(dir.join("bracket.stl"), b"solid bracket\nendsolid bracket\n").unwrap();
    dir
}

fn name_of(p: &Path) -> String {
    p.file_name().unwrap().to_string_lossy().to_string()
}

fn exact(out: &egui::FullOutput, text: &str) -> bool {
    find_text_in_shapes(&out.shapes, text).is_some_and(|f| f.text == text)
}

/// One frame of the whole Media page, `later` seconds after the last, with `events`.
fn frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, later: f64, events: Vec<egui::Event>) {
    let time = Some(ctx.input(|i| i.time) + later);
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 1000.0))), time, events, ..Default::default() };
    let _ = ctx.run(input, |ctx| crate::gui::pages::media::draw(ctx, theme, state));
}

/// Click `text` a second after the last click, as a person does: two clicks closer together than
/// that are one double-click to egui, whatever they land on.
fn tap(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, text: &str) {
    frame(ctx, theme, state, 1.0, Vec::new());
    click(ctx, theme, state, text);
}

/// ADD A FOLDER, REMOVE IT, AND IT SURVIVES A RESTART: "Add a folder" opens the app's folder
/// picker; the folder chosen there is a row with its name and path, kept in the settings (marked
/// for saving, written by `from_gui_state` and read back by `apply_to_gui_state`, which is what a
/// restart does); Remove forgets it here and leaves the folder itself where it was.
///
/// Seen red 2026-10-18 with `apply_to_gui_state` not reading `media_folders`: "the folder survives
/// a restart" (left: [] / right: [the folder]).
#[test]
fn a_folder_is_added_and_removed_and_survives_a_restart() {
    let (ctx, theme) = ctx_and_theme();
    let mut state = on_the_tab();
    let root = crate::test_temp::dir("yf_add");
    assert!(shows(&settled(&ctx, &theme, &mut state), NONE_YET), "with none yet, it says how to add one");

    tap(&ctx, &theme, &mut state, ADD_FOLDER);
    let picker = state.media.folders.picker.as_mut().expect("Add a folder opens the folder picker");
    assert!(picker.dir_mode, "a picker that chooses a folder");
    picker.current_dir = root.to_path_buf();
    tap(&ctx, &theme, &mut state, &format!("Use folder: {}", name_of(&root)));
    assert!(state.media.folders.picker.is_none(), "the picker closes");
    assert_eq!(state.settings.media_folders, vec![root.to_path_buf()], "the folder is added");
    assert!(state.settings_dirty, "and the settings are saved with it");
    let out = settled(&ctx, &theme, &mut state);
    assert!(exact(&out, &name_of(&root)) && exact(&out, &root.display().to_string()), "its row shows its name and its path");

    // A restart: the settings written, then read into a fresh app.
    let json = serde_json::to_string(&crate::config::AppConfig::from_gui_state(&state)).unwrap();
    let back: crate::config::AppConfig = serde_json::from_str(&json).unwrap();
    let mut fresh = GuiState::default();
    back.apply_to_gui_state(&mut fresh);
    assert_eq!(fresh.settings.media_folders, vec![root.to_path_buf()], "the folder survives a restart");

    state.settings_dirty = false;
    tap(&ctx, &theme, &mut state, REMOVE);
    assert!(state.settings.media_folders.is_empty(), "Remove forgets it");
    assert!(state.settings_dirty, "and that is saved too");
    assert!(state.media.folders.root.is_none(), "it is no longer open");
    assert!(root.is_dir(), "nothing on the disk is touched");
}

/// SUB-FOLDERS OPEN IN PLACE, AND UP NEVER GOES ABOVE THE FOLDER ADDED: a click on the folder's
/// row opens it, a click on a sub-folder opens that, Up goes back, and at the folder added Up
/// stays there (the tab never leaves the folders the person chose).
///
/// Seen red 2026-10-18 with Up allowed at the folder added (its button enabled and `up` without
/// its guard): "Up stops at the folder added" got the folder above it.
#[test]
fn sub_folders_open_and_up_stops_at_the_folder_added() {
    let (ctx, theme) = ctx_and_theme();
    let mut state = on_the_tab();
    let root = folder("yf_up");
    state.settings.media_folders = vec![root.to_path_buf()];
    assert!(shows(&settled(&ctx, &theme, &mut state), PICK_ONE), "with none open, it says to click one");
    tap(&ctx, &theme, &mut state, &name_of(&root));
    assert_eq!(state.media.folders.root, Some(root.to_path_buf()), "the row's click opens it");
    tap(&ctx, &theme, &mut state, "trips");
    assert_eq!(state.media.folders.dir, root.join("trips"), "a sub-folder opens in place");
    tap(&ctx, &theme, &mut state, "day one");
    assert_eq!(state.media.folders.dir, root.join("trips").join("day one"), "and one inside it");
    tap(&ctx, &theme, &mut state, UP);
    tap(&ctx, &theme, &mut state, UP);
    assert_eq!(state.media.folders.dir, root.to_path_buf(), "Up goes back to the folder added");
    tap(&ctx, &theme, &mut state, UP);
    assert_eq!(state.media.folders.dir, root.to_path_buf(), "Up stops at the folder added");
}

/// THE THREE VIEWS SHOW A FOLDER'S FILES, EACH WITH ITS TYPE: List, Details and Pictures (the file
/// picker's own, widgets/file_views.rs) each draw every entry's name and its type's badge (Folder,
/// Picture, Video, Text, 3D model).
///
/// Seen red 2026-10-18 with the List view's badge left out (file_views.rs `list`): "List: the
/// badge \"Picture\" is drawn".
#[test]
fn the_three_views_show_a_folders_files_with_their_types() {
    let (ctx, theme) = ctx_and_theme();
    let mut state = on_the_tab();
    let root = folder("yf_views");
    add_folder(&mut state, &root);
    for (view, name) in [(PickerView::List, "List"), (PickerView::Details, "Details"), (PickerView::Pictures, "Pictures")] {
        memory::with(|m| {
            m.view = view;
            m.across = Some(5);
        });
        let out = settled(&ctx, &theme, &mut state);
        for entry in ["trips", "dawn.png", "clip.mp4", "notes.txt", "bracket.stl"] {
            assert!(shows(&out, entry), "{name}: {entry} is drawn");
        }
        for badge in ["Picture", "Video", "Text", "3D model"] {
            assert!(exact(&out, badge), "{name}: the badge {badge:?} is drawn");
        }
        let folders = find_text_in_shapes(&out.shapes, "Folder").map(|f| f.matches).unwrap_or(0);
        assert!(folders >= 2, "{name}: the sub-folder has its badge too, beside the folder's own row ({folders})");
    }
}

/// TWO FILES TICKED AND PUT ON MEDIA OPEN ADD TO MEDIA: a click ticks a file, "2 chosen" says so,
/// and Put on Media opens Add to Media on the Media tab with the first ticked as the item and the
/// second as a file added to it, exactly as dropping them on the window does; nothing is uploaded
/// (the form waits for Upload), and the ticks are done with.
///
/// Seen red 2026-10-18 with `put_on_media` handing over the ticks in reverse: "the first ticked is
/// the item" got second.stl.
#[test]
fn two_files_ticked_and_put_on_media_open_add_to_media() {
    let (ctx, theme) = ctx_and_theme();
    let mut state = on_the_tab();
    let root = crate::test_temp::dir("yf_put");
    std::fs::write(root.join("first.webm"), vec![1u8; 4000]).unwrap();
    std::fs::write(root.join("second.stl"), b"solid s\nendsolid s\n").unwrap();
    add_folder(&mut state, &root);
    tap(&ctx, &theme, &mut state, "first.webm");
    tap(&ctx, &theme, &mut state, "second.stl");
    let ticked: Vec<&str> = state.media.folders.chosen.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(ticked, ["first.webm", "second.stl"], "a click ticks, in the order clicked");
    assert!(shows(&settled(&ctx, &theme, &mut state), "2 chosen"), "the line says how many");

    tap(&ctx, &theme, &mut state, PUT_ON_MEDIA);
    assert_eq!(state.media.tab, MediaTab::Media, "the Files tab is shown");
    let form = state.media.form.clone().expect("Add to Files is open");
    assert_eq!(form.source, add::Source::Picked { path: root.join("first.webm"), name: "first.webm".into(), len: 4000 }, "the first ticked is the item");
    assert_eq!(form.more.iter().map(|m| m.path.clone()).collect::<Vec<_>>(), vec![root.join("second.stl")], "the second is added to it");
    assert!(state.media.uploads.is_empty(), "nothing is uploaded before Upload is pressed");
    assert!(state.media.folders.chosen.is_empty(), "the ticks are done with");
    let out = settled(&ctx, &theme, &mut state);
    assert!(shows(&out, add::MORE_LEAD) && shows(&out, add::submit_words(&form)), "the form is on screen with both files");

    // Signed out, Put on Media cannot go, and says why.
    let mut out_of_it = on_the_tab();
    out_of_it.upload_token = None;
    add_folder(&mut out_of_it, &root);
    tap(&ctx, &theme, &mut out_of_it, "first.webm");
    assert!(shows(&settled(&ctx, &theme, &mut out_of_it), NEED_SIGN_IN), "signed out, it says to sign in first");
    tap(&ctx, &theme, &mut out_of_it, PUT_ON_MEDIA);
    assert!(out_of_it.media.form.is_none() && out_of_it.media.tab == MediaTab::YourFolders, "and nothing opens");
}

/// A FILE PUT ON MEDIA FROM HERE IS MARKED UNTIL IT CHANGES: an Add to Media upload from a file on
/// this computer, published by a server on this machine's loopback address, is noted in the private
/// record (the item's number and title, the file's path, size and date); in Your folders the file
/// then carries "On <server> as <title>", a link that opens the item on the Media tab; once the
/// file's size changes it is a different file and the mark goes.
///
/// Seen red 2026-10-18 with `note_published` not called from uploading.rs `published`: "the publish
/// is noted" (left: 0 rows / right: 1); and with `marks_for` ignoring the size: "a file whose size
/// changed is not marked".
#[test]
fn a_file_put_on_media_from_here_is_marked_until_it_changes() {
    let (ctx, theme) = ctx_and_theme();
    let (base, _seen) = http_server(vec![(Duration::ZERO, 200, r#"{"ok":true,"item":{"id":51,"title":"Dusk","url":"/uploads/1700_dusk.webm"}}"#)]);
    let mut state = page_state(demo_answer(), true);
    state.server_url = base.clone();
    sync_server(&mut state);
    state.media.requested = true;
    state.upload_token = Some((base.clone(), crate::gui::pages::media::resumable::tests::TOKEN.to_string()));
    state.private_key_bytes = Some(vec![7u8; 32]);
    let root = crate::test_temp::dir("yf_marks");
    let file = root.join("dusk.webm");
    std::fs::write(&file, (0..20_000).map(|i| (i % 251) as u8).collect::<Vec<u8>>()).unwrap();
    std::fs::write(root.join("other.webm"), vec![3u8; 500]).unwrap();
    let form = add::Form { title: "Dusk".into(), ..add::Form::new(add::Source::Picked { path: file.clone(), name: "dusk.webm".into(), len: 20_000 }) };
    uploading::start_with(&mut state, &form, crate::gui::pages::media::resumable::tests::Fake::new(8192)).expect("it starts");
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !state.media.uploads.is_empty() {
        poll(&ctx, &mut state);
        uploading::poll(&ctx, &mut state);
        assert!(std::time::Instant::now() < deadline, "the upload is published: {}", state.media.status);
        std::thread::sleep(Duration::from_millis(5));
    }
    let rows = from_here::with(|r| r.sent.clone());
    assert_eq!(rows.len(), 1, "the publish is noted (left: rows / right: 1): {}", state.media.status);
    assert_eq!((rows[0].item, rows[0].title.as_str(), rows[0].file.path.clone(), rows[0].file.size), (51, "Dusk", file.clone(), 20_000), "with its item and its file");

    state.media.tab = MediaTab::YourFolders;
    add_folder(&mut state, &root);
    let host = crate::gui::pages::server_strip::host_of(&base);
    let mark = format!("On {host} as Dusk");
    let out = settled(&ctx, &theme, &mut state);
    assert!(exact(&out, &mark), "the file is marked {mark:?}");
    assert_eq!(find_text_in_shapes(&out.shapes, &format!("On {host}")).map(|f| f.matches), Some(1), "and only that file");

    tap(&ctx, &theme, &mut state, &mark);
    assert_eq!(state.media.tab, MediaTab::Media, "the mark goes to the Files tab");
    assert_eq!(state.media.item.as_ref().map(|p| p.id), Some(51), "and opens the item");

    state.media.item = None;
    state.media.open = None;
    state.media.tab = MediaTab::YourFolders;
    std::fs::write(&file, vec![9u8; 20_001]).unwrap();
    std::thread::sleep(RELIST + Duration::from_millis(100));
    let out = settled(&ctx, &theme, &mut state);
    assert!(shows(&out, "dusk.webm"), "the file is still listed");
    assert!(!shows(&out, &mark), "a file whose size changed is not marked");
}

/// A FOLDER THAT IS NOT THERE SAYS SO AND IS NOT OPENED: a folder added and then gone (a drive
/// unplugged) keeps its row, with the line saying why it is empty, and a click on it opens
/// nothing; Remove still takes it off.
///
/// Seen red 2026-10-18 with a missing folder's name clickable like any other: "a click on it opens
/// nothing" found it open.
#[test]
fn a_folder_that_is_not_there_says_so() {
    let (ctx, theme) = ctx_and_theme();
    let mut state = on_the_tab();
    let parent = crate::test_temp::dir("yf_missing");
    let gone = parent.join("unplugged_drive_folder");
    state.settings.media_folders = vec![gone.clone()];
    let out = settled(&ctx, &theme, &mut state);
    assert!(shows(&out, MISSING), "its row says it is not there");
    assert!(exact(&out, &gone.display().to_string()), "with its path");
    tap(&ctx, &theme, &mut state, "unplugged_drive_folder");
    assert!(state.media.folders.root.is_none(), "a click on it opens nothing");
    tap(&ctx, &theme, &mut state, REMOVE);
    assert!(state.settings.media_folders.is_empty(), "Remove still takes it off");
}

/// A DOUBLE-CLICK ON A FOLDER OPENS IT AND TICKS NOTHING: its first click opens the folder, and its
/// second, landing on the file the folder now shows under the pointer, is not taken as a tick; a
/// click a moment later is.
///
/// Seen red 2026-10-18 with the double-click guard left out of `draw_browser`: "the second click
/// ticks nothing" found the file inside ticked.
#[test]
fn a_double_click_on_a_folder_opens_it_and_ticks_nothing() {
    let (ctx, theme) = ctx_and_theme();
    let mut state = on_the_tab();
    let root = crate::test_temp::dir("yf_double");
    let inner = root.join("a folder with a long name, to land on");
    std::fs::create_dir_all(&inner).unwrap();
    std::fs::write(inner.join("inside, under the pointer.txt"), b"words").unwrap();
    add_folder(&mut state, &root);
    let out = settled(&ctx, &theme, &mut state);
    let at = find_text_in_shapes(&out.shapes, "a folder with a long name, to land on").expect("the folder is listed").rect.center();
    let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::default() };
    frame(&ctx, &theme, &mut state, 1.0, vec![egui::Event::PointerMoved(at)]);
    for events in [vec![press(true)], vec![press(false)], vec![press(true)], vec![press(false)], vec![]] {
        frame(&ctx, &theme, &mut state, 1.0 / 60.0, events);
    }
    assert_eq!(state.media.folders.dir, inner, "the first click opens it");
    assert!(state.media.folders.chosen.is_empty(), "the second click ticks nothing: {:?}", state.media.folders.chosen.iter().map(|c| &c.name).collect::<Vec<_>>());
    tap(&ctx, &theme, &mut state, "inside, under the pointer.txt");
    assert_eq!(state.media.folders.chosen.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["inside, under the pointer.txt"], "a click a moment later ticks it");
}

