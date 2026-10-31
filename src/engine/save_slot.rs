//! Which save is live, and the home it builds (ship homes increment 6,
//! docs/design/ship-homes-increment-6-plan.md sections 3.6 and 3.7).
//!
//! Your own home is saves/offline_home.json; each server you play on keeps its own save for you,
//! made FRESH from the start you choose there (the operator's rule of 2026-10-04: server worlds
//! start every player fresh; only your name, look and outfit come along). This module decides
//! which save is live (`wanted`, from the pairing the picker made, never a moment's step out of
//! the shared world), makes a server's fresh save (`fresh_save`), swaps the live save when the
//! pairing changes (`follow`: `write_active`, then `take_up`, which builds the save's home with
//! `build_home`), builds the home a save's start puts on its plot (`home_for`), and takes the
//! server's `game_home` answers (`on_game_message`, `choose_on_server`).
//!
//! WHERE IT RUNS (lib.rs):
//! - at startup, `boot` reads the config, picks the save the pairing asks for, reads it (a server's
//!   is made fresh when there is none) and makes it the live one, before the player is spawned
//!   with its start's kit and opening;
//! - every frame, `follow` runs after `home_plot::follow_server`, and the join gate waits for
//!   `slot_ready`, so a switch of server never joins with the previous server's start;
//! - the picker's Enter and the probe's verbs pair (`enter_pairing`), a server card's Choose asks
//!   for a home there (`choose_on_server`), and engine/net_route.rs hands `game_home` answers to
//!   `on_game_message`.
//!
//! The types they share are persistence's `SaveSlot`, `ShipPlace` and `ShipMeals`, and
//! ship::starts. Which save is live (`ActiveHome`), the choices waiting for a slot's fresh save, the
//! slot a pairing asks for (`slot_for`) and an erased account's save set aside are
//! persistence/live_save.rs; the questions the picker asks on the GUI state (this server's slot,
//! asking where you live, a card's Choose) are gui/pages/where_you_live.rs (the crate split's
//! rung 0, part B, 2026-10-31: the GUI reached up into this file for them).

use crate::engine::state::EngineState;
use crate::gui::GuiState;
use crate::persistence::live_save::{
    active_home, active_home_machines_path, choice_for, choose_new_home, forget_choice, remember_choice,
    restored_save_waiting, set_active_home, slot_for, ActiveHome, Choice,
};
use crate::persistence::{SaveSlot, SavedHome, ShipPlace, WorldSave};
use crate::ship::ship_structure::{HomeDesign, ShipStructure};
use crate::ship::starts::{HomeAnswer, Start, Starts};
use std::cell::RefCell;
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
///
/// An account erased on that server (gui/connections.rs `account_erased_here`) has no home there:
/// its save is set aside (`set_aside`) and your own home is live until the person signs up there
/// again (Connect), when a fresh save is made.
pub fn wanted(gui: &GuiState) -> SaveSlot {
    let server = gui.active_server_key();
    slot_for(&gui.launcher_last_world, &gui.profile_public_key, &server, gui.account_erased_here(&server))
}

/// The save the config asks for at startup (`wanted`, from the config before it is applied: its
/// pairing, identity, server and erased accounts). No config (a first run) is your own home.
pub fn boot_slot(config: Option<&crate::config::AppConfig>) -> SaveSlot {
    let Some(c) = config else { return SaveSlot::Offline };
    let server = crate::gui::pages::chat::norm_server_url(&c.server_url);
    let erased = c.account_erased_on.contains_key(&crate::gui::erased_entry(&c.public_key_hex, &server));
    slot_for(&c.last_world, &c.public_key_hex, &server, erased)
}

/// What `boot` found: the live save's slot, and its save (None: your own home has none yet).
#[derive(Debug, Clone)]
pub struct Boot {
    pub slot: SaveSlot,
    pub save: Option<WorldSave>,
}

/// At startup (lib.rs, before the player is spawned): the save the config's pairing asks for
/// (`boot_slot`) becomes the live one (`ActiveHome`, its start, household and place), so the player
/// is spawned with its start's kit and opening and the household file loaded is its own. Your own
/// home's save is read when it is there (none on a first run: the default home, as always). A
/// server's save is read, or made fresh at once (`open_slot`, the default start, with your own
/// home's character as its look), so a relaunch on a server pairing plays that server's save and
/// never your own home's. A server save that cannot be read at all (a disk error) is logged and
/// your own home is live; `follow` tries the server's again once the world is up.
pub fn boot(saves_dir: &Path, data_dir: &Path, config: Option<&crate::config::AppConfig>) -> Boot {
    let slot = boot_slot(config);
    let offline_path = crate::persistence::slot_path(saves_dir, &SaveSlot::Offline);
    let offline = read_save(&offline_path);
    if matches!(slot, SaveSlot::Server { .. }) {
        let starts = Starts::load(data_dir);
        let look = offline.as_ref().map(Look::of_save).unwrap_or_default();
        match open_slot(saves_dir, data_dir, &starts, &slot, None, None, &look) {
            Ok((save, path, fresh)) => {
                set_active_home(ActiveHome::of_save(slot.clone(), path, &save));
                log::info!("Startup: playing this server's save, a {} ({}){}", save.start, save.household, if fresh { ", made fresh" } else { "" });
                return Boot { slot, save: Some(save) };
            }
            Err(e) => log::warn!("Startup: {e}; your own home is live until the server's save can be read"),
        }
    }
    let home = match &offline {
        Some(save) => ActiveHome::of_save(SaveSlot::Offline, offline_path, save),
        None => ActiveHome {
            slot: SaveSlot::Offline,
            path: offline_path,
            start: crate::persistence::default_start(),
            household: crate::persistence::default_household(),
            server: String::new(),
            ship_place: None,
        },
    };
    set_active_home(home);
    Boot { slot: SaveSlot::Offline, save: offline }
}

/// A save file as it reads, None when there is none or it does not read (logged).
fn read_save(path: &Path) -> Option<WorldSave> {
    if !path.exists() {
        return None;
    }
    crate::persistence::load_world(path).map_err(|e| log::warn!("{}: {e}", path.display())).ok()
}

/// The join gate's word (lib.rs, `home_plot::JoinGate::slot_ready`): the live save is the one this
/// server's world wants, a server's save. A switch of server joins only once `follow` has taken up
/// that server's save, so the join never asks for the previous server's start; and your own home
/// (paired with a home, or no identity) never joins a server's world with your own things.
pub fn slot_ready(gui: &GuiState) -> bool {
    let want = wanted(gui);
    matches!(want, SaveSlot::Server { .. }) && gui.save_slot == want
}

/// Whether the live save may be swapped now (pure): the world has loaded (the home is built on
/// the ship it stands on), no snapshot restore is waiting to be loaded (its file must not be
/// written over first), and the build editor is shut (its edits are the live home's).
pub fn may_swap(world_loaded: bool, restore_waiting: bool, editor_open: bool) -> bool {
    world_loaded && !restore_waiting && !editor_open
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
    // A fresh body: a save with none gives the body a new character starts with. And a new mining
    // drone, full and unworn, with no standing order.
    save.body = None;
    save.mining_drone = None;
    save.mining_order = None;
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
    crate::play_journal::record("world", "Saved the game", serde_json::json!({ "server": state.gui_state.server_url }));
    look
}

/// The swap's second save step (3.6 steps 2 to 5): make `slot` the live save and put it on the
/// running world. Its file is read, or a fresh save of `start` and `household` written there
/// (`open_slot`); it becomes `ActiveHome` (before it is applied, so the household's machine file
/// it loads is its own, `machines::home_ron_path`); it goes onto the world as
/// `save_load::apply_onto_running_world` puts a save there (`take_up_before_home`), its home is
/// built on the ship (`build_home`: the save's start on the save's plot, with its household's
/// machines and what its start carries) and its pieces carried there, then `take_up_after_home`;
/// and `look`, the look the character had, is put back on, since the look crosses at every entry.
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
    // The home the save holds (its start on its plot, the character's own design or the default,
    // its household's machines), then its pieces carried to the plot the home stands on, as
    // `apply_onto_running_world` does.
    if let Err(e) = build_home(state) {
        log::warn!("Save swap: {e}; the home stays as it stood");
    }
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
    set_active_home(ActiveHome::of_save(slot.clone(), path.clone(), &save));
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

// ── The swap ─────────────────────────────────────────────────────────────────────────────────────

/// How long `follow` waits before it tries a swap that failed again (a save that could not be
/// read or written): a disk error does not clear in a frame, and retrying every frame would fill
/// the log.
pub const SWAP_RETRY_SECS: u64 = 30;

// Thread-local for the reason persistence::live_save's ACTIVE_HOME is: every swap runs on the one
// main thread, and a thread-local keeps each test's its own. (The choices waiting for a slot's fresh
// save are live_save's; the start a server card's Choose asked for is where_you_live.rs's.)
thread_local! {
    /// The slot a swap last failed for, and when.
    static BACKOFF: RefCell<Option<(SaveSlot, std::time::Instant)>> = const { RefCell::new(None) };
    /// The server the last `game_home` answer came from (its normalized address).
    static ANSWER_FROM: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Swap the live save when the pairing asks for another (3.6, every frame after
/// `home_plot::follow_server`): write the live one (`write_active`), read or make the other, put
/// it on the running world and build its home (`take_up`). Only while `may_swap` says so. A
/// choice waiting for a slot with no save yet is made into that save first (`write_chosen`), and
/// the live slot itself is taken up again when a choice waits for it and it has no save yet (the
/// new-home row on a first run). A swap that fails (a save that cannot be read or written) leaves
/// the live save as it was, says so once, and is tried again after `SWAP_RETRY_SECS`.
pub fn follow(state: &mut EngineState) {
    let want = wanted(&state.gui_state);
    let saves = crate::persistence::saves_dir();
    let unmade = |slot: &SaveSlot| !crate::persistence::slot_path(&saves, slot).exists();
    let chosen = choice_for(&want).filter(|_| unmade(&want));
    let swap = want != state.gui_state.save_slot;
    if !swap && chosen.is_none() {
        return;
    }
    if !may_swap(state.world_loaded, restored_save_waiting(), state.gui_state.construction_active) {
        return;
    }
    if BACKOFF.with(|b| b.borrow().as_ref().is_some_and(|(s, at)| *s == want && at.elapsed().as_secs() < SWAP_RETRY_SECS)) {
        return;
    }
    // The live save first (a slot taken up again for its choice has no file to write yet).
    let look = if swap { write_active(state) } else { Look::of_world(&state.game_world.world) };
    // No choice: the server's own word, when it says this identity already holds a home there.
    let chosen = chosen.or_else(|| known_home(&state.gui_state, &want).filter(|_| unmade(&want)));
    let result = match &chosen {
        Some(c) => write_chosen(&saves, &state.data_dir, &want, c, &look)
            .and_then(|_| take_up(state, &want, Some(&c.start), Some(&c.household), &look)),
        None => take_up(state, &want, None, None, &look),
    };
    match result {
        Ok(_) => {
            forget_choice(&want);
            BACKOFF.with(|b| *b.borrow_mut() = None);
        }
        Err(e) => {
            log::warn!("Save swap: {e}; staying in the save that is live, and trying again in {SWAP_RETRY_SECS} s");
            let told = BACKOFF.with(|b| b.borrow().as_ref().is_some_and(|(s, _)| *s == want));
            if !told {
                state.gui_state.pending_notices.push(format!("Your save for this world could not be opened ({e}); you are still in the one you were in."));
            }
            BACKOFF.with(|b| *b.borrow_mut() = Some((want, std::time::Instant::now())));
        }
    }
}

/// What the server's last `game_home` answer says this identity holds there (a home kept on
/// another computer, or kept by the server while this computer had no save), as a choice for
/// that server's fresh save: its start, the start's first household, and its plot. None for
/// another server's answer or your own home.
fn known_home(gui: &GuiState, slot: &SaveSlot) -> Option<Choice> {
    let SaveSlot::Server { server, .. } = slot else { return None };
    if ANSWER_FROM.with(|a| a.borrow().clone()) != *server {
        return None;
    }
    let held = gui.home_answer.as_ref()?.plot.as_ref()?;
    let starts = crate::gui::pages::where_you_live::starts();
    Some(Choice {
        start: held.start.clone(),
        household: starts.household(&held.start, "").id.clone(),
        place: place_on(gui.ship_structure.as_ref(), &held.id),
    })
}

/// Write the fresh save `choice` asks for at `slot`'s file, standing on its place aboard, when the
/// slot has no save yet (its fresh save would otherwise know no place). Nothing for a choice with
/// no place: `open_slot` makes that save itself.
fn write_chosen(saves_dir: &Path, data_dir: &Path, slot: &SaveSlot, choice: &Choice, look: &Look) -> Result<(), String> {
    let path = crate::persistence::slot_path(saves_dir, slot);
    if path.exists() || choice.place.is_none() {
        return Ok(());
    }
    let server = match slot {
        SaveSlot::Offline => "",
        SaveSlot::Server { server, .. } => server.as_str(),
    };
    let mut save = fresh_save(data_dir, &Starts::load(data_dir), &choice.start, &choice.household, server, look);
    save.ship_place = choice.place.clone();
    crate::persistence::save_world(&path, &save)?;
    log::info!("Made a fresh {} save of a {} ({}), on the plot chosen there", save.kind, save.start, save.household);
    Ok(())
}

/// Build the live save's home on the running ship (3.6 step 5): its start on its plot
/// (`home_for`: the place it stands on, else the first free kind of plot; the character's own
/// design, the data file in the Dev mode, the shipped design on a server), its household's
/// machines (`machines::home_ron_path` reads `ActiveHome`), the views of them refreshed, the home
/// rebuilt, the save's machine levels landed, the editor's undo started again from it, and what
/// its start carries of the homestead's world (world_load.rs `refresh_home_content`). The player
/// stands at its door, which is Respawn too.
///
/// The save's frame is published for the new home BEFORE the rebuild, so the rebuild carries
/// nothing along: the save just put on the world carries its own pieces to the home by its own
/// frame (`take_up`, `carry_saved_pieces_home`).
pub fn build_home(state: &mut EngineState) -> Result<(), String> {
    let active = active_home();
    let starts = Starts::load(&state.data_dir);
    // The live ship's file (a Dev edit of the ship is kept), else the ship file on disk.
    let file = match state.gui_state.ship_structure.as_ref() {
        Some(s) => s.ship_file(),
        None => ShipStructure::load_ship_file(&state.data_dir)?,
    };
    let dev = crate::engine::own_home::authors_data_files(state.gui_state.settings.play_mode);
    let own = state.gui_state.own_home.clone();
    let ship = home_for(file, &state.data_dir, &starts, &active.start, &active.household, active.ship_place.as_ref(), own.as_ref(), dev, &active.slot)?;
    reload_household(state);
    state.gui_state.construction_zone = ship.home_zone_index();
    state.gui_state.ship_structure = Some(ship);
    crate::engine::home_plot::publish_home_frame(state);
    crate::engine::home_meshes::rebuild_homestead(state);
    crate::systems::machine_levels::land_held(&mut state.game_world.world);
    let snap = crate::engine::editor::editor_snapshot(state);
    crate::engine::home_plot::history_after_move(&mut state.construction_history, snap);
    crate::engine::world_load::refresh_home_content(state);
    if let Some(door) = state.gui_state.ship_structure.as_ref().and_then(crate::engine::home_plot::own_door) {
        state.fps_spawn = door;
        crate::engine::home_plot::put_player_at(state, door);
    }
    state.boot_plot = state.gui_state.ship_structure.as_ref().and_then(|s| s.home_plot()).map(|p| p.id.clone());
    Ok(())
}

/// The live household's machines again (`machines::home_ron_path`, from `ActiveHome`), with the
/// character's own in the household's place (`own_home::machines_for_world`), and what is drawn
/// from them (`refresh_household_views`).
pub fn reload_household(state: &mut EngineState) {
    let path = active_home_machines_path(&state.data_dir);
    if let Some(home) = crate::engine::own_home::machines_for_world(&mut state.gui_state, &path) {
        state.gui_state.home_machines = Some(home);
    }
    refresh_household_views(&mut state.gui_state, &state.data_dir);
}

/// What the Home page and the garden draw from the household's machines, made again after the
/// household or the start changed (it was read only at startup): the homestead's loops and the
/// garden areas.
pub fn refresh_household_views(gui: &mut GuiState, data_dir: &Path) {
    gui.homestead_loops = gui.home_machines.as_ref().map(|h| h.loops.clone()).unwrap_or_default();
    if let Some(home) = gui.home_machines.as_ref() {
        gui.garden_areas = crate::gui::garden_areas_of(home, data_dir);
    }
}

/// Make the live save a home of `start` for `household` (a welcome that gives a plot of another
/// kind, `home_plot::apply_welcome_home`; a home chosen while out of the shared world): its start
/// and household in `ActiveHome`, so the next save writes them and the household file loaded is
/// the new one, and its machines reloaded. The caller builds the home.
pub fn change_start(state: &mut EngineState, start: &str, household: &str) {
    let mut active = active_home();
    if active.start == start && active.household == household {
        return;
    }
    log::info!("Where you live: now a {start} ({household})");
    active.start = start.to_string();
    active.household = household.to_string();
    set_active_home(active);
    reload_household(state);
}

// ── The home of a start ──────────────────────────────────────────────────────────────────────────

/// The row of `start` in `starts`, else the default start's (a save naming a start this version
/// does not know).
pub fn start_row<'a>(starts: &'a Starts, start: &str) -> &'a Start {
    starts.get(start).unwrap_or_else(|| starts.default_start())
}

/// The live save's start, read from `data_dir` (world_load.rs: what the home carries of the
/// homestead's world, and whether it breathes its own air).
pub fn live_start(data_dir: &Path) -> Start {
    start_row(&Starts::load(data_dir), &active_home().start).clone()
}

/// Whether the live save's home breathes its own air (`Start::own_air`; a cabin is on the ship's
/// ventilation), from the starts file read once per run: asked on every rebuild of the home.
pub fn live_own_air() -> bool {
    start_row(crate::gui::pages::where_you_live::starts(), &active_home().start).own_air
}

/// Whether the live save's start carries `content` of the homestead's world (`Start::home_content`:
/// the showcase garden's first planting, engine/hot_reload_gates.rs), from the starts file read
/// once per run: asked every frame.
pub fn live_carries(content: crate::ship::starts::HomeContent) -> bool {
    start_row(crate::gui::pages::where_you_live::starts(), &active_home().start).home_content.contains(&content)
}

/// The plot `place` names on `ship_file` for a home of `kind` (pure): its plot when the place was
/// taken on this same ship (its hash: a plot id names a place only on the same ship; after a ship
/// change it is not trusted) and that plot exists and takes a home of `kind`. None otherwise.
pub fn place_plot(ship_file: &ShipStructure, place: Option<&ShipPlace>, kind: &str) -> Option<String> {
    let p = place?;
    let here = p.ship_hash == ship_file.ship_hash() && ship_file.plots.iter().any(|q| q.id == p.plot && q.kind == kind);
    here.then(|| p.plot.clone())
}

/// The place aboard of the plot `plot` on `ship` (its ship's id and hash), None without a ship.
pub fn place_on(ship: Option<&ShipStructure>, plot: &str) -> Option<ShipPlace> {
    let ship = ship?;
    Some(ShipPlace { ship_id: ship.id.clone(), ship_hash: ship.ship_hash(), plot: plot.to_string() })
}

/// The design of a home of `start` when it is not the character's own (3.7, pure apart from
/// reading data/): in the Dev mode the data file (`HomeDesign::load`, which Dev authors); for a
/// server's save the SHIPPED design (`HomeDesign::built_in`), never data/homes/<kind>.ron, which an
/// offline Dev edit can change, so every player's home on a server is the one the server and
/// every neighbour draw; your own home the data file. `data_dir` None reads the shipped design.
pub fn design_of(data_dir: Option<&Path>, start: &Start, dev: bool, server: bool) -> Result<HomeDesign, String> {
    match data_dir {
        Some(d) if dev || !server => HomeDesign::load(d, &start.design),
        _ => HomeDesign::built_in(&start.design).ok_or_else(|| format!("no {} home design is built into this app", start.design)),
    }
}

/// The home a save's start puts on the ship (3.7, pure apart from reading data/): `ship_file`
/// with the home assembled on its plot, which is `ship_place`'s when it was taken on this ship
/// and takes the start's kind (`place_plot`), else the ship's default plot of that kind
/// (`ShipStructure::default_plot_of_kind`); and its design, which is the character's own (`own`)
/// when it is of that kind, outside the Dev mode, and goes on that plot, else `design_of`. Err
/// when the ship has no plot of the start's kind, or the design does not go on it. `household`
/// names the machine file, which the caller loads (`machines::home_ron_path`).
#[allow(clippy::too_many_arguments)]
pub fn home_for(
    ship_file: ShipStructure,
    data_dir: &Path,
    starts: &Starts,
    start: &str,
    _household: &str,
    ship_place: Option<&ShipPlace>,
    own: Option<&SavedHome>,
    dev: bool,
    slot: &SaveSlot,
) -> Result<ShipStructure, String> {
    let row = start_row(starts, start);
    let kind = row.plot_kind.as_str();
    let plot = place_plot(&ship_file, ship_place, kind)
        .or_else(|| ship_file.default_plot_of_kind(kind))
        .ok_or_else(|| format!("this ship has no {kind} plot for a {} home", row.name))?;
    assemble_start(ship_file, &plot, row, Some(data_dir), own, dev, matches!(slot, SaveSlot::Server { .. }))
}

/// A home of `start` assembled on `plot` of `ship_file` (`home_for`'s design rule, shared with a
/// welcome that gives a plot of another kind, engine/home_plot.rs): the character's own design
/// (`own`) when it is of the start's kind, outside the Dev mode, and goes on the plot; else
/// `design_of`. Pure apart from reading data/.
pub fn assemble_start(
    ship_file: ShipStructure,
    plot: &str,
    start: &Start,
    data_dir: Option<&Path>,
    own: Option<&SavedHome>,
    dev: bool,
    server: bool,
) -> Result<ShipStructure, String> {
    let kind = start.plot_kind.as_str();
    if let Some(own) = own.filter(|o| !dev && o.design.kind == kind) {
        match ship_file.clone().assemble(own.design.clone(), plot) {
            Ok(ship) => return Ok(ship),
            Err(e) => log::warn!("your own {kind} home does not go on plot {plot} ({e}); the {kind} design is used instead"),
        }
    }
    let design = design_of(data_dir, start, dev, server)?;
    ship_file.assemble(design, plot)
}

/// Where the live save's home stands aboard, from a welcome (`home_plot::plot_memory_after`):
/// `Some(plot)` on the ship the game runs, or `None` (a guest, a plot given back). A change is
/// written into the save at once (`WorldSave::ship_place`, as the remembered plot was written into
/// the config at once before increment 6), so the next world load builds the home there
/// (`home_for`) even when the app is closed without its save on quit. The save written then holds
/// the home's frame as it stood, which the next world load carries to the plot (`HomeFrame`).
pub fn set_ship_place(state: &mut EngineState, plot: Option<String>) {
    let place = plot.and_then(|p| place_on(state.gui_state.ship_structure.as_ref(), &p));
    let mut active = active_home();
    if active.ship_place == place {
        return;
    }
    log::info!("Where you live: this save's plot is now {:?} (kept in the save)", place.as_ref().map(|p| &p.plot));
    active.ship_place = place;
    set_active_home(active);
    crate::engine::own_home::refresh_own_home(&mut state.gui_state);
    crate::save_load::save_active_home(
        &state.game_world.world,
        &state.gui_state.placed_items,
        &state.data_store,
        !state.gui_state.settings.fresh_world_each_launch,
        state.gui_state.own_home.as_ref(),
    );
}

// ── The server's game_home answers ───────────────────────────────────────────────────────────────

/// The server's `game_home` answer (3.7, engine/net_route.rs): kept for the picker's server card
/// and the probe (`GuiState::home_answer`, `take_answer`). A choice made (`chose`, with the plot
/// now held) goes into the server's save (`took_home`); and a choice made while standing in the
/// world as a guest steps out and joins again, so the next welcome gives the plot (the relay hands
/// out plots only at a join). True when it was one.
pub fn on_game_message(state: &mut EngineState, v: &serde_json::Value) -> bool {
    let Some(answer) = HomeAnswer::from_message(v) else { return false };
    if let Some((slot, choice)) = take_answer(&mut state.gui_state, answer) {
        took_home(state, &slot, choice);
    }
    true
}

/// `on_game_message` on the GUI state (pure apart from the thread-locals it keeps): the answer is
/// kept, with the server it came from; a choice made (`chose`, and the plot held) is returned as
/// the choice for this identity's save on that server: the held plot's start, the household the
/// card picked for it (`choose_on_server`), else the start's first, and the plot as a place on the
/// ship the game runs.
pub fn take_answer(gui: &mut GuiState, answer: HomeAnswer) -> Option<(SaveSlot, Choice)> {
    ANSWER_FROM.with(|a| *a.borrow_mut() = gui.active_server_key());
    let taken = match (answer.chose, answer.plot.as_ref(), crate::gui::pages::where_you_live::server_slot(gui)) {
        (true, Some(held), Some(slot)) => {
            let asked = crate::gui::pages::where_you_live::take_asked();
            let household = asked.filter(|(s, _)| *s == held.start).map(|(_, h)| h).unwrap_or_default();
            let household = crate::gui::pages::where_you_live::starts().household(&held.start, &household).id.clone();
            let place = place_on(gui.ship_structure.as_ref(), &held.id);
            Some((slot, Choice { start: held.start.clone(), household, place }))
        }
        _ => None,
    };
    gui.home_answer = Some(answer);
    taken
}

/// What a home chosen on a server does to the save there (`took_home`, pure).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AfterChoice {
    /// Not the live save, and none yet: the choice waits for its fresh save (`follow`).
    Wait,
    /// Not the live save, kept from before (its home there was given back): its start, household
    /// and place are written into it.
    WriteInto,
    /// The live save, standing in the world as a guest: out and in again
    /// (`home_plot::respawn_through_relay`), and the welcome builds that kind's home on the plot.
    JoinAgain,
    /// The live save, in the world on a plot of its own (the relay refuses a second home, so this
    /// is never sent; kept for completeness): nothing more.
    Stay,
    /// The live save, out of the shared world: its start changes and its home is built now.
    BuildNow,
}

/// `AfterChoice` from where the save stands (pure): `live`, the choice is for the live save;
/// `exists`, that save has a file; `joined`, the game is in the shared world; `last_welcome`, the
/// last welcome's word (`EngineState::last_welcome`).
pub fn after_choice(live: bool, exists: bool, joined: bool, last_welcome: Option<&str>) -> AfterChoice {
    match (live, exists, joined) {
        (false, false, _) => AfterChoice::Wait,
        (false, true, _) => AfterChoice::WriteInto,
        (true, _, true) if last_welcome == Some("guest") => AfterChoice::JoinAgain,
        (true, _, true) => AfterChoice::Stay,
        (true, _, false) => AfterChoice::BuildNow,
    }
}

/// A home chosen on the server whose save is `slot` (`on_game_message`): what the save there says
/// from now on (`after_choice`). The live save keeps the place at once in every case.
pub fn took_home(state: &mut EngineState, slot: &SaveSlot, choice: Choice) {
    let live = state.gui_state.save_slot == *slot;
    let path = crate::persistence::slot_path(&crate::persistence::saves_dir(), slot);
    if live {
        let mut active = active_home();
        active.ship_place = choice.place.clone();
        set_active_home(active);
    }
    match after_choice(live, path.exists(), state.game_joined, state.last_welcome) {
        AfterChoice::Wait => remember_choice(slot, choice),
        AfterChoice::WriteInto => match crate::persistence::load_world(&path) {
            Ok(mut save) => {
                save.start = choice.start.clone();
                save.household = choice.household.clone();
                save.ship_place = choice.place.clone();
                if let Err(e) = crate::persistence::save_world(&path, &save) {
                    log::warn!("Where you live: the home chosen could not be written into the save there: {e}");
                }
            }
            Err(e) => log::warn!("Where you live: the save there does not read ({e}); the home chosen is not in it"),
        },
        AfterChoice::JoinAgain => {
            log::info!("Where you live: a {} chosen as a guest; joining again so the server's welcome gives it", choice.start);
            crate::engine::home_plot::respawn_through_relay(state);
        }
        AfterChoice::Stay => {}
        AfterChoice::BuildNow => {
            change_start(state, &choice.start, &choice.household);
            if let Err(e) = build_home(state) {
                log::warn!("Where you live: {e}; the home is built at the next world load");
            }
        }
    }
}

/// A server card's Choose (3.9; the probe's `{"choose_start":"cabin"}`): send `game_choose_home`
/// for `start` to the server the game talks to, with the household the card picked for it
/// (`where_you_live::draft_household`). Its answer (`on_game_message`) puts the home chosen into
/// that server's save. Err with the sentence to show when it cannot be asked.
pub fn choose_on_server(state: &mut EngineState, start: &str) -> Result<(), String> {
    crate::gui::pages::where_you_live::choose_on_server(&state.gui_state, start)
}

/// The picker's Enter (3.9; the probe's `{"pairing":"home"}` / `{"pairing":"server"}`): pair with
/// your own home (alone, out of the shared world) or with the connected server (its shared world),
/// record the pairing as Play repeats it, and save the config; `follow` then makes that save the
/// live one. Your own home with no save yet is made of the new-home row's choice
/// (`where_you_live::draft_choice`). Err with the sentence to show when the pairing cannot be
/// made.
pub fn enter_pairing(state: &mut EngineState, pairing: &str) -> Result<(), String> {
    enter_pairing_gui(&mut state.gui_state, pairing)
}

/// `enter_pairing` on the GUI state alone (the picker's Enter is drawn on it).
pub fn enter_pairing_gui(gui: &mut GuiState, pairing: &str) -> Result<(), String> {
    match pairing {
        "home" => {
            let own = read_save(&crate::persistence::slot_path(&crate::persistence::saves_dir(), &SaveSlot::Offline));
            gui.launcher_where_kind = crate::gui::LauncherWhere::Home;
            gui.launcher_selected_world = own.as_ref().map(|s| s.name.clone()).unwrap_or_default();
            if own.is_none() {
                let c = crate::gui::pages::where_you_live::draft_choice(crate::gui::pages::where_you_live::starts());
                choose_new_home(&c.start, &c.household);
            }
            gui.copresence_solo = true;
        }
        "server" => {
            if !gui.ws_client.as_ref().is_some_and(|w| w.is_connected()) {
                return Err("Connect to a server in Chat first.".into());
            }
            if crate::gui::pages::where_you_live::server_slot(gui).is_none() {
                return Err("A server keeps your home there under your identity: make one first (Settings > Identity).".into());
            }
            gui.launcher_where_kind = crate::gui::LauncherWhere::Server;
            gui.launcher_selected_server = Some(crate::gui::pages::showroom::CONNECTED_SERVER_ID.to_string());
            gui.copresence_solo = false;
        }
        other => return Err(format!("'{other}' is not a pairing: home or server")),
    }
    gui.record_pairing();
    crate::config::AppConfig::from_gui_state(gui).save();
    log::info!("Paired with {pairing}: {}", gui.launcher_last_world);
    Ok(())
}

/// A save the picker or a snapshot restore asked to load onto the running world (lib.rs, from
/// `save_load::take_pending_load`). Your own home's save while a server's is live (the picker's
/// character preview: its WHO rows are your own home's characters) puts only its look on: the
/// look crosses into every world, and its things would otherwise go into that server's save at
/// the next save. Any other save loads whole (`save_load::apply_onto_running_world`).
pub fn apply_pending_load(state: &mut EngineState, save: &WorldSave) {
    if active_home().is_server() && save.kind != "server" {
        Look::of_save(save).put_on(&mut state.game_world.world, &mut state.gui_state);
        log::info!("Launcher: '{}' wears that character's look here; this server's save keeps its own things", save.name);
        return;
    }
    crate::save_load::apply_onto_running_world(state, save);
    log::info!("Launcher: loaded character '{}'", save.name);
}

/// The save half of the swap (lane 1C): which save is live, a fresh server save, the apply.
#[cfg(test)]
#[path = "save_slot_tests.rs"]
mod save_half_tests;

/// The swap and the home (lane 2A).
#[cfg(test)]
#[path = "save_slot_home_tests.rs"]
mod home_tests;
