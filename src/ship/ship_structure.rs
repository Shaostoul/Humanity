//! ShipStructure: MANY enclosed zones on one ship (increment A of
//! docs/design/ship-superstructure.md, v0.754).
//!
//! The insight (from the design doc): the proven HomeStructure primitive -- a fixed outer box +
//! freely drawn interior walls + per-structure materials + glass-or-steel roof + placed lights +
//! openings + spawn -- is the right primitive for EVERY pressurized space on the ship. What was
//! missing is PLURALITY: there could be exactly one. This module adds it. A ship is a list of
//! ZONES; each zone carries an id, a label, a purpose tag, a world origin offset, and the ENTIRE
//! existing `HomeStructure` body UNCHANGED as its payload. All zone-body coordinates stay
//! zone-LOCAL (metres from the zone box's min corner); the `origin` places the box in the world.
//!
//! Increments B (generated corridors between zone openings) and C (the Commons authoring) build on
//! this file: B adds a `corridors: [...]` list beside `zones` and generates connecting tubes from
//! two zones' door openings; C is pure data (a big glass-roofed zone + machines). Neither needs a
//! schema change here beyond adding fields.
//!
//! File: `data/blueprints/ship_structure.ron`. The old single-home file
//! (`data/blueprints/home_structure.ron`) migrated outright into it (no-compat-debt, pre-launch);
//! `load_or_adopt` still ADOPTS a legacy data dir once -- see its doc comment.

use crate::renderer::mesh::Vertex;
use crate::ship::fibonacci::HomesteadMeshes;
use crate::ship::home_structure::HomeStructure;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::path::Path;

fn default_purpose() -> String {
    "residence".to_string()
}

/// One pressurized zone of the ship: a labelled, purpose-tagged, world-placed `HomeStructure` box.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipZone {
    /// Stable id ("home", "commons", "bay", ...). Unique across the ship; machines reference it.
    pub id: String,
    /// Human label shown in the editor ("Player Home", "The Commons").
    #[serde(default)]
    pub label: String,
    /// Purpose tag the GUI + sims read: residence | commons | bay | agriculture | corridor.
    #[serde(default = "default_purpose")]
    pub purpose: String,
    /// World offset of the zone box's MIN corner (x, y, z); y is the deck height. Zone-body
    /// coordinates are local to this corner.
    #[serde(default)]
    pub origin: (f32, f32, f32),
    /// The entire existing home model, unchanged: box dims, interior walls, openings, materials,
    /// roof, lights, spawn, structures, road/rail graphs, intra-zone volumes.
    pub body: HomeStructure,
}

impl ShipZone {
    /// This zone's origin as a Vec3 (the world position of its box min corner).
    pub fn origin_vec(&self) -> Vec3 {
        Vec3::new(self.origin.0, self.origin.1, self.origin.2)
    }
}

/// The whole ship: a list of zones. Always at least one (validation enforces it).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipStructure {
    pub zones: Vec<ShipZone>,
}

/// The active zone's body, by index -- a FREE function on the Option field (not a GuiState method)
/// so callers can borrow `gui_state.ship_structure` alone while still touching sibling GuiState
/// fields (dirty flags, selections) inside the same block. This is what lets the ~100 existing
/// editor sites keep their shape: `zone_body_mut(&mut g.ship_structure, g.construction_zone)`
/// replaces `g.home_structure.as_mut()` one-for-one.
pub fn zone_body(ship: &Option<ShipStructure>, idx: usize) -> Option<&HomeStructure> {
    ship.as_ref().and_then(|s| s.zones.get(idx)).map(|z| &z.body)
}

/// Mutable twin of `zone_body` (see its doc comment for the free-function rationale).
pub fn zone_body_mut(ship: &mut Option<ShipStructure>, idx: usize) -> Option<&mut HomeStructure> {
    ship.as_mut().and_then(|s| s.zones.get_mut(idx)).map(|z| &mut z.body)
}

/// The active zone's world origin (ZERO when no ship / bad index -- the legacy world position, so
/// every pre-zone code path is unchanged for the home at the origin).
pub fn zone_origin(ship: &Option<ShipStructure>, idx: usize) -> Vec3 {
    ship.as_ref()
        .and_then(|s| s.zones.get(idx))
        .map(|z| z.origin_vec())
        .unwrap_or(Vec3::ZERO)
}

impl ShipStructure {
    /// Structural sanity: at least one zone, every id non-empty and unique. Run on every load so a
    /// hand-edited file with duplicate ids fails loudly instead of machines clamping into the
    /// wrong box.
    pub fn validate(&self) -> Result<(), String> {
        if self.zones.is_empty() {
            return Err("ship_structure has no zones (at least one is required)".to_string());
        }
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for z in &self.zones {
            if z.id.trim().is_empty() {
                return Err("a ship zone has an empty id".to_string());
            }
            if !seen.insert(z.id.as_str()) {
                return Err(format!("duplicate ship zone id '{}'", z.id));
            }
        }
        Ok(())
    }

    /// Index of the "home" zone (the player's own allotment): the zone with id "home", else zone 0.
    /// Deterministic fallback so a hand-renamed file still resolves somewhere stable.
    pub fn home_zone_index(&self) -> usize {
        self.zones.iter().position(|z| z.id == "home").unwrap_or(0)
    }

    /// Index of a zone by id.
    pub fn zone_index(&self, id: &str) -> Option<usize> {
        self.zones.iter().position(|z| z.id == id)
    }

    /// True when ANY zone has a clear/glass roof (drives the transparent ceiling pass).
    pub fn any_glass_roof(&self) -> bool {
        self.zones.iter().any(|z| z.body.roof_is_glass())
    }

    /// Mint a unique ship-zone id from a base ("zone" -> "zone_2", "zone_3", ...).
    pub fn unique_ship_zone_id(&self, base: &str) -> String {
        if self.zone_index(base).is_none() {
            return base.to_string();
        }
        let mut n = 2usize;
        loop {
            let id = format!("{base}_{n}");
            if self.zone_index(&id).is_none() {
                return id;
            }
            n += 1;
        }
    }

    /// An origin for a NEW zone that is clear of every existing zone: past the furthest +X extent,
    /// with a walking gap, on the ground plane. Deliberately simple (a row of boxes) -- corridor
    /// generation (increment B) is what ties them together.
    pub fn next_free_origin(&self, gap: f32) -> (f32, f32, f32) {
        let max_x = self
            .zones
            .iter()
            .map(|z| z.origin.0 + z.body.width)
            .fold(0.0_f32, f32::max);
        (max_x + gap.max(0.0), 0.0, 0.0)
    }

    /// Add a new zone: a modest default box (10 x 10 x 3 m, steel shell, the default glass roof)
    /// placed clear of every existing zone. Returns its index.
    pub fn add_zone(&mut self, label: &str, purpose: &str) -> usize {
        let id = self.unique_ship_zone_id("zone");
        let origin = self.next_free_origin(10.0);
        let body: HomeStructure = ron::from_str("(width: 10.0, depth: 10.0, height: 3.0)")
            .expect("the default zone body literal parses");
        self.zones.push(ShipZone {
            id,
            label: label.to_string(),
            purpose: purpose.to_string(),
            origin,
            body,
        });
        self.zones.len() - 1
    }

    /// Remove the zone at `idx`. Refuses the home zone and the last remaining zone (the ship must
    /// always keep the player's home). Returns true if removed.
    pub fn remove_zone(&mut self, idx: usize) -> bool {
        if self.zones.len() <= 1 || idx >= self.zones.len() || idx == self.home_zone_index() {
            return false;
        }
        self.zones.remove(idx);
        true
    }

    /// Per-zone footprints for machine placement clamping: (zone id, world origin, (w, d, h)).
    /// Ordered as declared, so `machines::resolve_zone_rect`'s first-zone fallback is deterministic.
    pub fn zone_rects(&self) -> Vec<crate::machines::ZoneRect> {
        self.zones
            .iter()
            .map(|z| crate::machines::ZoneRect {
                id: z.id.clone(),
                origin: z.origin,
                size: (z.body.width, z.body.depth, z.body.height),
            })
            .collect()
    }

    /// World AABB of all zone boxes (min, max) -- the conduit-node clamp bounds.
    pub fn world_bounds(&self) -> (Vec3, Vec3) {
        let mut mn = Vec3::splat(f32::INFINITY);
        let mut mx = Vec3::splat(f32::NEG_INFINITY);
        for z in &self.zones {
            let o = z.origin_vec();
            mn = mn.min(o);
            mx = mx.max(o + Vec3::new(z.body.width, z.body.height, z.body.depth));
        }
        if self.zones.is_empty() {
            (Vec3::ZERO, Vec3::ZERO)
        } else {
            (mn, mx)
        }
    }

    /// Load from RON, validating. None (with a warning) on a missing/invalid file -- the caller
    /// falls back exactly as it did for a broken home_structure.ron.
    pub fn load(path: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(path).ok()?;
        match ron::from_str::<ShipStructure>(&text) {
            Ok(s) => match s.validate() {
                Ok(()) => Some(s),
                Err(e) => {
                    log::warn!("ship_structure: {} is invalid: {e}", path.display());
                    None
                }
            },
            Err(e) => {
                log::warn!("ship_structure: failed to parse {}: {e}", path.display());
                None
            }
        }
    }

    /// Load the ship from a blueprints dir, ADOPTING a legacy single-home data dir once.
    ///
    /// - `ship_structure.ron` present -> load it (the normal path).
    /// - absent but `home_structure.ron` present -> wrap the old file as zone "home" ONCE. The
    ///   next in-editor Save writes `ship_structure.ron`, after which the old file is never read
    ///   again (this branch stops being taken). This is a one-time in-code ADOPTION at load for
    ///   data dirs written before increment A -- not a kept compatibility layer; delete it when
    ///   no pre-A data dirs remain in the wild.
    pub fn load_or_adopt(blueprints_dir: &Path) -> Option<Self> {
        let ship_path = blueprints_dir.join("ship_structure.ron");
        if ship_path.exists() {
            return Self::load(&ship_path);
        }
        let body = HomeStructure::load(&blueprints_dir.join("home_structure.ron"))?;
        log::info!(
            "ship_structure: adopted legacy home_structure.ron as zone 'home' (one-time; the next save writes ship_structure.ron)"
        );
        Some(ShipStructure {
            zones: vec![ShipZone {
                id: "home".to_string(),
                label: "Player Home".to_string(),
                purpose: "residence".to_string(),
                origin: (0.0, 0.0, 0.0),
                body,
            }],
        })
    }

    /// Write back to RON, preserving an existing file's leading comment header (the v0.526 lesson,
    /// same discipline as `HomeStructure::save`).
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let config = ron::ser::PrettyConfig::default().struct_names(false);
        let body = ron::ser::to_string_pretty(self, config).map_err(|e| e.to_string())?;
        let preserved = std::fs::read_to_string(path).ok().and_then(|existing| {
            let header: String = existing
                .lines()
                .take_while(|l| l.trim_start().starts_with("//") || l.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            if header.contains("//") {
                Some(format!("{}\n\n", header.trim_end()))
            } else {
                None
            }
        });
        let header = preserved.unwrap_or_else(|| {
            "// HumanityOS ship structure: MANY pressurized zones, each a fixed outer box + freely\n\
             // placed interior walls (the proven home model, now plural). Each zone: id, label,\n\
             // purpose (residence|commons|bay|agriculture|corridor), a world `origin` for its box\n\
             // min corner, and the full home `body` in zone-local metres. Design doc:\n\
             // docs/design/ship-superstructure.md (increment A).\n\n"
                .to_string()
        });
        std::fs::write(path, format!("{header}{body}")).map_err(|e| e.to_string())
    }

    /// Generate the renderable meshes for the WHOLE ship: each zone's body generates through the
    /// unchanged `HomeStructure::generate_meshes`, then its vertices (and room metadata) translate
    /// by the zone origin, and everything merges into ONE `HomesteadMeshes` so the existing
    /// `apply_homestead_meshes` upload path is a drop-in (chosen over per-zone RenderObjects: the
    /// apply path reuses mesh/material SLOTS by index across per-frame rebuilds, and a merged
    /// result keeps that reuse logic untouched -- the least-churn option the design doc allows).
    ///
    /// Roofs are per-zone (the task's "glass-or-steel roof" per zone): GLASS-roof zones' ceilings
    /// merge into `ceilings` (the transparent always-visible pass), OPAQUE-roof zones' ceilings
    /// merge into `ceilings_opaque` (rendered with the opaque ceiling material, gated by the
    /// show-roof toggle exactly like the old single opaque roof).
    ///
    /// Room ids: the home zone keeps its raw ids ("home", "room_N") so every existing id-keyed
    /// lookup (room_types display names, the v0.706 spawn-room fallback) is unchanged; other
    /// zones prefix "<zone_id>:" so ids stay unique across the ship. Only the home zone keeps a
    /// spawn room (the camera spawns there on world load).
    pub fn generate_meshes(&self) -> HomesteadMeshes {
        let home_idx = self.home_zone_index();
        let mut out = HomesteadMeshes {
            floors: Vec::new(),
            walls: (Vec::new(), Vec::new()),
            material_walls: Vec::new(),
            trim: (Vec::new(), Vec::new()),
            windows: (Vec::new(), Vec::new()),
            mirrors: (Vec::new(), Vec::new()),
            ceilings: (Vec::new(), Vec::new()),
            ceilings_opaque: (Vec::new(), Vec::new()),
            room_info: Vec::new(),
        };
        for (zi, z) in self.zones.iter().enumerate() {
            let o = z.origin_vec();
            let m = z.body.generate_meshes();
            for (v, i, c, mt) in m.floors {
                out.floors.push((shift_verts(v, o), i, c, mt));
            }
            for (v, i, c) in m.material_walls {
                out.material_walls.push((shift_verts(v, o), i, c));
            }
            merge_shifted(&mut out.walls, m.walls, o);
            merge_shifted(&mut out.trim, m.trim, o);
            merge_shifted(&mut out.windows, m.windows, o);
            merge_shifted(&mut out.mirrors, m.mirrors, o);
            if z.body.roof_is_glass() {
                merge_shifted(&mut out.ceilings, m.ceilings, o);
            } else {
                merge_shifted(&mut out.ceilings_opaque, m.ceilings, o);
            }
            // A body never fills ceilings_opaque itself today, but merge it anyway so a future
            // body-level split cannot silently drop geometry here.
            merge_shifted(&mut out.ceilings_opaque, m.ceilings_opaque, o);
            for mut r in m.room_info {
                r.center += o;
                if zi != home_idx {
                    r.id = format!("{}:{}", z.id, r.id);
                    r.is_spawn_room = false;
                }
                out.room_info.push(r);
            }
        }
        out
    }
}

/// Translate a vertex buffer by a zone origin.
fn shift_verts(mut verts: Vec<Vertex>, o: Vec3) -> Vec<Vertex> {
    for v in verts.iter_mut() {
        v.position[0] += o.x;
        v.position[1] += o.y;
        v.position[2] += o.z;
    }
    verts
}

/// Append a (verts, indices) family onto an accumulator, translated by a zone origin.
fn merge_shifted(acc: &mut (Vec<Vertex>, Vec<u32>), add: (Vec<Vertex>, Vec<u32>), o: Vec3) {
    let base = acc.0.len() as u32;
    acc.0.extend(shift_verts(add.0, o));
    acc.1.extend(add.1.into_iter().map(|i| i + base));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(w: f32, d: f32, h: f32) -> HomeStructure {
        ron::from_str::<HomeStructure>(&format!("(width: {w}, depth: {d}, height: {h})"))
            .expect("body literal parses")
    }

    fn zone(id: &str, origin: (f32, f32, f32), w: f32, d: f32, h: f32) -> ShipZone {
        ShipZone {
            id: id.to_string(),
            label: id.to_string(),
            purpose: "residence".to_string(),
            origin,
            body: body(w, d, h),
        }
    }

    fn two_zone_ship() -> ShipStructure {
        ShipStructure {
            zones: vec![
                zone("home", (0.0, 0.0, 0.0), 55.0, 89.0, 3.0),
                zone("commons", (70.0, 0.0, 0.0), 20.0, 30.0, 6.0),
            ],
        }
    }

    /// A unique temp path for a round-trip test (no tempfile dep in the crate).
    fn temp_path(name: &str) -> std::path::PathBuf {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("hos_ship_structure_{name}_{n}"))
    }

    #[test]
    fn ship_structure_round_trip_preserves_zones() {
        let ship = two_zone_ship();
        let dir = temp_path("roundtrip");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ship_structure.ron");
        ship.save(&path).expect("saves");
        let back = ShipStructure::load(&path).expect("loads back");
        assert_eq!(back.zones.len(), 2);
        assert_eq!(back.zones[0].id, "home");
        assert_eq!(back.zones[1].id, "commons");
        assert_eq!(back.zones[1].origin, (70.0, 0.0, 0.0));
        assert_eq!(back.zones[1].body.width, 20.0);
        assert_eq!(back.zones[1].body.height, 6.0);
        // The saved file leads with a comment header (the header-preserving save discipline).
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("//"), "save writes a comment header");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn adoption_wraps_a_legacy_home_structure_as_zone_home() {
        let dir = temp_path("adopt");
        std::fs::create_dir_all(&dir).unwrap();
        // A legacy data dir: home_structure.ron only, no ship_structure.ron.
        std::fs::write(
            dir.join("home_structure.ron"),
            "(width: 55.0, depth: 89.0, height: 3.0)",
        )
        .unwrap();
        let ship = ShipStructure::load_or_adopt(&dir).expect("adopts the legacy file");
        assert_eq!(ship.zones.len(), 1);
        assert_eq!(ship.zones[0].id, "home");
        assert_eq!(ship.zones[0].origin, (0.0, 0.0, 0.0));
        assert_eq!(ship.zones[0].body.width, 55.0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_present_ship_structure_wins_over_the_legacy_file() {
        let dir = temp_path("prefer_new");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("home_structure.ron"),
            "(width: 55.0, depth: 89.0, height: 3.0)",
        )
        .unwrap();
        two_zone_ship().save(&dir.join("ship_structure.ron")).unwrap();
        let ship = ShipStructure::load_or_adopt(&dir).expect("loads the new file");
        assert_eq!(ship.zones.len(), 2, "the ship file wins; the legacy file is ignored");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn duplicate_zone_ids_fail_validation() {
        let mut ship = two_zone_ship();
        ship.zones[1].id = "home".to_string();
        assert!(ship.validate().is_err(), "duplicate ids must be rejected");
        // And a load of such a file returns None (falls back like a broken file).
        let dir = temp_path("dupe");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ship_structure.ron");
        // Serialize WITHOUT validating (save doesn't validate; load does).
        ship.save(&path).unwrap();
        assert!(ShipStructure::load(&path).is_none(), "an invalid file must not load");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_and_blank_ids_fail_validation() {
        assert!(ShipStructure { zones: Vec::new() }.validate().is_err(), "no zones");
        let mut ship = two_zone_ship();
        ship.zones[1].id = "  ".to_string();
        assert!(ship.validate().is_err(), "blank id");
    }

    #[test]
    fn generate_meshes_offsets_each_zone_by_its_origin() {
        let ship = two_zone_ship();
        let m = ship.generate_meshes();
        // Two zones, one floor each.
        assert_eq!(m.floors.len(), 2);
        // The commons floor (index 1, zone order preserved) spans x = 70..90.
        let xs: Vec<f32> = m.floors[1].0.iter().map(|v| v.position[0]).collect();
        let min_x = xs.iter().cloned().fold(f32::MAX, f32::min);
        let max_x = xs.iter().cloned().fold(f32::MIN, f32::max);
        assert!((min_x - 70.0).abs() < 1e-3, "commons floor min x at its origin, got {min_x}");
        assert!((max_x - 90.0).abs() < 1e-3, "commons floor max x at origin + width, got {max_x}");
        // Room metadata offsets too: the commons room centre sits inside 70..90.
        let commons_room = m.room_info.iter().find(|r| r.id.starts_with("commons:"))
            .expect("the commons zone's room id is prefixed with its zone id");
        assert!(commons_room.center.x > 70.0 && commons_room.center.x < 90.0);
        assert!(!commons_room.is_spawn_room, "only the home zone keeps a spawn room");
        // The home zone keeps its raw id + the spawn flag.
        let home_room = m.room_info.iter().find(|r| r.id == "home").expect("home room id unprefixed");
        assert!(home_room.is_spawn_room);
    }

    #[test]
    fn per_zone_roofs_split_glass_from_opaque() {
        let mut ship = two_zone_ship();
        ship.zones[1].body.roof_material = 1; // commons: opaque steel roof
        let m = ship.generate_meshes();
        assert!(!m.ceilings.0.is_empty(), "the glass-roof home fills the transparent ceiling buffer");
        assert!(!m.ceilings_opaque.0.is_empty(), "the opaque-roof commons fills the opaque buffer");
        // The opaque buffer's geometry sits at the commons origin (x >= 70).
        let min_x = m.ceilings_opaque.0.iter().map(|v| v.position[0]).fold(f32::MAX, f32::min);
        assert!(min_x >= 70.0 - 1e-3, "opaque ceilings belong to the commons zone, got min x {min_x}");
        assert!(ship.any_glass_roof());
        ship.zones[0].body.roof_material = 1;
        assert!(!ship.any_glass_roof());
    }

    #[test]
    fn add_zone_lands_clear_of_existing_zones_and_remove_protects_home() {
        let mut ship = two_zone_ship();
        let idx = ship.add_zone("New Zone", "bay");
        assert_eq!(ship.zones.len(), 3);
        let z = &ship.zones[idx];
        assert_eq!(z.body.width, 10.0);
        assert_eq!(z.body.depth, 10.0);
        assert_eq!(z.body.height, 3.0);
        // Past the commons' far edge (70 + 20) plus the gap.
        assert!(z.origin.0 >= 90.0 + 10.0 - 1e-3, "new zone clear of existing ones, got {}", z.origin.0);
        assert!(ship.validate().is_ok(), "minted id is unique");
        // Deleting the home zone is refused; deleting the new zone works.
        let home = ship.home_zone_index();
        assert!(!ship.remove_zone(home), "the home zone cannot be deleted");
        assert!(ship.remove_zone(idx), "a non-home zone deletes");
        assert_eq!(ship.zones.len(), 2);
        // The last remaining zone can never be deleted.
        assert!(ship.remove_zone(1));
        assert!(!ship.remove_zone(0), "the last zone cannot be deleted");
    }

    #[test]
    fn zone_body_accessors_resolve_the_indexed_zone() {
        let mut ship = Some(two_zone_ship());
        assert_eq!(zone_body(&ship, 1).map(|b| b.width), Some(20.0));
        assert_eq!(zone_origin(&ship, 1), Vec3::new(70.0, 0.0, 0.0));
        assert_eq!(zone_origin(&ship, 99), Vec3::ZERO, "bad index -> ZERO origin");
        zone_body_mut(&mut ship, 1).unwrap().height = 8.0;
        assert_eq!(zone_body(&ship, 1).map(|b| b.height), Some(8.0));
        let none: Option<ShipStructure> = None;
        assert!(zone_body(&none, 0).is_none());
    }

    #[test]
    fn parses_the_shipped_ship_structure() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("blueprints")
            .join("ship_structure.ron");
        let ship = ShipStructure::load(&path).expect("ship_structure.ron parses + validates");
        let home = &ship.zones[ship.home_zone_index()];
        assert_eq!(home.id, "home");
        assert!(home.body.width > 0.0 && home.body.depth > 0.0 && home.body.height > 0.0);
        assert!(!home.body.walls.is_empty(), "the migrated home kept its interior walls");
    }
}
