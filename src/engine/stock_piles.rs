//! Visible storage (2026-09-26; the gameplay arc's step 4). The operator,
//! asking for it: "In the game Starmade they had this cool feature where a
//! designated area would fill up with cargo as the ship inventory was
//! filled." Here the home's stored goods (the organize-layer pool) are drawn
//! as crates on the decks of the home's STORAGE racks (the Barn's pallet
//! shelving, `ZoneFiller` mesh_kind "rack"), bottom decks first, so the barn
//! fills as the stock grows and empties as it is used; a storage zone with no
//! racks gets floor stacks instead. And every water tank
//! carries a level bar above it, so a cistern running dry is visible from
//! across the room.
//!
//! Placeholder geometry by design (one box mesh, two flat materials), the
//! same build-once pattern as the parked vehicles; real crate and tank models
//! can replace the boxes without touching the layout, which is the part
//! with rules and tests.

use glam::{Quat, Vec3};

use crate::gui::PlacedItem;
use crate::renderer::RenderObject;

/// One storage crate: 0.6 x 0.4 x 0.4 m, 96 L (a common plastic tote size).
pub const CRATE_SIZE: Vec3 = Vec3::new(0.6, 0.4, 0.4);
pub const CRATE_VOLUME_L: f32 = 96.0;
/// A stored item whose volume the item registry does not know (the places
/// spine still holds some free-text labels) counts as this much.
pub const UNKNOWN_ITEM_VOLUME_L: f32 = 5.0;
/// Clear margin kept from the zone's walls, metres.
const INSET_M: f32 = 0.8;
/// Gap between neighbouring crates, metres.
const GAP_M: f32 = 0.06;
/// An aisle is left after every this many crate columns, so the pile reads
/// as racks you could walk between rather than a solid block.
const AISLE_EVERY: u32 = 4;
const AISLE_M: f32 = 1.2;
/// Crates are stacked at most this high.
const MAX_LAYERS: u32 = 4;
/// Height of a tank's level gauge plate, metres.
const GAUGE_H_M: f32 = 0.3;

/// Stock is drawn by kind (2026-09-26): dry goods in sacks, liquids and
/// perishable food in barrels, everything else in crates. Index into the
/// arrays below; the order is also the order they fill the racks in.
pub const CRATE: usize = 0;
pub const SACK: usize = 1;
pub const BARREL: usize = 2;
/// A grain or flour sack: about 40 L (a 25 kg feed sack), lying flat.
pub const SACK_SIZE: Vec3 = Vec3::new(0.58, 0.26, 0.38);
/// A small cask: about 60 L, 0.40 m across, 0.46 m tall.
pub const BARREL_SIZE: Vec3 = Vec3::new(0.40, 0.46, 0.40);
/// Litres one unit of each kind holds: crate, sack, barrel.
pub const UNIT_VOLUME_L: [f32; 3] = [CRATE_VOLUME_L, 40.0, 60.0];

/// Which kind of container an item's content class is stored in.
pub fn kind_for_class(class: &str) -> usize {
    match class {
        "dry_goods" => SACK,
        "water" | "liquid" | "flammable" | "food" => BARREL,
        _ => CRATE,
    }
}

/// The litres of stock of each kind (crate, sack, barrel) in `pool`.
pub fn stored_volumes(
    pool: &[PlacedItem],
    volume_of: impl Fn(&str) -> f32,
    class_of: impl Fn(&str) -> String,
) -> [f32; 3] {
    let mut out = [0.0_f32; 3];
    for p in pool {
        let v = volume_of(&p.key);
        let v = if v > 0.0 { v } else { UNKNOWN_ITEM_VOLUME_L };
        out[kind_for_class(&class_of(&p.key))] += v * p.qty as f32;
    }
    out
}

/// How many units of each kind those volumes fill.
pub fn unit_counts(volumes: [f32; 3]) -> [u32; 3] {
    let mut out = [0u32; 3];
    for k in 0..3 {
        out[k] = if volumes[k] <= 0.0 { 0 } else { (volumes[k] / UNIT_VOLUME_L[k]).ceil() as u32 };
    }
    out
}

/// Deal the kinds onto the slots in order (crates, then sacks, then
/// barrels), so each kind stands together on the racks.
pub fn assign_kinds(slots: Vec<Vec3>, counts: [u32; 3]) -> Vec<(Vec3, usize)> {
    let mut kinds = Vec::new();
    for k in 0..3 {
        kinds.extend(std::iter::repeat(k).take(counts[k] as usize));
    }
    slots.into_iter().zip(kinds).collect()
}

/// Crate BASE CENTRES (the unit box mesh is floor-anchored) for `count` crates across `zones` (each a (min corner,
/// size) box in world metres), filling one floor spot to its full stack
/// height before the next, from the back wall forward, zone by zone. Stops
/// when every zone is full, so a stock bigger than the barn caps at a full
/// barn instead of spilling through the walls.
pub fn pile_positions(zones: &[(Vec3, Vec3)], count: u32) -> Vec<Vec3> {
    let mut out = Vec::new();
    let step_x = CRATE_SIZE.x + GAP_M;
    let step_z = CRATE_SIZE.z + GAP_M;
    for &(origin, size) in zones {
        let usable_w = size.x - 2.0 * INSET_M;
        let usable_d = size.z - 2.0 * INSET_M;
        if usable_w < CRATE_SIZE.x || usable_d < CRATE_SIZE.z {
            continue;
        }
        let layers = ((size.y / CRATE_SIZE.y).floor() as u32).clamp(1, MAX_LAYERS);
        // Column x positions, with an aisle after every AISLE_EVERY columns.
        let mut cols = Vec::new();
        let mut x = 0.0_f32;
        let mut in_group = 0;
        while x + CRATE_SIZE.x <= usable_w {
            cols.push(x);
            in_group += 1;
            x += step_x;
            if in_group == AISLE_EVERY {
                in_group = 0;
                x += AISLE_M;
            }
        }
        let rows = ((usable_d + GAP_M) / step_z).floor() as u32;
        for r in 0..rows {
            for &cx in &cols {
                for layer in 0..layers {
                    if out.len() as u32 >= count {
                        return out;
                    }
                    out.push(Vec3::new(
                        origin.x + INSET_M + cx + CRATE_SIZE.x * 0.5,
                        origin.y + CRATE_SIZE.y * layer as f32,
                        origin.z + INSET_M + r as f32 * step_z + CRATE_SIZE.z * 0.5,
                    ));
                }
            }
        }
    }
    out
}

/// Crate BASE CENTRES on the decks of the racks a rack filler builds in one
/// zone (min corner `origin`, extent `size`): the same cells and deck levels
/// the mesh bake uses, lowest deck of every rack first, then the next deck
/// up, each deck filled from its back edge forward.
pub fn rack_slots(origin: Vec3, size: Vec3, filler: &crate::ship::structure::ZoneFiller) -> Vec<Vec3> {
    use crate::ship::structure::{rack_deck_levels, RACK_DECK_M, RACK_POST_M};
    let (fw, fd) = filler.footprint;
    let cells = filler.cells(origin.x, origin.z, size.x, size.z);
    let levels = rack_deck_levels(filler.built_height(size.y));
    let step_x = CRATE_SIZE.x + GAP_M;
    let step_z = CRATE_SIZE.z + GAP_M;
    let inner_w = fw - 2.0 * RACK_POST_M;
    let inner_d = fd - 2.0 * RACK_POST_M;
    let nx = ((inner_w + GAP_M) / step_x).floor().max(0.0) as u32;
    let nz = ((inner_d + GAP_M) / step_z).floor().max(0.0) as u32;
    // Centre the crate block on the deck.
    let pad_x = (inner_w - (nx as f32 * step_x - GAP_M)) * 0.5;
    let pad_z = (inner_d - (nz as f32 * step_z - GAP_M)) * 0.5;
    let mut out = Vec::new();
    for level in levels {
        for &(cx, cz) in &cells {
            for iz in 0..nz {
                for ix in 0..nx {
                    out.push(Vec3::new(
                        cx + RACK_POST_M + pad_x + ix as f32 * step_x + CRATE_SIZE.x * 0.5,
                        origin.y + level + RACK_DECK_M,
                        cz + RACK_POST_M + pad_z + iz as f32 * step_z + CRATE_SIZE.z * 0.5,
                    ));
                }
            }
        }
    }
    out
}

/// Labels of the home's storage zones ("Barn"): a place in the organize tree
/// with the same label is where that zone's stock is filed.
fn storage_room_labels(state: &crate::engine::state::EngineState) -> Vec<String> {
    let Some(ship) = state.gui_state.ship_structure.as_ref() else {
        return Vec::new();
    };
    let Some(home) = ship.zones.get(ship.home_zone_index()) else {
        return Vec::new();
    };
    home.body
        .zones
        .iter()
        .filter(|z| z.type_id == "storage" && !z.label.is_empty())
        .map(|z| z.label.clone())
        .collect()
}

/// True when an organize-pool container PATH ("1/1", the index scheme the
/// inventory renderer walks) passes through a place labelled as one of
/// `rooms`, or is the loose-in-the-home fallback "Home".
pub fn in_storage_room(places: &[crate::gui::Place], path: &str, rooms: &[String]) -> bool {
    if path == "Home" {
        return true;
    }
    let mut idx = path.split('/').map(|s| s.parse::<usize>().ok());
    let Some(Some(first)) = idx.next() else { return false };
    let Some(mut node) = places.get(first) else { return false };
    loop {
        if rooms.iter().any(|r| r == &node.label) {
            return true;
        }
        match idx.next() {
            Some(Some(j)) => match node.children.get(j) {
                Some(c) => node = c,
                None => return false,
            },
            _ => return false,
        }
    }
}

/// File what automated machines made this tick (the "home_stock_outputs"
/// channel, 2026-09-26) into home storage: the organize-pool place labelled
/// as a storage zone (the Barn), else loose in the home. Merges into an
/// unworn entry of the same item there. The Barn's crates follow.
pub fn receive_machine_outputs(state: &mut crate::engine::state::EngineState) {
    let made: Vec<(String, u32)> = state
        .data_store
        .get::<std::sync::Mutex<Vec<(String, u32)>>>("home_stock_outputs")
        .and_then(|m| m.lock().ok().map(|mut v| std::mem::take(&mut *v)))
        .unwrap_or_default();
    if made.is_empty() {
        return;
    }
    let rooms = storage_room_labels(state);
    let container = store_path(&state.gui_state.places, &rooms).unwrap_or_else(|| "Home".to_string());
    let reg = state.data_store.get::<crate::systems::inventory::ItemRegistry>("item_registry");
    // A machine turns out standard goods (2026-09-26, crafting::quality).
    let standard = state
        .data_store
        .get::<crate::systems::crafting::quality::QualityLevels>("quality_levels")
        .map_or(0, |l| l.standard());
    for (id, qty) in made {
        let quality = if reg.map_or(false, |r| r.durability_for(&id) > 0) { standard } else { 0 };
        let pool = &mut state.gui_state.placed_items;
        if let Some(p) = pool
            .iter_mut()
            .find(|p| p.key == id && p.container == container && p.wear == 0 && p.quality == quality)
        {
            p.qty += qty;
        } else {
            let name = reg.and_then(|r| r.items.get(&id).map(|d| d.name.clone())).unwrap_or_else(|| id.clone());
            pool.push(PlacedItem { key: id, name, qty, container: container.clone(), wear: 0, quality });
        }
    }
}

/// The organize-pool container path of the first place labelled as one of
/// `rooms` (depth-first, the renderer's index scheme), e.g. the Barn.
pub fn store_path(places: &[crate::gui::Place], rooms: &[String]) -> Option<String> {
    fn walk(p: &crate::gui::Place, path: String, rooms: &[String]) -> Option<String> {
        if rooms.iter().any(|r| r == &p.label) {
            return Some(path);
        }
        for (j, c) in p.children.iter().enumerate() {
            if c.kind != "item" {
                if let Some(found) = walk(c, format!("{path}/{j}"), rooms) {
                    return Some(found);
                }
            }
        }
        None
    }
    places.iter().enumerate().find_map(|(i, p)| walk(p, i.to_string(), rooms))
}

/// The home's storage zones in world metres, with their crate slots: rack
/// decks where the zone's filler builds racks, floor stacks otherwise.
fn home_storage_slots(state: &crate::engine::state::EngineState, count: u32) -> Vec<Vec3> {
    let Some(ship) = state.gui_state.ship_structure.as_ref() else {
        return Vec::new();
    };
    let Some(home) = ship.zones.get(ship.home_zone_index()) else {
        return Vec::new();
    };
    let base = home.origin_vec();
    let mut out = Vec::new();
    for z in home.body.zones.iter().filter(|z| z.type_id == "storage") {
        let origin = base + Vec3::new(z.origin.0, z.origin.1, z.origin.2);
        let size = Vec3::new(z.size.0, z.size.1, z.size.2);
        let left = count.saturating_sub(out.len() as u32);
        if left == 0 {
            break;
        }
        match crate::ship::structure::zone_filler(&z.type_id).filter(|f| f.mesh_kind == "rack") {
            Some(f) => out.extend(rack_slots(origin, size, f).into_iter().take(left as usize)),
            None => out.extend(pile_positions(&[(origin, size)], left)),
        }
    }
    out
}

/// Push the crates and tank level bars into this frame's render list.
pub fn push_render_objects(state: &mut crate::engine::state::EngineState, out: &mut Vec<RenderObject>) {
    use crate::renderer::mesh::Mesh;
    if state.stock_pile_mesh.is_none() {
        let unit_box = state.renderer.add_mesh(Mesh::box_xyz(&state.renderer.device, 1.0, 1.0, 1.0));
        let unit_barrel = state.renderer.add_mesh(Mesh::cylinder_capped(&state.renderer.device, 0.5, 1.0, 16));
        state.stock_pile_mesh = Some([unit_box, unit_barrel]);
    }
    if state.stock_pile_mats.is_none() {
        // theme-exempt: placeholder crate wood, gauge back, water blue, sack burlap, cask oak; world props, not UI colours.
        let wood = state.renderer.add_material_typed([0.55, 0.42, 0.26, 1.0], 0.05, 0.8, 0.0);
        let back = state.renderer.add_material_typed([0.08, 0.08, 0.09, 1.0], 0.0, 0.9, 0.0);
        // The water level glows a little so the gauge reads in a dim plant room.
        let water = state.renderer.add_material_full([0.20, 0.45, 0.85, 1.0], 0.1, 0.4, 0.0, 0.8);
        // theme-exempt: burlap and cask colours for the sacks and barrels.
        let burlap = state.renderer.add_material_typed([0.72, 0.62, 0.44, 1.0], 0.0, 0.95, 0.0);
        let cask = state.renderer.add_material_typed([0.40, 0.26, 0.15, 1.0], 0.1, 0.7, 0.0);
        // theme-exempt: the lit amber band of a vessel's level gauge.
        let goods = state.renderer.add_material_full([0.90, 0.62, 0.20, 1.0], 0.1, 0.4, 0.0, 0.8);
        state.stock_pile_mats = Some([wood, back, water, burlap, cask, goods]);
    }
    let [mesh, barrel_mesh] = state.stock_pile_mesh.unwrap();
    let [wood, back, water, burlap, cask, goods] = state.stock_pile_mats.unwrap();

    // Crates: what is filed in a storage room (the Barn) or loose in the
    // home, recomputed only when that volume changes. Things in named bags,
    // the garage or a car trunk are elsewhere and are not drawn here.
    let rooms = storage_room_labels(state);
    if rooms.is_empty() {
        return; // the world (and its storage zones) has not loaded yet
    }
    let volumes = {
        let reg = state
            .data_store
            .get::<crate::systems::inventory::ItemRegistry>("item_registry");
        let stock: Vec<PlacedItem> = state
            .gui_state
            .placed_items
            .iter()
            .filter(|p| in_storage_room(&state.gui_state.places, &p.container, &rooms))
            .cloned()
            .collect();
        stored_volumes(
            &stock,
            |id| reg.map(|r| r.volume_for(id)).unwrap_or(0.0),
            |id| reg.map(|r| r.class_for(id).to_string()).unwrap_or_default(),
        )
    };
    let changed = (0..3).any(|k| (volumes[k] - state.stock_pile_cache.0[k]).abs() > 0.01);
    if changed {
        let counts = unit_counts(volumes);
        let want: u32 = counts.iter().sum();
        let slots = home_storage_slots(state, want);
        log::info!(
            "Stock piles: {:.0} L stored = {} crates, {} sacks, {} barrels; {} shown in the storage zones",
            volumes.iter().sum::<f32>(),
            counts[CRATE],
            counts[SACK],
            counts[BARREL],
            slots.len()
        );
        state.stock_pile_cache = (volumes, assign_kinds(slots, counts));
    }
    for (p, kind) in &state.stock_pile_cache.1 {
        let (scale, mesh, material) = match *kind {
            SACK => (SACK_SIZE, mesh, burlap),
            BARREL => (BARREL_SIZE, barrel_mesh, cask),
            _ => (CRATE_SIZE, mesh, wood),
        };
        out.push(RenderObject { fade: 0.0, position: *p, rotation: Quat::IDENTITY, scale, mesh, material });
    }

    // Level gauges: an upright dark plate above each water tank and each
    // vessel machine (grain silo, fuel drum, dairy tank; 2026-09-26), turned
    // with it, with the level as a lit band that grows from its left end:
    // blue for water, amber for what a vessel holds. As long as the machine
    // is wide (at least 1 m).
    let heights: std::collections::HashMap<String, (f32, f32, bool)> = state
        .gui_state
        .home_machines
        .as_ref()
        .map(|h| {
            h.all_instances()
                .into_iter()
                .filter_map(|i| {
                    // size is (w, h, d) for a box, (radius, h, _) for a
                    // cylinder, (radius, _, _) for a sphere.
                    h.catalog.get(&i.machine).map(|d| {
                        let hw = match d.shape.as_str() {
                            "cylinder" => (d.size.1, d.size.0 * 2.0),
                            "sphere" => (d.size.0 * 2.0, d.size.0 * 2.0),
                            _ => (d.size.1, d.size.0),
                        };
                        (i.id, (hw.0, hw.1, d.level_gauge))
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let gauges: Vec<(String, crate::ecs::components::Transform, f32, bool)> = {
        use crate::ecs::components::{MachineInstanceId, Transform, WaterTank};
        use crate::systems::inventory::containers::Container;
        let w = &state.game_world.world;
        let level = |have: f32, cap: f32| if cap > 0.0 { (have / cap).clamp(0.0, 1.0) } else { 0.0 };
        let mut v: Vec<(String, Transform, f32, bool)> = w
            .query::<(&WaterTank, &Transform, &MachineInstanceId)>()
            .iter()
            .map(|(_e, (t, tf, id))| (id.0.clone(), tf.clone(), level(t.liters, t.capacity_l), true))
            .collect();
        // Bulk vessels only: machines marked level_gauge (the silo, the fuel
        // drums). Shelves, drawers and cabinets hold goods and get no gauge.
        v.extend(
            w.query::<(&Container, &Transform, &MachineInstanceId)>()
                .iter()
                .filter(|(_e, (_, _, id))| heights.get(&id.0).map_or(false, |x| x.2))
                .map(|(_e, (c, tf, id))| (id.0.clone(), tf.clone(), level(c.used_liters, c.capacity_liters), false)),
        );
        v
    };
    for (id, tf, frac, is_water) in gauges {
        let (h, w, _) = heights.get(&id).copied().unwrap_or((2.0, 1.0, false));
        let len = w.max(1.0);
        let top = tf.position + Vec3::new(0.0, h + 0.3, 0.0);
        // Two crossed plates, so the level reads from any side (one plate is
        // a thin line seen end-on).
        for turn in [0.0_f32, std::f32::consts::FRAC_PI_2] {
            let rot = tf.rotation * Quat::from_rotation_y(turn);
            out.push(RenderObject {
                fade: 0.0,
                position: top,
                rotation: rot,
                scale: Vec3::new(len + 0.06, GAUGE_H_M, 0.04),
                mesh,
                material: back,
            });
            if frac > 0.0 {
                // Proud of the plate on both faces, so it reads from either side.
                let fill_len = len * frac;
                out.push(RenderObject {
                    fade: 0.0,
                    position: top + rot * Vec3::new(-(len - fill_len) * 0.5, 0.03, 0.0),
                    rotation: rot,
                    scale: Vec3::new(fill_len, GAUGE_H_M - 0.06, 0.07),
                    mesh,
                    material: if is_water { water } else { goods },
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(key: &str, qty: u32) -> PlacedItem {
        PlacedItem { key: key.into(), name: key.into(), qty, container: "Home".into(), wear: 0, quality: 0 }
    }

    #[test]
    fn stock_volume_uses_item_volumes_and_a_default_for_labels() {
        let pool = vec![item("wood_plank_0", 10), item("a free-text label", 3)];
        let v = stored_volumes(&pool, |id| if id == "wood_plank_0" { 4.0 } else { 0.0 }, |_| "solid".into());
        assert!((v[CRATE] - (40.0 + 3.0 * UNKNOWN_ITEM_VOLUME_L)).abs() < 1e-4, "{v:?}");
        assert_eq!(unit_counts([0.0, 0.0, 0.0]), [0, 0, 0]);
        assert_eq!(unit_counts([1.0, 1.0, 1.0]), [1, 1, 1]);
        assert_eq!(unit_counts([CRATE_VOLUME_L * 3.0, 0.0, 0.0])[CRATE], 3);
    }

    /// Stock is drawn by kind (2026-09-26): grain and flour in sacks, milk
    /// and water in barrels, tools and lumber in crates, each kind together.
    #[test]
    fn dry_goods_go_in_sacks_liquids_in_barrels_the_rest_in_crates() {
        assert_eq!(kind_for_class("dry_goods"), SACK);
        assert_eq!(kind_for_class("water"), BARREL);
        assert_eq!(kind_for_class("food"), BARREL);
        assert_eq!(kind_for_class("solid"), CRATE);
        let pool = vec![item("grain_wheat_0", 100), item("milk_0", 30), item("wood_plank_0", 12)];
        let class = |id: &str| match id {
            "grain_wheat_0" => "dry_goods".to_string(),
            "milk_0" => "food".to_string(),
            _ => "solid".to_string(),
        };
        let v = stored_volumes(&pool, |id| if id == "wood_plank_0" { 8.0 } else { 1.0 }, class);
        assert_eq!(v, [96.0, 100.0, 30.0]);
        let counts = unit_counts(v);
        assert_eq!(counts, [1, 3, 1], "one crate of planks, three sacks of grain, one barrel of milk");
        let slots: Vec<Vec3> = (0..5).map(|i| Vec3::new(i as f32, 0.0, 0.0)).collect();
        let kinds: Vec<usize> = assign_kinds(slots, counts).into_iter().map(|(_, k)| k).collect();
        assert_eq!(kinds, [CRATE, SACK, SACK, SACK, BARREL], "each kind stands together");
    }

    /// The barn fills with the stock: more stock, more crates; stacks go up
    /// before the pile moves on; nothing leaves the zone; a stock bigger than
    /// the barn caps at a full barn.
    #[test]
    fn crates_fill_the_zone_in_stacks_and_stay_inside_it() {
        let zone = (Vec3::new(40.0, 0.0, 73.0), Vec3::new(15.0, 3.0, 16.0));
        let few = pile_positions(&[zone], 6);
        assert_eq!(few.len(), 6);
        // The first MAX_LAYERS crates share one floor spot (a stack).
        assert!(few[1].x == few[0].x && few[1].z == few[0].z && few[1].y > few[0].y);
        let all = pile_positions(&[zone], 100_000);
        assert!(all.len() < 100_000, "capped at a full barn: {}", all.len());
        for p in &all {
            assert!(p.x > zone.0.x && p.x < zone.0.x + zone.1.x, "x {p}");
            assert!(p.z > zone.0.z && p.z < zone.0.z + zone.1.z, "z {p}");
            assert!(p.y >= zone.0.y && p.y + CRATE_SIZE.y <= zone.0.y + zone.1.y, "y {p}");
        }
        // Aisles: the x positions are not one continuous run.
        let mut xs: Vec<f32> = all.iter().map(|p| p.x).collect();
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        xs.dedup();
        let biggest_gap = xs.windows(2).map(|w| w[1] - w[0]).fold(0.0_f32, f32::max);
        assert!(biggest_gap > CRATE_SIZE.x + 1.0, "an aisle every few columns ({biggest_gap})");
        assert!(pile_positions(&[], 10).is_empty(), "no storage zone, no crates");
    }

    /// Only what is filed in the Barn (or loose in the home) is barn stock:
    /// the garage bags and the car trunk in the seeded places tree are not,
    /// and the seeded Barn is (so the default home's barn has crates).
    #[test]
    fn only_the_barn_stock_fills_the_barn() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let places = crate::gui::load_places(&root);
        let pool = crate::gui::flatten_placed_items(&places);
        let rooms = vec!["Barn".to_string()];
        let barn: Vec<&PlacedItem> = pool.iter().filter(|p| in_storage_room(&places, &p.container, &rooms)).collect();
        assert!(!barn.is_empty(), "the seeded barn holds stock");
        assert!(barn.iter().all(|p| !p.key.contains(' ')), "barn stock names real item ids: {:?}", barn.iter().map(|p| &p.key).collect::<Vec<_>>());
        let wheat = barn.iter().find(|p| p.key == "grain_wheat_0").expect("wheat in the barn");
        assert_eq!(wheat.qty, 400);
        assert_eq!(wheat.name, "Wheat grain");
        let elsewhere = pool.len() - barn.len();
        assert!(elsewhere > 0, "the garage and trunk items are not barn stock");
        assert!(in_storage_room(&places, "Home", &rooms), "loose home stock goes to the barn");
        assert!(!in_storage_room(&places, "9/9", &rooms));
        assert!(!in_storage_room(&places, "", &rooms));
        // Machine output is filed where the barn stock lives.
        let barn = store_path(&places, &rooms).expect("the seeded barn");
        assert!(in_storage_room(&places, &barn, &rooms));
        assert!(pool.iter().any(|p| p.key == "grain_wheat_0" && p.container == barn));
    }

    /// The Barn's crates sit ON the rack decks the mesh bake builds: every
    /// slot is inside a rack's footprint, resting on a deck's top surface,
    /// under the ceiling, and no two crates overlap. The bottom decks fill
    /// first.
    #[test]
    fn crates_sit_on_the_racks_that_were_built() {
        use crate::ship::structure::{rack_deck_levels, zone_filler, RACK_DECK_M};
        let f = zone_filler("storage").expect("storage filler in zone_filler.ron");
        assert_eq!(f.mesh_kind, "rack");
        let (origin, size) = (Vec3::new(40.0, 0.0, 73.0), Vec3::new(15.0, 3.0, 16.0));
        let slots = rack_slots(origin, size, f);
        let cells = f.cells(origin.x, origin.z, size.x, size.z);
        let levels = rack_deck_levels(f.built_height(size.y));
        assert!(!cells.is_empty() && levels.len() >= 3, "{} racks, {} decks", cells.len(), levels.len());
        assert!(slots.len() >= 200, "a barn holds a few hundred crates: {}", slots.len());
        let (fw, fd) = f.footprint;
        for p in &slots {
            let on_rack = cells.iter().any(|&(cx, cz)| {
                p.x - CRATE_SIZE.x * 0.5 >= cx && p.x + CRATE_SIZE.x * 0.5 <= cx + fw
                    && p.z - CRATE_SIZE.z * 0.5 >= cz && p.z + CRATE_SIZE.z * 0.5 <= cz + fd
            });
            assert!(on_rack, "crate at {p} is not on a rack");
            let on_deck = levels.iter().any(|l| (p.y - (origin.y + l + RACK_DECK_M)).abs() < 1e-4);
            assert!(on_deck, "crate at {p} does not rest on a deck");
            assert!(p.y + CRATE_SIZE.y < origin.y + size.y, "crate at {p} goes through the ceiling");
        }
        for (i, a) in slots.iter().enumerate() {
            for b in &slots[i + 1..] {
                let apart = (a.x - b.x).abs() >= CRATE_SIZE.x - 1e-4
                    || (a.z - b.z).abs() >= CRATE_SIZE.z - 1e-4
                    || (a.y - b.y).abs() >= CRATE_SIZE.y - 1e-4;
                assert!(apart, "crates overlap at {a} and {b}");
            }
        }
        // Bottom decks first: the first deck's worth of crates all share the lowest deck.
        let per_deck = slots.len() / levels.len();
        assert!(slots[..per_deck].iter().all(|p| (p.y - slots[0].y).abs() < 1e-4));
        // The racks never poke through the roof.
        assert!(f.built_height(size.y) < size.y);
    }
}
