//! Tests of the Play picker (showroom.rs) for ship homes increment 6, lane 2B
//! (docs/design/ship-homes-increment-6-plan.md 3.9): the WHO hint, the new-home row's choice and
//! the connected server's card, each drawn headlessly the way the app draws the picker
//! (`showroom::draw`) and read back from what was DRAWN, as where_you_live_tests.rs reads the
//! widget. Each was seen failing once on purpose; the break and the failure text are in its
//! comment.

use super::*;
use humanity_systems::persistence::live_save;
use crate::gui::screen_surface::find_text_in_shapes;
use humanity_systems::persistence::SaveSlot;
use humanity_ship::starts::{HomeAnswer, Starts};
use std::path::Path;

// ── The data ─────────────────────────────────────────────────────────────────────────────────────

/// The repo's starts file, as the game reads it.
fn the_starts() -> Starts {
    Starts::parse(&std::fs::read(Path::new("data").join(humanity_ship::starts::STARTS_FILE)).expect("data/homes/starts.ron"))
        .expect("the starts file keeps its rules")
}

/// The picker as a first run opens it: no home of your own yet, the new character and the
/// new-home row selected (what `preselect` picks with no saves), the saves already scanned (so
/// the test never reads this computer's saves folder).
fn new_home_state() -> GuiState {
    let mut gs = GuiState::default();
    gs.showroom_mode = 0;
    gs.launcher_homes_loaded = true;
    gs.character_name = "Wanderer".into();
    gs.launcher_who = String::new();
    gs.launcher_where_kind = LauncherWhere::Home;
    gs.launcher_selected_world = String::new();
    gs
}

/// The picker connected to ship.example (a client that records what it sends), signed in with
/// an identity, its live row selected in Open Net, and that server's info already fetched (so
/// the test makes no network request).
fn server_state() -> (GuiState, std::sync::mpsc::Receiver<String>) {
    let mut gs = GuiState::default();
    gs.showroom_mode = 0;
    gs.launcher_homes_loaded = true;
    gs.character_name = "Wanderer".into();
    gs.server_url = "https://ship.example".into();
    let (ws, sent) = crate::net::ws_client::WsClient::recording();
    gs.ws_client = Some(ws);
    gs.ws_identified = true;
    gs.profile_public_key = "ab".repeat(32);
    gs.launcher_where_kind = LauncherWhere::Server;
    gs.launcher_selected_server = Some(CONNECTED_SERVER_ID.to_string());
    let cache_id = format!("{CONNECTED_SERVER_ID}:{}", gs.server_url);
    gs.server_info_cache.insert(cache_id, crate::gui::ServerInfo::default());
    (gs, sent)
}

/// The operator's picker as his screenshot of 2026-11-02 shows it (2000 x 1125): Shaostoul, with
/// one home of his own on this computer, connected to united-humanity.us (the live row, selected
/// in Open Net) with public.guide and his own Super Secret Invite Only Server saved beside it, the
/// server's details fetched as the live server gives them, and no home on that server yet.
/// `ws` is the client the picker sends through (a recording one in the tests). The server's
/// answer is `operator_answer`, set after the card's first frame has asked (as the app gets it).
pub(crate) fn make_operator(gs: &mut GuiState, ws: crate::net::ws_client::WsClient) {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    gs.showroom_mode = 0;
    gs.launcher_homes = vec![crate::gui::LauncherHome {
        world: "My Homestead".into(),
        character: "Shaostoul".into(),
        design: "homestead".into(),
        timestamp: now.saturating_sub(26 * 3600),
    }];
    gs.launcher_homes_loaded = true;
    gs.launcher_who = "My Homestead".into();
    gs.character_name = "Shaostoul".into();
    gs.server_url = "https://united-humanity.us".into();
    gs.ws_client = Some(ws);
    gs.ws_identified = true;
    gs.profile_public_key = "ab".repeat(32);
    let saved = |name: &str, url: &str| crate::gui::ChatServer {
        id: format!("srv_{url}"),
        name: name.into(),
        url: url.into(),
        connected: false,
        channels: Vec::new(),
        voice_channels: Vec::new(),
    };
    gs.chat_servers = vec![saved("public.guide", "https://public.guide"), saved("Super Secret Invite Only Server", "http://127.0.0.1:3210")];
    gs.launcher_where_kind = LauncherWhere::Server;
    gs.launcher_selected_server = Some(CONNECTED_SERVER_ID.to_string());
    gs.server_info_cache.insert(
        format!("{CONNECTED_SERVER_ID}:{}", gs.server_url),
        crate::gui::ServerInfo {
            name: "HumanityOS".into(),
            description: "Official server for HumanityOS ran by Michael Boisson aka Shaostoul.".into(),
            version: "eb16165-1793650829".into(),
            users_online: 1,
            game_players: 0,
            member_count: 46,
            accord_compliant: false,
            channels: vec!["general".into(), "announcements".into(), "dev".into()],
            ..Default::default()
        },
    );
}

/// The live server's answer in the operator's screenshot: no home there yet, 16 cabins and 10
/// homesteads free, the Apartment not offered.
pub(crate) fn operator_answer(starts: &Starts) -> HomeAnswer {
    let mut a = wyl::tests::guest_answer(starts);
    for o in a.starts.iter_mut() {
        o.free = match o.id.as_str() {
            "cabin" => 16,
            "homestead" => 10,
            _ => 0,
        };
    }
    a
}

/// Every message the recording client was handed since the last call, parsed.
fn sent(rx: &std::sync::mpsc::Receiver<String>) -> Vec<serde_json::Value> {
    rx.try_iter().map(|s| serde_json::from_str(&s).expect("a JSON frame")).collect()
}

// ── Drawing headlessly ───────────────────────────────────────────────────────────────────────────

fn ctx_and_theme() -> (egui::Context, Theme) {
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    (ctx, theme)
}

/// One frame of the picker on a tall screen, so every row of the WHERE column is on it.
fn picker(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, events: Vec<egui::Event>) -> egui::FullOutput {
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 3000.0))),
        events,
        ..Default::default()
    };
    ctx.run(raw, |ctx| draw(ctx, theme, state))
}

fn visit<'a>(shape: &'a egui::Shape, f: &mut dyn FnMut(&'a egui::epaint::TextShape)) {
    match shape {
        egui::Shape::Text(t) => f(t),
        egui::Shape::Vec(v) => v.iter().for_each(|s| visit(s, f)),
        _ => {}
    }
}

/// Every text drawn, whole (a wrapped label is one text).
fn texts(out: &egui::FullOutput) -> Vec<String> {
    let mut all = Vec::new();
    for cs in &out.shapes {
        visit(&cs.shape, &mut |t| all.push(t.galley.text().to_string()));
    }
    all
}

/// The colour `text` is drawn in (its first section's), when it is drawn.
fn color_of(out: &egui::FullOutput, text: &str) -> Option<egui::Color32> {
    let mut found = None;
    for cs in &out.shapes {
        visit(&cs.shape, &mut |t| {
            if found.is_none() && t.galley.text() == text {
                found = t.galley.job.sections.first().map(|s| s.format.color);
            }
        });
    }
    found
}

/// The texts of `want` that `out` does not draw exactly.
fn missing(out: &egui::FullOutput, want: &[String]) -> Vec<String> {
    let drawn = texts(out);
    want.iter().filter(|w| !drawn.iter().any(|d| d == *w)).cloned().collect()
}

/// The texts of `never` that `out` draws exactly.
fn present(out: &egui::FullOutput, never: &[String]) -> Vec<String> {
    let drawn = texts(out);
    never.iter().filter(|n| drawn.iter().any(|d| d == *n)).cloned().collect()
}

/// Click the text `label` where the picker draws it (exactly that text, its first time drawn):
/// the canonical three frames (move, press, release).
fn click(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, label: &str) {
    let out = picker(ctx, theme, state, Vec::new());
    let found = find_text_in_shapes(&out.shapes, label).unwrap_or_else(|| panic!("{label:?} is drawn: {:?}", texts(&out)));
    assert_eq!(found.text, label, "{label:?} is drawn as itself");
    let pos = found.rect.center();
    let m = egui::Modifiers::default();
    picker(ctx, theme, state, vec![egui::Event::PointerMoved(pos)]);
    picker(ctx, theme, state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
    picker(ctx, theme, state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
}

/// The Enter bar's sentence, as drawn.
fn enter_bar(out: &egui::FullOutput) -> String {
    texts(out).into_iter().find(|t| t.starts_with("Enter as ")).unwrap_or_default()
}

// ── The tests ────────────────────────────────────────────────────────────────────────────────────

/// THE PICKER'S HINT SAYS EACH SERVER KEEPS ITS OWN SAVE (plan 3.9, the stale words of section 2).
/// Since increment 6 your own home is saves/offline_home.json and each server you play on keeps
/// its own save for you, made fresh from the home you choose there; only your name, look and
/// outfit cross. The hint said the opposite ("Gear and skills are one save on this computer,
/// shared by your home and every server you visit"), which was true before this increment. The
/// fact it rests on: one identity's saves on two servers and your own home are three files, and
/// a server's is under saves/servers/, which the picker's scan (top level only) never lists as
/// a character.
///
/// Seen red 2026-10-12 with the old hint kept: "the picker still says one save holds gear and
/// skills for every server: Your name and look travel with you. Gear and skills are one save on
/// this computer, shared by your home and every server you visit."
#[test]
fn the_pickers_hint_says_each_server_keeps_its_own_save() {
    // The hint is drawn by key since 2026-11-02 (`characters.who_hint`).
    let who_hint = t("characters.who_hint");
    assert!(
        !who_hint.contains("one save on this computer"),
        "the picker still says one save holds gear and skills for every server: {who_hint}"
    );
    assert!(who_hint.contains("Your name and look travel with you."), "{who_hint}");
    assert!(who_hint.contains("Each server you visit keeps its own home and things for you, starting fresh"), "{who_hint}");
    assert!(who_hint.contains("your own home keeps yours"), "{who_hint}");
    let dir = Path::new("saves-anywhere");
    let me = "ab".repeat(32);
    let own = humanity_systems::persistence::slot_path(dir, &SaveSlot::Offline);
    let a = humanity_systems::persistence::slot_path(dir, &SaveSlot::Server { server: "https://a.example".into(), identity: me.clone() });
    let b = humanity_systems::persistence::slot_path(dir, &SaveSlot::Server { server: "https://b.example".into(), identity: me });
    assert_eq!(own.file_name().and_then(|n| n.to_str()), Some("offline_home.json"), "{}", own.display());
    assert!(a != b && a != own && b != own, "three worlds, three saves: {} {} {}", own.display(), a.display(), b.display());
    for server in [&a, &b] {
        assert_eq!(server.parent(), Some(dir.join("servers").as_path()), "a server's save is under saves/servers/: {}", server.display());
    }
}

/// THE NEW-HOME ROW OFFERS THE STARTS FROM DATA (plan 3.9, 2B). On a first run (no home of your own
/// yet) the picker's new-home row reads "A new home"; selected, it shows the choice: its title and
/// hint, then every start of data/homes/starts.ron in its order as a card, each with its first
/// sentence (the whole is in the card's fold since the 2026-11-02 relayout), the default
/// marked "Homestead (most people start here)", and the Apartment, not offered yet, GREYED (drawn
/// muted, with no radio) with why. The Enter bar names the home Enter makes: "Enter as Wanderer ->
/// a new Homestead (solo)" by default, "a new Cabin" once the Cabin is picked, and still the Cabin
/// after a click on the Apartment, which cannot be chosen. Enter remembers the choice for your own
/// home's save (`live_save::choice_for(Offline)`: a Cabin, just you, no place yet), which
/// `save_slot::follow` makes once the pairing is recorded. With a home of your own already saved,
/// the row offers no choice (there is one save for your own home), says a new character moves into
/// it, the Enter bar names that home, and Enter remembers nothing.
///
/// Seen red 2026-10-12 with the Apartment selectable (where_you_live.rs `draw_choice`'s greyed
/// branch skipped, `if false && !s.offered`, so every start got a radio): "the new-home row is
/// missing these", left: ["Apartments come with the first apartment block, in a later update."].
#[test]
fn the_new_home_row_offers_the_starts_from_data() {
    let starts = the_starts();
    let (ctx, theme) = ctx_and_theme();
    wyl::reset_draft();
    OWN_HOME.with(|o| *o.borrow_mut() = None);
    let mut gs = new_home_state();

    let out = picker(&ctx, &theme, &mut gs, Vec::new());
    let mut want = vec![t("characters.new_home").to_string(), t("where_you_live.title").to_string(), t("where_you_live.hint").to_string()];
    for s in &starts.starts {
        want.push(wyl::row_name(&starts, &s.id, &s.name));
        want.push(wyl::first_sentence(&s.sentence).to_string());
        if !s.offered {
            want.push(s.not_offered_why.clone());
        }
    }
    assert_eq!(missing(&out, &want), Vec::<String>::new(), "the new-home row is missing these");
    assert!(want.contains(&"Homestead (most people start here)".to_string()), "the default is marked: {want:?}");
    let apartment = starts.get("apartment").expect("the apartment row");
    assert!(!apartment.offered, "the data keeps the apartment for later");
    assert_eq!(color_of(&out, &apartment.name), Some(theme.text_muted()), "the Apartment row is drawn as a choice");
    assert_eq!(color_of(&out, "Cabin"), Some(theme.text_primary()), "the Cabin is a choice");
    assert_eq!(enter_bar(&out), "Enter as Wanderer -> a new Homestead (solo)");

    click(&ctx, &theme, &mut gs, "Cabin");
    assert_eq!(enter_bar(&picker(&ctx, &theme, &mut gs, Vec::new())), "Enter as Wanderer -> a new Cabin (solo)");
    click(&ctx, &theme, &mut gs, "Apartment");
    assert_eq!(enter_bar(&picker(&ctx, &theme, &mut gs, Vec::new())), "Enter as Wanderer -> a new Cabin (solo)", "the Apartment cannot be chosen");

    click(&ctx, &theme, &mut gs, "Enter");
    assert!(gs.showroom_confirm, "Enter commits the pairing");
    assert_eq!(
        live_save::choice_for(&SaveSlot::Offline),
        Some(live_save::Choice { start: "cabin".into(), household: "solo".into(), place: None }),
        "Enter remembers the new home for your own home's save"
    );

    // A home of your own already saved: no choice, and Enter remembers nothing.
    OWN_HOME.with(|o| *o.borrow_mut() = Some("My Homestead".into()));
    gs.showroom_confirm = false;
    let out = picker(&ctx, &theme, &mut gs, Vec::new());
    assert_eq!(missing(&out, &[t("characters.new_home_taken").to_string()]), Vec::<String>::new());
    assert_eq!(present(&out, &[t("where_you_live.title").to_string(), "Cabin".to_string()]), Vec::<String>::new(), "no choice over a home already saved");
    assert_eq!(enter_bar(&out), "Enter as Wanderer -> My Homestead (solo)");
    assert_eq!(remember_new_home(&gs), None);
    OWN_HOME.with(|o| *o.borrow_mut() = None);
}

/// A SERVER CARD SHOWS WHERE YOU LIVE OR THE CHOICE (plan 3.9, 2B). The card of the server the game
/// is connected to, in Open Net, as the picker draws it:
/// - opened with an answer kept from before (another server's, or an old one), it does not show it:
///   it asks this server (`game_home_request`, sent once) and says it is asking;
/// - the server says you live in its cabin c2: "Where you live here: a Cabin, c2." with the note
///   that a server starts you fresh, and NO choice (no title, no Choose, no free counts);
/// - the server says you hold none there: "You have no home on this server yet.", the choice with
///   "15 free" for the Cabin and "none free" for the Homestead, and Choose; Choose on the Cabin
///   sends `game_choose_home` for the cabin, and the card says it is asking until the reply;
/// - the reply that it was chosen: where you live there, and that it is chosen;
/// - with no identity, the card says a server keeps your home under one, and asks nothing.
///
/// Seen red 2026-10-12 with the choice shown while a home is held (where_you_live.rs
/// `draw_server`'s held branch skipped, `held_line(starts, a).filter(|_| false)`, so a held home
/// got the choice): "the held home's card", left: ["Where you live here: a Cabin, c2."].
#[test]
fn a_server_card_shows_where_you_live_or_the_choice() {
    let starts = the_starts();
    let (ctx, theme) = ctx_and_theme();
    wyl::reset_draft();
    HERE.with(|h| *h.borrow_mut() = HereCard::default());
    let (mut gs, rx) = server_state();
    let held = wyl::tests::held_answer(&starts);
    let guest = wyl::tests::guest_answer(&starts);
    let held_line = wyl::held_line(&starts, &held).expect("c2 is held");
    assert_eq!(held_line, "Where you live here: a Cabin, c2.");
    let choice_words = vec![t("where_you_live.title").to_string(), t("where_you_live.choose").to_string(), "15 free".to_string(), t("where_you_live.no_home_here").to_string()];

    // An answer from before the card asked is not shown; the card asks this server.
    gs.home_answer = Some(held.clone());
    let out = picker(&ctx, &theme, &mut gs, Vec::new());
    let asked = sent(&rx);
    assert_eq!(asked, vec![serde_json::json!({"type": "game_home_request"})], "the card asks this server once");
    assert!(gs.home_answer.is_none(), "the old answer is put aside until this server's comes");
    assert_eq!(missing(&out, &[t("where_you_live.asking").to_string()]), Vec::<String>::new());
    assert_eq!(present(&out, &[held_line.clone()]), Vec::<String>::new(), "an answer from before is not shown");

    // You live there: where, and no choice.
    gs.home_answer = Some(held.clone());
    let out = picker(&ctx, &theme, &mut gs, Vec::new());
    assert_eq!(missing(&out, &[held_line.clone(), t("where_you_live.server_note").to_string()]), Vec::<String>::new(), "the held home's card");
    assert_eq!(present(&out, &choice_words), Vec::<String>::new(), "a held home's card shows the choice");
    assert_eq!(present(&out, &[t("where_you_live.asking").to_string()]), Vec::<String>::new(), "the answer came: no longer asking");
    assert!(sent(&rx).is_empty(), "asked once, not every frame");

    // You hold none there: the choice, with the free counts, and Choose.
    gs.home_answer = Some(guest.clone());
    let out = picker(&ctx, &theme, &mut gs, Vec::new());
    let mut want = choice_words.clone();
    want.extend(["none free".to_string(), t("where_you_live.server_note").to_string()]);
    assert_eq!(missing(&out, &want), Vec::<String>::new(), "a guest's card");
    assert_eq!(present(&out, &[held_line.clone()]), Vec::<String>::new());
    click(&ctx, &theme, &mut gs, t("where_you_live.choose"));
    assert_eq!(sent(&rx), vec![serde_json::json!({"type": "game_choose_home", "start": "cabin"})], "Choose on the Cabin asks for the cabin");
    let out = picker(&ctx, &theme, &mut gs, Vec::new());
    assert_eq!(missing(&out, &[t("where_you_live.asking").to_string()]), Vec::<String>::new(), "asking until the reply");

    // The reply: chosen.
    gs.home_answer = Some(HomeAnswer { chose: true, ..held.clone() });
    let out = picker(&ctx, &theme, &mut gs, Vec::new());
    assert_eq!(missing(&out, &[held_line.clone(), t("where_you_live.chosen").to_string()]), Vec::<String>::new(), "the reply shown");
    assert_eq!(present(&out, &[t("where_you_live.asking").to_string()]), Vec::<String>::new());

    // No identity: no home to ask for.
    gs.profile_public_key.clear();
    HERE.with(|h| *h.borrow_mut() = HereCard::default());
    let out = picker(&ctx, &theme, &mut gs, Vec::new());
    assert_eq!(missing(&out, &[t("where_you_live.needs_identity").to_string()]), Vec::<String>::new());
    assert!(sent(&rx).is_empty(), "nothing asked without an identity");
}

// ── The layout (the operator, 2026-11-02: "The left and right bar areas are small so a lot of
// text is crammed vertically. We need to even things out.") ──────────────────────────────────────

/// One frame of the picker at `w` x `h`.
fn picker_at(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, w: f32, h: f32, events: Vec<egui::Event>) -> egui::FullOutput {
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(w, h))),
        events,
        ..Default::default()
    };
    ctx.run(raw, |ctx| draw(ctx, theme, state))
}

/// The operator's picker settled at `w` x `h`: the card's first frame asks the server, then its
/// answer comes, then a few frames for egui to settle its panels and scroll areas.
fn operator_picker(w: f32, h: f32) -> (egui::Context, Theme, GuiState) {
    let starts = the_starts();
    let (ctx, theme) = ctx_and_theme();
    wyl::reset_draft();
    HERE.with(|h| *h.borrow_mut() = HereCard::default());
    let (ws, _sent) = crate::net::ws_client::WsClient::recording();
    let mut gs = GuiState::default();
    make_operator(&mut gs, ws);
    picker_at(&ctx, &theme, &mut gs, w, h, Vec::new());
    for _ in 0..4 {
        gs.home_answer = Some(operator_answer(&starts));
        picker_at(&ctx, &theme, &mut gs, w, h, Vec::new());
    }
    (ctx, theme, gs)
}

/// At least this share of the window for each side column at the operator's size: the WHO column
/// a fifth, the WHERE column two fifths (it holds the servers, their details and the kinds of
/// home). These are the test's own numbers, not the page's, so the page cannot quietly lower them.
const WHO_AT_LEAST: f32 = 0.20;
const WHERE_AT_LEAST: f32 = 0.40;

/// THE PICKER READS EVENLY AT THE OPERATOR'S SIZE (2000 x 1125, his screenshot of 2026-11-02):
/// each side column takes at least its share of the window (the WHO column a fifth, the WHERE
/// column two fifths), so their words run in lines rather than towers, and the WHERE column's
/// whole content, the connected server's card with its kinds of home and the other two servers
/// under it, fits without a scroll. His screen is 2560 x 1440 (a 65-inch TV; the screenshot came
/// scaled down to 2000 x 1125), the game's window on it 2560 x 1387 maximized (the rigs read his
/// client so, scripts/probe-sweep.js); and 1707 x 960 points is that screen with the text and
/// everything else half again as large, read from a sofa (docs/design/top-menu-and-game-pages.md
/// section 5, text size). The screenshot's 2000 x 1125 is kept too.
///
/// Seen red 2026-11-02 on the layout as it was (fixed 230 and 310 point columns): "the WHO column
/// is 230 of 2000 points, under its share of 400"; and at 2560 with the WHO column capped at 440
/// points: "the WHO column is 440 of 2560 points, under its share of 512".
#[test]
fn the_picker_reads_evenly_at_the_operators_size() {
    for (w, h) in [(2560.0, 1440.0), (2560.0, 1387.0), (1707.0, 960.0), (2000.0, 1125.0)] {
        reads_evenly_at(w, h);
    }
}

fn reads_evenly_at(w: f32, h: f32) {
    let (_ctx, _theme, _gs) = operator_picker(w, h);
    let d = drawn();
    assert_eq!(d.screen.width(), w, "the picker was drawn at the operator's size: {d:?}");
    assert!(
        d.who.width() + 0.5 >= WHO_AT_LEAST * w,
        "the WHO column is {} of {w} points, under its share of {}",
        d.who.width(),
        WHO_AT_LEAST * w
    );
    assert!(
        d.where_.width() + 0.5 >= WHERE_AT_LEAST * w,
        "the WHERE column is {} of {w} points, under its share of {}",
        d.where_.width(),
        WHERE_AT_LEAST * w
    );
    assert!(
        d.where_content_h <= d.where_view_h + 0.5,
        "the WHERE column needs a scroll at {w} x {h}: its content is {} points tall in {} points of room",
        d.where_content_h,
        d.where_view_h
    );
}

/// THE STAGE KEEPS ITS ROOM AT EVERY SIZE: from the app's own restored window (1280 x 720) through
/// the operator's 2000 x 1125 to 4K (3840 x 2160), the character's stage between the two columns
/// is at least a quarter of the window, and neither column is narrower than the narrowest it is
/// laid out for (260 points), so a small window shrinks the columns' share before their words
/// stack into towers.
#[test]
fn the_stage_keeps_its_room_at_every_size() {
    for (w, h) in [(1280.0, 720.0), (2000.0, 1125.0), (2560.0, 1440.0), (3840.0, 2160.0)] {
        let (_ctx, _theme, _gs) = operator_picker(w, h);
        let d = drawn();
        let stage = d.where_.left() - d.who.right();
        assert!(stage >= 0.25 * w, "at {w} x {h} the stage is {stage} points of {w}: {d:?}");
        assert!(d.who.width() >= 259.5 && d.where_.width() >= 259.5, "at {w} x {h} a column is under 260 points: {d:?}");
    }
}

/// Click the `nth` (from 0) text exactly `label` where the picker draws it at `w` x `h`: the
/// canonical three frames (move, press, release), the server's answer set before each frame as
/// the app keeps it.
fn click_at_size(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, w: f32, h: f32, label: &str, nth: usize) {
    let answer = state.home_answer.clone();
    let out = picker_at(ctx, theme, state, w, h, Vec::new());
    let mut found = Vec::new();
    for cs in &out.shapes {
        visit(&cs.shape, &mut |t| {
            if t.galley.text() == label {
                found.push(egui::Rect::from_min_size(t.pos, t.galley.size()).center());
            }
        });
    }
    let pos = *found.get(nth).unwrap_or_else(|| panic!("{label:?} is drawn {} times, not {}: {:?}", found.len(), nth + 1, texts(&out)));
    assert!(pos.x < w && pos.y < h, "{label:?} is on the screen at {w} x {h}: {pos:?}");
    let m = egui::Modifiers::default();
    for ev in [
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m },
        egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m },
    ] {
        if state.home_answer.is_none() {
            state.home_answer = answer.clone();
        }
        picker_at(ctx, theme, state, w, h, vec![ev]);
    }
}

/// EVERY CONTROL ON THE PICKER ANSWERS A CLICK, not only draws (memory "shows != works"): on the
/// operator's picker at his 2000 x 1125, a click on each lands where it is drawn and does what it
/// says. The public.guide card selects that server and the united-humanity.us card selects the
/// live one back; Server details opens the server's address; a kind of home's "What it is like"
/// opens its whole sentence under the row; the Homestead's "Just you" picks that household and its
/// Choose asks the server for a homestead for just you; the My Homestead card selects your own
/// home; Enter commits; "+ New character" makes a new one, and Edit look opens the editor.
///
/// Seen red 2026-11-02 with the cards drawn but their click target sensing only the pointer
/// passing over (`Sense::hover()` in `pick_card`): "the public.guide card selects that server",
/// left: Some("__connected__").
#[test]
fn every_control_on_the_picker_answers_a_click() {
    let (w, h) = (2000.0, 1125.0);
    let starts = the_starts();
    let (ctx, theme, mut gs) = operator_picker(w, h);
    let rx = {
        // A fresh recording client, so the test reads only what its own clicks send.
        let (ws, rx) = crate::net::ws_client::WsClient::recording();
        gs.ws_client = Some(ws);
        rx
    };
    let homestead_cabin = starts.get("cabin").expect("the cabin").sentence.clone();

    click_at_size(&ctx, &theme, &mut gs, w, h, "public.guide", 0);
    assert_eq!(gs.launcher_selected_server.as_deref(), Some("srv_https://public.guide"), "the public.guide card selects that server");
    click_at_size(&ctx, &theme, &mut gs, w, h, "united-humanity.us", 0);
    assert_eq!(gs.launcher_selected_server.as_deref(), Some(CONNECTED_SERVER_ID), "the live server's card selects it back");

    let out = picker_at(&ctx, &theme, &mut gs, w, h, Vec::new());
    assert_eq!(present(&out, &[t("characters.address").to_string() + ":"]), Vec::<String>::new(), "the address is folded away at first");
    click_at_size(&ctx, &theme, &mut gs, w, h, t("characters.server_details"), 0);
    let out = picker_at(&ctx, &theme, &mut gs, w, h, Vec::new());
    assert_eq!(missing(&out, &[t("characters.address").to_string() + ":", "https://united-humanity.us".to_string()]), Vec::<String>::new(), "Server details opens the address");

    assert_eq!(present(&out, &[homestead_cabin.clone()]), Vec::<String>::new(), "the Cabin's whole sentence is folded away at first");
    click_at_size(&ctx, &theme, &mut gs, w, h, t("where_you_live.more"), 0);
    let out = picker_at(&ctx, &theme, &mut gs, w, h, Vec::new());
    assert_eq!(missing(&out, &[homestead_cabin]), Vec::<String>::new(), "the Cabin's fold opens its whole sentence");

    click_at_size(&ctx, &theme, &mut gs, w, h, "Just you", 0);
    assert_eq!(wyl::draft_household(&starts, "homestead"), "solo", "the Homestead's household choice answers");
    click_at_size(&ctx, &theme, &mut gs, w, h, t("where_you_live.choose"), 1);
    assert_eq!(sent(&rx), vec![serde_json::json!({"type": "game_choose_home", "start": "homestead"})], "the Homestead's Choose asks for a homestead");
    assert_eq!(wyl::take_asked(), Some(("homestead".to_string(), "solo".to_string())), "for just you");

    click_at_size(&ctx, &theme, &mut gs, w, h, t("characters.enter"), 0);
    assert!(gs.showroom_confirm, "Enter commits the pairing");
    gs.showroom_confirm = false;

    click_at_size(&ctx, &theme, &mut gs, w, h, "My Homestead", 0);
    assert_eq!(gs.launcher_where_kind, LauncherWhere::Home, "the home's card selects your own home");
    assert_eq!(gs.launcher_selected_world, "My Homestead");

    click_at_size(&ctx, &theme, &mut gs, w, h, t("characters.new_character"), 0);
    assert!(gs.launcher_who.is_empty(), "+ New character picks a new one");
    assert_eq!(gs.character_name, t("characters.default_name"));

    click_at_size(&ctx, &theme, &mut gs, w, h, t("characters.edit_look"), 0);
    assert_eq!(gs.showroom_mode, 1, "Edit look opens the editor");
    assert!(gs.showroom_return_to_picker, "whose Done comes back here");
}

/// THE CHARACTER STANDS IN THE MIDDLE OF THE STAGE: with the columns of the operator's 2000 x 1125
/// (400 and 880 points), the stage between them is centred 240 points left of the window's middle,
/// and the camera's lens shift (`stage_shift`, which lib.rs gives the camera each frame) draws the
/// point the showroom camera orbits, the character, in the middle of the stage, not of the window.
///
/// Seen red 2026-11-02 with `stage_shift` returning 0 (the lens left centred): "the character is
/// drawn at 1000 points, the stage's middle is 760".
#[test]
fn the_character_stands_in_the_middle_of_the_stage() {
    let (w, h) = (2000.0, 1125.0);
    let (_ctx, _theme, _gs) = operator_picker(w, h);
    let d = drawn();
    let middle = (d.who.right() + d.where_.left()) * 0.5;
    let mut cam = crate::renderer::camera::Camera::new();
    cam.mode = crate::renderer::camera::CameraMode::Orbit;
    cam.aspect = w / h;
    cam.orbit_target = glam::Vec3::new(0.0, 0.9, 0.0);
    cam.orbit_distance = 3.2;
    cam.lens_shift_x = stage_shift();
    let clip = cam.view_projection_matrix() * cam.orbit_target.extend(1.0);
    let at = (clip.x / clip.w + 1.0) * 0.5 * w;
    assert!((at - middle).abs() < 1.0, "the character is drawn at {at} points, the stage's middle is {middle}");
    assert!((middle - w * 0.5).abs() > 100.0, "the stage is off the window's middle at this size: {middle}");
}
