// Child of pages/apps.rs: the headless tests' shared helpers (drawing the page whole, finding the
// text it drew, clicking it), and the tests of the page as a whole (2026-10-19): its six parts each
// in its colour and with its name, a name drawn once on a short tile, Epic's covers only when the
// person turns them on, and HumanityOS's own tools opening their pages. The games' tests are in
// games_tests.rs and game_detail_tests.rs, the programs' and the help services' in
// programs_tests.rs. No window, no store, no program: in a test build `games::launch` and Epic's
// cover fetch only record what they were asked, and winget is never run. Each test was seen
// failing once on purpose; what it saw red is at the test.

use std::path::PathBuf;

use super::*;
use crate::games::fixtures;
use crate::games::launch::recorder;
use crate::games::AddedApp;
use crate::gui::screen_surface::find_text_in_shapes;

pub(crate) fn ctx_and_theme() -> (egui::Context, Theme) {
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    ctx.all_styles_mut(|s| s.animation_time = 0.0);
    (ctx, theme)
}

/// One frame of the page, tall enough that every row is on screen, `later` seconds after the last.
pub(crate) fn frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, later: f64, events: Vec<egui::Event>) -> egui::FullOutput {
    frame_sized(ctx, theme, state, later, events, egui::vec2(1400.0, 1600.0))
}

pub(crate) fn frame_sized(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, later: f64, events: Vec<egui::Event>, size: egui::Vec2) -> egui::FullOutput {
    let time = Some(ctx.input(|i| i.time) + later);
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)), time, events, ..Default::default() };
    ctx.run(input, |ctx| draw(ctx, theme, state))
}

pub(crate) fn settled(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) -> egui::FullOutput {
    for _ in 0..3 {
        frame(ctx, theme, state, 0.0, Vec::new());
    }
    frame(ctx, theme, state, 0.0, Vec::new())
}

/// Draw until both workers' reads are in (they read scratch folders, so they are quick).
pub(crate) fn read_in(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    for _ in 0..1000 {
        frame(ctx, theme, state, 0.0, Vec::new());
        let games = state.apps.games.found.is_some() && state.apps.games.reading.is_none();
        let programs = state.apps.programs.found.is_some() && state.apps.programs.reading.is_none();
        if games && programs {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("the reads never came in");
}

/// Every text drawn, with where.
pub(crate) fn texts(out: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
    fn visit(shape: &egui::Shape, clip: egui::Rect, out: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Text(t) => {
                let rect = t.galley.rect.translate(t.pos.to_vec2()).intersect(clip);
                if rect.is_positive() {
                    out.push((t.galley.text().to_string(), rect));
                }
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| visit(s, clip, out)),
            _ => {}
        }
    }
    let mut out_texts = Vec::new();
    for cs in &out.shapes {
        visit(&cs.shape, cs.clip_rect, &mut out_texts);
    }
    out_texts
}

/// How many times exactly `text` is drawn.
pub(crate) fn count(out: &egui::FullOutput, text: &str) -> usize {
    texts(out).iter().filter(|(t, _)| t == text).count()
}

pub(crate) fn exact(out: &egui::FullOutput, text: &str) -> bool {
    count(out, text) > 0
}

/// Where exactly `text` is drawn (the first).
pub(crate) fn at(out: &egui::FullOutput, text: &str) -> Option<egui::Rect> {
    texts(out).into_iter().find(|(t, _)| t == text).map(|(_, r)| r)
}

/// The `button` drawn on the same line as the text `row` (the nearest one on that line).
pub(crate) fn in_row(out: &egui::FullOutput, row: &str, button: &str) -> Option<egui::Rect> {
    let y = at(out, row)?.center().y;
    texts(out).into_iter().filter(|(t, r)| t == button && (r.center().y - y).abs() < 14.0).map(|(_, r)| r).next()
}

/// The `button` on the tile whose name is `name`: below the name (on its cover, or the line under
/// its picture), in the same column, the nearest.
pub(crate) fn on_tile(out: &egui::FullOutput, name: &str, button: &str) -> Option<egui::Rect> {
    let n = at(out, name)?;
    texts(out)
        .into_iter()
        .filter(|(t, r)| t == button && r.center().y > n.center().y && r.center().y - n.center().y < 120.0 && r.left() >= n.left() - 30.0 && r.left() < n.left() + 120.0)
        .map(|(_, r)| r)
        .min_by(|a, b| a.center().y.total_cmp(&b.center().y))
}

/// Move the pointer to `pos`, press and release there; the release frame's output (what the page
/// asked of the platform that frame, a web page to open say).
pub(crate) fn click_at(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, pos: egui::Pos2) -> egui::FullOutput {
    let m = egui::Modifiers::default();
    // A second after the last click, as a person clicks: two clicks closer than that are one
    // double-click to egui.
    frame(ctx, theme, state, 1.0, vec![egui::Event::PointerMoved(pos)]);
    frame(ctx, theme, state, 0.0, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
    frame(ctx, theme, state, 0.0, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }])
}

pub(crate) fn click(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, text: &str) -> egui::FullOutput {
    let out = settled(ctx, theme, state);
    let pos = find_text_in_shapes(&out.shapes, text).unwrap_or_else(|| panic!("{text:?} is drawn")).rect.center();
    click_at(ctx, theme, state, pos)
}

pub(crate) fn click_in_row(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, row: &str, button: &str) {
    let out = settled(ctx, theme, state);
    let pos = in_row(&out, row, button).unwrap_or_else(|| panic!("{button:?} is drawn on {row:?}'s line")).center();
    click_at(ctx, theme, state, pos);
}

/// The web pages a frame asked egui to open (egui 0.31 sends them as output commands; the app's
/// window opens them in the browser), each with whether in a new tab.
pub(crate) fn opened(out: &egui::FullOutput) -> Vec<(String, bool)> {
    out.platform_output.commands.iter().filter_map(|c| if let egui::OutputCommand::OpenUrl(o) = c { Some((o.url.clone(), o.new_tab)) } else { None }).collect()
}

/// An app on the Apps page, reading `fx`'s stores (and nothing of this computer's), with `added`
/// added by hand; no catalog and none of HumanityOS's own tools (their parts are then left out), so
/// the page holds the games and what was added. Nothing a test does writes the person's
/// config.json.
pub(crate) fn page_with(fx: &fixtures::AllFixture, added: Vec<AddedApp>) -> GuiState {
    crate::config::keep_saves_off_disk();
    let mut state = GuiState::default();
    state.active_page = GuiPage::Apps;
    state.apps.games.sources = games::Where::These(fx.sources());
    state.settings.apps_added = added;
    state
}

/// Open `name`'s details: a click on its name.
pub(crate) fn open_details(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, name: &str) {
    click(ctx, theme, state, name);
    assert!(state.apps.selected_game().is_some_and(|g| g.name == name), "{name}'s details are open: {:?}", state.apps.selected);
}

/// A program to add, in a scratch folder: "Dwarf Fortress.exe".
pub(crate) fn a_program(tag: &str) -> (crate::test_temp::TempPath, PathBuf) {
    let dir = crate::test_temp::dir(tag);
    let exe = dir.join("Dwarf Fortress.exe");
    std::fs::write(&exe, "x").unwrap();
    (dir, exe)
}

/// The whole page: the games' fixture stores, an added program, the shipped catalog (with the
/// programs' fixture records) and the shipped list of HumanityOS's own tools.
fn whole_page(tag: &str) -> (fixtures::AllFixture, programs_tests::Fixture, crate::test_temp::TempPath, GuiState) {
    let fx = fixtures::all(tag);
    let pfx = programs_tests::fixture(&format!("{tag}_programs"));
    let (dir, exe) = a_program(&format!("{tag}_added"));
    let mut state = page_with(&fx, vec![AddedApp { name: "Dwarf Fortress Classic".into(), exe }]);
    state.external_catalog = programs_tests::shipped_catalog();
    state.apps.programs.sources = programs::Where::These(pfx.sources.clone());
    state.apps.own_tools = own_tools::shipped();
    (fx, pfx, dir, state)
}

/// Whether a frame painted a rectangle filled with exactly `fill`.
fn painted(out: &egui::FullOutput, fill: egui::Color32) -> bool {
    fn walk(s: &egui::Shape, want: egui::Color32) -> bool {
        match s {
            egui::Shape::Rect(r) => r.fill == want,
            egui::Shape::Vec(v) => v.iter().any(|x| walk(x, want)),
            _ => false,
        }
    }
    out.shapes.iter().any(|c| walk(&c.shape, fill))
}

/// THE SIX PARTS, EACH UNDER ITS NAME AND IN ITS COLOUR, IN THEIR ORDER (2026-10-19, the operator:
/// "Red section could be games. Green could be humanityOS tools. Blue could be third party tools."
/// and "what if help is C and services is M and then we still have Y"): Games, HumanityOS tools,
/// Other programs, Help, Services and Added by you, top to bottom, each band painted in its own
/// theme colour; a chip each in the toolbar with how many it holds; and a chip narrows the page to
/// its part.
///
/// Seen red 2026-10-19 with the yellow part left out of `parts` (its arm drawing nothing): "Added by
/// you's band is drawn" failed.
#[test]
fn six_parts_each_under_its_name_and_in_its_colour() {
    let (ctx, theme) = ctx_and_theme();
    let (_fx, _pfx, _dir, mut state) = whole_page("ap_six");
    read_in(&ctx, &theme, &mut state);
    // Tall enough for every part.
    for _ in 0..3 {
        frame_sized(&ctx, &theme, &mut state, 0.0, Vec::new(), egui::vec2(1400.0, 9000.0));
    }
    let out = frame_sized(&ctx, &theme, &mut state, 0.0, Vec::new(), egui::vec2(1400.0, 9000.0));
    let mut last = f32::MIN;
    for s in Section::ALL {
        // The band's title is the first of its words drawn below the toolbar's chips.
        let y = texts(&out).into_iter().filter(|(t, _)| t == s.words()).map(|(_, r)| r.center().y).max_by(f32::total_cmp);
        let y = y.unwrap_or_else(|| panic!("{}'s band is drawn", s.words()));
        assert!(y > last, "{} comes after the part before it", s.words());
        last = y;
        assert!(painted(&out, crate::gui::widgets::tint_section::color(&theme, s.tint())), "{}'s band is in its colour", s.words());
        assert!(exact(&out, &format!("{} {}", s.words(), library::count(&state, s))), "{}'s chip, with how many", s.words());
    }
    assert!(exact(&out, "Added by you 1") && exact(&out, "Games 8"), "the games and what was added are counted apart");
    let help = format!("Help {}", library::count(&state, Section::Help));
    click(&ctx, &theme, &mut state, &help);
    assert_eq!(state.settings.apps_view.section, Some(Section::Help));
    let out = settled(&ctx, &theme, &mut state);
    assert!(exact(&out, "Crisis Text Line") && !exact(&out, "Feeding America") && !exact(&out, "Orbital Farmstead"), "the Help chip shows Help alone");
}

/// A NAME IS DRAWN ONCE, AND A TILE IS SHORT (the operator, 2026-10-19: "the app tiles are too big.
/// The titles are stated twice, we don't need that."): a game with no picture (Sunken Archive) has
/// its name on its cover only; one whose picture is in (Orbital Farmstead) has it once, under the
/// picture; a tile is its cover and at most three short lines (about a third shorter than the
/// tiles before); and five or more fit in a row of the page at 1400 wide.
///
/// Seen red 2026-10-19 with the name line drawn under every tile (`has_art` always true in
/// `game_tile`): "Sunken Archive's name, once" (left: 2, right: 1).
#[test]
fn a_name_is_drawn_once_and_a_tile_is_short() {
    let (ctx, theme) = ctx_and_theme();
    let fx = fixtures::all("ap_once");
    let mut state = page_with(&fx, Vec::new());
    read_in(&ctx, &theme, &mut state);
    let textured = |out: &egui::FullOutput| out.shapes.iter().filter(|cs| matches!(&cs.shape, egui::Shape::Rect(r) if r.brush.is_some())).count();
    let mut out = settled(&ctx, &theme, &mut state);
    for _ in 0..300 {
        if textured(&out) >= 3 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        out = frame(&ctx, &theme, &mut state, 0.0, Vec::new());
    }
    assert_eq!(count(&out, "Sunken Archive"), 1, "Sunken Archive's name, once");
    assert_eq!(count(&out, "Orbital Farmstead"), 1, "Orbital Farmstead's, once, under its picture");
    // A tile's card: the rectangles filled in the card colour as wide as a tile.
    let cards: Vec<egui::Rect> = out
        .shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            egui::Shape::Rect(r) if r.fill == theme.bg_card() && r.rect.width() > 120.0 && r.rect.width() < 320.0 => Some(r.rect),
            _ => None,
        })
        .collect();
    assert!(!cards.is_empty(), "the tiles' cards are drawn");
    let tallest = cards.iter().map(|r| r.height()).fold(0.0, f32::max);
    let width = cards[0].width();
    let cover = (width / library::COVER_ASPECT).round();
    let line = theme.font_size_small + 2.0 * theme.button_pad_y + 2.0;
    assert!(tallest <= cover + 3.0 * line + 4.0 * theme.spacing_sm, "a tile is its cover and three short lines: {tallest} for a cover of {cover}");
    let first_row = cards.iter().filter(|r| (r.top() - cards[0].top()).abs() < 1.0).count();
    assert!(first_row >= 5, "five or more to a row: {first_row}");
}

/// EPIC'S COVERS, ONLY WHEN THE PERSON TURNS THEM ON (2026-10-19; the games finding's judgement
/// call 1 and its dated note): with the switch off (as it starts) nothing is asked of Epic however
/// many frames are drawn; the switch says what it tells Epic; turned on, kept in the settings, the
/// installed Epic game's cover is asked for once, from Epic's own host, decoded into a texture and
/// drawn on its tile (its name then once, under it); turned off, the texture is let go and nothing
/// more is asked. Every request goes through the one recorded fetch.
///
/// Seen red 2026-10-19 with `covers::update` not looking at the switch (fetching whenever there was
/// something to fetch): "switch off, nothing is asked" got ["https://cdn1.epicgames.com/item/ns1/lantern-tall"].
#[test]
fn epic_covers_only_when_turned_on() {
    use crate::games::epic_covers::recorder as epic;
    let (ctx, theme) = ctx_and_theme();
    let fx = fixtures::all("ap_covers");
    let mut state = page_with(&fx, Vec::new());
    let _ = epic::recorded();
    epic::answer_with(None);
    read_in(&ctx, &theme, &mut state);
    for _ in 0..20 {
        frame(&ctx, &theme, &mut state, 0.0, Vec::new());
    }
    assert!(epic::recorded().is_empty(), "switch off, nothing is asked");
    assert!(!state.settings.apps_epic_covers, "off unless turned on");
    let out = settled(&ctx, &theme, &mut state);
    assert!(exact(&out, covers::SWITCH), "the switch, saying what it tells Epic");
    assert_eq!(count(&out, "Lantern Valley"), 1, "no cover yet: its name on its plain cover");

    state.settings_dirty = false;
    click(&ctx, &theme, &mut state, covers::SWITCH);
    assert!(state.settings.apps_epic_covers && state.settings_dirty, "turned on, and kept");
    for _ in 0..10 {
        frame(&ctx, &theme, &mut state, 0.0, Vec::new());
    }
    assert_eq!(epic::recorded(), vec!["https://cdn1.epicgames.com/item/ns1/lantern-tall".to_string()], "its cover, once, from Epic's own host");
    assert_eq!(state.apps.covers.textures.len(), 1, "decoded into memory");
    let out = settled(&ctx, &theme, &mut state);
    assert_eq!(count(&out, "Lantern Valley"), 1, "its name once, under its cover");
    let json = serde_json::to_string(&crate::config::AppConfig::from_gui_state(&state)).unwrap();
    let mut fresh = GuiState::default();
    serde_json::from_str::<crate::config::AppConfig>(&json).unwrap().apply_to_gui_state(&mut fresh);
    assert!(fresh.settings.apps_epic_covers, "the switch survives a restart");

    click(&ctx, &theme, &mut state, covers::SWITCH);
    for _ in 0..10 {
        frame(&ctx, &theme, &mut state, 0.0, Vec::new());
    }
    assert!(!state.settings.apps_epic_covers && state.apps.covers.textures.is_empty(), "turned off, let go");
    assert!(epic::recorded().is_empty(), "and nothing more is asked");
}

/// A FETCHED COVER IS KEPT SMALL, IN MEMORY: a wide picture larger than the page draws is shrunk to
/// 1280 on its longer side, its shape kept; something that is not a picture is refused.
///
/// Seen red 2026-10-19 with the shrink left out of `decode`: "shrunk to 1280 wide" got
/// Some((2560, 1440)).
#[test]
fn a_fetched_cover_is_kept_small() {
    let big = crate::games::epic_covers::tests::png(2560, 1440);
    assert_eq!(covers::decodes(&big), Some((1280, 720)), "shrunk to 1280 wide");
    assert_eq!(covers::decodes(&crate::games::epic_covers::tests::png(8, 4)), Some((8, 4)));
    assert_eq!(covers::decodes(b"<html>not a picture</html>"), None);
}

/// HUMANITYOS'S OWN TOOLS OPEN THEIR PAGES: the green part lists the shipped tools, Maps' tile has
/// Open, and Open goes to the Maps page; a click on a tool's cover opens its details, with its
/// line, and their Open goes to its page too. Nothing is started outside the app.
///
/// Seen red 2026-10-19 with `Act::OpenPage` doing nothing: "Open goes to the Maps page" (left:
/// Apps, right: Maps).
#[test]
fn humanityos_tools_open_their_pages() {
    let (ctx, theme) = ctx_and_theme();
    let fx = fixtures::all("ap_tools");
    let mut state = page_with(&fx, Vec::new());
    state.apps.own_tools = own_tools::shipped();
    state.settings.apps_view.section = Some(Section::Tools);
    read_in(&ctx, &theme, &mut state);
    let out = settled(&ctx, &theme, &mut state);
    for t in ["Maps", "Tasks", "Crafting", "Library", "Studio", "Watch", "Media", "Inventory"] {
        assert!(exact(&out, t), "{t} is listed");
    }
    let open = on_tile(&out, "Maps", own_tools::OPEN).expect("Open on Maps' tile");
    click_at(&ctx, &theme, &mut state, open.center());
    assert_eq!(state.active_page, GuiPage::Maps, "Open goes to the Maps page");

    state.active_page = GuiPage::Apps;
    click(&ctx, &theme, &mut state, "Studio");
    let studio = state.apps.own_tools.iter().position(|t| t.name == "Studio");
    assert_eq!(state.apps.selected, studio.map(Picked::Tool), "its details open");
    let line = state.apps.own_tools[studio.unwrap()].line.clone();
    let out = settled(&ctx, &theme, &mut state);
    assert!(count(&out, &line) >= 2, "its line, on its tile and in its details");
    let opens: Vec<egui::Rect> = texts(&out).into_iter().filter(|(t, _)| t == own_tools::OPEN).map(|(_, r)| r).collect();
    let details_open = opens.iter().max_by(|a, b| a.left().total_cmp(&b.left())).unwrap();
    click_at(&ctx, &theme, &mut state, details_open.center());
    assert_eq!(state.active_page, GuiPage::Studio, "its details' Open goes to its page");
    assert!(recorder::recorded().is_empty(), "nothing started outside the app");
}
