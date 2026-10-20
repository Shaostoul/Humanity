// Child of media/chat_card.rs (#[path]): a Media item posted in chat, drawn as a card, headless and
// with no server reached (the item is put in place as if the server had answered; the one address
// the app could try is a closed loopback port). Each test was seen failing once on purpose,
// recorded at the test.

use super::*;
use crate::gui::pages::media::item_view::item_tests::rich_item;

/// The server every test message came through: a loopback port nothing listens on.
const SERVER: &str = "http://127.0.0.1:9";

/// Every text one frame drew.
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

/// A MEDIA ITEM'S ADDRESS ON THE MESSAGE'S OWN SERVER IS A CARD; ANY OTHER /media/ LINK STAYS A
/// LINK: the full address and the bare path both count, once each, punctuation after it left off;
/// another website's /media/ page and anything not a number do not; the card's addresses come out
/// of the message's words. Seen red 2026-10-17 with every host taken as the message's own:
/// "another website's /media/ page stays a link"; and with the scheme matched in lower case only:
/// "a phone's capital H" (left: []).
#[test]
fn an_items_address_on_its_own_server_is_a_card() {
    let text = "Look: http://127.0.0.1:9/media/42, and /media/7 again /media/42 https://other.example/media/9 /media/x /media/";
    assert_eq!(item_links(text, SERVER), vec![42, 7], "another website's /media/ page stays a link");
    assert_eq!(without_links(text, SERVER, &[42, 7]), "Look: , and again https://other.example/media/9 /media/x /media/");
    assert_eq!(without_links("https://h/media/7", SERVER, &[]), "https://h/media/7", "nothing taken out when there is no card");
    assert_eq!(item_links("Https://127.0.0.1:9/media/8", SERVER), vec![8], "a phone's capital H");
}

/// ONLY A WHOLE ADDRESS IS TAKEN OUT OF THE WORDS (a review, 2026-10-17): an address inside other
/// letters stays (taking `/media/5` out of `htt/media/5ps://evil.example/login` made a live link
/// of the rest, and a stranger's DM got it past the link hold); `/media/55` keeps its digits when
/// only 5 is a card; another website's page with the same number stays; a line without an address
/// keeps its spacing, so a code block keeps its indentation. Seen red 2026-10-17 with the plain
/// replacement back: "the glued token stays as it was" (left: "Look https://evil.example/login").
#[test]
fn only_a_whole_address_is_taken_out_of_the_words() {
    assert_eq!(without_links("Look /media/5 htt/media/5ps://evil.example/login", SERVER, &[5]), "Look htt/media/5ps://evil.example/login", "the glued token stays as it was");
    assert_eq!(without_links("see /media/5 and /media/55", SERVER, &[5]), "see and /media/55");
    assert_eq!(without_links("see /media/5 and /media/55", SERVER, &[5, 55]), "see and");
    assert_eq!(without_links("own /media/5 mirror https://other.site/media/5", SERVER, &[5]), "own mirror https://other.site/media/5", "another website's page stays");
    let code = "Try this:\n```\n    let x = 1;\n        go(x);\n```\n\nhttp://127.0.0.1:9/media/7";
    assert_eq!(without_links(code, SERVER, &[7]), "Try this:\n```\n    let x = 1;\n        go(x);\n```\n", "a code block keeps its indentation, a blank line stays");
    assert_eq!(without_links("  indented /media/7 words  here", SERVER, &[7]), "  indented words here");
}

/// A STRANGER'S DM KEEPS ITS MEDIA LINK HELD (a review, 2026-10-17: the two apps disagreed): while
/// the DM's links are held, no card and the address stays in the words; once the person presses
/// the link line's Open, the card comes. Seen red 2026-10-17 with the hold left out of
/// `Posted::of`: "no card while the link is held: [7]".
#[test]
fn a_strangers_dm_keeps_its_media_link_held_until_opened() {
    let mut state = with_item();
    state.profile_public_key = "me".into();
    let msg = crate::gui::ChatMessage { channel: "dm:zed".into(), sender_key: "zed".into(), content: "look /media/7".into(), ..Default::default() };
    let posted = crate::gui::pages::chat::public_files::Posted::of(&state, &msg, false);
    assert!(posted.items.is_empty(), "no card while the link is held: {:?}", posted.items);
    assert_eq!(posted.words_of(&msg.content), "look /media/7", "the address stays, to be held");
    state.warnings.links_opened.insert(crate::engine::warnings::message_key(&msg));
    let posted = crate::gui::pages::chat::public_files::Posted::of(&state, &msg, false);
    assert_eq!(posted.items, vec![7], "opened: its card");
}

/// A LABELLED CARD SAYS THE LABEL'S NAME BEFORE THE MEDIA PAGE IS OPENED (a review, 2026-10-17):
/// the card's own answer carries the server's label names, so a labelled picture asks "Labelled:
/// Flashing lights.", never the label's id. Seen red 2026-10-17 with the names left out of
/// `on_answer`: "the label's name" (it said "Labelled: flashing.").
#[test]
fn a_labelled_card_says_the_labels_name_before_the_media_page_is_opened() {
    let mut state = with_item();
    state.media.cards.clear();
    assert!(state.media.list.labels.is_empty(), "test setup: the Files page has not answered");
    let answer = serde_json::json!({
        "items": [{"id": 7, "title": "Dusk", "url": "/uploads/7_dusk.png", "filename": "7_dusk.png", "kind": "picture", "visibility": "public", "uploader_key": "ann", "uploader_name": "Ann", "labels": ["flashing"]}],
        "labels": [{"id": "flashing", "label": "Flashing lights", "desc": "Fast flashing or strobing light"}]
    });
    on_answer(&mut state, 7, Ok(answer));
    state.media.cards_open.insert(key_of("ann", 1000, 7));
    let (views, _) = views(&state, "ann", 1000, &[7]);
    assert!(matches!(&views[0].picture, Picture::Ask { line, .. } if line == "Labelled: Flashing lights."), "the label's name: {:?}", views[0].picture);
}

/// A FILE'S NAME IS SHOWN AS IT IS AND SAVED WITH ITS ENDING (a review, 2026-10-17): the card and
/// the item page show the uploader's own name, only made plain (no direction marks), and the badge
/// judges that; a name past 80 letters is saved shortened before its ending, never through it.
/// Seen red 2026-10-17 with the 80-letter cut back: "a long name keeps its ending" (a name ending
/// "aaaa", no ".exe"); and with the shown name not made plain: "the real name, no direction marks"
/// (left: "Ann's poi (v2)\u{202e}piz.exe").
#[test]
fn a_files_name_is_shown_as_it_is_and_saved_with_its_ending() {
    use crate::gui::pages::media::open::{safe_name, shown_file_name};
    let long = format!("{}.exe", "a".repeat(90));
    let saved = safe_name(&long);
    assert!(saved.ends_with(".exe") && saved.chars().count() <= 80, "a long name keeps its ending: {saved:?}");
    assert!(safe_name(&format!("{}.tar.gz", "b".repeat(90))).ends_with(".tar.gz"), "a tarball's two-part ending too");
    let mut item = rich_item(("ann", "Ann"));
    item.files[1].original_name = "Ann's poi (v2)\u{202E}piz.exe".into();
    assert_eq!(shown_file_name(&item, &item.files[1]), "Ann's poi (v2)piz.exe", "the real name, no direction marks");
    assert_eq!(crate::net::file_risk::file_type(&shown_file_name(&item, &item.files[1]), None).id, "program", "and its badge says what it is");
    let mut item = rich_item(("ann", "Ann"));
    assert_eq!(count_line(&item), "2 files, 2 MB");
    item.file_count = 1;
    item.total_bytes = 0;
    assert_eq!(count_line(&item), "1 file, 2 MB");
}

/// A state whose Media page is for the test's server, holding item 7 (Ann's) as if it had come.
fn with_item() -> GuiState {
    let mut state = GuiState::default();
    state.server_url = SERVER.into();
    state.connected_server_url = SERVER.into();
    crate::gui::pages::media::sync_server(&mut state);
    state.media.cards.insert(7, CardState::Ready(rich_item(("ann", "Ann"))));
    state
}

/// One frame of the cards of a message from Ann (sent at 1000) linking item 7.
fn card_frame(ctx: &egui::Context, state: &mut GuiState) -> egui::FullOutput {
    let theme = crate::gui::theme::load_theme();
    let (views, needs) = views(state, "ann", 1000, &[7]);
    let mut acts = needs;
    let out = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| draw_all(ui, &theme, &views, theme.bg_primary(), &mut acts));
    });
    state.media.card_acts.extend(acts);
    out
}

/// THE CARD STARTS AS ONE LINE AND OPENS OUT TO EVERYTHING (operator, 2026-10-17: "Card could start
/// as a collapsed line ... switchable between expanded and collapsed"): collapsed, its type, its
/// title, "2 files, 2 MB" and Open, not its description or files; opened out, who put it there,
/// its description and every file with its type; closed again, one line. Seen red 2026-10-17 with
/// the toggle ignored: "opened out: who and what".
#[test]
fn the_card_starts_as_one_line_and_opens_out_to_everything() {
    let ctx = egui::Context::default();
    let mut state = with_item();
    let t = texts(&card_frame(&ctx, &mut state));
    assert!(t.contains("Dusk") && t.contains("2 files, 2 MB") && t.contains(OPEN) && t.contains("3D model"), "one line: {t}");
    assert!(!t.contains("textures.zip") && !t.contains("First line of it"), "collapsed: no files, no description: {t}");
    assert!(state.media.card_acts.is_empty(), "nothing to ask: the item is in");

    state.media.card_acts.push(Act::Toggle(key_of("ann", 1000, 7)));
    apply(&mut state);
    let t = texts(&card_frame(&ctx, &mut state));
    assert!(t.contains("by Ann in Fire flow arts") && t.contains("First line of it"), "opened out: who and what: {t}");
    assert!(t.contains("dusk.glb, 2 MB") && t.contains("textures.zip, 4 KB") && t.contains("Archive"), "opened out: every file, with its type: {t}");

    state.media.card_acts.push(Act::Toggle(key_of("ann", 1000, 7)));
    apply(&mut state);
    assert!(!texts(&card_frame(&ctx, &mut state)).contains("textures.zip"), "closed again: one line");
}

/// OPEN BRINGS UP THE ITEM'S FULL WINDOW, OVER THE CHAT (operator: "when someone clicks on the file
/// in chat it brings up the full popup window for that item"): an item not in yet is asked for
/// once; Open makes it the open item, and the window draws on the chat page with its files and a
/// Download each. Seen red 2026-10-17 with the item window drawn only on the Media page: "the
/// item's window, over the chat".
#[test]
fn open_brings_up_the_items_full_window_over_the_chat() {
    let mut state = with_item();
    state.media.cards.remove(&7);
    let (_, needs) = views(&state, "ann", 1000, &[7]);
    assert_eq!(needs, vec![Act::Fetch(7)], "an item not in yet is asked for");
    let mut state = with_item();
    state.active_page = crate::gui::GuiPage::Chat;
    state.media.card_acts.push(Act::Open(7));
    apply(&mut state);
    assert_eq!(state.media.item.as_ref().map(|p| p.id), Some(7), "Open makes it the open item");
    let ctx = egui::Context::default();
    let theme = crate::gui::theme::load_theme();
    let mut t = String::new();
    for _ in 0..3 {
        let out = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 1000.0))), ..Default::default() }, |ctx| {
            crate::gui::pages::media::draw_item_window(ctx, &theme, &mut state);
        });
        t = texts(&out);
    }
    assert!(t.contains("Dusk") && t.contains("textures.zip") && t.matches("Download").count() >= 2, "the item's window, over the chat: {t}");
}

/// A MESSAGE HOLDING AN ITEM'S ADDRESS SHOWS ITS CARD IN THE CHAT, NOT THE BARE ADDRESS: the chat
/// page draws the card's title and count, and the address itself is not drawn. Seen red 2026-10-17
/// with the chat not asking for cards: "the card's title is drawn".
#[test]
fn a_message_holding_an_items_address_shows_its_card() {
    let ctx = egui::Context::default();
    let theme = crate::gui::theme::load_theme();
    let mut state = with_item();
    state.chat_active_channel = "general".into();
    state.chat_messages.push(crate::gui::ChatMessage {
        sender_name: "Ann".into(),
        sender_key: "ann".into(),
        content: "http://127.0.0.1:9/media/7".into(),
        timestamp: "09:20".into(),
        timestamp_ms: 1000,
        channel: "general".into(),
        server: SERVER.into(),
        ..Default::default()
    });
    let mut t = String::new();
    for _ in 0..3 {
        let out = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))), ..Default::default() }, |ctx| {
            crate::gui::pages::chat::draw(ctx, &theme, &mut state);
        });
        t = texts(&out);
    }
    assert!(t.contains("Dusk") && t.contains("2 files, 2 MB"), "the card's title is drawn: {t}");
    assert!(!t.contains("/media/7"), "the bare address is not drawn");
}

/// THE PROTECTED SETUP LEAVES OUT A PICTURE LINKED BY SOMEONE WHO IS NOT A FRIEND, WHOEVER PUT THE
/// ITEM THERE: with the setup on, a friend's picture item linked by the friend shows under the
/// click-to-load rule, but the same item linked by a stranger shows the preset's hidden line and is
/// never fetched (the website's card holds the same rule). Seen red 2026-10-17 with the sender's
/// rule left out of `picture_of`: "a stranger's link to a friend's picture is left out: Loading"
/// (the friend's picture was being fetched for the stranger's link).
#[test]
fn a_strangers_link_to_a_friends_picture_is_left_out_by_the_protected_setup() {
    use crate::net::dm_store::{DmStore, SentPass};
    crate::config::keep_saves_off_disk();
    let seed = vec![171u8; 32];
    let me = "me-171".to_string();
    let mut state = with_item();
    state.profile_public_key = me.clone();
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let mut store = DmStore::load(&seed, &me, &format!("https://card-protected-{nanos}.example"));
    store.set_following("ann", true);
    store.set_follower("ann", true);
    store.record_pass_sent("ann", SentPass { serial: "a1".repeat(16), may: "invite,message,trade,voice_message".into() });
    state.dm_store = Some(store);
    assert!(crate::engine::protected::ensure_preset(&mut state), "the preset loads");
    let preset = state.protected.preset.clone().unwrap();
    let pin = crate::net::protected::PinVerifier::with_salt("7391", &[3u8; 16], 1_000);
    state.protected.setup = crate::net::protected::ProtectedSetup::applied(&preset, pin, &me, vec!["ann".into()]);
    let picture = Item { kind: "picture".into(), stored: "7_dusk.png".into(), url: "/uploads/7_dusk.png".into(), ..rich_item(("ann", "Ann")) };
    state.media.cards.insert(7, CardState::Ready(picture));
    state.media.cards_open.insert(key_of("ann", 1000, 7));
    state.media.cards_open.insert(key_of("zed", 1000, 7));

    let (from_ann, _) = views(&state, "ann", 1000, &[7]);
    assert!(!matches!(from_ann[0].picture, Picture::Hidden(_)), "the friend's own link follows the click-to-load rule: {:?}", from_ann[0].picture);
    let (from_zed, acts) = views(&state, "zed", 1000, &[7]);
    assert!(matches!(&from_zed[0].picture, Picture::Hidden(line) if *line == preset.picture_hidden_line), "a stranger's link to a friend's picture is left out: {:?}", from_zed[0].picture);
    assert!(!acts.iter().any(|a| matches!(a, Act::Picture(_))), "and never fetched");
    state.dm_store.as_ref().unwrap().remove_file_for_test();
}
