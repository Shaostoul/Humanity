// Child of engine/world_hold.rs (BUG-253). Headless: a home whose garden would announce (a tomato
// flowering in an indoor tower, nothing to pollinate it), its FarmingSystem in a SystemRunner,
// ticked frame by frame through the same `tick` and `take_notices` lib.rs calls, and the toasts
// drawn into an egui context with no window after each frame, as the game draws them.

use std::sync::Mutex;

use humanity_core::ecs::components::{CropInstance, CropPollination, Irrigator};
use humanity_core::ecs::systems::SystemRunner;
use humanity_core::hot_reload::data_store::DataStore;
use humanity_systems::farming::fixtures::gardening::{make_store, set_garden_speed};
use humanity_systems::farming::FarmingSystem;

use super::{take_notices, tick};
use crate::gui::theme::{load_theme, Theme};
use crate::gui::{GuiPage, GuiState};

/// What the flowering notice says (farming/pollination.rs `flowering_notice`).
const POLLEN: &str = "indoors nothing carries their pollen";

/// A home whose garden would announce: the irrigation on (so nothing is about thirst), and a
/// tomato in flower in the indoor tower "ntower_3" with no hive, device or hand to pollinate it,
/// at 100x garden speed.
fn home() -> (hecs::World, DataStore, SystemRunner, hecs::Entity) {
    let mut data = make_store();
    set_garden_speed(&data, 100.0);
    data.insert("player_notices", Mutex::new(Vec::<String>::new()));
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    let tomato = world.spawn((CropInstance {
        crop_def_id: "tomato".to_string(),
        growth_stage: "flower".to_string(),
        planted_at: 0.0,
        water_level: 1.0,
        health: 100.0,
        tower_id: Some("ntower_3".to_string()),
        tower_slot: Some(0),
        health_seconds: 0.0,
        growing_seconds: 0.0,
    },));
    let mut runner = SystemRunner::new();
    runner.register(FarmingSystem::new());
    (world, data, runner, tomato)
}

/// The game's frame, its world part: the systems through `tick`, their notices through
/// `take_notices`, and the toasts drawn (which is where a notice becomes a toast).
struct Frames {
    ctx: egui::Context,
    theme: Theme,
    world: hecs::World,
    data: DataStore,
    runner: SystemRunner,
    tomato: hecs::Entity,
}

impl Frames {
    fn new() -> Self {
        let (world, data, runner, tomato) = home();
        let theme = load_theme();
        let ctx = egui::Context::default();
        theme.apply_to_egui(&ctx);
        Self { ctx, theme, world, data, runner, tomato }
    }

    fn run(&mut self, gui: &mut GuiState, world_loaded: bool, n: usize) {
        for _ in 0..n {
            tick(&mut self.runner, &mut self.world, &self.data, gui, world_loaded, 1.0);
            take_notices(&self.data, gui);
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(2560.0, 1440.0))),
                ..Default::default()
            };
            let theme = &self.theme;
            let _ = self.ctx.run(input, |ctx| crate::gui::widgets::draw_toasts(ctx, theme, gui));
        }
    }

    /// The garden days the tomato has spent in flower: the world's time, as the crop keeps it.
    fn flowering_days(&self) -> f64 {
        self.world.get::<&CropPollination>(self.tomato).map(|r| r.flowering_days).unwrap_or(0.0)
    }
}

fn toasts_saying(gui: &GuiState, words: &str) -> usize {
    gui.toasts.iter().filter(|t| t.text.contains(words)).count()
}

/// NOTHING OF A WORLD RUNS OR SPEAKS BEFORE ENTER (BUG-253): the app open on a page with the home
/// loaded at startup, then Play's picker open over the loaded world, raise no toast and grow
/// nothing (the tomato spends no time in flower); Enter shows the garden's notice, once, and the
/// world's time runs from then.
///
/// Seen red 2026-11-06 with `tick` and `take_notices` as lib.rs ran the frame before (the systems
/// ticked by the frame's time whatever the person was doing, their notices pushed straight to the
/// toasts): "on the dashboard before Play, no garden toast: 1 shown".
#[test]
fn the_picker_open_over_a_home_whose_garden_would_announce_raises_no_toast_until_enter() {
    let mut f = Frames::new();
    let mut gui = GuiState::default();

    // The app open on its first page, the home put on the world at startup, no 3D world yet.
    gui.active_page = GuiPage::Humanity;
    f.run(&mut gui, false, 30);
    assert_eq!(toasts_saying(&gui, POLLEN), 0, "on the dashboard before Play, no garden toast: {} shown", toasts_saying(&gui, POLLEN));
    assert!(!gui.world_running, "the world is held before the person enters it");

    // Play: the 3D world loads behind the picker (Who / Where), which is open over it.
    gui.active_page = GuiPage::None;
    gui.showroom_active = true;
    gui.showroom_mode = 0;
    f.run(&mut gui, true, 30);
    assert_eq!(toasts_saying(&gui, POLLEN), 0, "with the picker open, no garden toast: {:?}", gui.toasts.iter().map(|t| &t.text).collect::<Vec<_>>());
    assert!(gui.held_world_notices.iter().any(|n| n.contains(POLLEN)), "the garden's notice waits for the world: {:?}", gui.held_world_notices);
    assert_eq!(f.flowering_days(), 0.0, "no time passed for the garden before Enter");

    // The picker's Edit look is still the picker: held.
    gui.showroom_mode = 1;
    gui.showroom_return_to_picker = true;
    f.run(&mut gui, true, 5);
    assert_eq!(toasts_saying(&gui, POLLEN), 0, "in the picker's Edit look, no garden toast");
    gui.showroom_mode = 0;
    gui.showroom_return_to_picker = false;

    // Enter: the pairing committed, the picker closed (lib.rs's showroom_confirm).
    gui.showroom_active = false;
    f.run(&mut gui, true, 30);
    assert_eq!(toasts_saying(&gui, POLLEN), 1, "Enter shows the garden's notice once: {:?}", gui.toasts.iter().map(|t| &t.text).collect::<Vec<_>>());
    assert!(gui.world_entered && gui.world_running, "the world runs once entered");
    assert!(gui.held_world_notices.is_empty(), "nothing is left waiting");
    assert!(f.flowering_days() > 0.0, "the garden's time runs once the world is entered");

    // A page over the world after entering does not stop it (as it never has): the Inventory.
    gui.active_page = GuiPage::Inventory;
    let before = f.flowering_days();
    f.run(&mut gui, true, 5);
    assert!(f.flowering_days() > before, "a page over an entered world leaves it running");
}

/// A save put on the world while it is held (the picker's preview of another character) drops
/// what the world before it said while held, and its own "while you were away" waits for Enter
/// too: the person hears about the world they enter, not the one the picker showed first.
///
/// Seen red 2026-11-06 with `after_resume` pushing its notice straight to the toasts as before:
/// "the away notice waits for Enter: pending [\"While you were away ...\"]".
#[test]
fn a_load_while_held_drops_the_old_worlds_notices_and_its_own_waits() {
    let mut gui = GuiState::default();
    gui.active_page = GuiPage::None;
    gui.showroom_active = true;
    assert!(!gui.note_world_frame(true), "the picker holds the world");
    gui.world_notice("The old world's beds are flowering.".to_string());
    let save = humanity_systems::persistence::WorldSave::new_offline("t", "homestead");
    let r = crate::save_load::Resumed { away_secs: 3600.0, crops_aged: 3, ..Default::default() };
    crate::save_load::after_resume(&mut gui, &save, &r);
    assert!(gui.pending_notices.is_empty(), "the away notice waits for Enter: pending {:?}", gui.pending_notices);
    assert!(!gui.held_world_notices.iter().any(|n| n.contains("old world")), "the old world's line is dropped: {:?}", gui.held_world_notices);
    assert!(gui.held_world_notices.iter().any(|n| n.starts_with("While you were away")), "the load's own line waits: {:?}", gui.held_world_notices);
    gui.showroom_active = false;
    assert!(gui.note_world_frame(true), "Enter runs the world");
    assert!(gui.pending_notices.iter().any(|n| n.starts_with("While you were away")), "and shows what waited: {:?}", gui.pending_notices);
}
