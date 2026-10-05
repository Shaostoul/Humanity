//! What dying costs: the Death setting's two modes (operator decision, 2026-10-04;
//! docs/design/death-and-your-pack.md).
//!
//! The operator, accepting the first-hour audit's recommendation: "Today the death screen
//! says 'Nothing was lost.' I'd keep that as the Simplified mode. In Realistic mode, your
//! carried items would stay where you fell for a while, to go back for." That is the
//! project's dual-mode rule (CLAUDE.md, "Dual modes"): a deep system ships full realism
//! and a simplified mode, and general play defaults to the simplified one.
//!
//! - SIMPLIFIED (the default): nothing is lost. The death screen says so, as it always did.
//! - REALISTIC: everything in the backpack stays where the player fell, as a pack
//!   (`LeftPack`, an entity of its own). What they wear (the `Outfit`) stays on them, and
//!   so does everything that was never in the backpack: credits, skills, quests, home
//!   storage. The pack lies on the floor or the ground where they fell; where no one can
//!   stand (open space, deep water) it lies at the nearest place that can be walked to
//!   (`aboard_spot`, `ground_spot`). Walking back to it and pressing E takes back as much
//!   as the backpack holds and leaves the rest in the pack (`take_back`). It stays for the
//!   minutes of PLAY data/world/death.ron says, counted only while the player is in the
//!   world and alive (`count_down`), so time with the game closed never counts and quitting
//!   never loses it; then it is gone, with a notice.
//!
//! On a shared server the pack is the player's alone for now: it lives in their own game
//! and their own save, so nobody else sees it or can take it. A pack others can loot is a
//! later decision for the operator (the design note says what it would need).
//!
//! This file is the rules and the arithmetic only: no window, no renderer, no settings, so
//! it compiles into the relay with the save format (`persistence::WorldSave::left_packs`)
//! and every rule is tested without a game. The engine's half (where the player fell, the
//! drawing, the marker, the E press, the clock) is src/engine/death_pack.rs.
//!
//! Every number the Realistic mode plays by is in data/world/death.ron (`DeathRules`);
//! none is written here.

use std::path::Path;

use glam::{DVec3, Vec3};
use serde::{Deserialize, Serialize};

use crate::ecs::components::Controllable;
use crate::systems::inventory::{Inventory, ItemRegistry, ItemStack};

/// The rules file, relative to the data directory. Loaded once, into the GUI's share
/// (`DeathPackHud::rules`, through `DeathRules::default` when the `GuiState` is made at
/// startup, after the data directory is known); restart the game to apply an edit.
pub const RULES_FILE: &str = "world/death.ron";

/// Which death the player chose in Settings > Gameplay > Death.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DeathMode {
    /// Nothing is lost (the default, the house rule for deep systems).
    #[default]
    Simplified,
    /// The backpack's contents stay where you fell, in a pack to go back for.
    Realistic,
}

impl DeathMode {
    /// The mode from the saved switch (`AppConfig::death_realistic`).
    pub fn from_realistic(realistic: bool) -> Self {
        if realistic {
            DeathMode::Realistic
        } else {
            DeathMode::Simplified
        }
    }
}

/// Everything the Realistic mode plays by: data/world/death.ron, field for field. Each
/// field is described in the file.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct DeathRules {
    pub keep_minutes_of_play: f64,
    pub warn_minutes_left: f64,
    pub reach_m: f64,
    pub facing_cos: f64,
    pub air_m: f64,
    pub wall_clearance_m: f32,
    pub room_slack_m: f32,
    pub open_space_m: f32,
    pub shore: ShoreSearch,
    pub look: PackLook,
}

/// How the nearest dry ground is searched for after a death over deep water (`nearest_dry`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ShoreSearch {
    pub first_ring_m: f64,
    pub ring_growth: f64,
    pub bearings: u32,
    pub max_m: f64,
    pub onto_land_m: f64,
}

/// How a pack looks where it lies.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PackLook {
    /// The main bag: width, height and depth, metres.
    pub size_m: (f32, f32, f32),
    /// Linear RGB, 0 to 1.
    pub canvas: (f32, f32, f32),
    pub straps: (f32, f32, f32),
}

impl DeathRules {
    /// Parse the rules from the text of data/world/death.ron.
    pub fn from_ron(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| e.to_string())
    }

    /// The rules the game plays by: data/world/death.ron on disk (a mod or a tune changes
    /// it without a rebuild), else the copy built into the exe. A disk copy that does not
    /// parse is said in the log and the built-in copy is used instead, never a crash.
    pub fn load(data_dir: &Path) -> Self {
        if let Some(text) = crate::embedded_data::read_data_or_embedded(data_dir, "world/death.ron") {
            match Self::from_ron(&text) {
                Ok(rules) => return rules,
                Err(e) => log::warn!("data/world/death.ron did not parse: {e}"),
            }
        }
        crate::embedded_data::note_builtin_copy("world/death.ron", "the file on disk is missing or did not parse");
        let built_in = crate::embedded_data::get_embedded("world/death.ron").unwrap_or_default();
        Self::from_ron(built_in).expect("the built-in data/world/death.ron parses (pinned by a test)")
    }

    /// Seconds of play a pack stays.
    pub fn keep_s(&self) -> f64 {
        self.keep_minutes_of_play.max(0.0) * 60.0
    }

    /// Seconds of play left when the warning is given (0 for none).
    pub fn warn_s(&self) -> f64 {
        self.warn_minutes_left.max(0.0) * 60.0
    }
}

impl Default for DeathRules {
    /// The rules as the data folder has them (`load` from the game's data directory). A
    /// default exists only so `GuiState` can be built before the engine loads them; it reads
    /// the same file, so no number is written here.
    fn default() -> Self {
        Self::load(&crate::data_dir())
    }
}

/// Where a pack lies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PackPlace {
    /// Aboard the ship: on a floor at `at`, in ship metres (the home frame the rooms, the
    /// built pieces and the parked vehicles are in).
    Aboard { at: [f32; 3] },
    /// On the ground of a planet or moon: `at` is the point on its ground in the body's own
    /// frame, the one its terrain is built in (metres from its centre, f64: an f32 at planet
    /// scale is half a metre coarse).
    Ground { body: String, at: [f64; 3] },
}

/// Whether the pack lies where the player fell, and if not, why and how far from it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum Landing {
    /// On the floor or the ground where they fell.
    #[default]
    WhereYouFell,
    /// They died in the air over the ground: it fell to the ground below, `drop_m` down.
    FromAir { drop_m: f64 },
    /// They died in open space (aboard, outside every room; off the ship, near no ground):
    /// it lies on the nearest floor that can be walked to, `dist_m` away.
    FromOpenSpace { dist_m: f64 },
    /// They died over deep water: it lies on the nearest dry ground, `dist_m` away.
    FromDeepWater { dist_m: f64 },
    /// They died on or over a body with no ground to walk to near them (a world of water with
    /// no dry ground within the shore search, a body with no known ground): it lies on the
    /// ship's nearest floor, `dist_m` away.
    FromNoGround { dist_m: f64 },
}

/// A pack left where its owner fell (an entity of its own). Saved whole
/// (`persistence::WorldSave::left_packs`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeftPack {
    /// What was in the backpack, each stack as it was (wear, grade and a food's age kept).
    pub items: Vec<ItemStack>,
    pub place: PackPlace,
    /// Where it lies, in words for a sentence: "in the Kitchen", "on Earth".
    #[serde(default)]
    pub place_words: String,
    #[serde(default)]
    pub landing: Landing,
    /// Seconds of play since it was left: counted only while the player is in the world and
    /// alive (`count_down`), never while the game is closed.
    #[serde(default)]
    pub played_s: f64,
    /// The "gone soon" notice was given.
    #[serde(default)]
    pub warned: bool,
    /// Left by the death still on screen: the death screen describes this pack, including
    /// after a quit and a load on the death screen. Cleared once the player is alive.
    #[serde(default)]
    pub fresh: bool,
}

impl LeftPack {
    /// How many items it holds.
    pub fn count(&self) -> u32 {
        self.items.iter().map(|s| s.quantity).sum()
    }
}

/// Where a death leaves the pack, worked out by the engine from where the player fell.
#[derive(Debug, Clone, PartialEq)]
pub struct PackSpot {
    pub place: PackPlace,
    pub place_words: String,
    pub landing: Landing,
}

/// What the death screen says about the backpack.
#[derive(Debug, Clone, PartialEq)]
pub enum DeathNote {
    /// Simplified (or a death out of the world, with nowhere to leave it): nothing lost.
    NothingLost,
    /// Realistic, with nothing in the backpack: nothing was left.
    NothingCarried,
    /// Realistic: `items` items stay in a pack, `place_words`, landed as `landing`.
    Left { items: u32, place_words: String, landing: Landing },
}

/// The death screen's and the crosshair's share of the GUI (`GuiState::death_pack`).
#[derive(Debug, Clone, Default)]
pub struct DeathPackHud {
    /// What the death screen says about the backpack: set when the death is surfaced, None
    /// while alive.
    pub note: Option<DeathNote>,
    /// The crosshair prompt at a pack in reach ("[E] Take back your pack (14 items)"), empty
    /// otherwise.
    pub prompt: String,
    /// The rules, for the death screen and the Settings hint.
    pub rules: DeathRules,
}

/// "1 item", "14 items".
pub fn items_words(n: u32) -> String {
    if n == 1 {
        "1 item".to_string()
    } else {
        format!("{n} items")
    }
}

/// "1 minute", "60 minutes", rounded up (so "gone in 0 minutes" is never said).
pub fn minutes_words(minutes: f64) -> String {
    let m = minutes.max(0.0).ceil().max(1.0) as u64;
    if m == 1 {
        "1 minute".to_string()
    } else {
        format!("{m} minutes")
    }
}

/// The player: the controllable entity with a backpack.
pub fn player(world: &hecs::World) -> Option<hecs::Entity> {
    world.query::<(&Inventory, &Controllable)>().iter().next().map(|(e, _)| e)
}

/// A death has just been surfaced (the engine's `after_tick`): in Simplified nothing
/// happens; in Realistic everything in the backpack goes into a pack at `spot`, which the
/// engine worked out from where the player fell (None: there was nowhere to leave it, a
/// death outside the world, so nothing is lost). Worn clothing and gear are the `Outfit`,
/// which is not touched. Returns what the death screen says.
pub fn on_death(world: &mut hecs::World, mode: DeathMode, spot: Option<PackSpot>) -> DeathNote {
    if mode == DeathMode::Simplified {
        return DeathNote::NothingLost;
    }
    let Some(p) = player(world) else { return DeathNote::NothingLost };
    let carried = world.get::<&Inventory>(p).is_ok_and(|inv| inv.slots.iter().flatten().any(|s| s.quantity > 0));
    if !carried {
        return DeathNote::NothingCarried;
    }
    let Some(spot) = spot else { return DeathNote::NothingLost };
    let items = match world.get::<&mut Inventory>(p) {
        Ok(mut inv) => empty_backpack(&mut inv),
        Err(_) => return DeathNote::NothingLost,
    };
    let pack = LeftPack {
        items,
        place: spot.place,
        place_words: spot.place_words.clone(),
        landing: spot.landing.clone(),
        played_s: 0.0,
        warned: false,
        fresh: true,
    };
    let items = pack.count();
    world.spawn((pack,));
    DeathNote::Left { items, place_words: spot.place_words, landing: spot.landing }
}

/// Take every stack out of the backpack. Its load is zero now (the InventorySystem's next
/// tick recounts it anyway); the slots it has grown stay.
fn empty_backpack(inv: &mut Inventory) -> Vec<ItemStack> {
    let items: Vec<ItemStack> = inv.slots.iter_mut().filter_map(|s| s.take()).filter(|s| s.quantity > 0).collect();
    inv.weight_current = 0.0;
    inv.volume_current_l = 0.0;
    inv.encumbered = false;
    items
}

/// What an E press at a pack did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TakeBack {
    /// Items now back in the backpack.
    pub taken: u32,
    /// Items still in the pack (it is gone when this is 0).
    pub left: u32,
}

/// The E press at `pack`: as much of it as the backpack takes goes back in, each stack with
/// its wear, grade and age, through the same volume limit every add to the backpack meets;
/// the rest stays in the pack. An emptied pack is gone. None when there is no player or no
/// such pack.
pub fn take_back(world: &mut hecs::World, pack: hecs::Entity, items: Option<&ItemRegistry>) -> Option<TakeBack> {
    let p = player(world)?;
    let mut contents = world.get::<&LeftPack>(pack).ok()?.items.clone();
    let taken = {
        let mut inv = world.get::<&mut Inventory>(p).ok()?;
        put_back(&mut inv, &mut contents, items)
    };
    let left: u32 = contents.iter().map(|s| s.quantity).sum();
    if left == 0 {
        let _ = world.despawn(pack);
    } else if let Ok(mut lp) = world.get::<&mut LeftPack>(pack) {
        lp.items = contents;
    }
    Some(TakeBack { taken, left })
}

/// Move as much of `contents` into `inv` as fits; each stack keeps what did not fit.
/// Returns how many items went in.
fn put_back(inv: &mut Inventory, contents: &mut Vec<ItemStack>, items: Option<&ItemRegistry>) -> u32 {
    let mut taken = 0;
    for st in contents.iter_mut() {
        let max_stack = items.map_or(st.max_stack.max(1), |r| r.max_stack_for(&st.item_id));
        let volume = items.map_or(0.0, |r| r.volume_for(&st.item_id));
        let left = inv.add_item_worn(&st.item_id, st.quantity, max_stack, volume, st.wear, st.quality, st.age_s);
        taken += st.quantity - left;
        st.quantity = left;
    }
    contents.retain(|s| s.quantity > 0);
    taken
}

/// Something a pack's clock did (`count_down`), for a notice.
#[derive(Debug, Clone, PartialEq)]
pub enum PackEvent {
    /// `minutes_left` minutes of play before the pack `place_words` (holding `items`) is gone.
    Warn { place_words: String, items: u32, minutes_left: f64 },
    /// The pack `place_words` is gone, with the `items` it held.
    Gone { place_words: String, items: u32 },
}

/// `secs` more of play: every pack's clock moves on; a pack past the rules' time is gone,
/// and one inside the warning time says so once. The engine calls this only while the
/// player is in the world and alive, so time with the game closed never counts.
pub fn count_down(world: &mut hecs::World, secs: f64, rules: &DeathRules) -> Vec<PackEvent> {
    if !(secs > 0.0) {
        return Vec::new();
    }
    let (keep, warn) = (rules.keep_s(), rules.warn_s());
    let mut events = Vec::new();
    let mut gone = Vec::new();
    for (e, p) in world.query_mut::<&mut LeftPack>() {
        p.played_s += secs;
        let left = keep - p.played_s;
        if left <= 0.0 {
            gone.push(e);
            events.push(PackEvent::Gone { place_words: p.place_words.clone(), items: p.count() });
        } else if !p.warned && warn > 0.0 && left <= warn {
            p.warned = true;
            events.push(PackEvent::Warn { place_words: p.place_words.clone(), items: p.count(), minutes_left: left / 60.0 });
        }
    }
    for e in gone {
        let _ = world.despawn(e);
    }
    events
}

/// The packs in the world, oldest first, for the save.
pub fn packs(world: &hecs::World) -> Vec<LeftPack> {
    let mut out: Vec<LeftPack> = world.query::<&LeftPack>().iter().map(|(_e, p)| p.clone()).collect();
    out.sort_by(|a, b| b.played_s.total_cmp(&a.played_s));
    out
}

/// Put a save's packs into the world. The save is authoritative, like the backpack it was
/// saved with: the packs in the world go and the saved ones come back, each with the play
/// time it had (the time the game was closed is not added: a pack counts only play).
pub fn restore(world: &mut hecs::World, saved: &[LeftPack]) {
    let old: Vec<hecs::Entity> = world.query_mut::<&LeftPack>().into_iter().map(|(e, _)| e).collect();
    for e in old {
        let _ = world.despawn(e);
    }
    for p in saved {
        world.spawn((p.clone(),));
    }
}

/// Every pack is no longer the death on screen's: the player is alive.
pub fn settle(world: &mut hecs::World) {
    for (_e, p) in world.query_mut::<&mut LeftPack>() {
        p.fresh = false;
    }
}

/// The death screen's note for a death in progress that came back with a save (quit on the
/// death screen and loaded again): the pack it left, else nothing carried when the backpack
/// is empty in Realistic, else nothing lost.
pub fn note_for_loaded_death(world: &hecs::World, mode: DeathMode) -> DeathNote {
    if let Some((_e, p)) = world.query::<&LeftPack>().iter().find(|(_e, p)| p.fresh) {
        return DeathNote::Left { items: p.count(), place_words: p.place_words.clone(), landing: p.landing.clone() };
    }
    let empty = player(world)
        .and_then(|e| world.get::<&Inventory>(e).ok().map(|inv| inv.slots.iter().flatten().all(|s| s.quantity == 0)))
        .unwrap_or(false);
    if mode == DeathMode::Realistic && empty {
        DeathNote::NothingCarried
    } else {
        DeathNote::NothingLost
    }
}

/// Food in a pack keeps aging, as food does wherever it is kept (first-hour audit S6): by
/// the game seconds at room temperature the home's air gave this frame
/// (`engine::stock_piles::age_home_storage`). KNOWN GAP, the planet chest's: a pack on a
/// planet ages at the home air's rate, not that planet's weather.
pub fn age_food(world: &mut hecs::World, secs: f64, is_food: impl Fn(&str) -> bool) {
    if !(secs > 0.0) {
        return;
    }
    for (_e, p) in world.query_mut::<&mut LeftPack>() {
        for st in p.items.iter_mut().filter(|s| is_food(&s.item_id)) {
            st.age_s += secs;
        }
    }
}

/// Carry the packs lying aboard inside the box `from` by `delta`: the home moved to another
/// plot, and what lies in it goes with it, as its built pieces and parked vehicles do
/// (engine/home_plot.rs `carry_built_pieces`). `inside` says whether a point is in `from`.
/// Returns how many moved.
pub fn carry_aboard(world: &mut hecs::World, inside: impl Fn(Vec3) -> bool, delta: Vec3) -> usize {
    if delta == Vec3::ZERO {
        return 0;
    }
    let mut moved = 0;
    for (_e, p) in world.query_mut::<&mut LeftPack>() {
        if let PackPlace::Aboard { at } = &mut p.place {
            let v = Vec3::from_array(*at);
            if inside(v) {
                *at = (v + delta).to_array();
                moved += 1;
            }
        }
    }
    moved
}

/// Where a pack left aboard lies (`aboard_spot`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AboardSpot {
    /// On a floor, ship metres.
    pub at: Vec3,
    /// Which of the rooms it lies in.
    pub room: usize,
    /// None: where the player fell. Some(metres): moved there from open space.
    pub moved_m: Option<f32>,
}

/// Where a pack left aboard lies. `rooms` are the walkable rooms' boxes, (min, max) in ship
/// metres, a room's floor at its min.y (the floor every walking player is kept on, lib.rs);
/// `feet` is where the player's feet were and `floor_under` the floor the walk had under
/// them (a deck or a stair top). In a room (its box, `slack` metres under its floor or over
/// its ceiling included): there, on that floor, kept `clearance` in from the walls. Outside
/// every room by more than `open_space_m` (on the hull, out in space): open space, where no
/// one can walk, so it lies on the nearest room's floor and says how far that was. Outside
/// by less (a doorway between two rooms' boxes, a wall's width): on the nearest floor, as
/// where they fell. None with no rooms. The three distances are data/world/death.ron's.
pub fn aboard_spot(rooms: &[(Vec3, Vec3)], feet: Vec3, floor_under: f32, clearance: f32, slack: f32, open_space_m: f32) -> Option<AboardSpot> {
    let inset = |lo: f32, hi: f32, v: f32| {
        if hi - lo > 2.0 * clearance {
            v.clamp(lo + clearance, hi - clearance)
        } else {
            0.5 * (lo + hi)
        }
    };
    let over = |r: &(Vec3, Vec3)| feet.x >= r.0.x && feet.x <= r.1.x && feet.z >= r.0.z && feet.z <= r.1.z;
    // Standing in a room: the highest floor at or under the feet (stacked storeys).
    let standing = rooms
        .iter()
        .enumerate()
        .filter(|(_, r)| over(r) && feet.y >= r.0.y - slack && feet.y <= r.1.y + slack)
        .max_by(|a, b| a.1 .0.y.total_cmp(&b.1 .0.y));
    if let Some((i, r)) = standing {
        let y = if floor_under >= r.0.y - 0.01 && floor_under <= r.1.y { floor_under } else { r.0.y };
        return Some(AboardSpot { at: Vec3::new(inset(r.0.x, r.1.x, feet.x), y, inset(r.0.z, r.1.z, feet.z)), room: i, moved_m: None });
    }
    rooms
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let at = Vec3::new(inset(r.0.x, r.1.x, feet.x), r.0.y, inset(r.0.z, r.1.z, feet.z));
            (i, at, at.distance(feet))
        })
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(room, at, d)| AboardSpot { at, room, moved_m: (d > open_space_m).then_some(d) })
}

/// Where a pack left on a planet's ground lies (`ground_spot`).
#[derive(Debug, Clone, PartialEq)]
pub struct GroundSpot {
    /// On the ground, in the body's own frame (metres from its centre).
    pub at: DVec3,
    pub landing: Landing,
}

/// Where a pack left on a planet lies. `eye` is the player's eye in the body's frame,
/// `eye_height` how far over their feet it is, and `ground(dir)` the ground's radius under
/// a unit direction and whether it is the floor of a sea there. On dry ground: there (from
/// the air, it fell to the ground below and says how far). Over deep water: the nearest dry
/// ground (`nearest_dry`). None when there is no dry ground within the rules' reach, or no
/// direction to stand on (the eye at the body's centre).
pub fn ground_spot(eye: DVec3, eye_height: f64, rules: &DeathRules, ground: impl Fn(DVec3) -> (f64, bool)) -> Option<GroundSpot> {
    let up = eye.normalize_or_zero();
    if up == DVec3::ZERO {
        return None;
    }
    let (r0, wet) = ground(up);
    if wet {
        let (dir, r, dist_m) = nearest_dry(up, r0, &rules.shore, &ground)?;
        return Some(GroundSpot { at: dir * r, landing: Landing::FromDeepWater { dist_m } });
    }
    let drop_m = eye.length() - eye_height - r0;
    let landing = if drop_m > rules.air_m { Landing::FromAir { drop_m } } else { Landing::WhereYouFell };
    Some(GroundSpot { at: up * r0, landing })
}

/// The nearest dry ground to `up` (a unit direction of the body's frame, over water), on a
/// body whose ground there is `radius_m` from its centre: (its direction, its ground radius,
/// how far along the surface from `up`). Searched in rings (data/world/death.ron `shore`);
/// at the first ring with dry ground it walks back along that bearing to the shore, then
/// `onto_land_m` past it when that is dry too. None when no ring out to `max_m` has any.
pub fn nearest_dry(up: DVec3, radius_m: f64, s: &ShoreSearch, ground: &impl Fn(DVec3) -> (f64, bool)) -> Option<(DVec3, f64, f64)> {
    let (east, north) = tangent_basis(up);
    let radius_m = radius_m.max(1.0);
    let along = |bearing: f64, dist_m: f64| {
        let t = east * bearing.cos() + north * bearing.sin();
        let a = dist_m / radius_m;
        (up * a.cos() + t * a.sin()).normalize()
    };
    let dry = |d: DVec3| !ground(d).1;
    let n = s.bearings.max(4);
    let growth = s.ring_growth.max(1.05);
    let (mut inner, mut ring) = (0.0_f64, s.first_ring_m.max(1.0));
    while ring <= s.max_m {
        for i in 0..n {
            let bearing = std::f64::consts::TAU * f64::from(i) / f64::from(n);
            if !dry(along(bearing, ring)) {
                continue;
            }
            // Wet at `inner` (the whole ring there was), dry at `ring`: halve to the shore.
            let (mut wet_m, mut dry_m) = (inner, ring);
            for _ in 0..24 {
                let mid = 0.5 * (wet_m + dry_m);
                if dry(along(bearing, mid)) {
                    dry_m = mid;
                } else {
                    wet_m = mid;
                }
            }
            let onto = dry_m + s.onto_land_m.max(0.0);
            let dist = if dry(along(bearing, onto)) { onto } else { dry_m };
            let dir = along(bearing, dist);
            return Some((dir, ground(dir).0, dist));
        }
        inner = ring;
        ring *= growth;
    }
    None
}

/// Two unit directions across the surface at `up` (any pair at right angles will do: the
/// search goes all the way round).
fn tangent_basis(up: DVec3) -> (DVec3, DVec3) {
    let pole = if up.y.abs() > 0.99 { DVec3::X } else { DVec3::Y };
    let east = pole.cross(up).normalize();
    let north = up.cross(east).normalize();
    (east, north)
}
