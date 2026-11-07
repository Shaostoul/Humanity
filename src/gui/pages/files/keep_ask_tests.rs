//! THE FILES PAGE ASKS "SEND IT ANYWAY, OR DON'T SEND?" (2026-11-07, docs/BUGS.md BUG-264; the
//! operator: "Let's do that."), as the chat does since BUG-260: Add to Files, Shared Files, and an
//! upload that stops because its file could not be checked (an item's Edit, which has no look of its
//! own). The same two answers everywhere (chat/attach_step.rs `keep_answers`): "Don't send"
//! first, the primary, holding the focus so Enter answers it; "Send anyway" second, each with its
//! hint by key. "Send anyway" sends the file exactly as picked, with `keep_metadata` and the
//! person's signed choice. The website's half: scripts/tests/files-web-files.test.js 3,
//! drop-files-web.test.js 3b and 7b. Each test was seen red first, recorded at the test.

use super::resumable::tests::{Fake, TOKEN};
use super::tests::{click, ctx_and_theme, demo_answer, page_state, settled};
use super::*;
use crate::gui::screen_surface::find_text_in_shapes;
use crate::gui::theme::Theme;
use humanity_core::i18n::t;

/// A copy of a shared fixture (tests/fixtures/describe) under a name of its own.
fn fixture_copy(fixture: &str, tag: &str, ext: &str) -> (humanity_core::test_temp::TempPath, Vec<u8>) {
    let bytes = std::fs::read(std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("tests/fixtures/describe").join(fixture)).unwrap();
    let path = humanity_core::test_temp::file(tag, ext);
    std::fs::write(&path, &bytes).unwrap();
    (path, bytes)
}

fn frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, events: Vec<egui::Event>) -> egui::FullOutput {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 1000.0))),
        events,
        ..Default::default()
    };
    ctx.run(input, |ctx| draw(ctx, theme, state))
}

fn enter() -> egui::Event {
    egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::default() }
}

/// Where `text` is drawn, the last of its kind (a window over the page).
fn at(out: &egui::FullOutput, text: &str) -> egui::Rect {
    find_text_in_shapes(&out.shapes, text).unwrap_or_else(|| panic!("{text:?} is drawn")).rect
}

/// Draw until `text` is drawn (the look is worked out on a worker), at most five seconds.
fn until_drawn(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, text: &str) -> egui::FullOutput {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let out = settled(ctx, theme, state);
        if find_text_in_shapes(&out.shapes, text).is_some() {
            return out;
        }
        assert!(std::time::Instant::now() < deadline, "{text:?} is never drawn");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// The two answers are drawn "Don't send" first, "Send anyway" second, and the focus is on one of
/// them (Enter then answers it: the tests press Enter and see which).
fn assert_the_two_answers(ctx: &egui::Context, out: &egui::FullOutput, what: &str) {
    let (no, yes) = (at(out, t("upload_keep.dont_send")), at(out, t("upload_keep.send_anyway")));
    assert!(no.left() < yes.left(), "{what}: \"Don't send\" first");
    assert!(ctx.memory(|m| m.focused().is_some()), "{what}: an answer holds the focus");
}

/// ADD TO FILES ASKS. A picture whose structure breaks (it could not be checked) opens Add to Files
/// with the chat's question and its two answers in place of Upload: Enter answers "Don't send"
/// (the form closes, nothing starts, the page says so); "Send anyway" starts the upload, and that
/// upload is the file exactly as picked with `keep_metadata` and the signed choice.
///
/// Seen red 2026-11-07 with the form's question taken out (`asking` never set, so Upload as before,
/// the clean refusing it later on the worker): "\"Don't send\" is never drawn".
#[test]
fn add_to_files_asks_about_a_file_it_could_not_check() {
    let (ctx, theme) = ctx_and_theme();
    let mut state = page_state(demo_answer(), true);
    state.files.mine_failed = true; // the share is the server's to check
    state.private_key_bytes = Some(vec![7u8; 32]);
    let (path, bytes) = fixture_copy("jpeg-lost-sync.jpg", "keep_ask_add", "jpg");
    add::picked(&mut state, &path);
    let out = until_drawn(&ctx, &theme, &mut state, t("upload_keep.dont_send"));
    assert!(find_text_in_shapes(&out.shapes, add::submit_words(state.files.form.as_ref().unwrap())).is_none(), "no Upload button while it asks");
    assert!(find_text_in_shapes(&out.shapes, t("upload_keep.question_picture")).is_some(), "the chat's question");
    assert_the_two_answers(&ctx, &out, "Add to Files");
    frame(&ctx, &theme, &mut state, vec![enter()]);
    settled(&ctx, &theme, &mut state);
    assert!(state.files.form.is_none(), "Enter answers \"Don't send\": the form closes");
    assert!(state.files.uploads.is_empty(), "nothing starts");
    assert_eq!(state.files.status, t("upload_keep.not_sent"), "and the page says so");

    add::picked(&mut state, &path);
    until_drawn(&ctx, &theme, &mut state, t("upload_keep.send_anyway"));
    click(&ctx, &theme, &mut state, t("upload_keep.send_anyway"));
    assert!(state.files.form.is_none(), "Send anyway: the form is done");
    assert_eq!(state.files.uploads.len(), 1, "its upload started");
    assert_eq!(state.files.uploads[0].record.path, path.to_path_buf(), "the file as picked");

    // What that upload sends, through the simulated server: the bytes as picked, the flag, the choice.
    let form = add::Form { anyway: true, remove: true, ..add::Form::new(add::Source::Picked { path: path.to_path_buf(), name: "garden.jpg".into(), len: bytes.len() as u64 }) };
    let fake = Fake::new(8192);
    state.upload_token = Some((tests::SERVER.to_string(), TOKEN.to_string()));
    for e in &state.files.uploads {
        e.run.ctl.cancel(); // the one started above, to a closed port on this machine
    }
    state.files.uploads.clear();
    uploading::start_with(&mut state, &form, fake.clone()).expect("it starts");
    super::uploads_tests::until(&ctx, &mut state, "the upload is up", |s| s.files.uploads.iter().all(|e| !matches!(e.after, uploading::After::Uploading)));
    assert_eq!(fake.s().data, bytes, "exactly as picked, whatever the switch says");
    let start = fake.s().start_body.clone();
    assert_eq!(start["keep_metadata"], 1, "keep_metadata");
    assert!(start["keep"]["sig"].is_string(), "with the signed choice: {start}");
}

/// SHARED FILES ASKS. The same picture picked for Shared Files: the step shows the question and the
/// two answers in place of Upload; Enter answers "Don't send" (nothing goes); "Send anyway" uploads
/// it exactly as picked, `keep_metadata=1` with the signed choice (a server on this machine's
/// loopback address takes the upload and keeps what it was sent).
///
/// Seen red 2026-11-07 with the step's question taken out (Upload as before, which the clean then
/// refused with "Not uploaded: ..."): "\"Don't send\" is drawn".
#[test]
fn shared_files_asks_about_a_file_it_could_not_check() {
    use super::shared_upload_step::{self as step_mod, Outcome};
    let (ctx, theme) = ctx_and_theme();
    let (base, seen) = super::uploads_tests::http_server(vec![(std::time::Duration::ZERO, 200, r#"{"url":"/uploads/1_garden.jpg"}"#)]);
    let mut state = page_state(demo_answer(), true);
    state.private_key_bytes = Some(vec![7u8; 32]);
    let (path, bytes) = fixture_copy("jpeg-lost-sync.jpg", "keep_ask_shared", "jpg");
    let mut step = step_mod::start(&state, path.to_path_buf(), "garden.jpg".into());
    let run = |state: &mut GuiState, step: &mut step_mod::UploadStep, events: Vec<egui::Event>| {
        let mut outcome = None;
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 1000.0))), events, ..Default::default() };
        let out = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| outcome = Some(step_mod::draw(ui, &theme, state, step, &base, TOKEN)));
        });
        (out, outcome.unwrap())
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let out = loop {
        let (out, o) = run(&mut state, &mut step, Vec::new());
        assert!(matches!(o, Outcome::Waiting), "it waits for the person");
        if find_text_in_shapes(&out.shapes, t("upload_keep.dont_send")).is_some() {
            let (out, _) = run(&mut state, &mut step, Vec::new());
            break out;
        }
        assert!(std::time::Instant::now() < deadline, "\"Don't send\" is drawn");
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    assert!(find_text_in_shapes(&out.shapes, "Upload").is_none_or(|f| f.text != "Upload"), "no Upload button while it asks");
    assert_the_two_answers(&ctx, &out, "Shared Files");
    let (_, o) = run(&mut state, &mut step, vec![enter()]);
    assert!(matches!(o, Outcome::NotSent), "Enter answers \"Don't send\"");

    let mut step = step_mod::start(&state, path.to_path_buf(), "garden.jpg".into());
    let mut out = run(&mut state, &mut step, Vec::new()).0;
    while find_text_in_shapes(&out.shapes, t("upload_keep.send_anyway")).is_none() {
        assert!(std::time::Instant::now() < deadline, "\"Send anyway\" is drawn");
        std::thread::sleep(std::time::Duration::from_millis(10));
        out = run(&mut state, &mut step, Vec::new()).0;
    }
    let pos = at(&out, t("upload_keep.send_anyway")).center();
    let m = egui::Modifiers::default();
    run(&mut state, &mut step, vec![egui::Event::PointerMoved(pos)]);
    run(&mut state, &mut step, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
    let (_, o) = run(&mut state, &mut step, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
    let Outcome::Uploading(rx) = o else { panic!("Send anyway uploads it") };
    let answer = rx.recv_timeout(std::time::Duration::from_secs(10)).expect("the upload answers");
    assert!(answer.is_ok(), "the upload went: {answer:?}");
    let sent = seen.lock().unwrap().clone();
    let line: String = sent[0].chars().take(120).collect();
    assert!(sent[0].starts_with("POST /api/upload?") && sent[0].contains("keep_metadata=1") && sent[0].contains("share=1"), "keep_metadata=1: {line}");
    assert!(sent[0].contains("name=\"keep\"") && sent[0].contains("\"sig\""), "with the signed choice");
    assert!(sent[0].contains("Exif") && bytes.windows(4).any(|w| w == b"Exif"), "the file as picked, its hidden details in it");
}

/// AN UPLOAD THAT COULD NOT BE CHECKED ASKS (an item's Edit, which has no look of its own). The
/// worker stops before anything reaches the server, the row goes,
/// and the page asks; Enter answers "Don't send" and says so; "Send anyway" uploads the person's
/// own file again, exactly as picked, with `keep_metadata` and the signed choice, to the same item.
///
/// Seen red 2026-11-07 with the worker failing such a file as before (`Event::Failed` with the old
/// refusal words in place of `Event::Unchecked`): "the page asks" (nothing asked).
#[test]
fn an_upload_that_could_not_be_checked_is_asked_about() {
    use super::api_items::{AttachTo, FileRole};
    let (ctx, theme) = ctx_and_theme();
    let mut state = page_state(demo_answer(), true);
    state.private_key_bytes = Some(vec![7u8; 32]);
    state.upload_token = Some((tests::SERVER.to_string(), TOKEN.to_string()));
    let (path, bytes) = fixture_copy("jpeg-lost-sync.jpg", "keep_ask_attach", "jpg");
    let to = AttachTo { id: 7, filename: "7_dusk.glb".into(), role: FileRole::File };
    let fake = Fake::new(8192);
    uploading::start_attach_with(&mut state, &path, "garden.jpg", bytes.len() as u64, to.clone(), fake.clone()).expect("it starts");
    super::uploads_tests::until(&ctx, &mut state, "the page asks", |s| s.files.keep_ask.is_some());
    assert!(state.files.uploads.is_empty(), "the row goes");
    assert_eq!(fake.count("start"), 0, "nothing reached the server");
    let out = settled(&ctx, &theme, &mut state);
    assert!(find_text_in_shapes(&out.shapes, t("upload_keep.question_picture")).is_some(), "the chat's question");
    assert_the_two_answers(&ctx, &out, "the question");
    frame(&ctx, &theme, &mut state, vec![enter()]);
    settled(&ctx, &theme, &mut state);
    assert!(state.files.keep_ask.is_none(), "Enter answers \"Don't send\"");
    assert_eq!(state.files.status, t("upload_keep.not_sent"));
    assert!(state.files.uploads.is_empty(), "and nothing starts");

    uploading::start_attach_with(&mut state, &path, "garden.jpg", bytes.len() as u64, to, Fake::new(8192)).expect("it starts");
    super::uploads_tests::until(&ctx, &mut state, "the page asks again", |s| s.files.keep_ask.is_some());
    let record = state.files.keep_ask.take().unwrap();
    assert_eq!(record.path, path.to_path_buf(), "the person's own file");
    let fake = Fake::new(8192);
    uploading::send_anyway_with(&mut state, record, fake.clone()).expect("Send anyway starts it");
    assert!(state.files.uploads[0].record.attach.is_some(), "to the same item");
    super::uploads_tests::until(&ctx, &mut state, "the upload is up", |s| s.files.uploads.iter().all(|e| !matches!(e.after, uploading::After::Uploading)));
    assert_eq!(fake.s().data, bytes, "exactly as picked");
    let start = fake.s().start_body.clone();
    assert_eq!(start["keep_metadata"], 1, "keep_metadata");
    assert!(start["keep"]["sig"].is_string(), "with the signed choice: {start}");
}

/// THE SERVER'S 422 AT THE FINISH IS ASKED ABOUT (2026-11-07, BUG-275; BUG-264's "Not in this
/// fix"). An upload this app cleaned (a phone photo, so its cleaned copy is what goes up) that the
/// server still could not clean at the finish (its 422, the relay's `CANNOT_CLEAN`; it drops the
/// upload, nothing stored): the row goes and the page asks the chat's question about the file AS
/// PICKED, not the cleaned copy, for Add to Files and an item's Edit alike. Enter answers "Don't
/// send": nothing starts again and the page says so. "Send anyway" uploads the person's own file
/// exactly as picked, with `keep_metadata` and the signed choice, to the same place. Under the
/// protected setup the server's words stay and nothing is asked.
///
/// Seen red 2026-11-07 against the worker and page from before (a 422 at the finish ended as
/// `Event::Failed`): "Add to Files: the page asks (it said \"garden.jpg: This picture could not be
/// read all the way through, ...\")".
#[test]
fn the_servers_422_at_the_finish_is_asked_about() {
    use super::api_items::{AttachTo, FileRole};
    use super::resumable::tests::Trick;
    use std::sync::Arc;
    let (ctx, theme) = ctx_and_theme();
    let to = AttachTo { id: 7, filename: "7_dusk.glb".into(), role: FileRole::File };
    let (path, bytes) = fixture_copy("jpeg-phone.jpg", "keep_ask_server", "jpg");
    let form = add::Form { remove: true, ..add::Form::new(add::Source::Picked { path: path.to_path_buf(), name: "garden.jpg".into(), len: bytes.len() as u64 }) };
    let start = |state: &mut GuiState, route: &str, fake: Arc<Fake>| {
        if route == "Add to Files" {
            uploading::start_with(state, &form, fake).expect("it starts");
        } else {
            uploading::start_attach_with(state, &path, "garden.jpg", bytes.len() as u64, to.clone(), fake).expect("it starts");
        }
    };
    let refusing = || {
        let fake = Fake::new(8192);
        fake.s().finish_trick = Some(Trick::CannotClean);
        fake
    };
    let ended = |s: &GuiState| s.files.keep_ask.is_some() || s.files.uploads.is_empty();
    for route in ["Add to Files", "an item's Edit"] {
        let mut state = page_state(demo_answer(), true);
        state.private_key_bytes = Some(vec![7u8; 32]);
        state.upload_token = Some((tests::SERVER.to_string(), TOKEN.to_string()));
        let fake = refusing();
        start(&mut state, route, fake.clone());
        super::uploads_tests::until(&ctx, &mut state, "the upload ends", ended);
        assert!(state.files.keep_ask.is_some(), "{route}: the page asks (it said {:?})", state.files.status);
        assert!(state.files.uploads.is_empty(), "{route}: the row goes");
        assert_ne!(fake.s().data, bytes, "{route}: test setup: the cleaned copy went up first");
        assert!(fake.s().id.is_none() && fake.s().done.is_none(), "{route}: the server stored nothing");
        let record = state.files.keep_ask.clone().unwrap();
        assert_eq!((record.path.clone(), record.size), (path.to_path_buf(), bytes.len() as u64), "{route}: the file as picked, not the cleaned copy");
        let out = settled(&ctx, &theme, &mut state);
        assert!(find_text_in_shapes(&out.shapes, t("upload_keep.question_picture")).is_some(), "{route}: the chat's question");
        assert_the_two_answers(&ctx, &out, route);
        frame(&ctx, &theme, &mut state, vec![enter()]);
        settled(&ctx, &theme, &mut state);
        assert!(state.files.keep_ask.is_none(), "{route}: Enter answers \"Don't send\"");
        assert_eq!(state.files.status, t("upload_keep.not_sent"), "{route}: and the page says so");
        assert!(state.files.uploads.is_empty() && fake.count("start") == 1, "{route}: nothing starts again");

        // "Send anyway" in the window starts the person's own file again...
        start(&mut state, route, refusing());
        super::uploads_tests::until(&ctx, &mut state, "the upload ends again", ended);
        assert!(state.files.keep_ask.is_some(), "{route}: asked again");
        click(&ctx, &theme, &mut state, t("upload_keep.send_anyway"));
        assert!(state.files.keep_ask.is_none(), "{route}: Send anyway answers it");
        assert_eq!(state.files.uploads.len(), 1, "{route}: its upload started");
        assert_eq!(state.files.uploads[0].record.path, path.to_path_buf(), "{route}: the file as picked");
        assert_eq!(state.files.uploads[0].record.attach.is_some(), route != "Add to Files", "{route}: to the same place");
        for e in &state.files.uploads {
            e.run.ctl.cancel(); // that one went to a closed port on this machine
        }
        state.files.uploads.clear();

        // ...and what it sends, through the simulated server: the bytes as picked, the flag, the choice.
        start(&mut state, route, refusing());
        super::uploads_tests::until(&ctx, &mut state, "the upload ends a third time", ended);
        let record = state.files.keep_ask.take().expect("asked a third time");
        let again = Fake::new(8192);
        uploading::send_anyway_with(&mut state, record, again.clone()).expect("Send anyway starts it");
        super::uploads_tests::until(&ctx, &mut state, "the upload is up", |s| s.files.uploads.iter().all(|e| !matches!(e.after, uploading::After::Uploading)));
        assert_eq!(again.s().data, bytes, "{route}: exactly as picked");
        let body = again.s().start_body.clone();
        assert_eq!(body["keep_metadata"], 1, "{route}: keep_metadata");
        assert!(body["keep"]["sig"].is_string(), "{route}: with the signed choice: {body}");
    }

    // The protected setup sends no file with its details: the server's words, nothing asked.
    let mut state = page_state(demo_answer(), true);
    state.private_key_bytes = Some(vec![7u8; 32]);
    state.upload_token = Some((tests::SERVER.to_string(), TOKEN.to_string()));
    state.protected.setup.on = true;
    let fake = refusing();
    start(&mut state, "Add to Files", fake.clone());
    super::uploads_tests::until(&ctx, &mut state, "the protected upload ends", ended);
    assert!(state.files.keep_ask.is_none(), "the protected setup: nothing asked");
    assert!(state.files.status.contains(crate::relay::upload_rules::PICTURE_UNREADABLE), "the protected setup: the server's words: {:?}", state.files.status);
    assert_eq!(fake.count("start"), 1, "the protected setup: nothing more is sent");
}

/// SHARED FILES ASKS AT THE SERVER'S 422 (2026-11-07, BUG-275). A phone photo picked for Shared
/// Files is cleaned and uploaded on Upload; the server answers 422 (it could not clean it after
/// all, and stored nothing): the Shared Files place asks the chat's question in its own window
/// about the file as picked. Enter answers "Don't send" (nothing more is sent, and it says so);
/// "Send anyway" uploads it again exactly as picked, `keep_metadata=1` with the signed choice.
/// Under the protected setup the server's words stay. (A server on this machine's loopback address
/// answers 422, then stores the second, then lists.)
///
/// Seen red 2026-11-07 against the Shared Files code from before (the 422 was shown as the server's
/// words): "dont-send: the question opens (the place said \"This picture could not be read all the
/// way through, ...\")".
#[test]
fn shared_files_asks_at_the_servers_422() {
    use std::time::Duration;
    let (ctx, theme) = ctx_and_theme();
    let words = crate::relay::upload_rules::PICTURE_UNREADABLE;
    let run = |state: &mut GuiState, events: Vec<egui::Event>| {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 1000.0))), events, ..Default::default() };
        ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| super::shared_files::draw_list(ui, &theme, state));
        })
    };
    let until = |state: &mut GuiState, what: &str, done: &dyn Fn(&GuiState, &egui::FullOutput) -> bool| -> egui::FullOutput {
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            let out = run(state, Vec::new());
            if done(state, &out) {
                return run(state, Vec::new());
            }
            assert!(std::time::Instant::now() < deadline, "{what} (the place said {:?})", state.files.shared.shared_status);
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    let press = |state: &mut GuiState, out: &egui::FullOutput, text: &str| {
        let pos = at(out, text).center();
        let m = egui::Modifiers::default();
        run(state, vec![egui::Event::PointerMoved(pos)]);
        run(state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
        run(state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
    };
    let exact = |out: &egui::FullOutput, text: &str| find_text_in_shapes(&out.shapes, text).is_some_and(|f| f.text == text);
    for answer in ["dont-send", "send-anyway", "protected"] {
        let zero = Duration::ZERO;
        let (base, seen) = super::uploads_tests::http_server(vec![(zero, 422, words), (zero, 200, r#"{"url":"/uploads/1_garden.jpg"}"#), (zero, 200, r#"{"files":[]}"#)]);
        let mut state = page_state(demo_answer(), true);
        state.private_key_bytes = Some(vec![7u8; 32]);
        state.server_url = base.clone();
        let here = crate::gui::pages::chat::norm_server_url(&base);
        state.upload_token = Some((here.clone(), TOKEN.to_string()));
        state.files.shared.shared_server = here;
        state.files.shared.shared_loaded = true;
        state.protected.setup.on = answer == "protected";
        let (path, bytes) = fixture_copy("jpeg-phone.jpg", "keep_ask_shared_422", "jpg");
        state.files.shared.shared_step = Some(super::shared_upload_step::start(&state, path.to_path_buf(), "garden.jpg".into()));
        // The step shows what the photo holds; Upload sends it cleaned.
        let out = until(&mut state, "the step offers Upload", &|_, out| exact(out, "Upload"));
        press(&mut state, &out, "Upload");
        let answered = |s: &GuiState, out: &egui::FullOutput| find_text_in_shapes(&out.shapes, t("upload_keep.dont_send")).is_some() || s.files.shared.shared_status.contains("could not be read");
        let out = until(&mut state, "the server answers", &answered);
        let first = seen.lock().unwrap()[0].clone();
        let line: String = first.chars().take(120).collect();
        assert!(first.contains("share=1") && !first.contains("keep_metadata"), "{answer}: test setup: the first went cleaned: {line}");
        assert!(!first.contains("Exif"), "{answer}: test setup: without its hidden details");
        if answer == "protected" {
            assert!(find_text_in_shapes(&out.shapes, t("upload_keep.dont_send")).is_none(), "the protected setup: nothing asked");
            assert_eq!(state.files.shared.shared_status, words, "the protected setup: the server's words");
            assert_eq!(seen.lock().unwrap().len(), 1, "the protected setup: nothing more is sent");
            continue;
        }
        assert!(find_text_in_shapes(&out.shapes, t("upload_keep.question_picture")).is_some(), "{answer}: the question opens (the place said {:?})", state.files.shared.shared_status);
        assert_the_two_answers(&ctx, &out, "Shared Files");
        if answer == "dont-send" {
            run(&mut state, vec![enter()]);
            let out = run(&mut state, Vec::new());
            assert!(find_text_in_shapes(&out.shapes, t("upload_keep.dont_send")).is_none(), "Enter answers \"Don't send\"");
            assert_eq!(state.files.shared.shared_status, t("upload_keep.not_sent"), "and the place says so");
            std::thread::sleep(Duration::from_millis(50));
            assert_eq!(seen.lock().unwrap().len(), 1, "nothing more is sent");
            continue;
        }
        press(&mut state, &out, t("upload_keep.send_anyway"));
        until(&mut state, "it is sent again and stored", &|s, _| s.files.shared.shared_status.starts_with("Done") || s.files.shared.shared_status.contains("file(s) on the server"));
        let second = seen.lock().unwrap()[1].clone();
        let line: String = second.chars().take(120).collect();
        assert!(second.starts_with("POST /api/upload?") && second.contains("share=1") && second.contains("keep_metadata=1"), "Send anyway: keep_metadata=1: {line}");
        assert!(second.contains("name=\"keep\"") && second.contains("\"sig\""), "Send anyway: with the signed choice");
        assert!(second.contains("Exif") && bytes.windows(4).any(|w| w == b"Exif"), "Send anyway: the photo as picked, its hidden details in it");
    }
}
