//! The character's OWN home (2026-10-04): what the build editor changes outside the Dev mode is
//! kept in the character's save, never in the shared data files.
//!
//! WHY. Until this, every play mode wrote the editor's edits into the data files
//! (data/machines/home.ron, data/homes/<kind>.ron), which are the DEFAULT home every new player
//! starts from. Once fresh installs started in Normal mode with progress kept (the operator's
//! decision of 2026-10-04, src/config.rs), that let a Normal player:
//!   - keep a placed machine through "Start every session from the default home" for free (the
//!     machine was in the data file, so the fresh home had it, while the item was spent);
//!   - get a machine back after a snapshot restore that had already given its item back (the
//!     restore rewinds the save, not the data file);
//!   - put that machine in every other character's home on the install.
//!
//! THE RULE (`Capability::DefaultHomeAuthoring`, Dev only):
//!   - Dev writes the data files, exactly as before: the operator authors the default home and
//!     the ship in-game, and every test rig is a Dev sandbox (scripts/lib/rig-gameplay.js).
//!   - Normal and Creative keep the character's own home in their save (`WorldSave::home`,
//!     `persistence::SavedHome`): the home DESIGN (walls, openings, lights, stairs, the spawn
//!     point; the whole home zone's body) and the household's MACHINES (instances, arrays,
//!     connections, conduit graph, and what was paid for each). The precedent is
//!     `WorldSave::constructions`, the blueprint builds moved into the save for the same reason.
//!
//! STRUCTURAL EDITS GO INTO THE SAVE TOO, with the machines, because the home's two halves are
//! one thing to the player and to the world: machines stand at zone-local metres inside the
//! design's walls, so keeping one without the other would put a saved machine inside a wall the
//! default home has moved. The design is the same `HomeDesign` a data file holds, assembled onto
//! the player's plot at world load in place of the file's design of the same kind (when it fits
//! the plot; otherwise the file's, with a log line).
//!
//! A character with no own home (None) lives in the default home, and an update to the data
//! files reaches them. Their home becomes their own at the first edit outside Dev, from then on.
//!
//! PAYMENTS (`MachineHome::paid`): a placement outside Creative and Dev takes the machine's item
//! (engine::editor `pay_for_machine`). Removing the machine by ANY path gives the item back
//! (`refund_removed_machines`, every frame before the editor's history tick, so no undo snapshot
//! ever holds a payment for a machine that is gone); an undo or redo that brings a paid machine
//! back takes the item again, and is refused when the player no longer has it
//! (`settle_restore`).

use crate::config::{Capability, PlayMode};
use crate::engine::state::EngineState;
use crate::gui::GuiState;
use crate::machines::MachineHome;
use crate::persistence::SavedHome;
use crate::ship::ship_structure::{HomeDesign, ShipStructure};
use std::collections::BTreeMap;
use std::path::Path;

/// Whether this play mode's build edits are written to the shared data files (the Dev mode) or
/// kept in the character's own save (Normal, Creative).
pub(crate) fn authors_data_files(mode: PlayMode) -> bool {
    mode.allows(Capability::DefaultHomeAuthoring)
}

/// The character's own home that applies now: the save's, unless the Dev mode is authoring the
/// data files (Dev always builds and sees the default home).
pub(crate) fn in_effect(gui: &GuiState) -> Option<&SavedHome> {
    gui.own_home.as_ref().filter(|_| !authors_data_files(gui.settings.play_mode))
}

/// The home design the world load puts on the player's plot in place of the design file's, if
/// any (`ShipStructure::assemble_from_own`).
pub(crate) fn design_in_effect(gui: &GuiState) -> Option<&HomeDesign> {
    in_effect(gui).map(|o| &o.design)
}

/// The character's own home as the editor has it now: the assembled ship's home design and the
/// household's machine rows. None when the ship was not assembled from a design (the legacy
/// fallback layout), which has no home of the player's to keep.
pub(crate) fn capture(ship: Option<&ShipStructure>, machines: Option<&MachineHome>) -> Option<SavedHome> {
    let design = ship?.home_design()?;
    Some(SavedHome { design, machines: machines.map(|m| m.household_rows()).unwrap_or_default() })
}

/// Put the own home's machines into a layout loaded from the data files (nothing for None).
pub(crate) fn overlay_machines(home: &mut MachineHome, own: Option<&SavedHome>) {
    if let Some(own) = own {
        home.set_household_rows(own.machines.clone());
    }
}

/// Where the editor's edits went.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Kept {
    /// The shared data files (the Dev mode); the line the editor shows.
    DataFiles(String),
    /// The character's own home (`GuiState::own_home`); the caller writes the save.
    Save(String),
}

/// Keep the build editor's edits where this play mode keeps them. Dev: the shared data files,
/// exactly as before 2026-10-04 (the home design always, the ship file and the ship's machines,
/// the household's machines at `machines_path`). Normal and Creative: the home captured into
/// `gui.own_home`, and NO file written (the caller writes the save, `keep_edits_now`). Pure on
/// `gui` and the files under `data_dir`.
pub(crate) fn keep_editor_edits(gui: &mut GuiState, data_dir: &Path, machines_path: &Path) -> Result<Kept, String> {
    if authors_data_files(gui.settings.play_mode) {
        return write_data_files(gui, data_dir, machines_path).map(Kept::DataFiles);
    }
    keep_own_home(gui).map(Kept::Save)
}

/// The Dev mode's save (the editor's Save button and autosave until 2026-10-04, unchanged): the
/// home design always, the ship file and the ship's machines with ShipStructureEditing, then the
/// household's machines. Each is attempted; Err names what was not written.
pub(crate) fn write_data_files(gui: &GuiState, data_dir: &Path, machines_path: &Path) -> Result<String, String> {
    let mut errors: Vec<String> = Vec::new();
    let mut note = String::new();
    if let Some(ship) = gui.ship_structure.as_ref() {
        let ship_scope = gui.settings.play_mode.allows(Capability::ShipStructureEditing);
        match ship.save_assembled(data_dir, ship_scope) {
            Ok(n) => {
                note = n;
                if ship_scope {
                    if let Some(machines) = gui.home_machines.as_ref() {
                        match machines.save_ship_part(machines_path) {
                            Ok(Some(p)) => log::info!("Ship machines written to {}", p.display()),
                            Ok(None) => {}
                            Err(e) => errors.push(format!("The ship's machines were NOT saved: {e}")),
                        }
                    }
                }
            }
            Err(e) => errors.push(e),
        }
    }
    if let Some(machines) = gui.home_machines.as_ref() {
        match machines.save(machines_path) {
            Ok(()) => {
                log::info!("Machine layout written to {}", machines_path.display());
                if note.is_empty() {
                    note = "Saved the machines.".to_string();
                }
            }
            Err(e) => errors.push(format!("The machines were NOT saved: {e}")),
        }
    }
    if errors.is_empty() {
        Ok(note)
    } else {
        Err(format!("{note} {}", errors.join(" ")).trim().to_string())
    }
}

/// Capture the character's own home into `gui.own_home` (Normal and Creative): from now on it is
/// their home, written into every save that keeps progress. The line the editor shows says
/// where it went, and that it is kept for this session only while "Start every session from the
/// default home" is on.
pub(crate) fn keep_own_home(gui: &mut GuiState) -> Result<String, String> {
    let own = capture(gui.ship_structure.as_ref(), gui.home_machines.as_ref()).ok_or_else(|| {
        "Not kept: your home did not load from its design (the fallback layout is showing), so \
         outside the Dev mode there is no home of yours to keep these changes in."
            .to_string()
    })?;
    gui.own_home = Some(own);
    gui.own_home_live = true;
    Ok(if gui.settings.fresh_world_each_launch {
        "Kept for this session only: \"Start every session from the default home\" is on \
         (Settings > Gameplay), so your next session starts from the default home."
            .to_string()
    } else {
        "Saved your home in your character's save.".to_string()
    })
}

/// Before a save that keeps progress: while the live home is the character's own (it came from
/// their save, or they changed it outside Dev this session), record it as it stands, so a save
/// never pairs a backpack that paid for a machine with a home that does not have it yet. Leaves
/// the own home as it is in the Dev mode, before the world has loaded, and for a character in
/// the default home.
pub(crate) fn refresh_own_home(gui: &mut GuiState) {
    if !gui.own_home_live || authors_data_files(gui.settings.play_mode) {
        return;
    }
    if let Some(now) = capture(gui.ship_structure.as_ref(), gui.home_machines.as_ref()) {
        gui.own_home = Some(now);
    }
}

/// The editor's Save button, its 60 s autosave and the flush on quit (engine::editor
/// `autosave_ship_structure`, lib.rs): keep the edits where the mode keeps them, and outside Dev
/// write the save at once, so the edit and what it cost reach the disk together.
pub(crate) fn keep_edits_now(state: &mut EngineState) -> Result<String, String> {
    let machines_path = crate::machines::home_ron_path(&state.data_dir);
    match keep_editor_edits(&mut state.gui_state, &state.data_dir, &machines_path)? {
        Kept::DataFiles(note) => Ok(note),
        Kept::Save(note) => {
            crate::save_load::save_active_home(
                &state.game_world.world,
                &state.gui_state.placed_items,
                &state.data_store,
                !state.gui_state.settings.fresh_world_each_launch,
                state.gui_state.own_home.as_ref(),
            );
            Ok(note)
        }
    }
}

/// At startup, once the save's progress has been applied (lib.rs): the save's own home becomes
/// the session's, and outside the Dev mode its machines take the household's place in the layout
/// loaded from the data files, so the Home page, the offline catch-up and the menu's live power
/// count the character's own machines. True when the layout changed (the caller respawns the
/// menu's machine entities). The design goes on the plot at world load (`design_in_effect`).
pub(crate) fn adopt_saved_home(gui: &mut GuiState, save: &crate::persistence::WorldSave) -> bool {
    gui.own_home = save.home.clone();
    let Some(own) = in_effect(gui).cloned() else {
        return false;
    };
    let Some(home) = gui.home_machines.as_mut() else {
        return false;
    };
    home.set_household_rows(own.machines);
    true
}

/// The machine layout for world load: the data file's, with the character's own machines in
/// place of the household's when one applies. Also says whether the live home is now their own.
pub(crate) fn machines_for_world(gui: &mut GuiState, path: &Path) -> Option<MachineHome> {
    let mut home = MachineHome::load(path)?;
    let own = in_effect(gui).cloned();
    gui.own_home_live = own.is_some();
    overlay_machines(&mut home, own.as_ref());
    Some(home)
}

/// A save applied onto the running world (a snapshot restore, the character picker's Play,
/// lib.rs `launcher_pending_load`), after `apply_save_to_world`: the home goes back to the one
/// that save holds, the character's own or the default, machines and design both, so a restore
/// that gave a machine's item back also takes the machine away. Rebuilds the home (meshes,
/// colliders, machine entities) and puts the save's machine levels on machines it brought back.
/// The Dev mode's home is the data files', which a save does not hold: there only the save's own
/// home is taken up (to be written back), and the home standing is left as it is.
pub(crate) fn reapply_saved_home(state: &mut EngineState, save: &crate::persistence::WorldSave) {
    state.gui_state.own_home = save.home.clone();
    if authors_data_files(state.gui_state.settings.play_mode) {
        return;
    }
    let path = crate::machines::home_ron_path(&state.data_dir);
    if let Some(home) = machines_for_world(&mut state.gui_state, &path) {
        state.gui_state.home_machines = Some(home);
    }
    let own_design = design_in_effect(&state.gui_state).cloned();
    if let Some(ship) = state.gui_state.ship_structure.as_ref() {
        if let (Some(plot), Some(now)) = (ship.home_plot().map(|p| p.id.clone()), ship.home_design()) {
            let design = own_design
                .filter(|d| d.kind == now.kind)
                .or_else(|| HomeDesign::load(&state.data_dir, &now.kind).ok());
            if let Some(design) = design {
                match ship.ship_file().assemble(design, &plot) {
                    Ok(next) => state.gui_state.ship_structure = Some(next),
                    Err(e) => log::warn!("own home: the restored home does not go on plot {plot} ({e}); the home stays as it stood"),
                }
            }
        }
    }
    crate::engine::home_meshes::rebuild_homestead(state);
    crate::systems::machine_levels::land_held(&mut state.game_world.world);
}

/// Give back what was paid for every machine no longer placed (removed by any path since the
/// last frame), each with a line for the player. Called every frame, before the periodic save
/// and the editor's history tick.
pub(crate) fn refund_removed_machines(state: &mut EngineState) {
    let gone = match state.gui_state.home_machines.as_mut() {
        Some(h) if !h.paid.is_empty() => h.take_unplaced_payments(),
        _ => return,
    };
    for (_id, item) in gone {
        let line = give_back_with_line(state, &item);
        state.gui_state.pending_notices.push(line);
    }
}

/// `give_back` for the live game, with the line that tells the player where the item went.
fn give_back_with_line(state: &mut EngineState, item: &str) -> String {
    let name = item_name(state.data_store.get::<crate::systems::inventory::ItemRegistry>("item_registry"), item);
    match give_back(&mut state.game_world.world, &state.data_store, item) {
        GaveBack::Backpack => format!("Your {name} is back in your backpack."),
        GaveBack::Storage => format!("Your {name} went back to your home's storage: the backpack had no room."),
    }
}

/// Where a payment went back to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum GaveBack {
    Backpack,
    Storage,
}

fn item_name(items: Option<&crate::systems::inventory::ItemRegistry>, item: &str) -> String {
    items.and_then(|r| r.items.get(item)).map(|d| d.name.clone()).unwrap_or_else(|| item.to_string())
}

/// Give one `item` back to the player: into the backpack when it fits (by volume, as anything
/// carried), else into the home's storage, through the channel a machine's output takes
/// (`home_stock_outputs`, filed into the Barn by engine/stock_piles.rs `receive_machine_outputs`
/// the same frame). Pure on the world and the data store.
pub(crate) fn give_back(
    world: &mut hecs::World,
    data: &crate::hot_reload::data_store::DataStore,
    item: &str,
) -> GaveBack {
    use crate::ecs::components::Controllable;
    use crate::systems::inventory::Inventory;
    let items = data.get::<crate::systems::inventory::ItemRegistry>("item_registry");
    let max_stack = items.map(|r| r.max_stack_for(item)).unwrap_or(99);
    let unit_vol = items.map(|r| r.volume_for(item)).unwrap_or(0.0);
    let player = world.query::<(&Inventory, &Controllable)>().iter().next().map(|(e, _)| e);
    if let Some(mut pack) = player.and_then(|p| world.get::<&mut Inventory>(p).ok()) {
        if pack.add_item_volume_gated(item, 1, max_stack, unit_vol) == 0 {
            return GaveBack::Backpack;
        }
    }
    if let Some(Ok(mut out)) = data.get::<std::sync::Mutex<Vec<(String, u32)>>>("home_stock_outputs").map(|m| m.lock()) {
        out.push((item.to_string(), 1));
    }
    GaveBack::Storage
}

/// What putting `target` in place of the layout `now` costs and gives back: (the payments that
/// come back with it, to be taken again; the ones it drops, to be given back), each (instance id,
/// item). Pure.
pub(crate) fn payment_changes(now: Option<&MachineHome>, target: Option<&MachineHome>) -> (Vec<(String, String)>, Vec<(String, String)>) {
    let empty = BTreeMap::new();
    let now = now.map_or(&empty, |h| &h.paid);
    let then = target.map_or(&empty, |h| &h.paid);
    let charges = then.iter().filter(|(id, item)| now.get(*id) != Some(*item)).map(|(a, b)| (a.clone(), b.clone())).collect();
    let refunds = now.iter().filter(|(id, item)| then.get(*id) != Some(*item)).map(|(a, b)| (a.clone(), b.clone())).collect();
    (charges, refunds)
}

/// Settle the payments of an undo or a redo that is about to put `target` in place: the paid
/// machines it brings back take their items again (from the backpack, else the home's storage
/// while it is in reach), and the ones it removes give theirs back. Err (and nothing changed)
/// when the player no longer has an item to put a machine back with: the step is refused, with
/// the line saying what is missing.
pub(crate) fn settle_restore(state: &mut EngineState, target: Option<&MachineHome>) -> Result<(), String> {
    let (charges, refunds) = payment_changes(state.gui_state.home_machines.as_ref(), target);
    if !charges.is_empty() {
        let storage = crate::systems::crafting::home_store::HomeStore::here(&state.data_store).reachable();
        let mut need: BTreeMap<&str, u32> = BTreeMap::new();
        for (_, item) in &charges {
            *need.entry(item.as_str()).or_insert(0) += 1;
        }
        for (item, n) in &need {
            let have = crate::engine::editor::count_available(&state.game_world.world, &state.gui_state.placed_items, storage, item);
            if have < *n {
                let items = state.data_store.get::<crate::systems::inventory::ItemRegistry>("item_registry");
                return Err(format!(
                    "Cannot put that back: it takes {n} {} and you have {have}{}.",
                    item_name(items, item),
                    if storage { "" } else { " in your backpack (your home storage is out of reach here)" }
                ));
            }
        }
        for (_, item) in &charges {
            crate::engine::editor::take_one(&mut state.game_world.world, &mut state.gui_state.placed_items, storage, item);
            let name = item_name(state.data_store.get::<crate::systems::inventory::ItemRegistry>("item_registry"), item);
            state.gui_state.pending_notices.push(format!("Putting it back took your {name} again."));
        }
    }
    for (_, item) in refunds {
        let line = give_back_with_line(state, &item);
        state.gui_state.pending_notices.push(line);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machines::MachineInstance;
    use crate::ship::ship_structure::ShipStructure;

    /// A throwaway data dir holding the tree's ship file and machine files and the SHIPPED
    /// homestead design (never the developer's own data/homes/homestead.ron, which an editor
    /// Save rewrites).
    fn data_dir(tag: &str) -> std::path::PathBuf {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let dir = std::env::temp_dir().join(format!("hos_own_home_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for rel in ["blueprints/ship_structure.ron", "machines/home.ron", "machines/ship.ron"] {
            let to = dir.join(rel);
            std::fs::create_dir_all(to.parent().unwrap()).unwrap();
            std::fs::copy(repo.join(rel), &to).unwrap();
        }
        let design = HomeDesign::built_in("homestead").expect("the homestead design is built in");
        design.save(&dir.join("homes").join("homestead.ron")).unwrap();
        dir
    }

    /// The files an editor save may write, as bytes (absent = None).
    fn data_files(dir: &Path) -> Vec<(String, Option<Vec<u8>>)> {
        ["blueprints/ship_structure.ron", "homes/homestead.ron", "machines/home.ron", "machines/ship.ron"]
            .iter()
            .map(|rel| (rel.to_string(), std::fs::read(dir.join(rel)).ok()))
            .collect()
    }

    /// A session on `dir`: the ship assembled onto the default plot, the machines loaded.
    fn session(dir: &Path, mode: PlayMode) -> GuiState {
        let mut gui = GuiState::default();
        gui.settings.play_mode = mode;
        gui.ship_structure = Some(ShipStructure::load_and_assemble(dir, None).expect("the ship assembles"));
        gui.home_machines = MachineHome::load(&dir.join("machines").join("home.ron"));
        gui
    }

    /// What the editor does on a placement paid for (engine::editor `try_place_held_machine`):
    /// a smelter row in the home zone, and its item in the payments. And a wall drawn across the
    /// home (`try_place_wall_node`), a structural edit. Returns the smelter's id.
    fn place_and_build(gui: &mut GuiState, paid: Option<&str>) -> String {
        let home = gui.home_machines.as_mut().expect("machines");
        let id = home.unique_instance_id("smelter");
        home.instances.push(MachineInstance {
            id: id.clone(),
            machine: "smelter".into(),
            room: "home".into(),
            offset: (3.0, 0.0, 3.0),
            rotation: 0.0,
            zone: "home".into(),
            screen_source: None,
        });
        if let Some(item) = paid {
            home.paid.insert(id.clone(), item.to_string());
        }
        let ship = gui.ship_structure.as_mut().unwrap();
        let hz = ship.home_zone_index();
        let body = &mut ship.zones[hz].body;
        let mut wall = body.walls[0].clone();
        wall.a = (1.0, 1.0);
        wall.b = (1.0, 4.0);
        body.walls.push(wall);
        id
    }

    fn walls(ship: &ShipStructure) -> usize {
        ship.zones[ship.home_zone_index()].body.walls.len()
    }

    /// A NORMAL-MODE PLACEMENT GOES TO THE SAVE, NOT THE DATA FILES, AND COMES BACK AFTER A SAVE
    /// AND A LOAD (2026-10-04). The editor kept every mode's edits in the shared data files, so a
    /// machine placed in Normal survived a fresh start for free, came back after a snapshot
    /// restore that gave its item back, and stood in every other character's home on the install.
    /// Now Normal's edits go to the character's own home in their save, the files stay
    /// byte-for-byte as they were, and a later session gets both halves back: the machine (with
    /// what was paid for it) into the layout, the wall into the home assembled on the plot.
    ///
    /// Seen red 2026-10-05 against the editor's save as it was (every mode wrote the files:
    /// `keep_editor_edits` sending Normal to `write_data_files` too): "a Normal-mode edit wrote
    /// homes/homestead.ron".
    #[test]
    fn a_normal_mode_placement_is_kept_in_the_save_not_the_data_files_and_comes_back() {
        let dir = data_dir("normal");
        let machines_path = dir.join("machines").join("home.ron");
        let mut gui = session(&dir, PlayMode::Normal);
        let before = data_files(&dir);
        let walls_before = walls(gui.ship_structure.as_ref().unwrap());
        let id = place_and_build(&mut gui, Some("smelter_0"));

        let kept = keep_editor_edits(&mut gui, &dir, &machines_path).expect("kept");
        for ((rel, was), (_, now)) in before.iter().zip(data_files(&dir)) {
            assert!(*was == now, "a Normal-mode edit wrote {rel}");
        }
        assert!(matches!(kept, Kept::Save(_)), "Normal keeps its edits in the save: {kept:?}");
        let own = gui.own_home.clone().expect("the character's own home holds the edit");
        assert!(own.machines.instances.iter().any(|i| i.id == id));
        assert_eq!(own.machines.paid.get(&id).map(String::as_str), Some("smelter_0"));

        // The save, written and read back.
        let save_path = dir.join("saves").join("offline_home.json");
        let world = hecs::World::new();
        let data = crate::hot_reload::data_store::DataStore::new();
        crate::save_load::save_home_at(&save_path, &world, &[], &data, true, gui.own_home.as_ref());
        let back = crate::persistence::load_world(&save_path).expect("the save reads back");
        assert!(back.home.is_some(), "the save holds the home");

        // The next session: the data files' layout, with the save's home put in.
        let mut next = GuiState::default();
        next.home_machines = MachineHome::load(&machines_path);
        assert!(!next.home_machines.as_ref().unwrap().instances.iter().any(|i| i.id == id), "the data file never had it");
        assert!(adopt_saved_home(&mut next, &back), "the save's home applies in Normal");
        let m = next.home_machines.as_ref().unwrap();
        assert!(m.instances.iter().any(|i| i.id == id && i.machine == "smelter"), "the smelter comes back after a save and a load");
        assert_eq!(m.paid.get(&id).map(String::as_str), Some("smelter_0"), "and what was paid for it");
        assert!(m.instances.iter().any(|i| MachineHome::is_ship_zone(&i.zone)), "the ship's machines are still there");
        // And the wall: the world load puts the save's design on the plot.
        let ship = crate::engine::home_plot::assemble_for_boot(&dir, &next).expect("assembles");
        assert_eq!(walls(&ship), walls_before + 1, "the wall comes back after a save and a load");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A DEV-MODE PLACEMENT STILL GOES TO THE DATA FILES: the operator authors the default home
    /// in-game, so the same edit in Dev is written to data/machines/home.ron and
    /// data/homes/homestead.ron, the character's save gets no home of its own, and nothing paid
    /// is written into a shared file.
    ///
    /// Seen red 2026-10-05 with `keep_editor_edits` sending Dev to the save: "a Dev-mode edit is
    /// written to the data files: Save(\"Saved your home in your character's save.\")" (with the
    /// fresh-home switch still on by default the line read "Kept for this session only: ...").
    #[test]
    fn a_dev_mode_placement_still_goes_to_the_data_files() {
        let dir = data_dir("dev");
        let machines_path = dir.join("machines").join("home.ron");
        let mut gui = session(&dir, PlayMode::Dev);
        let walls_before = walls(gui.ship_structure.as_ref().unwrap());
        let id = place_and_build(&mut gui, Some("smelter_0"));
        let kept = keep_editor_edits(&mut gui, &dir, &machines_path).expect("written");
        assert!(matches!(kept, Kept::DataFiles(_)), "a Dev-mode edit is written to the data files: {kept:?}");
        assert!(gui.own_home.is_none(), "the Dev mode gives the character no home of their own");
        let written = MachineHome::load(&machines_path).expect("home.ron loads");
        assert!(written.instances.iter().any(|i| i.id == id), "the smelter is in data/machines/home.ron");
        assert!(written.paid.is_empty(), "a payment is never written into a shared file");
        let text = std::fs::read_to_string(&machines_path).unwrap();
        assert!(!text.contains("paid"), "home.ron carries no payments");
        let design = HomeDesign::load(&dir, "homestead").expect("the home design loads");
        assert_eq!(design.body.walls.len(), walls_before + 1, "the wall is in data/homes/homestead.ron");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The own home is the save's only outside Dev: in the Dev mode a save's own home is kept as
    /// it is but not applied (Dev builds and sees the default home), and a character with none
    /// keeps the data files' layout.
    #[test]
    fn a_saved_home_applies_outside_dev_only() {
        let dir = data_dir("dev_skips");
        let machines_path = dir.join("machines").join("home.ron");
        let mut normal = session(&dir, PlayMode::Normal);
        let id = place_and_build(&mut normal, None);
        keep_own_home(&mut normal).unwrap();
        let mut save = crate::persistence::WorldSave::new_offline("t", "fibonacci");
        save.home = normal.own_home.clone();
        let mut dev = GuiState::default();
        dev.settings.play_mode = PlayMode::Dev;
        dev.home_machines = MachineHome::load(&machines_path);
        assert!(!adopt_saved_home(&mut dev, &save), "Dev does not apply the save's home");
        assert!(dev.own_home.is_some(), "but keeps it, to write back into the save");
        assert!(!dev.home_machines.as_ref().unwrap().instances.iter().any(|i| i.id == id));
        assert!(design_in_effect(&dev).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Removing a paid machine gives its payment back, by any path (the ledger is pruned against
    /// the placed rows), and an undo or redo settles what it brings back or takes away.
    #[test]
    fn a_removed_machine_gives_its_payment_back_and_a_restore_settles_both_ways() {
        let dir = data_dir("refund");
        let mut gui = session(&dir, PlayMode::Normal);
        let before = gui.home_machines.clone();
        let id = place_and_build(&mut gui, Some("smelter_0"));
        let placed = gui.home_machines.clone();
        // Undo of the placement: the smelter's payment goes back; redo takes it again.
        let (charges, refunds) = payment_changes(placed.as_ref(), before.as_ref());
        assert!(charges.is_empty());
        assert_eq!(refunds, vec![(id.clone(), "smelter_0".to_string())], "undoing a paid placement gives the item back");
        let (charges, refunds) = payment_changes(before.as_ref(), placed.as_ref());
        assert_eq!(charges, vec![(id.clone(), "smelter_0".to_string())], "redoing it takes the item again");
        assert!(refunds.is_empty());
        // Deleted by the Remove button (MachineHome::remove_instance): the payment is taken out
        // once, to be given back, and never again.
        let home = gui.home_machines.as_mut().unwrap();
        home.remove_instance(&id);
        assert_eq!(home.take_unplaced_payments(), vec![(id.clone(), "smelter_0".to_string())]);
        assert!(home.take_unplaced_payments().is_empty(), "a payment is given back once");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Where a payment goes back: the backpack when it fits, else the home's storage channel.
    #[test]
    fn a_payment_goes_back_to_the_backpack_else_to_storage() {
        use crate::ecs::components::Controllable;
        use crate::systems::inventory::Inventory;
        let mut data = crate::hot_reload::data_store::DataStore::new();
        data.insert("home_stock_outputs", std::sync::Mutex::new(Vec::<(String, u32)>::new()));
        let mut world = hecs::World::new();
        let player = world.spawn((Inventory::new(4), Controllable));
        assert_eq!(give_back(&mut world, &data, "smelter_0"), GaveBack::Backpack);
        assert_eq!(world.get::<&Inventory>(player).unwrap().count_item("smelter_0"), 1);
        // A full backpack: storage.
        {
            let mut inv = world.get::<&mut Inventory>(player).unwrap();
            for slot in inv.slots.iter_mut() {
                *slot = Some(crate::systems::inventory::ItemStack::new("rock_0".into(), 99, 99));
            }
        }
        assert_eq!(give_back(&mut world, &data, "smelter_0"), GaveBack::Storage);
        let out = data.get::<std::sync::Mutex<Vec<(String, u32)>>>("home_stock_outputs").unwrap().lock().unwrap().clone();
        assert_eq!(out, vec![("smelter_0".to_string(), 1)]);
    }
}
