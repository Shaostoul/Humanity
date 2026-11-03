// Child of gui/navigation.rs (#[path]): the menu file's rules, that every desktop page has exactly
// one place in it, and that the desktop's top row is drawn from it. Headless: the row is drawn into
// an egui context with no window, and what it painted (and where a click on it goes) is read back.
// Each test was seen red once; what it saw is at the test.

use super::*;
use crate::gui::theme::load_theme;
use crate::gui::GuiState;

/// The data folder's copy of the file, read and checked.
fn shipped_text() -> String {
    std::fs::read_to_string(Path::new("data").join(FILE)).expect("data/gui/navigation.json is there")
}

fn shipped() -> Menu {
    parse(&shipped_text()).unwrap_or_else(|e| panic!("data/{FILE} reads: {e}"))
}

/// THE FILE READS, BOTH COPIES: the data folder's and the one built into the exe (the menu a data
/// folder without the file falls back to) pass every rule, so the menu is never empty. Named for
/// `just validate-data`, which runs the tests called `shipped` and `parses`.
///
/// Seen red 2026-11-03 with the own-life category's hint written `nav.own_life_hint` (a key
/// i18n-add.js had not added): "data/gui/navigation.json reads: the entry `own_life`: its hint
/// `nav.own_life_hint` is not a key in data/i18n/en.json".
#[test]
fn the_shipped_navigation_file_parses() {
    let disk = shipped();
    let exe = parse(humanity_core::embedded_data::NAVIGATION_JSON).unwrap_or_else(|e| panic!("the exe's copy reads: {e}"));
    for (whose, m) in [("data/", &disk), ("the exe's", &exe)] {
        assert_eq!(m.file.categories.len(), 8, "{whose}: the design's eight categories");
        assert!(!m.desktop_row.is_empty(), "{whose}: today's desktop row has buttons");
    }
    let ids: Vec<&str> = disk.file.categories.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["humanity", "play", "chat", "own_life", "apps", "library", "dev", "settings"], "in the design's order (section 3)");
    // The own-life category's name is open (16.1): its id stays neutral and its words are one key.
    let own = &disk.file.categories[3];
    assert_eq!((own.id.as_str(), own.label.as_str()), ("own_life", "nav.own_life"));
}

/// NONE LOST, NONE TWICE: every desktop page (`dispatch::ALL_PAGES`, the world included) is in
/// exactly one place of the file as a whole page: a category, an item, or `outside` with its
/// reason. A part of a page (`section`) is a place of its own and does not count.
///
/// Seen red 2026-11-03 with the Crafting item taken out of the file: "GuiPage::Crafting
/// (`crafting`) is in 0 places of data/gui/navigation.json: []"; and with the file's rules
/// switched off and Crafting written into Apps' row as well: "GuiPage::Crafting (`crafting`) is in
/// 2 places of data/gui/navigation.json: [\"crafting\", \"crafting2\"]" (with the rules on, that
/// file is refused when read: "the entry `crafting2`: the desktop page `crafting` is another
/// entry's already").
#[test]
fn every_desktop_page_is_in_the_menu_file_once() {
    let menu = shipped();
    let places = menu.file.places();
    let mut problems = Vec::new();
    for &page in crate::gui::dispatch::ALL_PAGES {
        let id = crate::gui::dispatch::page_id(page);
        let whole: Vec<&str> = places.iter().filter(|p| p.page == Some(id) && p.section.is_none()).map(|p| p.id).collect();
        if whole.len() != 1 {
            problems.push(format!("GuiPage::{page:?} (`{id}`) is in {} places of data/{FILE}: {whole:?}", whole.len()));
        }
    }
    assert!(problems.is_empty(), "every desktop page in exactly one place:\n  {}", problems.join("\n  "));
}

/// The entry `id` in a file read as JSON (a category, an item, or one outside the rows).
fn entry<'a>(v: &'a mut serde_json::Value, id: &str) -> &'a mut serde_json::Value {
    let mut path = None;
    for (ci, c) in v["categories"].as_array().expect("categories").iter().enumerate() {
        if c["id"] == id {
            path = Some(format!("/categories/{ci}"));
        }
        for (ii, i) in c["items"].as_array().map(|a| a.as_slice()).unwrap_or_default().iter().enumerate() {
            if i["id"] == id {
                path = Some(format!("/categories/{ci}/items/{ii}"));
            }
        }
    }
    for (oi, o) in v["outside"].as_array().expect("outside").iter().enumerate() {
        if o["id"] == id {
            path = Some(format!("/outside/{oi}"));
        }
    }
    let path = path.unwrap_or_else(|| panic!("the file has an entry `{id}`"));
    v.pointer_mut(&path).expect("the entry")
}

/// A MALFORMED ROW FAILS LOUDLY: each of these one-line breakages of the shipped file is refused
/// whole, with words naming the entry and what is wrong (the app then uses the exe's copy at start,
/// or keeps its menu on a save, and says so in the log).
///
/// Seen red 2026-11-03 with the rules switched off (`parse` not calling `check`): only the JSON
/// reader's own refusal (the unknown field) held; "a word key en.json lacks: was read", "a desktop
/// page twice: was read", "a colour that is not a token: was read", and so on for 14 more, and two
/// refused for the wrong reason ("an id twice: refused, but saying \"today's desktop row names
/// `tasks`, which is no entry\"").
#[test]
fn a_malformed_row_fails_loudly() {
    type Break = fn(&mut serde_json::Value);
    let cases: &[(&str, Break, &str)] = &[
        ("a field the file does not know", |v| entry(v, "crafting")["pgae"] = "crafting".into(), "unknown field `pgae`"),
        ("a word key en.json lacks", |v| entry(v, "crafting")["label"] = "nav.craftin".into(), "the entry `crafting`: its label `nav.craftin` is not a key in data/i18n/en.json"),
        ("a page that is not a page", |v| entry(v, "crafting")["page"] = "craft".into(), "the entry `crafting`: `craft` is not a desktop page"),
        ("an id twice", |v| entry(v, "tasks")["id"] = "maps".into(), "the entry `maps` is in the file twice"),
        ("a desktop page twice", |v| entry(v, "tasks")["page"] = "maps".into(), "the desktop page `maps` is another entry's already"),
        ("a website address twice", |v| entry(v, "tasks")["web"] = "/maps".into(), "the website address `/maps` is another entry's already"),
        ("a desktop-only page with an address", |v| entry(v, "studio")["web"] = "/studio".into(), "the entry `studio` is on the desktop only, so it has no website address"),
        ("a website-only page with a desktop page", |v| {
            entry(v, "ops")["page"] = "dev".into();
            entry(v, "ops")["section"] = "ops".into();
        }, "the entry `ops` is on the website only, so it has no desktop page"),
        ("a part with no page", |v| entry(v, "worlds")["section"] = "list".into(), "the entry `worlds`: a section is a part of a page, and it has no page"),
        ("an entry that opens nothing", |v| {
            entry(v, "governance")["page"] = serde_json::Value::Null;
            entry(v, "governance")["web"] = serde_json::Value::Null;
        }, "the entry `governance` opens nothing on either app"),
        ("an address without its slash", |v| entry(v, "tasks")["web"] = "tasks".into(), "the website address `tasks` does not start with /"),
        ("a colour that is not a token", |v| entry(v, "chat")["colour"] = "section_cyann".into(), "the category `chat`: `section_cyann` is not a theme token"),
        ("an outside entry with no reason", |v| entry(v, "hi")["why"] = " ".into(), "the entry `hi` is outside the rows and does not say why"),
        ("today's row naming no entry", |v| v["today_row"]["desktop"][0]["items"][0] = "plya".into(), "today's desktop row names `plya`, which is no entry"),
        ("today's row naming a part of a page", |v| v["today_row"]["desktop"][0]["items"][0] = "claude".into(), "the single row opens whole pages"),
        ("today's row showing one twice", |v| v["today_row"]["desktop"][1]["items"][0] = "play".into(), "today's desktop row shows `play` twice"),
        ("today's website row naming a desktop-only page", |v| v["today_row"]["web"][0]["items"][0] = "studio".into(), "today's website row names `studio`, which has no website address"),
        ("web_active for a tab not in the row", |v| v["today_row"]["web_active"]["studio"] = "studio".into(), "web_active names `studio`"),
    ];
    let base: serde_json::Value = serde_json::from_str(&shipped_text()).expect("the shipped file is JSON");
    let mut let_through = Vec::new();
    for (what, break_it, says) in cases {
        let mut v = base.clone();
        break_it(&mut v);
        match parse(&v.to_string()) {
            Ok(_) => let_through.push(format!("{what}: was read")),
            Err(e) if !e.contains(says) => let_through.push(format!("{what}: refused, but saying {e:?} (expected {says:?})")),
            Err(_) => {}
        }
    }
    assert!(parse("{ \"categories\": [").is_err(), "a file cut short is refused");
    assert!(let_through.is_empty(), "every breakage refused, naming it:\n  {}", let_through.join("\n  "));
}

/// Every text the frame painted inside the top `band` points, in paint order, with where.
fn texts(out: &egui::FullOutput, band: f32) -> Vec<(String, egui::Rect)> {
    fn visit(shape: &egui::Shape, clip: egui::Rect, band: f32, out: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Text(t) => {
                let rect = t.galley.rect.translate(t.pos.to_vec2()).intersect(clip);
                if rect.is_positive() && rect.top() < band {
                    out.push((t.galley.text().to_string(), rect));
                }
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| visit(s, clip, band, out)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    for cs in &out.shapes {
        visit(&cs.shape, cs.clip_rect, band, &mut found);
    }
    found
}

/// One frame of the top menu, wide enough for one row, with `events`.
fn nav_frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, events: Vec<egui::Event>) -> egui::FullOutput {
    let time = Some(ctx.input(|i| i.time) + 0.5);
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(2400.0, 200.0))), time, events, ..Default::default() };
    ctx.run(input, |ctx| crate::gui::pages::escape_menu::draw_nav_bar(ctx, theme, state))
}

/// THE DESKTOP'S ROW IS THE FILE'S (the parity pair's desktop half; the website's half is
/// scripts/tests/navigation-web.test.js): the single top row shows, in order, the words of
/// `today_row.desktop`'s entries (read from the file here, not through the row the module built),
/// a dot between two groups, then the Aa switch; and a click on each button goes where its entry's
/// page says (Characters to the picker).
///
/// Seen red 2026-11-03 with the row still written by hand in escape_menu.rs (where it passed with
/// the shipped file, clicks included) and the file's Quests and Tasks swapped: "the row's words
/// are the file's; left: [\"Play\", ... \"Home\", \"·\", \"Quests\", \"·\", \"Tasks\", ...] right:
/// [\"Play\", ... \"Home\", \"·\", \"Tasks\", \"·\", \"Quests\", ...]". Drawn from the file, the same
/// swap passes.
#[test]
fn the_desktop_row_is_drawn_from_the_file() {
    let menu = super::menu();
    let file = &menu.file;
    let places = file.places();
    let place = |id: &str| *places.iter().find(|p| p.id == id).unwrap_or_else(|| panic!("an entry `{id}`"));
    let mut want: Vec<String> = Vec::new();
    for (gi, g) in file.today_row.desktop.iter().enumerate() {
        if gi > 0 {
            want.push("·".to_string());
        }
        for id in &g.items {
            want.push(humanity_core::i18n::t_dyn(place(id).label.expect("words")).to_string());
        }
    }
    // Read with the buttons' words only (no icons): Humanity's icon is a painted "H", which is not
    // one of the row's words. The look with icons is the snapshots' (nav_bar_full.png).
    let mode = crate::gui::NavDisplayMode::TextOnly;
    want.push(mode.glyph().to_string());

    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    let theme = load_theme();
    theme.apply_to_egui(&ctx);
    ctx.all_styles_mut(|s| s.animation_time = 0.0);
    let mut state = GuiState::default();
    state.nav_display_mode = mode;
    nav_frame(&ctx, &theme, &mut state, Vec::new());
    let out = nav_frame(&ctx, &theme, &mut state, Vec::new());
    let drawn = texts(&out, 60.0);
    let words: Vec<String> = drawn.iter().map(|(s, _)| s.clone()).collect();
    assert_eq!(words, want, "the row's words are the file's");

    // A click on each button opens its entry's page.
    for g in &file.today_row.desktop {
        for id in &g.items {
            let p = place(id);
            let label = humanity_core::i18n::t_dyn(p.label.expect("words"));
            let page = p.page.and_then(crate::gui::dispatch::page_from_id).expect("a desktop page");
            state.active_page = GuiPage::MainMenu;
            state.launcher_open_select = false;
            let out = nav_frame(&ctx, &theme, &mut state, Vec::new());
            let at = texts(&out, 60.0).into_iter().find(|(s, _)| s == label).unwrap_or_else(|| panic!("{label:?} is drawn")).1.center();
            let m = egui::Modifiers::default();
            nav_frame(&ctx, &theme, &mut state, vec![egui::Event::PointerMoved(at)]);
            nav_frame(&ctx, &theme, &mut state, vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
            nav_frame(&ctx, &theme, &mut state, vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
            assert_eq!(state.active_page, page, "{label} opens `{}`", p.page.unwrap_or_default());
            if p.section == Some(PICKER) {
                assert!(state.launcher_open_select, "{label} opens the character picker");
            }
        }
    }
}

/// THE COLOURS THE FILE MAY NAME ARE THE THEME'S: each name the menu knows reads its own token,
/// so a group drawn in `nav_sim` is the theme's purple, whatever the theme says it is.
#[test]
fn colour_names_read_their_tokens() {
    let theme = load_theme();
    assert_eq!(colour_of("nav_sim").map(|f| f(&theme)), Some(theme.nav_sim()));
    assert_eq!(colour_of("section_cyan").map(|f| f(&theme)), Some(theme.section_cyan()));
    assert!(colour_of("nav_simm").is_none(), "a name the menu does not know is none");
    for g in &shipped().file.today_row.desktop {
        assert!(colour_of(&g.colour).is_some(), "today's row's `{}` is a token", g.colour);
    }
}
