//! Sowing a seed from the backpack (2026-10-12, first-hour audit F11).
//!
//! The backpack's Plant button used to make a "loose" crop with no place in
//! the world (`tower_id` None): the renderer never drew it, nothing watered
//! it, the grow lights could not find it, and it died about ten game hours
//! after its last hand watering. At the 1x clock its lettuce also took 45 real
//! days, which nothing said.
//!
//! **Where a seed goes now: into the nearest free place of a grow machine
//! within reach of where the player stands**, the same places the Garden
//! page's Plant buttons fill: a tower cup, a bed, tray or field plot, a
//! mushroom rack's shelf. From there it is the same crop as theirs: drawn in
//! that cup or plot, kept watered by the home's irrigation, lit, fed and
//! harvested the same way, and listed in Inventory > Garden under its machine.
//!
//! WHY a machine and not the ground the player stands on: every place a crop
//! grows in this game is a grow machine. The drawing, the water, the light,
//! the soil memory, the pests and the weeds all find a crop by its machine
//! and its place in it (`CropInstance::tower_id` and `tower_slot`), and the
//! home is a station deck with no soil of its own. A crop in bare deck would
//! need all of that built a second time; a crop in a machine needs none of it.
//!
//! WHY within reach: a seed is sown by hand. Pressing Plant in the kitchen and
//! having it land in a bed three rooms away would be a teleport, and on a
//! shared server the nearest bed might be someone else's. Reach is measured
//! across the floor to the machine's near edge, and a machine on another
//! deck is out of reach. When nothing with room is in reach, nothing is
//! planted, the seed stays in the backpack, and the notice names the nearest
//! machine with room and how far it is. (The Garden page's buttons still
//! plant a whole tower or bed from anywhere: they are the home's own planting
//! for a machine the player picked by name.)
//!
//! WHAT may go where (`GrowMedium::takes`, data/garden/grow_media.ron): green
//! plants into beds, trays, fields and towers; a mushroom's spawn only onto a
//! mushroom rack; nothing into a fish tank.
//!
//! A place the renderer could not draw is never chosen: a tower must have a
//! design (its cups are drawn up that design's helix), and a crop's place
//! index is always below its machine's count of places. The tag a crop gets
//! is the one the renderer resolves: the design id for the first tower of
//! that design (the Garden page's tower button plants under it, and the
//! renderer dresses that tower with it), else the machine's own id.

use std::collections::{HashMap, HashSet};

use glam::Vec3;

use crate::ecs::components::{CropInstance, STAGE_DEAD};
use crate::hot_reload::data_store::DataStore;
use crate::systems::grow_machines::{GrowMedium, Takes};

use super::lighting::GrowPlot;
use super::outlook::{self, WaterSupply};
use super::{PlantDef, PlantRegistry};

/// How far a hand reaches to sow, metres across the floor from where the
/// player stands to a grow machine's near edge: the 5 m a walk-up machine is
/// used from (engine::built_uses `REACH_M`).
pub const PLANT_REACH_M: f32 = 5.0;

/// A grow machine whose base stands more than this above or below the
/// player's feet is on another deck, and out of reach.
pub const SAME_DECK_M: f32 = 2.0;

/// The kind of place a species is sown into: a green plant (it `needs_light`)
/// into beds, trays, fields and towers; a fungus onto a mushroom rack.
pub fn seed_takes(def: &PlantDef) -> Takes {
    if def.needs_light {
        Takes::Plants
    } else {
        Takes::Fungi
    }
}

/// One grow machine as sowing sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    /// The grow-area tag its crops carry (`CropInstance::tower_id`).
    pub tag: String,
    /// The other tag crops in it may carry, whose places count as taken too:
    /// the tower's own instance id when `tag` is its design id.
    pub also: Option<String>,
    /// What it is, in words: "aeroponic tower", "potato grow bed".
    pub name: String,
    /// What one place in it is called: "cup", "plot", "shelf".
    pub unit: &'static str,
    /// How many places it has: a tower's cups, a bed's plots.
    pub capacity: u32,
    pub takes: Takes,
    /// Where it stands, in the frame "camera_position" is in.
    pub pos: [f32; 3],
    /// Its footprint as a disc, metres: reach is measured to this edge.
    pub radius_m: f32,
}

/// Every grow machine a seed can be sown into, in the order the engine
/// published them ("grow_plots", with "grow_instances" naming each bed's
/// type and plots). A tower is a plot with cups; anything else must be a
/// machine the engine divided into plots. A tower's design (its alias) is
/// owned by the first tower listing it, as the grow lights read it.
pub fn places(plots: &[GrowPlot], instances: Option<&HashMap<String, Vec<(String, u32)>>>, media: &[GrowMedium]) -> Vec<Place> {
    let of_instance: HashMap<&str, (&str, u32)> = instances
        .into_iter()
        .flatten()
        .flat_map(|(ty, list)| list.iter().map(move |(id, n)| (id.as_str(), (ty.as_str(), *n))))
        .collect();
    let medium = |ty: &str| media.iter().find(|m| m.matches(ty));
    let mut owned: HashSet<&str> = HashSet::new();
    let mut out = Vec::new();
    for p in plots {
        let first_alias = p.aliases.iter().map(String::as_str).find(|a| !owned.contains(a));
        owned.extend(p.aliases.iter().map(String::as_str));
        let radius_m = p.footprint_m2.max(0.0).sqrt() * std::f32::consts::FRAC_1_SQRT_2;
        if p.cups > 0 {
            let design = p.aliases.first().map(|a| format!("aeroponic_tower_{a}"));
            let takes = design.as_deref().and_then(medium).map_or(Takes::Plants, |m| m.takes);
            let (tag, also) = match first_alias {
                Some(a) => (a.to_string(), Some(p.id.clone())),
                None => (p.id.clone(), None),
            };
            out.push(Place { tag, also, name: "aeroponic tower".to_string(), unit: "cup", capacity: p.cups, takes, pos: p.pos, radius_m });
            continue;
        }
        let Some(&(ty, n)) = of_instance.get(p.id.as_str()) else { continue };
        // A tower with no design has no cups the renderer can place a plant in.
        if ty.starts_with("aeroponic_tower_") {
            continue;
        }
        let Some(m) = medium(ty) else { continue };
        out.push(Place {
            tag: p.id.clone(),
            also: None,
            name: ty.replace('_', " "),
            unit: if m.stacked { "shelf" } else { "plot" },
            capacity: n.max(1),
            takes: m.takes,
            pos: p.pos,
            radius_m,
        });
    }
    out
}

/// Where a seed is sown: a place's tag (and its other tag), the place in it,
/// and how far it is from the player (None when where the player stands is
/// not known: a headless world).
#[derive(Debug, Clone, PartialEq)]
pub struct Spot {
    pub tag: String,
    pub also: Option<String>,
    pub slot: u32,
    pub capacity: u32,
    pub name: String,
    pub unit: &'static str,
    pub distance_m: Option<f32>,
}

/// Why nothing was sown.
#[derive(Debug, Clone, PartialEq)]
pub enum Refusal {
    /// No grow machine here takes this kind of seed.
    NoPlace(Takes),
    /// Every place that takes it already has a crop.
    Full(Takes),
    /// Places with room exist, none within reach: the nearest, how far its
    /// edge is across the floor, and whether it is on another deck.
    OutOfReach { kind: Takes, name: String, distance_m: f32, other_deck: bool },
}

/// Choose where a seed of kind `kind` goes. `taken` holds the (tag, place) of
/// every living crop; `feet` is where the player stands. The nearest machine
/// in reach with room wins (ties in published order) and its lowest free place
/// is used. Without `feet` the first machine with room is used.
pub fn choose(places: &[Place], taken: &HashSet<(String, u32)>, kind: Takes, feet: Option<Vec3>) -> Result<Spot, Refusal> {
    let fits: Vec<&Place> = places.iter().filter(|p| p.takes == kind).collect();
    if fits.is_empty() {
        return Err(Refusal::NoPlace(kind));
    }
    let is_taken = |tag: &str, s: u32| taken.contains(&(tag.to_string(), s));
    let free: Vec<(&Place, u32)> = fits
        .into_iter()
        .filter_map(|p| (0..p.capacity).find(|&s| !is_taken(&p.tag, s) && !p.also.as_deref().is_some_and(|a| is_taken(a, s))).map(|s| (p, s)))
        .collect();
    if free.is_empty() {
        return Err(Refusal::Full(kind));
    }
    let spot = |p: &Place, slot: u32, distance_m: Option<f32>| Spot {
        tag: p.tag.clone(),
        also: p.also.clone(),
        slot,
        capacity: p.capacity,
        name: p.name.clone(),
        unit: p.unit,
        distance_m,
    };
    let Some(feet) = feet else {
        let (p, s) = free[0];
        return Ok(spot(p, s, None));
    };
    // (on another deck, metres to the near edge across the floor) for each:
    // the same deck first, then the nearest, then the published order.
    let measured: Vec<(bool, f32, &Place, u32)> = free
        .iter()
        .map(|&(p, s)| {
            let across = Vec3::new(p.pos[0] - feet.x, 0.0, p.pos[2] - feet.z).length();
            let other_deck = (p.pos[1] - feet.y).abs() > SAME_DECK_M;
            (other_deck, (across - p.radius_m).max(0.0), p, s)
        })
        .collect();
    type Measured<'a> = (bool, f32, &'a Place, u32);
    let nearer = |a: &&Measured, b: &&Measured| a.0.cmp(&b.0).then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    if let Some(&(_, d, p, s)) = measured.iter().filter(|m| !m.0 && m.1 <= PLANT_REACH_M).min_by(nearer) {
        return Ok(spot(p, s, Some(d)));
    }
    let &(other_deck, distance_m, p, _) = measured.iter().min_by(nearer).expect("free is not empty");
    Err(Refusal::OutOfReach { kind, name: p.name.clone(), distance_m, other_deck })
}

/// How a refusal names a kind of seed: what is sown, where it goes, and how
/// "all of them are full" is said.
fn kind_words(k: Takes) -> (&'static str, &'static str, &'static str) {
    match k {
        Takes::Fungi => ("spawn", "mushroom rack", "every shelf of every mushroom rack is already growing"),
        _ => ("a seed", "bed, tray, field or tower", "every bed, tray, field and tower cup already has a plant"),
    }
}

/// The refusal as the player reads it.
pub fn refusal_words(r: &Refusal) -> String {
    let (what, places_word, full) = kind_words(match r {
        Refusal::NoPlace(k) | Refusal::Full(k) | Refusal::OutOfReach { kind: k, .. } => *k,
    });
    match r {
        Refusal::NoPlace(Takes::Fungi) => "Nothing planted: mushroom spawn grows on a mushroom rack, and there is none here.".to_string(),
        Refusal::NoPlace(_) => "Nothing planted: a seed is sown into a bed, tray, field or tower, and there is none here.".to_string(),
        Refusal::Full(_) => super::nothing_planted_notice(false, full),
        Refusal::OutOfReach { name, distance_m, other_deck, .. } => {
            let where_ = if *other_deck { "on another deck".to_string() } else { format!("{distance_m:.0} m away") };
            format!(
                "Nothing planted: you sow {what} where you stand, and no {places_word} with room is within {PLANT_REACH_M:.0} m of you. \
                 The nearest with room is the {name}, {where_}."
            )
        }
    }
}

/// How far a sown place is, for the notice: "beside you", "3 m from you".
fn near_words(distance_m: Option<f32>) -> String {
    match distance_m {
        Some(d) if d < 1.0 => " beside you".to_string(),
        Some(d) => format!(" {d:.0} m from you"),
        None => String::new(),
    }
}

/// Where the player stands: the home-local camera the game publishes every
/// frame, lowered by the standing eye height, else the controlled entity's
/// place (a headless world), else not known.
fn feet(world: &hecs::World, data: &DataStore) -> Option<Vec3> {
    if let Some(eye) = data.get::<Vec3>("camera_position") {
        return Some(*eye - Vec3::Y * crate::ecs::components::EYE_HEIGHT_M as f32);
    }
    world
        .query::<(&crate::ecs::components::Transform, &crate::ecs::components::Controllable)>()
        .iter()
        .next()
        .map(|(_, (t, _))| t.position)
}

/// The Plant button on a seed in the backpack ("plant_request"): choose the
/// place, take the seed (free in creative mode), sow it, and say where it
/// went, when it ripens and how it is watered. Or say why nothing was sown,
/// and keep the seed.
pub(super) fn sow_from_backpack(
    world: &mut hecs::World,
    data: &DataStore,
    seed_id: &str,
    plants: Option<&PlantRegistry>,
    creative: bool,
    supply: &WaterSupply,
) {
    let Some(def) = super::plant_id_from_seed(seed_id).and_then(|p| plants.and_then(|r| r.get(&p))) else {
        log::debug!("[Farming] no plant def for seed {seed_id}; not planted");
        super::push_notice(data, format!("Nothing planted: the garden has no growing data for {seed_id}."));
        return;
    };
    let media = crate::systems::grow_machines::load_grow_media(&crate::data_dir());
    let plots: &[GrowPlot] = data.get::<Vec<GrowPlot>>("grow_plots").map(Vec::as_slice).unwrap_or(&[]);
    let here = places(plots, data.get::<HashMap<String, Vec<(String, u32)>>>("grow_instances"), &media);
    let taken: HashSet<(String, u32)> = world
        .query::<&CropInstance>()
        .iter()
        .filter(|(_, c)| c.growth_stage != STAGE_DEAD)
        .filter_map(|(_, c)| c.tower_id.clone().zip(c.tower_slot))
        .collect();
    let spot = match choose(&here, &taken, seed_takes(def), feet(world, data)) {
        Ok(s) => s,
        Err(r) => {
            super::push_notice(data, refusal_words(&r));
            return;
        }
    };
    if !creative {
        let mut had = false;
        for (_e, (inv, _c)) in
            world.query_mut::<(&mut crate::systems::inventory::Inventory, &crate::ecs::components::Controllable)>()
        {
            if inv.has_item(seed_id, 1) {
                inv.remove_item(seed_id, 1);
                had = true;
            }
            break;
        }
        if !had {
            let name = data
                .get::<crate::systems::inventory::ItemRegistry>("item_registry")
                .and_then(|r| r.items.get(seed_id))
                .map_or(seed_id.to_string(), |d| d.name.to_lowercase());
            super::push_notice(data, format!("Nothing planted: your backpack holds no {name}."));
            return;
        }
    }
    // A dead crop in the place (under either tag) is cleared first, its soil
    // kept for the new one, the way the tower and bed buttons replant.
    for tag in std::iter::once(&spot.tag).chain(spot.also.iter()) {
        let dead: Vec<hecs::Entity> = world
            .query::<&CropInstance>()
            .iter()
            .filter(|(_, c)| c.tower_id.as_deref() == Some(tag.as_str()) && c.tower_slot == Some(spot.slot) && c.growth_stage == STAGE_DEAD)
            .map(|(e, _)| e)
            .collect();
        super::clear_dead_keeping_soil(world, dead, tag, spot.slot);
    }
    let (elapsed_s, time_scale) = outlook::clock(data);
    let crop = CropInstance {
        crop_def_id: def.id.clone(),
        growth_stage: def.first_stage().to_string(),
        planted_at: elapsed_s,
        water_level: 1.0,
        health: 100.0,
        tower_id: Some(spot.tag.clone()),
        tower_slot: Some(spot.slot),
        health_seconds: 0.0,
        growing_seconds: 0.0,
    };
    let said = outlook::outlook(&crop, Some(def), elapsed_s, time_scale, supply.holds(Some(spot.tag.as_str())), supply.water_available);
    super::spawn_in_remembered_soil(world, crop);
    log::info!("[Farming] planted {} (from {seed_id}) in {} {}", def.id, spot.tag, spot.slot);
    // The player planted it (2026-10-04, the opening's Plant step).
    crate::systems::quests::push_quest_event(data, crate::systems::quests::plant_event_key(&def.id));
    super::push_notice(data, planted_words(def, &spot, &said));
}

/// The notice for a seed sown: where, when it ripens, and its water.
pub fn planted_words(def: &PlantDef, spot: &Spot, said: &outlook::CropOutlook) -> String {
    format!(
        "Planted {} in the {}{}, {} {} of {}. It ripens in {}. {} Look after it in Inventory > Garden.",
        def.name.to_lowercase(),
        spot.name,
        near_words(spot.distance_m),
        spot.unit,
        spot.slot + 1,
        spot.capacity,
        said.ripens,
        said.water
    )
}
