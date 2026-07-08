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
//! Increment B (this file): GENERATED CORRIDORS. A `corridors: [...]` list beside `zones`;
//! each row references two zones' DOOR openings and extrudes a straight, axis-aligned box tube
//! between them (floor slab + two side walls + a glass-or-shell lid), cuts a door-sized aperture
//! through each zone's perimeter shell where the tube meets it (mesh AND collision, so the hallway
//! is genuinely walkable, not decoration), and registers a walkable room bound per corridor.
//! Increment C (the Commons authoring) is pure data on top: a big glass-roofed zone + machines +
//! corridor rows -- no schema change needed here.
//!
//! File: `data/blueprints/ship_structure.ron`. The old single-home file
//! (`data/blueprints/home_structure.ron`) migrated outright into it (no-compat-debt, pre-launch);
//! `load_or_adopt` still ADOPTS a legacy data dir once -- see its doc comment.

use crate::renderer::mesh::Vertex;
use crate::ship::fibonacci::{floor_quad, wall_box, HomesteadMeshes, RoomInfo};
use crate::ship::home_structure::{HomeStructure, OpeningKind, ShellCut};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::path::Path;

fn default_purpose() -> String {
    "residence".to_string()
}

fn default_corridor_width() -> f32 {
    3.0
}

/// Corridor side-wall thickness (metres) -- matches the legacy interior-wall default (0.15 m).
pub const CORRIDOR_WALL_THICKNESS: f32 = 0.15;
/// Tiny vertical clearance so a tube's floor/lid never sit COPLANAR with a zone's floor/ceiling
/// where the tube overlaps the zone footprint (coplanar quads z-fight). 1 cm: imperceptible, and
/// collision is a 2D XZ push-out so it changes nothing gameplay-side.
const CORRIDOR_SURFACE_EPS: f32 = 0.01;
/// Lateral misalignment allowed between the two openings' centres, as a fraction of the corridor
/// width (the tube centreline splits the difference; the doors still land inside the tube mouth).
const CORRIDOR_LATERAL_TOL_FRAC: f32 = 0.25;
/// |sin| of the max angle a door's wall may deviate from perpendicular to the run (~5 degrees) --
/// v1 corridors meet their openings head-on.
const CORRIDOR_WALL_ALIGN_TOL: f32 = 0.09;
/// Minimum run length (metres): two openings closer than this are a doorway, not a corridor.
const CORRIDOR_MIN_RUN: f32 = 0.25;
/// Minimum corridor width (metres): narrower than this is not walkable (player radius 0.3).
const CORRIDOR_MIN_WIDTH: f32 = 0.5;

/// One generated corridor (ship-superstructure increment B): a straight, axis-aligned tube between
/// two zones' DOOR openings. `from_opening`/`to_opening` index that zone's door list in the
/// canonical `zone_door_refs` order (walls in declaration order, openings in declaration order,
/// windows skipped -- a corridor can only meet a door). v1 corridors are STRAIGHT: validation
/// rejects opening pairs that do not face each other along one world axis (L-bends are a
/// documented follow-up: two segments + an elbow).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipCorridor {
    pub from_zone: String,
    /// Index into the from-zone's door openings (doors only, not windows -- `zone_door_refs`).
    pub from_opening: usize,
    pub to_zone: String,
    pub to_opening: usize,
    /// Tube width in metres (outer, across the run). Side walls sit AT the edges, so the clear
    /// interior is width minus one wall thickness.
    #[serde(default = "default_corridor_width")]
    pub width: f32,
    /// Glass lid (rides the transparent always-visible ceiling pass, exactly like a glass zone
    /// roof); false = an opaque lid in the show-roof-gated opaque pass, like a steel zone roof.
    #[serde(default)]
    pub glass_top: bool,
}

/// One DOOR opening of a zone body, in the CANONICAL corridor-indexing order: walls in declaration
/// order, openings in declaration order, WINDOWS skipped, degenerate widths (<= 0.01 m, matching
/// `panel_placements`' guard) skipped. `ShipCorridor::{from,to}_opening` index into this list, the
/// editor's door combos display it, and `corridor_geometry` resolves through it. Zone-LOCAL coords.
#[derive(Debug, Clone, PartialEq)]
pub struct DoorRef {
    pub wall_index: usize,
    pub opening_index: usize,
    /// Aperture centre on the wall line (x, z), zone-local metres.
    pub center: (f32, f32),
    /// Unit direction ALONG the wall (a -> b).
    pub along: (f32, f32),
    pub width: f32,
    pub height: f32,
    pub sill: f32,
}

/// Enumerate a zone body's DOOR openings in the canonical corridor-indexing order (see `DoorRef`).
pub fn zone_door_refs(body: &HomeStructure) -> Vec<DoorRef> {
    let mut out = Vec::new();
    for (wi, wall) in body.walls.iter().enumerate() {
        let (dx, dz) = (wall.b.0 - wall.a.0, wall.b.1 - wall.a.1);
        let len = (dx * dx + dz * dz).sqrt();
        if len < 1e-4 {
            continue;
        }
        let dir = (dx / len, dz / len);
        for (oi, op) in wall.openings.iter().enumerate() {
            if op.kind != OpeningKind::Door || op.width <= 0.01 {
                continue;
            }
            let s = (op.at + op.width * 0.5).clamp(0.0, len);
            out.push(DoorRef {
                wall_index: wi,
                opening_index: oi,
                center: (wall.a.0 + dir.0 * s, wall.a.1 + dir.1 * s),
                along: dir,
                width: op.width,
                height: op.height,
                sill: op.sill,
            });
        }
    }
    out
}

/// The world axis a v1 corridor runs along (straight + axis-aligned; the design doc's L-bends are
/// a follow-up).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorridorAxis {
    X,
    Z,
}

impl CorridorAxis {
    fn name(self) -> &'static str {
        match self {
            CorridorAxis::X => "X",
            CorridorAxis::Z => "Z",
        }
    }
}

/// A corridor RESOLVED to world geometry: where the tube actually is. Everything generation and
/// collision need, computed once by `ShipStructure::corridor_geometry` (which doubles as the
/// validator -- an Err is the honest reason this corridor cannot exist).
#[derive(Debug, Clone, PartialEq)]
pub struct CorridorGeom {
    pub axis: CorridorAxis,
    /// World span along the run axis (start < end): the two openings' axis coordinates.
    pub start: f32,
    pub end: f32,
    /// World lateral centreline (z when axis = X; x when axis = Z): the midpoint of the two
    /// openings' lateral coordinates (they may differ by up to width * CORRIDOR_LATERAL_TOL_FRAC).
    pub lat: f32,
    /// World floor (deck) height: the openings' shared sill y.
    pub floor_y: f32,
    /// Interior tube height: the SHORTER zone's box height, so the lid never rises above either
    /// roofline and every door (door height <= zone height) always fits.
    pub height: f32,
    pub width: f32,
    pub glass_top: bool,
    /// The two openings' world centres at sill level -- the tube's endpoints.
    pub end_from: Vec3,
    pub end_to: Vec3,
    pub from_zone_idx: usize,
    pub to_zone_idx: usize,
    /// The from/to door apertures' (width, height) -- what the shell cuts open.
    pub door_from: (f32, f32),
    pub door_to: (f32, f32),
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
    /// Generated corridors between zones' door openings (increment B). Serde-defaulted so every
    /// pre-B ship_structure.ron (no `corridors` field) keeps loading unchanged.
    #[serde(default)]
    pub corridors: Vec<ShipCorridor>,
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
    /// Structural sanity: at least one zone, every id non-empty and unique, every corridor
    /// resolvable (zones exist, door indices in range, from != to, openings facing + aligned).
    /// Run on every load so a hand-edited file fails loudly instead of machines clamping into the
    /// wrong box or a hallway floating unattached. (The in-editor SAVE path prunes broken corridor
    /// rows first -- `prune_invalid_corridors` -- so a file written by the editor always re-loads.)
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
        for (i, c) in self.corridors.iter().enumerate() {
            self.corridor_geometry(c)
                .map_err(|e| format!("corridor {i} ({} -> {}): {e}", c.from_zone, c.to_zone))?;
        }
        Ok(())
    }

    /// Resolve a corridor row to world geometry, or the honest reason it cannot exist. This IS the
    /// corridor validator: `validate` (load), the editor's Create button, mesh generation, and
    /// collision all go through it, so they can never disagree about what a corridor is.
    pub fn corridor_geometry(&self, c: &ShipCorridor) -> Result<CorridorGeom, String> {
        let from_idx = self
            .zone_index(&c.from_zone)
            .ok_or_else(|| format!("unknown zone '{}'", c.from_zone))?;
        let to_idx = self
            .zone_index(&c.to_zone)
            .ok_or_else(|| format!("unknown zone '{}'", c.to_zone))?;
        if from_idx == to_idx {
            return Err(format!("connects zone '{}' to itself", c.from_zone));
        }
        if c.width < CORRIDOR_MIN_WIDTH {
            return Err(format!(
                "width {:.2} m is below the {CORRIDOR_MIN_WIDTH} m minimum",
                c.width
            ));
        }
        let zf = &self.zones[from_idx];
        let zt = &self.zones[to_idx];
        let doors_f = zone_door_refs(&zf.body);
        let doors_t = zone_door_refs(&zt.body);
        let df = doors_f.get(c.from_opening).ok_or_else(|| {
            format!(
                "zone '{}' has {} door opening(s); from_opening {} is out of range",
                c.from_zone,
                doors_f.len(),
                c.from_opening
            )
        })?;
        let dt = doors_t.get(c.to_opening).ok_or_else(|| {
            format!(
                "zone '{}' has {} door opening(s); to_opening {} is out of range",
                c.to_zone,
                doors_t.len(),
                c.to_opening
            )
        })?;
        let of = zf.origin_vec();
        let ot = zt.origin_vec();
        let end_from = Vec3::new(of.x + df.center.0, of.y + df.sill, of.z + df.center.1);
        let end_to = Vec3::new(ot.x + dt.center.0, ot.y + dt.sill, ot.z + dt.center.1);
        if (end_from.y - end_to.y).abs() > 0.01 {
            return Err(format!(
                "openings are at different deck heights ({:.2} vs {:.2} m); v1 corridors are level",
                end_from.y, end_to.y
            ));
        }
        let (dx, dz) = (end_to.x - end_from.x, end_to.z - end_from.z);
        let (axis, run, lateral) = if dx.abs() >= dz.abs() {
            (CorridorAxis::X, dx, dz)
        } else {
            (CorridorAxis::Z, dz, dx)
        };
        if run.abs() < CORRIDOR_MIN_RUN {
            return Err(format!(
                "openings are only {:.2} m apart along the run; too short for a corridor",
                run.abs()
            ));
        }
        let lat_tol = c.width * CORRIDOR_LATERAL_TOL_FRAC;
        if lateral.abs() > lat_tol {
            return Err(format!(
                "openings are offset {:.2} m sideways (allowed: width/4 = {:.2} m); v1 corridors are \
                 straight and axis-aligned -- move a door or a zone origin (L-bends are a follow-up)",
                lateral.abs(),
                lat_tol
            ));
        }
        // Each door's wall must run PERPENDICULAR to the corridor axis, so the door faces down the
        // tube instead of sideways into a tube wall.
        let misaligned = |along: (f32, f32)| match axis {
            CorridorAxis::X => along.0.abs() > CORRIDOR_WALL_ALIGN_TOL,
            CorridorAxis::Z => along.1.abs() > CORRIDOR_WALL_ALIGN_TOL,
        };
        if misaligned(df.along) {
            return Err(format!(
                "the from opening's wall is not perpendicular to the corridor's {} run; v1 corridors \
                 meet openings head-on",
                axis.name()
            ));
        }
        if misaligned(dt.along) {
            return Err(format!(
                "the to opening's wall is not perpendicular to the corridor's {} run; v1 corridors \
                 meet openings head-on",
                axis.name()
            ));
        }
        let (fa, ta, fl, tl) = match axis {
            CorridorAxis::X => (end_from.x, end_to.x, end_from.z, end_to.z),
            CorridorAxis::Z => (end_from.z, end_to.z, end_from.x, end_to.x),
        };
        Ok(CorridorGeom {
            axis,
            start: fa.min(ta),
            end: fa.max(ta),
            lat: (fl + tl) * 0.5,
            floor_y: end_from.y,
            height: zf.body.height.min(zt.body.height).max(1.0),
            width: c.width,
            glass_top: c.glass_top,
            end_from,
            end_to,
            from_zone_idx: from_idx,
            to_zone_idx: to_idx,
            door_from: (df.width, df.height),
            door_to: (dt.width, dt.height),
        })
    }

    /// The corridor APERTURES through zone `zi`'s perimeter shell: for each valid corridor ending
    /// in this zone, one door-sized cut through the perimeter face its tube leaves by (the face in
    /// the run direction toward the other zone). Door-sized -- the walkable opening IS the door;
    /// the tube (which may be wider) encloses the cut from outside. Fed to
    /// `generate_meshes_with_shell_cuts` (mesh) and `wall_segments_with_shell_cuts` (collision).
    pub fn shell_cuts_for_zone(&self, zi: usize) -> Vec<ShellCut> {
        let Some(zone) = self.zones.get(zi) else {
            return Vec::new();
        };
        let (w, d) = (zone.body.width, zone.body.depth);
        let o = zone.origin_vec();
        let mut cuts = Vec::new();
        for c in &self.corridors {
            let Ok(g) = self.corridor_geometry(c) else {
                continue; // broken rows cut nothing (the corridors panel shows why)
            };
            // Which end of this corridor (if either) lands in zone `zi`, and the door aperture there.
            let (end, other, (dw, dh)) = if g.from_zone_idx == zi {
                (g.end_from, g.end_to, g.door_from)
            } else if g.to_zone_idx == zi {
                (g.end_to, g.end_from, g.door_to)
            } else {
                continue;
            };
            // The tube leaves the zone through the perimeter face in the run direction toward the
            // other end. Edge indices + along-edge params follow the perimeter winding documented
            // on `ShellCut` (0: z=0, 1: x=w, 2: z=d, 3: x=0).
            let (edge, at) = match g.axis {
                CorridorAxis::X if other.x > end.x => (1usize, (end.z - o.z) - dw * 0.5),
                CorridorAxis::X => (3, d - ((end.z - o.z) + dw * 0.5)),
                CorridorAxis::Z if other.z > end.z => (2, w - ((end.x - o.x) + dw * 0.5)),
                CorridorAxis::Z => (0, (end.x - o.x) - dw * 0.5),
            };
            cuts.push(ShellCut {
                edge,
                at,
                width: dw,
                height: dh.min(zone.body.height),
            });
        }
        cuts
    }

    /// Drop corridor rows that no longer resolve (a referenced zone/door was deleted, or an edit
    /// moved the openings out of alignment). Returns how many were dropped. Called by the engine's
    /// SAVE path so a written ship_structure.ron ALWAYS re-loads (`validate` rejects the whole file
    /// on a bad corridor); LIVE editing deliberately keeps invalid rows (mesh + collision skip
    /// them, the corridors panel shows the error) so a transient misalignment while dragging a
    /// zone origin does not silently destroy the row.
    pub fn prune_invalid_corridors(&mut self) -> usize {
        let bad: Vec<usize> = self
            .corridors
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                self.corridor_geometry(c).err().map(|e| {
                    log::warn!(
                        "ship_structure: dropping corridor {i} ({} -> {}): {e}",
                        c.from_zone,
                        c.to_zone
                    );
                    i
                })
            })
            .collect();
        for i in bad.iter().rev() {
            self.corridors.remove(*i);
        }
        bad.len()
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
    /// always keep the player's home). Corridors referencing the removed zone are dangling, so
    /// they go with it. Returns true if removed.
    pub fn remove_zone(&mut self, idx: usize) -> bool {
        if self.zones.len() <= 1 || idx >= self.zones.len() || idx == self.home_zone_index() {
            return false;
        }
        let removed_id = self.zones[idx].id.clone();
        self.zones.remove(idx);
        self.corridors
            .retain(|c| c.from_zone != removed_id && c.to_zone != removed_id);
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
            corridors: Vec::new(),
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
             // min corner, and the full home `body` in zone-local metres. `corridors` rows generate\n\
             // straight tubes between two zones' door openings, e.g.\n\
             // (from_zone: \"home\", from_opening: 0, to_zone: \"commons\", to_opening: 0, width: 3.0, glass_top: true).\n\
             // Design doc: docs/design/ship-superstructure.md (increments A + B).\n\n"
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
            // Corridor apertures cut through this zone's perimeter shell (increment B): the body
            // generates with door-sized holes where corridor tubes meet its box, so a hallway is
            // walkable INTO, not butted against sealed hull. Empty for most zones = the exact
            // pre-B path.
            let m = z.body.generate_meshes_with_shell_cuts(&self.shell_cuts_for_zone(zi));
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
        // GENERATED CORRIDORS (increment B): each valid corridor extrudes a straight box tube
        // between its two door openings -- a floor slab, two side walls, and a lid. The pieces
        // merge into the SAME mesh families as zone geometry (floors / material_walls / ceilings
        // or ceilings_opaque), so the apply path, the render slots, and the transparent-glass pass
        // are all untouched. Each corridor also registers a RoomInfo ("corridor_<i>") so room
        // bounds, the sealed-atmosphere fold, and the "you are in <room>" HUD treat mid-hallway as
        // INSIDE the ship (design point D: the shared pressurized volume spans the tubes).
        // Geometry-invalid rows are skipped WITHOUT logging here -- this runs every editor drag
        // frame; validate() (load), prune_invalid_corridors() (save), and the corridors panel own
        // the reporting.
        for (ci, c) in self.corridors.iter().enumerate() {
            let Ok(g) = self.corridor_geometry(c) else {
                continue;
            };
            // The tube inherits the FROM zone's shell material (the zone it was built from); a
            // per-corridor material override is a follow-up.
            let mat = self.zones[g.from_zone_idx].body.shell_material;
            let col = HomeStructure::material_color(mat);
            let hw = g.width * 0.5;
            let len = g.end - g.start;
            // Min corner + span of the tube footprint, axis-dependent.
            let (fx, fz, sx, sz) = match g.axis {
                CorridorAxis::X => (g.start, g.lat - hw, len, g.width),
                CorridorAxis::Z => (g.lat - hw, g.start, g.width, len),
            };
            // Floor slab: lifted 1 cm (CORRIDOR_SURFACE_EPS) so it never sits coplanar with a zone
            // floor where the tube overlaps the box footprint (coplanar quads z-fight).
            let (fv, fi) = floor_quad(
                Vec3::new(fx, g.floor_y + CORRIDOR_SURFACE_EPS, fz),
                Vec3::new(sx, 0.0, sz),
            );
            out.floors.push((fv, fi, col, mat));
            // Two side walls, the full run, the full tube height (the SHORTER zone's box height --
            // see CorridorGeom::height). Both merge into one material_walls entry.
            let mut sides: (Vec<Vertex>, Vec<u32>) = (Vec::new(), Vec::new());
            for s in [-1.0f32, 1.0] {
                let (a, b) = match g.axis {
                    CorridorAxis::X => (
                        Vec3::new(g.start, 0.0, g.lat + hw * s),
                        Vec3::new(g.end, 0.0, g.lat + hw * s),
                    ),
                    CorridorAxis::Z => (
                        Vec3::new(g.lat + hw * s, 0.0, g.start),
                        Vec3::new(g.lat + hw * s, 0.0, g.end),
                    ),
                };
                merge_shifted(
                    &mut sides,
                    wall_box(a, b, g.floor_y, g.height, CORRIDOR_WALL_THICKNESS),
                    Vec3::ZERO,
                );
            }
            out.material_walls.push((sides.0, sides.1, col));
            // Lid: same span as the floor, dropped 1 cm below the tube top (the same z-fight guard
            // against a zone ceiling at an equal height). A GLASS lid rides the transparent
            // always-visible ceiling pass EXACTLY like a glass zone roof; an opaque lid joins the
            // show-roof-gated opaque pass like a steel zone roof.
            let lid = floor_quad(
                Vec3::new(fx, g.floor_y + g.height - CORRIDOR_SURFACE_EPS, fz),
                Vec3::new(sx, 0.0, sz),
            );
            if g.glass_top {
                merge_shifted(&mut out.ceilings, lid, Vec3::ZERO);
            } else {
                merge_shifted(&mut out.ceilings_opaque, lid, Vec3::ZERO);
            }
            // Walkable bound: the tube registers as a "room" so the player mid-hallway is inside.
            let (center, dims) = match g.axis {
                CorridorAxis::X => (
                    Vec3::new((g.start + g.end) * 0.5, g.floor_y + g.height * 0.5, g.lat),
                    Vec3::new(len, g.height, g.width),
                ),
                CorridorAxis::Z => (
                    Vec3::new(g.lat, g.floor_y + g.height * 0.5, (g.start + g.end) * 0.5),
                    Vec3::new(g.width, g.height, len),
                ),
            };
            out.room_info.push(RoomInfo {
                id: format!("corridor_{ci}"),
                center,
                dimensions: dims,
                is_hologram_room: false,
                is_spawn_room: false,
            });
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
            corridors: Vec::new(),
        }
    }

    /// A zone body with one wall along its x = `wall_x` line (running +Z from z1 to z2) carrying a
    /// single door centred at `door_z` -- the corridor-test workhorse (doors facing along X).
    fn body_with_door(w: f32, d: f32, h: f32, wall_x: f32, z1: f32, z2: f32, door_z: f32, door_w: f32) -> HomeStructure {
        let mut b = body(w, d, h);
        let at = door_z - z1 - door_w * 0.5;
        b.walls = vec![ron::from_str(&format!(
            "(a: ({wall_x}, {z1}), b: ({wall_x}, {z2}), height: {h}, material: 1, openings: [\
             (kind: Door, at: {at}, width: {door_w}, sill: 0.0, height: 2.1, style: \"swing\", \
             open_dist: 2.6, locked: false, auto_open: true, control_panel: false, locks: [])])"
        ))
        .expect("door wall literal parses")];
        b
    }

    /// Two 10 x 10 zones, 10 m apart along +X, each with a door ON its facing perimeter edge at
    /// world z = 5: home's door on its x = 10 edge, commons' door on its x = 0 edge (world x = 20).
    fn corridor_ship() -> ShipStructure {
        ShipStructure {
            zones: vec![
                ShipZone {
                    id: "home".to_string(),
                    label: "Player Home".to_string(),
                    purpose: "residence".to_string(),
                    origin: (0.0, 0.0, 0.0),
                    body: body_with_door(10.0, 10.0, 3.0, 10.0, 3.0, 7.0, 5.0, 1.0),
                },
                ShipZone {
                    id: "commons".to_string(),
                    label: "The Commons".to_string(),
                    purpose: "commons".to_string(),
                    origin: (20.0, 0.0, 2.0),
                    // Door at LOCAL z = 3 -> world z = 5 (origin.z = 2), on the x = 0 edge.
                    body: body_with_door(8.0, 8.0, 6.0, 0.0, 1.0, 5.0, 3.0, 1.0),
                },
            ],
            corridors: vec![ShipCorridor {
                from_zone: "home".to_string(),
                from_opening: 0,
                to_zone: "commons".to_string(),
                to_opening: 0,
                width: 3.0,
                glass_top: false,
            }],
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
        let none = ShipStructure { zones: Vec::new(), corridors: Vec::new() };
        assert!(none.validate().is_err(), "no zones");
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

    // ── Increment B: generated corridors ──────────────────────────────────────────────────────

    #[test]
    fn a_corridors_less_ron_loads_with_the_serde_default() {
        // A pre-B file (no `corridors` field at all) must keep loading -- empty corridor list.
        let ship: ShipStructure = ron::from_str(
            "(zones: [(id: \"home\", body: (width: 10.0, depth: 10.0, height: 3.0))])",
        )
        .expect("a corridors-less RON parses");
        assert!(ship.corridors.is_empty(), "serde default fills an empty corridor list");
        assert!(ship.validate().is_ok());
    }

    #[test]
    fn corridors_round_trip_through_ron() {
        let ship = corridor_ship();
        let dir = temp_path("corridor_roundtrip");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ship_structure.ron");
        ship.save(&path).expect("saves");
        let back = ShipStructure::load(&path).expect("loads back (corridors validate)");
        assert_eq!(back.corridors.len(), 1);
        assert_eq!(back.corridors[0].from_zone, "home");
        assert_eq!(back.corridors[0].to_zone, "commons");
        assert!((back.corridors[0].width - 3.0).abs() < 1e-6);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corridor_validation_rejects_bad_references() {
        // Unknown zone.
        let mut ship = corridor_ship();
        ship.corridors[0].to_zone = "nowhere".to_string();
        let e = ship.validate().unwrap_err();
        assert!(e.contains("unknown zone 'nowhere'"), "got: {e}");
        // Out-of-range door index (each test zone has exactly one door).
        let mut ship = corridor_ship();
        ship.corridors[0].from_opening = 5;
        let e = ship.validate().unwrap_err();
        assert!(e.contains("out of range"), "got: {e}");
        // Same zone on both ends.
        let mut ship = corridor_ship();
        ship.corridors[0].to_zone = "home".to_string();
        let e = ship.validate().unwrap_err();
        assert!(e.contains("itself"), "got: {e}");
    }

    #[test]
    fn corridor_validation_rejects_non_facing_openings() {
        // Slide the commons sideways so the two door centres are offset 4 m laterally (> width/4).
        let mut ship = corridor_ship();
        ship.zones[1].origin.2 += 4.0;
        let e = ship.validate().unwrap_err();
        assert!(e.contains("sideways"), "lateral offset rejected honestly, got: {e}");
        // Turn the from door's wall PARALLEL to the run (a wall along +X at z = 5, so the door
        // centre stays on the corridor line at world (5, 0, 5) but faces +Z): the corridor still
        // runs along X, but the door no longer faces down it.
        let mut ship = corridor_ship();
        ship.zones[0].body.walls = vec![ron::from_str(
            "(a: (3.0, 5.0), b: (7.0, 5.0), height: 3.0, material: 1, openings: [\
             (kind: Door, at: 1.5, width: 1.0, sill: 0.0, height: 2.1, style: \"swing\", \
             open_dist: 2.6, locked: false, auto_open: true, control_panel: false, locks: [])])",
        )
        .unwrap()];
        let e = ship.validate().unwrap_err();
        assert!(e.contains("not perpendicular"), "misaligned wall rejected honestly, got: {e}");
        // Different deck heights.
        let mut ship = corridor_ship();
        ship.zones[1].origin.1 = 2.5;
        let e = ship.validate().unwrap_err();
        assert!(e.contains("deck heights"), "got: {e}");
    }

    #[test]
    fn corridor_geometry_endpoints_match_the_openings_world_positions() {
        let ship = corridor_ship();
        let g = ship.corridor_geometry(&ship.corridors[0]).expect("valid corridor resolves");
        // Home door: wall x = 10, door centre z = 5, origin (0,0,0) -> world (10, 0, 5).
        assert_eq!(g.end_from, Vec3::new(10.0, 0.0, 5.0));
        // Commons door: wall x = 0 local, centre z = 3 local, origin (20, 0, 2) -> world (20, 0, 5).
        assert_eq!(g.end_to, Vec3::new(20.0, 0.0, 5.0));
        assert_eq!(g.axis, CorridorAxis::X);
        assert!((g.start - 10.0).abs() < 1e-4 && (g.end - 20.0).abs() < 1e-4);
        assert!((g.lat - 5.0).abs() < 1e-4);
        assert!((g.height - 3.0).abs() < 1e-4, "the SHORTER zone's height (3 vs 6), got {}", g.height);
    }

    #[test]
    fn corridor_tube_meshes_span_between_the_zones() {
        let ship = corridor_ship();
        let m = ship.generate_meshes();
        // 2 zone floors + 1 corridor floor slab.
        assert_eq!(m.floors.len(), 3, "each corridor adds one floor slab");
        let (cv, _, _, _) = &m.floors[2];
        let xs: Vec<f32> = cv.iter().map(|v| v.position[0]).collect();
        let zs: Vec<f32> = cv.iter().map(|v| v.position[2]).collect();
        let (min_x, max_x) = (xs.iter().cloned().fold(f32::MAX, f32::min), xs.iter().cloned().fold(f32::MIN, f32::max));
        let (min_z, max_z) = (zs.iter().cloned().fold(f32::MAX, f32::min), zs.iter().cloned().fold(f32::MIN, f32::max));
        assert!((min_x - 10.0).abs() < 1e-3 && (max_x - 20.0).abs() < 1e-3, "floor spans opening to opening, got x {min_x}..{max_x}");
        assert!((min_z - 3.5).abs() < 1e-3 && (max_z - 6.5).abs() < 1e-3, "floor spans the 3 m width about z = 5, got z {min_z}..{max_z}");
        // The walkable bound registers, centred mid-tube.
        let cr = m.room_info.iter().find(|r| r.id == "corridor_0").expect("corridor room bound");
        assert!((cr.center.x - 15.0).abs() < 1e-3 && (cr.center.z - 5.0).abs() < 1e-3);
        assert_eq!(cr.dimensions, Vec3::new(10.0, 3.0, 3.0));
        assert!(!cr.is_spawn_room);
    }

    #[test]
    fn corridor_glass_top_picks_the_transparent_ceiling_pass() {
        // Opaque lid (glass_top: false) -> ceilings_opaque gains the lid quad.
        let ship = corridor_ship();
        let opaque_before = ship.generate_meshes();
        // Both test zones have GLASS roofs (default), so all ceilings_opaque geometry is the lid.
        assert_eq!(opaque_before.ceilings_opaque.0.len(), 4, "the opaque lid is one quad");
        // Glass lid -> it moves to the transparent `ceilings` family instead.
        let mut ship = corridor_ship();
        ship.corridors[0].glass_top = true;
        let m = ship.generate_meshes();
        assert!(m.ceilings_opaque.0.is_empty(), "no opaque lid when glass_top");
        assert_eq!(
            m.ceilings.0.len(),
            opaque_before.ceilings.0.len() + 4,
            "the glass lid joins the zone glass roofs' transparent pass"
        );
        // The lid quad sits at the tube top (the shorter zone's height, minus the z-fight guard),
        // spanning the run -- the ceilings family also holds the ZONE glass roofs, so look for the
        // lid's verts specifically (x inside the 10..20 run at y = 3.0 - 0.01).
        let lid_verts = m
            .ceilings
            .0
            .iter()
            .filter(|v| v.position[0] > 10.0 - 1e-3 && v.position[0] < 20.0 + 1e-3 && (v.position[1] - 2.99).abs() < 1e-3)
            .count();
        assert_eq!(lid_verts, 4, "the glass lid quad sits at height 3.0 - 0.01 over the run");
    }

    #[test]
    fn shell_cuts_open_the_perimeter_where_the_tube_meets_each_zone() {
        let ship = corridor_ship();
        // Home (zone 0): the tube leaves through its x = w (edge 1) face; the cut is door-sized
        // (1 m wide about z = 5 -> at = 4.5) and door-height.
        let cuts = ship.shell_cuts_for_zone(0);
        assert_eq!(cuts.len(), 1);
        assert_eq!(cuts[0].edge, 1);
        assert!((cuts[0].at - 4.5).abs() < 1e-4, "got {}", cuts[0].at);
        assert!((cuts[0].width - 1.0).abs() < 1e-4);
        assert!((cuts[0].height - 2.1).abs() < 1e-4);
        // Commons (zone 1): the tube enters through its x = 0 (edge 3) face. Edge 3 runs from
        // (0, d) to (0, 0), so at = d - (local z + w/2) = 8 - 3.5 = 4.5.
        let cuts = ship.shell_cuts_for_zone(1);
        assert_eq!(cuts.len(), 1);
        assert_eq!(cuts[0].edge, 3);
        assert!((cuts[0].at - 4.5).abs() < 1e-4, "got {}", cuts[0].at);
        // A zone the corridor never touches gets no cuts.
        let mut ship3 = corridor_ship();
        ship3.zones.push(zone("bay", (0.0, 0.0, 40.0), 10.0, 10.0, 3.0));
        assert!(ship3.shell_cuts_for_zone(2).is_empty());
    }

    #[test]
    fn removing_a_zone_drops_its_corridors_and_prune_drops_broken_rows() {
        let mut ship = corridor_ship();
        // remove_zone("commons") takes its corridor with it.
        let commons = ship.zone_index("commons").unwrap();
        assert!(ship.remove_zone(commons));
        assert!(ship.corridors.is_empty(), "the dangling corridor went with its zone");
        // prune_invalid_corridors drops a row whose door vanished, keeps the valid shape.
        let mut ship = corridor_ship();
        assert_eq!(ship.prune_invalid_corridors(), 0, "a valid corridor is kept");
        ship.zones[0].body.walls.clear(); // the from door is gone
        assert_eq!(ship.prune_invalid_corridors(), 1, "the broken row is dropped");
        assert!(ship.corridors.is_empty());
        assert!(ship.validate().is_ok(), "post-prune the ship always validates");
    }

    #[test]
    fn zone_door_refs_skip_windows_and_keep_declaration_order() {
        let mut b = body_with_door(10.0, 10.0, 3.0, 10.0, 3.0, 7.0, 5.0, 1.0);
        // Prepend a WINDOW on the same wall: door indexing must skip it.
        let win: crate::ship::home_structure::Opening = ron::from_str(
            "(kind: Window, at: 0.2, width: 0.8, sill: 1.0, height: 1.2, style: \"fixed\", \
             open_dist: 2.6, locked: false, auto_open: true, control_panel: false, locks: [])",
        )
        .unwrap();
        b.walls[0].openings.insert(0, win);
        let doors = zone_door_refs(&b);
        assert_eq!(doors.len(), 1, "only the door counts");
        assert_eq!(doors[0].opening_index, 1, "the raw opening index is preserved for display");
        assert!((doors[0].center.0 - 10.0).abs() < 1e-4 && (doors[0].center.1 - 5.0).abs() < 1e-4);
    }
}
