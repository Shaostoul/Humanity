//! The chat footer's tests, drawn for real under a headless context in lib.rs's order: the bottom
//! strip (the warning, then the footer), then a page standing in the room left.

use super::*;
use crate::gui::bottom_strip;
use crate::gui::ChatMessage;

const SCREEN: egui::Vec2 = egui::vec2(1280.0, 720.0);
/// A fixed "now" for the strip (seconds since 1970), so a mute's end is where the test puts it.
const NOW: i64 = 1_800_000_000;

fn ctx_theme() -> (egui::Context, Theme) {
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    ctx.all_styles_mut(|s| s.animation_time = 0.0);
    (ctx, theme)
}

fn line(name: &str, words: &str, ts: u64) -> ChatMessage {
    ChatMessage { sender_name: name.into(), sender_key: format!("key-{name}"), content: words.into(), timestamp_ms: ts, channel: "global".into(), ..Default::default() }
}

/// Signed in to a server, on `page`, with the footer in `mode` and two lines in #global.
fn state_on(page: GuiPage, mode: Mode) -> GuiState {
    crate::config::keep_saves_off_disk(); // folding keeps the choice: never in the real config.json
    let mut s = GuiState::default();
    s.onboarding_complete = true;
    s.active_page = page;
    s.connected_server_url = "https://chat.example.test".into();
    s.chat_active_channel = "global".into();
    s.chat_footer.mode = mode;
    s.keymaps = crate::gui::pages::keymap::load_keymaps(&std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data"));
    s.chat_messages = vec![line("Ada", "the seeds are in", 1_000), line("Grace", "the tomatoes are ripe", 2_000)];
    s
}

/// What a frame left: where the page's room ends, the strip's height, the frame's shapes.
struct Frame {
    page_bottom: f32,
    strip_h: f32,
    out: egui::FullOutput,
}

fn frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, events: Vec<egui::Event>) -> Frame {
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN)), events, ..Default::default() };
    let (mut page_bottom, mut strip_h) = (0.0, 0.0);
    let out = ctx.run(input, |ctx| {
        bottom_strip::draw_at(ctx, theme, state, "0.0.0-test", NOW);
        page_bottom = ctx.available_rect().bottom();
        strip_h = bottom_strip::height(ctx);
        egui::CentralPanel::default().show(ctx, |ui| ui.label("the page"));
    });
    Frame { page_bottom, strip_h, out }
}

/// A few frames, so the measured bars settle; the last one's.
fn settle(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) -> Frame {
    for _ in 0..4 {
        frame(ctx, theme, state, Vec::new());
    }
    frame(ctx, theme, state, Vec::new())
}

fn drawn_text(out: &egui::FullOutput, words: &str) -> Option<egui::Rect> {
    crate::gui::screen_surface::find_text_in_shapes(&out.shapes, words).map(|f| f.rect)
}

/// The warning's height alone on a page: the footer off.
fn warning_only(ctx: &egui::Context, theme: &Theme) -> f32 {
    let mut s = state_on(GuiPage::Humanity, Mode::Off);
    settle(ctx, theme, &mut s).strip_h
}

/// DRAWN ON A PAGE, NOT ON CHAT, NOT IN THE WORLD, NOT SIGNED OUT: folded, the footer takes room
/// above the warning on the Library and shows its line; on the Chat page, the title screen and
/// signed out it takes none (the strip is the warning alone), and in the world there is no strip.
///
/// Seen red 2026-11-15 with `bottom_strip::draw_at` not calling the footer: "folded, the footer took no
/// room on the Library: strip 24 against the warning's 24".
#[test]
fn the_footer_is_drawn_on_a_page_and_not_on_chat_or_in_the_world() {
    let (ctx, theme) = ctx_theme();
    let warning = warning_only(&ctx, &theme);
    assert!(warning > 10.0, "the warning drew ({warning})");

    let mut lib = state_on(GuiPage::Library, Mode::Folded);
    let f = settle(&ctx, &theme, &mut lib);
    assert!(f.strip_h > warning + 10.0, "folded, the footer took no room on the Library: strip {} against the warning's {warning}", f.strip_h);
    assert!((SCREEN.y - f.page_bottom - f.strip_h).abs() < 0.5, "the page's room ends where the strip begins");
    assert!(drawn_text(&f.out, t("chat_footer.open_in_chat")).is_some(), "the footer's line is drawn");

    for page in [GuiPage::Chat, GuiPage::MainMenu] {
        let (ctx, theme) = ctx_theme();
        let mut s = state_on(page, Mode::Folded);
        let f = settle(&ctx, &theme, &mut s);
        assert!((f.strip_h - warning).abs() < 0.5, "on {page:?} the strip is the warning alone ({} against {warning})", f.strip_h);
        assert!(drawn_text(&f.out, t("chat_footer.open_in_chat")).is_none(), "no footer on {page:?}");
    }
    let (ctx, theme) = ctx_theme();
    let mut world = state_on(GuiPage::None, Mode::Unfolded);
    assert_eq!(settle(&ctx, &theme, &mut world).strip_h, 0.0, "nothing at the bottom of the first-person world");

    let (ctx, theme) = ctx_theme();
    let mut signed_out = state_on(GuiPage::Library, Mode::Folded);
    signed_out.connected_server_url.clear();
    assert!((settle(&ctx, &theme, &mut signed_out).strip_h - warning).abs() < 0.5, "not signed in to a server: no footer");
}

/// FOLDED IS ONE LINE: the warning's height, give or take its own edge, with the newest message
/// as "name: words" on it.
///
/// Seen red 2026-11-15 with `bottom_strip::draw_at` not calling the footer: "folded is one line: 0 against the
/// warning's 24". (With the line's parts stacked in a `ui.vertical`, the folded footer measured
/// about 68 points, nearly three lines.)
#[test]
fn folded_is_one_line_with_the_newest_message() {
    let (ctx, theme) = ctx_theme();
    let warning = warning_only(&ctx, &theme);
    let mut s = state_on(GuiPage::Inventory, Mode::Folded);
    let f = settle(&ctx, &theme, &mut s);
    let footer = f.strip_h - warning;
    assert!((footer - warning).abs() <= 4.0, "folded is one line: {footer} against the warning's {warning}");
    let newest = tf("chat_footer.newest", &[("name", &"Grace"), ("words", &"the tomatoes are ripe")]);
    assert!(drawn_text(&f.out, &newest).is_some(), "the newest message is on the line as {newest:?}");
}

/// UNFOLDED, THE BOX IS WHOLLY ON SCREEN ABOVE THE WARNING, and with the warning muted (the strip
/// then stands on the window's bottom): the composer's box lies inside the footer, which ends
/// where the warning begins, and the strip is about its default height.
///
/// Seen red 2026-11-15 three ways: with `bottom_strip::draw_at` not calling the footer, "unfolded at 180: the strip is
/// 24 with the warning's 24"; with the line laid out right to left in the whole strip (it centred
/// itself in it and pushed the messages and the box out), "the strip is 382"; and with egui's own
/// resizable panel and its rect read as the height (the bar's last two pixels grew it each frame),
/// "the strip is 206.03125 with the warning's 24". The bar's tools: seen red with the footer's bar
/// given the Chat page's search, pins, help and Attach too, "the footer's bar has Send (true) and
/// not the Chat page's Attach (true)".
#[test]
fn unfolded_the_box_is_on_screen_above_the_warning_and_with_it_muted() {
    for muted in [false, true] {
        for height in [DEFAULT_HEIGHT, MIN_HEIGHT] {
            let (ctx, theme) = ctx_theme();
            let warning = if muted { 0.0 } else { warning_only(&ctx, &theme) };
            let mut s = state_on(GuiPage::Library, Mode::Unfolded);
            s.chat_footer.height = height;
            if muted {
                s.wip_mute = Some(bottom_strip::WipMute { until: Some(NOW + 3_600), version: "0.0.0-test".into() });
            }
            let f = settle(&ctx, &theme, &mut s);
            let footer_bottom = SCREEN.y - warning;
            let footer_top = SCREEN.y - f.strip_h;
            assert!((f.strip_h - warning - height).abs() < 2.0, "unfolded at {height}: the strip is {} with the warning's {warning}", f.strip_h);
            let r = ctx.read_response(egui::Id::new(composer::INPUT_ID)).expect("the composer is drawn").rect;
            assert!(r.bottom() <= footer_bottom + 0.5 && r.top() >= footer_top, "the box {r:?} runs outside the footer ({footer_top}..{footer_bottom}, muted {muted}, height {height})");
            assert!(r.height() >= 16.0, "the box is a box ({r:?})");
            // Send, and none of the Chat page's own tools, whose panels are the Chat page's.
            let send = drawn_text(&f.out, t("chat.send")).is_some();
            let attach = drawn_text(&f.out, t("chat.attach")).is_some();
            assert!(send && !attach, "the footer's bar has Send ({send}) and not the Chat page's Attach ({attach})");
        }
    }
}

/// A MESSAGE TYPED IN THE FOOTER GOES THROUGH THE CHAT PAGE'S SEND PATH INTO THE SAME CHANNEL:
/// typed into the footer's box and sent with Enter, the line leaves on the socket for #global,
/// its copy is in the conversation reading "Sending..." until the server's comes back (the line
/// limit's follow-up, only `send_composed_content` does that), and the draft is empty, on the
/// Chat page too (one `chat_input`).
///
/// Seen red 2026-11-15 with `bottom_strip::draw_at` not calling the footer: "the composer is drawn"; and with the
/// line laid out in the whole strip (the box pushed off the window), "the footer's box writes the
/// Chat page's draft" (left: "").
#[test]
fn a_line_typed_in_the_footer_goes_out_through_the_chat_pages_send_path() {
    let (ctx, theme) = ctx_theme();
    let mut s = state_on(GuiPage::Library, Mode::Unfolded);
    let (client, sent) = crate::net::ws_client::WsClient::recording();
    s.ws_client = Some(client);
    settle(&ctx, &theme, &mut s);
    let box_at = ctx.read_response(egui::Id::new(composer::INPUT_ID)).expect("the composer is drawn").rect.center();
    let m = egui::Modifiers::default();
    frame(&ctx, &theme, &mut s, vec![egui::Event::PointerMoved(box_at)]);
    frame(&ctx, &theme, &mut s, vec![egui::Event::PointerButton { pos: box_at, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
    frame(&ctx, &theme, &mut s, vec![egui::Event::PointerButton { pos: box_at, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
    frame(&ctx, &theme, &mut s, vec![egui::Event::Text("hello from the Library".into())]);
    assert_eq!(s.chat_input, "hello from the Library", "the footer's box writes the Chat page's draft");
    let enter = egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: m };
    frame(&ctx, &theme, &mut s, vec![enter]);
    let frames: Vec<String> = sent.try_iter().collect();
    let line = frames.iter().find(|f| f.contains("hello from the Library")).unwrap_or_else(|| panic!("the line left on the socket: {frames:?}"));
    let v: serde_json::Value = serde_json::from_str(line).expect("a JSON frame");
    assert_eq!(v["channel"], "global", "into the channel the Chat page has open: {line}");
    let copy = s.chat_messages.iter().find(|m| m.content == "hello from the Library").expect("its copy is in the conversation");
    assert_eq!(copy.channel, "global");
    assert_eq!(copy.send_state, crate::gui::pages::chat::line_limit::LineSend::Sending, "followed until the server's copy comes back");
    assert!(s.chat_input.is_empty(), "and the draft is empty, here and on the Chat page");
}

/// THE SAME DRAFT IN BOTH VIEWS: a sentence started on the Chat page is in the footer's box on the
/// Library, and is still there back on the Chat page.
///
/// Seen red 2026-11-15 with `bottom_strip::draw_at` not calling the footer, and with the line laid out in the whole
/// strip (the box pushed off the window): "the Chat page's draft is in the footer's box".
#[test]
fn the_draft_is_the_same_on_the_chat_page_and_in_the_footer() {
    let (ctx, theme) = ctx_theme();
    let mut s = state_on(GuiPage::Library, Mode::Unfolded);
    s.chat_input = "half a sentence".into();
    let f = settle(&ctx, &theme, &mut s);
    assert!(drawn_text(&f.out, "half a sentence").is_some(), "the Chat page's draft is in the footer's box");
    s.go(GuiPage::Chat, crate::gui::How::From);
    assert_eq!(s.chat_input, "half a sentence");
}

/// OFF DRAWS NOTHING AND TAKES NO ROOM, also with the warning muted (the window's whole height is
/// the page's then); the key does not bring it back from Off.
///
/// Seen red 2026-11-15 with `shows` not asking about the mode and Off drawing the folded line:
/// "Off drew the footer".
#[test]
fn off_draws_nothing_and_takes_no_room() {
    let (ctx, theme) = ctx_theme();
    let warning = warning_only(&ctx, &theme);
    let mut s = state_on(GuiPage::Library, Mode::Off);
    let f = settle(&ctx, &theme, &mut s);
    assert!((f.strip_h - warning).abs() < 0.5, "Off took room: {} against the warning's {warning}", f.strip_h);
    assert!(drawn_text(&f.out, t("chat_footer.open_in_chat")).is_none(), "Off drew the footer");

    let (ctx, theme) = ctx_theme();
    let mut muted = state_on(GuiPage::Library, Mode::Off);
    muted.wip_mute = Some(bottom_strip::WipMute { until: Some(NOW + 3_600), version: "0.0.0-test".into() });
    let f = settle(&ctx, &theme, &mut muted);
    assert_eq!(f.strip_h, 0.0, "muted and Off: nothing at the bottom");
    assert_eq!(f.page_bottom, SCREEN.y, "the page has the whole window");
    let alt_c = egui::Event::Key { key: egui::Key::C, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::ALT };
    frame(&ctx, &theme, &mut muted, vec![alt_c]);
    assert_eq!(muted.chat_footer.mode, Mode::Off, "the key folds and unfolds; it does not turn the footer on");
}

/// THE KEY FROM THE KEYMAP FOLDS AND UNFOLDS IT, and Show more and Show less do the same.
///
/// Seen red 2026-11-15 with `bottom_strip::draw_at` not calling the footer and the keymap row's id removed: "Alt+C
/// unfolds it" (left: Folded).
#[test]
fn the_keymaps_key_and_the_links_fold_and_unfold_it() {
    let (ctx, theme) = ctx_theme();
    let mut s = state_on(GuiPage::Library, Mode::Folded);
    settle(&ctx, &theme, &mut s);
    let alt_c = || egui::Event::Key { key: egui::Key::C, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::ALT };
    frame(&ctx, &theme, &mut s, vec![alt_c()]);
    assert_eq!(s.chat_footer.mode, Mode::Unfolded, "Alt+C unfolds it");
    frame(&ctx, &theme, &mut s, vec![alt_c()]);
    assert_eq!(s.chat_footer.mode, Mode::Folded, "and folds it again");

    let f = settle(&ctx, &theme, &mut s);
    let at = drawn_text(&f.out, t("chat_footer.unfold")).expect("Show more is on the folded line").center();
    let m = egui::Modifiers::default();
    frame(&ctx, &theme, &mut s, vec![egui::Event::PointerMoved(at)]);
    frame(&ctx, &theme, &mut s, vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
    frame(&ctx, &theme, &mut s, vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
    assert_eq!(s.chat_footer.mode, Mode::Unfolded, "Show more unfolds it");
}

/// THE TOP EDGE RESIZES IT, WITHIN ITS RANGE, AND THE HEIGHT IS KEPT: dragged up 60 points the
/// strip is 60 taller and the setting says so; dragged far up it stops at 40% of the window, far
/// down at its lowest.
///
/// Seen red 2026-11-15 with `bottom_strip::draw_at` not calling the footer: "dragged up 60: 0"; and with the
/// panel's own rect read as its height: "dragged up 60: 242.03125".
#[test]
fn the_top_edge_resizes_it_within_its_range() {
    let (ctx, theme) = ctx_theme();
    let mut s = state_on(GuiPage::Library, Mode::Unfolded);
    let warning = warning_only(&ctx, &theme);
    let drag = |s: &mut GuiState, dy: f32| {
        let top = SCREEN.y - settle(&ctx, &theme, s).strip_h;
        let at = egui::pos2(SCREEN.x / 3.0, top + 1.0);
        let m = egui::Modifiers::default();
        frame(&ctx, &theme, s, vec![egui::Event::PointerMoved(at)]);
        frame(&ctx, &theme, s, vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
        let to = at + egui::vec2(0.0, dy);
        frame(&ctx, &theme, s, vec![egui::Event::PointerMoved(at + egui::vec2(0.0, dy / 2.0))]);
        frame(&ctx, &theme, s, vec![egui::Event::PointerMoved(to)]);
        frame(&ctx, &theme, s, vec![egui::Event::PointerButton { pos: to, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
        settle(&ctx, &theme, s).strip_h - warning
    };
    let h = drag(&mut s, -60.0);
    assert!((h - (DEFAULT_HEIGHT + 60.0)).abs() < 1.0, "dragged up 60: {h}");
    assert!((s.chat_footer.height - h).abs() < 1.0, "and kept: {}", s.chat_footer.height);
    let h = drag(&mut s, -400.0);
    assert!((h - SCREEN.y * MAX_SHARE).abs() < 1.0, "no taller than 40% of the window: {h}");
    let h = drag(&mut s, 400.0);
    assert!((h - MIN_HEIGHT).abs() < 1.0, "no lower than its lowest: {h}");
}

/// WINDOWS STAND ON THE FOOTER: `bottom_strip::height` is the warning and the footer together, so
/// the toasts and the dialogs (and the chat videos' dock, which stands on `room_above`) are kept
/// above the footer, not only above the warning.
///
/// Seen red 2026-11-15 with `bottom_strip::draw_at` not calling the footer: "the room above the strip leaves out
/// the footer: [[0.0 0.0] - [1280.0 696.0]]".
#[test]
fn windows_stand_on_the_footer() {
    let (ctx, theme) = ctx_theme();
    let mut s = state_on(GuiPage::Library, Mode::Unfolded);
    s.pending_toasts.push(("Saved".into(), crate::gui::ToastKind::Info));
    let mut room = egui::Rect::NOTHING;
    for _ in 0..4 {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN)), ..Default::default() };
        let _ = ctx.run(input, |ctx| {
            bottom_strip::draw_at(ctx, &theme, &mut s, "0.0.0-test", NOW);
            room = bottom_strip::room_above(ctx);
            crate::gui::widgets::draw_toasts(ctx, &theme, &mut s);
        });
    }
    let toast = ctx.memory(|m| m.area_rect(egui::Id::new(("hos_toast", 0usize)))).expect("the toast drew");
    assert!(room.bottom() < SCREEN.y - DEFAULT_HEIGHT, "the room above the strip leaves out the footer: {room:?}");
    assert!(toast.bottom() <= room.bottom() - 47.5, "a toast stands on the footer: {toast:?}, the strip's top at {}", room.bottom());
}

/// THE UNREAD COUNT IS THE CHAT PAGE'S OWN MARKS: two rooms and a DM marked here, and a room on
/// another signed-in server; the active server's own entry in the saved list is not counted twice.
/// Seen red 2026-11-15 with every saved server counted, the active one's entry too: 5, not 4.
#[test]
fn the_unread_count_reads_the_chat_pages_marks() {
    let mut s = state_on(GuiPage::Library, Mode::Folded);
    assert_eq!(unread_count(&s), 0);
    let room = |id: &str, unread: bool| crate::gui::ChatChannel { id: id.into(), name: id.into(), unread, ..Default::default() };
    s.chat_channels = vec![room("global", false), room("garden", true), room("tools", true)];
    s.chat_dms = vec![crate::gui::ChatDm { user_name: "Ada".into(), user_key: "k".into(), last_message: String::new(), timestamp: String::new(), unread: true }];
    let mut other = crate::gui::ServerConnection::default();
    other.url = "https://other.example.test".into();
    other.channels = vec![room("news", true)];
    let mut here = crate::gui::ServerConnection::default();
    here.url = s.connected_server_url.clone();
    here.channels = vec![room("garden", true)];
    s.connections = vec![other, here];
    assert_eq!(unread_count(&s), 4);
}

/// config.json's record is read leniently: a mode this build does not know is the default and never
/// fails the whole file; a known one and a height come back as written. Seen red 2026-11-15 with
/// the record read strictly: "the file still loads: unknown variant `sideways`".
#[test]
fn the_kept_setting_reads_leniently() {
    #[derive(serde::Deserialize)]
    struct Holder {
        #[serde(default, deserialize_with = "read_kept")]
        f: ChatFooter,
    }
    let read = |json: &str| serde_json::from_str::<Holder>(json).expect("the file still loads").f;
    assert_eq!(read(r#"{"f":{"mode":"unfolded","height":240}}"#), ChatFooter { mode: Mode::Unfolded, height: 240.0 });
    assert_eq!(read(r#"{"f":{"mode":"sideways","height":240}}"#), ChatFooter::default());
    assert_eq!(read(r#"{"f":"nonsense"}"#), ChatFooter::default());
    assert_eq!(read(r#"{}"#), ChatFooter::default());
    assert_eq!(serde_json::to_string(&ChatFooter::default()).unwrap(), r#"{"mode":"folded","height":220.0}"#);
}
