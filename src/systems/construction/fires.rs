//! Built fires (BUG-153, 2026-10-05): a campfire you build burns its logs,
//! warms whoever stands near it, and goes out.
//!
//! WHAT IT REPLACED. The Campfire ability ("Build a campfire that provides
//! warmth light and slow healing") spent 15 energy and restored 3 health, and
//! nothing else: no fire was built, nothing warmed or lit, and its
//! `campfire_warmth` status effect was applied by no code. Now the ability
//! builds the `campfire` blueprint in front of the player
//! (`systems::abilities`) through the one path every built piece takes
//! (`construction::begin_build`), and a blueprint that `burns` is a fire:
//!
//! - It is LIT when it is finished, with the fuel it was built with: the
//!   campfire's three Wood Logs are its first load ([`lit_when_finished`]).
//! - It BURNS that fuel down on the game clock ([`burn`], run by the
//!   ConstructionSystem) and is OUT when the fuel is gone. E at it puts one
//!   more fuel item from the pack on it ([`add_fuel`]), up to what it holds.
//!   A log put on a fire that is out lights it again: A GAME CHOICE, since no
//!   tinder, spark or banked embers are modelled.
//! - While it burns it RADIATES heat in all directions, and the body heat
//!   model sees that as warmer surroundings for a person near it
//!   ([`warmth_at`], taken into the mean radiant temperature by
//!   `engine::survival_env` through `body_heat::radiant_with_source_c`). An
//!   out fire radiates nothing.
//! - It gives NO LIGHT yet. The renderer's point lights are not evaluated in
//!   the pass that draws a planet's ground and everything built on it (that
//!   pass's light count is 0 by design, `80-fragment-shared.wgsl`, v0.1155),
//!   and a campfire stands only there, so lighting it is renderer work, not a
//!   data entry (docs/BUGS.md, BUG-153).
//!
//! Which pieces burn is data (`Blueprint::burns` in data/blueprints/basic.ron),
//! so a fireplace or a wood stove is a data edit with its own numbers.
//!
//! THE CAMPFIRE'S NUMBERS (data/blueprints/basic.ron), and where each comes from:
//!
//! - HOW FAST IT BURNS. The US Forest Service's Missoula Fire Sciences
//!   Laboratory burned a campground fire ring (an "Accessible Campground Fire
//!   Ring", 32 inches across with an 8 inch heat shield) the way people use
//!   one, for the Minnesota Pollution Control Agency: Urbanski, "Recreational
//!   Outdoor Firepit Emissions Testing", 20 November 2021, printed in MPCA,
//!   "Minnesota Residential Wood Combustion Survey Results", December 2022,
//!   document aq-ei4-48; summarised in Urbanski, Lincoln, Baker, Nordgren and
//!   Jackson, "Recreational Fire Pit Emissions Testing", US EPA International
//!   Emissions Inventory Conference, Seattle, 28 September 2023. Its steady
//!   burn stage, "Typical recreational use": split oak, maple and birch added
//!   every 10 minutes, "Duration 55 min", "Fuel added 11 kg". That is 12 kg an
//!   hour, so one of the game's 8 kg Wood Logs (data/items.csv) lasts 40
//!   minutes: `seconds_per_fuel: 2400`.
//! - HOW MUCH HEAT IT RADIATES. That fire releases about 51 kW: 11 kg in 55
//!   minutes is 10.4 kg of dry wood an hour at the 15 percent moisture the
//!   Forest Service's typical burn assumes ("Split hardwood at MC = 15%"),
//!   times the 17.5 kJ per gram NIST measured as the effective heat of
//!   combustion of Douglas fir (Sung, Mueller, Bundy, Fernandez and Hamins,
//!   "Global Burning Properties of Little Bluestem, Excelsior and Douglas
//!   Fir", NIST Technical Note 2314, April 2025, Table 3, doi
//!   10.6028/NIST.TN.2314; excelsior, which is wood wool, was 16.9). The share
//!   of that radiated is the radiative fraction, which the same note measured:
//!   "For the three fuel types, the average radiative fraction was 0.32"
//!   (Douglas fir alone 0.29, falling from 0.41 to 0.15 as its moisture rose
//!   from 7 to 85 percent). 0.32 x 51 kW = 16 kW: `radiant_watts: 16000`.
//! - HOW IT FALLS OFF. The point source model: the radiant heat flux at a
//!   distance R is the radiated power over 4 pi R^2, the energy taken as
//!   "released at a point located at the center of the fire" (US NRC,
//!   NUREG-1805, Fire Dynamics Tools, 2004, chapter 5, equation 5-1, after
//!   Drysdale 1998). The centre is put at half the piece's height (0.3 m for
//!   the 0.6 m ring) and taken no nearer than [`NEAREST_M`]: close in the
//!   model is rough (NUREG-1805 again: it "overestimates the intensity of
//!   thermal radiation at the observer's (target) locations close to the
//!   fire").
//! - A CHECK AGAINST THE SAME TEST. Its radiometers sat 27 inches (0.69 m)
//!   from the ring's centre, 4 inches above the rim, and read 0.122 Btu per
//!   minute per square inch (3.3 kW/m2) through the steady burn with very dry
//!   wood and 0.024 (0.65 kW/m2) with wood at 10 to 20 percent moisture (the
//!   report's Table 2). 16 kW gives 2.7 kW/m2 there: inside that range,
//!   toward the dry end. Those radiometers looked just over the 8 inch
//!   shield, which hides the burning bed a person standing back from a ring
//!   looks down into.
//! - WHAT IT HOLDS: four logs at a time, A GAME CHOICE (about what a ring a
//!   metre across takes stacked).
//!
//! WHAT A PERSON FEELS. A standing person's middle, 1 m up and 1.5 m from the
//! ring's centre, is 1.65 m from its radiating centre and gets 465 W/m2 there,
//! about half the beam of a clear noon sun; at 20 m, 3 W/m2. What the body
//! absorbs is that times the projected area factor of a standing body for the
//! fire's angle below the level (Fanger 1970, `body_heat::projected_area_factor`),
//! with the fire's infrared absorbed as the long-wave the body trades with its
//! surroundings, and it is added to the mean radiant temperature as fourth
//! powers (`body_heat::radiant_with_source_c`). On a clear, calm 0 C night that
//! takes the surroundings a person feels 1.5 m from the fire from -11 C (the
//! open sky) to about 16 C, and 1 m from it (half a metre from the ring) to
//! 31 C; at 20 m it moves them 0.2 C. Standing in the everyday outfit through
//! that night, half a metre from the ring the core stays at 36.8 C and the
//! body shivers 3 W/m2, too little to show as Shivering; at 1.5 m it shivers
//! 24 W/m2 by morning; at 20 m, 56, as with no fire at all. The tests in
//! `engine::survival_env` hold what that does to a body over an hour and a
//! night.
//!
//! Everything here is pure data and math over the ECS, so it compiles in every
//! feature set (the relay build includes `systems/`).

use super::site::PlanetSite;
use super::{Blueprint, BlueprintRegistry, Structure};
use crate::ecs::components::{Controllable, Transform};
use crate::systems::inventory::{Inventory, ItemRegistry};
use glam::{DVec3, Vec3};
use serde::Deserialize;

/// What a burning piece burns and gives off (a blueprint's `burns`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Burn {
    /// The item that feeds it, one at a time (a data/items.csv id). The build's
    /// own items of this kind are its first load.
    pub fuel: String,
    /// Game seconds one fuel item burns.
    pub seconds_per_fuel: f32,
    /// The most fuel items it holds at once.
    pub holds: u32,
    /// Heat it radiates while it burns, watts, the same in every direction.
    pub radiant_watts: f32,
}

impl Burn {
    /// The most burning it holds, game seconds.
    pub fn full_s(&self) -> f32 {
        self.holds as f32 * self.seconds_per_fuel
    }
}

/// A built fire's fuel: the component on a finished `Structure` whose
/// blueprint `burns`. Saved with the piece (`persistence::ConstructionSave::fire_s`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FireFuel {
    /// Game seconds of burning left. 0 = out.
    pub seconds_left: f32,
}

impl FireFuel {
    /// Is it burning?
    pub fn burning(&self) -> bool {
        self.seconds_left > 0.0
    }
}

/// A standing person's middle, metres above their feet: where a fire's warmth
/// is taken (the body heat model's person is one standing body, and the
/// projected area factor is a standing body's). The shelter test's chest height.
pub const BODY_MIDDLE_M: f64 = 1.0;

/// Nearer to a fire's radiating centre than this, metres, its warmth is taken
/// at this distance: half the campfire's width, the edge of the ring. Inside
/// it a person is standing in the fire, which a built piece's solid box
/// (`engine::build_place::built_piece_segments`) does not allow anyway.
pub const NEAREST_M: f64 = 0.5;

/// The fuel a fire blueprint is lit with when it is finished: the fuel items
/// it was built with, burning from the start. None for a piece that does not
/// burn.
pub fn lit_when_finished(bp: &Blueprint) -> Option<FireFuel> {
    let burn = bp.burns.as_ref()?;
    let built_with: u32 = bp.materials.iter().filter(|(id, _)| *id == burn.fuel).map(|(_, n)| *n).sum();
    Some(FireFuel { seconds_left: built_with.min(burn.holds) as f32 * burn.seconds_per_fuel })
}

/// Burn every built fire's fuel down by `dt_s` game seconds. One that runs
/// out is out (0), and stays out until fuel is put on it.
pub fn burn(world: &mut hecs::World, dt_s: f32) {
    if dt_s <= 0.0 {
        return;
    }
    for (_e, fuel) in world.query_mut::<&mut FireFuel>() {
        fuel.seconds_left = (fuel.seconds_left - dt_s).max(0.0);
    }
}

/// A span of game seconds as people say it: "40 min", "1 h 20 min", "2 h".
pub fn span(seconds: f32) -> String {
    let minutes = (seconds.max(0.0) / 60.0).round() as u32;
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

/// The burn rule and display names for the fire `fire`: its blueprint's
/// `burns`, its own name and its fuel's name (the items' names where the
/// registry has them, else the ids).
fn fire_of(
    world: &hecs::World,
    registry: Option<&BlueprintRegistry>,
    items: Option<&ItemRegistry>,
    fire: hecs::Entity,
) -> Option<(Burn, String, String)> {
    let s = world.get::<&Structure>(fire).ok()?;
    let bp = registry?.get(&s.blueprint_id)?;
    let burn = bp.burns.clone()?;
    let fuel_name = items.and_then(|r| r.items.get(&burn.fuel)).map_or_else(|| burn.fuel.clone(), |d| d.name.clone());
    Some((burn, bp.name.clone(), fuel_name))
}

/// The crosshair line at a built fire: what E does, and how it is burning.
pub fn tend_prompt(
    world: &hecs::World,
    registry: Option<&BlueprintRegistry>,
    items: Option<&ItemRegistry>,
    fire: hecs::Entity,
) -> String {
    let Some((burn, name, fuel_name)) = fire_of(world, registry, items, fire) else {
        return String::new();
    };
    let left = world.get::<&FireFuel>(fire).map_or(0.0, |f| f.seconds_left);
    let state = if left <= 0.0 {
        format!("out: a {fuel_name} lights it again")
    } else if left + burn.seconds_per_fuel > burn.full_s() + 0.5 {
        format!("full, {} left", span(left))
    } else {
        format!("burning, {} left", span(left))
    };
    format!("[E] put a {fuel_name} on the {name} ({state})")
}

/// E at a built fire: take one of its fuel items from the player's pack and
/// put it on, up to what it holds. A fire that is out is lit again by it (A
/// GAME CHOICE: no tinder or spark is modelled). Returns the line for the
/// player either way; nothing is taken when it refuses.
pub fn add_fuel(
    world: &mut hecs::World,
    registry: Option<&BlueprintRegistry>,
    items: Option<&ItemRegistry>,
    fire: hecs::Entity,
) -> String {
    let Some((burn, name, fuel_name)) = fire_of(world, registry, items, fire) else {
        return "Nothing there to put fuel on".to_string();
    };
    let left = world.get::<&FireFuel>(fire).map_or(0.0, |f| f.seconds_left);
    if left + burn.seconds_per_fuel > burn.full_s() + 0.5 {
        return format!("The {name} is full: it holds {} at a time ({} of burning left)", burn.holds, span(left));
    }
    let player = world.query::<(&Inventory, &Controllable)>().iter().next().map(|(e, _)| e);
    // `remove_item` answers how many it could NOT take.
    let taken = player.is_some_and(|p| {
        world.get::<&mut Inventory>(p).is_ok_and(|mut inv| inv.count_item(&burn.fuel) > 0 && inv.remove_item(&burn.fuel, 1) == 0)
    });
    if !taken {
        return format!("You have no {fuel_name} in your pack to put on the {name}");
    }
    let now = left + burn.seconds_per_fuel;
    let _ = world.insert_one(fire, FireFuel { seconds_left: now });
    if left <= 0.0 {
        format!("Put a {fuel_name} on the {name} and lit it again: {} of burning", span(now))
    } else {
        format!("Put a {fuel_name} on the {name}: {} of burning left", span(now))
    }
}

/// What taking down a piece gives back: its materials, but for a fire only
/// the fuel it has not burned yet, in whole items (a log half burned is
/// coals and ash, not a log). The ring's stones all come back.
pub fn materials_back(bp: &Blueprint, fuel: Option<&FireFuel>) -> Vec<(String, u32)> {
    let Some(burn) = bp.burns.as_ref() else {
        return bp.materials.clone();
    };
    let mut back: Vec<(String, u32)> = bp.materials.iter().filter(|(id, _)| *id != burn.fuel).cloned().collect();
    let left = fuel.map_or(0.0, |f| f.seconds_left);
    let whole = (left / burn.seconds_per_fuel).floor() as u32;
    if whole > 0 {
        back.push((burn.fuel.clone(), whole));
    }
    back
}

/// The radiant heat flux, W/m2, at `distance_m` from a point radiating
/// `radiant_watts` the same in every direction: the power over the area of
/// the sphere at that distance (NUREG-1805 equation 5-1), taken no nearer
/// than [`NEAREST_M`].
pub fn irradiance_w_m2(radiant_watts: f64, distance_m: f64) -> f64 {
    let d = distance_m.max(NEAREST_M);
    radiant_watts / (4.0 * std::f64::consts::PI * d * d)
}

/// The heat a standing person absorbs from the burning fires on `body`, W per
/// square metre of their radiating area: for each fire, its irradiance at the
/// person's middle times the projected area factor of a standing body for the
/// fire's angle below the level (`body_heat::projected_area_factor`, symmetric
/// above and below). `middle` is the person's middle and `up` the local up,
/// both in the body's frame in f64 (CLAUDE.md, "f32 at planet scale"): a
/// fire's place comes from its build site the same way (`PlanetSite::to_body`),
/// so a fire in any site on the body counts by its true distance. An out
/// fire, a fire on another body, and a piece that does not burn give nothing.
pub fn warmth_at(world: &hecs::World, registry: Option<&BlueprintRegistry>, body: &str, middle: DVec3, up: DVec3) -> f64 {
    let up = up.normalize_or_zero();
    let mut absorbed = 0.0;
    for (_e, (s, tf, site, fuel)) in world.query::<(&Structure, &Transform, &PlanetSite, &FireFuel)>().iter() {
        if site.body != body || !fuel.burning() {
            continue;
        }
        let Some(burn) = registry.and_then(|r| r.get(&s.blueprint_id)).and_then(|bp| bp.burns.as_ref()) else {
            continue;
        };
        // The radiating centre: half the piece's height above its base (a
        // piece's box is bottom-origin, `uses::ray_hits_box`).
        let source = site.to_body(tf.position + Vec3::Y * (tf.scale.y * 0.5));
        let to_fire = source - middle;
        let d = to_fire.length();
        let below_deg = if d > 0.0 { (to_fire.dot(up) / d).clamp(-1.0, 1.0).asin().to_degrees().abs() } else { 90.0 };
        absorbed += crate::systems::body_heat::projected_area_factor(below_deg) * irradiance_w_m2(f64::from(burn.radiant_watts), d);
    }
    absorbed
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Quat;

    const LOG: &str = "wood_log_0";
    const STONE: &str = "stone_raw_0";

    fn shipped() -> BlueprintRegistry {
        BlueprintRegistry::from_ron(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/blueprints/basic.ron"))).unwrap()
    }

    fn items() -> ItemRegistry {
        ItemRegistry::from_csv(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/items.csv"))).unwrap()
    }

    fn earth_site() -> PlanetSite {
        PlanetSite { body: "earth".into(), origin: DVec3::new(0.0, 6_371_000.0, 0.0) }
    }

    /// A finished campfire at the site's origin with `seconds_left` of fuel.
    fn campfire(world: &mut hecs::World, reg: &BlueprintRegistry, seconds_left: f32) -> hecs::Entity {
        let bp = reg.get("campfire").expect("campfire in basic.ron");
        world.spawn((
            Transform { position: Vec3::ZERO, rotation: Quat::IDENTITY, scale: Vec3::from_array(bp.size) },
            Structure { blueprint_id: "campfire".into(), health: bp.health, max_health: bp.health, provides: bp.provides.clone(), uid: 1 },
            earth_site(),
            FireFuel { seconds_left },
        ))
    }

    /// THE CAMPFIRE'S NUMBERS ARE THE SOURCED ONES (the module doc). The data
    /// must agree with the derivation written there, so a change to either
    /// (or to the Wood Log's mass in items.csv) is caught here: a log burns as
    /// long as 8 kg takes at the Forest Service ring's 11 kg in 55 minutes,
    /// and the ring radiates NIST's 0.32 of what that wood releases (dry at 15
    /// percent moisture, 17.5 MJ per kg). It is built from 6 Raw Stone and 3
    /// Wood Logs, outdoors only.
    #[test]
    fn the_campfire_numbers_are_the_sourced_ones() {
        let reg = shipped();
        let bp = reg.get("campfire").expect("campfire in basic.ron");
        let burn = bp.burns.as_ref().expect("a campfire burns");
        assert!(bp.outdoors_only, "a campfire is built outdoors");
        assert_eq!(burn.fuel, LOG);
        let mut materials = bp.materials.clone();
        materials.sort();
        assert_eq!(materials, vec![(STONE.to_string(), 6), (LOG.to_string(), 3)]);
        // Forest Service steady burn: 11 kg as weighed in 55 minutes.
        let kg_per_s = 11.0 / (55.0 * 60.0);
        let log_kg = f64::from(items().items.get(LOG).expect("Wood Log in items.csv").mass_kg);
        let log_s = log_kg / kg_per_s;
        assert!((f64::from(burn.seconds_per_fuel) - log_s).abs() < 1.0, "a {log_kg} kg log burns {log_s:.0} s, data says {}", burn.seconds_per_fuel);
        // NIST TN 2314: 17.5 MJ/kg of dry wood, radiative fraction 0.32; the
        // wood dry at 15 percent moisture (dry basis).
        let radiant_w = 0.32 * (kg_per_s / 1.15) * 17.5e6;
        assert!((f64::from(burn.radiant_watts) / radiant_w - 1.0).abs() < 0.03, "{radiant_w:.0} W from the sources, data says {}", burn.radiant_watts);
    }

    /// A CAMPFIRE BURNS ITS LOGS AND TAKES MORE (BUG-153). Finished, it is lit
    /// with the three logs it was built with (2 h); E puts a fourth from the
    /// pack on, a fifth is refused (it holds four) and stays in the pack;
    /// burnt down it is out; a log lights it again; with no log in the pack E
    /// changes nothing. Taking it down gives back its stones and only the
    /// whole logs it has not burned. Red check, run: `burn` leaving the fuel
    /// alone keeps the fire burning after 2 h 40 min and the "out" assertion
    /// fails.
    #[test]
    fn a_campfire_burns_its_logs_and_takes_more() {
        let (reg, items) = (shipped(), items());
        let bp = reg.get("campfire").unwrap();
        let lit = lit_when_finished(bp).expect("a campfire is lit when finished");
        assert_eq!(lit.seconds_left, 3.0 * 2400.0, "its three logs, 40 minutes each");
        assert!(lit_when_finished(reg.get("wood_wall").unwrap()).is_none(), "a wall does not burn");

        let mut world = hecs::World::new();
        let mut pack = Inventory::new(16);
        pack.add_item(LOG, 2, 99);
        let player = world.spawn((pack, Controllable));
        let fire = campfire(&mut world, &reg, lit.seconds_left);
        let logs = |w: &hecs::World| w.get::<&Inventory>(player).unwrap().count_item(LOG);
        let left = |w: &hecs::World| w.get::<&FireFuel>(fire).unwrap().seconds_left;
        assert_eq!(tend_prompt(&world, Some(&reg), Some(&items), fire), "[E] put a Wood Log on the Campfire (burning, 2 h left)");

        let msg = add_fuel(&mut world, Some(&reg), Some(&items), fire);
        assert_eq!((left(&world), logs(&world)), (4.0 * 2400.0, 1), "{msg}");
        assert!(tend_prompt(&world, Some(&reg), Some(&items), fire).contains("full, 2 h 40 min left"));
        let msg = add_fuel(&mut world, Some(&reg), Some(&items), fire);
        assert!(msg.contains("full"), "{msg}");
        assert_eq!((left(&world), logs(&world)), (4.0 * 2400.0, 1), "a fifth log is refused and kept");

        burn(&mut world, 4.0 * 2400.0 + 1.0);
        assert_eq!(left(&world), 0.0, "burnt down, out");
        assert!(!world.get::<&FireFuel>(fire).unwrap().burning());
        assert!(tend_prompt(&world, Some(&reg), Some(&items), fire).contains("out: a Wood Log lights it again"));
        let msg = add_fuel(&mut world, Some(&reg), Some(&items), fire);
        assert!(msg.contains("lit it again"), "{msg}");
        assert_eq!((left(&world), logs(&world)), (2400.0, 0));
        let msg = add_fuel(&mut world, Some(&reg), Some(&items), fire);
        assert!(msg.contains("no Wood Log"), "{msg}");
        assert_eq!(left(&world), 2400.0, "nothing put on without a log");

        let back = |s: f32| {
            let mut b = materials_back(bp, Some(&FireFuel { seconds_left: s }));
            b.sort();
            b
        };
        assert_eq!(back(2400.0), vec![(STONE.to_string(), 6), (LOG.to_string(), 1)], "one whole log left");
        assert_eq!(back(2399.0), vec![(STONE.to_string(), 6)], "a log half burnt is not a log");
        assert_eq!(materials_back(reg.get("wood_wall").unwrap(), None), reg.get("wood_wall").unwrap().materials, "a wall gives back all of it");
        assert_eq!(span(4800.0), "1 h 20 min");
        assert_eq!(span(2400.0), "40 min");
        assert_eq!(span(7200.0), "2 h");
    }

    /// A FIRE'S WARMTH FALLS WITH THE SQUARE OF THE DISTANCE AND AN OUT FIRE
    /// GIVES NONE (BUG-153). For a standing person's middle on Earth: 1.5 m
    /// from a burning campfire's centre, over 100 W/m2 absorbed (a warmth
    /// the body heat model feels, `engine::survival_env`'s tests); at 3 m
    /// under a third of that; at 20 m under 1.5 W/m2. The same fire out
    /// gives nothing, and so does a burning fire on another body. Red check,
    /// run: dropping the `burning()` filter from `warmth_at` warms the person
    /// beside the out fire and the out-fire assertion fails.
    #[test]
    fn a_fires_warmth_falls_off_with_distance_and_an_out_fire_gives_none() {
        let reg = shipped();
        let site = earth_site();
        let up = site.origin.normalize();
        let middle_at = |x: f32| site.to_body(Vec3::new(x, BODY_MIDDLE_M as f32, 0.0));
        let mut world = hecs::World::new();
        let fire = campfire(&mut world, &reg, 2400.0);
        let w = |world: &hecs::World, x: f32| warmth_at(world, Some(&reg), "earth", middle_at(x), up);
        let (near, mid, far) = (w(&world, 1.5), w(&world, 3.0), w(&world, 20.0));
        assert!(near > 100.0, "1.5 m: {near:.1} W/m2");
        assert!(mid < near / 3.0, "3 m: {mid:.1} against {near:.1} at 1.5 m");
        assert!(far < 1.5, "20 m: {far:.2} W/m2");
        assert_eq!(warmth_at(&world, Some(&reg), "moon", middle_at(1.5), up), 0.0, "a fire on another body");

        world.get::<&mut FireFuel>(fire).unwrap().seconds_left = 0.0;
        assert_eq!(w(&world, 1.5), 0.0, "an out fire gives no heat");
    }
}
