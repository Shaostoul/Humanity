//! The cog's settings panel: its choice changes what the placed screen shows, and the choice
//! survives the home file's save and load. Driven through the panel itself on a headless egui
//! context (a `ScreenCore`, the same synthetic-event API a wall screen takes): the radio named "A
//! camera" is found where it is drawn and clicked.

use super::*;
use crate::gui::screen_surface::ScreenCore;
use crate::gui::theme::load_theme;
use humanity_ship::machines::ZoneRect;

/// A home with one wall screen showing the inventory and one camera post.
fn home() -> MachineHome {
    ron::from_str(
        r#"(
            catalog: {
                "wall_screen": (shape: "box", size: (1.2, 0.7, 0.05), color: (0.1, 0.1, 0.1), label: "Wall screen",
                    screen: Some((source: "inventory", px: (1280, 720), face: "front", bezel_m: 0.02, brightness: 1.0))),
                "camera_post": (shape: "box", size: (0.14, 1.7, 0.14), color: (0.2, 0.2, 0.2), camera: Some((0.0, -8.0, 70.0))),
            },
            instances: [
                (id: "wall_screen_1", machine: "wall_screen", room: "g", offset: (5.0, 1.2, 5.0)),
                (id: "camera_post_1", machine: "camera_post", room: "g", offset: (8.0, 0.0, 8.0)),
            ],
            connections: [],
        )"#,
    )
    .expect("the test home parses")
}

/// What placed screen `id` shows after the home is saved the way the editor saves it and loaded
/// back: the source its placement carries, parsed the way its surface parses it.
fn shows_after_save_and_load(home: &MachineHome, id: &str) -> ScreenSource {
    let text = ron::ser::to_string_pretty(home, ron::ser::PrettyConfig::default().struct_names(false)).expect("the home saves");
    let back: MachineHome = ron::from_str(&text).expect("the saved home loads");
    let zones = vec![ZoneRect { id: "home".into(), origin: (0.0, 0.0, 0.0), size: (20.0, 20.0, 4.0) }];
    let p = back.placements(&std::collections::HashMap::new(), Some(&zones)).into_iter().find(|p| p.id == id).expect("placed");
    ScreenSource::parse(&p.screen.expect("a screen").source)
}

/// The two rung-2 choices, written and read back through the save: always the Tasks page, then
/// always the camera post. A choice that is already made changes nothing.
///
/// Seen red 2026-11-06 with `set_show` not writing the source: "the same choice again changes
/// nothing" failed (nothing had been written, so the repeat still claimed a change).
#[test]
fn the_cogs_choice_changes_the_source_and_survives_a_save_and_load() {
    let mut home = home();
    assert_eq!(shows_after_save_and_load(&home, "wall_screen_1"), ScreenSource::Page(GuiPage::Inventory), "the def's page to start");
    assert!(set_show(&mut home, "wall_screen_1", &Show::Page(GuiPage::Tasks)));
    assert!(!set_show(&mut home, "wall_screen_1", &Show::Page(GuiPage::Tasks)), "the same choice again changes nothing");
    assert_eq!(shows_after_save_and_load(&home, "wall_screen_1"), ScreenSource::Page(GuiPage::Tasks));
    assert_eq!(camera_posts(&home), vec!["camera_post_1".to_string()]);
    assert!(set_show(&mut home, "wall_screen_1", &Show::Camera("camera_post_1".into())));
    assert_eq!(shows_after_save_and_load(&home, "wall_screen_1"), ScreenSource::Camera("camera_post_1".into()));
    assert!(set_size(&mut home, "wall_screen_1", Some((2.214, 1.245))));
    assert_eq!(current(&home, "wall_screen_1").map(|c| (c.2, c.3)), Some(((2.214, 1.245), true)), "a size from the panel is the screen's own");
    assert!(!showable_pages().contains(&GuiPage::None), "the engine's own page is not offered");
}

/// The panel itself: a click on its "A camera" radio, found where the panel drew it, makes the
/// screen show the camera post, and a click on "A HumanityOS page" makes it show a page again.
///
/// Seen red 2026-11-06 with the camera radio's click ignored (`if false && camera_radio.clicked()`):
/// "the camera radio chose the camera post" failed, the source still the def's "inventory".
#[test]
fn clicking_a_camera_in_the_panel_makes_the_screen_show_it() {
    let theme = load_theme();
    let mut state = GuiState::default();
    state.home_machines = Some(home());
    state.screen_settings_open = Some("wall_screen_1".into());
    let mut core = ScreenCore::new("panel_test", "tasks", 1280, 900, &theme);
    let run = |core: &mut ScreenCore, state: &mut GuiState| {
        core.run_with(state, |ctx, st| draw(ctx, &theme, st));
    };
    run(&mut core, &mut state);
    run(&mut core, &mut state);
    let click_text = |core: &mut ScreenCore, state: &mut GuiState, text: &str| {
        core.find_text(text);
        run(core, state);
        let found = core.take_found_text().flatten().unwrap_or_else(|| panic!("the panel draws {text:?}"));
        let c = found.rect.center();
        let uv = (c.x / 1280.0, c.y / 900.0);
        core.pointer_moved(uv);
        run(core, state);
        core.button(uv, true);
        run(core, state);
        core.button(uv, false);
        run(core, state);
    };
    click_text(&mut core, &mut state, "A camera");
    let source = state.home_machines.as_ref().and_then(|h| current(h, "wall_screen_1")).map(|c| c.1);
    assert_eq!(source.as_deref(), Some("camera:camera_post_1"), "the camera radio chose the camera post");
    assert!(state.construction_machines_dirty, "and asked the engine to follow");
    click_text(&mut core, &mut state, "A HumanityOS page");
    let source = state.home_machines.as_ref().and_then(|h| current(h, "wall_screen_1")).map(|c| c.1);
    assert_eq!(source.as_deref(), Some("page:tasks"), "the page radio chose a page again");
}
