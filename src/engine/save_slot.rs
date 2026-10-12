//! Which save is live, and the home it builds (ship homes increment 6,
//! docs/design/ship-homes-increment-6-plan.md sections 3.6 and 3.7).
//!
//! Your own home is saves/offline_home.json; each server you play on keeps its own save for you,
//! made FRESH from the start you choose there (the operator's rule of 2026-10-04: server worlds
//! start every player fresh; only your name, look and outfit come along). This module decides
//! which save is live (`wanted`, from the pairing the picker made, never a moment's step out of
//! the shared world), makes a server's fresh save (`fresh_save`), swaps the live save when the
//! pairing changes (`follow`), builds the home a save's start puts on its plot (`home_for`), and
//! takes the server's `game_home` answers (`on_game_message`, `choose_on_server`).
//!
//! The save half (lane 1C) is built: `wanted`, `fresh_save`, and the swap's two save steps,
//! `write_active` (write the live save) and `take_up` (read or make the other, put it on the
//! running world, make it the live one). Nothing calls them yet: lane 2A wires them in. STILL
//! STUBS, for lane 2A to fill (each does what the game did before increment 6): `follow` (the swap
//! itself: when it runs, and the home it builds after `take_up`), `home_for`, `on_game_message`,
//! `enter_pairing` and `choose_on_server`. The types they share are persistence's `SaveSlot`,
//! `ShipPlace` and `ShipMeals`, save_load's `ActiveHome`, and ship::starts.

use crate::engine::state::EngineState;
use crate::persistence::{SaveSlot, SavedHome, ShipPlace, WorldSave};
use crate::ship::ship_structure::ShipStructure;
use crate::ship::starts::Starts;
use std::path::{Path, PathBuf};

/// What crosses into every world you enter: your character's name, look and outfit, nothing else.
#[derive(Debug, Clone, Default)]
pub struct Look {
    pub name: String,
    pub appearance: crate::ecs::components::Appearance,
    pub outfit: crate::ecs::components::Outfit,
}

impl Look {
    /// The live character's look (the player entity's name, appearance and outfit); the default
    /// look for a world with no player.
    pub fn of_world(world: &hecs::World) -> Look {
        use crate::ecs::components::{Appearance, Controllable, Name, Outfit};
        let mut q = world.query::<(&Name, &Appearance, &Outfit, &Controllable)>();
        q.iter()
            .next()
            .map(|(_e, (n, a, o, _))| Look { name: n.0.clone(), appearance: a.clone(), outfit: o.clone() })
            .unwrap_or_default()
    }

    /// The look a save carries.
    pub fn of_save(save: &WorldSave) -> Look {
        Look { name: save.character_name.clone(), appearance: save.appearance.clone(), outfit: save.outfit.clone() }
    }

    /// Put this look on the live character and on the editing copies the Appearance page and the
    /// avatar read (an empty name leaves the name as it is).
    pub fn put_on(&self, world: &mut hecs::World, gui: &mut crate::gui::GuiState) {
        use crate::ecs::components::{Appearance, Controllable, Name, Outfit};
        if let Some((_e, (n, a, o, _))) = world.query_mut::<(&mut Name, &mut Appearance, &mut Outfit, &Controllable)>().into_iter().next() {
            if !self.name.is_empty() {
                n.0 = self.name.clone();
            }
            *a = self.appearance.clone();
            *o = self.outfit.clone();
        }
        if !self.name.is_empty() {
            gui.character_name = self.name.clone();
        }
        gui.appearance = self.appearance.clone();
        gui.outfit = self.outfit.clone();
        gui.appearance_dirty = true;
        gui.outfit_dirty = true;
    }
}

/// The save the pairing asks for (3.6, pure): your own home when the picker's pairing is a
/// home (`launcher_last_world` starting "home:"), when there is no identity, or when there is no
/// server to keep a save for; else this server's save for this identity. It reads the pairing
/// only, never `copresence_solo`: Dev travel, fly mode and the `solo` verb step out of the shared
/// world for a moment and must never swap the save. An empty pairing (a config from before
/// pairings, a rig's fresh folder) is a server's save, the same rule `copresence_solo` starts
/// from (config.rs `apply_to_gui_state`).
pub fn wanted(gui: &crate::gui::GuiState) -> SaveSlot {
    let identity = gui.profile_public_key.trim().to_ascii_lowercase();
    if gui.launcher_last_world.starts_with("home:") || identity.is_empty() {
        return SaveSlot::Offline;
    }
    let server = crate::engine::home_plot::active_server_key(gui);
    if server.is_empty() {
        return SaveSlot::Offline;
    }
    SaveSlot::Server { server, identity }
}

/// A fresh save of the start `start` for `household` (3.6, pure apart from reading data/): what a
/// new character there has and nothing else. Made by spawning a new character of the start
/// (`save_load::spawn_player_of_start`: its kit in the backpack and its opening accepted) and the
/// new world's asteroids (`save_load::fresh_asteroids`) into a world of its own, then setting every
/// field: no skills, the default credits, a fresh body (`body: None`), the start's storage as the
/// home's, no own home, no crops, vehicles, drone, builds, machine levels, packs or settled trades,
/// no place aboard yet, no meals counted, and `look`'s name, look and outfit, which is all that
/// crosses from the world you came from (the operator's rule of 2026-10-04). `server` is the
/// server it is for ("server" saves); empty makes a new home of your own ("offline").
///
/// An unknown or unoffered start is the default (`Starts::resolve`), and a household the start
/// does not have is its first (`Starts::household`).
pub fn fresh_save(data_dir: &Path, starts: &Starts, start: &str, household: &str, server: &str, look: &Look) -> WorldSave {
    let start = starts.resolve(Some(start));
    let mut world = hecs::World::new();
    crate::save_load::spawn_player_of_start(&mut world, start);
    crate::save_load::fresh_asteroids(&mut world, data_dir);
    let new = crate::save_load::extract_world_save(&world);

    let mut save = WorldSave::new_offline(format!("My {}", start.name), start.id.clone());
    save.kind = if server.is_empty() { "offline" } else { "server" }.to_string();
    save.server = server.to_string();
    save.household = starts.household(&start.id, household).id.clone();
    save.progress_saved = true;
    // The start's kit, as a new character carries it.
    save.inventory = new.inventory;
    save.inventory_state = new.inventory_state;
    save.inventory_age = new.inventory_age;
    save.skills = Default::default();
    save.credits = crate::ecs::components::Wallet::default().credits;
    // The start's opening and nothing done: Some, so a load replaces the quests of the world it
    // lands on.
    save.quests = Some(new.quests.unwrap_or_default());
    // The new world's asteroids: Some, so a load replaces the ones mined down at home.
    save.asteroids = Some(new.asteroids.unwrap_or_default());
    // The start's storage: Some, so a load replaces the pool of the home it lands on.
    let places = crate::save_load::start_places(data_dir, &start.id);
    save.placed_items = Some(crate::gui::flatten_placed_items(&places));
    // A fresh body: a save with none gives the body a new character starts with.
    save.body = None;
    save.home = None;
    save.character_name = look.name.clone();
    save.appearance = look.appearance.clone();
    save.outfit = look.outfit.clone();
    save
}

/// The save of `slot` under `saves_dir` and its file, and whether it was made just now: the file
/// when it reads; else a fresh save (`fresh_save`) of `start` (the picker's choice; None: the
/// default) for `household` (None: the start's first), written there at once. A file that is
/// there but is not a save this version reads is set aside beside it
/// (`<name>.unreadable-<ms>.json`, never deleted) and a fresh one made; a file that cannot be read
/// at all (locked, a disk error) is an error, and nothing changes.
#[allow(clippy::too_many_arguments)]
pub fn open_slot(
    saves_dir: &Path,
    data_dir: &Path,
    starts: &Starts,
    slot: &SaveSlot,
    start: Option<&str>,
    household: Option<&str>,
    look: &Look,
) -> Result<(WorldSave, PathBuf, bool), String> {
    let path = crate::persistence::slot_path(saves_dir, slot);
    match std::fs::read(&path) {
        Ok(bytes) => match serde_json::from_slice::<WorldSave>(&bytes) {
            Ok(save) => return Ok((save, path, false)),
            Err(e) => {
                let aside = path.with_extension(format!("unreadable-{}.json", crate::persistence::now_ms()));
                std::fs::rename(&path, &aside)
                    .map_err(|re| format!("{} is not a save this version reads ({e}), and could not be set aside: {re}", path.display()))?;
                log::warn!("{} is not a save this version reads ({e}); set aside as {}, and a fresh one made", path.display(), aside.display());
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("could not read {}: {e}", path.display())),
    }
    let server = match slot {
        SaveSlot::Offline => "",
        SaveSlot::Server { server, .. } => server.as_str(),
    };
    let start = start.unwrap_or(starts.default.as_str());
    let household = household.unwrap_or("");
    let save = fresh_save(data_dir, starts, start, household, server, look);
    crate::persistence::save_world(&path, &save)?;
    log::info!("Made a fresh {} save of a {} ({})", save.kind, save.start, save.household);
    Ok((save, path, true))
}

/// The swap's first save step (3.6 step 1): write the live save as it stands, the way leaving the
/// game does (`quit::save_before_exit`): a machine removed in the last frame gives its item back,
/// the build editor's unsaved edits are kept, the character's own home is brought up to date, and
/// then the save is written (your own home's progress only while "Start every session from the
/// default home" is off; a server's always, `save_load::save_active_home`). Returns the look the
/// character had, which crosses into the next save (`take_up`).
pub fn write_active(state: &mut EngineState) -> Look {
    let look = Look::of_world(&state.game_world.world);
    crate::engine::own_home::refund_removed_machines(state);
    crate::engine::editor::autosave_ship_structure(state, true);
    crate::engine::own_home::refresh_own_home(&mut state.gui_state);
    crate::save_load::save_active_home(
        &state.game_world.world,
        &state.gui_state.placed_items,
        &state.data_store,
        !state.gui_state.settings.fresh_world_each_launch,
        state.gui_state.own_home.as_ref(),
    );
    look
}

/// The swap's second save step (3.6 steps 2 to 4): make `slot` the live save and put it on the
/// running world. Its file is read, or a fresh save of `start` and `household` written there
/// (`open_slot`); it becomes `ActiveHome` (before it is applied, so the household's machine file
/// it loads is its own, `machines::home_ron_path`); it goes onto the world as
/// `save_load::apply_onto_running_world` puts a save there (`take_up_before_home`, the home
/// rebuilt, `take_up_after_home`); and `look`, the look the character had, is put back on, since
/// the look crosses at every entry. Returns the save, whose home the caller then builds on the
/// ship (lane 2A, `home_for`).
///
/// Your own home while "Start every session from the default home" is on (or a save of it that
/// carries no progress) is the DEFAULT home, as at a launch: a fresh home of its start, never what
/// the server's world held, with the character from the file. The file is left as it is.
pub fn take_up(
    state: &mut EngineState,
    slot: &SaveSlot,
    start: Option<&str>,
    household: Option<&str>,
    look: &Look,
) -> Result<WorldSave, String> {
    let starts = Starts::load(&state.data_dir);
    let taken = take_up_before_home(
        &mut state.game_world.world,
        &mut state.data_store,
        &mut state.gui_state,
        &crate::persistence::saves_dir(),
        &state.data_dir,
        &starts,
        slot,
        start,
        household,
        look,
    )?;
    // The home the save holds (the character's own or the default, machines and walls), and its
    // pieces carried to the plot the home stands on, as `apply_onto_running_world` does.
    crate::engine::own_home::reapply_saved_home(state, &taken.applied);
    crate::engine::home_plot::carry_saved_pieces_home(state, taken.applied.home_plot_box);
    take_up_after_home(&mut state.game_world.world, &state.data_store, &mut state.gui_state, &taken, look);
    state.station_spawn_snap = true;
    log::info!("Now playing the {} save: a {} ({})", taken.save.kind, taken.save.start, taken.save.household);
    Ok(taken.save)
}

/// What `take_up_before_home` took up.
#[derive(Debug, Clone)]
pub struct TakenUp {
    /// The slot's save, as its file holds it (or as it was just written).
    pub save: WorldSave,
    /// What went on the world: `save`, or your own home's default home (`what_to_apply`).
    pub applied: WorldSave,
    /// The slot's file.
    pub path: PathBuf,
    /// Made just now (there was no save of this slot).
    pub fresh: bool,
}

/// `take_up` before the home is rebuilt, pure on the world, the DataStore and the GUI state (a test
/// drives it): open the slot, make it the live save, and put what applies on the world
/// (`save_load::put_save_on_world`, its progress always).
#[allow(clippy::too_many_arguments)]
pub fn take_up_before_home(
    world: &mut hecs::World,
    data: &mut crate::hot_reload::data_store::DataStore,
    gui: &mut crate::gui::GuiState,
    saves_dir: &Path,
    data_dir: &Path,
    starts: &Starts,
    slot: &SaveSlot,
    start: Option<&str>,
    household: Option<&str>,
    look: &Look,
) -> Result<TakenUp, String> {
    let (save, path, fresh) = open_slot(saves_dir, data_dir, starts, slot, start, household, look)?;
    let applied = what_to_apply(data_dir, starts, &save, gui.settings.fresh_world_each_launch);
    crate::save_load::set_active_home(crate::save_load::ActiveHome::of_save(slot.clone(), path.clone(), &save));
    gui.save_slot = slot.clone();
    crate::save_load::put_save_on_world(world, data, gui, data_dir, &applied, true);
    Ok(TakenUp { save, applied, path, fresh })
}

/// `take_up` after the home is rebuilt (pure, like the first half): the clock, the home's storage
/// and the rest of `save_load::settle_save_on_world`, then the look the character had put back on.
pub fn take_up_after_home(
    world: &mut hecs::World,
    data: &crate::hot_reload::data_store::DataStore,
    gui: &mut crate::gui::GuiState,
    taken: &TakenUp,
    look: &Look,
) {
    crate::save_load::settle_save_on_world(world, data, gui, &taken.applied, true);
    look.put_on(world, gui);
}

/// The save `take_up` puts on the world for `save` (pure apart from reading data/): the save itself
/// when its progress applies (`save_load::applies_progress`), else a fresh home of its start and
/// household carrying its character (your own home's default home).
pub fn what_to_apply(data_dir: &Path, starts: &Starts, save: &WorldSave, fresh_world_each_launch: bool) -> WorldSave {
    if crate::save_load::applies_progress(save, fresh_world_each_launch) {
        return save.clone();
    }
    let mut home = fresh_save(data_dir, starts, &save.start, &save.household, &save.server, &Look::of_save(save));
    home.kind = save.kind.clone();
    home.settled_trades = save.settled_trades.clone();
    home
}

/// Swap the live save when the pairing asks for another (3.6, every frame after
/// `home_plot::follow_server`): write the live one, read or make the other, apply it onto the
/// running world, and build its home.
///
/// STUB (lane 2A fills it): nothing; one save serves every world.
pub fn follow(_state: &mut EngineState) {}

/// The home a save's start puts on the ship (3.7, pure): its plot (`ship_place` when this ship's
/// and of the start's kind, else `ShipStructure::default_plot_of_kind`) and its design (the
/// save's own home outside the Dev mode, the data file in the Dev mode, the SHIPPED design on a
/// server, the data file offline).
///
/// STUB (lane 2A fills it): refuses, so nothing builds from it yet.
#[allow(clippy::too_many_arguments)]
pub fn home_for(
    _ship_file: ShipStructure,
    _data_dir: &Path,
    _starts: &Starts,
    _start: &str,
    _household: &str,
    _ship_place: Option<&ShipPlace>,
    _own: Option<&SavedHome>,
    _dev: bool,
    _slot: &SaveSlot,
) -> Result<ShipStructure, String> {
    Err("the home of a start is built from increment 6's Wave 2 on (engine/save_slot.rs home_for)".into())
}

/// The server's `game_home` answer (3.7): kept for the picker and Settings; a `chose` while a
/// guest steps out and joins again so the welcome gives the plot. True when it was one.
///
/// STUB (lane 2A fills it): takes none.
pub fn on_game_message(_state: &mut EngineState, _v: &serde_json::Value) -> bool {
    false
}

/// The picker's Enter (3.9; the probe's `{"pairing":"home"}` / `{"pairing":"server"}`): pair
/// with your own home or the connected server, which `follow` then makes the live save.
///
/// STUB (lane 2A fills it).
pub fn enter_pairing(_state: &mut EngineState, _pairing: &str) -> Result<(), String> {
    Err("choosing where you live comes with increment 6's Wave 2".into())
}

/// A server card's Choose (3.9; the probe's `{"choose_start":"cabin"}`): send
/// `game_choose_home` for `start`, and on `chose: true` write the server save's start and place.
///
/// STUB (lane 2A fills it).
pub fn choose_on_server(_state: &mut EngineState, _start: &str) -> Result<(), String> {
    Err("choosing where you live comes with increment 6's Wave 2".into())
}

/// The save half of the swap (lane 1C): which save is live, a fresh server save, the apply.
#[cfg(test)]
#[path = "save_slot_tests.rs"]
mod save_half_tests;
