//! Tests of the fleet panel (fleet_ledger.rs). Each was seen to fail before it passed; the
//! failure text is in its comment.

use super::*;
use crate::gui::screen_surface::find_text_in_shapes;

fn ledger(standing: &str, used: f64, contributed: f64) -> FleetLedger {
    FleetLedger { supply: "unlimited".into(), used, contributed, standing: standing.into(), ..Default::default() }
}

fn bread_slot(n: u32) -> Option<crate::gui::GuiItemSlot> {
    Some(crate::gui::GuiItemSlot { item_id: "bread_0".into(), name: "Bread".into(), quantity: n, ..Default::default() })
}

/// THE HEADLINE SAYS RED OR BLACK IN PLAIN WORDS, and by how much.
///
/// Seen red 2026-10-04 with the difference not made positive (given minus used): "left:
/// \"In the red: you have used -18 CR more from the fleet than you have given it.\" / right:
/// \"In the red: you have used 18 CR more from the fleet than you have given it.\"".
#[test]
fn the_headline_says_red_or_black_in_plain_words() {
    assert_eq!(standing_sentence(&ledger("black", 10.0, 22.0)), "In the black: you have given the fleet 12 CR more than you have used from it.");
    assert_eq!(standing_sentence(&ledger("red", 30.0, 12.0)), "In the red: you have used 18 CR more from the fleet than you have given it.");
    assert_eq!(standing_sentence(&ledger("even", 0.0, 0.0)), "Nothing yet: you have not used anything from the fleet or given it anything.");
    assert_eq!(standing_sentence(&ledger("even", 5.0, 5.0)), "Even: you have given the fleet as much as you have used from it.");
    assert_eq!(credits(4.5), "4.5 CR");
    assert_eq!(amount_text(3.0, "meal", None), "3 meals");
    assert_eq!(amount_text(1.0, "meal", None), "1 meal");
    assert_eq!(amount_text(2.25, "kWh", None), "2.25 kWh");
    assert_eq!(amount_text(2.0, "", Some("Bread")), "2 Bread");
    assert_eq!(amount_text(2.0, "", None), "2 items");
}

/// A DIFFERENCE THAT PRINTS AS NOTHING READS EVEN (finding 17 of the 2026-10-04 review): a
/// meal used (10 CR) against 6.68 kWh of power returned (10.02 CR) is +0.02, which credits()
/// prints "0 CR"; even if a server says "black", the headline is the even one, never "0 CR
/// more". A tenth of a credit still reads.
///
/// Seen red 2026-10-04 with the fallback taken out of `standing_sentence`: "left: \"In the
/// black: you have given the fleet 0 CR more than you have used from it.\" / right: \"Even:
/// you have given the fleet as much as you have used from it.\"".
#[test]
fn a_difference_that_prints_as_nothing_reads_even() {
    assert_eq!(standing_sentence(&ledger("black", 10.0, 10.02)), "Even: you have given the fleet as much as you have used from it.");
    assert_eq!(standing_sentence(&ledger("red", 10.02, 10.0)), "Even: you have given the fleet as much as you have used from it.");
    assert_eq!(standing_sentence(&ledger("black", 10.0, 10.1)), "In the black: you have given the fleet 0.1 CR more than you have used from it.");
}

/// A LINE SAYS WHICH GAME DAY AND WHICH REAL DATE, NO FINER (finding 10 of the 2026-10-04
/// review): the server keeps a line's game time to the start of its game day, so the panel
/// shows no hour.
///
/// Seen red 2026-10-04 with the hour and minute still shown: "left: \"Day 3, 14:20
/// (2026-10-04)\" / right: \"Day 3 (2026-10-04)\"".
#[test]
fn a_line_says_its_game_day_and_real_date() {
    assert_eq!(when_text(86_400.0 * 2.0 + 3600.0 * 14.0 + 60.0 * 20.0, 20_730), "Day 3 (2026-10-04)");
    assert_eq!(when_text(0.0, 20_730), "Day 1 (2026-10-04)");
}

/// The server's message reads into the ledger the panel draws.
///
/// Seen red 2026-10-04 with the used total read from "used" (the message says
/// "used_value"): "left: (0.0, 6.0, \"red\") / right: (10.0, 6.0, \"red\")".
#[test]
fn a_ledger_message_reads() {
    let v = serde_json::json!({
        "type": "game_fleet_ledger", "supply": "unlimited", "used_value": 10.0, "contributed_value": 6.0,
        "standing": "red", "kinds": [{"kind": "meal", "label": "Meal from the ship's stores", "unit": "meal", "direction": "used", "quantity": 1.0, "value": 10.0}],
        "recent": [{"label": "Given to the fleet", "unit": "", "direction": "contributed", "item_name": "Bread", "quantity": 2.0, "value": 6.0, "game_time": 100.0, "real_day": 20730}],
    });
    let l = FleetLedger::from_json(&v).unwrap();
    assert_eq!((l.used, l.contributed, l.standing.as_str()), (10.0, 6.0, "red"));
    assert_eq!(l.kinds[0].label, "Meal from the ship's stores");
    assert_eq!(l.recent[0].item_name.as_deref(), Some("Bread"));
    assert!(FleetLedger::from_json(&serde_json::json!({"type": "game_fleet_ledger", "error": "failed"})).is_none());
    let t = FleetTotals::from_json(&serde_json::json!({"type": "game_fleet_totals", "players": 2, "withheld": true, "others_needed": 3}));
    assert!(t.withheld && t.others_needed == 3);
}

/// A give is checked before it goes: not in the world, too far from a store, more than the
/// backpack holds, no connection, all say why and ask for nothing.
///
/// Seen red 2026-10-04 with `free_to_give` not counting what a give asked this frame: "a
/// second give of the same 3 loaves was let through / left: \"Not connected to the server.\" /
/// right: \"You carry only 0 of that free to give.\"".
#[test]
fn a_give_is_checked_before_it_goes() {
    let mut gs = GuiState::default();
    gs.fleet.stores = vec![FleetStore { entity_id: 12, name: "The mess hall's stores".into(), position: [67.0, 1.0, 22.0] }];
    gs.inventory_items = vec![bread_slot(3)];
    assert_eq!(start_give(&mut gs, "bread_0", 1), "Join a server's shared world to give to its fleet.");
    gs.copresence_active = true;
    gs.fleet.my_position = Some([67.0, 1.7, 40.0]);
    assert!(start_give(&mut gs, "bread_0", 1).starts_with("Walk within 5 m of The mess hall's stores"), "too far");
    gs.fleet.my_position = Some([68.0, 1.7, 22.0]);
    assert_eq!(start_give(&mut gs, "bread_0", 4), "You carry only 3 of that free to give.");
    assert_eq!(start_give(&mut gs, "bread_0", 3), "Not connected to the server.");
    assert!(gs.fleet.outbox.is_empty(), "nothing asked for without a connection");
    // A give already asked for this frame counts against the backpack.
    gs.fleet.outbox.push(FleetGive { give_id: "give-1".into(), store: 12, item_id: "bread_0".into(), name: "Bread".into(), qty: 3, wear: 0, quality: 0 });
    assert_eq!(free_to_give(&gs, "bread_0"), 0, "a second give of the same 3 loaves was let through / left: {} / right: 0", free_to_give(&gs, "bread_0"));
}

/// LOAVES PROMISED TO A CONFIRMED TRADE ARE NOT FREE TO GIVE (finding 1 of the 2026-10-04
/// review): the player confirmed a trade offering 5 Bread and carries 7, so 2 are free; once
/// the other player confirms, their game receives the 5 whether or not they are still here.
/// An unconfirmed offer promises nothing yet.
///
/// Seen red 2026-10-04 with `free_to_give` not counting trade promises: "2 of 7 loaves are
/// free / left: 7 / right: 2".
#[test]
fn loaves_promised_to_a_confirmed_trade_are_not_free_to_give() {
    let mut gs = GuiState::default();
    gs.profile_public_key = "me".into();
    gs.inventory_items = vec![bread_slot(7)];
    let offer = crate::gui::GuiTradeItem { item_type: "item".into(), name: "Bread".into(), quantity: 5, reference_id: Some("bread_0".into()), ..Default::default() };
    gs.trades = vec![crate::gui::GuiTrade {
        id: "t-1".into(),
        initiator_key: "me".into(),
        recipient_key: "them".into(),
        status: "active".into(),
        initiator_items: vec![offer],
        initiator_confirmed: true,
        ..Default::default()
    }];
    let free = free_to_give(&gs, "bread_0");
    assert_eq!(free, 2, "2 of 7 loaves are free / left: {free} / right: 2");
    gs.trades[0].initiator_confirmed = false;
    assert_eq!(free_to_give(&gs, "bread_0"), 7, "an unconfirmed offer promises nothing yet");
}

/// ONE GIVE WAITS AT A TIME (findings 4 and 13 of the 2026-10-04 review): while a give to
/// this server is held for its answer, another is refused in words (and the button is off), so
/// a double click cannot send two gives inside the relay's 200 ms. A give held for ANOTHER
/// server does not block this one.
///
/// Seen red 2026-10-04 with `start_give` not checking for a waiting give: "a second give
/// waits / left: \"Not connected to the server.\" / right: starts with \"Waiting for the
/// server\"".
#[test]
fn one_give_waits_at_a_time() {
    let mut gs = GuiState::default();
    gs.copresence_active = true;
    gs.connected_server_url = "wss://here".into();
    gs.fleet.stores = vec![FleetStore { entity_id: 12, name: "The mess hall's stores".into(), position: [67.0, 1.0, 22.0] }];
    gs.fleet.my_position = Some([68.0, 1.7, 22.0]);
    gs.inventory_items = vec![bread_slot(3)];
    let held = |server: &str| FleetHeld { give_id: "g-1".into(), server: server.into(), store: 12, item_id: "bread_0".into(), name: "Bread".into(), qty: 1, ..Default::default() };
    gs.fleet.held = vec![held("wss://here")];
    let second = start_give(&mut gs, "bread_0", 1);
    assert!(second.starts_with("Waiting for the server to record 1 Bread"), "a second give waits / left: {second:?} / right: starts with \"Waiting for the server\"");
    gs.fleet.held = vec![held("wss://elsewhere")];
    assert_eq!(waiting_give(&gs), None, "a give held for another server does not block this one");
}

/// THE WEB'S GAME ADMIN WINDOW OFFERS THE SAME FLEET SUPPLY CONTROL: the same two modes under
/// the same names, the same message, and app.js keeps the mode and the totals it shows; it says
/// when the totals are held back.
///
/// Seen red 2026-10-04 with the web's stocked label written "Stocked (stores can run out)":
/// "the web window offers Stocked (realistic: stores can run empty) too: ['stocked',
/// 'Stocked (realistic: stores can run empty)'] in FLEET_MODES". And seen red the same day
/// before the web read `withheld`: "the web says when the totals are held back".
#[test]
fn the_web_window_offers_the_same_fleet_supply_control() {
    let js = std::fs::read_to_string("web/chat/chat-game-admin.js").expect("web/chat/chat-game-admin.js");
    for mode in ["unlimited", "stocked"] {
        let entry = format!("['{mode}', '{}']", mode_name(mode));
        assert!(js.contains(&entry), "the web window offers {} too: {entry} in FLEET_MODES", mode_name(mode));
    }
    assert!(js.contains("type: 'server_settings_update', fleet_supply_mode: chosen"), "the web sends the same update");
    assert!(js.contains("type: 'game_fleet_totals_request'"), "and asks for the same totals");
    assert!(js.contains("t.withheld") && js.contains("The totals are shown once at least "), "the web says when the totals are held back");
    let app = std::fs::read_to_string("web/chat/app.js").expect("web/chat/app.js");
    assert!(app.contains("msg.settings.fleet_supply_mode") && app.contains("case 'game_fleet_totals':"), "app.js keeps the mode and the totals");
}

/// THE WEB SHOWS THE PLAYER'S OWN LEDGER, READ-ONLY, IN THE SAME WORDS (finding 14 of the
/// 2026-10-04 review): the ledger is the server's record, so a browser signed in as the player
/// can show it with no game world. web/chat/chat-fleet.js asks for it the way the game does,
/// app.js hands it the answer, the page loads the file, the command palette opens it, and its
/// headline and notes say what the panel says. Taking a meal and giving stay in the game.
///
/// Seen red 2026-10-04 before the web view was written: "web/chat/chat-fleet.js: The system
/// cannot find the file specified. (os error 2)".
#[test]
fn the_web_shows_the_players_ledger_in_the_same_words() {
    let raw = std::fs::read_to_string("web/chat/chat-fleet.js").unwrap_or_else(|e| panic!("web/chat/chat-fleet.js: {e}"));
    assert!(raw.contains("type: 'game_fleet_ledger_request'"), "it asks for the ledger the way the game does");
    // The JavaScript's text as it reads: white space flattened, its strings joined ('a ' + 'b'),
    // its quotes unescaped.
    let flat = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    let js = flat(&raw).replace("' + '", "").replace("\\'", "'");
    for words in [
        "In the black: you have given the fleet ",
        " more than you have used from it.",
        "In the red: you have used ",
        " more from the fleet than you have given it.",
        "Even: you have given the fleet as much as you have used from it.",
        "Nothing yet: you have not used anything from the fleet or given it anything.",
        "Used from the fleet: ",
        "Given to the fleet: ",
        "Of which power: ",
        "in the game, at a fleet store",
    ] {
        assert!(js.contains(words), "the web says {words:?} as the panel does");
    }
    for s in [supply_sentence("unlimited"), supply_sentence("stocked")] {
        assert!(js.contains(s), "the web says {s:?}");
    }
    assert!(js.contains(&flat(INTRO)) && js.contains(&flat(POWER_NOTE)), "and the same intro and power note");
    let app = std::fs::read_to_string("web/chat/app.js").unwrap();
    assert!(app.contains("case 'game_fleet_ledger':"), "app.js hands the answer to it");
    let index = std::fs::read_to_string("web/chat/index.html").unwrap();
    assert!(index.contains("/chat/chat-fleet.js"), "the chat page loads it");
    let palette = std::fs::read_to_string("data/commands.json").unwrap();
    assert!(palette.contains("\"action_id\": \"openFleetLedger\""), "the command palette opens it");
    let ui = std::fs::read_to_string("web/chat/chat-ui.js").unwrap();
    assert!(ui.contains("openFleetLedger:"), "and the palette knows the action");
}

/// One headless frame of the fleet section in a plain panel.
fn frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) -> egui::FullOutput {
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, 1100.0))), ..Default::default() };
    ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| draw_section(ui, theme, state));
    })
}

fn ctx_and_theme() -> (egui::Context, Theme) {
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    (ctx, theme)
}

/// THE PANEL SHOWS THE LEDGER IN WORDS: the headline, both totals, the power note and power's
/// share (finding 18), the supply mode without "nobody goes without" (finding 7), a line, and,
/// at a store, the Take a meal and Give buttons; out of the world it says to join one.
///
/// Seen red 2026-10-04 with `draw_ledger` not called when a ledger is in hand: "the panel
/// shows the headline" (no shape with that text was drawn). And seen red the same day before
/// the power sentence was added: "the panel shows \"counted by itself every minute\"".
#[test]
fn the_panel_shows_the_ledger_in_words() {
    let (ctx, theme) = ctx_and_theme();
    let mut gs = GuiState::default();
    let out = frame(&ctx, &theme, &mut gs);
    assert!(find_text_in_shapes(&out.shapes, "Join one to see yours").is_some(), "out of the world it says to join one");
    gs.copresence_active = true;
    gs.fleet.ledger = Some(demo_ledger());
    gs.fleet.stores = vec![FleetStore { entity_id: 12, name: "The mess hall's stores".into(), position: [67.0, 1.0, 22.0] }];
    gs.fleet.my_position = Some([68.0, 1.7, 22.0]);
    gs.fleet.prices.insert("bread_0".into(), 3.0);
    gs.inventory_items = vec![bread_slot(3)];
    let out = frame(&ctx, &theme, &mut gs);
    for text in [
        "In the red: you have used 8.5 CR more",
        "Used from the fleet: 14.5 CR",
        "Given to the fleet: 6 CR",
        "counted by itself every minute",
        "Of which power: 4.5 CR used, 0 CR given back.",
        "fleet is unlimited",
        "2 Bread",
        "Take a meal",
        "Give to the fleet",
    ] {
        assert!(find_text_in_shapes(&out.shapes, text).is_some(), "the panel shows {text:?}");
    }
    assert!(find_text_in_shapes(&out.shapes, "nobody goes without").is_none(), "no promise that nobody goes without");
    // An admin switched the server to stocked after the ledger came: the server's settings,
    // which every connected app is sent, say so at once. Seen red 2026-10-04 with the
    // sentence read from the ledger only: "after the switch the panel says the fleet is
    // stocked".
    let mut s = crate::relay::storage::ServerSettings::default();
    s.fleet_supply_mode = "stocked".into();
    gs.server_settings = Some(s);
    let out = frame(&ctx, &theme, &mut gs);
    assert!(find_text_in_shapes(&out.shapes, "fleet is stocked").is_some(), "after the switch the panel says the fleet is stocked");
    // Creative mode on: the give form says gifts are recorded but not counted.
    gs.creative_mode = true;
    let out = frame(&ctx, &theme, &mut gs);
    assert!(find_text_in_shapes(&out.shapes, "Creative mode is on").is_some(), "Creative mode is named at the give form");
}

/// OUT OF THE WORLD, A LEDGER IN HAND IS SAID TO BE FROM THE LAST VISIT, and gives held for a
/// server are shown where they are (finding 16 of the 2026-10-04 review).
///
/// Seen red 2026-10-04 before the label: "out of the world the ledger says it is from the last
/// visit".
#[test]
fn out_of_the_world_the_ledger_is_from_the_last_visit() {
    let (ctx, theme) = ctx_and_theme();
    let mut gs = GuiState::default();
    gs.fleet.ledger = Some(demo_ledger());
    gs.connected_server_url = "wss://here".into();
    gs.fleet.held = vec![FleetHeld { give_id: "g-1".into(), server: "wss://there".into(), item_id: "bread_0".into(), name: "Bread".into(), qty: 2, ..Default::default() }];
    let out = frame(&ctx, &theme, &mut gs);
    assert!(find_text_in_shapes(&out.shapes, "as of your last visit").is_some(), "out of the world the ledger says it is from the last visit");
    assert!(find_text_in_shapes(&out.shapes, "2 Bread are held for the fleet of the server at wss://there").is_some(), "a give held for another server is shown");
}

/// A SERVER SWITCH FORGETS THE OLD SERVER'S LEDGER (finding 16 of the 2026-10-04 review): its
/// ledger, stores, totals and what was sent to it go; gives held for it stay, tagged with it.
///
/// Seen red 2026-10-04 before the switch cleared the fleet: "the old server's ledger is gone
/// after a switch / left: true / right: false".
#[test]
fn a_server_switch_forgets_the_old_servers_ledger() {
    let mut gs = GuiState::default();
    gs.fleet.ledger = Some(demo_ledger());
    gs.fleet.stores = vec![FleetStore { entity_id: 12, name: "The mess hall's stores".into(), position: [0.0; 3] }];
    gs.fleet.totals = Some(FleetTotals::default());
    gs.fleet.sent = vec!["g-1".into()];
    gs.fleet.held = vec![FleetHeld { give_id: "g-1".into(), server: "wss://a".into(), ..Default::default() }];
    gs.park_active_connection();
    let kept = gs.fleet.ledger.is_some();
    assert!(!kept, "the old server's ledger is gone after a switch / left: {kept} / right: false");
    assert!(gs.fleet.stores.is_empty() && gs.fleet.totals.is_none() && gs.fleet.sent.is_empty());
    assert_eq!(gs.fleet.held.len(), 1, "a give held for that server stays held");
}

/// One headless frame of the admin's Fleet supply section, with `events`.
fn admin_frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, events: Vec<egui::Event>) -> egui::FullOutput {
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, 800.0))), events, ..Default::default() };
    ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| draw_admin(ui, theme, state));
    })
}

fn admin_click(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, text: &str) {
    let out = admin_frame(ctx, theme, state, Vec::new());
    let pos = find_text_in_shapes(&out.shapes, text).unwrap_or_else(|| panic!("{text} is drawn")).rect.center();
    let m = egui::Modifiers::default();
    admin_frame(ctx, theme, state, vec![egui::Event::PointerMoved(pos)]);
    admin_frame(ctx, theme, state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
    admin_frame(ctx, theme, state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
}

/// THE ADMIN PICKS A MODE AND APPLIES IT, the way Server Settings > ADMIN > Fleet supply is
/// used: the server says unlimited, the admin picks Stocked, Apply with the link down says
/// the server was NOT asked and keeps the pick.
///
/// Seen red 2026-10-04 with the status set whether or not anything was sent: "with no
/// connection the section says: Asked the server to make the fleet stocked.".
#[test]
fn picking_a_fleet_mode_and_applying_with_no_connection_says_so() {
    let (ctx, theme) = ctx_and_theme();
    let mut state = GuiState::default();
    state.server_settings = Some(crate::relay::storage::ServerSettings::default());
    let out = admin_frame(&ctx, &theme, &mut state, Vec::new());
    assert!(find_text_in_shapes(&out.shapes, "Now: Unlimited (never runs out)").is_some(), "the section says the server's mode");
    admin_click(&ctx, &theme, &mut state, "Stocked (realistic: stores can run empty)");
    assert_eq!(state.fleet.admin_draft.as_deref(), Some("stocked"), "the admin's pick");
    admin_click(&ctx, &theme, &mut state, "Apply to the fleet");
    assert_eq!(state.game_admin_status, "Not connected to the server.", "with no connection the section says: {}", state.game_admin_status);
    assert_eq!(state.fleet.admin_draft.as_deref(), Some("stocked"), "and keeps the pick");
}

/// THE ADMIN IS TOLD WHAT THE TOTALS HIDE AND WHY THEY ARE HELD BACK (finding 9 of the
/// 2026-10-04 review): the section says the totals wait for three other players and that
/// watching them can still hint at who did what; a withheld answer reads as such, with no sums.
///
/// Seen red 2026-10-04 before the note: "the admin is told the totals are not anonymous with
/// few players".
#[test]
fn the_admin_is_told_why_totals_are_held_back() {
    let (ctx, theme) = ctx_and_theme();
    let mut state = GuiState::default();
    state.fleet.totals = Some(FleetTotals { players: 2, withheld: true, others_needed: 3, ..Default::default() });
    let out = admin_frame(&ctx, &theme, &mut state, Vec::new());
    assert!(find_text_in_shapes(&out.shapes, "minus your own lines").is_some(), "the admin is told the totals are not anonymous with few players");
    assert!(find_text_in_shapes(&out.shapes, "2 players have a ledger. The totals are shown once at least 3 players other than you have one.").is_some(), "a withheld answer reads as such");
}

/// THE ADMIN MAP LISTS THE FLEET'S CONTROLS (finding 15 of the 2026-10-04 review): Server
/// Settings > ADMIN's map (data/admin/ops_registry.json) names the fleet supply and the
/// fleet's totals, with their native place and the web window, as the world clock's entry does.
///
/// Seen red 2026-10-04 before the entries: "the admin map lists \"Set whether the fleet's
/// stores can run out\"".
#[test]
fn the_admin_map_lists_the_fleet_supply_and_totals() {
    let reg = crate::gui::ops_registry::ops_registry();
    for name in ["Set whether the fleet's stores can run out", "See the fleet's totals"] {
        let a = reg.actions.iter().find(|a| a.name == name).unwrap_or_else(|| panic!("the admin map lists {name:?}"));
        assert!(a.how.contains("Fleet supply"), "{name}: says where: {}", a.how);
        assert!(a.also_available.as_ref().is_some_and(|v| v.iter().any(|s| s.contains("Game Admin window"))), "{name}: and the web window");
    }
}

/// The ledger the snapshot and the panel test draw: a meal used, a day's power, bread given.
pub(crate) fn demo_ledger() -> FleetLedger {
    FleetLedger::from_json(&serde_json::json!({
        "supply": "unlimited", "used_value": 14.5, "contributed_value": 6.0, "standing": "red",
        "kinds": [
            {"kind": "meal", "label": "Meal from the ship's stores", "unit": "meal", "direction": "used", "quantity": 1.0, "value": 10.0},
            {"kind": "power_drawn", "label": "Power from the ship's reactor", "unit": "kWh", "direction": "used", "quantity": 3.0, "value": 4.5},
            {"kind": "item", "label": "Given to the fleet", "unit": "", "direction": "contributed", "quantity": 2.0, "value": 6.0},
        ],
        "recent": [
            {"label": "Given to the fleet", "unit": "", "direction": "contributed", "item_name": "Bread", "quantity": 2.0, "value": 6.0, "game_time": 172_800.0, "real_day": 20730},
            {"label": "Power from the ship's reactor", "unit": "kWh", "direction": "used", "quantity": 3.0, "value": 4.5, "game_time": 172_800.0, "real_day": 20730},
            {"label": "Meal from the ship's stores", "unit": "meal", "direction": "used", "quantity": 1.0, "value": 10.0, "game_time": 172_800.0, "real_day": 20730},
        ],
    }))
    .unwrap()
}
