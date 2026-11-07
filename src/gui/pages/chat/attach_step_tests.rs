// Child of chat/attach_step.rs (#[path]): the attach step's rules and its flow, with a real file
// looked at by the worker and the frames drawn headless.

use super::*;
use crate::net::files_policy::{AcceptFrom, FilesInMessages};

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("tests/fixtures/describe").join(name)
}

fn look_of(name: &str) -> Result<Look, String> {
    crate::net::file_clean::look_file(&fixture(name), name).map_err(|e| e.to_string())
}

/// What may be sent: a checked file always; a file that could not be checked only as "Send anyway"
/// (to a private conversation, and since 2026-11-06 to a public place too, BUG-260: there it goes as
/// it is with the signed choice), never under the protected setup (a DM included); kept by choice
/// (the switch off) it goes anywhere as it is; nothing before the look.
/// Seen red 2026-10-11 with `verdict` ignoring the protected setup: "the protected setup sends no
/// unchecked file, not even to a DM".
#[test]
fn what_may_be_sent() {
    let fine = look_of("jpeg-phone.jpg");
    let broken = look_of("mp4-broken-box.mp4");
    assert!(broken.as_ref().unwrap().unchecked, "test setup");
    let v = |look, remove, private, protected| verdict(Some(look), remove, private, protected);
    assert_eq!(v(&fine, true, false, false), Verdict { can_send: true, anyway: false });
    assert_eq!(v(&broken, true, true, false), Verdict { can_send: true, anyway: true }, "a DM: Send anyway");
    assert_eq!(v(&broken, true, false, false), Verdict { can_send: true, anyway: true }, "a public place: Send anyway (BUG-260)");
    assert_eq!(v(&broken, true, true, true), Verdict { can_send: false, anyway: false }, "the protected setup sends no unchecked file, not even to a DM");
    assert_eq!(v(&broken, false, false, false), Verdict { can_send: true, anyway: false }, "kept by choice it goes as it is");
    assert_eq!(verdict(None, true, true, false), Verdict { can_send: false, anyway: false }, "nothing before the look");
}

/// The step asks only when it has something to say: details found, a file that could not be
/// checked, or a line about the recipient; a file with nothing hidden in it goes at once. Seen red
/// 2026-10-11 with `needs_asking` ignoring the line: "a line about the recipient".
#[test]
fn the_step_asks_only_when_it_has_something_to_say() {
    assert!(needs_asking(&look_of("jpeg-phone.jpg"), None), "details found");
    assert!(needs_asking(&look_of("mp4-no-moov.mp4"), None), "could not be checked");
    assert!(!needs_asking(&look_of("other.txt"), None), "a plain file goes at once");
    assert!(!needs_asking(&look_of("jpeg-clean.jpg"), None), "a clean photo too");
    assert!(needs_asking(&look_of("other.txt"), Some(&"Ann accepts files only from friends.".to_string())), "a line about the recipient");
}

/// A signed-in app in a DM with Ann, uploads to a closed local port (so nothing leaves the machine).
fn app() -> GuiState {
    let mut gs = GuiState::default();
    gs.server_url = "http://127.0.0.1:9".into();
    gs.profile_public_key = "me".into();
    gs.chat_active_channel = "dm:ann".into();
    gs.upload_token = Some((norm_server_url("http://127.0.0.1:9"), "tok".into()));
    gs
}

/// Draw frames until the step's look is in (the worker reads the file), at most two seconds.
fn frames_until_looked(gs: &mut GuiState) {
    let ctx = egui::Context::default();
    let theme = crate::gui::theme::load_theme();
    for _ in 0..200 {
        let _ = ctx.run(egui::RawInput::default(), |ctx| draw(ctx, &theme, gs));
        let looked = gs.files_flow.step.as_ref().map_or(true, |s| s.looking.look.is_some() && s.opened.elapsed() > QUIET_LOOK);
        if looked {
            let _ = ctx.run(egui::RawInput::default(), |ctx| draw(ctx, &theme, gs));
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// The flow: a phone photo opens the step and waits for Send, its switch as last chosen; a plain
/// file goes at once (its upload starts, the step is gone); under the protected setup the switch
/// starts on whatever was last chosen; leaving the conversation drops the step with nothing sent;
/// the sender's line appears from Ann's published word. Seen red 2026-10-11 with `draw` sending
/// every file once its look was in: "a phone photo waits for Send".
#[test]
fn a_photo_waits_for_send_and_a_plain_file_goes_at_once() {
    let mut gs = app();
    gs.settings.files_in_messages = FilesInMessages { remove_details: false, ..Default::default() };
    let photo = fixture("jpeg-phone.jpg");
    let job = attach_send::attach_file_job(&mut gs, &photo).expect("a job");
    open(&mut gs, job, &photo);
    frames_until_looked(&mut gs);
    let step = gs.files_flow.step.as_ref().expect("a phone photo waits for Send");
    assert!(!step.remove, "the switch starts as last chosen");
    assert!(gs.clipboard_upload.is_none(), "nothing uploaded yet");

    gs.chat_active_channel = "global".into();
    frames_until_looked(&mut gs);
    assert!(gs.files_flow.step.is_none() && gs.clipboard_upload.is_none(), "leaving the conversation drops it, nothing sent");

    gs.chat_active_channel = "dm:ann".into();
    let plain = fixture("other.txt");
    let job = attach_send::attach_file_job(&mut gs, &plain).expect("a job");
    open(&mut gs, job, &plain);
    frames_until_looked(&mut gs);
    assert!(gs.files_flow.step.is_none(), "a plain file does not ask");
    assert!(gs.clipboard_upload.is_some(), "its upload started at once");

    let mut gs = app();
    gs.settings.files_in_messages.remove_details = false;
    gs.protected.setup.on = true;
    assert!(remove_at_start(&gs), "the protected setup always removes");
    remember_switch(&mut gs, false);
    assert!(!gs.settings_dirty, "and does not remember a choice it did not let the person make");

    let mut gs = app();
    gs.files_flow.accepts.insert((norm_server_url("http://127.0.0.1:9"), "ann".into()), ("Ann".into(), AcceptFrom::Friends.word().into()));
    assert!(sender_line_for(&gs, "ann").unwrap().starts_with("Ann accepts files only from friends."), "the sender's line");
}

/// THE WORD IS ASKED BY NAME ON THE SIGNED-IN SOCKET, AS THE WEBSITE ASKS (BUG-241, 2026-11-03).
/// Opening a DM with Ann sends one `profile_request` for her name (every profile answer carries
/// `accepts_files`, relay handlers/profile.rs, whether she is in the public directory or not), and
/// her answer gives the sender's line. It asked `GET /api/members/{key}` until then, which since
/// BUG-241 finds only someone who chose to be listed, so the line was never shown for anyone else.
/// An answer about someone not asked about, or for another server, changes nothing.
///
/// Seen red 2026-11-03 against the old lookup (a GET to the server on a worker, nothing on the
/// socket): "one profile_request for her name, on the socket" (left [], right [{"name":"Ann",
/// "type":"profile_request"}]).
#[test]
fn the_word_is_asked_by_name_on_the_socket() {
    let mut gs = app();
    let (client, sent) = crate::net::ws_client::WsClient::recording_as("Me");
    gs.ws_client = Some(client);
    gs.ws_identified = true;
    gs.chat_users = vec![crate::gui::ChatUser { name: "Ann".into(), public_key: "ann_key".into(), role: "member".into(), status: "online".into() }];
    let asked = |sent: &std::sync::mpsc::Receiver<String>| -> Vec<serde_json::Value> {
        sent.try_iter().map(|t| serde_json::from_str::<serde_json::Value>(&t).unwrap()).filter(|f| f["type"] == "profile_request").collect()
    };
    ask_accepts(&mut gs, "ann_key");
    assert_eq!(asked(&sent), vec![serde_json::json!({ "type": "profile_request", "name": "Ann" })], "one profile_request for her name, on the socket");
    ask_accepts(&mut gs, "ann_key");
    assert!(asked(&sent).is_empty(), "asked once per server and person");

    take_profile_answer(&mut gs, &serde_json::json!({ "type": "profile_data", "name": "Bob", "bio": "", "socials": "{}", "accepts_files": "nobody" }));
    assert!(sender_line_for(&gs, "ann_key").is_none(), "an answer about someone not asked about changes nothing");
    take_profile_answer(&mut gs, &serde_json::json!({ "type": "profile_data", "name": "Ann", "bio": "", "socials": "{}", "accepts_files": "friends" }));
    assert!(
        sender_line_for(&gs, "ann_key").is_some_and(|l| l.starts_with("Ann accepts files only from friends.")),
        "her answer gives the sender's line: {:?}",
        sender_line_for(&gs, "ann_key")
    );
}

/// The member answer's word and name are what the sender's line reads (a profile answer, or the
/// relay's `GET /api/members/{key}`, which carry the same two fields). Seen red 2026-10-11 with a
/// missing word read as "friends": "an older server says nothing".
#[test]
fn the_member_answer_gives_the_word() {
    let a = serde_json::json!({ "public_key": "ann", "name": "Ann", "accepts_files": "nobody" });
    assert_eq!(accepts_from_answer(&a), Some(("Ann".to_string(), "nobody".to_string())));
    assert_eq!(accepts_from_answer(&serde_json::json!({ "name": "Ann" })), None, "an older server says nothing");
}

/// "SEND A FILE" SHOWS THE FILE WITH ITS TYPE, THEN WHAT IS HIDDEN IN IT, EACH IN ITS OWN PART
/// (2026-10-17): The file (blue) with the badge before its name ("Picture" and ".jpg" for a phone
/// photo), then Hidden details (green) with what was found. Seen red 2026-10-17 with the badge
/// left out: "\"Picture\" is drawn" (nothing said what the file was).
#[test]
fn send_a_file_shows_the_file_with_its_type_then_what_is_hidden_in_it() {
    use crate::gui::widgets::tint_section::{painted, text_starts_at, Tint};
    let mut gs = app();
    gs.settings.files_in_messages = FilesInMessages { remove_details: false, ..Default::default() };
    let photo = fixture("jpeg-phone.jpg");
    let job = attach_send::attach_file_job(&mut gs, &photo).expect("a job");
    open(&mut gs, job, &photo);
    frames_until_looked(&mut gs);
    assert!(gs.files_flow.step.is_some(), "a phone photo waits for Send");
    let ctx = egui::Context::default();
    // No fade-in, so the colours read are the colours themselves.
    ctx.all_styles_mut(|s| s.animation_time = 0.0);
    let theme = crate::gui::theme::load_theme();
    let mut out = ctx.run(egui::RawInput::default(), |ctx| draw(ctx, &theme, &mut gs));
    for _ in 0..3 {
        out = ctx.run(egui::RawInput::default(), |ctx| draw(ctx, &theme, &mut gs));
    }
    let at = |t: &str| crate::gui::screen_surface::find_text_in_shapes(&out.shapes, t).map(|f| f.rect).unwrap_or_else(|| panic!("{t:?} is drawn"));
    let (file, hidden) = (at(THE_FILE), at(HIDDEN_DETAILS));
    assert!(file.top() < hidden.top(), "The file, then Hidden details");
    let (badge, ending) = (at("Picture"), at(".jpg"));
    let name = text_starts_at(&out, "jpeg-phone.jpg (").expect("the name is drawn");
    assert!(badge.right() < name && ending.right() < name && badge.top() < hidden.top(), "the photo's type, before its name");
    assert!(painted(&out, &theme, Tint::Blue) && painted(&out, &theme, Tint::Green), "each part in its colour");
}

/// A FILE NAMED LIKE A PICTURE THAT IS A PROGRAM IS RED BEFORE IT GOES (2026-10-17, as the website's
/// attach step does): "Send a file" reads its first bytes for the badge, which says Program, red,
/// not Picture. Seen red 2026-10-17 with the badge drawn from the name alone: "\"Program\" is drawn".
#[test]
fn a_file_named_like_a_picture_that_is_a_program_is_red_before_it_goes() {
    let mut gs = app();
    let fake = humanity_core::test_temp::file("attach_step_disguised", "jpg");
    std::fs::write(&fake, [0x4d, 0x5a, 0x90, 0x00, 0x03, 0x00, 0x00, 0x00, 0x04, 0x00]).unwrap();
    let job = attach_send::attach_file_job(&mut gs, &fake).expect("a job");
    open(&mut gs, job, &fake);
    frames_until_looked(&mut gs);
    assert!(gs.files_flow.step.is_some(), "a picture that cannot be read as one waits for Send anyway");
    let ctx = egui::Context::default();
    ctx.all_styles_mut(|s| s.animation_time = 0.0);
    let theme = crate::gui::theme::load_theme();
    let mut out = ctx.run(egui::RawInput::default(), |ctx| draw(ctx, &theme, &mut gs));
    for _ in 0..3 {
        out = ctx.run(egui::RawInput::default(), |ctx| draw(ctx, &theme, &mut gs));
    }
    let program = crate::gui::widgets::tint_section::texts_at(&out, "Program");
    assert!(!program.is_empty(), "\"Program\" is drawn");
    assert!(crate::gui::widgets::tint_section::texts_at(&out, "Picture").is_empty(), "not Picture");
}
