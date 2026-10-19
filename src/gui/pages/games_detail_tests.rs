// Child of pages/games.rs (#[path]): headless tests of the library's new shape (2026-10-19): the
// Grid's tiles, the sort and the search, the view and the sort kept over a restart, a game's
// details (its numbers, achievements, screenshots, mods and links), a screenshot shown large, and
// that drawing any of it reads no store file. Drawn over games/fixtures.rs's scratch stores; in a
// test build `games::launch` only records, and a web page asked for is only in egui's output.
// Each test was seen failing once on purpose; what it saw red is at the test.

use super::tests::{at, click, click_at, count, ctx_and_theme, exact, frame, in_row, open_details, page_with, read_in, settled, texts};
use super::*;
use crate::games::fixtures;
use crate::games::launch::recorder;

/// THE GRID'S TILES: each game's tile shows its store and one number (by name: the time played,
/// else its size), its achievements, and Play under its name, or why it cannot start in Play's
/// place; the tiles are in name order.
///
/// Seen red 2026-10-19 with `tile_number` giving nothing (each tile its store alone): "Orbital
/// Farmstead's tile: its store and time played" failed.
#[test]
fn the_grids_tiles() {
    let (ctx, theme) = ctx_and_theme();
    let fx = fixtures::all("gd_grid");
    let mut state = page_with(&fx, Vec::new());
    read_in(&ctx, &theme, &mut state);
    let out = settled(&ctx, &theme, &mut state);
    assert!(exact(&out, "Steam \u{b7} 12.6 hours"), "Orbital Farmstead's tile: its store and time played");
    assert!(exact(&out, "Steam \u{b7} 42 minutes"), "Old Lighthouse's");
    assert_eq!(count(&out, "Epic Games Store \u{b7} 8 GB"), 2, "an Epic game never played: its size");
    assert_eq!(count(&out, "3 of 5 achievements"), 1, "its achievements");
    // Its name under the cover (the lower of the two while the cover's picture is read: the plain
    // panel shows the name too).
    let name = texts(&out).into_iter().filter(|(t, _)| t == "Orbital Farmstead").map(|(_, r)| r).max_by(|a, b| a.center().y.total_cmp(&b.center().y)).expect("its name");
    let under = texts(&out).into_iter().any(|(t, r)| t == PLAY && (r.center().x - name.center().x).abs() < 160.0 && r.center().y > name.center().y && r.center().y - name.center().y < 100.0);
    assert!(under, "Play is on its tile, under its name");
    assert_eq!(count(&out, PLAY), 4, "Play on the four that can start");
    assert!(exact(&out, "Being installed by Steam") && exact(&out, "Being installed by Epic"), "a game being installed says so in Play's place");
    assert_eq!(count(&out, "Its folder is not there"), 2);
    let x = |n: &str| at(&out, n).unwrap().center();
    assert!(x("Deep Rover").x < x("Gone Valley").x && x("Gone Valley").x < x("Lantern Valley").x, "name order, left to right");
    assert!(recorder::recorded().is_empty(), "drawing starts nothing");
}

/// THE SORT AND THE SEARCH: by name; recently played, most played and size each put the largest
/// first and a game with no such number after every game with one, by name; the search keeps the
/// names holding every word typed, letters' case aside; the store pill narrows to that store.
/// Through the page: the "Most played" pill sorts the tiles so, and typing in the search field
/// leaves only the games whose names match.
///
/// Seen red 2026-10-19 with the numbers compared smallest first (`number(ga).cmp(&number(gb))`):
/// "most played: the most first" got ["Deep Rover", "Gone Valley", ...] (the unplayed first).
#[test]
fn the_sort_and_the_search() {
    let (ctx, theme) = ctx_and_theme();
    let fx = fixtures::all("gd_sort");
    let mut state = page_with(&fx, Vec::new());
    read_in(&ctx, &theme, &mut state);
    let all = state.games.all.clone();
    let names = |sort: SortBy, search: &str, filter: Option<Store>| -> Vec<String> { order_of(&all, sort, search, filter).into_iter().map(|i| all[i].name.clone()).collect() };
    assert_eq!(names(SortBy::Name, "", None), vec!["Deep Rover", "Gone Valley", "Lantern Valley", "Night Courier", "Old Forge", "Old Lighthouse", "Orbital Farmstead", "Sunken Archive"]);
    assert_eq!(names(SortBy::MostPlayed, "", None)[..3], ["Orbital Farmstead", "Old Lighthouse", "Deep Rover"], "most played: the most first");
    assert_eq!(names(SortBy::RecentlyPlayed, "", None)[..2], ["Orbital Farmstead", "Deep Rover"], "recently played: the only one with a time first, then by name");
    assert_eq!(names(SortBy::Size, "", None), vec!["Orbital Farmstead", "Lantern Valley", "Night Courier", "Old Lighthouse", "Deep Rover", "Sunken Archive", "Old Forge", "Gone Valley"], "size: the largest first, a tie by name, none last");
    assert_eq!(names(SortBy::Name, "OLD", None), vec!["Old Forge", "Old Lighthouse"], "the search, letters' case aside");
    assert_eq!(names(SortBy::Name, "valley lan", None), vec!["Lantern Valley"], "every word typed");
    assert_eq!(names(SortBy::Name, "", Some(Store::Epic)), vec!["Lantern Valley", "Night Courier"], "the store pill narrows");

    click(&ctx, &theme, &mut state, "Most played");
    assert_eq!(state.settings.games_sort, SortBy::MostPlayed);
    assert!(state.settings_dirty, "the sort is kept in the settings");
    let out = settled(&ctx, &theme, &mut state);
    let pos = |n: &str| at(&out, n).unwrap().center();
    assert!(pos("Orbital Farmstead").x < pos("Old Lighthouse").x && pos("Old Lighthouse").x < pos("Deep Rover").x, "the tiles follow the sort");
    assert!(exact(&out, "Steam \u{b7} 12.6 hours"));

    let field = at(&out, SEARCH_HINT).expect("the search field's hint").center();
    click_at(&ctx, &theme, &mut state, field);
    frame(&ctx, &theme, &mut state, 0.0, vec![egui::Event::Text("light".into())]);
    let out = settled(&ctx, &theme, &mut state);
    assert!(exact(&out, "Old Lighthouse") && !exact(&out, "Orbital Farmstead") && !exact(&out, "Old Forge"), "only the names that match");
    frame(&ctx, &theme, &mut state, 0.0, vec![egui::Event::Text("zzz".into())]);
    assert!(exact(&settled(&ctx, &theme, &mut state), "No game's name holds \"lightzzz\"."), "none matching says so");
}

/// THE VIEW AND THE SORT SURVIVE A RESTART: List and Size are picked on the page, marked for
/// saving, written by `from_gui_state` and read back by `apply_to_gui_state` (what a restart does);
/// the List view draws its headings; a config written before they existed shows the Grid by name.
///
/// Seen red 2026-10-19 with `apply_to_gui_state` not reading `games_layout`: "the view survives a
/// restart" (left: Grid, right: List).
#[test]
fn the_view_and_the_sort_survive_a_restart() {
    let (ctx, theme) = ctx_and_theme();
    let fx = fixtures::all("gd_view");
    let mut state = page_with(&fx, Vec::new());
    read_in(&ctx, &theme, &mut state);
    assert_eq!(state.settings.games_layout, Layout::Grid, "the Grid first");
    click(&ctx, &theme, &mut state, LIST);
    assert_eq!(state.settings.games_layout, Layout::List);
    assert!(state.settings_dirty);
    click(&ctx, &theme, &mut state, "Size");
    assert_eq!(state.settings.games_sort, SortBy::Size);
    let out = settled(&ctx, &theme, &mut state);
    assert!(exact(&out, "Last played") && exact(&out, "Achievements") && exact(&out, "31.2 GB"), "the List's headings and columns");

    let json = serde_json::to_string(&crate::config::AppConfig::from_gui_state(&state)).unwrap();
    let back: crate::config::AppConfig = serde_json::from_str(&json).unwrap();
    let mut fresh = GuiState::default();
    back.apply_to_gui_state(&mut fresh);
    assert_eq!(fresh.settings.games_layout, Layout::List, "the view survives a restart");
    assert_eq!(fresh.settings.games_sort, SortBy::Size, "the sort survives a restart");

    let mut old: serde_json::Value = serde_json::from_str(&json).unwrap();
    old.as_object_mut().unwrap().retain(|k, _| k != "games_layout" && k != "games_sort");
    let older: crate::config::AppConfig = serde_json::from_value(old).unwrap();
    assert_eq!((older.games_layout, older.games_sort), (Layout::Grid, SortBy::Name), "a config from before shows the Grid by name");
}

/// A GAME'S DETAILS: Orbital Farmstead's numbers (time played, last played, size, last update, its
/// folder), its achievements (three of five, the unlocked first with the day, a hidden one's
/// description kept hidden), its screenshots, its Workshop mods by number, and its links; an Epic
/// game's version and no links; Close closes them.
///
/// Seen red 2026-10-19 with the hidden achievement's description shown (`item.hidden && !unlocked`
/// replaced by `false`): "a hidden one's description is kept hidden" failed (the waterfall line was
/// drawn).
#[test]
fn a_games_details() {
    let (ctx, theme) = ctx_and_theme();
    let fx = fixtures::all("gd_details");
    let mut state = page_with(&fx, Vec::new());
    read_in(&ctx, &theme, &mut state);
    open_details(&ctx, &theme, &mut state, "Orbital Farmstead");
    let out = settled(&ctx, &theme, &mut state);
    for (label, value) in [("Played", "12.6 hours"), ("Last played", "21 September 2026"), ("Size on disk", "31.2 GB"), ("Last updated", "3 October 2026")] {
        assert!(in_row(&out, label, value).is_some(), "{label}: {value}");
    }
    let folder = fx.steam.root.join("steamapps").join("common").join("Orbital Farmstead");
    // The folder is a long path, wrapped over several lines beside its label: drawn whole.
    assert!(exact(&out, "Installed in") && exact(&out, &folder.display().to_string()), "its folder");
    assert!(in_row(&out, detail::ACHIEVEMENTS, "3 of 5").is_some(), "three of five");
    let y = |n: &str| at(&out, n).unwrap_or_else(|| panic!("{n} is drawn")).center().y;
    let order = ["Seedling", "First Harvest", "Long Night", "Deep Roots", "Secret Garden"];
    assert!(order.windows(2).all(|w| y(w[0]) < y(w[1])), "the unlocked first, newest first, then the rest");
    assert!(in_row(&out, "Seedling", "21 Sep 2026").is_some() && in_row(&out, "First Harvest", "10 Sep 2026").is_some(), "each with the day it was unlocked");
    assert!(exact(&out, "Plant your first seed.") && exact(&out, detail::HIDDEN), "descriptions, a hidden one's replaced");
    assert!(!exact(&out, "Find the garden behind the waterfall."), "a hidden one's description is kept hidden");
    assert!(in_row(&out, SCREENSHOTS_HEADING, "3").is_some(), "three screenshots");
    assert!(exact(&out, "Steam Workshop: 3 items, 1.4 GB") && exact(&out, detail::MODS_NOTE));
    assert!(y("Item 3000002") < y("Item 3000001") && y("Item 3000001") < y("Item 3000003"), "the mods by number, the most recently updated first");
    for link in ["Store page", "Community hub", "Steam store on the web", "PCGamingWiki", "SteamDB"] {
        assert!(exact(&out, link), "the {link} link");
    }
    assert!(recorder::recorded().is_empty(), "opening its details starts nothing");

    open_details(&ctx, &theme, &mut state, "Lantern Valley");
    let out = settled(&ctx, &theme, &mut state);
    assert!(in_row(&out, "Version", "2.4.1-CL-118").is_some() && in_row(&out, "Size on disk", "8 GB").is_some(), "an Epic game's version and size");
    assert!(!exact(&out, detail::LINKS) && !exact(&out, detail::ACHIEVEMENTS), "no links and no achievements for it");
    click(&ctx, &theme, &mut state, CLOSE);
    assert!(state.games.selected.is_none(), "Close closes them");
}

/// The screenshots' heading (the details' own word).
const SCREENSHOTS_HEADING: &str = detail::SCREENSHOTS;

/// The web pages a frame asked egui to open (egui 0.31 sends them as output commands; the app's
/// window opens them in the browser), each with whether in a new tab.
fn opened(out: &egui::FullOutput) -> Vec<(String, bool)> {
    out.platform_output.commands.iter().filter_map(|c| if let egui::OutputCommand::OpenUrl(o) = c { Some((o.url.clone(), o.new_tab)) } else { None }).collect()
}

/// EVERY LINK IN THE DETAILS OPENS EXACTLY ITS ADDRESS: the two in Steam through the one launcher
/// (recorded); the three on the web as a web page egui is asked to open, in a new tab, and nothing
/// started; a mod's Open in Steam its own page; the screenshots' and the mods' Show in folder their
/// folders. A web address on no listed host is refused.
///
/// Seen red 2026-10-19 with the web links sent through the launcher (`Opens::Web(u) =>
/// Act::Open(Launch::Link(u))`): "PCGamingWiki opens in the browser" got no open_url, the link
/// recorded instead.
#[test]
fn every_link_in_the_details_opens_exactly_its_address() {
    let (ctx, theme) = ctx_and_theme();
    let fx = fixtures::all("gd_links");
    let mut state = page_with(&fx, Vec::new());
    read_in(&ctx, &theme, &mut state);
    open_details(&ctx, &theme, &mut state, "Orbital Farmstead");
    let _ = recorder::recorded();

    click(&ctx, &theme, &mut state, "Store page");
    assert_eq!(recorder::recorded(), vec![Launch::Link("steam://store/1000010".into())]);
    click(&ctx, &theme, &mut state, "Community hub");
    assert_eq!(recorder::recorded(), vec![Launch::Link("steam://url/GameHub/1000010".into())]);
    for (label, url) in [
        ("Steam store on the web", "https://store.steampowered.com/app/1000010"),
        ("PCGamingWiki", "https://www.pcgamingwiki.com/api/appid.php?appid=1000010"),
        ("SteamDB", "https://steamdb.info/app/1000010/"),
    ] {
        let out = click(&ctx, &theme, &mut state, label);
        assert_eq!(opened(&out), vec![(url.to_string(), true)], "{label} opens in the browser");
        assert!(recorder::recorded().is_empty(), "{label} starts nothing");
    }

    let out = settled(&ctx, &theme, &mut state);
    let mod_button = in_row(&out, "Item 3000002", crate::games::OPEN_IN_STEAM).expect("a mod's Open in Steam");
    click_at(&ctx, &theme, &mut state, mod_button.center());
    assert_eq!(recorder::recorded(), vec![Launch::Link("steam://url/CommunityFilePage/3000002".into())]);

    let mut folders: Vec<egui::Rect> = texts(&settled(&ctx, &theme, &mut state)).into_iter().filter(|(t, _)| t == SHOW_IN_FOLDER).map(|(_, r)| r).collect();
    folders.sort_by(|a, b| a.center().y.total_cmp(&b.center().y));
    assert_eq!(folders.len(), 3, "the game's, its screenshots' and its mods'");
    click_at(&ctx, &theme, &mut state, folders[1].center());
    let shots = fx.steam.root.join("userdata").join(fixtures::ACCOUNT.to_string()).join("760").join("remote").join("1000010").join("screenshots");
    assert_eq!(recorder::recorded(), vec![Launch::ShowInFolder(shots)], "the screenshots' folder");
    click_at(&ctx, &theme, &mut state, folders[2].center());
    assert_eq!(recorder::recorded(), vec![Launch::ShowInFolder(fx.steam.root.join("steamapps").join("workshop").join("content").join("1000010"))], "the mods' folder");

    let out = ctx.run(egui::RawInput::default(), |ctx| apply(ctx, &mut state, Act::OpenWeb("https://evil.example/app/1".into())));
    assert!(opened(&out).is_empty(), "a host not listed is not opened");
    assert!(state.games.status.as_ref().is_some_and(|(_, problem)| *problem), "and the page says so");
}

/// A SCREENSHOT SHOWN LARGE: a click on one opens it over the page, named and numbered; Next and
/// the right arrow key step to the older ones, and stop at the last; its Show in folder shows that
/// picture; a click beside it closes it.
///
/// Seen red 2026-10-19 with the arrow keys left out of `viewer`: "the right arrow steps on" (left:
/// Some(1), right: Some(2)).
#[test]
fn a_screenshot_shown_large() {
    let (ctx, theme) = ctx_and_theme();
    let fx = fixtures::all("gd_viewer");
    let mut state = page_with(&fx, Vec::new());
    read_in(&ctx, &theme, &mut state);
    open_details(&ctx, &theme, &mut state, "Orbital Farmstead");
    let out = settled(&ctx, &theme, &mut state);
    // The pictures sit between their heading and their Show in folder.
    let heading = at(&out, SCREENSHOTS_HEADING).unwrap();
    let mut folders: Vec<egui::Rect> = texts(&out).into_iter().filter(|(t, _)| t == SHOW_IN_FOLDER).map(|(_, r)| r).collect();
    folders.sort_by(|a, b| a.center().y.total_cmp(&b.center().y));
    let first = egui::pos2(heading.left() + 30.0, (heading.bottom() + folders[1].top()) / 2.0);
    click_at(&ctx, &theme, &mut state, first);
    assert_eq!(state.games.viewer, Some(0), "a click opens the newest large");
    assert!(exact(&settled(&ctx, &theme, &mut state), "Orbital Farmstead: screenshot 1 of 3"));

    click(&ctx, &theme, &mut state, "Next");
    assert_eq!(state.games.viewer, Some(1), "Next steps on");
    let right = egui::Event::Key { key: egui::Key::ArrowRight, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::default() };
    frame(&ctx, &theme, &mut state, 0.0, vec![right.clone()]);
    frame(&ctx, &theme, &mut state, 0.0, Vec::new());
    assert_eq!(state.games.viewer, Some(2), "the right arrow steps on");
    frame(&ctx, &theme, &mut state, 0.0, vec![right]);
    frame(&ctx, &theme, &mut state, 0.0, Vec::new());
    assert_eq!(state.games.viewer, Some(2), "and stops at the last");

    // The window's own Show in folder is the last one drawn (over the page).
    let out = settled(&ctx, &theme, &mut state);
    let in_window = texts(&out).into_iter().filter(|(t, _)| t == SHOW_IN_FOLDER).map(|(_, r)| r).last().unwrap();
    let _ = recorder::recorded();
    click_at(&ctx, &theme, &mut state, in_window.center());
    let oldest = state.games.selected_game().unwrap().details.screenshots.as_ref().unwrap().shots[2].path.clone();
    assert!(oldest.ends_with("20261001120000_1.jpg"));
    assert_eq!(recorder::recorded(), vec![Launch::ShowInFolder(oldest)], "its Show in folder shows that picture");
    click_at(&ctx, &theme, &mut state, egui::pos2(8.0, 8.0));
    assert_eq!(state.games.viewer, None, "a click beside it closes it");
}

/// STEAM'S CACHED PICTURES ON THE TILES AND THE BANNER: Orbital Farmstead's tile shows the picture
/// Steam's client cached (its name then drawn once, under it, not on a plain panel), Sunken
/// Archive's, with none, its name on the plain panel; Orbital Farmstead's details show its banner
/// with its lettering over it. The pictures are read on thumbnails' own workers, so the frames are
/// drawn until they are in.
///
/// Seen red 2026-10-19 with the tile handed no picture (`picture` always `None`): "its picture,
/// its name drawn once" (left: 2, right: 1).
#[test]
fn steams_cached_pictures_on_the_tiles_and_the_banner() {
    let (ctx, theme) = ctx_and_theme();
    let fx = fixtures::all("gd_pictures");
    let mut state = page_with(&fx, Vec::new());
    read_in(&ctx, &theme, &mut state);
    let textured = |out: &egui::FullOutput| out.shapes.iter().filter(|cs| matches!(&cs.shape, egui::Shape::Rect(r) if r.brush.is_some())).count();
    let mut out = settled(&ctx, &theme, &mut state);
    for _ in 0..300 {
        if count(&out, "Orbital Farmstead") == 1 && textured(&out) >= 3 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        out = frame(&ctx, &theme, &mut state, 0.0, Vec::new());
    }
    assert_eq!(count(&out, "Orbital Farmstead"), 1, "its picture, its name drawn once");
    assert_eq!(count(&out, "Sunken Archive"), 2, "no picture: its name on the plain panel and under it");
    assert_eq!(textured(&out), 3, "Orbital Farmstead's, Old Lighthouse's and Deep Rover's pictures are drawn, and no other");

    open_details(&ctx, &theme, &mut state, "Orbital Farmstead");
    let mut images = 0;
    for _ in 0..200 {
        let out = frame(&ctx, &theme, &mut state, 0.0, Vec::new());
        images = out.shapes.iter().filter(|cs| matches!(&cs.shape, egui::Shape::Mesh(_))).count();
        if images > 0 && count(&out, "Orbital Farmstead") == 2 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(images > 0, "its lettering is drawn over its banner");
}

/// THE SECOND WHY OPENS THE PICTURES FINDING: the line about covers and logos has its own Why,
/// which opens the 19 October 2026 finding in the Library (the first Why still opens the 13
/// October one).
///
/// Seen red 2026-10-19 before the finding was in the Library's catalogue: "the second Why opens
/// the Library" (left: Games, right: Library).
#[test]
fn the_second_why_opens_the_pictures_finding() {
    let (ctx, theme) = ctx_and_theme();
    let fx = fixtures::all("gd_why2");
    let mut state = page_with(&fx, Vec::new());
    state.library = crate::gui::load_library(std::path::Path::new("data")).sections;
    read_in(&ctx, &theme, &mut state);
    let out = settled(&ctx, &theme, &mut state);
    let mut whys: Vec<egui::Rect> = texts(&out).into_iter().filter(|(t, _)| t == WHY).map(|(_, r)| r).collect();
    whys.sort_by(|a, b| a.center().y.total_cmp(&b.center().y));
    click_at(&ctx, &theme, &mut state, whys[1].center());
    assert_eq!(state.active_page, GuiPage::Library, "the second Why opens the Library: {:?}", state.games.status);
    assert!(state.library.iter().flat_map(|s| s.categories.iter()).flat_map(|c| c.entries.iter()).any(|e| e.slug == PICTURES_FINDING_SLUG && e.body.contains("Researched 19 October 2026")), "the document is that finding");
}

/// AN EPIC GAME'S PROGRAM ICON ON ITS PLAIN TILE (Windows): Lantern Valley's program, here a copy
/// of Windows' own Explorer, gives its icon, made a texture once, and its tile stays the plain
/// panel with its name.
///
/// Seen red 2026-10-19 with `ensure_icons` not called from `draw`: "its icon is a texture" failed.
#[cfg(windows)]
#[test]
fn an_epic_games_program_icon_on_its_plain_tile() {
    let (ctx, theme) = ctx_and_theme();
    let fx = fixtures::all("gd_icon");
    let explorer = std::path::PathBuf::from(std::env::var("SystemRoot").unwrap()).join("explorer.exe");
    std::fs::copy(&explorer, fx.epic.lantern.join("Binaries").join("Win64").join("LanternValley.exe")).unwrap();
    let mut state = page_with(&fx, Vec::new());
    read_in(&ctx, &theme, &mut state);
    let out = settled(&ctx, &theme, &mut state);
    assert!(state.games.icons.contains_key(&(Store::Epic, "Lantern".to_string())), "its icon is a texture");
    assert_eq!(state.games.icons.len(), 1, "only the program that has one");
    assert_eq!(count(&out, "Lantern Valley"), 2, "its tile is the plain panel with its name");
}

/// DRAWING THE PAGE READS NO STORE FILE: with a game's details open (its achievements,
/// screenshots and mods), sixty frames of the Grid and the List and ten of a screenshot shown large
/// read nothing of the stores' on the UI thread. The counter counts this thread's reads (a read
/// here moves it), so a quiet counter is the page reading nothing, not a counter that never moves.
///
/// Seen red 2026-10-19 with the details listing the screenshots' folder each frame
/// (`crate::games::list_dir(&s.folder)` in `screenshots`): "drawing reads nothing" (left: 70,
/// right: 0 more reads).
#[test]
fn drawing_the_page_reads_no_store_file() {
    let (ctx, theme) = ctx_and_theme();
    let fx = fixtures::all("gd_no_reads");
    let mut state = page_with(&fx, Vec::new());
    read_in(&ctx, &theme, &mut state);
    open_details(&ctx, &theme, &mut state, "Orbital Farmstead");

    let before = crate::games::disk_reads_here();
    for _ in 0..30 {
        frame(&ctx, &theme, &mut state, 0.0, Vec::new());
    }
    state.settings.games_layout = Layout::List;
    for _ in 0..30 {
        frame(&ctx, &theme, &mut state, 0.0, Vec::new());
    }
    state.games.viewer = Some(0);
    for _ in 0..10 {
        frame(&ctx, &theme, &mut state, 0.0, Vec::new());
    }
    assert_eq!(crate::games::disk_reads_here() - before, 0, "drawing reads nothing");

    let control = crate::games::disk_reads_here();
    let _ = crate::games::read(&fx.sources());
    assert!(crate::games::disk_reads_here() > control, "the counter counts a read on this thread");
}
