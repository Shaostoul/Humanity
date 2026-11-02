//! The warning bar's mute (gui/bottom_strip/mute.rs): the rule both apps share, the choices as
//! data, that a mute hides the bar and frees its room, that it ends at its time, at the next launch
//! for a session's and with a new version, that the title screen and the warning's own page show
//! the bar whatever the mute, and that the bar's Mute, its choices and the Settings row are
//! clickable, not only drawn (synthetic pointer clicks through egui, headless).
//!
//! Each was seen red first, 2026-11-02 (the list is in the commit that added them).

use std::cell::RefCell;
use std::collections::HashMap;

use super::*;
use crate::gui::bottom_strip::{draw_at, height, room_above, DETAILS_SLUG};
use crate::gui::theme::Theme;

thread_local! {
    /// Where each control of the mute was laid out on the last frame, by name (a test has no
    /// cursor, and egui has no query by content).
    static RECTS: RefCell<HashMap<String, egui::Rect>> = RefCell::new(HashMap::new());
}

/// Note where a control was laid out (called by the drawing code under `cfg(test)`).
pub(crate) fn record(name: &str, rect: egui::Rect) {
    RECTS.with(|r| r.borrow_mut().insert(name.to_string(), rect));
}

fn rect_of(name: &str) -> Option<egui::Rect> {
    RECTS.with(|r| r.borrow().get(name).copied())
}

fn forget_rects() {
    RECTS.with(|r| r.borrow_mut().clear());
}

const V: &str = "0.1547.0";
const NOW: i64 = 1_800_000_000;
const SCREEN: egui::Rect = egui::Rect { min: egui::pos2(0.0, 0.0), max: egui::pos2(1280.0, 720.0) };

fn choice(id: &str) -> &'static Choice {
    choices().iter().find(|c| c.id == id).unwrap_or_else(|| panic!("data/gui/wip_mute.json has no {id:?}"))
}

/// A headless egui context with the app's fonts and theme.
fn context(theme: &Theme) -> egui::Context {
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    theme.apply_to_egui(&ctx);
    ctx
}

/// One frame with these events.
fn frame(ctx: &egui::Context, events: Vec<egui::Event>, build: &mut dyn FnMut(&egui::Context)) {
    let input = egui::RawInput { screen_rect: Some(SCREEN), events, ..Default::default() };
    let _ = ctx.run(input, |ctx| build(ctx));
}

/// A primary click at `pos`: the pointer moves there, then presses and releases, each in its own
/// frame (the sequence a re-laid-out widget needs to take the click).
fn click(ctx: &egui::Context, pos: egui::Pos2, build: &mut dyn FnMut(&egui::Context)) {
    let m = egui::Modifiers::default();
    frame(ctx, vec![egui::Event::PointerMoved(pos)], build);
    frame(ctx, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }], build);
    frame(ctx, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }], build);
}

/// Where the page ends above the bottom strip, after drawing it for `state` at `now`, and the
/// strip's height as `height` reports it: (page bottom, height, room_above's bottom).
fn room(theme: &Theme, state: &mut GuiState, version: &str, now: i64) -> (f32, f32, f32) {
    let ctx = context(theme);
    let (mut bottom, mut h, mut above) = (0.0, 0.0, 0.0);
    for _ in 0..2 {
        frame(&ctx, Vec::new(), &mut |ctx| {
            draw_at(ctx, theme, state, version, now);
            bottom = ctx.available_rect().bottom();
            h = height(ctx);
            above = room_above(ctx).bottom();
        });
    }
    (bottom, h, above)
}

fn on_page(page: GuiPage) -> GuiState {
    let mut state = GuiState::default();
    state.active_page = page;
    state.onboarding_complete = true;
    state
}

/// The rule is the website's: shell.js `hosWipMuted`, run on the same list of cases by
/// scripts/tests/wip-mute-web.test.js.
///
/// Seen red 2026-11-02 with `muted` ignoring the version: "this session, but a new version started".
#[test]
fn the_rule_is_the_one_the_website_runs() {
    let text = std::fs::read_to_string("scripts/tests/fixtures/wip-mute-cases.json").expect("read the shared cases");
    let list: serde_json::Value = serde_json::from_str(&text).expect("the cases are JSON");
    let version = list["version"].as_str().expect("the cases name the running version");
    let cases = list["cases"].as_array().expect("the cases are a list");
    assert!(cases.len() >= 10, "the shared cases are nearly empty");
    for case in cases {
        // What the app reads back from config.json: a record it cannot read is no mute.
        let mute = from_record(&case["mute"]);
        let now = case["now"].as_i64().expect("now is a number");
        let want = case["muted"].as_bool().expect("muted is true or false");
        assert_eq!(muted(mute.as_ref(), version, now), want, "{}", case["why"]);
    }
}

/// The choices ship, in the operator's order, and every word they name is a key English has; a
/// choice that is neither this session nor a time is refused, so it cannot make a mute that never
/// ends.
///
/// Seen red 2026-11-02 with the day's `seconds` set to 0 in data/: "the choice "day" must be either
/// `session` or a number of `seconds` above 0".
#[test]
fn the_shipped_choices_load_and_every_word_is_a_key() {
    for (whose, text) in [("data/", std::fs::read_to_string("data/gui/wip_mute.json").expect("read")), ("the exe's", humanity_core::embedded_data::WIP_MUTE_JSON.to_string())] {
        let file = File::parse(&text).unwrap_or_else(|e| panic!("{whose} wip_mute.json: {e}"));
        let ids: Vec<&str> = file.mute_for.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["session", "day", "week"], "{whose} copy");
        for c in &file.mute_for {
            assert!(humanity_core::i18n::has(&c.label), "{} label key {}", c.id, c.label);
            assert!(humanity_core::i18n::has(&c.hint), "{} hint key {}", c.id, c.hint);
        }
    }
    assert_eq!(choice("day").seconds, Some(86_400));
    assert_eq!(choice("week").seconds, Some(604_800));
    assert!(choice("session").session);
    for bad in [r#"{"mute_for":[{"id":"x","label":"a","hint":"b"}]}"#, r#"{"mute_for":[{"id":"x","session":true,"seconds":60,"label":"a","hint":"b"}]}"#, r#"{"mute_for":[{"id":"x","seconds":0,"label":"a","hint":"b"}]}"#] {
        assert!(File::parse(bad).is_err(), "{bad} must be refused");
    }
}

/// Muted, the bar draws nothing and takes no room: the page reaches the bottom of the window,
/// `height` is 0 and `room_above` is the whole screen, so nothing anchored to the bottom is left
/// standing on a gap. Not muted, it takes its line.
///
/// Seen red 2026-11-02 with draw_at ignoring the mute: "a day's mute hid nothing".
#[test]
fn a_mute_hides_the_bar_and_frees_its_room() {
    let theme = crate::gui::theme::load_theme();
    let mut state = on_page(GuiPage::Humanity);
    let (bottom, h, above) = room(&theme, &mut state, V, NOW);
    assert!(bottom < SCREEN.bottom() - 10.0 && h > 10.0, "not muted, the bar takes its line (page ends at {bottom}, height {h})");
    assert!((above - bottom).abs() < 0.5, "room_above is the page's room");
    state.wip_mute = Some(mute_for(choice("day"), V, NOW));
    let (bottom, h, above) = room(&theme, &mut state, V, NOW + 60);
    assert_eq!(bottom, SCREEN.bottom(), "a day's mute hid nothing: the page ends at {bottom}");
    assert_eq!(h, 0.0, "muted, nothing may stand on the bar's height");
    assert_eq!(above, SCREEN.bottom(), "muted, room_above is the whole screen");
}

/// A day's mute lasts a day, a week's a week: hidden the second before, back at its time.
///
/// Seen red 2026-11-02 with `muted` comparing `now <= until`: "day: back at the day's end".
#[test]
fn a_mute_ends_at_its_time() {
    let theme = crate::gui::theme::load_theme();
    for (id, secs) in [("day", 86_400), ("week", 604_800)] {
        let mut state = on_page(GuiPage::Inventory);
        state.wip_mute = Some(mute_for(choice(id), V, NOW));
        assert_eq!(room(&theme, &mut state, V, NOW + secs - 1).1, 0.0, "{id}: still muted the second before its end");
        assert!(room(&theme, &mut state, V, NOW + secs).1 > 10.0, "{id}: back at the {id}'s end");
    }
}

/// A session's mute is never written to config.json, so the next launch (a fresh GuiState with the
/// saved config applied, as lib.rs boots) shows the bar; a day's mute is kept and still holds.
///
/// Seen red 2026-11-02 with `kept` keeping every mute: "a session's mute is not kept".
#[test]
fn a_session_mute_ends_at_the_next_launch_and_a_days_does_not() {
    let theme = crate::gui::theme::load_theme();
    let relaunch = |state: &GuiState| -> GuiState {
        let text = serde_json::to_string(&crate::config::AppConfig::from_gui_state(state)).expect("config serializes");
        let config: crate::config::AppConfig = serde_json::from_str(&text).expect("config reads back");
        let mut fresh = on_page(GuiPage::Humanity);
        config.apply_to_gui_state(&mut fresh);
        fresh.active_page = GuiPage::Humanity;
        fresh
    };
    let mut state = on_page(GuiPage::Humanity);
    state.wip_mute = Some(mute_for(choice("session"), V, NOW));
    assert_eq!(room(&theme, &mut state, V, NOW + 3600).1, 0.0, "muted for the session it was chosen in");
    let mut next = relaunch(&state);
    assert_eq!(next.wip_mute, None, "a session's mute is not kept");
    assert!(room(&theme, &mut next, V, NOW + 3600).1 > 10.0, "the next launch shows the bar");
    // A hand-written session mute in config.json is not taken either.
    let hand: crate::config::AppConfig = serde_json::from_str(&format!(r#"{{"wip_mute":{{"version":"{V}"}}}}"#)).expect("reads");
    let mut fresh = on_page(GuiPage::Humanity);
    hand.apply_to_gui_state(&mut fresh);
    assert_eq!(fresh.wip_mute, None, "a session mute found in config.json is not taken");

    state.wip_mute = Some(mute_for(choice("day"), V, NOW));
    let mut next = relaunch(&state);
    assert_eq!(next.wip_mute, Some(WipMute { until: Some(NOW + 86_400), version: V.into() }), "a day's mute is kept");
    assert_eq!(room(&theme, &mut next, V, NOW + 3600).1, 0.0, "and still holds after a relaunch");
}

/// A damaged or hand-edited `wip_mute` in config.json is no mute, and never makes the whole file
/// fail to load (a file that does not parse loads as the defaults, every other setting lost).
///
/// Seen red 2026-11-02 with the field read by a plain derive: ""tomorrow": the config failed to load
/// whole".
#[test]
fn a_damaged_mute_record_is_no_mute_and_costs_no_other_setting() {
    for bad in [r#""tomorrow""#, r#"[1800604800, "0.1547.0"]"#, r#"{"until": "tomorrow", "version": "0.1547.0"}"#, r#"{"until": 1800604800}"#, "7"] {
        let text = format!(r#"{{"fov": 61.5, "wip_mute": {bad}}}"#);
        let config: crate::config::AppConfig = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{bad}: the config failed to load whole: {e}"));
        assert_eq!(config.fov, 61.5, "{bad}: the other settings survive");
        assert_eq!(config.wip_mute, None, "{bad} is no mute");
    }
    let good: crate::config::AppConfig = serde_json::from_str(r#"{"wip_mute": {"until": 1800604800, "version": "0.1547.0"}}"#).expect("reads");
    assert_eq!(good.wip_mute, Some(WipMute { until: Some(1_800_604_800), version: "0.1547.0".into() }));
}

/// A mute holds on the version it was made on: the first launch of a new version shows the bar,
/// a week's mute or not.
///
/// Seen red 2026-11-02 with `muted` ignoring the version: "a new version shows the bar again".
#[test]
fn a_new_version_shows_the_bar_again() {
    let theme = crate::gui::theme::load_theme();
    for id in ["session", "day", "week"] {
        let mut state = on_page(GuiPage::Humanity);
        state.wip_mute = Some(mute_for(choice(id), "0.1546.2", NOW));
        assert!(room(&theme, &mut state, V, NOW + 60).1 > 10.0, "{id}: a new version shows the bar again");
    }
}

/// The title screen (the operator, 2026-11-02: "It should stay on the title. I think it helps keep
/// it honest.") and the warning's own page in the Library show the bar whatever the mute; any other
/// Library document does not.
///
/// Seen red 2026-11-02 with always_here answering false: "the title screen shows it while muted".
#[test]
fn the_title_screen_and_the_warnings_own_page_show_it_while_muted() {
    let theme = crate::gui::theme::load_theme();
    let mute = Some(mute_for(choice("week"), V, NOW));
    for setup_done in [false, true] {
        let mut title = on_page(GuiPage::MainMenu);
        title.onboarding_complete = setup_done;
        title.wip_mute = mute.clone();
        assert!(room(&theme, &mut title, V, NOW + 60).1 > 10.0, "the title screen shows it while muted (setup done: {setup_done})");
    }
    let mut lib = on_page(GuiPage::Library);
    lib.library = crate::gui::load_library(std::path::Path::new("data")).sections;
    lib.wip_mute = mute.clone();
    assert!(crate::gui::pages::library::show_doc(&lib, DETAILS_SLUG), "the Library ships the warning's page");
    assert!(room(&theme, &mut lib, V, NOW + 60).1 > 10.0, "the warning's own page shows it while muted");
    let other = lib.library.iter().flat_map(|s| s.categories.iter()).flat_map(|c| c.entries.iter()).find(|e| e.slug != DETAILS_SLUG).map(|e| e.slug.clone()).expect("another document");
    assert!(crate::gui::pages::library::show_doc(&lib, &other));
    assert_eq!(room(&theme, &mut lib, V, NOW + 60).1, 0.0, "another Library document ({other}) is muted like any page");
}

/// The bar's Mute opens its menu, a choice in it mutes (the bar is gone the next frame, its room
/// with it), and on the title screen, where it still shows, Mute reads Muted and Show it again in its
/// menu brings it back: each a real click, headless.
///
/// Seen red 2026-11-02 with the menu's choices drawn but their clicks not acted on: "For one day
/// muted the bar for a day".
#[test]
fn the_bars_mute_and_its_choices_take_clicks() {
    crate::config::keep_saves_off_disk();
    let theme = crate::gui::theme::load_theme();
    let ctx = context(&theme);
    let mut state = on_page(GuiPage::Humanity);
    let mut build = |ctx: &egui::Context| draw_at(ctx, &theme, &mut state, V, NOW);
    forget_rects();
    frame(&ctx, Vec::new(), &mut build);
    frame(&ctx, Vec::new(), &mut build);
    let mute = rect_of("mute").expect("the bar drew its Mute");
    assert!(SCREEN.contains(mute.center()), "Mute is on the screen: {mute:?}");
    assert!(rect_of("day").is_none(), "the menu is closed until Mute is clicked");
    click(&ctx, mute.center(), &mut build);
    frame(&ctx, Vec::new(), &mut build);
    let day = rect_of("day").expect("clicking Mute opened its menu");
    assert!(day.bottom() <= mute.top() + 0.5, "the menu opens above the bar ({day:?} over {mute:?})");
    for id in ["session", "week"] {
        assert!(rect_of(id).is_some(), "the menu offers {id}");
    }
    click(&ctx, day.center(), &mut build);
    drop(build);
    assert_eq!(state.wip_mute, Some(WipMute { until: Some(NOW + 86_400), version: V.into() }), "For one day muted the bar for a day");
    let mut h = -1.0;
    frame(&ctx, Vec::new(), &mut |ctx| {
        draw_at(ctx, &theme, &mut state, V, NOW);
        h = height(ctx);
    });
    assert_eq!(h, 0.0, "the bar is gone, and its room with it");

    // The title screen still shows it: its Mute reads Muted, and Show it again unmutes.
    state.active_page = GuiPage::MainMenu;
    let ctx = context(&theme);
    let mut build = |ctx: &egui::Context| draw_at(ctx, &theme, &mut state, V, NOW + 60);
    forget_rects();
    frame(&ctx, Vec::new(), &mut build);
    frame(&ctx, Vec::new(), &mut build);
    let muted_label = rect_of("mute").expect("the title screen's bar has its Mute");
    click(&ctx, muted_label.center(), &mut build);
    frame(&ctx, Vec::new(), &mut build);
    let again = rect_of("show_again").expect("muted, the menu offers Show it again");
    click(&ctx, again.center(), &mut build);
    drop(build);
    assert_eq!(state.wip_mute, None, "Show it again unmuted it");
}

/// Settings > Notifications > Muted says the bar is muted and until when, and Show it again there
/// brings it back; unmuted, its Mute offers the same choices, and one of them mutes. Real clicks.
///
/// Seen red 2026-11-02 with the row's Show it again drawn but not acted on.
#[test]
fn the_settings_row_shows_until_when_and_both_ways_take_clicks() {
    crate::config::keep_saves_off_disk();
    let theme = crate::gui::theme::load_theme();
    let ctx = context(&theme);
    let mut state = on_page(GuiPage::Settings);
    state.wip_mute = Some(mute_for(choice("week"), V, NOW));
    assert!(muted_words(state.wip_mute.as_ref().unwrap()).starts_with("Muted until 20"), "the row says until when: {}", muted_words(state.wip_mute.as_ref().unwrap()));
    assert_eq!(muted_words(&mute_for(choice("session"), V, NOW)), "Muted until HumanityOS is closed");
    let mut build = |ctx: &egui::Context| {
        egui::CentralPanel::default().show(ctx, |ui| settings_row_at(ui, &theme, &mut state, V, NOW + 60));
    };
    forget_rects();
    frame(&ctx, Vec::new(), &mut build);
    frame(&ctx, Vec::new(), &mut build);
    assert!(rect_of("settings_mute").is_none(), "muted, the row offers Show it again, not Mute");
    let again = rect_of("settings_show_again").expect("muted, the row has Show it again");
    click(&ctx, again.center(), &mut build);
    frame(&ctx, Vec::new(), &mut build);
    frame(&ctx, Vec::new(), &mut build);
    let mute = rect_of("settings_mute").expect("unmuted, the row offers Mute");
    click(&ctx, mute.center(), &mut build);
    frame(&ctx, Vec::new(), &mut build);
    let week = rect_of("settings_week").expect("the row's Mute opened its choices");
    click(&ctx, week.center(), &mut build);
    drop(build);
    assert_eq!(state.wip_mute, Some(WipMute { until: Some(NOW + 60 + 604_800), version: V.into() }), "For one week, chosen in Settings, muted it");
    // On 1.0 the row is gone with the warning.
    let mut drew = false;
    let ctx = context(&theme);
    forget_rects();
    frame(&ctx, Vec::new(), &mut |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let before = ui.cursor().top();
            settings_row_at(ui, &theme, &mut state, "1.0.0", NOW);
            drew = ui.cursor().top() > before;
        });
    });
    assert!(!drew, "on 1.0 there is no warning, so no row about muting it");
}
