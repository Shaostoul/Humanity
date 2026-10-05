//! Placing a built piece in the world (2026-09-27): the ghost, the keys, the
//! hint.
//!
//! Build on the Crafting page used to drop the piece 4 m ahead on the floor,
//! unturned, with no preview, so walls only ran east-west and a roof could
//! never go overhead. Now Build puts the blueprint IN HAND: the page closes,
//! a see-through scaffold (the ghost) follows the crosshair, the Toggle roof
//! key (R by default) turns it a quarter, Interact (E) builds it where the
//! ghost stands and keeps the piece in hand for the next one, and Esc puts
//! it down. Where it lands is `construction::placement::placement_pose`: x
//! and z on the metre grid, y on the floor, or on top of the walls for a
//! roof (`mount: OnTop` in the blueprint data). The ghost's pose is kept on
//! the placing state every frame and E builds exactly that pose, so the
//! piece goes where the ghost stood.
//!
//! WHERE (2026-09-27, the real fix for BUG-102): aboard, in the home frame,
//! on the deck; on a planet's ground, in the build site the player stands
//! in, on the ground under the crosshair (`engine::planet_build::ghost`).
//! Where a piece cannot go (open space, a vehicle, flying, the sea) the hint
//! says so. A piece whose box already stands there is shown but not built
//! again (no double spend).
//!
//! R is the roof toggle when nothing is in hand; while placing it turns the
//! piece (the Controls page says so), because one key holds one action and R
//! is where every building game puts rotate. lib.rs calls `frame` once a
//! frame and `key` from its key handler, ahead of the E chain and the menu
//! Escape.
//!
//! IN A SHARED WORLD (ship homes increment 5, 2026-10-05): where a piece may
//! go, and whether it is the player's own or kept by the server, is
//! `engine::shared_build::gate`, asked by the hint every frame and by E
//! (`press_build`) before anything is spent; offline the gate is the plot rule
//! of increment 1a, the Dev mode exempt. E on a piece the server keeps sends
//! it and draws nothing until the relay says it is kept; F on one asks the
//! relay to take it down (`take_down_entity`).

use crate::engine::planet_build;
use crate::engine::shared_build::{self, Gate};
use crate::engine::state::EngineState;
use crate::gui::{BuildPlacing, GuiPage, GuiState};
use crate::hot_reload::data_store::DataStore;
use crate::input::bindings::{pretty_key_name, GameAction};
use crate::ecs::components::Transform;
use crate::ship::build_frames::{plot_frame_id, BuildFrames};
use crate::ship::wall_collision::WallSegment;
use crate::systems::construction::shared::{self as contract, Action, Reason, SharedPiece};
use crate::systems::construction::{doorway, placement, BlueprintRegistry, BuildRequest, DoorOpen, PlanetSite, Structure};
use glam::Vec3;

/// What a key press does to the piece in hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlaceKey {
    /// Build it where the ghost stands (Interact).
    Build,
    /// Turn it a quarter (the Toggle roof key).
    Turn,
    /// Put it down (Esc).
    Stop,
    /// Take down the finished piece in view (the Swing tool key, F).
    TakeDown,
}

/// The placement meaning of a key press, or None when nothing is in hand, a
/// page is open, or the key is not one of placing's (so it keeps its usual
/// meaning). Pure over the GUI state, so it is tested without a window.
pub(crate) fn key_action(gui: &GuiState, key_name: &str, escape: bool) -> Option<PlaceKey> {
    if gui.build_placing.is_none() || gui.active_page != GuiPage::None {
        return None;
    }
    if escape {
        Some(PlaceKey::Stop)
    } else if gui.keybinds.is(GameAction::ToggleRoof, key_name) {
        Some(PlaceKey::Turn)
    } else if gui.keybinds.is(GameAction::Interact, key_name) {
        Some(PlaceKey::Build)
    } else if gui.keybinds.is(GameAction::AttackSwing, key_name) {
        Some(PlaceKey::TakeDown)
    } else {
        None
    }
}

/// A key press (lib.rs's key handler, presses only). Returns true when
/// placing used the key, so the handler stops there. A held key's repeats
/// are swallowed without acting, so holding E cannot build a stack of walls
/// in one spot.
pub(crate) fn key(state: &mut EngineState, key_name: &str, escape: bool, repeat: bool) -> bool {
    let Some(action) = key_action(&state.gui_state, key_name, escape) else { return false };
    if repeat {
        return true;
    }
    match action {
        PlaceKey::Stop => state.gui_state.build_placing = None,
        PlaceKey::TakeDown => take_down(state),
        PlaceKey::Turn => {
            if let Some(p) = state.gui_state.build_placing.as_mut() {
                p.quarter_turns = (p.quarter_turns + 1) % 4;
            }
        }
        PlaceKey::Build => return press_build(state),
    }
    true
}

/// What E did with the piece in hand ([`press_build_on`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Press {
    /// Nothing in hand, or nowhere to put it (no ghost): the key keeps its usual meaning.
    NoGhost,
    /// Nothing, quietly: the same piece already stands there or is on its way to the server, or
    /// on a planet the pack is short (the hint says which).
    Nothing,
    /// Refused, and the player told: the line shown.
    Refused(String),
    /// Asked for, as the player's own piece (None) or to be kept by the server in this frame.
    Asked(Option<String>),
}

/// E with a piece in hand (the key, and the dev IPC's `place` verb, [`place_at`]): build it where
/// its ghost stands. True when the key was used.
pub(crate) fn press_build(state: &mut EngineState) -> bool {
    let pressed = press_build_on(&mut state.gui_state, &state.game_world.world, &state.data_store, &state.shared_build);
    pressed != Press::NoGhost
}

/// What E does, on the parts it reads (tested without a window). Only where a ghost stands (first
/// person, on foot, aboard or on a planet's ground), and in this order:
/// 1. where it may go (`shared_build::gate`): refused, the line goes up in a notice and nothing is
///    spent (the hint shows it already, so E normally has no ghost there; the `place` verb has);
/// 2. never twice in one spot: a piece or scaffold with its box stands there, or one is on its way
///    to the server (`SharedBuild::waiting_at`, Wave 1B's note on a quick double press);
/// 3. too few materials: said where the player is looking, naming what is missing, instead of
///    sending a build the ConstructionSystem refuses where only the Crafting page shows it
///    (first-hour audit Friction 8, 2026-10-04);
/// 4. the build request, kept by the server in the gate's frame or the player's own. Either way
///    the ConstructionSystem checks the spot and takes the materials; a shared one then waits in
///    `shared::OUT_CHANNEL` for `shared_build::tick` to send.
pub(crate) fn press_build_on(gui: &mut GuiState, world: &hecs::World, data: &DataStore, sb: &shared_build::SharedBuild) -> Press {
    let Some(p) = gui.build_placing.as_ref() else { return Press::NoGhost };
    let Some(pose) = p.ghost.clone() else { return Press::NoGhost };
    let (blueprint_id, name, site, occupied, short) = (p.blueprint_id.clone(), p.name.clone(), p.site.clone(), p.occupied, p.short);
    let registry = data.get::<BlueprintRegistry>("blueprint_registry");
    let frame = match shared_build::gate_here(gui, registry, sb.ranks, registry.and_then(|r| r.get(&blueprint_id)), &pose, site.as_ref()) {
        Gate::Refused(words) => {
            let line = format!("{name} not built: {words}.");
            gui.pending_notices.push(line.clone());
            return Press::Refused(line);
        }
        Gate::Shared(frame) => Some(frame),
        Gate::Private => None,
    };
    if occupied || (frame.is_some() && sb.waiting_at(data, &pose)) {
        return Press::Nothing;
    }
    if let Some(why) = build_refusal(world, data, &blueprint_id, site.is_some()) {
        gui.pending_notices.push(why.clone());
        return Press::Refused(why);
    }
    if short {
        return Press::Nothing;
    }
    let request = BuildRequest::new(blueprint_id, pose).on(site).shared(frame.clone());
    if let Some(chan) = data.get::<std::sync::Mutex<Vec<BuildRequest>>>("build_request") {
        if let Ok(mut c) = chan.lock() {
            c.push(request);
        }
    }
    Press::Asked(frame)
}

/// The dev IPC's `place` verb (engine/ipc.rs, ship homes increment 5): what E does with
/// `blueprint_id` in hand, turned `turns` quarter turns, aimed at the floor point (x, z) in ship
/// metres, through E's own path ([`press_build`]: the gate, one piece per spot, the materials). The
/// pose is the ghost's own (`planet_build::aboard_pose`), on the deck of the plot or shared space
/// the point is over (elsewhere the floor the player stands on), so a rig's point lands exactly
/// where it names whatever floor the camera stands on. Nothing is left in hand after it, so no
/// ghost stands in a picture. Aboard only. Returns a line for the log.
pub(crate) fn place_at(state: &mut EngineState, blueprint_id: &str, x: f32, z: f32, turns: u8) -> String {
    let Some(bp) = state.data_store.get::<BlueprintRegistry>("blueprint_registry").and_then(|r| r.get(blueprint_id)).cloned() else {
        return format!("no blueprint {blueprint_id:?}");
    };
    if !state.aboard_station {
        return "not aboard: the place verb builds on the ship".to_string();
    }
    let turns = turns % 4;
    let deck = state.gui_state.ship_structure.as_ref().and_then(|s| BuildFrames::of_ship(s).frame_at(Vec3::new(x, 0.0, z)).map(|f| f.origin.y));
    let at = Vec3::new(x, deck.unwrap_or_else(|| state.controller.ground_floor()), z);
    let pose = planet_build::aboard_pose(state, &bp, at, turns);
    let occupied = placement::occupied(&state.game_world.world, &pose, None);
    state.gui_state.build_placing = Some(BuildPlacing {
        blueprint_id: bp.id.clone(),
        name: bp.name.clone(),
        quarter_turns: turns,
        ghost: Some(pose),
        site: None,
        occupied,
        short: false,
        hint: String::new(),
    });
    let pressed = press_build_on(&mut state.gui_state, &state.game_world.world, &state.data_store, &state.shared_build);
    state.gui_state.build_placing = None;
    format!("{} at ({x}, {z}), {turns} turn(s): {pressed:?}", bp.name)
}

/// Where the cast waiting to go builds, when it is a building ability
/// (BUG-153, 2026-10-05: the Campfire): the ghost a piece in hand would have
/// right now, put in `abilities::BUILD_SPOT_SLOT` for the AbilitySystem, which
/// builds there through the path a placed piece takes
/// (`construction::begin_build`). `frame` runs this before the main loop
/// moves the cast into the ability channel (the abilities bridge, later in
/// the same frame), so the spot is where the player stood and looked when
/// they cast. Aboard, a spot outside the player's own plot is refused as a
/// piece in hand is (`refused_off_plot`), except for a piece built only
/// outdoors, which the build itself refuses aboard first, with its outdoors
/// reason (`cast_spot`, the BUG-153 review).
fn publish_cast_spot(state: &mut EngineState) {
    use crate::systems::abilities::{AbilityRegistry, BuildSpot, BUILD_SPOT_SLOT};
    let Some((id, _)) = state.gui_state.pending_cast.as_ref() else { return };
    let builds = state
        .data_store
        .get::<AbilityRegistry>("ability_registry")
        .and_then(|r| r.get(id))
        .and_then(|d| d.builds.clone());
    let Some(blueprint) = builds else { return };
    let id = id.clone();
    let placed = planet_build::ghost(state, &blueprint, 0);
    let off_plot = refused_off_plot_here(&state.gui_state, &placed);
    let outdoors_only = state
        .data_store
        .get::<BlueprintRegistry>("blueprint_registry")
        .and_then(|r| r.get(&blueprint))
        .is_some_and(|bp| bp.outdoors_only);
    let guest = state.gui_state.ship_structure.as_ref().is_some_and(|s| s.home_is_away());
    let spot: BuildSpot = (id, cast_spot(&blueprint, placed, off_plot, outdoors_only, guest));
    state.data_store.insert(BUILD_SPOT_SLOT, std::sync::Mutex::new(Some(spot)));
}

/// Where a building ability's cast builds, or why it cannot, from the ghost a
/// piece in hand would have (`publish_cast_spot`): the build request at its
/// pose and in its frame, or the reason. Pure, so it is tested whole.
///
/// THE PLOT RULE NEVER SPEAKS FOR A PIECE BUILT ONLY OUTDOORS (the BUG-153
/// review, 2026-10-05, A2): such a piece is never built aboard, and the
/// build refuses it there with its own outdoors words before anything else
/// (`construction::begin_build`), the words the placing hint shows
/// ([`place_refusal_hint`]). Refused here as "only inside your own plot", a
/// Campfire cast in the Commons sent the player home, where it was refused
/// again as "never aboard"; and a guest, who has no plot, heard nothing true
/// about it at all.
pub(crate) fn cast_spot(
    blueprint: &str,
    placed: Result<planet_build::Ghost, planet_build::CannotBuild>,
    off_plot: bool,
    outdoors_only: bool,
    guest: bool,
) -> Result<BuildRequest, String> {
    match placed {
        Ok(_) if off_plot && !outdoors_only => Err(off_plot_reason(guest).to_string()),
        Ok(g) => Ok(BuildRequest::new(blueprint, g.pose).on(g.site)),
        Err(why) => Err(planet_build::cannot_build_reason(why).to_string()),
    }
}

/// The line under the crosshair when the piece in hand cannot go where its
/// ghost stands, before anything about materials, or None when it can: the
/// first reason that applies, in this order (the BUG-153 review, 2026-10-05,
/// A2). A piece built only outdoors hears its outdoors refusal first, aboard
/// or anywhere (`construction::outdoors_refusal`): it is the answer that
/// holds wherever the player goes, where "only inside your own plot" sent
/// them home to be refused again as "never aboard", and told a guest nothing
/// true. Then a piece that would roof over a campfire
/// (`construction::roofs_over_outdoors_piece`), then where it may go aboard:
/// `where_refused`, the words of `shared_build::gate` (offline the own-plot
/// rule, [`off_plot_reason`]; in a shared world someone else's plot, the
/// ship's shared spaces without the rank, ship homes increment 5). Pure, so
/// the order is tested whole.
pub(crate) fn place_refusal_hint(
    name: &str,
    not_outdoors: Option<crate::systems::construction::NotOutdoors>,
    roofs_over: Option<&str>,
    where_refused: Option<&str>,
) -> Option<String> {
    if let Some(why) = not_outdoors {
        return Some(format!("Placing {name}: {}   [Esc] done", why.reason()));
    }
    if let Some(fire) = roofs_over {
        return Some(format!("Placing {name}: {}   [Esc] done", crate::systems::construction::roof_over_fire_reason(fire)));
    }
    where_refused.map(|words| format!("Placing {name}: {words}   [Esc] done"))
}

/// Once a frame: pick up what Build was clicked on, drop it when the player
/// can no longer place (the build editor, the showroom, death), and move the
/// ghost and the hint. First, the spot for a building ability's cast
/// (`publish_cast_spot`).
pub(crate) fn frame(state: &mut EngineState) {
    publish_cast_spot(state);
    if let Some(id) = state.gui_state.pending_build.take() {
        let name = state
            .data_store
            .get::<BlueprintRegistry>("blueprint_registry")
            .and_then(|r| r.get(&id))
            .map_or_else(|| id.clone(), |bp| bp.name.clone());
        // Picking up another piece keeps the turn: the next wall of a room
        // usually runs the way the last one did.
        let quarter_turns = state.gui_state.build_placing.as_ref().map_or(0, |p| p.quarter_turns);
        state.gui_state.build_placing = Some(BuildPlacing {
            blueprint_id: id,
            name,
            quarter_turns,
            ghost: None,
            site: None,
            occupied: false,
            short: false,
            hint: String::new(),
        });
        state.gui_state.active_page = GuiPage::None;
    }
    let g = &state.gui_state;
    if g.construction_active || g.showroom_active || g.player_death_cause.is_some() {
        state.gui_state.build_placing = None;
        return;
    }
    let Some(p) = state.gui_state.build_placing.as_ref() else { return };
    // Where the piece would go, in the frame the player is in: the home
    // aboard, the build site they stand in on a planet (planet_build).
    let (name, turns) = (p.name.clone(), p.quarter_turns);
    let placed = planet_build::ghost(state, &p.blueprint_id, turns);
    // A piece built only outdoors (BUG-153: a campfire) is refused aboard,
    // under a roof and where there is no air to burn, and a roof is refused
    // over a campfire, with the reason here, by the rules the build itself
    // applies (`construction::outdoors_refusal`, `roofs_over_outdoors_piece`).
    let registry = state.data_store.get::<BlueprintRegistry>("blueprint_registry");
    let bp = registry.and_then(|r| r.get(&p.blueprint_id));
    let (not_outdoors, roofs_over) = match (&placed, bp) {
        (Ok(g), Some(bp)) => {
            let world = &state.game_world.world;
            (
                crate::systems::construction::outdoors_refusal(bp, world, &state.data_store, g.site.as_ref(), &g.pose),
                crate::systems::construction::roofs_over_outdoors_piece(bp, world, registry, g.site.as_ref(), &g.pose),
            )
        }
        _ => (None, None),
    };
    // Where it may go aboard (`shared_build::gate`): offline only inside your
    // own plot (increment 1a of docs/design/ship-homes-and-logistics.md), the
    // Dev mode anywhere; in a shared world (increment 5) the ship is the
    // server's, nobody is exempt, and a shell piece on your own plot is kept
    // by the server, which the hint says.
    let gate = match &placed {
        Ok(g) => shared_build::gate_here(&state.gui_state, registry, state.shared_build.ranks, bp, &g.pose, g.site.as_ref()),
        Err(_) => Gate::Private,
    };
    let where_refused = match &gate {
        Gate::Refused(words) => Some(words.as_str()),
        _ => None,
    };
    let refusal = match &placed {
        Ok(_) => place_refusal_hint(&name, not_outdoors, roofs_over.as_deref(), where_refused),
        Err(_) => None,
    };
    let kept = match (&gate, &placed) {
        (Gate::Shared(_), _) => Some(contract::KEPT_BY_THE_SERVER),
        (Gate::Private, Ok(g)) if state.gui_state.copresence_active && g.site.is_none() => Some(contract::ONLY_IN_YOUR_HOME),
        _ => None,
    };
    // On a planet only the pack counts (the home's storage is in orbit).
    let short = match &placed {
        Ok(g) if g.site.is_some() => carried_short(state, &p.blueprint_id),
        _ => None,
    };
    let keys = &state.gui_state.keybinds;
    let hint = match (&placed, &short) {
        _ if refusal.is_some() => refusal.clone().unwrap_or_default(),
        (Ok(_), Some((item, more))) => short_hint(&name, item, *more),
        (Ok(g), None) => placing_hint(
            &name,
            turns,
            g.above_floor,
            g.occupied,
            &pretty_key_name(keys.pair(GameAction::Interact).0),
            &pretty_key_name(keys.pair(GameAction::ToggleRoof).0),
            &pretty_key_name(keys.pair(GameAction::AttackSwing).0),
            kept,
        ),
        (Err(why), _) => planet_build::cannot_build_hint(&name, *why),
    };
    if let Some(p) = state.gui_state.build_placing.as_mut() {
        p.hint = hint;
        p.short = short.is_some();
        match placed {
            Ok(g) if refusal.is_none() => {
                p.ghost = Some(g.pose);
                p.site = g.site;
                p.occupied = g.occupied;
            }
            _ => {
                p.ghost = None;
                p.site = None;
                p.occupied = false;
            }
        }
    }
}

/// Whether the ghost in hand is refused because it would stand outside the builder's own plot:
/// aboard (a ghost with no planet site), without the ship's structure to edit (`ship_scope`
/// false), and `outside_own_plot`. A planet build, a ghost that could not be placed at all, and
/// `ship_scope` (the Dev mode, offline) are never refused here.
pub(crate) fn refused_off_plot(
    ship: Option<&crate::ship::ship_structure::ShipStructure>,
    ship_scope: bool,
    placed: &Result<planet_build::Ghost, planet_build::CannotBuild>,
) -> bool {
    !ship_scope && matches!(placed, Ok(g) if g.site.is_none() && outside_own_plot(ship, &g.pose))
}

/// `refused_off_plot` for this session as it stands: the ship it is on, and whether the ship's
/// structure is its to edit (`config::ship_editing_for`: the Dev mode, and only out of a shared
/// world, ship homes increment 5). The one decision `frame` and a building ability's cast
/// (`publish_cast_spot`) use, kept pure so it is tested whole.
pub(crate) fn refused_off_plot_here(gui: &GuiState, placed: &Result<planet_build::Ghost, planet_build::CannotBuild>) -> bool {
    refused_off_plot(gui.ship_structure.as_ref(), crate::config::ship_editing_for(gui), placed)
}

/// True when a piece built aboard with `pose` (ship metres) would reach outside the builder's own
/// plot: any part of its turned footprint (`placement::world_aabb`, x and z) lies past the plot
/// box, give or take the slack that lets a wall lie ON the plot line the way the home's own shell
/// does (`build_frames::EDGE_SLACK_M`, half the thickest wall). Height is not bounded, the same
/// 2D rule collision uses. Testing the centre alone let a 4 x 4 m foundation centred 2 m inside
/// the edge cover the home's door and a metre of the shared corridor (the critic's review of 1a).
/// The rule is the relay's own since increment 5 (`BuildFrame::footprint_inside`, the plot's frame
/// measured from its corner), so the game and the server bound a plot alike. False without an
/// assembled ship, where there is no plot to bound against (the legacy layout). TRUE everywhere
/// aboard while the home is put away (a guest, ship homes increment 2): a guest has no plot of
/// this ship to build on. The Dev-mode exemption is `refused_off_plot`'s.
pub(crate) fn outside_own_plot(ship: Option<&crate::ship::ship_structure::ShipStructure>, pose: &Transform) -> bool {
    let Some(s) = ship else { return false };
    let Some(plot) = s.home_plot() else { return s.home_is_away() };
    let frames = BuildFrames::of_ship(s);
    let Some(frame) = frames.get(&plot_frame_id(&plot.id)) else { return true };
    !frame.footprint_inside(&frame.to_local(pose))
}

/// The line under the crosshair when the piece in hand points outside your plot, offline. `guest`:
/// the player has no plot of this ship (ship homes increment 2, the home is put away). Since
/// increment 5 the game shows it through `place_refusal_hint` with the gate's words, so only the
/// tests spell it out whole.
#[cfg(test)]
pub(crate) fn off_plot_hint(name: &str, guest: bool) -> String {
    format!("Placing {name}: {}   [Esc] done", off_plot_reason(guest))
}

/// Why a piece is not built aboard outside your plot, in the words after "Placing X: " and after
/// a building ability's name (`cast_spot`). `guest`: the player has no plot of this ship.
pub(crate) fn off_plot_reason(guest: bool) -> &'static str {
    if guest {
        "you are a guest on this ship, with no plot of your own to build on"
    } else {
        "you can build only inside your own plot (your home)"
    }
}

/// What the player's pack is short of for `blueprint_id`: the item's name
/// and how many more, or None when they carry enough
/// (`construction::materials_short` with no home storage, the rule the
/// ConstructionSystem applies to a planet build).
fn carried_short(state: &EngineState, blueprint_id: &str) -> Option<(String, u32)> {
    use crate::ecs::components::Controllable;
    use crate::systems::inventory::{Inventory, ItemRegistry};
    let bp = state.data_store.get::<BlueprintRegistry>("blueprint_registry")?.get(blueprint_id)?;
    let world = &state.game_world.world;
    let mut q = world.query::<(&Inventory, &Controllable)>();
    let (_e, (inv, _)) = q.iter().next()?;
    let (id, more) = crate::systems::construction::materials_short(bp, |id| inv.count_item(id), None)?;
    let name = state
        .data_store
        .get::<ItemRegistry>("item_registry")
        .and_then(|r| r.items.get(&id).map(|d| d.name.clone()))
        .unwrap_or(id);
    Some((name, more))
}

/// Why Interact will not build `blueprint_id` where the player stands, as the
/// line shown at the crosshair (`set_placing_note`): every material it is short
/// of, by the items' names, or None when there is enough. Counted the way the
/// ConstructionSystem counts before it builds: the pack, and aboard the home's
/// storage too while it is in reach (`HomeStore::here`); on a planet only the
/// pack. Pure over the world and the DataStore, so it is tested without a
/// window. (First-hour audit 2026-10-04, Friction 8.)
pub(crate) fn build_refusal(
    world: &hecs::World,
    data: &crate::hot_reload::data_store::DataStore,
    blueprint_id: &str,
    on_planet: bool,
) -> Option<String> {
    use crate::ecs::components::Controllable;
    use crate::systems::inventory::{Inventory, ItemRegistry};
    let bp = data.get::<BlueprintRegistry>("blueprint_registry")?.get(blueprint_id)?;
    let mut q = world.query::<(&Inventory, &Controllable)>();
    let (_e, (pack, _)) = q.iter().next()?;
    // Aboard, the home's storage counts while it is in reach; not for a guest,
    // whose home is put away, and never on a planet (its storage is in orbit).
    let away = if on_planet { None } else { crate::systems::crafting::home_store::HomeStore::here(data).not_here };
    let storage_counts = !on_planet && away.is_none();
    let stock = data.get::<std::sync::Mutex<std::collections::HashMap<String, u32>>>("home_stock");
    let stored = |id: &str| stock.and_then(|m| m.lock().ok().map(|s| s.get(id).copied().unwrap_or(0))).unwrap_or(0);
    let stores: Option<&dyn Fn(&str) -> u32> = if storage_counts { Some(&stored) } else { None };
    let missing = crate::systems::construction::materials_missing(bp, |id| pack.count_item(id), stores);
    if missing.is_empty() {
        return None;
    }
    // Worded as the build's own refusal words it (`construction::missing_list`).
    let list = crate::systems::construction::missing_list(&missing, data.get::<ItemRegistry>("item_registry"));
    Some(if on_planet {
        format!("Not enough to build the {} here: carry {list} (on a planet you build from what you carry)", bp.name)
    } else if let Some(why) = away {
        format!("Not enough to build the {}: carry {list} ({why})", bp.name)
    } else {
        format!("Not enough to build the {}: {list}, counting your backpack and home storage", bp.name)
    })
}

/// The line under the crosshair on a planet when the pack holds too little:
/// what is missing, and why the home's storage does not count.
pub(crate) fn short_hint(name: &str, item: &str, more: u32) -> String {
    format!("Placing {name}: carry {more} more {item} to build it here (on a planet you build from what you carry)   [Esc] done")
}

/// The line under the crosshair while placing. `above_floor` is how high the
/// ghost rests above the floor it is aimed at (a roof on walls); `occupied`
/// says the same piece already stands there, so E will not build it again.
/// `take_down_key` takes down the finished piece in view (`take_down`).
/// `kept`: in a shared world, who keeps what E builds, after the build key
/// (ship homes increment 5): "[E] build here (kept by the server: anyone near
/// sees it)" or "[E] build here (only in your own home)"; None offline.
#[allow(clippy::too_many_arguments)]
pub(crate) fn placing_hint(
    name: &str,
    quarter_turns: u8,
    above_floor: f32,
    occupied: bool,
    build_key: &str,
    turn_key: &str,
    take_down_key: &str,
    kept: Option<&str>,
) -> String {
    let turned = match quarter_turns % 4 {
        0 => String::new(),
        q => format!(", turned {} degrees", u32::from(q) * 90),
    };
    let on_top = if above_floor > 0.01 { format!(", on top at {above_floor:.1} m") } else { String::new() };
    if occupied {
        return format!(
            "Placing {name}{turned}{on_top}: already built here   [{turn_key}] turn   [{take_down_key}] take down   [Esc] done"
        );
    }
    let kept = kept.map(|k| format!(" ({k})")).unwrap_or_default();
    format!("Placing {name}{turned}{on_top}   [{build_key}] build here{kept}   [{turn_key}] turn   [{take_down_key}] take down   [Esc] done")
}

/// How far away a piece can be taken down from, metres: the reach a piece is
/// placed within (the ghost stands 1.5 to 8 m ahead).
const TAKE_DOWN_REACH_M: f32 = 8.0;

/// What taking down the piece in view would do, or why it will not: the
/// piece, its name and the materials that go back. A store that still holds
/// anything is refused, so taking down a chest can never lose what is in it.
/// Pure over the world and the placed items, so it is tested without a window.
pub(crate) fn take_down_plan(
    world: &hecs::World,
    placed: &[crate::systems::inventory::placed::PlacedItem],
    registry: Option<&BlueprintRegistry>,
    eye: Vec3,
    forward: Vec3,
    frame: Option<&PlanetSite>,
    walls: &[WallSegment],
) -> Result<(hecs::Entity, String, Vec<(String, u32)>), String> {
    let Some(e) = crate::systems::construction::uses::first_in_view(world, registry, eye, forward, TAKE_DOWN_REACH_M, frame, walls) else {
        return Err("Nothing built in reach to take down".to_string());
    };
    let (id, uid) = match world.get::<&Structure>(e) {
        Ok(s) => (s.blueprint_id.clone(), s.uid),
        Err(_) => return Err("Nothing built in reach to take down".to_string()),
    };
    let bp = registry.and_then(|r| r.get(&id));
    let name = bp.map_or_else(|| id.clone(), |b| b.name.clone());
    let path = crate::systems::construction::uses::storage_path(uid);
    if uid != 0 && placed.iter().any(|it| it.container == path && it.qty > 0) {
        return Err(format!("Empty the {name} before taking it down"));
    }
    // A fire gives back its stones and only the logs it has not burned
    // (BUG-153), so burning logs and taking the ring down cannot make logs.
    let fuel = world.get::<&crate::systems::construction::fires::FireFuel>(e).ok().map(|f| *f);
    let back = bp.map(|b| crate::systems::construction::fires::materials_back(b, fuel.as_ref())).unwrap_or_default();
    Ok((e, name, back))
}

/// Take down the finished piece the player looks at (the Swing tool key with a
/// piece in hand, 2026-09-28). Its materials go back into the pack through the
/// same channel "Take to backpack" uses, so what does not fit goes back to
/// storage and the player is told; the piece is gone. Built pieces are solid
/// (`built_piece_segments`), so without this four walls could shut a player in.
/// Every material comes back: a game choice, since nothing models what
/// dismantling breaks. A fire's burnt logs do not (BUG-153,
/// `fires::materials_back`): only its stones and the logs still whole.
fn take_down(state: &mut EngineState) {
    let Some(f) = planet_build::player_frame(state) else {
        set_placing_note(state, "Nothing built in reach to take down".to_string());
        return;
    };
    let plan = take_down_plan(
        &state.game_world.world,
        &state.gui_state.placed_items,
        state.data_store.get::<BlueprintRegistry>("blueprint_registry"),
        f.eye,
        f.forward,
        f.site.as_ref(),
        // Aboard, the home's own walls hide what is behind them.
        if f.site.is_none() { state.wall_colliders.as_slice() } else { &[] },
    );
    match plan {
        Err(why) => set_placing_note(state, why),
        Ok((e, name, materials)) => take_down_entity(state, e, &name, &materials),
    }
}

/// Take down the piece `e` (F, and the dev IPC's `take_down` verb): the player's own at once
/// (`apply_take_down`); one the server keeps (ship homes increment 5) by asking the relay, when
/// `shared_build::take_down_gate` says it would let this player, else the reason why not. A piece
/// the server keeps stays standing until the relay says it came down; then its materials come
/// back (`shared_build`, `game_unbuilt`).
pub(crate) fn take_down_entity(state: &mut EngineState, e: hecs::Entity, name: &str, materials: &[(String, u32)]) {
    let line = take_down_entity_on(&mut state.shared_build, &mut state.game_world.world, &state.data_store, &state.gui_state, e, name, materials);
    if let Some(line) = line {
        state.gui_state.pending_notices.push(line);
    }
}

/// [`take_down_entity`] on the parts it touches (tested without a window): the line to show, or
/// None while a take-down waits for the relay.
pub(crate) fn take_down_entity_on(
    sb: &mut shared_build::SharedBuild,
    world: &mut hecs::World,
    data: &DataStore,
    gui: &GuiState,
    e: hecs::Entity,
    name: &str,
    materials: &[(String, u32)],
) -> Option<String> {
    let kept = world.get::<&SharedPiece>(e).ok().map(|k| (*k).clone());
    let Some(kept) = kept else { return Some(apply_take_down(world, data, e, name, materials)) };
    match shared_build::take_down_gate(&kept, gui.ship_structure.as_ref(), sb.ranks) {
        Err(why) => Some(contract::refusal_message(Action::Unbuild, name, Reason::NotAllowed, Some(why))),
        Ok(()) => {
            sb.ask_take_down(shared_build::Unbuild { piece_id: kept.piece_id, name: name.to_string(), materials: materials.to_vec() });
            None
        }
    }
}

/// The dev IPC's `take_down` verb (engine/ipc.rs, ship homes increment 5): what F does at the
/// piece the server keeps as `piece_id`, wherever the player looks. Returns a line for the log.
pub(crate) fn take_down_piece(state: &mut EngineState, piece_id: u64) -> String {
    let Some(e) = shared_build::entity_of(&state.game_world.world, piece_id) else {
        return format!("no piece {piece_id} the server keeps in this world");
    };
    let world = &state.game_world.world;
    let id = match (world.get::<&Structure>(e), world.get::<&crate::systems::construction::Construction>(e)) {
        (Ok(s), _) => s.blueprint_id.clone(),
        (_, Ok(c)) => c.blueprint_id.clone(),
        _ => String::new(),
    };
    let bp = state.data_store.get::<BlueprintRegistry>("blueprint_registry").and_then(|r| r.get(&id));
    let name = bp.map_or_else(|| id.clone(), |b| b.name.clone());
    let materials = bp.map(|b| crate::systems::construction::fires::materials_back(b, None)).unwrap_or_default();
    take_down_entity(state, e, &name, &materials);
    format!("piece {piece_id} ({name}) asked to come down")
}

/// What a take-down does, once `take_down_plan` has chosen the piece: its
/// materials go back through the "Take to backpack" channel (so what does
/// not fit returns to storage, and the player is told), the piece is gone,
/// and the returned line names what came back by the items' names. Split out
/// of `take_down` so it is tested without a window (the review of
/// 2026-09-28 found it untested; the line also printed item ids).
pub(crate) fn apply_take_down(
    world: &mut hecs::World,
    data: &crate::hot_reload::data_store::DataStore,
    e: hecs::Entity,
    name: &str,
    materials: &[(String, u32)],
) -> String {
    use crate::systems::inventory::{ItemRegistry, TransferOp};
    if let Some(chan) = data.get::<std::sync::Mutex<Vec<TransferOp>>>("inventory_transfer_ops") {
        if let Ok(mut c) = chan.lock() {
            for (id, qty) in materials {
                c.push(TransferOp { item_id: id.clone(), qty: *qty, add: true, ..Default::default() });
            }
        }
    }
    let _ = world.despawn(e);
    let items = data.get::<ItemRegistry>("item_registry");
    let got: Vec<String> = materials
        .iter()
        .map(|(id, q)| {
            let item = items.and_then(|r| r.items.get(id).map(|d| d.name.clone())).unwrap_or_else(|| id.clone());
            format!("{q} {item}")
        })
        .collect();
    if got.is_empty() {
        format!("Took down the {name}")
    } else {
        format!("Took down the {name}: {} back", got.join(", "))
    }
}

/// A refusal shown in the placing hint's place for a moment: through the
/// notice toasts, which is where a player already looks for "why not".
fn set_placing_note(state: &mut EngineState, msg: String) {
    state.gui_state.pending_notices.push(msg);
}

/// Built pieces are solid (2026-09-28, arc C Tier B: "built pieces have no
/// collision, you walk through them"). Every finished piece in `frame` (the
/// home aboard, `None`; a planet build site on the ground) whose box reaches from above the knee to the eye becomes a blocking
/// segment for `ship::wall_collision::resolve`, beside the home's own walls:
/// a wall, a bed, a chest or a machine stops you like a wall of the home.
/// Roofs overhead and floors underfoot sit outside that band, and scaffolds are
/// not `Structure`s yet, so a piece going up can still be walked through.
///
/// `eye` is in that frame's metres. On a planet the walk calls this through
/// `planet_build::collide_on_site`, because there the player moves by the frame
/// lock's anchor rather than the camera. A wall with a door in it blocks as
/// its parts (`doorway::parts`): the wall either side, and the door while it
/// is shut; the lintel is over the head.
pub(crate) fn built_piece_segments(
    world: &hecs::World,
    registry: Option<&BlueprintRegistry>,
    frame: Option<&PlanetSite>,
    eye: Vec3,
    eye_height: f32,
) -> Vec<WallSegment> {
    let feet_y = eye.y - eye_height;
    let mut out = Vec::new();
    for (_e, (s, tf, site, open)) in world.query::<(&Structure, &Transform, Option<&PlanetSite>, Option<&DoorOpen>)>().iter() {
        if !crate::systems::construction::site::in_frame(site, frame) {
            continue;
        }
        let parts = registry.and_then(|r| r.get(&s.blueprint_id)).and_then(|bp| doorway::piece_parts(bp, tf, open.is_some()));
        let boxes: Vec<(Vec3, Vec3)> = match parts {
            Some(parts) => parts.iter().map(|(p, _)| placement::world_aabb(p)).collect(),
            None => vec![placement::world_aabb(tf)],
        };
        for (lo, hi) in boxes {
            if lo.y < eye.y && hi.y > feet_y + STEP_OVER_M {
                out.push(segment_of_box(lo, hi));
            }
        }
    }
    out
}

/// A piece lower than this (m above the feet) is stepped over, not walked into:
/// a foundation or a floor tile.
const STEP_OVER_M: f32 = 0.4;

/// The blocking segment for a box on the ground: along its longer side, inset
/// by half its thickness at each end, with that half thickness as the radius,
/// so the rounded ends stay inside the box's footprint. A square box becomes a
/// point with its half width as the radius.
fn segment_of_box(lo: Vec3, hi: Vec3) -> WallSegment {
    let (cx, cz) = ((lo.x + hi.x) * 0.5, (lo.z + hi.z) * 0.5);
    let (half_x, half_z) = ((hi.x - lo.x) * 0.5, (hi.z - lo.z) * 0.5);
    if half_x >= half_z {
        let run = half_x - half_z;
        WallSegment { a: (cx - run, cz), b: (cx + run, cz), half_thickness: half_z }
    } else {
        let run = half_z - half_x;
        WallSegment { a: (cx, cz - run), b: (cx, cz + run), half_thickness: half_x }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Increment 1a: aboard, a piece in hand is buildable only inside your own plot, ALL of it,
    /// not just its centre. On the shipped ship assembled at p1 (0..55 x 0..89), with real
    /// blueprints placed the way the ghost places them: a foundation in the yard and one flush
    /// in the far corner are yours; a 4 x 4 m foundation centred at (54, 40) is refused, because
    /// it spans x 52..56 across the home's door and into the corridor (the critic's case, which
    /// the centre-only check let through); so is a 4 m wall centred on the east edge, running
    /// north-south it fits and running east-west it reaches x 57; the Commons and p2 are
    /// refused. Assembled at p2, the rule follows the plot. Without a ship there is no bound.
    /// Then the whole decision `frame` uses (`refused_off_plot`): the Dev mode builds anywhere,
    /// a planet ghost is never bounded by a plot, and a ghost that could not be placed is not
    /// "off plot". Red checks, run: testing `pose.position` alone again fails the (54, 40)
    /// foundation; dropping `!ship_scope` from `refused_off_plot` fails the Dev line.
    #[test]
    fn a_piece_goes_aboard_only_inside_your_own_plot() {
        use crate::ship::ship_structure::ShipStructure;
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let reg = BlueprintRegistry::from_ron(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/blueprints/basic.ron"))).unwrap();
        let world = hecs::World::new();
        let pose = |id: &str, x: f32, z: f32, turns: u8| {
            placement::placement_pose(reg.get(id).unwrap(), Vec3::new(x, 0.0, z), turns, &world, &reg, None)
        };
        let p1 = ShipStructure::load_and_assemble_shipped(&data, Some("p1")).expect("assembles at p1");
        let out = |s: &ShipStructure, id: &str, x: f32, z: f32, turns: u8| outside_own_plot(Some(s), &pose(id, x, z, turns));
        assert!(!out(&p1, "wood_foundation", 30.0, 20.0, 0), "a foundation in the yard is yours");
        assert!(!out(&p1, "wood_foundation", 53.0, 87.0, 0), "flush in the far corner is still yours");
        assert!(out(&p1, "wood_foundation", 54.0, 40.0, 0), "centred inside, but it covers the door and a metre of the corridor");
        assert!(!out(&p1, "wood_wall", 55.0, 30.0, 1), "a wall on the east edge, running north-south, is on the line: it fits");
        assert!(out(&p1, "wood_wall", 55.0, 40.0, 0), "running east-west from the edge it reaches x 57");
        assert!(out(&p1, "wood_foundation", 80.0, 40.0, 0), "the Commons is not your plot");
        assert!(out(&p1, "wood_foundation", 30.0, 140.0, 0), "p2 is someone else's plot");
        let p2 = ShipStructure::load_and_assemble_shipped(&data, Some("p2")).expect("assembles at p2");
        assert!(!out(&p2, "wood_foundation", 30.0, 140.0, 0), "assembled at p2, p2 is yours");
        assert!(out(&p2, "wood_foundation", 30.0, 20.0, 0), "and p1 is not");
        assert!(!outside_own_plot(None, &pose("wood_foundation", 1000.0, 1000.0, 0)), "no ship, no bound");
        assert!(off_plot_hint("Wall", false).contains("only inside your own plot"));
        // A guest (ship homes increment 2, the home put away) has no plot of this ship: nothing
        // aboard is theirs, not even the yard of the plot the home stood on. Seen red 2026-10-04
        // before `outside_own_plot` knew an away home (no plot read as the legacy layout's "no
        // bound"): "a guest builds in the default plot's yard".
        let away = p1.put_home_away().expect("the home can be put away");
        assert!(out(&away, "wood_foundation", 30.0, 20.0, 0), "a guest builds in the default plot's yard");
        assert!(out(&away, "wood_foundation", 80.0, 40.0, 0), "or in the Commons");
        assert!(off_plot_hint("Wall", true).contains("a guest on this ship"));

        // The whole decision.
        let ghost = |x: f32, z: f32, site: Option<PlanetSite>| -> Result<planet_build::Ghost, planet_build::CannotBuild> {
            Ok(planet_build::Ghost { pose: pose("wood_foundation", x, z, 0), site, above_floor: 0.0, occupied: false })
        };
        assert!(refused_off_plot(Some(&p1), false, &ghost(80.0, 40.0, None)), "Normal mode, the Commons: refused");
        assert!(!refused_off_plot(Some(&p1), false, &ghost(30.0, 20.0, None)), "Normal mode, your yard: built");
        assert!(!refused_off_plot(Some(&p1), true, &ghost(80.0, 40.0, None)), "the Dev mode builds anywhere");
        let site = PlanetSite { body: "earth".into(), origin: glam::DVec3::new(6.371e6, 0.0, 0.0) };
        assert!(!refused_off_plot(Some(&p1), false, &ghost(80.0, 40.0, Some(site))), "a planet site has no plot");
        assert!(!refused_off_plot(Some(&p1), false, &Err(planet_build::CannotBuild::OpenSpace)), "no ghost, nothing to refuse");
    }

    /// The shipped blueprint catalog in a DataStore, as the build reads it.
    fn catalog() -> crate::hot_reload::data_store::DataStore {
        let mut data = crate::hot_reload::data_store::DataStore::new();
        data.insert(
            "blueprint_registry",
            BlueprintRegistry::from_ron(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/blueprints/basic.ron"))).unwrap(),
        );
        data
    }

    /// The ghost of `id` in the Commons of the shipped ship (outside every
    /// plot), as the ghost places a piece aboard.
    fn in_the_commons(reg: &BlueprintRegistry, id: &str) -> Result<planet_build::Ghost, planet_build::CannotBuild> {
        let pose = placement::placement_pose(reg.get(id).unwrap(), Vec3::new(80.0, 0.0, 40.0), 0, &hecs::World::new(), reg, None);
        Ok(planet_build::Ghost { pose, site: None, above_floor: 0.0, occupied: false })
    }

    /// AN OUTDOORS-ONLY PIECE HEARS ITS OUTDOORS REFUSAL FIRST, ABOARD OR ANYWHERE (the BUG-153
    /// review, 2026-10-05, A2). On the shipped ship assembled at p1, a Campfire in hand in the
    /// Commons, outside the player's own plot: the line under the crosshair says it is never built
    /// aboard, not that it goes only in their own plot (which sent them home, to be refused there
    /// as "never aboard"); a guest, whose home is put away, hears the same. A Wood Wall in the
    /// Commons still hears the plot rule, and a guest's wall the guest's words. Seen red
    /// 2026-10-05 with the plot rule checked first, the order `frame` used: "Placing Campfire:
    /// you can build only inside your own plot (your home)   [Esc] done".
    #[test]
    fn an_outdoors_piece_hears_its_outdoors_refusal_before_the_plot_rule() {
        use crate::ship::ship_structure::ShipStructure;
        use crate::systems::construction::{outdoors_refusal, NotOutdoors};
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let data = catalog();
        let reg = data.get::<BlueprintRegistry>("blueprint_registry").unwrap();
        let p1 = ShipStructure::load_and_assemble_shipped(&dir, Some("p1")).expect("assembles at p1");
        let off_plot = refused_off_plot(Some(&p1), false, &in_the_commons(reg, "campfire"));
        assert!(off_plot, "the Commons is outside the player's own plot");
        let pose = in_the_commons(reg, "campfire").map(|g| g.pose).unwrap();
        let not_outdoors = outdoors_refusal(reg.get("campfire").unwrap(), &hecs::World::new(), &data, None, &pose);
        assert_eq!(not_outdoors, Some(NotOutdoors::Aboard));
        let words = NotOutdoors::Aboard.reason();

        // The plot rule's words when it refuses (what `frame` hands over from the gate offline).
        let plot_words = |off: bool, guest: bool| off.then(|| off_plot_reason(guest));
        let hint = place_refusal_hint("Campfire", not_outdoors, None, plot_words(off_plot, false)).expect("refused");
        assert_eq!(hint, format!("Placing Campfire: {words}   [Esc] done"));
        let away = p1.put_home_away().expect("the home can be put away");
        let guest_off_plot = refused_off_plot(Some(&away), false, &in_the_commons(reg, "campfire"));
        let guest = place_refusal_hint("Campfire", not_outdoors, None, plot_words(guest_off_plot, true)).expect("refused");
        assert_eq!(guest, format!("Placing Campfire: {words}   [Esc] done"), "a guest hears the same");
        let wall_off_plot = refused_off_plot(Some(&p1), false, &in_the_commons(reg, "wood_wall"));
        let wall = place_refusal_hint("Wood Wall", None, None, plot_words(wall_off_plot, false));
        assert_eq!(wall, Some(off_plot_hint("Wood Wall", false)), "a wall still hears the plot rule");
        assert_eq!(place_refusal_hint("Wood Wall", None, None, plot_words(true, true)), Some(off_plot_hint("Wood Wall", true)));
        assert_eq!(place_refusal_hint("Wood Wall", None, None, None), None, "in your own plot: nothing to refuse");
    }

    /// A CAMPFIRE CAST OUTSIDE YOUR PLOT IS REFUSED FOR BEING ABOARD, NOT FOR THE PLOT (the
    /// BUG-153 review, 2026-10-05, A2). The Campfire ability cast in the Commons of the shipped
    /// ship: its spot reaches the build (`cast_spot`), which refuses it with the campfire's
    /// outdoors words, the placing hint's, and spends nothing, where the cast used to stop at
    /// "you can build only inside your own plot (your home)". For a guest the same. A piece that
    /// can be built aboard still hears the plot rule from the cast, a guest in a guest's words.
    /// Seen red 2026-10-05 with the plot rule checked first, the order `publish_cast_spot` used:
    /// "the plot rule does not speak for a campfire: \"you can build only inside your own plot
    /// (your home)\"".
    #[test]
    fn a_campfire_cast_outside_your_plot_hears_the_outdoors_words() {
        use crate::ecs::components::Controllable;
        use crate::systems::construction::{begin_build, NotOutdoors};
        use crate::systems::inventory::Inventory;
        let data = catalog();
        let reg = data.get::<BlueprintRegistry>("blueprint_registry").unwrap();
        let words = format!("The Campfire is not built here: {}", NotOutdoors::Aboard.reason());
        for guest in [false, true] {
            let request = cast_spot("campfire", in_the_commons(reg, "campfire"), true, true, guest)
                .expect("the plot rule does not speak for a campfire");
            let mut world = hecs::World::new();
            let mut pack = Inventory::new(16);
            pack.add_item("stone_raw_0", 6, 99);
            pack.add_item("wood_log_0", 3, 99);
            let player = world.spawn((pack, Controllable));
            let refused = begin_build(&mut world, &data, request).expect_err("never aboard");
            assert_eq!(refused, words, "guest: {guest}");
            assert_eq!(world.get::<&Inventory>(player).unwrap().count_item("wood_log_0"), 3, "nothing spent");
        }
        assert_eq!(
            cast_spot("wood_wall", in_the_commons(reg, "wood_wall"), true, false, false).err().as_deref(),
            Some(off_plot_reason(false)),
            "a piece that can be built aboard still hears the plot rule"
        );
        assert_eq!(cast_spot("wood_wall", in_the_commons(reg, "wood_wall"), true, false, true).err().as_deref(), Some(off_plot_reason(true)));
    }

    /// THE DEV MODE IN A SHARED WORLD BUILDS ONLY ON ITS OWN PLOT (ship homes increment 5,
    /// 2026-10-05). Offline the Dev mode builds anywhere aboard, the Commons and other plots
    /// included. Joined to a shared world it has no exemption, because the ship is the server's
    /// (`config::ship_editing_for`): on the shipped ship assembled at p1, the Commons and p2
    /// (someone else's plot) are refused and its own yard is not, as for a Normal player. The
    /// decision is the one `frame` and a building ability's cast use (`refused_off_plot_here`).
    ///
    /// Seen red 2026-10-05 with the Dev exemption kept (`refused_off_plot_here` asking
    /// `play_mode.allows(ShipStructureEditing)` alone, as `frame` did): "in a shared world the
    /// Dev mode was let build in the Commons".
    #[test]
    fn dev_in_a_shared_world_builds_only_on_its_own_plot() {
        use crate::ship::ship_structure::ShipStructure;
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let data = catalog();
        let reg = data.get::<BlueprintRegistry>("blueprint_registry").unwrap();
        let at = |x: f32, z: f32| -> Result<planet_build::Ghost, planet_build::CannotBuild> {
            let bp = reg.get("wood_foundation").unwrap();
            let pose = placement::placement_pose(bp, Vec3::new(x, 0.0, z), 0, &hecs::World::new(), reg, None);
            Ok(planet_build::Ghost { pose, site: None, above_floor: 0.0, occupied: false })
        };
        let (commons, theirs, yard) = (at(80.0, 40.0), at(30.0, 140.0), at(30.0, 20.0));
        let mut gui = GuiState::default();
        gui.settings.play_mode = crate::config::PlayMode::Dev;
        gui.ship_structure = Some(ShipStructure::load_and_assemble_shipped(&dir, Some("p1")).expect("assembles at p1"));
        assert!(!refused_off_plot_here(&gui, &commons), "offline, the Dev mode builds in the Commons");
        assert!(!refused_off_plot_here(&gui, &theirs), "offline, the Dev mode builds on any plot");
        gui.copresence_active = true;
        assert!(refused_off_plot_here(&gui, &commons), "in a shared world the Dev mode was let build in the Commons");
        assert!(refused_off_plot_here(&gui, &theirs), "in a shared world the Dev mode was let build on p2, someone else's plot");
        assert!(!refused_off_plot_here(&gui, &yard), "in a shared world the Dev mode still builds in its own yard");
        gui.settings.play_mode = crate::config::PlayMode::Normal;
        assert!(refused_off_plot_here(&gui, &commons), "a Normal player is refused in the Commons, as before");
        assert!(!refused_off_plot_here(&gui, &yard), "and builds in their own yard");
    }

    fn holding() -> GuiState {
        let mut g = GuiState::default();
        g.active_page = GuiPage::None;
        g.build_placing = Some(BuildPlacing {
            blueprint_id: "wood_wall".into(),
            name: "Wood Wall".into(),
            quarter_turns: 0,
            ghost: None,
            site: None,
            occupied: false,
            short: false,
            hint: String::new(),
        });
        g
    }

    /// With a piece in hand and no page open, E builds, R turns and Esc
    /// stops; a rebound roof key turns instead of R; with nothing in hand or
    /// a page open, no key is placing's. Red check, run: dropping the
    /// `build_placing.is_none()` guard makes R turn nothing-in-hand and the
    /// last assertion fails.
    #[test]
    fn placing_keys_follow_the_binds_and_only_while_placing() {
        let mut g = holding();
        assert_eq!(key_action(&g, "KeyE", false), Some(PlaceKey::Build));
        assert_eq!(key_action(&g, "KeyR", false), Some(PlaceKey::Turn));
        assert_eq!(key_action(&g, "Escape", true), Some(PlaceKey::Stop));
        assert_eq!(key_action(&g, "KeyF", false), Some(PlaceKey::TakeDown), "the Swing tool key takes down");
        assert_eq!(key_action(&g, "KeyW", false), None, "walking still walks");
        g.keybinds.force_bind(GameAction::ToggleRoof, false, "KeyT");
        assert_eq!(key_action(&g, "KeyT", false), Some(PlaceKey::Turn));
        assert_eq!(key_action(&g, "KeyR", false), None);
        g.active_page = GuiPage::Inventory;
        assert_eq!(key_action(&g, "KeyE", false), None, "a page open: keys are the page's");
        let mut empty = holding();
        empty.build_placing = None;
        assert_eq!(key_action(&empty, "KeyR", false), None, "nothing in hand: R toggles the roof");
    }

    /// The hint names the piece, its turn, whether it rests on top, and the
    /// live keys; over a piece already built it says so and offers no build
    /// key; and each place a piece cannot go has its own plain reason. In a
    /// shared world it says who keeps what E builds (ship homes increment 5).
    #[test]
    fn the_hint_says_the_turn_and_the_keys() {
        assert_eq!(
            placing_hint("Wood Wall", 1, 0.0, false, "E", "R", "F", None),
            "Placing Wood Wall, turned 90 degrees   [E] build here   [R] turn   [F] take down   [Esc] done"
        );
        assert_eq!(
            placing_hint("Wood Roof", 4, 3.0, false, "E", "T", "F", None),
            "Placing Wood Roof, on top at 3.0 m   [E] build here   [T] turn   [F] take down   [Esc] done"
        );
        let twice = placing_hint("Wood Wall", 0, 0.0, true, "E", "R", "F", None);
        assert!(twice.contains("already built here") && !twice.contains("[E]"), "{twice}");
        assert_eq!(
            placing_hint("Wood Wall", 0, 0.0, false, "E", "R", "F", Some(contract::KEPT_BY_THE_SERVER)),
            "Placing Wood Wall   [E] build here (kept by the server: anyone near sees it)   [R] turn   [F] take down   [Esc] done"
        );
        let own = placing_hint("Bed", 0, 0.0, false, "E", "R", "F", Some(contract::ONLY_IN_YOUR_HOME));
        assert!(own.contains("[E] build here (only in your own home)"), "{own}");
        use planet_build::{cannot_build_hint, CannotBuild};
        assert!(cannot_build_hint("Bed", CannotBuild::NotFirstPerson).contains("first person"));
        assert!(cannot_build_hint("Bed", CannotBuild::Driving).contains("vehicle"));
        assert!(cannot_build_hint("Bed", CannotBuild::OpenSpace).contains("open space"));
        assert!(cannot_build_hint("Bed", CannotBuild::NotOnGround).contains("stand on the ground"));
        assert!(cannot_build_hint("Bed", CannotBuild::OnWater).contains("water"));
        let short = short_hint("Wood Wall", "Wood Plank", 4);
        assert!(short.contains("carry 4 more Wood Plank") && short.contains("what you carry") && !short.contains("build here"), "{short}");
    }

    /// A BUILD SHORT OF MATERIALS IS REFUSED AT THE CROSSHAIR, BY NAME
    /// (first-hour audit 2026-10-04, Friction 8). Aboard, Interact with too
    /// few planks sent the build, the ConstructionSystem refused it, and the
    /// reason showed only on the Crafting page, with raw item ids ("need 3x
    /// wood_plank_0 to build Wood Wall"): in the world nothing happened. Now
    /// the press is answered where the player looks, counted the way the build
    /// counts: 2 planks carried and 1 in storage are 3 short of a wall's 6
    /// aboard; on a planet only the 2 carried count; a guest's put-away home
    /// does not count; a bed short of two things names both; and with enough
    /// there is nothing to say.
    ///
    /// Seen red 2026-10-04 with `build_refusal` returning None (nothing reached
    /// the crosshair): "a short wall is refused at the crosshair".
    #[test]
    fn a_short_build_is_refused_at_the_crosshair_by_name() {
        use crate::ecs::components::Controllable;
        use crate::systems::crafting::home_store::HOME_STORAGE_HERE;
        use crate::systems::inventory::{Inventory, ItemRegistry};
        use std::sync::Mutex;
        let mut data = crate::hot_reload::data_store::DataStore::new();
        let reg = BlueprintRegistry::from_ron(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/blueprints/basic.ron"))).unwrap();
        data.insert("blueprint_registry", reg);
        data.insert("item_registry", ItemRegistry::from_csv(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/items.csv"))).unwrap());
        let stock: std::collections::HashMap<String, u32> = [("wood_plank_0".to_string(), 1)].into_iter().collect();
        data.insert("home_stock", Mutex::new(stock));
        let mut world = hecs::World::new();
        let mut pack = Inventory::new(16);
        pack.add_item("wood_plank_0", 2, 99);
        let player = world.spawn((pack, Controllable));

        let wall = build_refusal(&world, &data, "wood_wall", false);
        assert_eq!(
            wall.as_deref(),
            Some("Not enough to build the Wood Wall: 3 more Wood Plank, counting your backpack and home storage"),
            "a short wall is refused at the crosshair"
        );
        let planet = build_refusal(&world, &data, "wood_wall", true).expect("short on a planet");
        assert_eq!(planet, "Not enough to build the Wood Wall here: carry 4 more Wood Plank (on a planet you build from what you carry)");
        let bed = build_refusal(&world, &data, "bed", false).expect("a bed is short of two things");
        assert_eq!(bed, "Not enough to build the Bed: 3 more Wood Plank and 4 more Fiber Bundle, counting your backpack and home storage");
        assert!(!bed.contains("_0"), "no item ids: {bed}");

        // A guest: the put-away home's storage does not count, and the line says why.
        data.insert(HOME_STORAGE_HERE, Mutex::new(false));
        let guest = build_refusal(&world, &data, "wood_wall", false).expect("short as a guest");
        assert!(guest.contains("carry 4 more Wood Plank") && guest.contains("not on this ship"), "{guest}");
        data.insert(HOME_STORAGE_HERE, Mutex::new(true));

        // Enough: carried 5 and 1 stored make the wall's 6.
        world.get::<&mut Inventory>(player).unwrap().add_item("wood_plank_0", 3, 99);
        assert_eq!(build_refusal(&world, &data, "wood_wall", false), None, "enough planks: nothing to refuse");
        assert!(build_refusal(&world, &data, "wood_wall", true).is_some(), "but on a planet 5 carried are 1 short");
    }
}

#[cfg(test)]
mod collision_tests {
    use super::*;
    use crate::ship::wall_collision::{resolve, PLAYER_RADIUS};
    use glam::Quat;

    fn place(world: &mut hecs::World, reg: &BlueprintRegistry, id: &str, x: f32, z: f32, turns: u8) {
        let bp = reg.get(id).unwrap();
        let tf = placement::placement_pose(bp, Vec3::new(x, 0.0, z), turns, world, reg, None);
        world.spawn((tf, Structure { blueprint_id: id.into(), health: bp.health, max_health: bp.health, provides: bp.provides.clone(), uid: 0 }));
    }

    /// A BUILT WALL IS SOLID (2026-09-28). Walking east into a wall built
    /// across the way stops the player at the wall; a roof overhead does not
    /// block the way under it; a piece on a planet site is not in the home's
    /// list; and with no pieces the walk goes through. Red check, run:
    /// returning an empty list from `built_piece_segments` lets the walk pass
    /// through the wall and fails the first assertion.
    #[test]
    fn a_built_wall_stops_the_player_and_a_roof_overhead_does_not() {
        let reg = BlueprintRegistry::from_ron(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/blueprints/basic.ron"))).unwrap();
        let eye_h = 1.7;
        let start = Vec3::new(-2.0, eye_h, 0.0);
        let goal = Vec3::new(2.0, eye_h, 0.0);
        let walk = |world: &hecs::World| {
            let segs = built_piece_segments(world, None, None, start, eye_h);
            resolve(start, goal, PLAYER_RADIUS, &[], &segs)
        };
        // A wall running north-south at x = 0 (one quarter turn).
        let mut world = hecs::World::new();
        place(&mut world, &reg, "wood_wall", 0.0, 0.0, 1);
        let end = walk(&world);
        assert!(end.x < -PLAYER_RADIUS + 0.05, "stopped west of the wall, at {end}");

        // A roof alone, overhead: the way under it is open.
        let mut roofed = hecs::World::new();
        let roof = reg.get("roof").unwrap();
        roofed.spawn((
            Transform { position: Vec3::new(0.0, 3.0, 0.0), rotation: Quat::IDENTITY, scale: Vec3::from_array(roof.size) },
            Structure { blueprint_id: "roof".into(), health: 1.0, max_health: 1.0, provides: roof.provides.clone(), uid: 0 },
        ));
        assert!(built_piece_segments(&roofed, None, None, start, eye_h).is_empty(), "a roof overhead is not in the way");
        assert!((walk(&roofed).x - goal.x).abs() < 1e-4);

        // The same wall on a planet site is not a home piece.
        let mut site = hecs::World::new();
        let bp = reg.get("wood_wall").unwrap();
        let tf = placement::placement_pose(bp, Vec3::ZERO, 1, &site, &reg, None);
        site.spawn((
            tf,
            Structure { blueprint_id: "wood_wall".into(), health: 1.0, max_health: 1.0, provides: bp.provides.clone(), uid: 0 },
            PlanetSite { body: "earth".into(), origin: glam::DVec3::new(6.371e6, 0.0, 0.0) },
        ));
        assert!(built_piece_segments(&site, None, None, start, eye_h).is_empty(), "planet sites are not the home frame");
        assert!((walk(&hecs::World::new()).x - goal.x).abs() < 1e-4, "nothing built, nothing in the way");
    }

    /// A DOOR IN A WALL (2026-09-28). A shut door blocks the doorway, an open
    /// one lets the player through it, and the wall beside the gap blocks
    /// either way. Red check, run: collecting the whole piece's box instead
    /// of its parts fails the open-door walk.
    #[test]
    fn a_doorway_lets_you_through_only_when_the_door_is_open() {
        let reg = BlueprintRegistry::from_ron(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/blueprints/basic.ron"))).unwrap();
        assert!(reg.get("wood_wall_door").and_then(|b| b.doorway).is_some(), "the shipped doorway wall");
        let eye_h = 1.7;
        let mut world = hecs::World::new();
        // Running north-south at x = 0, the gap at z -0.5 .. 0.5.
        place(&mut world, &reg, "wood_wall_door", 0.0, 0.0, 1);
        let e = world.query::<&Structure>().iter().next().map(|(e, _)| e).unwrap();
        let walk = |world: &hecs::World, z: f32| {
            let (start, goal) = (Vec3::new(-2.0, eye_h, z), Vec3::new(2.0, eye_h, z));
            let segs = built_piece_segments(world, Some(&reg), None, start, eye_h);
            resolve(start, goal, PLAYER_RADIUS, &[], &segs)
        };
        assert!(walk(&world, 0.0).x < -0.25, "a shut door blocks the doorway");
        world.insert_one(e, DoorOpen).unwrap();
        assert!((walk(&world, 0.0).x - 2.0).abs() < 1e-3, "an open door lets you through: {}", walk(&world, 0.0));
        assert!(walk(&world, 1.5).x < -0.25, "the wall beside the gap still blocks");

        // A window is glass: walking at it stops you, like the wall. The wall
        // under the sill would stop you on its own, so the glass is checked
        // as a segment of its own: the two sides, the wall under the sill and
        // the pane (the lintel is over the head). Red check, run: leaving the
        // glass out of collision fails the count.
        let mut glazed = hecs::World::new();
        place(&mut glazed, &reg, "wood_wall_window", 0.0, 0.0, 1);
        assert!(walk(&glazed, 0.0).x < -0.25, "a window stops you");
        let segs = built_piece_segments(&glazed, Some(&reg), None, Vec3::new(-2.0, eye_h, 0.0), eye_h);
        assert_eq!(segs.len(), 4, "the sides, the wall under the sill and the pane");
    }

    /// The segment stays inside the box's footprint: a long thin box runs
    /// along its length, a square one is a point with its half width.
    #[test]
    fn a_box_becomes_a_segment_inside_its_footprint() {
        let s = segment_of_box(Vec3::new(-1.0, 0.0, -0.1), Vec3::new(1.0, 3.0, 0.1));
        assert_eq!((s.a, s.b, s.half_thickness), ((-0.9, 0.0), (0.9, 0.0), 0.1));
        let s = segment_of_box(Vec3::new(2.0, 0.0, 2.0), Vec3::new(2.8, 1.0, 2.8));
        assert!((s.a.0 - 2.4).abs() < 1e-6 && s.a == s.b && (s.half_thickness - 0.4).abs() < 1e-6);
        let s = segment_of_box(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.2, 3.0, 2.0));
        assert!((s.a.1 - 0.1).abs() < 1e-6 && (s.b.1 - 1.9).abs() < 1e-6 && (s.a.0 - 0.1).abs() < 1e-6);
    }
}

#[cfg(test)]
mod take_down_tests {
    use super::*;
    use crate::systems::construction::placement;

    fn place(world: &mut hecs::World, reg: &BlueprintRegistry, id: &str, x: f32, z: f32, turns: u8, uid: u32) -> hecs::Entity {
        let bp = reg.get(id).unwrap();
        let tf = placement::placement_pose(bp, Vec3::new(x, 0.0, z), turns, world, reg, None);
        world.spawn((tf, Structure { blueprint_id: id.into(), health: bp.health, max_health: bp.health, provides: bp.provides.clone(), uid }))
    }

    /// THE TAKE-DOWN ITSELF (2026-09-28): the piece is gone, every material
    /// is queued back to the pack as an add, and the line says what came
    /// back. Red check, run: leaving out the despawn fails the first
    /// assertion.
    #[test]
    fn a_take_down_removes_the_piece_and_queues_its_materials() {
        use crate::systems::inventory::TransferOp;
        let reg = BlueprintRegistry::from_ron(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/blueprints/basic.ron"))).unwrap();
        let mut world = hecs::World::new();
        let wall = place(&mut world, &reg, "wood_wall", 0.0, 0.0, 1, 1);
        let mut data = crate::hot_reload::data_store::DataStore::new();
        data.insert("inventory_transfer_ops", std::sync::Mutex::new(Vec::<TransferOp>::new()));
        let materials = vec![("wood_plank_0".to_string(), 6)];
        let line = apply_take_down(&mut world, &data, wall, "Wood Wall", &materials);
        assert!(!world.contains(wall), "the wall is gone");
        let ops = data.get::<std::sync::Mutex<Vec<TransferOp>>>("inventory_transfer_ops").unwrap().lock().unwrap().clone();
        assert_eq!(ops, vec![TransferOp { item_id: "wood_plank_0".into(), qty: 6, add: true, ..Default::default() }]);
        assert_eq!(line, "Took down the Wood Wall: 6 wood_plank_0 back", "no item registry here: the id stands in for the name");
    }

    /// TAKE DOWN (2026-09-28). Looking at a wall within reach names it and
    /// gives back its materials; looking away, or at a wall past the reach,
    /// finds nothing; a chest that still holds something is refused until it
    /// is empty, and an empty one comes down. Red check, run: dropping the
    /// store check lets the full chest come down and fails the refusal.
    #[test]
    fn take_down_finds_the_piece_in_view_and_keeps_a_full_chest() {
        let reg = BlueprintRegistry::from_ron(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/blueprints/basic.ron"))).unwrap();
        let mut world = hecs::World::new();
        let wall = place(&mut world, &reg, "wood_wall", 0.0, 0.0, 1, 1);
        let eye = Vec3::new(-2.0, 1.5, 0.0);
        let (e, name, materials) = take_down_plan(&world, &[], Some(&reg), eye, Vec3::X, None, &[]).expect("a wall in view");
        assert_eq!((e, name.as_str()), (wall, "Wood Wall"));
        assert_eq!(materials, vec![("wood_plank_0".to_string(), 6)], "every material comes back");
        assert!(take_down_plan(&world, &[], Some(&reg), eye, Vec3::NEG_X, None, &[]).is_err(), "looking away");
        assert!(take_down_plan(&world, &[], Some(&reg), Vec3::new(-20.0, 1.5, 0.0), Vec3::X, None, &[]).is_err(), "out of reach");

        let mut stores = hecs::World::new();
        let chest = place(&mut stores, &reg, "storage_chest", 0.0, 0.0, 0, 7);
        let held = vec![crate::systems::inventory::placed::PlacedItem {
            key: "wood_plank_0".into(),
            name: "Wood Plank".into(),
            qty: 3,
            container: crate::systems::construction::uses::storage_path(7),
            ..Default::default()
        }];
        let at_chest = Vec3::new(0.0, 0.5, -2.0);
        let refused = take_down_plan(&stores, &held, Some(&reg), at_chest, Vec3::Z, None, &[]).unwrap_err();
        assert_eq!(refused, "Empty the Storage Chest before taking it down");
        let (e, _, _) = take_down_plan(&stores, &[], Some(&reg), at_chest, Vec3::Z, None, &[]).expect("an empty chest comes down");
        assert_eq!(e, chest);
    }
}

#[cfg(test)]
mod shared_world_tests {
    //! E and F in a server's shared world (ship homes increment 5, 2026-10-05).
    use super::*;
    use crate::ecs::components::Controllable;
    use crate::ecs::systems::System;
    use crate::engine::shared_build::{on_message, tick_core, welcome, Ctx, SharedBuild};
    use crate::ship::ship_structure::ShipStructure;
    use crate::systems::construction::shared::{FromRelay, Piece};
    use crate::systems::construction::{Construction, ConstructionSystem};
    use crate::systems::inventory::{Inventory, ItemRegistry, TransferOp};
    use std::sync::Mutex;

    /// The relay's clock, Unix seconds.
    const T: f64 = 1_759_500_000.0;

    fn registry() -> BlueprintRegistry {
        BlueprintRegistry::from_ron(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/blueprints/basic.ron"))).unwrap()
    }

    /// The data E, the ConstructionSystem and the shared books read.
    fn store() -> DataStore {
        let mut data = DataStore::new();
        data.insert("blueprint_registry", registry());
        data.insert("item_registry", ItemRegistry::from_csv(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/items.csv"))).unwrap());
        data.insert("build_request", Mutex::new(Vec::<BuildRequest>::new()));
        data.insert("build_status", Mutex::new(String::new()));
        data.insert("quest_events", Mutex::new(Vec::<String>::new()));
        data.insert("inventory_transfer_ops", Mutex::new(Vec::<TransferOp>::new()));
        data.insert(contract::OUT_CHANNEL, contract::OutQueue::default());
        data
    }

    /// The home on p1, in a shared world.
    fn joined_at_p1() -> GuiState {
        let mut gui = GuiState::default();
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        gui.ship_structure = Some(ShipStructure::load_and_assemble_shipped(&data, Some("p1")).expect("assembles at p1"));
        gui.copresence_active = true;
        gui
    }

    fn hear(sb: &mut SharedBuild, world: &mut hecs::World, data: &DataStore, gui: &mut GuiState, m: &FromRelay) {
        on_message(&mut Ctx { sb, world, data, gui }, &serde_json::to_value(m).unwrap());
    }

    fn pieces_in(world: &hecs::World) -> usize {
        world.query::<&Construction>().iter().count() + world.query::<&Structure>().iter().count()
    }

    fn sent(sb: &mut SharedBuild) -> Vec<serde_json::Value> {
        sb.outbox.drain(..).map(|m| serde_json::from_str(&m).unwrap()).collect()
    }

    /// E ON A SHAREABLE PIECE IN THE SHARED WORLD SENDS IT AND PUTS UP NOTHING. The home on p1, a
    /// Wood Wall in hand in p1's yard, 10 planks in the pack: E asks for it kept by the server in
    /// `plot:p1`; the ConstructionSystem takes its 6 planks and puts up nothing of its own; the tick
    /// sends one `game_build`, measured from p1's corner; a second E on the same spot spends nothing
    /// (it is on its way); and only the relay's answer puts the piece up, a scaffold that is ours.
    ///
    /// Seen red 2026-10-05 with E asking for a private build (the request without
    /// `.shared(frame)`, as E did before): "E in the shared world put up a scaffold of its own:
    /// left: 1, right: 0".
    #[test]
    fn e_on_a_shareable_piece_in_the_shared_world_sends_and_spawns_nothing() {
        let data = store();
        let reg = registry();
        let mut world = hecs::World::new();
        let mut pack = Inventory::new(16);
        pack.add_item("wood_plank_0", 10, 99);
        let player = world.spawn((pack, Controllable));
        let mut gui = joined_at_p1();
        let mut sb = SharedBuild::default();
        welcome(&mut Ctx { sb: &mut sb, world: &mut world, data: &data, gui: &mut gui }, &serde_json::json!({ "type": "game_welcome" }));
        hear(&mut sb, &mut world, &data, &mut gui, &FromRelay::Pieces { frame: "plot:p1".into(), seq: 0, server_time: T, part: 1, parts: 1, pieces: Vec::new() });
        let bp = reg.get("wood_wall").unwrap();
        let pose = placement::placement_pose(bp, Vec3::new(30.0, 0.0, 20.0), 0, &world, &reg, None);
        gui.build_placing = Some(BuildPlacing {
            blueprint_id: "wood_wall".into(),
            name: "Wood Wall".into(),
            quarter_turns: 0,
            ghost: Some(pose.clone()),
            site: None,
            occupied: false,
            short: false,
            hint: String::new(),
        });

        assert_eq!(press_build_on(&mut gui, &world, &data, &sb), Press::Asked(Some("plot:p1".into())));
        ConstructionSystem::new().tick(&mut world, 0.016, &data);
        assert_eq!(pieces_in(&world), 0, "E in the shared world put up a scaffold of its own: left: {}, right: 0", pieces_in(&world));
        assert_eq!(world.get::<&Inventory>(player).unwrap().count_item("wood_plank_0"), 4, "the wall's 6 planks, spent once");

        tick_core(&mut Ctx { sb: &mut sb, world: &mut world, data: &data, gui: &mut gui }, 0.3, true, true);
        let out = sent(&mut sb);
        assert_eq!(out.len(), 1, "{out:?}");
        assert_eq!((out[0]["type"].as_str(), out[0]["frame"].as_str(), out[0]["blueprint_id"].as_str()), (Some("game_build"), Some("plot:p1"), Some("wood_wall")));
        assert_eq!(out[0]["position"], serde_json::json!([30.0, 0.0, 20.0]), "measured from p1's corner, the ship's origin");
        let req = out[0]["req_id"].as_u64().unwrap() as u32;

        assert_eq!(press_build_on(&mut gui, &world, &data, &sb), Press::Nothing, "on its way: a second E spends nothing");
        assert!(data.get::<Mutex<Vec<BuildRequest>>>("build_request").unwrap().lock().unwrap().is_empty());

        let kept = Piece { piece_id: 40, blueprint_id: "wood_wall".into(), position: [30.0, 0.0, 20.0], rotation: [0.0, 0.0, 0.0, 1.0], scale: bp.size, placed_at: T, mine: true };
        hear(&mut sb, &mut world, &data, &mut gui, &FromRelay::Built { frame: "plot:p1".into(), seq: 1, server_time: T, piece: kept, req_id: Some(req) });
        let mut q = world.query::<(&Construction, &SharedPiece)>();
        let up: Vec<(u64, bool)> = q.iter().map(|(_e, (_c, k))| (k.piece_id, k.mine)).collect();
        assert_eq!(up, vec![(40, true)], "the relay's answer puts it up, a scaffold, ours");
        drop(q);
        assert_eq!(sb.counts(), (0, 0, 0), "settled");
    }

    /// F ON A PIECE THE SERVER KEEPS SENDS `game_unbuild` AND WAITS. The home on p1: F at the wall
    /// the server keeps on p1 asks the relay and changes nothing (the wall stands, nothing comes
    /// back); the tick sends `game_unbuild` with the piece's id; when the relay says it came down
    /// with our req_id, it goes and its 6 planks come back, and the player is told. F at a wall on
    /// p2 (someone else's plot) is refused in the take-down sentence, sends nothing, and the wall
    /// stands.
    ///
    /// Seen red 2026-10-05 with F taking every piece down at once (`apply_take_down`, as F did
    /// before): it took the wall down and said so, left: `Some("Took down the Wood Wall: 6 Wood
    /// Plank back")`, right: `None`.
    #[test]
    fn f_on_a_shared_piece_sends_game_unbuild_and_waits() {
        let data = store();
        let reg = registry();
        let mut world = hecs::World::new();
        world.spawn((Inventory::new(16), Controllable));
        let mut gui = joined_at_p1();
        let mut sb = SharedBuild::default();
        welcome(&mut Ctx { sb: &mut sb, world: &mut world, data: &data, gui: &mut gui }, &serde_json::json!({ "type": "game_welcome" }));
        let size = reg.get("wood_wall").unwrap().size;
        let wall = |id: u64, x: f32| Piece { piece_id: id, blueprint_id: "wood_wall".into(), position: [x, 0.0, 20.0], rotation: [0.0, 0.0, 0.0, 1.0], scale: size, placed_at: T - 100.0, mine: true };
        let list = |frame: &str, seq: u64, pieces: Vec<Piece>| FromRelay::Pieces { frame: frame.into(), seq, server_time: T, part: 1, parts: 1, pieces };
        hear(&mut sb, &mut world, &data, &mut gui, &list("plot:p1", 1, vec![wall(7, 20.0)]));
        hear(&mut sb, &mut world, &data, &mut gui, &list("plot:p2", 3, vec![wall(9, 20.0)]));
        let seven = shared_build::entity_of(&world, 7).expect("piece 7 stands");
        let nine = shared_build::entity_of(&world, 9).expect("piece 9 stands");
        let materials = vec![("wood_plank_0".to_string(), 6)];
        let ops = |data: &DataStore| data.get::<Mutex<Vec<TransferOp>>>("inventory_transfer_ops").unwrap().lock().unwrap().clone();

        assert_eq!(take_down_entity_on(&mut sb, &mut world, &data, &gui, seven, "Wood Wall", &materials), None);
        assert!(world.contains(seven), "F took the piece down before the server said so");
        assert!(ops(&data).is_empty(), "nothing back yet");
        tick_core(&mut Ctx { sb: &mut sb, world: &mut world, data: &data, gui: &mut gui }, 0.3, true, true);
        let out = sent(&mut sb);
        assert_eq!(out.len(), 1, "{out:?}");
        assert_eq!((out[0]["type"].as_str(), out[0]["piece_id"].as_u64()), (Some("game_unbuild"), Some(7)));
        assert!(out[0].get("permit").is_none(), "no permit on your own plot");
        assert!(world.contains(seven), "still standing until the relay says so");
        let req = out[0]["req_id"].as_u64().map(|r| r as u32);

        hear(&mut sb, &mut world, &data, &mut gui, &FromRelay::Unbuilt { frame: "plot:p1".into(), seq: 2, piece_id: 7, req_id: req });
        assert!(!world.contains(seven), "the relay said it came down");
        assert_eq!(ops(&data), vec![TransferOp { item_id: "wood_plank_0".into(), qty: 6, add: true, ..Default::default() }], "its planks back, once");
        assert_eq!(gui.pending_notices, vec!["Took down the Wood Wall: 6 Wood Plank back".to_string()]);

        let refused = take_down_entity_on(&mut sb, &mut world, &data, &gui, nine, "Wood Wall", &materials);
        assert_eq!(refused.as_deref(), Some("Wood Wall not taken down: this is someone else's plot, and you cannot take down what stands on it."));
        tick_core(&mut Ctx { sb: &mut sb, world: &mut world, data: &data, gui: &mut gui }, 0.3, true, true);
        assert!(sent(&mut sb).is_empty() && world.contains(nine), "nothing sent, and it stands");
    }
}
