//! Construction system: blueprint placement, building progress, snap grid.
//!
//! Blueprints define buildable structures loaded from RON files.
//! Construction progresses over time and consumes inventory materials.

pub mod structural;
pub mod routing;
pub mod solver;
pub mod placement;
pub mod site;
pub mod uses;
pub mod doorway;
pub mod fires;
/// Shared building (ship homes increment 5, 2026-10-05): the pieces a server keeps for everyone
/// in its shared world, the messages about them, and the one set of rules the relay and the game
/// both check a build against. See `shared.rs`.
pub mod shared;

pub use site::PlanetSite;

use crate::ecs::components::Transform;
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use glam::Vec3;
use serde::Deserialize;
use std::collections::HashMap;

/// A buildable structure definition loaded from data files.
#[derive(Debug, Clone, Deserialize)]
pub struct Blueprint {
    pub id: String,
    pub name: String,
    pub category: String,
    pub materials: Vec<(String, u32)>,
    pub build_time: f32,
    pub size: [f32; 3],
    pub snap_to: Vec<String>,
    /// Where the placed piece sits (2026-09-27): on the floor, or on top of
    /// the `snap_to` pieces its footprint covers (a roof on walls). See
    /// `Mount` and `placement::placement_pose`.
    #[serde(default)]
    pub mount: Mount,
    pub health: f32,
    pub provides: Option<String>,
    /// Machine types this structure serves as once BUILT (2026-09-25): a
    /// built furnace counts as a "smelter" for the recipe station gate, a
    /// crafting table as a "workbench". The names are the home machines'
    /// types, which is what a recipe's station_required strips down to.
    /// Before this a built structure did nothing at all (the playable
    /// assessment's 2.4: "the construction sink terminates in a decorative
    /// box"). Empty for structures that are not workstations.
    #[serde(default)]
    pub stations: Vec<String>,
    /// An ELECTRIC station's working draw, in watts (2026-09-26): once built
    /// it joins the home's power like a placed station machine, drawing
    /// `idle_watts` until a craft runs at it (see `wire_built_stations`).
    /// 0 = it needs no power (a workbench, a fire-fed furnace).
    #[serde(default)]
    pub power_watts: f32,
    #[serde(default)]
    pub idle_watts: f32,
    /// Shed priority (1 critical .. 5 optional), as the machines use.
    #[serde(default)]
    pub power_priority: u8,
    /// What a built GENERATOR makes (2026-09-27): a solar panel's peak watts,
    /// or a steady generator's watts, in the home machines' own terms
    /// (`machines::MachinePower::Solar` / `Generator`). Once built it joins
    /// the power of where it stands (see `wire_built_generators`), and what it
    /// makes offsets what that island draws from the ship's reactor. None =
    /// it makes no power.
    #[serde(default)]
    pub generates: Option<crate::machines::MachinePower>,
    /// A door set into the piece (2026-09-28): a gap `width` wide and
    /// `height` tall, centred along the piece's length, with a door leaf that
    /// E opens and closes. None = a solid piece. See `doorway::parts`.
    #[serde(default)]
    pub doorway: Option<Doorway>,
    /// A window set into the piece (2026-09-28): a glazed opening `width`
    /// wide and `height` tall whose bottom sits `sill` above the floor,
    /// centred along the piece's length. None = no window. See
    /// `doorway::piece_parts`.
    #[serde(default)]
    pub window: Option<Window>,
    /// Built only outdoors (BUG-153, 2026-10-05): on a planet's open ground,
    /// never aboard the ship, under a built roof, or where there is no air to
    /// breathe. A fire is. See [`outdoors_refusal`].
    #[serde(default)]
    pub outdoors_only: bool,
    /// A fire (BUG-153): what it burns, how long one fuel item lasts, how many
    /// it holds and the heat it radiates. None = it does not burn. See
    /// [`fires`].
    #[serde(default)]
    pub burns: Option<fires::Burn>,
    /// Kept by the server when built in a shared world (ship homes increment 5, 2026-10-05,
    /// `shared.rs`): the piece goes to the relay, which keeps it and shows it to everyone near,
    /// instead of into the builder's own save. Only a piece whose pose is its whole state may say
    /// so: a foundation, a wall, a window wall, a roof. A door (open or shut), a chest (its
    /// contents), a station or a generator (power), a bed (rest) or a fire (its fuel) carries
    /// state the server does not keep, and the data test in `shared.rs` fails if one is marked.
    /// False, the default: it stays in the builder's own home, as every build did before.
    #[serde(default)]
    pub shared: bool,
}

/// Why a piece built only outdoors cannot go where it was asked
/// ([`outdoors_refusal`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotOutdoors {
    /// In the home frame: aboard the ship, whose rooms and halls are sealed,
    /// or on its hull, where there is no air.
    Aboard,
    /// On ground where the open air cannot be breathed (the Moon, Mars, high
    /// in the death zone), so there is no air for a fire to burn.
    NoAir,
    /// Under a finished roof (`uses::shelter_at`).
    UnderRoof,
}

impl NotOutdoors {
    /// Why, in words that follow "The Campfire is not built here: " and
    /// "Placing Campfire: ".
    pub fn reason(self) -> &'static str {
        match self {
            NotOutdoors::Aboard => "it is built outdoors on a planet's ground, never indoors or aboard the ship",
            NotOutdoors::NoAir => "a fire needs air to burn, and there is no breathable air here",
            NotOutdoors::UnderRoof => "it needs open sky over it, because under a roof its smoke would fill the shelter",
        }
    }
}

/// Why a piece built only outdoors (`Blueprint::outdoors_only`, a fire)
/// cannot stand at `pose` in `site`, or None when it can, or when the piece
/// goes anywhere (BUG-153, 2026-10-05). Outdoors is a planet's open ground
/// with air to burn: not the home frame (`site` None: aboard the ship or on
/// its hull), not where the open air there cannot be breathed (the
/// `"body_environment"` the engine publishes for the body the player stands
/// on, `BodyEnvironment::breathable_outside`; none published reads as no
/// air), and not under a finished roof (`uses::shelter_at` at the piece's
/// base). Applied by `begin_build`, so every way of building agrees, and by
/// the placing hint (`engine::build_place`).
pub fn outdoors_refusal(
    bp: &Blueprint,
    world: &hecs::World,
    data: &DataStore,
    site: Option<&PlanetSite>,
    pose: &Transform,
) -> Option<NotOutdoors> {
    if !bp.outdoors_only {
        return None;
    }
    let Some(site) = site else {
        return Some(NotOutdoors::Aboard);
    };
    let air = data
        .get::<crate::systems::body_environment::BodyEnvironment>("body_environment")
        .is_some_and(|e| e.body_id == site.body && e.breathable_outside());
    if !air {
        return Some(NotOutdoors::NoAir);
    }
    if uses::shelter_at(world, pose.position, Some(site)).roofed {
        return Some(NotOutdoors::UnderRoof);
    }
    None
}

/// What a build is short of, as words: "6 more Raw Stone and 3 more Wood
/// Log", by the items' names where the registry knows them, the ids
/// otherwise. The build's refusal (`begin_build`) and the crosshair's
/// (`engine::build_place::build_refusal`) both say it this way.
pub fn missing_list(missing: &[(String, u32)], items: Option<&crate::systems::inventory::ItemRegistry>) -> String {
    let parts: Vec<String> = missing
        .iter()
        .map(|(id, more)| {
            let name = items.and_then(|r| r.items.get(id)).map_or_else(|| id.clone(), |d| d.name.clone());
            format!("{more} more {name}")
        })
        .collect();
    match parts.as_slice() {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// A glazed opening in a piece (`Blueprint::window`), metres.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Window {
    pub width: f32,
    pub height: f32,
    pub sill: f32,
}

/// The gap a doorway piece leaves for its door, metres (`Blueprint::doorway`).
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Doorway {
    pub width: f32,
    pub height: f32,
}

/// Marks a doorway piece whose door stands open (E toggles it,
/// `engine::built_uses`). Saved with the piece (`persistence::ConstructionSave::open`).
#[derive(Debug, Clone, Copy, Default)]
pub struct DoorOpen;

/// Give every finished structure whose blueprint GENERATES power a live
/// generator (2026-09-27): a `PowerGenerator` (and a `SolarPanel`, which the
/// SolarSystem runs with the sun) on a power island. A generator built in the
/// home joins the home's strongest island, the one the ship's reactor feeds
/// in the Station-supplied mode, so what it makes is drawn from the reactor
/// one watt less for one watt; one built on a planet site gets that site's
/// own island (`ship_power::site_island`), which no reactor reaches, and
/// which the site's electric stations then join (`site_power_island`).
pub fn wire_built_generators(world: &mut hecs::World, registry: &BlueprintRegistry) {
    use crate::ecs::components::{PowerCircuit, PowerGenerator, SolarPanel};
    use crate::machines::MachinePower;
    let todo: Vec<(hecs::Entity, MachinePower, Option<PlanetSite>)> = world
        .query::<hecs::Without<(&Structure, Option<&PlanetSite>), &PowerGenerator>>()
        .iter()
        .filter_map(|(e, (s, site))| registry.get(&s.blueprint_id)?.generates.clone().map(|g| (e, g, site.cloned())))
        .collect();
    if todo.is_empty() {
        return;
    }
    let mut by_island: HashMap<u32, f32> = HashMap::new();
    for (_e, (g, pc)) in world.query::<hecs::Without<(&PowerGenerator, &PowerCircuit), &PlanetSite>>().iter() {
        *by_island.entry(pc.island).or_default() += g.output_watts;
    }
    let home_island = by_island
        .into_iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map_or(0, |(i, _)| i);
    for (e, power, site) in todo {
        let island = site.as_ref().map_or(home_island, crate::systems::ship_power::site_island);
        let _ = world.insert_one(e, PowerCircuit { island });
        match power {
            MachinePower::Solar { peak_watts, .. } => {
                let _ = world.insert(
                    e,
                    (PowerGenerator { output_watts: peak_watts, fuel_per_second: 0.0, active: true }, SolarPanel { peak_watts }),
                );
            }
            MachinePower::Generator { watts, fuel_lph, .. } if fuel_lph <= 0.0 => {
                let _ = world.insert_one(e, PowerGenerator { output_watts: watts, fuel_per_second: 0.0, active: true });
            }
            _ => log::warn!("a blueprint's `generates` must be Solar or a fuel-free Generator"),
        }
    }
}

/// Where a placed piece sits (2026-09-27). Data, not code: a blueprint says
/// `mount: OnTop` and the placement reads it, so any new piece that goes on
/// top of others (a second roof, a shelf on a bench) is a data edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
pub enum Mount {
    /// On the floor where it is placed (the default).
    #[default]
    Floor,
    /// On top of the `snap_to` pieces its footprint covers, at the height of
    /// the tallest of them; on the floor where there are none. A roof over
    /// walls rests at the walls' height, a wall on a foundation on the
    /// foundation.
    OnTop,
}

/// One build the player asked for (2026-09-27): the blueprint, the pose the
/// ghost preview stood at when E was pressed (`placement::placement_pose`,
/// computed by `engine::build_place` every frame and kept on the placing
/// state), and the frame that pose is in. The ConstructionSystem builds
/// exactly that pose; it used to recompute it from the camera at the key
/// press, which is not the ghost the player saw (review of the shelter
/// commit).
#[derive(Debug, Clone)]
pub struct BuildRequest {
    pub blueprint_id: String,
    /// Where the piece goes, in `site`'s frame.
    pub pose: Transform,
    /// The build site on a planet the pose is in, or None for the home frame
    /// (aboard). See `site`.
    pub site: Option<PlanetSite>,
    /// The shared world's frame this piece is to be kept in (ship homes increment 5,
    /// 2026-10-05, `shared.rs`): `"plot:p3"` or `"zone:commons"` (`ship::build_frames`), or
    /// None for a private build in the builder's own home, as every build was before. Set by
    /// the placing code's gate while the player is in a shared world. The pose stays in ship
    /// metres (the home frame's) either way; this only says where the piece will be kept. With
    /// it set, the ConstructionSystem checks the spot and takes the materials as for any build,
    /// then hands the build to the engine as a `shared::SharedBuildIntent` in
    /// `shared::OUT_CHANNEL` instead of putting up a scaffold; the scaffold comes when the relay
    /// says the piece is built.
    pub shared_frame: Option<String>,
}

impl BuildRequest {
    /// A request for a piece at `pose` in the home frame, private.
    pub fn new(blueprint_id: impl Into<String>, pose: Transform) -> Self {
        Self { blueprint_id: blueprint_id.into(), pose, site: None, shared_frame: None }
    }

    /// The same request in a planet build site's frame.
    pub fn on(mut self, site: Option<PlanetSite>) -> Self {
        self.site = site;
        self
    }

    /// The same request, to be kept in the shared world's `frame` (`"plot:p3"`), or private
    /// (None). See `shared_frame`.
    pub fn shared(mut self, frame: Option<String>) -> Self {
        self.shared_frame = frame;
        self
    }
}

/// Give every finished structure whose blueprint draws power the components
/// a placed station machine carries (2026-09-26): its station type and its
/// working/idle loads, and a power consumer at idle draw on a power island.
/// The crafting system then refuses a craft there without power and raises
/// its draw while it works. Runs each tick, so it also covers structures
/// restored from a save.
///
/// WHICH GRID (2026-09-27, the planet-build review). A station built in the
/// home joins the home's strongest island (the one with the most
/// generation), as before. A station built on a planet's ground does NOT:
/// the home is in orbit, and a stove on Earth drawing from its batteries
/// was a leak. It joins the power of its own build site, when something in
/// that site makes power ([`site_power_island`]); until then it carries its
/// station type and its loads but no power consumer, so it reads as a
/// station with no power (`CraftingSystem::station_unpowered_at`), and it is
/// wired the tick its site gains power.
pub fn wire_built_stations(world: &mut hecs::World, registry: &BlueprintRegistry) {
    use crate::ecs::components::{MachineType, PowerCircuit, PowerConsumer, PowerGenerator, StationLoad};
    let electric = |s: &Structure| {
        let bp = registry.get(&s.blueprint_id)?;
        let station = bp.stations.first()?.clone();
        (bp.power_watts > 0.0).then(|| (station, bp.power_watts, bp.idle_watts, bp.power_priority.max(1)))
    };
    // Not wired yet: new, or restored from a save, or a planet station whose
    // site had no power when it was last looked at.
    let todo: Vec<(hecs::Entity, String, f32, f32, u8, Option<PlanetSite>)> = world
        .query::<hecs::Without<(&Structure, Option<&PlanetSite>), &PowerConsumer>>()
        .iter()
        .filter_map(|(e, (s, site))| electric(s).map(|(st, w, i, p)| (e, st, w, i, p, site.cloned())))
        .collect();
    if todo.is_empty() {
        return;
    }
    // The home's islands: generation that is not on a planet.
    let mut by_island: HashMap<u32, f32> = HashMap::new();
    for (_e, (g, pc)) in world.query::<hecs::Without<(&PowerGenerator, &PowerCircuit), &PlanetSite>>().iter() {
        *by_island.entry(pc.island).or_default() += g.output_watts;
    }
    let home_island = by_island
        .into_iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map_or(0, |(i, _)| i);
    for (e, station, watts, idle, priority, site) in todo {
        let island = match &site {
            None => Some(home_island),
            Some(s) => site_power_island(world, s),
        };
        let _ = world.insert(e, (MachineType(station), StationLoad { active_watts: watts, idle_watts: idle }));
        if let Some(island) = island {
            let _ = world.insert(e, (PowerConsumer { draw_watts: idle, priority, enabled: true }, PowerCircuit { island }));
        }
    }
}

/// The power island of a planet build site: the island of a generator that
/// stands in `site`, or None when nothing there makes power (the home's
/// generators are in orbit and never count).
pub fn site_power_island(world: &hecs::World, site: &PlanetSite) -> Option<u32> {
    use crate::ecs::components::{PowerCircuit, PowerGenerator};
    let mut q = world.query::<(&PowerGenerator, &PowerCircuit, &PlanetSite)>();
    let found = q.iter().find(|(_e, (_, _, s))| *s == site).map(|(_e, (_, pc, _))| pc.island);
    found
}

/// Where the player's crafting stations are this frame (2026-09-27, the
/// planet-build review): aboard, the home's machines and the pieces built in
/// the home; on a planet's ground, the stations built in the site they stand
/// in; anywhere else (open space, a planet with nothing built near) none.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum StationsWhere {
    #[default]
    Home,
    Site(PlanetSite),
    Nowhere,
}

impl StationsWhere {
    /// The frame whose pieces serve here (None = the home), or None nowhere.
    pub fn frame(&self) -> Option<Option<&PlanetSite>> {
        match self {
            StationsWhere::Home => Some(None),
            StationsWhere::Site(s) => Some(Some(s)),
            StationsWhere::Nowhere => None,
        }
    }
}

/// Every machine type the player's FINISHED structures in `frame` serve as
/// (None = the home frame), for the recipe station gate (see
/// `Blueprint::stations`). A scaffold still going up is a `Construction`,
/// not a `Structure`, so it serves as nothing yet. A furnace built on Earth
/// is a smelter only at its site, never for a craft aboard the home.
pub fn built_station_types(
    world: &hecs::World,
    registry: &BlueprintRegistry,
    frame: Option<&PlanetSite>,
) -> std::collections::HashSet<String> {
    built_station_types_where(world, registry, frame, |_| true)
}

/// `built_station_types` over the pieces `keep` accepts by where they stand
/// (their pose; a piece with none is kept): aboard, a guest's put-away home
/// is in the home frame but not on the ship (engine::built_uses).
pub fn built_station_types_where(
    world: &hecs::World,
    registry: &BlueprintRegistry,
    frame: Option<&PlanetSite>,
    keep: impl Fn(Option<&Transform>) -> bool,
) -> std::collections::HashSet<String> {
    let mut out = std::collections::HashSet::new();
    for (_e, (s, site, pose)) in world.query::<(&Structure, Option<&PlanetSite>, Option<&Transform>)>().iter() {
        if !site::in_frame(site, frame) || !keep(pose) {
            continue;
        }
        if let Some(bp) = registry.get(&s.blueprint_id) {
            out.extend(bp.stations.iter().cloned());
        }
    }
    out
}

/// The first material a build of `bp` is short of, and how many more it
/// needs, or None when there is enough. `pack` counts what the builder
/// carries; `stores` what the home's storage holds, or None on a planet,
/// where only what the player carries can be used (the home's storage is in
/// orbit; the planet-build review, 2026-09-27).
pub fn materials_short(bp: &Blueprint, pack: impl Fn(&str) -> u32, stores: Option<&dyn Fn(&str) -> u32>) -> Option<(String, u32)> {
    materials_missing(bp, pack, stores).into_iter().next()
}

/// Every material a build of `bp` is short of, with how many more of each, in
/// the blueprint's order; empty when there is enough. The same count as
/// `materials_short`, which is its first entry: the build refuses on it, and
/// the crosshair names all of them (engine::build_place, first-hour audit
/// Friction 8).
pub fn materials_missing(bp: &Blueprint, pack: impl Fn(&str) -> u32, stores: Option<&dyn Fn(&str) -> u32>) -> Vec<(String, u32)> {
    bp.materials
        .iter()
        .filter_map(|(id, qty)| {
            let have = pack(id) + stores.map_or(0, |s| s(id));
            (have < *qty).then(|| (id.clone(), qty - have))
        })
        .collect()
}

/// Registry of all available blueprints.
pub struct BlueprintRegistry {
    pub blueprints: HashMap<String, Blueprint>,
}

impl BlueprintRegistry {
    pub fn new() -> Self {
        Self {
            blueprints: HashMap::new(),
        }
    }

    pub fn register(&mut self, bp: Blueprint) {
        self.blueprints.insert(bp.id.clone(), bp);
    }

    pub fn get(&self, id: &str) -> Option<&Blueprint> {
        self.blueprints.get(id)
    }

    /// Parse a RON array of `Blueprint` entries (the shape `data/blueprints/basic.ron` already
    /// ships, e.g. `[(id: "wood_wall", ...), ...]`) into a populated registry. Was never called
    /// anywhere before this (`ConstructionSystem` was registered but had nothing to build from --
    /// `queue_build` always missed the registry lookup and silently skipped every pending build).
    pub fn from_ron(bytes: &[u8]) -> Result<Self, String> {
        let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
        let blueprints: Vec<Blueprint> = ron::from_str(text).map_err(|e| e.to_string())?;
        let mut reg = Self::new();
        for bp in blueprints {
            reg.register(bp);
        }
        Ok(reg)
    }
}

/// Component for entities currently under construction.
pub struct Construction {
    pub blueprint_id: String,
    pub progress: f32,
    pub build_time: f32,
    pub builder_key: Option<String>,
}

/// Component for completed structures.
pub struct Structure {
    pub blueprint_id: String,
    pub health: f32,
    pub max_health: f32,
    pub provides: Option<String>,
    /// Stable identity within this home (2026-09-27), saved with the
    /// structure, so what refers to it outlives a restart: a built chest's
    /// contents are filed under `uses::storage_path(uid)`. 0 = not assigned
    /// yet; `uses::assign_uids` gives it the next free number on the
    /// ConstructionSystem's tick.
    pub uid: u32,
}

/// Construction system processes active builds each frame.
pub struct ConstructionSystem {
    /// Pending build commands.
    pending_builds: Vec<BuildRequest>,
}

impl ConstructionSystem {
    pub fn new() -> Self {
        Self {
            pending_builds: Vec::new(),
        }
    }

    /// Queue a build command.
    pub fn queue_build(&mut self, request: BuildRequest) {
        self.pending_builds.push(request);
    }
}

/// Start one build: refuse it, with the reason and nothing taken, or take its
/// materials and put up its scaffold, which the ConstructionSystem finishes
/// over the blueprint's build time. Returns the status line either way.
///
/// THE ONE PATH every build takes (2026-10-05, BUG-153: moved out of the
/// ConstructionSystem's tick unchanged, so a building ability, the Campfire,
/// builds exactly as a piece placed from the Crafting page does). In order:
/// an unknown blueprint; a piece built only outdoors where it cannot stand
/// ([`outdoors_refusal`]); the same piece already standing there; too few
/// materials. Blueprint builds take their materials in every play mode,
/// Creative and Dev included (the Dev page's "stock all materials" is how Dev
/// builds freely; the build editor's own machine placement is what goes free
/// there).
pub fn begin_build(world: &mut hecs::World, data: &DataStore, req: BuildRequest) -> Result<String, String> {
    let Some(bp) = data.get::<BlueprintRegistry>("blueprint_registry").and_then(|r| r.get(&req.blueprint_id).cloned()) else {
        return Err(format!("Unknown blueprint '{}'", req.blueprint_id));
    };
    // OUTDOORS ONLY (BUG-153): a fire is never built aboard the ship, under
    // a roof, or where there is no air to burn.
    if let Some(why) = outdoors_refusal(&bp, world, data, req.site.as_ref(), &req.pose) {
        return Err(format!("The {} is not built here: {}", bp.name, why.reason()));
    }
    // ONE PIECE PER SPOT (review of the shelter commit): a piece, or a
    // scaffold still going up, with this exact box already stands here, so a
    // second press would spend the materials twice for what looks like one
    // wall. Refused before anything is taken.
    if placement::occupied(world, &req.pose, req.site.as_ref()) {
        return Err(format!("{} not built: one already stands there", bp.name));
    }

    // MATERIALS ARE REAL (v0.746): the doc header always said "consumes
    // inventory materials" but nothing ever did. Count backpack + home
    // storage (the same home_stock mirror auto-machines use, v0.737), refuse
    // honestly when short, consume BACKPACK-FIRST when not.
    let home_stock = data.get::<std::sync::Mutex<std::collections::HashMap<String, u32>>>("home_stock");
    let home_count = |id: &str| -> u32 {
        home_stock
            .as_ref()
            .and_then(|m| m.lock().ok().map(|s| s.get(id).copied().unwrap_or(0)))
            .unwrap_or(0)
    };
    let player = world
        .query::<(&crate::systems::inventory::Inventory, &crate::ecs::components::Controllable)>()
        .iter()
        .next()
        .map(|(e, _)| e);
    let Some(player) = player else {
        return Err("No builder inventory".to_string());
    };
    // On a planet only the pack counts: the home's storage is in orbit
    // (review of the planet build, 2026-09-27).
    let on_planet = req.site.is_some();
    // Aboard, the home's storage counts where a hand craft's does
    // (crafting::home_store): not for a guest, whose home is put away off
    // this ship. The Crafting page's structures list counts the same way, so
    // the list and the build agree (the review of BUG-147).
    let away = if on_planet { None } else { crate::systems::crafting::home_store::HomeStore::here(data).not_here };
    let storage_counts = !on_planet && away.is_none();
    let missing = {
        let inv = world.get::<&crate::systems::inventory::Inventory>(player).expect("player inventory queried above");
        let stores: Option<&dyn Fn(&str) -> u32> = if storage_counts { Some(&home_count) } else { None };
        materials_missing(&bp, |id| inv.count_item(id), stores)
    };
    if !missing.is_empty() {
        // Every item it is short of, by name (2026-10-05; the first one, by
        // id, before).
        let list = missing_list(&missing, data.get::<crate::systems::inventory::ItemRegistry>("item_registry"));
        return Err(if on_planet {
            format!("need {list} in your pack to build {} here: on a planet you build from what you carry", bp.name)
        } else if let Some(why) = away {
            format!("need {list} in your pack to build {} here: {why}", bp.name)
        } else {
            format!("need {list} to build {}", bp.name)
        });
    }
    if let Ok(mut inv) = world.get::<&mut crate::systems::inventory::Inventory>(player) {
        for (id, qty) in &bp.materials {
            let from_pack = inv.count_item(id).min(*qty);
            if from_pack > 0 {
                inv.remove_item(id, from_pack);
            }
            let remainder = qty - from_pack;
            if remainder > 0 && storage_counts {
                if let Some(m) = home_stock.as_ref() {
                    if let Ok(mut s) = m.lock() {
                        if let Some(c) = s.get_mut(id) {
                            *c = c.saturating_sub(remainder);
                        }
                    }
                }
            }
        }
    }

    // Where it goes: the ghost's pose, as the player saw it (x and z on the
    // metre grid, turned, on the floor or on top of what it rests on), in its
    // frame: the home, or a site on a planet.
    let scaffold = world.spawn((
        req.pose,
        Construction {
            blueprint_id: bp.id.clone(),
            progress: 0.0,
            build_time: bp.build_time,
            builder_key: None,
        },
    ));
    if let Some(site) = req.site {
        let _ = world.insert_one(scaffold, site);
    }
    Ok(format!("Building {}...", bp.name))
}

impl System for ConstructionSystem {
    fn name(&self) -> &str {
        "Construction"
    }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, data: &DataStore) {
        // Process pending build commands: the internal queue (tests/API) PLUS the
        // "build_request" DataStore channel the GUI writes (v0.746, closure ladder
        // rung 2 — queue_build had zero callers before this channel existed).
        let mut builds: Vec<BuildRequest> = self.pending_builds.drain(..).collect();
        if let Some(chan) = data.get::<std::sync::Mutex<Vec<BuildRequest>>>("build_request") {
            if let Ok(mut c) = chan.lock() {
                builds.append(&mut c);
            }
        }
        let registry = data.get::<BlueprintRegistry>("blueprint_registry");
        let mut status: Option<String> = None;

        // Each one started or refused, with its status line (`begin_build`,
        // which a building ability calls too).
        for req in builds {
            status = Some(match begin_build(world, data, req) {
                Ok(started) => started,
                Err(refused) => refused,
            });
        }

        // Advance active constructions
        let mut completed = Vec::new();

        for (entity, construction) in world.query_mut::<&mut Construction>() {
            construction.progress += dt;
            if construction.progress >= construction.build_time {
                completed.push((entity, construction.blueprint_id.clone()));
            }
        }

        // Convert completed constructions to structures
        for (entity, bp_id) in completed {
            let _ = world.remove_one::<Construction>(entity);

            // A fire is lit as it is finished, with the fuel it was built
            // with (BUG-153: a campfire's three logs, `fires`).
            let (health, provides, name, fire) = registry
                .as_ref()
                .and_then(|r| r.get(&bp_id))
                .map(|bp| (bp.health, bp.provides.clone(), bp.name.clone(), fires::lit_when_finished(bp)))
                .unwrap_or((100.0, None, bp_id.clone(), None));

            let _ = world.insert_one(
                entity,
                Structure {
                    blueprint_id: bp_id.clone(),
                    health,
                    max_health: health,
                    provides,
                    uid: 0,
                },
            );
            if let Some(fuel) = fire {
                let _ = world.insert_one(entity, fuel);
            }
            // Completion is PROGRESS (v0.746): the construction quest chain's
            // Build objectives finally advance, and building trains the builder.
            crate::systems::quests::push_quest_event(data, format!("build_{bp_id}"));
            crate::systems::skills::award_skill_xp(data, "shelter_building", 15);
            // Placement thunk (v0.985): a finished structure lands audibly.
            crate::systems::push_sfx_event(
                data,
                "sfx.place_block",
                "audio/sfx/place_block.ogg",
            );
            status = Some(format!("{name} complete"));
        }

        // Every finished structure gets its stable uid: one just completed,
        // or one restored from a save written before uids existed.
        uses::assign_uids(world);

        // Built fires burn their fuel down on the game clock (BUG-153), the
        // clock the world's other stocks run on (`time::scaled_dt`), so a log
        // lasts 40 minutes of game time at any time speed and a fire burns
        // down through a night asleep.
        fires::burn(world, crate::systems::time::scaled_dt(dt, data));

        // Built electric stations join the home's power (2026-09-26).
        // Generators first, so a site's stations find its island this tick.
        if let Some(reg) = registry.as_ref() {
            wire_built_generators(world, reg);
            wire_built_stations(world, reg);
        }

        // One honest status line for the GUI (missing materials, in-progress,
        // completed) — same pattern as auto_craft_status.
        if let Some(s) = status {
            if let Some(slot) = data.get::<std::sync::Mutex<String>>("build_status") {
                if let Ok(mut b) = slot.lock() {
                    *b = s;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `data/blueprints/basic.ron` shipped a real foundation/wall/door/window/roof/
    /// furniture/machine catalog with nothing loading it (registered 2026-07-01, see
    /// lib.rs's load_data_registries) -- pins that `from_ron` actually parses the real
    /// shipped file, not just a synthetic fixture.
    #[test]
    fn from_ron_parses_the_real_shipped_blueprint_catalog() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("blueprints")
            .join("basic.ron");
        let bytes = std::fs::read(&path).expect("data/blueprints/basic.ron exists");
        let reg = BlueprintRegistry::from_ron(&bytes).expect("basic.ron parses as Vec<Blueprint>");
        assert!(reg.blueprints.len() >= 10, "expected a real multi-entry catalog, got {}", reg.blueprints.len());

        let wall = reg.get("wood_wall").expect("wood_wall is in the shipped catalog");
        assert_eq!(wall.category, "wall");
        assert!(!wall.materials.is_empty(), "a wall must cost real materials");
        assert!(wall.build_time > 0.0);

        let furnace = reg.get("furnace").expect("furnace is in the shipped catalog");
        assert_eq!(furnace.provides.as_deref(), Some("smelting"));
    }

    /// A registry built from garbage bytes must fail cleanly (Err), not panic --
    /// `load_data_registries` logs a warning and leaves ConstructionSystem idle on a
    /// bad/missing file rather than crashing world init.
    /// Every material a shipped blueprint asks for is a REAL item (2026-09-25).
    /// The catalog asked for "wood", "stone", "iron", "silicate" and "fiber",
    /// none of which is an item id, so in play nothing could ever be built:
    /// the build was refused for "need 6x wood" forever. The build tests
    /// below never saw it because they stock the player with the blueprint's
    /// own ids, so their evidence was their setup. This reads the shipped
    /// items.csv instead. Made red on the old catalog before the ids were
    /// fixed.
    #[test]
    fn every_blueprint_material_is_a_real_item() {
        let items = crate::systems::inventory::ItemRegistry::from_csv(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/items.csv"
        )))
        .expect("items.csv");
        let reg = shipped_registry();
        let mut missing: Vec<String> = Vec::new();
        for bp in reg.blueprints.values() {
            for (id, _) in &bp.materials {
                if !items.items.contains_key(id) {
                    missing.push(format!("{} needs '{id}'", bp.id));
                }
            }
        }
        missing.sort();
        assert!(missing.is_empty(), "blueprint materials that are not items: {missing:?}");
    }

    /// A FINISHED furnace serves as a smelter and a finished crafting table
    /// as a workbench; a scaffold still going up serves as nothing. Reads
    /// the shipped catalog, so the data and the gate cannot drift apart.
    #[test]
    fn built_structures_serve_as_their_stations_once_finished() {
        let reg = shipped_registry();
        let mut world = hecs::World::new();
        assert!(built_station_types(&world, &reg, None).is_empty());
        world.spawn((Construction {
            blueprint_id: "furnace".into(),
            progress: 1.0,
            build_time: 12.0,
            builder_key: None,
        },));
        assert!(built_station_types(&world, &reg, None).is_empty(), "a scaffold is not a smelter yet");
        for id in ["furnace", "crafting_table", "wood_wall"] {
            world.spawn((Structure { blueprint_id: id.into(), health: 1.0, max_health: 1.0, provides: None, uid: 0 },));
        }
        let got = built_station_types(&world, &reg, None);
        assert!(got.contains("smelter") && got.contains("workbench"), "{got:?}");
        assert!(got.contains("kiln"), "a furnace fires clay too: {got:?}");
    }

    /// A built stove joins the home's power (2026-09-26): it gets a power
    /// consumer on the island with the most generation, at its idle draw, and
    /// a workbench (no power role) gets nothing.
    #[test]
    fn a_built_stove_joins_the_home_power_and_a_workbench_does_not() {
        use crate::ecs::components::{MachineType, PowerCircuit, PowerConsumer, PowerGenerator, StationLoad};
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("blueprints").join("basic.ron");
        let reg = BlueprintRegistry::from_ron(&std::fs::read(path).unwrap()).unwrap();
        let mut world = hecs::World::new();
        world.spawn((PowerGenerator { output_watts: 50.0, fuel_per_second: 0.0, active: true }, PowerCircuit { island: 1 }));
        world.spawn((PowerGenerator { output_watts: 3000.0, fuel_per_second: 0.0, active: true }, PowerCircuit { island: 2 }));
        let stove = world.spawn((Structure { blueprint_id: "stove".into(), health: 1.0, max_health: 1.0, provides: None, uid: 0 },));
        let bench = world.spawn((Structure { blueprint_id: "crafting_table".into(), health: 1.0, max_health: 1.0, provides: None, uid: 0 },));
        wire_built_stations(&mut world, &reg);
        assert_eq!(world.get::<&MachineType>(stove).unwrap().0, "stove");
        assert_eq!(world.get::<&PowerCircuit>(stove).unwrap().island, 2, "the island with the most generation");
        let load = *world.get::<&StationLoad>(stove).unwrap();
        assert!(load.active_watts > 0.0);
        assert_eq!(world.get::<&PowerConsumer>(stove).unwrap().draw_watts, load.idle_watts, "idle until it works");
        assert!(world.get::<&PowerConsumer>(bench).is_err(), "a workbench needs no power");
        wire_built_stations(&mut world, &reg);
        assert_eq!(world.query::<&StationLoad>().iter().count(), 1, "wired once, not again");
    }

    /// THE HOME'S GRID STAYS IN ORBIT (planet-build review, 2026-09-27). A
    /// stove built at a site on Earth, while the home has 3 kW of generation,
    /// is not put on the home's island: it has its station type and its
    /// loads, no power consumer, and so it reads as a stove with no power
    /// at its site, while a stove built in the home is powered there. It is
    /// a station only in its own frame. When its site gains a generator of
    /// its own, it is wired to that generator's island. Red check, run: the
    /// old wiring (every station onto the strongest island) puts the Earth
    /// stove on the home's island 2 and the first assertion fails.
    #[test]
    fn a_stove_built_on_a_planet_is_not_on_the_homes_grid() {
        use crate::ecs::components::{PowerCircuit, PowerConsumer, PowerGenerator, StationLoad};
        use crate::systems::crafting::CraftingSystem;
        let reg = shipped_registry();
        let site = PlanetSite { body: "earth".into(), origin: glam::DVec3::new(0.0, 6_371_000.0, 0.0) };
        let mut world = hecs::World::new();
        world.spawn((PowerGenerator { output_watts: 3000.0, fuel_per_second: 0.0, active: true }, PowerCircuit { island: 2 }));
        let stove = || Structure { blueprint_id: "stove".into(), health: 1.0, max_health: 1.0, provides: None, uid: 0 };
        let on_earth = world.spawn((stove(), site.clone()));
        let aboard = world.spawn((stove(),));
        wire_built_stations(&mut world, &reg);
        assert!(world.get::<&PowerConsumer>(on_earth).is_err(), "the Earth stove is not on the home's grid");
        assert!(world.get::<&StationLoad>(on_earth).is_ok(), "but it is a known electric station");
        assert_eq!(world.get::<&PowerCircuit>(aboard).unwrap().island, 2, "the home's stove is");
        assert!(CraftingSystem::station_unpowered_at(&world, "stove", Some(&site)).is_some(), "no power at the site");
        assert!(CraftingSystem::station_unpowered_at(&world, "stove", None).is_none(), "powered aboard");
        // Station availability follows the frame.
        let mut other = hecs::World::new();
        other.spawn((Structure { blueprint_id: "furnace".into(), health: 1.0, max_health: 1.0, provides: None, uid: 0 }, site.clone()));
        assert!(built_station_types(&other, &reg, Some(&site)).contains("smelter"), "a smelter at its site");
        assert!(built_station_types(&other, &reg, None).is_empty(), "and not aboard the home");
        // The site gains power of its own: the stove joins that island.
        world.spawn((PowerGenerator { output_watts: 800.0, fuel_per_second: 0.0, active: true }, PowerCircuit { island: 9 }, site.clone()));
        wire_built_stations(&mut world, &reg);
        assert_eq!(world.get::<&PowerCircuit>(on_earth).unwrap().island, 9, "the site's own power");
        assert_eq!(world.get::<&PowerCircuit>(aboard).unwrap().island, 2, "the home's stove is unmoved");
        assert!(CraftingSystem::station_unpowered_at(&world, "stove", Some(&site)).is_none());
    }

    /// A PLANET BUILD TAKES WHAT THE PLAYER CARRIES (planet-build review,
    /// 2026-09-27). With an empty pack and the home's storage full of planks,
    /// a wall aboard is built from the storage, and a wall at a site on a
    /// planet is refused with a status that says to carry them; nothing is
    /// taken from the storage for it. Carrying the planks, it builds and the
    /// storage is untouched. Red check, run: counting the home's storage for
    /// a planet build (what the ConstructionSystem did before) builds the
    /// planet wall from orbit and the scaffold-count assertion fails.
    #[test]
    fn a_planet_build_takes_what_the_player_carries() {
        use crate::ecs::components::Controllable;
        use crate::systems::inventory::Inventory;
        let reg = shipped_registry();
        let wall = reg.get("wood_wall").unwrap().clone();
        let (plank, per_wall) = wall.materials[0].clone();
        let site = PlanetSite { body: "earth".into(), origin: glam::DVec3::new(0.0, 6_371_000.0, 0.0) };
        let pose = placement::placement_pose(&wall, Vec3::new(0.0, 0.0, 2.0), 0, &hecs::World::new(), &reg, None);
        let mut data = build_store(reg, vec![BuildRequest::new("wood_wall", pose.clone()).on(Some(site.clone()))]);
        let stock: HashMap<String, u32> = [(plank.clone(), per_wall * 10)].into_iter().collect();
        data.insert("home_stock", std::sync::Mutex::new(stock));
        let mut world = hecs::World::new();
        let player = world.spawn((Inventory::new(16), Controllable));
        let mut sys = ConstructionSystem::new();
        sys.tick(&mut world, 0.05, &data);
        assert_eq!(world.query::<&Construction>().iter().count(), 0, "nothing built on the planet from orbit");
        let status = data.get::<std::sync::Mutex<String>>("build_status").unwrap().lock().unwrap().clone();
        assert!(status.contains("in your pack") && status.contains("what you carry"), "{status}");
        let stored = |d: &DataStore| d.get::<std::sync::Mutex<HashMap<String, u32>>>("home_stock").unwrap().lock().unwrap()[&plank];
        assert_eq!(stored(&data), per_wall * 10, "the home's storage untouched");
        // Aboard, the same wall comes out of the storage.
        data.get::<std::sync::Mutex<Vec<BuildRequest>>>("build_request").unwrap().lock().unwrap().push(BuildRequest::new("wood_wall", pose.clone()));
        sys.tick(&mut world, 0.05, &data);
        assert_eq!(world.query::<(&Construction, &PlanetSite)>().iter().count(), 0);
        assert_eq!(world.query::<&Construction>().iter().count(), 1, "aboard it builds from the storage");
        assert_eq!(stored(&data), per_wall * 9);
        // Carrying the planks, the planet wall goes up and the storage is kept.
        world.get::<&mut Inventory>(player).unwrap().add_item(&plank, per_wall, 999);
        data.get::<std::sync::Mutex<Vec<BuildRequest>>>("build_request").unwrap().lock().unwrap().push(BuildRequest::new("wood_wall", pose).on(Some(site)));
        sys.tick(&mut world, 0.05, &data);
        assert_eq!(world.query::<(&Construction, &PlanetSite)>().iter().count(), 1, "built from the pack");
        assert_eq!(world.get::<&Inventory>(player).unwrap().count_item(&plank), 0);
        assert_eq!(stored(&data), per_wall * 9, "nothing more from orbit");
    }

    /// A GUEST BUILDS FROM WHAT THEY CARRY, as a guest crafts (the review of
    /// BUG-147). A guest's home is put away, off this ship, so its storage is
    /// not in reach for a build aboard either: the Crafting page's structures
    /// list already counts it that way (`home_storage_here`), and the build
    /// must agree with the list. Refused with the reason, storage untouched;
    /// with the home back on this ship the same wall comes out of storage.
    /// Seen red before the fix: "a guest: nothing built from the put-away
    /// home's storage" (left: 1, right: 0).
    #[test]
    fn a_guest_builds_from_what_they_carry() {
        use crate::ecs::components::Controllable;
        use crate::systems::crafting::home_store::HOME_STORAGE_HERE;
        use crate::systems::inventory::Inventory;
        let reg = shipped_registry();
        let wall = reg.get("wood_wall").unwrap().clone();
        let (plank, per_wall) = wall.materials[0].clone();
        let pose = placement::placement_pose(&wall, Vec3::new(0.0, 0.0, 2.0), 0, &hecs::World::new(), &reg, None);
        let mut data = build_store(reg, vec![BuildRequest::new("wood_wall", pose.clone())]);
        let stock: HashMap<String, u32> = [(plank.clone(), per_wall * 10)].into_iter().collect();
        data.insert("home_stock", std::sync::Mutex::new(stock));
        data.insert(HOME_STORAGE_HERE, std::sync::Mutex::new(false));
        let mut world = hecs::World::new();
        world.spawn((Inventory::new(16), Controllable));
        let mut sys = ConstructionSystem::new();
        sys.tick(&mut world, 0.05, &data);
        assert_eq!(world.query::<&Construction>().iter().count(), 0, "a guest: nothing built from the put-away home's storage");
        let status = data.get::<std::sync::Mutex<String>>("build_status").unwrap().lock().unwrap().clone();
        assert!(status.contains("in your pack") && status.contains("not on this ship"), "{status}");
        let stored = |d: &DataStore| d.get::<std::sync::Mutex<HashMap<String, u32>>>("home_stock").unwrap().lock().unwrap()[&plank];
        assert_eq!(stored(&data), per_wall * 10, "the put-away home's storage untouched");
        // The home back on this ship: the wall comes out of its storage.
        *data.get::<std::sync::Mutex<bool>>(HOME_STORAGE_HERE).unwrap().lock().unwrap() = true;
        data.get::<std::sync::Mutex<Vec<BuildRequest>>>("build_request").unwrap().lock().unwrap().push(BuildRequest::new("wood_wall", pose));
        sys.tick(&mut world, 0.05, &data);
        assert_eq!(world.query::<&Construction>().iter().count(), 1, "at home it builds from the storage");
        assert_eq!(stored(&data), per_wall * 9);
    }

    #[test]
    fn from_ron_rejects_malformed_data_without_panicking() {
        let result = BlueprintRegistry::from_ron(b"not valid ron at all {{{");
        assert!(result.is_err());
    }

    fn shipped_registry() -> BlueprintRegistry {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("blueprints")
            .join("basic.ron");
        BlueprintRegistry::from_ron(&std::fs::read(path).unwrap()).unwrap()
    }

    fn build_store(reg: BlueprintRegistry, request: Vec<BuildRequest>) -> DataStore {
        let mut data = DataStore::new();
        data.insert("blueprint_registry", reg);
        data.insert("build_request", std::sync::Mutex::new(request));
        data.insert("build_status", std::sync::Mutex::new(String::new()));
        data.insert("quest_events", std::sync::Mutex::new(Vec::<String>::new()));
        data
    }

    /// v0.746 (closure ladder rung 2): THE BUILD LOOP. A build_request consumes
    /// the blueprint's materials from the builder's inventory, spawns a timed
    /// Construction, converts it to a Structure at completion, and fires the
    /// build_<id> quest event the authored construction quests wait on.
    #[test]
    fn build_request_consumes_materials_and_completes() {
        use crate::ecs::components::Controllable;
        use crate::systems::inventory::Inventory;

        let reg = shipped_registry();
        let wall = reg.get("wood_wall").unwrap().clone();
        let ghost = placement::placement_pose(&wall, Vec3::new(1.2, 0.0, 3.7), 0, &hecs::World::new(), &reg, None);
        let data = build_store(reg, vec![BuildRequest::new("wood_wall", ghost)]);
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(16);
        for (id, qty) in &wall.materials {
            inv.add_item(id, *qty, 99);
        }
        let player = world.spawn((inv, Controllable));

        let mut sys = ConstructionSystem::new();
        sys.tick(&mut world, 0.05, &data);

        {
            let inv = world.get::<&Inventory>(player).unwrap();
            for (id, _qty) in &wall.materials {
                assert_eq!(inv.count_item(id), 0, "{id} consumed at build start");
            }
        }
        {
            let mut q = world.query::<(&Construction, &Transform)>();
            let (_, (_, tf)) = q.iter().next().expect("a Construction spawned");
            assert_eq!(tf.position, Vec3::new(1.0, 0.0, 4.0), "snapped to the metre grid");
        }

        // Run past the build time: the scaffold becomes a real Structure.
        sys.tick(&mut world, wall.build_time + 1.0, &data);
        assert_eq!(world.query::<&Structure>().iter().count(), 1, "structure completed");
        assert_eq!(world.query::<&Construction>().iter().count(), 0);
        let events = data
            .get::<std::sync::Mutex<Vec<String>>>("quest_events")
            .unwrap()
            .lock()
            .unwrap()
            .clone();
        assert!(
            events.iter().any(|e| e == "build_wood_wall"),
            "build quest event fired, got {events:?}"
        );
    }

    /// Building with nothing in the pack is REFUSED with an honest status line
    /// (nothing spawns, nothing is consumed).
    #[test]
    fn build_refused_without_materials() {
        use crate::ecs::components::Controllable;
        use crate::systems::inventory::Inventory;

        let data = build_store(
            shipped_registry(),
            vec![BuildRequest::new("wood_wall", Transform::default())],
        );
        let mut world = hecs::World::new();
        world.spawn((Inventory::new(8), Controllable));

        let mut sys = ConstructionSystem::new();
        sys.tick(&mut world, 0.05, &data);

        assert_eq!(world.query::<&Construction>().iter().count(), 0, "no scaffold");
        let status = data
            .get::<std::sync::Mutex<String>>("build_status")
            .unwrap()
            .lock()
            .unwrap()
            .clone();
        assert!(status.contains("need"), "status explains the shortage: {status}");
    }

    /// A player stocked for THREE walls presses E twice at one spot and once
    /// beside it (review of the shelter commit: nothing stopped two pieces
    /// being built in one spot, spending the materials twice). The second
    /// press at the same spot is refused, while the first is still a
    /// scaffold, and after it is finished; nothing is taken for it; the
    /// press beside it builds. Red check, run: removing the `occupied` guard
    /// from the ConstructionSystem builds the double and spends a second
    /// wall's planks, and the scaffold-count assertion fails.
    #[test]
    fn a_duplicate_build_is_refused_and_costs_nothing() {
        use crate::ecs::components::Controllable;
        use crate::systems::inventory::Inventory;
        let reg = shipped_registry();
        let wall = reg.get("wood_wall").unwrap().clone();
        let empty = hecs::World::new();
        let here = placement::placement_pose(&wall, Vec3::new(0.0, 0.0, 2.0), 0, &empty, &reg, None);
        let beside = placement::placement_pose(&wall, Vec3::new(4.0, 0.0, 2.0), 0, &empty, &reg, None);
        let data = build_store(reg, vec![BuildRequest::new("wood_wall", here.clone()), BuildRequest::new("wood_wall", here.clone())]);
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(16);
        let (plank, per_wall) = wall.materials[0].clone();
        inv.add_item(&plank, per_wall * 3, 999);
        let player = world.spawn((inv, Controllable));
        let mut sys = ConstructionSystem::new();
        sys.tick(&mut world, 0.05, &data);
        assert_eq!(world.query::<&Construction>().iter().count(), 1, "one scaffold, not two");
        assert_eq!(world.get::<&Inventory>(player).unwrap().count_item(&plank), per_wall * 2, "one wall's planks spent");
        let status = data.get::<std::sync::Mutex<String>>("build_status").unwrap().lock().unwrap().clone();
        assert!(status.contains("already stands"), "the status says why: {status}");

        // Finished, it still refuses the same spot; the spot beside it builds.
        sys.tick(&mut world, wall.build_time + 1.0, &data);
        {
            let chan = data.get::<std::sync::Mutex<Vec<BuildRequest>>>("build_request").unwrap();
            let mut c = chan.lock().unwrap();
            c.push(BuildRequest::new("wood_wall", here.clone()));
            c.push(BuildRequest::new("wood_wall", beside));
        }
        sys.tick(&mut world, 0.05, &data);
        assert_eq!(world.query::<&Structure>().iter().count(), 1);
        assert_eq!(world.query::<&Construction>().iter().count(), 1, "only the wall beside it");
        assert_eq!(world.get::<&Inventory>(player).unwrap().count_item(&plank), per_wall, "two walls' planks spent in all");
    }

    /// The build lands exactly at the ghost's pose, in the ghost's frame: a
    /// request carrying a pose off the metre grid, turned by an angle no
    /// quarter turn makes, at a height no floor has, in a planet build site,
    /// is built at that pose to the bit and carries its site. Red check,
    /// run: recomputing the pose at the key press from the aimed point
    /// (what the ConstructionSystem did before) snaps it back onto the grid,
    /// and the position assertion fails.
    #[test]
    fn the_build_lands_exactly_at_the_ghosts_pose() {
        use crate::ecs::components::Controllable;
        use crate::systems::inventory::Inventory;
        let reg = shipped_registry();
        let wall = reg.get("wood_wall").unwrap().clone();
        let site = PlanetSite { body: "earth".into(), origin: glam::DVec3::new(1_000.0, 6_370_000.0, -2_000.0) };
        let ghost = Transform {
            position: Vec3::new(3.37, 0.42, -1.19),
            rotation: glam::Quat::from_rotation_y(0.3),
            scale: Vec3::from_array(wall.size),
        };
        let data = build_store(reg, vec![BuildRequest::new("wood_wall", ghost.clone()).on(Some(site.clone()))]);
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(16);
        for (id, qty) in &wall.materials {
            inv.add_item(id, *qty, 99);
        }
        world.spawn((inv, Controllable));
        ConstructionSystem::new().tick(&mut world, 0.05, &data);
        let mut q = world.query::<(&Construction, &Transform, &PlanetSite)>();
        let (_e, (_c, tf, s)) = q.iter().next().expect("a scaffold in the site");
        assert_eq!(tf.position, ghost.position, "built where the ghost stood");
        assert_eq!(tf.rotation, ghost.rotation);
        assert_eq!(tf.scale, ghost.scale);
        assert_eq!(*s, site, "in the ghost's frame");
    }
}
