// Child of pages/tools.rs (#[path]): headless tests of the Tools page as a library (2026-10-19),
// drawn whole over the shipped catalog and over Windows' and Steam's records written as fixtures
// (scratch folders and a `Records` of the test's own: the real registry is never read). No window
// and no program: in a test build `games::launch` only records what it was asked
// (games/launch.rs), and winget is never run: software/install.rs's runner only records there and
// answers what the test set (its `recorder`). Each test was seen
// failing once on purpose; what it saw red is at the test.

use std::path::{Path, PathBuf};

use super::*;
use crate::games::launch::recorder;
use crate::gui::pages::games::tests::{at, count, ctx_and_theme, exact, in_row, texts};
use crate::gui::screen_surface::find_text_in_shapes;
use crate::gui::GuiPage;
use crate::software::records::{AppPath, Hive, Records, Uninstall};

/// One frame of the page, `later` seconds after the last.
pub(super) fn frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, later: f64, events: Vec<egui::Event>) -> egui::FullOutput {
    let time = Some(ctx.input(|i| i.time) + later);
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 1600.0))), time, events, ..Default::default() };
    ctx.run(input, |ctx| draw(ctx, theme, state))
}

pub(super) fn settled(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) -> egui::FullOutput {
    for _ in 0..3 {
        frame(ctx, theme, state, 0.0, Vec::new());
    }
    frame(ctx, theme, state, 0.0, Vec::new())
}

/// Press and release at `pos`, a second after the last click; the release frame's output.
fn press(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, pos: egui::Pos2) -> egui::FullOutput {
    let m = egui::Modifiers::default();
    frame(ctx, theme, state, 1.0, vec![egui::Event::PointerMoved(pos)]);
    frame(ctx, theme, state, 0.0, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
    frame(ctx, theme, state, 0.0, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }])
}

fn click(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, text: &str) -> egui::FullOutput {
    let out = settled(ctx, theme, state);
    let pos = find_text_in_shapes(&out.shapes, text).unwrap_or_else(|| panic!("{text:?} is drawn")).rect.center();
    press(ctx, theme, state, pos)
}

/// Draw until the worker's answer is in (it reads the test's own records, so it is quick).
fn found_in(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    for _ in 0..1000 {
        frame(ctx, theme, state, 0.0, Vec::new());
        if state.tools.found.is_some() && state.tools.reading.is_none() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("the read never came in");
}

/// The web pages a frame asked egui to open, each with whether in a new tab.
fn opened(out: &egui::FullOutput) -> Vec<(String, bool)> {
    out.platform_output.commands.iter().filter_map(|c| if let egui::OutputCommand::OpenUrl(o) = c { Some((o.url.clone(), o.new_tab)) } else { None }).collect()
}

/// A program file in a scratch folder.
fn program(dir: &Path, rel: &str) -> PathBuf {
    let p = rel.split('/').fold(dir.to_path_buf(), |p, part| p.join(part));
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, "x").unwrap();
    p
}

/// This computer's records as a fixture, the way the operator's computer has them (2026-10-19):
/// GIMP 3 in its own folder, Notepad++ known to App Paths, DaVinci Resolve with no folder recorded,
/// and Blender installed by Steam.
pub(crate) struct Fixture {
    _dir: crate::test_temp::TempPath,
    pub sources: software::Sources,
    pub gimp_folder: PathBuf,
    pub gimp: PathBuf,
    pub notepad: PathBuf,
}

pub(crate) fn fixture(tag: &str) -> Fixture {
    let dir = crate::test_temp::dir(tag);
    let gimp_folder = dir.join("GIMP 3");
    let gimp = program(&gimp_folder, "bin/gimp-3.exe");
    let notepad = program(&dir, "Notepad++/notepad++.exe");
    let steam = dir.join("Steam");
    std::fs::create_dir_all(steam.join("steamapps").join("common").join("Blender")).unwrap();
    program(&steam, "steamapps/common/Blender/blender.exe");
    std::fs::write(steam.join("steamapps").join("appmanifest_365670.acf"), "\"AppState\"\n{\n\t\"appid\"\t\t\"365670\"\n\t\"name\"\t\t\"Blender\"\n\t\"StateFlags\"\t\t\"4\"\n\t\"installdir\"\t\t\"Blender\"\n}\n").unwrap();
    let record = |name: &str, version: &str, location: Option<&Path>, icon: Option<&Path>| Uninstall {
        key: name.into(),
        display_name: name.into(),
        version: Some(version.into()),
        location: location.map(Path::to_path_buf),
        icon: icon.map(|p| format!("\"{}\",0", p.display())),
        hive: Hive::Machine,
    };
    let records = Records {
        uninstall: vec![
            record("GIMP 3.2.4", "3.2.4.0", Some(&gimp_folder), Some(&gimp)),
            record("Notepad++ (64-bit x64)", "8.9.6.4", None, None),
            record("DaVinci Resolve", "20.0.00049", None, None),
            record("DaVinci Resolve Control Panels", "2.3.2.0", None, None),
        ],
        app_paths: vec![AppPath { exe_name: "notepad++.exe".into(), path: notepad.clone() }],
    };
    Fixture { _dir: dir, sources: software::Sources { records, steam_root: Some(steam) }, gimp_folder, gimp, notepad }
}

/// The shipped catalog.
pub(crate) fn shipped_catalog() -> Vec<ToolEntry> {
    software::catalog::parse(&std::fs::read_to_string("data/external/catalog.json").unwrap()).unwrap()
}

/// An app on the Tools page, over the shipped catalog and `fx`'s records, showing `view`. Nothing a
/// test does writes the person's config.json.
pub(crate) fn page_with(fx: &Fixture, view: View) -> GuiState {
    crate::config::keep_saves_off_disk();
    let mut state = GuiState::default();
    state.active_page = GuiPage::Tools;
    state.tools_catalog = shipped_catalog();
    state.tools.sources = Where::These(fx.sources.clone());
    state.settings.tools_view = view;
    state
}

/// The software in one category (few enough tiles to be on screen at once).
fn category(id: &str) -> View {
    View { kind: Some("software".into()), category: Some(id.into()), ..View::default() }
}

/// THE SHELF SHOWS WHAT IS INSTALLED AND WHAT IS NOT: in Image Editing, GIMP (installed, its
/// version, "Installed" on its cover) has Launch and Krita, Paint.NET (not installed) have
/// Download, and Photopea (it runs in the browser) has Open website; in Video Editing, DaVinci
/// Resolve is installed with no program recorded, so it says to start it from its shortcut, with
/// no Launch and no Download; the kind pills carry their counts; the page's lines are there; a help
/// service's tile has Open website and its website's name. Drawing the page starts nothing.
///
/// Seen red 2026-10-19 with `main_button`'s Launch arm never taken: "GIMP has Launch" (left: 0,
/// right: 1).
#[test]
fn the_shelf_shows_what_is_installed_and_what_is_not() {
    let (ctx, theme) = ctx_and_theme();
    let fx = fixture("tl_shelf");
    let mut state = page_with(&fx, category("image-editing"));
    found_in(&ctx, &theme, &mut state);
    let out = settled(&ctx, &theme, &mut state);
    assert!(exact(&out, TITLE) && exact(&out, INTRO) && exact(&out, WHERE_FROM), "the heading, the intro line and where the programs come from");
    #[cfg(windows)]
    assert!(exact(&out, FOUND_LINE), "how what is installed is found");
    let n = state.tools_catalog.len();
    let software = state.tools_catalog.iter().filter(|e| e.kind == "software").count();
    for pill in [format!("{EVERYTHING} {n}"), format!("Software {software}"), format!("Help and services {}", n - software)] {
        assert!(exact(&out, &pill), "the {pill} pill");
    }
    assert!(in_row(&out, "GIMP", "Image Editing and Digital Art \u{b7} GPL").is_none(), "the tiles' lines are under their names, not beside them");
    assert_eq!(count(&out, LAUNCH), 1, "GIMP has Launch");
    assert!(exact(&out, "Installed \u{b7} 3.2.4.0"), "its version");
    assert_eq!(count(&out, INSTALLED), 1, "\"Installed\" on its cover");
    assert_eq!(count(&out, DOWNLOAD), 2, "Krita and Paint.NET have Download");
    assert_eq!(count(&out, OPEN_WEBSITE), 1, "Photopea, which runs in the browser, has Open website");
    assert!(exact(&out, "Runs in your web browser"));

    state.settings.tools_view = category("video-editing");
    let out = settled(&ctx, &theme, &mut state);
    assert!(exact(&out, "Installed \u{b7} 20.0.00049") && exact(&out, NO_PROGRAM_SHORT), "DaVinci Resolve is installed, with no program to start");
    assert_eq!((count(&out, LAUNCH), count(&out, DOWNLOAD)), (0, 3), "no Launch, and Download only on the three not installed");

    state.settings.tools_view = View { kind: Some("service".into()), category: Some("water".into()), ..View::default() };
    let out = settled(&ctx, &theme, &mut state);
    assert!(exact(&out, "Appropedia") && exact(&out, "appropedia.org"), "a help service's tile, with its website's name");
    assert_eq!((count(&out, OPEN_WEBSITE), count(&out, DOWNLOAD), count(&out, LAUNCH)), (3, 0, 0));
    assert!(recorder::recorded().is_empty(), "drawing the page starts nothing");
}

/// LAUNCH GOES THROUGH THE ONE LAUNCHER: GIMP's Launch asks for its own program in its own folder;
/// Notepad++'s for the program App Paths names; Blender's (installed by Steam) for Steam's own
/// link, and the page says it asked Steam; a second Launch straight after is the same press; Show
/// in folder (in the details) shows the program; a program that will not start says why.
///
/// Seen red 2026-10-19 with Launch sending the program's folder to be shown instead (`Act::Open(
/// ShowInFolder)` for Launch's click): "GIMP: its own program" got [ShowInFolder(".../GIMP 3/bin/gimp-3.exe")].
#[test]
fn launch_goes_through_the_one_launcher() {
    let (ctx, theme) = ctx_and_theme();
    let fx = fixture("tl_launch");
    let mut state = page_with(&fx, View { kind: Some("software".into()), sort: SortBy::InstalledFirst, layout: Layout::List, ..View::default() });
    found_in(&ctx, &theme, &mut state);
    let _ = recorder::recorded();
    // Where Launch is on a program's row now (the status line above the shelf moves the rows).
    let launch_on = |state: &mut GuiState, name: &str| {
        let out = settled(&ctx, &theme, state);
        in_row(&out, name, LAUNCH).unwrap_or_else(|| panic!("Launch on {name}'s row")).center()
    };

    let gimp = launch_on(&mut state, "GIMP");
    press(&ctx, &theme, &mut state, gimp);
    assert_eq!(recorder::recorded(), vec![Launch::Program { exe: fx.gimp.clone(), args: String::new(), dir: fx.gimp_folder.join("bin") }], "GIMP: its own program");
    assert_eq!(state.tools.status, Some(("Started GIMP.".into(), false)));
    let gimp = launch_on(&mut state, "GIMP");
    press(&ctx, &theme, &mut state, gimp);
    assert!(recorder::recorded().is_empty(), "a second Launch straight after is the same press");

    let notepad = launch_on(&mut state, "Notepad++");
    press(&ctx, &theme, &mut state, notepad);
    assert_eq!(recorder::recorded(), vec![Launch::Program { exe: fx.notepad.clone(), args: String::new(), dir: fx.notepad.parent().unwrap().to_path_buf() }], "Notepad++: the program App Paths names");

    let blender = launch_on(&mut state, "Blender");
    press(&ctx, &theme, &mut state, blender);
    assert_eq!(recorder::recorded(), vec![Launch::Link("steam://rungameid/365670".into())], "Blender from Steam: Steam's own link");
    assert_eq!(state.tools.status, Some(("Asked Steam to start Blender.".into(), false)));

    click(&ctx, &theme, &mut state, "GIMP");
    assert_eq!(state.tools.selected.map(|i| state.tools_catalog[i].name.clone()), Some("GIMP".to_string()), "its details open");
    click(&ctx, &theme, &mut state, SHOW_IN_FOLDER);
    assert_eq!(recorder::recorded(), vec![Launch::ShowInFolder(fx.gimp.clone())], "Show in folder shows its program");

    recorder::answer_with(Err("Windows could not start it (test)".into()));
    std::thread::sleep(std::time::Duration::from_millis(10));
    state.tools.last_launch = None;
    let notepad = launch_on(&mut state, "Notepad++");
    press(&ctx, &theme, &mut state, notepad);
    assert_eq!(state.tools.status, Some(("Notepad++ did not start: Windows could not start it (test).".into(), true)), "a program that will not start says why");
    recorder::answer_with(Ok(()));
}

/// AN ENTRY'S DETAILS: GIMP's (installed) say so with its version, and give Launch, Show in folder,
/// Website and Download page, its description, its licence, where it runs, its download size, the
/// file types its category opens, its version, folder and program, where it was found, and its
/// winget package; Website and Download page open their own addresses in the browser and start
/// nothing; a help service's details have Open website and no program facts; Close closes them.
///
/// Seen red 2026-10-19 with the category's file types not carried onto its entries (software/
/// catalog.rs `parse`): "the file types its category opens" failed.
#[test]
fn an_entrys_details() {
    let (ctx, theme) = ctx_and_theme();
    let fx = fixture("tl_details");
    let mut state = page_with(&fx, category("image-editing"));
    found_in(&ctx, &theme, &mut state);
    click(&ctx, &theme, &mut state, "GIMP");
    let out = settled(&ctx, &theme, &mut state);
    assert!(exact(&out, "Installed, version 3.2.4.0"), "installed, its version");
    for b in [LAUNCH, SHOW_IN_FOLDER, WEBSITE, DOWNLOAD_PAGE] {
        assert!(exact(&out, b), "{b} in its details");
    }
    assert!(!exact(&out, INSTALL), "no Install for a program already installed");
    let gimp = state.tools_catalog.iter().find(|e| e.name == "GIMP").unwrap().clone();
    assert!(exact(&out, &gimp.description), "its description");
    for (label, value) in [("Licence", "GPL".to_string()), ("Runs on", "Windows, Mac, Linux".to_string()), ("Download size", gimp.size.clone()), ("Version", "3.2.4.0".to_string()), ("winget package", "GIMP.GIMP.3".to_string())] {
        assert!(in_row(&out, label, &value).is_some(), "{label}: {value}");
    }
    assert!(in_row(&out, "Opens", &gimp.extensions.join(", ")).is_some(), "the file types its category opens");
    assert!(exact(&out, &fx.gimp_folder.display().to_string()) && exact(&out, &fx.gimp.display().to_string()), "its folder and its program");
    assert!(in_row(&out, "Found in", crate::software::Via::Windows.words()).is_some());

    let _ = recorder::recorded();
    let out = click(&ctx, &theme, &mut state, WEBSITE);
    assert_eq!(opened(&out), vec![(gimp.url.clone(), true)], "Website opens its own address");
    let out = click(&ctx, &theme, &mut state, DOWNLOAD_PAGE);
    assert_eq!(opened(&out), vec![(gimp.download.clone().unwrap(), true)], "Download page opens the project's own download page");
    assert!(recorder::recorded().is_empty(), "neither starts anything");

    state.settings.tools_view = View { kind: Some("service".into()), category: Some("water".into()), ..View::default() };
    click(&ctx, &theme, &mut state, "Appropedia");
    let out = settled(&ctx, &theme, &mut state);
    assert!(in_row(&out, "Website", "appropedia.org").is_some(), "a help service's website");
    assert!(!exact(&out, "Licence") && !exact(&out, "Found in") && !exact(&out, DOWNLOAD_PAGE), "no program facts");
    let opens: Vec<egui::Rect> = texts(&out).into_iter().filter(|(t, _)| t == OPEN_WEBSITE).map(|(_, r)| r).collect();
    assert_eq!(opens.len(), 4, "its details' Open website and the three tiles'");
    click(&ctx, &theme, &mut state, CLOSE);
    assert!(state.tools.selected.is_none(), "Close closes them");
}

/// The plans the install runner was asked to run on this thread since the last call, as lines.
fn winget_asked() -> Vec<String> {
    crate::software::install::recorder::recorded().iter().map(|p| p.line()).collect()
}

/// THE CONSENT STEP COMES BEFORE ANY INSTALL: Krita (not found, in winget) has Install in its
/// details; pressing it only asks winget about the package (`winget show`), and the details then
/// show the consent step: the package and its publisher, its licence, that it lists no agreements,
/// Microsoft's two lines exactly as winget prints them, Windows' permission prompt, winget's usage
/// data, and what pressing "I agree, install" means. Cancel installs nothing. Only "I agree,
/// install" runs winget's install, with `--accept-package-agreements` for that one package, and
/// never `--accept-source-agreements` or `--interactive` (Krita is free software). Nothing goes
/// through the program launcher. GIMP (installed), Photopea (in the browser) and a help service
/// have no Install.
///
/// Seen red 2026-10-19 with Install going straight to winget's install (`install_pressed` making
/// the install plan from an `Agreed` of its own): "pressing Install only asks winget about it"
/// got ["winget install --id KDE.Krita ..."].
#[cfg(windows)]
#[test]
fn the_consent_step_comes_before_any_install() {
    use crate::software::install::{self, recorder as winget};
    let (ctx, theme) = ctx_and_theme();
    let fx = fixture("tl_consent");
    let mut state = page_with(&fx, category("image-editing"));
    found_in(&ctx, &theme, &mut state);
    winget::answer_show(None);
    let _ = winget_asked();
    let _ = recorder::recorded();
    click(&ctx, &theme, &mut state, "Krita");
    click(&ctx, &theme, &mut state, INSTALL);
    assert_eq!(winget_asked(), vec!["winget show --id KDE.Krita --exact --source winget --disable-interactivity".to_string()], "pressing Install only asks winget about it");
    let out = settled(&ctx, &theme, &mut state);
    assert!(exact(&out, "Install Krita") && exact(&out, AGREE_INSTALL) && exact(&out, CANCEL), "the consent step, with its two buttons");
    assert!(in_row(&out, "Publisher", "Test Publisher").is_some() && in_row(&out, "Licence", "GPL-3.0").is_some(), "the package's publisher and licence");
    assert!(exact(&out, detail::NO_AGREEMENTS), "that it lists no agreements");
    assert!(exact(&out, &format!("\"{}\"", install::DISCLAIMER_OWNER)) && exact(&out, &format!("\"{}\"", install::DISCLAIMER_MICROSOFT)), "Microsoft's two lines, exactly");
    assert!(exact(&out, detail::PERMISSION_LINE) && exact(&out, detail::USAGE_DATA_LINE), "Windows' prompt and winget's usage data");
    assert!(exact(&out, &detail::agree_words("Krita")), "what pressing I agree means, said before it is pressed");
    assert!(!exact(&out, detail::OWN_INSTALLER_LINE), "a free program's installer is not shown");
    assert!(winget_asked().is_empty(), "nothing installed while it asks");

    click(&ctx, &theme, &mut state, CANCEL);
    assert!(state.tools.install.is_none() && winget_asked().is_empty(), "Cancel installs nothing");
    assert!(exact(&settled(&ctx, &theme, &mut state), "Cancelled. Nothing was installed."));

    click(&ctx, &theme, &mut state, INSTALL);
    let _ = winget_asked();
    click(&ctx, &theme, &mut state, AGREE_INSTALL);
    let asked = winget_asked();
    assert_eq!(asked, vec!["winget install --id KDE.Krita --exact --source winget --accept-package-agreements --no-upgrade --disable-interactivity".to_string()], "only now, that package, the person's agreement passed on");
    assert!(recorder::recorded().is_empty(), "nothing went through the program launcher");

    for name in ["GIMP", "Photopea"] {
        click(&ctx, &theme, &mut state, name);
        assert!(!exact(&settled(&ctx, &theme, &mut state), INSTALL), "{name} has no Install");
    }
    state.settings.tools_view = View { kind: Some("service".into()), category: Some("water".into()), ..View::default() };
    click(&ctx, &theme, &mut state, "Appropedia");
    assert!(!exact(&settled(&ctx, &theme, &mut state), INSTALL), "a help service has no Install");
}

/// A PROGRAM WHOSE LICENCE IS NOT FREE SOFTWARE SHOWS ITS MAKER'S INSTALLER: VS Code's consent step
/// says its maker's own installer opens with its licence page, links its licence (the catalog's
/// page for it), and its install adds `--interactive`; a package that lists agreements in winget
/// has them shown, line by line.
///
/// Seen red 2026-10-19 with `interactive` always false in `agree`: "its maker's installer is shown"
/// failed (no --interactive).
#[cfg(windows)]
#[test]
fn a_program_not_free_shows_its_makers_installer() {
    use crate::software::install::recorder as winget;
    let (ctx, theme) = ctx_and_theme();
    let fx = fixture("tl_interactive");
    let mut state = page_with(&fx, category("code-editors"));
    found_in(&ctx, &theme, &mut state);
    winget::answer_show(Some(Ok("Found Microsoft Visual Studio Code [Microsoft.VisualStudioCode]\nVersion: 1.142.0\nPublisher: Microsoft Corporation\nLicense: Microsoft Software License\nLicense Url: https://code.visualstudio.com/license\nAgreements:\n  Terms of use: https://example.org/terms\nTags:\n  editor\n".into())));
    let _ = winget_asked();
    click(&ctx, &theme, &mut state, "VS Code");
    click(&ctx, &theme, &mut state, INSTALL);
    let out = settled(&ctx, &theme, &mut state);
    assert!(exact(&out, detail::OWN_INSTALLER_LINE), "its maker's own installer and licence page");
    assert!(exact(&out, "Terms of use: https://example.org/terms") && !exact(&out, detail::NO_AGREEMENTS), "its agreements, line by line");
    let link = click(&ctx, &theme, &mut state, "Read the licence");
    assert_eq!(opened(&link), vec![("https://code.visualstudio.com/license".to_string(), true)], "its licence's own page");
    let _ = winget_asked();
    click(&ctx, &theme, &mut state, AGREE_INSTALL);
    let asked = winget_asked();
    assert!(asked.len() == 1 && asked[0].ends_with("--disable-interactivity --interactive") && !asked[0].contains("accept-source"), "its maker's installer is shown: {asked:?}");
    winget::answer_show(None);
}

/// AFTER WINGET INSTALLS IT, THE PAGE LOOKS FOR IT AGAIN, AND ITS TILE TURNS TO LAUNCH: with Krita
/// installed by winget (its record and program written into the fixture as the installer would),
/// the page reads again on its own, says it is installed, and Krita's tile has Launch. While winget
/// runs, its tile and details say so, with winget's latest line, and no second Install is offered.
///
/// Seen red 2026-10-19 with `installed_now` not starting a read: "the page looks for it again"
/// (left: 1, right: 2).
#[cfg(windows)]
#[test]
fn after_winget_installs_it_the_page_looks_again() {
    use crate::software::install::{self, recorder as winget};
    let (ctx, theme) = ctx_and_theme();
    let mut fx = fixture("tl_redetect");
    let mut state = page_with(&fx, category("image-editing"));
    found_in(&ctx, &theme, &mut state);
    let krita = state.tools_catalog.iter().position(|e| e.name == "Krita").unwrap();

    // While winget runs (a channel the test holds stands for its worker).
    let (tx, rx) = std::sync::mpsc::channel();
    state.tools.install = Some(Installing::Running { entry: krita, rx, line: String::new() });
    state.tools.selected = Some(krita);
    tx.send(install::Event::Line("Downloading https://download.kde.org/krita.exe".into())).unwrap();
    let out = settled(&ctx, &theme, &mut state);
    assert!(exact(&out, "Installing Krita...") && exact(&out, "Downloading https://download.kde.org/krita.exe"), "its details show winget's latest line");
    assert_eq!(count(&out, "Installing..."), 1, "its tile says so");
    click(&ctx, &theme, &mut state, "Paint.NET");
    assert!(exact(&settled(&ctx, &theme, &mut state), "One install at a time: another is under way.") && !exact(&settled(&ctx, &theme, &mut state), INSTALL), "no second Install");
    drop(tx);
    state.tools.install = None;

    // winget installs it: the installer has written its record and its program.
    let folder = fx.gimp_folder.parent().unwrap().join("Krita (x64)");
    let exe = program(&folder, "bin/krita.exe");
    fx.sources.records.uninstall.push(Uninstall { key: "Krita".into(), display_name: "Krita (x64) 5.3.4.1".into(), version: Some("5.3.4.1".into()), location: Some(folder.clone()), icon: None, hive: Hive::Machine });
    state.tools.sources = Where::These(fx.sources.clone());
    winget::answer_install(Ok("Successfully installed".into()));
    click(&ctx, &theme, &mut state, "Krita");
    click(&ctx, &theme, &mut state, INSTALL);
    let reads = state.tools.reads_started;
    click(&ctx, &theme, &mut state, AGREE_INSTALL);
    frame(&ctx, &theme, &mut state, 0.0, Vec::new());
    assert_eq!(state.tools.reads_started, reads + 1, "the page looks for it again");
    found_in(&ctx, &theme, &mut state);
    assert_eq!(state.tools.installed(krita).and_then(|f| f.program.clone()), Some(exe), "found, with its program");
    let out = settled(&ctx, &theme, &mut state);
    assert!(exact(&out, "Krita is installed."), "and says so");
    assert_eq!(count(&out, LAUNCH), 3, "its tile and its details have Launch now, beside GIMP's");
}

/// A FAILURE IS SHOWN IN WORDS: winget's hash check failing reads as a plain sentence with
/// winget's own words, in the details and the status line, and nothing is looked for again; winget
/// missing says so and points to Download.
///
/// Seen red 2026-10-19 with a failed install treated as done (`(false, Err(f))` falling into
/// `installed_now`): "nothing is looked for again" (left: 2, right: 1).
#[cfg(windows)]
#[test]
fn a_failure_is_shown_in_words() {
    use crate::software::install::{self, recorder as winget};
    let (ctx, theme) = ctx_and_theme();
    let fx = fixture("tl_failure");
    let mut state = page_with(&fx, category("image-editing"));
    found_in(&ctx, &theme, &mut state);
    let words = install::failure_words(Some(0x8A15_0011), "Installer hash does not match");
    winget::answer_install(Err(install::Failure { code: Some(0x8A15_0011), words: words.clone() }));
    click(&ctx, &theme, &mut state, "Krita");
    click(&ctx, &theme, &mut state, INSTALL);
    let reads = state.tools.reads_started;
    click(&ctx, &theme, &mut state, AGREE_INSTALL);
    let out = settled(&ctx, &theme, &mut state);
    assert_eq!(state.tools.reads_started, reads, "nothing is looked for again");
    assert!(exact(&out, &words), "in its details, in plain words with winget's own: {words}");
    assert_eq!(state.tools.status, Some((format!("Krita did not install: {words}"), true)));

    winget::answer_show(Some(Err(install::Failure { code: None, words: install::WINGET_MISSING.into() })));
    click(&ctx, &theme, &mut state, "Paint.NET");
    click(&ctx, &theme, &mut state, INSTALL);
    assert!(exact(&settled(&ctx, &theme, &mut state), install::WINGET_MISSING), "winget missing says so");
    assert!(state.tools.install.is_none());
    winget::answer_show(None);
    winget::answer_install(Ok("Successfully installed".into()));
}

/// WEBSITE ONLY, SAYING WHOSE RULE: DaVinci Resolve (not found here), Aseprite, Ardour, Vectr and
/// Boxy SVG never offer Install, and each says why in its details (the catalog's words, naming the
/// maker's own rule or terms); a program winget does not have says that instead.
///
/// Seen red 2026-10-19 with the reason left out of the details (the `website_only` branch never
/// taken): "DaVinci Resolve says why" failed.
#[test]
fn website_only_saying_whose_rule() {
    let (ctx, theme) = ctx_and_theme();
    let mut fx = fixture("tl_website_only");
    fx.sources.records.uninstall.retain(|u| !u.display_name.starts_with("DaVinci"));
    let mut state = page_with(&fx, View { kind: Some("software".into()), layout: Layout::List, ..View::default() });
    found_in(&ctx, &theme, &mut state);
    for name in ["DaVinci Resolve", "Aseprite", "Ardour", "Vectr", "Boxy SVG"] {
        // Lower case, so the search field's own text is not the name clicked.
        state.tools.search = name.to_lowercase();
        click(&ctx, &theme, &mut state, name);
        let out = settled(&ctx, &theme, &mut state);
        let why = state.tools_catalog.iter().find(|e| e.name == name).and_then(|e| e.website_only.clone()).unwrap();
        assert!(!exact(&out, INSTALL), "{name} never offers Install");
        assert!(exact(&out, &format!("No Install here. {why}")), "{name} says why");
    }
    #[cfg(windows)]
    {
        state.tools.search = "libresprite".into();
        click(&ctx, &theme, &mut state, "LibreSprite");
        assert!(exact(&settled(&ctx, &theme, &mut state), detail::NOT_IN_WINGET), "a program winget does not have says so");
    }
    assert!(crate::software::install::recorder::recorded().is_empty(), "winget was never asked anything");
}

/// THE VIEW, THE SORT AND THE FILTERS SURVIVE A RESTART: List, Installed first, Software and a
/// category are picked on the page, marked for saving, written by `from_gui_state` and read back by
/// `apply_to_gui_state` (what a restart does); the List draws its headings; a click on a heading
/// sorts by it; a new kind clears the category; a config written before they existed shows every
/// entry in a Grid by name.
///
/// Seen red 2026-10-19 with `apply_to_gui_state` not reading `tools_view`: "the view survives a
/// restart" (left: View { layout: Grid, sort: Name, kind: None, category: None }, right: View {
/// layout: List, ..}).
#[test]
fn the_view_the_sort_and_the_filters_survive_a_restart() {
    let (ctx, theme) = ctx_and_theme();
    let fx = fixture("tl_view");
    let mut state = page_with(&fx, View::default());
    found_in(&ctx, &theme, &mut state);
    assert_eq!(state.settings.tools_view, View::default(), "a Grid of everything by name first");
    click(&ctx, &theme, &mut state, crate::gui::widgets::shelf::LIST);
    assert!(state.settings_dirty, "kept in the settings");
    click(&ctx, &theme, &mut state, "Installed first");
    let software = state.tools_catalog.iter().filter(|e| e.kind == "software").count();
    click(&ctx, &theme, &mut state, &format!("Software {software}"));
    click(&ctx, &theme, &mut state, "Image Editing and Digital Art");
    let want = View { layout: Layout::List, sort: SortBy::InstalledFirst, kind: Some("software".into()), category: Some("image-editing".into()) };
    assert_eq!(state.settings.tools_view, want);
    let out = settled(&ctx, &theme, &mut state);
    assert!(exact(&out, "Licence") && exact(&out, "Runs on"), "the List's headings");
    let y = |n: &str| at(&out, n).unwrap_or_else(|| panic!("{n} is drawn")).center().y;
    assert!(y("GIMP") < y("Krita") && y("Krita") < y("Paint.NET"), "installed first, then by name");

    let json = serde_json::to_string(&crate::config::AppConfig::from_gui_state(&state)).unwrap();
    let back: crate::config::AppConfig = serde_json::from_str(&json).unwrap();
    let mut fresh = GuiState::default();
    back.apply_to_gui_state(&mut fresh);
    assert_eq!(fresh.settings.tools_view, want, "the view survives a restart");

    click(&ctx, &theme, &mut state, "Name");
    assert_eq!(state.settings.tools_view.sort, SortBy::Name, "a click on a heading sorts by it");
    let everything = format!("{EVERYTHING} {}", state.tools_catalog.len());
    click(&ctx, &theme, &mut state, &everything);
    assert_eq!((state.settings.tools_view.kind.clone(), state.settings.tools_view.category.clone()), (None, None), "a new kind clears the category");

    let mut old: serde_json::Value = serde_json::from_str(&json).unwrap();
    old.as_object_mut().unwrap().retain(|k, _| k != "tools_view");
    let older: crate::config::AppConfig = serde_json::from_value(old).unwrap();
    assert_eq!(older.tools_view, View::default(), "a config from before shows a Grid of everything by name");
}

/// THE SORT AND THE SEARCH: by name; installed first (each part by name); by category; the search
/// keeps entries whose name, category or description hold every word typed, letters' case aside;
/// the kind and category narrow. Through the page: typing in the search field leaves only what
/// matches, and the Grid sorted by category shows each category under its name.
///
/// Seen red 2026-10-19 with installed first comparing the wrong way round (`installed(a).cmp(
/// &installed(b))`): "installed first, by name" got ["7-Zip", "Ardour", "Aseprite", "Audacity"].
#[test]
fn the_sort_and_the_search() {
    let (ctx, theme) = ctx_and_theme();
    let fx = fixture("tl_sort");
    let mut state = page_with(&fx, View::default());
    found_in(&ctx, &theme, &mut state);
    let catalog = state.tools_catalog.clone();
    let found = state.tools.found.clone();
    let names = |view: View, search: &str| -> Vec<String> { order_of(&catalog, found.as_ref(), &view, search).into_iter().map(|i| catalog[i].name.clone()).collect() };
    let image = category("image-editing");
    assert_eq!(names(image.clone(), ""), vec!["GIMP", "Krita", "Paint.NET", "Photopea"]);
    let first = names(View { sort: SortBy::InstalledFirst, kind: Some("software".into()), ..View::default() }, "");
    assert_eq!(first[..4], ["Blender", "DaVinci Resolve", "GIMP", "Notepad++"], "installed first, by name");
    let by_cat = names(View { sort: SortBy::Category, kind: Some("software".into()), ..View::default() }, "");
    assert_eq!(by_cat[..4], ["Blender", "FreeCAD", "MeshLab", "Wings 3D"], "by category (3D Modeling first), then by name");
    assert_eq!(names(View::default(), "PAINTING"), vec!["Krita"], "the description, letters' case aside");
    assert_eq!(names(View::default(), "video editor multi"), vec!["Kdenlive"], "every word typed");
    assert!(names(View { kind: Some("service".into()), ..View::default() }, "crisis text").contains(&"Crisis Text Line".to_string()), "a help service by its description");
    assert!(names(View { kind: Some("software".into()), ..View::default() }, "crisis").is_empty(), "the kind narrows the search");

    state.settings.tools_view = View { sort: SortBy::Category, kind: Some("software".into()), ..View::default() };
    let out = settled(&ctx, &theme, &mut state);
    assert!(exact(&out, "3D Modeling and Animation") && exact(&out, "Image Editing and Digital Art"), "each category under its name");
    let field = at(&out, SEARCH_HINT).expect("the search field's hint").center();
    press(&ctx, &theme, &mut state, field);
    frame(&ctx, &theme, &mut state, 0.0, vec![egui::Event::Text("sprite".into())]);
    let out = settled(&ctx, &theme, &mut state);
    assert!(exact(&out, "LibreSprite") && !exact(&out, "GIMP"), "only what matches");
    frame(&ctx, &theme, &mut state, 0.0, vec![egui::Event::Text("zzz".into())]);
    assert!(exact(&settled(&ctx, &theme, &mut state), "Nothing here matches \"spritezzz\"."), "none matching says so");
}

/// WHAT IS INSTALLED IS FOUND WHEN THE PAGE OPENS AND ON REFRESH, NEVER EVERY FRAME, AND DRAWING
/// READS NOTHING: thirty frames of the open page start one read; the page opened again reads again;
/// Refresh reads again and shows what changed (GIMP's record gone); a Refresh while a read is under
/// way starts no second; and with a program's details open, sixty frames of the Grid and the List
/// read no file and no record on the UI thread (the counter moves when this thread reads).
///
/// Seen red 2026-10-19 with `draw` reading on every frame (`opened` always true): "thirty frames
/// start one read" (left: 3, right: 1).
#[test]
fn found_on_open_and_refresh_and_drawing_reads_nothing() {
    let (ctx, theme) = ctx_and_theme();
    let mut fx = fixture("tl_reads");
    let mut state = page_with(&fx, category("image-editing"));
    for _ in 0..30 {
        frame(&ctx, &theme, &mut state, 0.0, Vec::new());
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(state.tools.reads_started, 1, "thirty frames start one read");
    found_in(&ctx, &theme, &mut state);
    state.tools.last_drawn = Some(Instant::now() - Duration::from_secs(10));
    found_in(&ctx, &theme, &mut state);
    assert_eq!(state.tools.reads_started, 2, "opened again, it reads again");

    fx.sources.records.uninstall.retain(|u| !u.display_name.starts_with("GIMP"));
    state.tools.sources = Where::These(fx.sources.clone());
    assert_eq!(count(&settled(&ctx, &theme, &mut state), LAUNCH), 1, "until it reads again it shows what it found");
    click(&ctx, &theme, &mut state, REFRESH);
    assert_eq!(state.tools.reads_started, 3, "Refresh reads again");
    found_in(&ctx, &theme, &mut state);
    assert_eq!(count(&settled(&ctx, &theme, &mut state), LAUNCH), 0, "and shows what changed");
    start_reading(&mut state);
    start_reading(&mut state);
    assert_eq!(state.tools.reads_started, 4, "a Refresh while a read is under way starts no second one");
    found_in(&ctx, &theme, &mut state);

    click(&ctx, &theme, &mut state, "Krita");
    let before = crate::games::disk_reads_here();
    for _ in 0..30 {
        frame(&ctx, &theme, &mut state, 0.0, Vec::new());
    }
    state.settings.tools_view.layout = Layout::List;
    for _ in 0..30 {
        frame(&ctx, &theme, &mut state, 0.0, Vec::new());
    }
    assert_eq!(crate::games::disk_reads_here() - before, 0, "drawing reads nothing");
    let control = crate::games::disk_reads_here();
    let _ = software::read(&fx.sources, &state.tools_catalog);
    assert!(crate::games::disk_reads_here() > control, "the counter counts a read on this thread");
}

/// AN INSTALLED PROGRAM'S OWN ICON ON ITS TILE (Windows): GIMP's program, here a copy of Windows'
/// own Explorer, gives its icon, made a texture once; the programs not installed have none.
///
/// Seen red 2026-10-19 with `ensure_icons` not called from `draw`: "its icon is a texture" failed.
#[cfg(windows)]
#[test]
fn an_installed_programs_own_icon() {
    let (ctx, theme) = ctx_and_theme();
    let fx = fixture("tl_icon");
    let explorer = PathBuf::from(std::env::var("SystemRoot").unwrap()).join("explorer.exe");
    std::fs::copy(&explorer, &fx.gimp).unwrap();
    let mut state = page_with(&fx, category("image-editing"));
    found_in(&ctx, &theme, &mut state);
    settled(&ctx, &theme, &mut state);
    let gimp = state.tools_catalog.iter().position(|e| e.name == "GIMP").unwrap();
    assert!(state.tools.icons.contains_key(&gimp), "its icon is a texture");
    assert_eq!(state.tools.icons.len(), 1, "only the installed program that has one");
}
