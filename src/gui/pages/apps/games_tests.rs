// Child of pages/apps.rs: headless tests of the Apps page's games (the Games library's tests,
// moved with it on 2026-10-19), drawn whole over the stores' records written into scratch folders
// (games/fixtures.rs). No window, no store and no game: in a test build `games::launch` only
// records what it was asked (games/launch.rs), and the page reads no store of this computer's. Each
// test was seen failing once on purpose; what it saw red is at the test.
//
// 2026-10-19: the page became a library (a Grid or a List, a game's details beside it), then the
// Apps page's red part, with what the person adds in its own yellow part, "Added by you" (Launch,
// not Play: it may be any program). The tests kept their purpose and moved with it: the buttons
// on each row are checked in the List view, where Play and the store's own button are on the
// game's line; Show in folder, Rename and Remove, in a game's details, are checked there. The
// details, the grid, the sort and the search are tested in game_detail_tests.rs.

use super::games::{self as g, Where};
use super::tests::{click, click_at, click_in_row, count, ctx_and_theme, exact, frame, in_row, open_details, page_with, read_in, settled, texts, a_program};
use super::*;
use crate::games::launch::recorder;
use crate::games::{fixtures, AddedApp, Sources};

/// The same, shown as a List (a row per game: Play and the store's own button on its line).
fn list_page_with(fx: &fixtures::AllFixture, added: Vec<AddedApp>) -> GuiState {
    let mut state = page_with(fx, added);
    state.settings.apps_view.layout = Layout::List;
    state
}

/// THE PAGE LISTS THE THREE STORES' GAMES, AND AN ADDED ONE UNDER ADDED BY YOU, WITH THEIR BUTTONS
/// (in the List view): a pill per store with its count; Play on each game that can start now and on
/// no other, a game that cannot saying so in Play's place, and Launch on the one added; "Open in
/// Steam" on every Steam game and "Open in the Epic Games Store" on every Epic one, and nothing of
/// the kind on a GOG game (GOG documents no link that opens GOG GALAXY); and the line about the
/// stores with its Why.
///
/// Seen red 2026-10-18 with the store's button left out of `draw_row`: "Open in Steam on every
/// Steam game" (left: 0, right: 4). Seen red again 2026-10-19 (the List view's row) with the store
/// button left out of `list`: the same assertion (left: 0, right: 4).
#[test]
fn the_three_stores_and_an_added_game_with_their_buttons() {
    let (ctx, theme) = ctx_and_theme();
    let fx = fixtures::all("gp_rows");
    let (_dir, exe) = a_program("gp_rows_added");
    let mut state = list_page_with(&fx, vec![AddedApp { name: "Dwarf Fortress Classic".into(), exe }]);
    read_in(&ctx, &theme, &mut state);
    let out = settled(&ctx, &theme, &mut state);

    assert!(exact(&out, TITLE) && exact(&out, g::STORES_LINE) && exact(&out, WHY), "the heading, the line about the stores and Why");
    assert!(exact(&out, g::PICTURES_LINE) && exact(&out, g::NOT_AFFILIATED), "what the pictures are, why no logos, and that HumanityOS is not affiliated with the stores");
    assert_eq!(count(&out, WHY), 2, "a Why for each line");
    for pill in ["All 8", "Steam 4", "Epic Games Store 2", "GOG 2", "Added by you 1", "Games 8"] {
        assert!(exact(&out, pill), "the {pill} pill");
    }
    for name in ["Orbital Farmstead", "Deep Rover", "Old Lighthouse", "Sunken Archive", "Lantern Valley", "Night Courier", "Old Forge", "Gone Valley", "Dwarf Fortress Classic"] {
        assert!(exact(&out, name), "{name} is listed");
    }
    assert!(!exact(&out, "Steamworks Common Redistributables"), "Steam's redistributables are not a game");
    for name in ["Orbital Farmstead", "Old Lighthouse", "Lantern Valley", "Old Forge"] {
        assert!(in_row(&out, name, g::PLAY).is_some(), "{name} has Play");
    }
    for name in ["Deep Rover", "Sunken Archive", "Night Courier", "Gone Valley"] {
        assert!(in_row(&out, name, g::PLAY).is_none(), "{name} has no Play");
    }
    assert_eq!(count(&out, g::PLAY), 4, "Play on the four that can start, no more");
    assert!(in_row(&out, "Dwarf Fortress Classic", g::LAUNCH).is_some() && in_row(&out, "Dwarf Fortress Classic", g::PLAY).is_none(), "Launch on what was added");
    assert!(in_row(&out, "Deep Rover", "Installing").is_some() && in_row(&out, "Night Courier", "Installing").is_some(), "a game being installed says so in Play's place");
    assert!(in_row(&out, "Sunken Archive", "Folder missing").is_some(), "a folder gone says so in Play's place");
    assert_eq!(count(&out, crate::games::OPEN_IN_STEAM), 4, "Open in Steam on every Steam game");
    assert_eq!(count(&out, crate::games::OPEN_IN_EPIC), 2, "Open in the Epic Games Store on every Epic game");
    assert!(in_row(&out, "Old Forge", crate::games::OPEN_IN_STEAM).is_none() && in_row(&out, "Old Forge", crate::games::OPEN_IN_EPIC).is_none(), "no store button on a GOG game");
    assert!(!texts(&out).iter().any(|(t, _)| t.contains("GALAXY")), "nothing offers to open GOG GALAXY");
    assert_eq!((count(&out, g::RENAME), count(&out, g::REMOVE)), (0, 0), "Rename and Remove are in an added game's details, not on its row");
    assert!(recorder::recorded().is_empty(), "drawing the page starts nothing");
}

/// A MISSING FOLDER OR PROGRAM SAYS SO IN ITS DETAILS, AND HAS NO PLAY: Steam's Sunken Archive and
/// GOG's Gone Valley, whose folders are not there, say so and have neither Play nor Show in
/// folder; an added program that was deleted says that, has no Launch, and its Show in folder
/// shows the folder.
///
/// Seen red 2026-10-18 with `why_not_words` saying nothing for a folder gone: "a folder gone says
/// so, twice" (left: 0, right: 2). Seen red 2026-10-19 (moved into the details) with the details
/// leaving out `why_not_words`: "Sunken Archive's details say its folder is gone" (left: 0, right: 1).
#[test]
fn a_missing_folder_or_program_says_so_and_has_no_play() {
    let (ctx, theme) = ctx_and_theme();
    let fx = fixtures::all("gp_missing");
    let (dir, exe) = a_program("gp_missing_added");
    std::fs::remove_file(&exe).unwrap();
    let mut state = page_with(&fx, vec![AddedApp { name: "Dwarf Fortress".into(), exe }]);
    read_in(&ctx, &theme, &mut state);

    for name in ["Sunken Archive", "Gone Valley"] {
        open_details(&ctx, &theme, &mut state, name);
        let out = settled(&ctx, &theme, &mut state);
        assert_eq!(count(&out, g::FOLDER_GONE), 1, "{name}'s details say its folder is gone");
        assert!(!exact(&out, SHOW_IN_FOLDER), "no Show in folder for a folder that is not there");
    }
    open_details(&ctx, &theme, &mut state, "Dwarf Fortress");
    let out = settled(&ctx, &theme, &mut state);
    assert_eq!(count(&out, g::PROGRAM_GONE), 1, "a program gone says so");
    assert_eq!(count(&out, g::PLAY), 4, "Play on the four tiles that can start (Orbital Farmstead, Old Lighthouse, Lantern Valley, Old Forge)");
    assert_eq!(count(&out, g::LAUNCH), 0, "none on the one added, and none in its details");
    click(&ctx, &theme, &mut state, SHOW_IN_FOLDER);
    assert_eq!(recorder::recorded(), vec![Launch::ShowInFolder(dir.to_path_buf())], "its program gone, Show in folder shows the folder");
}

/// PLAY ON EACH STORE ASKS THE ONE LAUNCHER FOR EXACTLY THE RIGHT THING: Steam's
/// steam://rungameid/<appid>; Epic's documented launch link with its three ids; a GOG game's own
/// primary program with its arguments and working directory; an added program's own program in its
/// own folder (its Launch). The store buttons send their links, Show in folder (in the details) its
/// folder, and a second press straight after the first starts nothing more. A game that will not
/// start says why.
///
/// Seen red 2026-10-18 with Play on a Steam game sending its "Open in Steam" link instead
/// (`store_button` in `play`): "Steam: rungameid" got [Link("steam://nav/games/details/1000010")].
#[test]
fn play_on_each_store_sends_exactly_its_link_or_program() {
    let (ctx, theme) = ctx_and_theme();
    let fx = fixtures::all("gp_play");
    let (dir, exe) = a_program("gp_play_added");
    let mut state = list_page_with(&fx, vec![AddedApp { name: "Dwarf Fortress Classic".into(), exe: exe.clone() }]);
    read_in(&ctx, &theme, &mut state);
    let _ = recorder::recorded();

    click_in_row(&ctx, &theme, &mut state, "Orbital Farmstead", g::PLAY);
    assert_eq!(recorder::recorded(), vec![Launch::Link("steam://rungameid/1000010".into())], "Steam: rungameid");
    assert_eq!(state.apps.status, Some(("Asked Steam to start Orbital Farmstead.".into(), false)));

    click_in_row(&ctx, &theme, &mut state, "Lantern Valley", g::PLAY);
    assert_eq!(recorder::recorded(), vec![Launch::Link("com.epicgames.launcher://apps/ns1%3Aitem1%3ALantern?action=launch&silent=true".into())], "Epic: its launch link");

    click_in_row(&ctx, &theme, &mut state, "Old Forge", g::PLAY);
    assert_eq!(
        recorder::recorded(),
        vec![Launch::Program { exe: fx.gog.forge.join("bin\\forge.exe"), args: "-windowed -lang en".into(), dir: fx.gog.forge.join("bin") }],
        "GOG: its own primary program, its arguments, its working directory"
    );

    click_in_row(&ctx, &theme, &mut state, "Dwarf Fortress Classic", g::LAUNCH);
    assert_eq!(recorder::recorded(), vec![Launch::Program { exe: exe.clone(), args: String::new(), dir: dir.to_path_buf() }], "added: its own program");
    click_in_row(&ctx, &theme, &mut state, "Dwarf Fortress Classic", g::LAUNCH);
    assert!(recorder::recorded().is_empty(), "a second press straight after is the same press");

    click_in_row(&ctx, &theme, &mut state, "Deep Rover", crate::games::OPEN_IN_STEAM);
    assert_eq!(recorder::recorded(), vec![Launch::Link("steam://nav/games/details/1000020".into())], "Open in Steam on a game Steam is installing");
    click_in_row(&ctx, &theme, &mut state, "Night Courier", crate::games::OPEN_IN_EPIC);
    assert_eq!(recorder::recorded(), vec![Launch::Link("com.epicgames.launcher://store".into())], "Open in the Epic Games Store");
    open_details(&ctx, &theme, &mut state, "Old Forge");
    click(&ctx, &theme, &mut state, SHOW_IN_FOLDER);
    assert_eq!(recorder::recorded(), vec![Launch::ShowInFolder(fx.gog.forge.clone())], "Show in folder shows its folder");

    recorder::answer_with(Err("Windows could not start it (test)".into()));
    click_in_row(&ctx, &theme, &mut state, "Old Lighthouse", g::PLAY);
    assert_eq!(state.apps.status, Some(("Old Lighthouse did not start: Windows could not start it (test).".into(), true)), "a game that will not start says why");
    assert!(exact(&settled(&ctx, &theme, &mut state), "Old Lighthouse did not start: Windows could not start it (test)."));
    recorder::answer_with(Ok(()));
}

/// ADD A PROGRAM, RENAME IT AND REMOVE IT, EACH SURVIVING A RESTART: "Add a program" (on Added by
/// you's band) opens the app's file picker offering programs; the program chosen is added under its
/// file name, kept in the settings (marked for saving, written by `from_gui_state`, read back by
/// `apply_to_gui_state`, which is what a restart does), and its details open; Rename (in its
/// details) changes the name and that is kept too; Remove takes it off and leaves the program where
/// it was. The same program is never added twice.
///
/// Seen red 2026-10-18 with `apply_to_gui_state` not reading `added_games` (now `apps_added`): "the
/// added game survives a restart" (left: [] / right: [AddedGame { name: "Dwarf Fortress", .. }]).
#[test]
fn add_rename_and_remove_survive_a_restart() {
    let (ctx, theme) = ctx_and_theme();
    let fx = fixtures::all("gp_add");
    let (dir, exe) = a_program("gp_add_prog");
    std::fs::write(dir.join("notes.txt"), "not a program").unwrap();
    let mut state = page_with(&fx, Vec::new());
    read_in(&ctx, &theme, &mut state);
    assert!(exact(&settled(&ctx, &theme, &mut state), g::ADDED_NONE), "with none added, it says how");

    click(&ctx, &theme, &mut state, g::ADD);
    let picker = state.apps.games.picker.as_mut().expect("Add a program opens the file picker");
    #[cfg(windows)]
    assert_eq!(picker.allowed_exts, vec!["exe".to_string()], "it offers programs");
    picker.current_dir = dir.to_path_buf();
    click(&ctx, &theme, &mut state, "Dwarf Fortress.exe");
    click(&ctx, &theme, &mut state, "Add Dwarf Fortress.exe");
    assert!(state.apps.games.picker.is_none(), "the picker closes");
    assert_eq!(state.settings.apps_added, vec![AddedApp { name: "Dwarf Fortress".into(), exe: exe.clone() }], "added under its file name");
    assert!(state.settings_dirty, "and the settings are saved with it");
    assert!(state.apps.selected_game().is_some_and(|g| g.name == "Dwarf Fortress"), "its details open");
    let restart = |state: &GuiState| {
        let json = serde_json::to_string(&crate::config::AppConfig::from_gui_state(state)).unwrap();
        let back: crate::config::AppConfig = serde_json::from_str(&json).unwrap();
        let mut fresh = GuiState::default();
        back.apply_to_gui_state(&mut fresh);
        fresh.settings.apps_added
    };
    assert_eq!(restart(&state), state.settings.apps_added, "the added program survives a restart");
    assert_eq!(count(&settled(&ctx, &theme, &mut state), g::LAUNCH), 2, "its tile has Launch, and its details too");

    state.settings_dirty = false;
    click(&ctx, &theme, &mut state, g::RENAME);
    assert_eq!(state.apps.games.renaming, Some((exe.clone(), "Dwarf Fortress".into())), "Rename edits its name");
    // Type at the end of the name (the field holding it is on Save's line), then press Enter.
    let field = in_row(&settled(&ctx, &theme, &mut state), g::SAVE, "Dwarf Fortress").expect("the name is in its field, beside Save").center();
    click_at(&ctx, &theme, &mut state, field);
    let key = |k: egui::Key| egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::default() };
    frame(&ctx, &theme, &mut state, 0.0, vec![key(egui::Key::End)]);
    frame(&ctx, &theme, &mut state, 0.0, vec![egui::Event::Text(" Classic".into())]);
    click(&ctx, &theme, &mut state, g::SAVE);
    assert_eq!(state.settings.apps_added[0].name, "Dwarf Fortress Classic", "the new name is kept");
    assert!(state.apps.games.renaming.is_none() && state.settings_dirty, "and saved");
    assert_eq!(restart(&state)[0].name, "Dwarf Fortress Classic", "the new name survives a restart");

    g::add_added(&mut state, &exe);
    assert_eq!(state.settings.apps_added.len(), 1, "the same program is not added twice");
    assert_eq!(state.apps.status, Some(("Dwarf Fortress is already on the list.".into(), false)));

    state.settings_dirty = false;
    click(&ctx, &theme, &mut state, g::REMOVE);
    assert!(state.settings.apps_added.is_empty() && state.settings_dirty, "Remove takes it off, and that is saved");
    assert!(restart(&state).is_empty(), "and stays off after a restart");
    assert!(exe.is_file(), "the program itself is not touched");
    settled(&ctx, &theme, &mut state);
    assert!(state.apps.selected.is_none(), "its details close with it");
    assert!(recorder::recorded().is_empty(), "none of that started anything");
}

/// A STORE THAT IS NOT INSTALLED HAS NO PILL: with only Steam found there is no Epic Games Store
/// or GOG pill (and nothing of theirs anywhere); with none found the page says so, and Added by you
/// is still there, saying how to add something.
///
/// Seen red 2026-10-18 with every store's section drawn whether found or not (the `let Some(list)`
/// replaced by an empty list): "no Epic Games Store section" failed. Seen red 2026-10-19 (pills)
/// with every store's pill pushed whether found or not: "no Epic Games Store pill" failed (its
/// pill, "Epic Games Store 0", drawn).
#[test]
fn a_store_not_installed_has_no_pill() {
    let (ctx, theme) = ctx_and_theme();
    let fx = fixtures::all("gp_one_store");
    let mut state = page_with(&fx, Vec::new());
    state.apps.games.sources = Where::These(Sources { steam_root: Some(fx.steam.root.clone()), ..Default::default() });
    read_in(&ctx, &theme, &mut state);
    let out = settled(&ctx, &theme, &mut state);
    assert!(exact(&out, "Steam 4") && exact(&out, "Orbital Farmstead"), "Steam is there");
    assert!(!texts(&out).iter().any(|(t, _)| t.starts_with("Epic Games Store")), "no Epic Games Store pill");
    assert!(!texts(&out).iter().any(|(t, _)| t.starts_with("GOG")), "no GOG pill");
    assert!(!exact(&out, g::NO_STORES));

    let mut none = page_with(&fx, Vec::new());
    none.apps.games.sources = Where::These(Sources::default());
    read_in(&ctx, &theme, &mut none);
    let out = settled(&ctx, &theme, &mut none);
    assert!(exact(&out, g::NO_STORES), "with no store found, it says so");
    assert!(!texts(&out).iter().any(|(t, _)| t.starts_with("Steam")), "and has no store pill");
    assert!(exact(&out, "Added by you 0") && exact(&out, g::ADDED_NONE), "Added by you is still there");
}

/// THE STORES ARE READ WHEN THE PAGE OPENS AND ON REFRESH, NEVER EVERY FRAME: thirty frames of the
/// open page read once; the page opened again (not drawn for a while) reads again; Refresh reads
/// again and shows what changed (a game Steam took off); a Refresh while a read is under way does
/// not start a second.
///
/// Seen red 2026-10-18 with `draw` reading on every frame (`opened` always true): "thirty frames
/// read once" (left: 12, right: 1: a new read each frame the last one had finished).
#[test]
fn the_stores_are_read_on_open_and_refresh_never_every_frame() {
    let (ctx, theme) = ctx_and_theme();
    let fx = fixtures::all("gp_reads");
    let mut state = page_with(&fx, Vec::new());
    for _ in 0..30 {
        frame(&ctx, &theme, &mut state, 0.0, Vec::new());
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(state.apps.games.reads_started, 1, "thirty frames read once");
    read_in(&ctx, &theme, &mut state);
    assert_eq!(state.apps.games.reads_started, 1, "and the read came in");

    state.apps.last_drawn = Some(Instant::now() - Duration::from_secs(10));
    read_in(&ctx, &theme, &mut state);
    assert_eq!(state.apps.games.reads_started, 2, "opened again, it reads again");

    std::fs::remove_file(fx.steam.root.join("steamapps").join("appmanifest_1000010.acf")).unwrap();
    assert!(exact(&settled(&ctx, &theme, &mut state), "Orbital Farmstead"), "until it reads again it shows what it read");
    click(&ctx, &theme, &mut state, REFRESH);
    assert_eq!(state.apps.games.reads_started, 3, "Refresh reads again");
    read_in(&ctx, &theme, &mut state);
    assert!(!exact(&settled(&ctx, &theme, &mut state), "Orbital Farmstead"), "and shows what changed");

    g::start_reading(&mut state);
    g::start_reading(&mut state);
    assert_eq!(state.apps.games.reads_started, 4, "a Refresh while a read is under way starts no second one");
}

/// WHY OPENS THE FINDING IN THE LIBRARY: the dated finding the games follow is a document in the
/// app's own Library (data/library, "Research Findings"), and Why opens it there.
///
/// Seen red 2026-10-18 before the finding was in the Library's catalogue: "Why opens the Library"
/// (left: Games, right: Library), the status saying the Library does not have it.
#[test]
fn why_opens_the_finding_in_the_library() {
    let (ctx, theme) = ctx_and_theme();
    let fx = fixtures::all("gp_why");
    let mut state = page_with(&fx, Vec::new());
    state.library = crate::gui::load_library(std::path::Path::new("data")).sections;
    read_in(&ctx, &theme, &mut state);
    click(&ctx, &theme, &mut state, WHY);
    assert_eq!(state.active_page, GuiPage::Library, "Why opens the Library: {:?}", state.apps.status);
    assert!(state.library.iter().flat_map(|s| s.categories.iter()).flat_map(|c| c.entries.iter()).any(|e| e.slug == g::FINDING_SLUG && e.body.contains("Researched 13 October 2026")), "the document is the finding itself");
}
