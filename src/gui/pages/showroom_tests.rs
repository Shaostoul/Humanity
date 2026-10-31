//! Tests of the Play picker (showroom.rs) for ship homes increment 6, lane 2B
//! (docs/design/ship-homes-increment-6-plan.md 3.9): the WHO hint, the new-home row's choice and
//! the connected server's card, each drawn headlessly the way the app draws the picker
//! (`showroom::draw`) and read back from what was DRAWN, as where_you_live_tests.rs reads the
//! widget. Each was seen failing once on purpose; the break and the failure text are in its
//! comment.

use super::*;
use crate::engine::save_slot;
use crate::persistence::live_save;
use crate::gui::screen_surface::find_text_in_shapes;
use crate::persistence::SaveSlot;
use crate::ship::starts::{HomeAnswer, Starts};
use std::path::Path;

// ── The data ─────────────────────────────────────────────────────────────────────────────────────

/// The repo's starts file, as the game reads it.
fn the_starts() -> Starts {
    Starts::parse(&std::fs::read(Path::new("data").join(crate::ship::starts::STARTS_FILE)).expect("data/homes/starts.ron"))
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
    assert!(
        !WHO_HINT.contains("one save on this computer"),
        "the picker still says one save holds gear and skills for every server: {WHO_HINT}"
    );
    assert!(WHO_HINT.contains("Your name and look travel with you."), "{WHO_HINT}");
    assert!(WHO_HINT.contains("Each server you visit keeps its own home and things for you, starting fresh"), "{WHO_HINT}");
    assert!(WHO_HINT.contains("your own home keeps yours"), "{WHO_HINT}");
    let dir = Path::new("saves-anywhere");
    let me = "ab".repeat(32);
    let own = crate::persistence::slot_path(dir, &SaveSlot::Offline);
    let a = crate::persistence::slot_path(dir, &SaveSlot::Server { server: "https://a.example".into(), identity: me.clone() });
    let b = crate::persistence::slot_path(dir, &SaveSlot::Server { server: "https://b.example".into(), identity: me });
    assert_eq!(own.file_name().and_then(|n| n.to_str()), Some("offline_home.json"), "{}", own.display());
    assert!(a != b && a != own && b != own, "three worlds, three saves: {} {} {}", own.display(), a.display(), b.display());
    for server in [&a, &b] {
        assert_eq!(server.parent(), Some(dir.join("servers").as_path()), "a server's save is under saves/servers/: {}", server.display());
    }
}

/// THE NEW-HOME ROW OFFERS THE STARTS FROM DATA (plan 3.9, 2B). On a first run (no home of your own
/// yet) the picker's new-home row reads "A new home"; selected, it shows the choice: its title and
/// hint, then every start of data/homes/starts.ron in its order, each with its sentence, the default
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
    let mut want = vec![NEW_HOME.to_string(), wyl::TITLE.to_string(), wyl::HINT.to_string()];
    for s in &starts.starts {
        want.push(wyl::row_name(&starts, &s.id, &s.name));
        want.push(s.sentence.clone());
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
    assert_eq!(missing(&out, &[NEW_HOME_TAKEN.to_string()]), Vec::<String>::new());
    assert_eq!(present(&out, &[wyl::TITLE.to_string(), "Cabin".to_string()]), Vec::<String>::new(), "no choice over a home already saved");
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
    let choice_words = vec![wyl::TITLE.to_string(), wyl::CHOOSE.to_string(), "15 free".to_string(), wyl::NO_HOME_HERE.to_string()];

    // An answer from before the card asked is not shown; the card asks this server.
    gs.home_answer = Some(held.clone());
    let out = picker(&ctx, &theme, &mut gs, Vec::new());
    let asked = sent(&rx);
    assert_eq!(asked, vec![serde_json::json!({"type": "game_home_request"})], "the card asks this server once");
    assert!(gs.home_answer.is_none(), "the old answer is put aside until this server's comes");
    assert_eq!(missing(&out, &[wyl::ASKING.to_string()]), Vec::<String>::new());
    assert_eq!(present(&out, &[held_line.clone()]), Vec::<String>::new(), "an answer from before is not shown");

    // You live there: where, and no choice.
    gs.home_answer = Some(held.clone());
    let out = picker(&ctx, &theme, &mut gs, Vec::new());
    assert_eq!(missing(&out, &[held_line.clone(), wyl::SERVER_NOTE.to_string()]), Vec::<String>::new(), "the held home's card");
    assert_eq!(present(&out, &choice_words), Vec::<String>::new(), "a held home's card shows the choice");
    assert_eq!(present(&out, &[wyl::ASKING.to_string()]), Vec::<String>::new(), "the answer came: no longer asking");
    assert!(sent(&rx).is_empty(), "asked once, not every frame");

    // You hold none there: the choice, with the free counts, and Choose.
    gs.home_answer = Some(guest.clone());
    let out = picker(&ctx, &theme, &mut gs, Vec::new());
    let mut want = choice_words.clone();
    want.extend(["none free".to_string(), wyl::SERVER_NOTE.to_string()]);
    assert_eq!(missing(&out, &want), Vec::<String>::new(), "a guest's card");
    assert_eq!(present(&out, &[held_line.clone()]), Vec::<String>::new());
    click(&ctx, &theme, &mut gs, wyl::CHOOSE);
    assert_eq!(sent(&rx), vec![serde_json::json!({"type": "game_choose_home", "start": "cabin"})], "Choose on the Cabin asks for the cabin");
    let out = picker(&ctx, &theme, &mut gs, Vec::new());
    assert_eq!(missing(&out, &[wyl::ASKING.to_string()]), Vec::<String>::new(), "asking until the reply");

    // The reply: chosen.
    gs.home_answer = Some(HomeAnswer { chose: true, ..held.clone() });
    let out = picker(&ctx, &theme, &mut gs, Vec::new());
    assert_eq!(missing(&out, &[held_line.clone(), wyl::CHOSEN.to_string()]), Vec::<String>::new(), "the reply shown");
    assert_eq!(present(&out, &[wyl::ASKING.to_string()]), Vec::<String>::new());

    // No identity: no home to ask for.
    gs.profile_public_key.clear();
    HERE.with(|h| *h.borrow_mut() = HereCard::default());
    let out = picker(&ctx, &theme, &mut gs, Vec::new());
    assert_eq!(missing(&out, &[NO_IDENTITY.to_string()]), Vec::<String>::new());
    assert!(sent(&rx).is_empty(), "nothing asked without an identity");
}
