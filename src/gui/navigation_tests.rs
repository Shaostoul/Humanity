// Child of gui/navigation.rs (#[path]): the menu file's rules, that every desktop page has exactly
// one place in it, and that the desktop's two rows hold only pages that exist, each once. What the
// rows DRAW, and what a click, a rest or a hold on them does, is pages/escape_menu_tests.rs.
// Each test was seen red once; what it saw is at the test.

use super::*;
use crate::gui::theme::load_theme;

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
/// `nav.own_life_hint` is not a key in data/i18n/en.json". Rung 2 (2026-11-03): seen red with the
/// data file's Play category put back on the world (`page: "none"`, the `world` entry taken out of
/// `outside`), rung 1's Play: "Play's button opens Play's page; left: None, right: Play".
#[test]
fn the_shipped_navigation_file_parses() {
    let disk = shipped();
    let exe = parse(humanity_core::embedded_data::NAVIGATION_JSON).unwrap_or_else(|e| panic!("the exe's copy reads: {e}"));
    for (whose, m) in [("data/", &disk), ("the exe's", &exe)] {
        assert_eq!(m.file.categories.len(), 8, "{whose}: the design's eight categories");
        assert_eq!(m.desktop.len(), 8, "{whose}: the desktop draws all eight");
    }
    let ids: Vec<&str> = disk.file.categories.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["humanity", "play", "chat", "own_life", "apps", "library", "dev", "settings"], "in the design's order (section 3)");
    // The own-life category's name is open (16.1): its id stays neutral and its words are one key,
    // "Life" since rung 2 as a working name.
    let own = &disk.file.categories[3];
    assert_eq!((own.id.as_str(), own.label.as_str()), ("own_life", "nav.own_life"));
    assert_eq!(humanity_core::i18n::t("nav.own_life"), "Life", "the working name (section 16.1)");
    // A tap on Play opens Play's own page (16.1, question 2).
    assert_eq!(disk.desktop[1].button.page, GuiPage::Play, "Play's button opens Play's page");
    assert_eq!(disk.file.timings, Timings { preview_rest_ms: 200, preview_leave_ms: 300, play_hold_ms: 800 });
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
/// entry's already"). Rung 2: seen red with Play's page in the app and the file's Play put back on
/// `none` (the `world` entry out of `outside`): "GuiPage::Play (`play`) is in 0 places of
/// data/gui/navigation.json: []".
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

/// EVERY PAGE IN THE ROWS EXISTS (rung 2; the brief: "none lost, none twice, no button to
/// nothing"): the desktop's two rows, as drawn, are exactly these, each button a page the app
/// draws, each page (or part) once in the rows, and no page the design adds and has not built
/// (`new`) is drawn under another page's name. Every page the rows leave out is reachable another
/// way, which `outside` says (the world through Play, Relay Control from the chat's sidebar).
///
/// Seen red 2026-11-03 with People opening the Directory again (rung 1's stand-in: `page:
/// "identity"` on the `new` entry, the Directory item taken out): "the two rows as drawn; left:
/// [(\"humanity\", [\"people\", \"governance\", \"laws\", \"donate\"]), ...", and with the Directory
/// marked `new`: "a page not built yet is drawn: directory (a stand-in under another page's
/// name)". Both are now refused when the file is read too (a `new` entry with a desktop page:
/// "the entry `people` is a page not built yet (`new`), so it opens no desktop page: `identity` is
/// another page, which would show under its name"), and so is Mission put on the Humanity page's
/// dashboard (`page: "humanity"`): "the entry `mission` is a page not built yet ...".
#[test]
fn every_page_in_the_rows_exists() {
    let menu = shipped();
    let drawn: Vec<(&str, Vec<&str>)> = menu.desktop.iter().map(|c| (c.id.as_str(), c.items.iter().map(|b| b.id.as_str()).collect())).collect();
    let want: Vec<(&str, Vec<&str>)> = vec![
        ("humanity", vec!["directory", "governance", "laws", "donate"]),
        ("play", vec!["characters", "quests", "inventory", "crafting"]),
        ("chat", vec!["watch", "studio", "files", "guilds"]),
        ("own_life", vec!["profile", "tasks", "maps", "wallet", "market", "trade"]),
        ("apps", vec!["notes", "calendar", "calculator", "browser"]),
        ("library", vec!["how_to"]),
        ("dev", vec!["bugs", "testing", "performance", "planet_tuner", "claude", "journal"]),
        ("settings", vec!["server_settings"]),
    ];
    assert_eq!(drawn, want, "the two rows as drawn (docs/design/navigation.md, rung 2)");

    let new: HashSet<&str> = menu.file.places().iter().filter(|p| p.new).map(|p| p.id).collect();
    let mut seen: HashSet<(&str, Option<String>)> = HashSet::new();
    for c in &menu.desktop {
        for b in std::iter::once(&c.button).chain(&c.items) {
            assert!(!new.contains(b.id.as_str()), "a page not built yet is drawn: {} (a stand-in under another page's name)", b.id);
            let page = crate::gui::dispatch::page_id(b.page);
            assert!(seen.insert((page, b.section.clone())), "{} opens a page another button opens: {page} {:?}", b.id, b.section);
            assert!(humanity_core::i18n::has(&b.label) && humanity_core::i18n::has(&b.hint), "{} has its words and its hint", b.id);
            // No button to nothing: the page is one the app draws (the world is Play's way in).
            assert!(b.page == GuiPage::None || crate::gui::dispatch::ALL_PAGES.contains(&b.page), "{} opens a page the app has", b.id);
        }
    }
    // What waits for rung 7, named so a change to it is deliberate.
    let mut waiting: Vec<&str> = new.iter().copied().collect();
    waiting.sort_unstable();
    assert_eq!(waiting, ["devlog", "mission", "people", "roadmap", "rules", "servers", "worlds"]);
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
/// `tasks`, which is no entry\""). Rung 2's cases seen red the same day the same way: "a part the
/// page does not have: was read", "a page not built yet standing in: was read", "a category with
/// no desktop picture: was read", both timings and web_label "was read", and "a category with no
/// page: refused, but saying \"the category `library` has no desktop page\"" (the rows' own
/// resolving, not the rule).
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
            entry(v, "ops")["section"] = "dev".into();
        }, "the entry `ops` is on the website only, so it has no desktop page"),
        ("a part with no page", |v| entry(v, "worlds")["section"] = "list".into(), "the entry `worlds`: a section is a part of a page, and it has no page"),
        ("a part the page does not have", |v| entry(v, "claude")["section"] = "claud".into(), "the entry `claude`: the desktop page `dev` has no part `claud`"),
        ("an entry that opens nothing", |v| {
            entry(v, "governance")["page"] = serde_json::Value::Null;
            entry(v, "governance")["web"] = serde_json::Value::Null;
        }, "the entry `governance` opens nothing on either app"),
        ("an address without its slash", |v| entry(v, "tasks")["web"] = "tasks".into(), "the website address `tasks` does not start with /"),
        ("a page not built yet standing in", |v| {
            entry(v, "people")["page"] = "notes".into();
        }, "the entry `people` is a page not built yet (`new`), so it opens no desktop page"),
        ("a colour that is not a token", |v| entry(v, "chat")["colour"] = "nav_cat_chatt".into(), "the category `chat`: `nav_cat_chatt` is not a theme token"),
        ("a category with no page", |v| entry(v, "library")["page"] = serde_json::Value::Null, "the category `library` opens no whole desktop page"),
        ("a category with no desktop picture", |v| entry(v, "apps")["icon"] = "grid".into(), "the category `apps`: its icon `grid` has no desktop picture"),
        ("a preview that waits a minute", |v| v["timings"]["preview_rest_ms"] = 60000.into(), "timings: `preview_rest_ms` is 60000 ms"),
        ("a hold as short as a click", |v| v["timings"]["play_hold_ms"] = 100.into(), "timings: `play_hold_ms` is 100 ms"),
        ("an outside entry with no reason", |v| entry(v, "hi")["why"] = " ".into(), "the entry `hi` is outside the rows and does not say why"),
        ("today's website row naming a desktop-only page", |v| v["today_row"]["web"][0]["items"][0] = "studio".into(), "today's website row names `studio`, which has no website address"),
        ("web_active for a tab not in the row", |v| v["today_row"]["web_active"]["studio"] = "studio".into(), "web_active names `studio`"),
        ("web_label for a tab not in the row", |v| v["today_row"]["web_label"]["studio"] = "nav.studio".into(), "web_label names `studio`"),
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

/// THE COLOURS THE FILE MAY NAME ARE THE THEME'S: each name the menu knows reads its own token,
/// so a category drawn in `nav_cat_play` is the theme's purple, whatever the theme says it is; and
/// each category names its own (`nav_cat_<id>`), each with its row in Settings (theme_editor_coverage).
///
/// Seen red 2026-11-03 with Chat's colour written `nav_cat_play` in the data file: "the category
/// `chat` names its own token; left: \"nav_cat_play\", right: \"nav_cat_chat\"".
#[test]
fn colour_names_read_their_tokens() {
    let theme = load_theme();
    assert_eq!(colour_of("nav_cat_play").map(|f| f(&theme)), Some(theme.nav_cat_play()));
    assert_eq!(colour_of("nav_cat_chat").map(|f| f(&theme)), Some(theme.nav_cat_chat()));
    assert!(colour_of("nav_cat_playy").is_none(), "a name the menu does not know is none");
    for c in &shipped().file.categories {
        assert_eq!(c.colour, format!("nav_cat_{}", c.id), "the category `{}` names its own token", c.id);
    }
}

/// WHERE A PAGE SITS IN THE ROWS: a category's own page docks its row with nothing marked, an
/// item's page docks its category's row with it marked, the Dev page's parts are the Dev row's
/// items (and its developer's tools the Dev page itself), and a page the rows leave out is in none.
///
/// Seen red 2026-11-03 with `place_of` blind to the Dev page's parts: "left: Some((6, None)),
/// right: Some((6, Some(4)))" (Claude marked as the Dev page itself).
#[test]
fn place_of_finds_each_page_in_its_row() {
    let menu = shipped();
    let cat = |id: &str| menu.desktop.iter().position(|c| c.id == id).unwrap();
    let item = |c: usize, id: &str| menu.desktop[c].items.iter().position(|b| b.id == id).unwrap();
    let (dev, play, humanity) = (cat("dev"), cat("play"), cat("humanity"));
    assert_eq!(menu.place_of(GuiPage::Play, ""), Some((play, None)));
    assert_eq!(menu.place_of(GuiPage::Crafting, ""), Some((play, Some(item(play, "crafting")))));
    assert_eq!(menu.place_of(GuiPage::Identity, ""), Some((humanity, Some(item(humanity, "directory")))));
    assert_eq!(menu.place_of(GuiPage::Dev, "claude"), Some((dev, Some(item(dev, "claude")))));
    assert_eq!(menu.place_of(GuiPage::Dev, "dev"), Some((dev, None)));
    assert_eq!(menu.place_of(GuiPage::BugReport, "claude"), Some((dev, Some(item(dev, "bugs")))));
    assert_eq!(menu.place_of(GuiPage::RelayControl, ""), None);
    assert_eq!(menu.place_of(GuiPage::MainMenu, ""), None);
}
